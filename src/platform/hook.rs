//! Global low-level keyboard hook, the periodic ticker, and hotkey capture.
//!
//! The hotkey is swallowed (it never reaches the focused app); every other key, and every
//! injected event such as our own simulated Ctrl+V, passes through. Exactly one `KeyDown` is
//! forwarded per physical press (auto-repeat is collapsed). Because a swallowed key never
//! registers with Windows, the held state is tracked from hook events, not `GetAsyncKeyState`.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetMessageW, PostThreadMessageW, SetWindowsHookExW, UnhookWindowsHookEx,
    HC_ACTION, KBDLLHOOKSTRUCT, LLKHF_INJECTED, MSG, WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP, WM_QUIT,
    WM_SYSKEYDOWN, WM_SYSKEYUP,
};

use crate::config::VirtualKey;
use crate::error::HookError;
use crate::hotkey::{decide, HookState, Input, KeyEvent, Millis};

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

    /// Whether the hotkey is currently held, as seen by the hook.
    pub fn is_down(&self) -> bool {
        self.down.load(Ordering::Relaxed)
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

/// Applies the pure decision to the shared state; returns true if the event must be swallowed.
fn on_key_event(ctx: &HookContext, event: KeyEvent) -> bool {
    let tracker = &ctx.tracker;
    let mut capture = tracker.capture.lock().ok();
    let capturing = capture.as_ref().is_some_and(|c| c.is_some());
    let decision = decide(
        HookState {
            hotkey: tracker.hotkey.load(Ordering::Relaxed),
            held: tracker.down.load(Ordering::Relaxed),
            capturing,
        },
        event,
    );
    tracker.down.store(decision.held, Ordering::Relaxed);
    if let (Some(vk), Some(slot)) = (decision.captured, capture.as_mut()) {
        if let Some(tx) = slot.take() {
            let _ = tx.send(VirtualKey(vk));
        }
    }
    if let Some(input) = decision.input {
        let _ = ctx.tx.send((input, tracker.now()));
    }
    decision.swallow
}

unsafe extern "system" fn hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 {
        // SAFETY: for HC_ACTION on WH_KEYBOARD_LL, lparam points to a valid KBDLLHOOKSTRUCT.
        let event = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
        let is_down = match wparam.0 as u32 {
            WM_KEYDOWN | WM_SYSKEYDOWN => Some(true),
            WM_KEYUP | WM_SYSKEYUP => Some(false),
            _ => None,
        };
        if let (Some(is_down), Some(ctx)) = (is_down, CONTEXT.get()) {
            let key = KeyEvent {
                vk: event.vkCode,
                is_down,
                injected: event.flags.0 & LLKHF_INJECTED.0 != 0,
            };
            if on_key_event(ctx, key) {
                return LRESULT(1); // swallow: the focused app never sees it
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
pub fn install_hook(
    tracker: Arc<KeyTracker>,
    tx: Sender<(Input, Millis)>,
) -> Result<HookHandle, HookError> {
    CONTEXT
        .set(HookContext { tracker, tx })
        .map_err(|_| HookError::AlreadyInstalled)?;
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
    let thread_id = ready_rx
        .recv()
        .map_err(|e| HookError::Install(e.to_string()))??;
    Ok(HookHandle { thread_id })
}

/// Sends `Tick` every `interval`. The state machine uses it for the maximum recording length,
/// which is what ends a recording whose key-up was lost (e.g. the screen was locked mid-hold).
pub fn spawn_ticker(tracker: Arc<KeyTracker>, tx: Sender<(Input, Millis)>, interval: Duration) {
    thread::spawn(move || loop {
        thread::sleep(interval);
        if tx.send((Input::Tick, tracker.now())).is_err() {
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
    let result = rx
        .recv_timeout(timeout)
        .map_err(|_| HookError::CaptureTimeout);
    if let Ok(mut capture) = tracker.capture.lock() {
        *capture = None;
    }
    result
}
