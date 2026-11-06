//! Global low-level keyboard hook, the periodic ticker, and hotkey capture.
//!
//! The hook never blocks a key (it always calls the next hook) and ignores injected
//! events, so our own simulated Ctrl+V cannot feed back into the state machine.
//! It forwards exactly one `KeyDown` per physical press (auto-repeat is deduplicated).

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetMessageW, PostThreadMessageW, SetWindowsHookExW, UnhookWindowsHookEx, HC_ACTION,
    KBDLLHOOKSTRUCT, LLKHF_INJECTED, MSG, WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP, WM_QUIT, WM_SYSKEYDOWN,
    WM_SYSKEYUP,
};

use crate::config::VirtualKey;
use crate::error::HookError;
use crate::hotkey::{should_synthesize_keyup, Input, Millis};

/// State shared between the hook thread, the ticker and the tray.
pub struct KeyTracker {
    hotkey: AtomicU32,
    down: AtomicBool,
    started: Instant,
    capture: Mutex<Option<Sender<VirtualKey>>>,
}

impl KeyTracker {
    pub fn new(hotkey: VirtualKey) -> Arc<KeyTracker> {
        Arc::new(KeyTracker {
            hotkey: AtomicU32::new(hotkey.0),
            down: AtomicBool::new(false),
            started: Instant::now(),
            capture: Mutex::new(None),
        })
    }

    pub fn hotkey(&self) -> VirtualKey {
        VirtualKey(self.hotkey.load(Ordering::Relaxed))
    }

    pub fn set_hotkey(&self, key: VirtualKey) {
        self.hotkey.store(key.0, Ordering::Relaxed);
        self.down.store(false, Ordering::Relaxed);
    }

    /// Milliseconds since the tracker was created; the clock the state machine runs on.
    pub fn now(&self) -> Millis {
        self.started.elapsed().as_millis() as Millis
    }
}

struct HookContext {
    tracker: Arc<KeyTracker>,
    tx: Sender<(Input, Millis)>,
}

static CONTEXT: OnceLock<HookContext> = OnceLock::new();

/// Decides what a key event means for the app. Kept free of Win32 types.
fn on_key_event(ctx: &HookContext, vk: u32, is_down: bool) {
    let tracker = &ctx.tracker;
    if is_down {
        if let Ok(mut capture) = tracker.capture.lock() {
            if let Some(tx) = capture.take() {
                let _ = tx.send(VirtualKey(vk));
                return;
            }
        }
    }
    if vk != tracker.hotkey.load(Ordering::Relaxed) {
        return;
    }
    let was_down = tracker.down.swap(is_down, Ordering::Relaxed);
    let now = tracker.now();
    match (was_down, is_down) {
        (false, true) => {
            let _ = ctx.tx.send((Input::KeyDown, now));
        }
        (true, false) => {
            let _ = ctx.tx.send((Input::KeyUp, now));
        }
        _ => {}
    }
}

unsafe extern "system" fn hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 {
        // SAFETY: for HC_ACTION on WH_KEYBOARD_LL, lparam points to a valid KBDLLHOOKSTRUCT.
        let event = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
        if event.flags.0 & LLKHF_INJECTED.0 == 0 {
            if let Some(ctx) = CONTEXT.get() {
                match wparam.0 as u32 {
                    WM_KEYDOWN | WM_SYSKEYDOWN => on_key_event(ctx, event.vkCode, true),
                    WM_KEYUP | WM_SYSKEYUP => on_key_event(ctx, event.vkCode, false),
                    _ => {}
                }
            }
        }
    }
    // SAFETY: forwarding the unchanged arguments of the hook we are handling.
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

/// Keeps the hook alive; dropping it uninstalls the hook and stops its thread.
pub struct HookHandle {
    thread_id: u32,
}

impl Drop for HookHandle {
    fn drop(&mut self) {
        // SAFETY: posting WM_QUIT to a thread id we created; failure just means it already exited.
        unsafe {
            let _ = PostThreadMessageW(self.thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
        }
    }
}

/// Installs the global keyboard hook on its own thread (with the message loop it requires).
pub fn install_hook(tracker: Arc<KeyTracker>, tx: Sender<(Input, Millis)>) -> Result<HookHandle, HookError> {
    CONTEXT.set(HookContext { tracker, tx }).map_err(|_| HookError::AlreadyInstalled)?;
    let (ready_tx, ready_rx) = mpsc::channel::<Result<u32, HookError>>();
    thread::spawn(move || {
        // SAFETY: standard hook installation; the callback is a valid `extern "system"` fn
        // and this thread pumps messages for as long as the hook lives.
        unsafe {
            let installed = GetModuleHandleW(None)
                .map_err(|e| HookError::Install(e.to_string()))
                .and_then(|module| {
                    SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook_proc), Some(module.into()), 0)
                        .map_err(|e| HookError::Install(e.to_string()))
                });
            match installed {
                Ok(hook) => {
                    let _ = ready_tx.send(Ok(GetCurrentThreadId()));
                    let mut msg = MSG::default();
                    while GetMessageW(&mut msg, None, 0, 0).as_bool() {}
                    let _ = UnhookWindowsHookEx(hook);
                }
                Err(e) => {
                    let _ = ready_tx.send(Err(e));
                }
            }
        }
    });
    let thread_id = ready_rx.recv().map_err(|e| HookError::Install(e.to_string()))??;
    Ok(HookHandle { thread_id })
}

/// Sends `Tick` every `interval`, and a synthetic `KeyUp` if the OS reports the hotkey up
/// while we still believe it is held (a lost key-up must never leave recording stuck on).
pub fn spawn_ticker(tracker: Arc<KeyTracker>, tx: Sender<(Input, Millis)>, interval: Duration) {
    thread::spawn(move || loop {
        thread::sleep(interval);
        let believed_down = tracker.down.load(Ordering::Relaxed);
        // SAFETY: GetAsyncKeyState has no preconditions.
        let actually_down = unsafe { GetAsyncKeyState(tracker.hotkey().0 as i32) } < 0;
        let now = tracker.now();
        if should_synthesize_keyup(believed_down, actually_down) {
            tracker.down.store(false, Ordering::Relaxed);
            if tx.send((Input::KeyUp, now)).is_err() {
                return;
            }
        }
        if tx.send((Input::Tick, now)).is_err() {
            return;
        }
    });
}

/// Waits for the next key press (for the tray's "Set hotkey") and returns it.
pub fn capture_next_key(tracker: &KeyTracker, timeout: Duration) -> Result<VirtualKey, HookError> {
    let (tx, rx) = mpsc::channel();
    if let Ok(mut capture) = tracker.capture.lock() {
        *capture = Some(tx);
    }
    let result = rx.recv_timeout(timeout).map_err(|_| HookError::CaptureTimeout);
    if let Ok(mut capture) = tracker.capture.lock() {
        *capture = None;
    }
    result
}
