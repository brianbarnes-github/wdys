//! Simulated Ctrl+V and live key state.

use std::mem::size_of;
use std::sync::Arc;

use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP,
    VIRTUAL_KEY, VK_CONTROL,
};

use crate::config::VirtualKey;
use crate::error::PasteError;
use crate::paste::Keyboard;
use crate::platform::hook::KeyTracker;

const VK_V: VIRTUAL_KEY = VIRTUAL_KEY(0x56);

fn key_event(key: VIRTUAL_KEY, flags: KEYBD_EVENT_FLAGS) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: key, wScan: 0, dwFlags: flags, time: 0, dwExtraInfo: 0 } },
    }
}

/// Reads the live hotkey from the tracker, so changing the hotkey from the tray
/// takes effect without rebuilding the app.
pub struct WinKeyboard {
    tracker: Arc<KeyTracker>,
}

impl WinKeyboard {
    pub fn new(tracker: Arc<KeyTracker>) -> WinKeyboard {
        WinKeyboard { tracker }
    }
}

impl Keyboard for WinKeyboard {
    fn send_paste(&mut self) -> Result<(), PasteError> {
        let inputs = [
            key_event(VK_CONTROL, KEYBD_EVENT_FLAGS(0)),
            key_event(VK_V, KEYBD_EVENT_FLAGS(0)),
            key_event(VK_V, KEYEVENTF_KEYUP),
            key_event(VK_CONTROL, KEYEVENTF_KEYUP),
        ];
        // SAFETY: `inputs` is a valid array of INPUT structures and the size matches.
        let sent = unsafe { SendInput(&inputs, size_of::<INPUT>() as i32) };
        if sent as usize == inputs.len() {
            Ok(())
        } else {
            Err(PasteError::Input(format!("only {sent} of {} key events were accepted", inputs.len())))
        }
    }

    fn is_down(&self, _key: VirtualKey) -> bool {
        // SAFETY: GetAsyncKeyState has no preconditions.
        unsafe { GetAsyncKeyState(self.tracker.hotkey().0 as i32) < 0 }
    }
}
