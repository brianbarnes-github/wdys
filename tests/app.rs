use std::cell::{Cell, RefCell};
use std::path::Path;
use std::rc::Rc;
use std::sync::mpsc::{self, Sender};
use std::time::Duration;

use wdys::app::{run_worker, App, Cue, TranscriptStore, TrayState, UiEvent};
use wdys::audio::{AudioClip, Recorder};
use wdys::config::{Config, VirtualKey};
use wdys::error::{AudioError, PasteError, TranscribeError};
use wdys::hotkey::{Input, Millis, State};
use wdys::paste::{Clipboard, Keyboard};
use wdys::transcribe::{LazyModel, ModelLoader, SpeechModel};

use Input::*;

type Shared<T> = Rc<RefCell<T>>;
type Wake = (Input, Millis);

fn tone(seconds: f32) -> AudioClip {
    let n = (16_000.0 * seconds) as usize;
    AudioClip {
        samples: (0..n)
            .map(|i| 0.5 * (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 16_000.0).sin())
            .collect(),
    }
}

#[derive(Default)]
struct RecLog {
    starts: u32,
    stops: u32,
    silent: bool,
    fail_next_start: bool,
    fail_next_stop: bool,
    lost: bool,
}

struct FakeRecorder(Shared<RecLog>);

impl Recorder for FakeRecorder {
    fn start(&mut self) -> Result<(), AudioError> {
        let mut log = self.0.borrow_mut();
        if std::mem::take(&mut log.fail_next_start) {
            return Err(AudioError::NoInputDevice);
        }
        log.starts += 1;
        Ok(())
    }
    fn is_lost(&self) -> bool {
        self.0.borrow().lost
    }
    fn stop(&mut self) -> Result<AudioClip, AudioError> {
        let mut log = self.0.borrow_mut();
        log.stops += 1;
        if std::mem::take(&mut log.fail_next_stop) || log.lost {
            return Err(AudioError::DeviceLost);
        }
        Ok(if log.silent {
            AudioClip {
                samples: vec![0.0; 16_000],
            }
        } else {
            tone(1.0)
        })
    }
}

struct FakeSpeech {
    text: String,
    calls: Rc<Cell<u32>>,
    extra: Option<Sender<Wake>>,
}

impl SpeechModel for FakeSpeech {
    fn transcribe(&mut self, _clip: &AudioClip) -> Result<String, TranscribeError> {
        self.calls.set(self.calls.get() + 1);
        if let Some(tx) = self.extra.take() {
            tx.send((KeyDown, 5_000)).unwrap();
            tx.send((KeyUp, 5_500)).unwrap();
        }
        Ok(self.text.clone())
    }
}

struct FakeLoader {
    text: String,
    loads: Rc<Cell<u32>>,
    calls: Rc<Cell<u32>>,
    extra: RefCell<Option<Sender<Wake>>>,
}

impl ModelLoader for FakeLoader {
    fn load(&self, _path: &Path) -> Result<Box<dyn SpeechModel>, TranscribeError> {
        self.loads.set(self.loads.get() + 1);
        Ok(Box::new(FakeSpeech {
            text: self.text.clone(),
            calls: self.calls.clone(),
            extra: self.extra.borrow_mut().take(),
        }))
    }
}

struct FakeClipboard {
    content: Shared<Option<String>>,
    writes: Shared<Vec<String>>,
}

impl Clipboard for FakeClipboard {
    fn get_text(&mut self) -> Result<Option<String>, PasteError> {
        Ok(self.content.borrow().clone())
    }
    fn set_text(&mut self, text: &str, _exclude: bool) -> Result<(), PasteError> {
        self.writes.borrow_mut().push(text.to_string());
        *self.content.borrow_mut() = Some(text.to_string());
        Ok(())
    }
    fn clear(&mut self) -> Result<(), PasteError> {
        *self.content.borrow_mut() = None;
        Ok(())
    }
}

struct FakeKeyboard {
    content: Shared<Option<String>>,
    pasted: Shared<Vec<String>>,
    fail_paste: Rc<Cell<bool>>,
}

impl Keyboard for FakeKeyboard {
    fn send_paste(&mut self) -> Result<(), PasteError> {
        if self.fail_paste.get() {
            return Err(PasteError::Input("blocked".into()));
        }
        self.pasted
            .borrow_mut()
            .push(self.content.borrow().clone().unwrap_or_default());
        Ok(())
    }
    fn is_down(&self, _key: VirtualKey) -> bool {
        false
    }
}

struct Rig {
    app: App<FakeRecorder, FakeClipboard, FakeKeyboard>,
    rec: Shared<RecLog>,
    loads: Rc<Cell<u32>>,
    model_calls: Rc<Cell<u32>>,
    content: Shared<Option<String>>,
    writes: Shared<Vec<String>>,
    pasted: Shared<Vec<String>>,
    store: TranscriptStore,
    fail_paste: Rc<Cell<bool>>,
}

fn rig_with(transcript: &str, config: Config, extra: Option<Sender<Wake>>) -> Rig {
    let rec = Rc::new(RefCell::new(RecLog::default()));
    let (loads, model_calls) = (Rc::new(Cell::new(0)), Rc::new(Cell::new(0)));
    let content = Rc::new(RefCell::new(Some("previous".to_string())));
    let (writes, pasted) = (
        Rc::new(RefCell::new(Vec::new())),
        Rc::new(RefCell::new(Vec::new())),
    );
    let loader = FakeLoader {
        text: transcript.to_string(),
        loads: loads.clone(),
        calls: model_calls.clone(),
        extra: RefCell::new(extra),
    };
    let idle = Millis::from(config.idle_unload_secs) * 1000;
    let model = LazyModel::new(Box::new(loader), "m.bin".into(), idle);
    let store = TranscriptStore::default();
    let fail_paste = Rc::new(Cell::new(false));
    let app = App::new(
        config,
        FakeRecorder(rec.clone()),
        model,
        FakeClipboard {
            content: content.clone(),
            writes: writes.clone(),
        },
        FakeKeyboard {
            content: content.clone(),
            pasted: pasted.clone(),
            fail_paste: fail_paste.clone(),
        },
        Box::new(|_: Duration| {}),
    )
    .with_transcript_store(store.clone());
    Rig {
        app,
        rec,
        loads,
        model_calls,
        content,
        writes,
        pasted,
        store,
        fail_paste,
    }
}

fn rig(transcript: &str) -> Rig {
    rig_with(transcript, Config::default(), None)
}

fn play(rig: &mut Rig, events: &[Wake]) -> Vec<UiEvent> {
    let mut out = Vec::new();
    for &(input, now) in events {
        rig.app.handle(input, now, &mut |e| out.push(e));
    }
    out
}

#[test]
fn hold_speak_release_pastes_cleaned_text() {
    let mut r = rig("Um, hello world");
    play(&mut r, &[(KeyDown, 0), (KeyUp, 1_000)]);
    assert_eq!(*r.pasted.borrow(), vec!["Hello world".to_string()]);
    assert_eq!(*r.content.borrow(), Some("previous".to_string()));
    assert_eq!(r.rec.borrow().starts, 1);
}

#[test]
fn double_press_hands_free_pastes_on_next_press() {
    let mut r = rig("hello");
    play(
        &mut r,
        &[(KeyDown, 0), (KeyUp, 100), (KeyDown, 300), (KeyUp, 400)],
    );
    assert!(r.pasted.borrow().is_empty());
    play(&mut r, &[(KeyDown, 5_000)]);
    assert_eq!(*r.pasted.borrow(), vec!["hello".to_string()]);
    assert_eq!(r.rec.borrow().starts, 2);
}

#[test]
fn short_tap_pastes_nothing_and_cancels_recording() {
    let mut r = rig("hello");
    play(&mut r, &[(KeyDown, 0), (KeyUp, 100)]);
    assert!(r.pasted.borrow().is_empty());
    assert_eq!((r.rec.borrow().starts, r.rec.borrow().stops), (1, 1));
    assert_eq!(r.loads.get(), 0);
}

#[test]
fn silent_clip_pastes_nothing_and_skips_the_model() {
    let mut r = rig("hello");
    r.rec.borrow_mut().silent = true;
    play(&mut r, &[(KeyDown, 0), (KeyUp, 1_000)]);
    assert!(r.pasted.borrow().is_empty());
    assert_eq!(r.model_calls.get(), 0);
}

#[test]
fn filler_only_transcript_pastes_nothing_and_leaves_clipboard_alone() {
    let mut r = rig("Um, uh. [BLANK_AUDIO]");
    play(&mut r, &[(KeyDown, 0), (KeyUp, 1_000)]);
    assert!(r.pasted.borrow().is_empty());
    assert!(r.writes.borrow().is_empty());
    assert_eq!(*r.content.borrow(), Some("previous".to_string()));
}

#[test]
fn max_length_tick_pastes() {
    let mut r = rig("hello");
    play(&mut r, &[(KeyDown, 0), (Tick, 120_000)]);
    assert_eq!(*r.pasted.borrow(), vec!["hello".to_string()]);
}

#[test]
fn input_during_transcription_is_dropped() {
    let (tx, rx) = mpsc::channel();
    let mut r = rig_with("hello", Config::default(), Some(tx.clone()));
    tx.send((KeyDown, 0)).unwrap();
    tx.send((KeyUp, 1_000)).unwrap();
    drop(tx);
    let events = RefCell::new(Vec::new());
    run_worker(rx, &mut r.app, &|e| events.borrow_mut().push(e));
    assert_eq!(
        r.rec.borrow().starts,
        1,
        "queued key events must not start a second recording"
    );
    assert_eq!(r.app.state(), State::Idle);
    assert_eq!(*r.pasted.borrow(), vec!["hello".to_string()]);
}

#[test]
fn recorder_start_failure_notifies_and_recovers() {
    let mut r = rig("hello");
    r.rec.borrow_mut().fail_next_start = true;
    let events = play(&mut r, &[(KeyDown, 0)]);
    assert!(events.iter().any(|e| matches!(e, UiEvent::Notify(_))));
    assert_eq!(r.app.state(), State::Idle);
    play(&mut r, &[(KeyUp, 10), (KeyDown, 100), (KeyUp, 1_500)]);
    assert_eq!(*r.pasted.borrow(), vec!["hello".to_string()]);
}

#[test]
fn recorder_stop_failure_notifies_and_pastes_nothing() {
    let mut r = rig("hello");
    r.rec.borrow_mut().fail_next_stop = true;
    let events = play(&mut r, &[(KeyDown, 0), (KeyUp, 1_000)]);
    assert!(events.iter().any(|e| matches!(e, UiEvent::Notify(_))));
    assert!(r.pasted.borrow().is_empty());
    assert_eq!(events.last(), Some(&UiEvent::Tray(TrayState::Idle)));
}

#[test]
fn tray_states_progress_idle_recording_transcribing_idle() {
    let mut r = rig("hello");
    let events = play(&mut r, &[(KeyDown, 0), (KeyUp, 1_000)]);
    let tray: Vec<_> = events
        .iter()
        .filter_map(|e| {
            if let UiEvent::Tray(t) = e {
                Some(*t)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(
        tray,
        vec![
            TrayState::Recording,
            TrayState::Transcribing,
            TrayState::Idle
        ]
    );
    assert!(events.contains(&UiEvent::Cue(Cue::Start)));
    assert!(events.contains(&UiEvent::Cue(Cue::Stop)));
}

#[test]
fn hands_free_entry_shows_hands_free_tray_state() {
    let mut r = rig("hello");
    let events = play(&mut r, &[(KeyDown, 0), (KeyUp, 100), (KeyDown, 300)]);
    assert_eq!(events.last(), Some(&UiEvent::Tray(TrayState::HandsFree)));
}

#[test]
fn sounds_off_emits_no_cues() {
    let mut r = rig_with(
        "hello",
        Config {
            sounds: false,
            ..Config::default()
        },
        None,
    );
    let events = play(&mut r, &[(KeyDown, 0), (KeyUp, 1_000)]);
    assert!(!events.iter().any(|e| matches!(e, UiEvent::Cue(_))));
}

#[test]
fn idle_tick_unloads_model_and_next_use_reloads_it() {
    let mut r = rig("hello");
    play(&mut r, &[(KeyDown, 0), (KeyUp, 1_000)]);
    assert_eq!(r.loads.get(), 1);
    play(&mut r, &[(Tick, 1_000 + 600_000)]);
    play(&mut r, &[(KeyDown, 700_000), (KeyUp, 701_000)]);
    assert_eq!(r.loads.get(), 2);
}

#[test]
fn device_lost_mid_recording_ends_the_recording_on_the_next_tick() {
    let mut r = rig("hello");
    play(&mut r, &[(KeyDown, 0)]);
    r.rec.borrow_mut().lost = true;
    let events = play(&mut r, &[(Tick, 200)]);
    assert!(events.iter().any(|e| matches!(e, UiEvent::Notify(_))));
    assert_eq!(r.rec.borrow().stops, 1);
    assert!(r.pasted.borrow().is_empty());
    assert_eq!(events.last(), Some(&UiEvent::Tray(TrayState::Idle)));
    // the key is still physically held; its release must not start anything
    play(&mut r, &[(KeyUp, 300)]);
    assert_eq!(r.app.state(), State::Idle);
    assert_eq!(r.rec.borrow().starts, 1);
}

#[test]
fn device_lost_during_hands_free_ends_the_recording_too() {
    let mut r = rig("hello");
    play(
        &mut r,
        &[(KeyDown, 0), (KeyUp, 100), (KeyDown, 300), (KeyUp, 400)],
    );
    r.rec.borrow_mut().lost = true;
    let events = play(&mut r, &[(Tick, 500)]);
    assert!(events.iter().any(|e| matches!(e, UiEvent::Notify(_))));
    assert_eq!(r.app.state(), State::Idle);
}

#[test]
fn errors_make_an_audible_error_cue() {
    let mut r = rig("hello");
    r.rec.borrow_mut().fail_next_start = true;
    let events = play(&mut r, &[(KeyDown, 0)]);
    assert!(events.contains(&UiEvent::Cue(Cue::Error)));
}

#[test]
fn error_cue_respects_the_sounds_setting() {
    let mut r = rig_with(
        "hello",
        Config {
            sounds: false,
            ..Config::default()
        },
        None,
    );
    r.rec.borrow_mut().fail_next_start = true;
    let events = play(&mut r, &[(KeyDown, 0)]);
    assert!(!events.iter().any(|e| matches!(e, UiEvent::Cue(_))));
}

fn remembered(r: &Rig) -> Option<String> {
    r.store.lock().unwrap().clone()
}

#[test]
fn the_cleaned_transcript_is_remembered_for_copy_last() {
    let mut r = rig("Um, hello world");
    play(&mut r, &[(KeyDown, 0), (KeyUp, 1_000)]);
    assert_eq!(remembered(&r), Some("Hello world".to_string()));
}

#[test]
fn a_failed_paste_keeps_the_text_on_the_clipboard_and_remembered_and_says_so() {
    let mut r = rig("hello");
    r.fail_paste.set(true);
    let events = play(&mut r, &[(KeyDown, 0), (KeyUp, 1_000)]);
    assert_eq!(*r.content.borrow(), Some("hello".to_string()));
    assert_eq!(remembered(&r), Some("hello".to_string()));
    assert!(events
        .iter()
        .any(|e| matches!(e, UiEvent::Notify(m) if m.contains("clipboard"))));
}

#[test]
fn nothing_is_remembered_for_silent_or_filler_only_recordings() {
    let mut r = rig("Um, uh.");
    play(&mut r, &[(KeyDown, 0), (KeyUp, 1_000)]);
    assert_eq!(remembered(&r), None);
    let mut silent = rig("hello");
    silent.rec.borrow_mut().silent = true;
    play(&mut silent, &[(KeyDown, 0), (KeyUp, 1_000)]);
    assert_eq!(remembered(&silent), None);
}

#[test]
fn an_empty_recording_does_not_erase_the_previous_transcript() {
    let mut r = rig("hello");
    play(&mut r, &[(KeyDown, 0), (KeyUp, 1_000)]);
    r.rec.borrow_mut().silent = true;
    play(&mut r, &[(KeyDown, 2_000), (KeyUp, 3_000)]);
    assert_eq!(remembered(&r), Some("hello".to_string()));
}

#[test]
fn a_new_transcript_replaces_the_previous_one() {
    let mut r = rig("fresh words");
    *r.store.lock().unwrap() = Some("stale words".to_string());
    play(&mut r, &[(KeyDown, 0), (KeyUp, 1_000)]);
    assert_eq!(remembered(&r), Some("fresh words".to_string()));
}
