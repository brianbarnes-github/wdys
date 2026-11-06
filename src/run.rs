//! Windows entry point: wires the portable core to the Win32 layer.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Sender};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use tray_icon::menu::{MenuEvent, MenuId};
use windows::core::HSTRING;
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, MessageBoxW, PostThreadMessageW, TranslateMessage, IDYES, MB_ICONINFORMATION,
    MB_ICONQUESTION, MB_OK, MB_TOPMOST, MB_YESNO, MSG, WM_APP,
};

use wdys::app::{run_worker, App, Cue, UiEvent};
use wdys::config::{self, Config, ModelSize, VirtualKey};
use wdys::error::{InstanceError, TranscribeError};
use wdys::hotkey::{Input, Millis};
use wdys::model_store::{ensure_model, model_path, model_sha256, Consent, UreqDownloader};
use wdys::platform::autostart;
use wdys::platform::clipboard::WinClipboard;
use wdys::platform::cues::play_cue;
use wdys::platform::hook::{capture_next_key, install_hook, spawn_ticker, KeyTracker};
use wdys::platform::instance;
use wdys::platform::keyboard::WinKeyboard;
use wdys::platform::recorder::CpalRecorder;
use wdys::platform::tray::{MenuAction, TrayUi};
use wdys::transcribe::{LazyModel, ModelLoader, SpeechModel};

const TICK_INTERVAL: Duration = Duration::from_millis(200);
const HOTKEY_CAPTURE_TIMEOUT: Duration = Duration::from_secs(10);

/// Messages for the main (tray) thread.
enum MainMsg {
    Ui(UiEvent),
    Info(String),
    Menu(MenuId),
    HotkeyChosen(VirtualKey),
}

/// Posts to the main thread so it wakes from `GetMessageW` and drains its channel.
#[derive(Clone)]
struct Mailbox {
    tx: Sender<MainMsg>,
    thread_id: u32,
}

impl Mailbox {
    fn send(&self, msg: MainMsg) {
        let _ = self.tx.send(msg);
        // SAFETY: waking a thread id that belongs to this process; failure just means it already exited.
        unsafe {
            let _ = PostThreadMessageW(self.thread_id, WM_APP, WPARAM(0), LPARAM(0));
        }
    }
}

fn message_box(text: &str, style: windows::Win32::UI::WindowsAndMessaging::MESSAGEBOX_STYLE) -> bool {
    // SAFETY: plain modal message box with no owner window.
    let answer = unsafe { MessageBoxW(Some(HWND::default()), &HSTRING::from(text), &HSTRING::from("wdys"), style | MB_TOPMOST) };
    answer == IDYES
}

struct DialogConsent;

impl Consent for DialogConsent {
    fn allow_download(&self, size: ModelSize, approx_bytes: u64) -> bool {
        message_box(
            &format!(
                "wdys needs to download its speech model ({size:?}, about {} MB) from huggingface.co.\n\n\
                 This is the only time wdys uses the network. Your voice never leaves this computer.\n\n\
                 Download now?",
                approx_bytes / 1_000_000
            ),
            MB_YESNO | MB_ICONQUESTION,
        )
    }
}

#[cfg(feature = "whisper")]
fn make_loader() -> Box<dyn ModelLoader> {
    Box::new(wdys::whisper::WhisperLoader)
}

#[cfg(not(feature = "whisper"))]
fn make_loader() -> Box<dyn ModelLoader> {
    struct Unavailable;
    impl ModelLoader for Unavailable {
        fn load(&self, _path: &Path) -> Result<Box<dyn SpeechModel>, TranscribeError> {
            Err(TranscribeError::Load("this build has no speech engine (built without the `whisper` feature)".into()))
        }
    }
    Box::new(Unavailable)
}

fn app_dir() -> PathBuf {
    std::env::var_os("APPDATA").map_or_else(|| PathBuf::from("."), PathBuf::from).join("wdys")
}

/// Runs on its own thread: owns the recorder (not `Send`), the model and the paste machinery.
fn spawn_worker(
    config: Config,
    models_dir: PathBuf,
    tracker: Arc<KeyTracker>,
    rx: mpsc::Receiver<(Input, Millis)>,
    mailbox: Mailbox,
) {
    thread::spawn(move || {
        let path = ensure_model(&models_dir, config.model, &UreqDownloader, &DialogConsent, model_sha256(config.model))
            .unwrap_or_else(|e| {
                mailbox.send(MainMsg::Ui(UiEvent::Notify(format!("Speech model unavailable: {e}"))));
                if config.sounds {
                    play_cue(Cue::Error);
                }
                model_path(&models_dir, config.model)
            });
        let idle = Millis::from(config.idle_unload_secs) * 1000;
        let model = LazyModel::new(make_loader(), path, idle);
        let mut app = App::new(
            config,
            CpalRecorder::default(),
            model,
            WinClipboard,
            WinKeyboard::new(tracker),
            Box::new(thread::sleep),
        );
        run_worker(rx, &mut app, &|event| match event {
            UiEvent::Cue(cue) => play_cue(cue),
            other => mailbox.send(MainMsg::Ui(other)),
        });
    });
}

fn save_config(path: &Path, config: &Config, mailbox: &Mailbox) {
    if let Err(e) = config::save(path, config) {
        mailbox.send(MainMsg::Info(format!("Could not save settings: {e}")));
    }
}

pub fn main() {
    let _instance = match instance::acquire() {
        Ok(guard) => guard,
        Err(InstanceError::AlreadyRunning) => {
            message_box("wdys is already running (look for it in the system tray).", MB_OK | MB_ICONINFORMATION);
            return;
        }
        Err(e) => {
            message_box(&format!("wdys could not start: {e}"), MB_OK | MB_ICONINFORMATION);
            return;
        }
    };

    let dir = app_dir();
    let config_path = dir.join("config.toml");
    let mut config = config::load(&config_path).unwrap_or_else(|e| {
        message_box(&format!("Your config file could not be read, using defaults.\n\n{e}"), MB_OK | MB_ICONINFORMATION);
        Config::default()
    });

    // SAFETY: GetCurrentThreadId has no preconditions.
    let (main_tx, main_rx) = mpsc::channel();
    let mailbox = Mailbox { tx: main_tx, thread_id: unsafe { GetCurrentThreadId() } };
    let (input_tx, input_rx) = mpsc::channel::<(Input, Millis)>();
    let tracker = KeyTracker::new(config.hotkey);

    let _hook = match install_hook(tracker.clone(), input_tx.clone()) {
        Ok(handle) => handle,
        Err(e) => {
            message_box(&format!("wdys could not install its keyboard hook: {e}"), MB_OK | MB_ICONINFORMATION);
            return;
        }
    };
    spawn_ticker(tracker.clone(), input_tx, TICK_INTERVAL);

    let mut tray = match TrayUi::new(&config) {
        Ok(tray) => tray,
        Err(e) => {
            message_box(&format!("wdys could not create its tray icon: {e}"), MB_OK | MB_ICONINFORMATION);
            return;
        }
    };
    let menu_mailbox = mailbox.clone();
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| menu_mailbox.send(MainMsg::Menu(event.id))));

    spawn_worker(config.clone(), dir.join("models"), tracker.clone(), input_rx, mailbox.clone());

    let mut msg = MSG::default();
    // SAFETY: standard Win32 message loop on the thread that owns the tray icon.
    while unsafe { GetMessageW(&mut msg, None, 0, 0) }.as_bool() {
        unsafe {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        while let Ok(message) = main_rx.try_recv() {
            match message {
                MainMsg::Ui(event) => tray.apply(&event),
                MainMsg::Info(text) => tray.show_info(&text),
                MainMsg::HotkeyChosen(key) => {
                    tracker.set_hotkey(key);
                    config.hotkey = key;
                    save_config(&config_path, &config, &mailbox);
                    tray.show_info(&format!("wdys: hotkey set (key code 0x{:X})", key.0));
                }
                MainMsg::Menu(id) => match tray.action_for(&id) {
                    Some(MenuAction::Quit) => return,
                    Some(MenuAction::SetHotkey) => {
                        tray.show_info("wdys: press the new hotkey now...");
                        let (tracker, mailbox) = (tracker.clone(), mailbox.clone());
                        thread::spawn(move || match capture_next_key(&tracker, HOTKEY_CAPTURE_TIMEOUT) {
                            Ok(key) => mailbox.send(MainMsg::HotkeyChosen(key)),
                            Err(e) => mailbox.send(MainMsg::Info(format!("wdys: {e}"))),
                        });
                    }
                    Some(MenuAction::UseModel(size)) => {
                        config.model = size;
                        tray.show_model(size);
                        save_config(&config_path, &config, &mailbox);
                        tray.show_info("wdys: restart to use the new model");
                    }
                    Some(MenuAction::ToggleStartup) => {
                        let wanted = tray.startup_checked();
                        match autostart::set(wanted) {
                            Ok(()) => {
                                config.start_with_windows = wanted;
                                save_config(&config_path, &config, &mailbox);
                            }
                            Err(e) => {
                                tray.set_startup_checked(!wanted);
                                tray.show_info(&format!("wdys: could not change startup setting: {e}"));
                            }
                        }
                    }
                    Some(MenuAction::OpenConfig) => {
                        if !config_path.exists() {
                            save_config(&config_path, &config, &mailbox);
                        }
                        let _ = std::process::Command::new("notepad").arg(&config_path).spawn();
                    }
                    None => {}
                },
            }
        }
    }
}
