use std::time::Duration;

use crate::config::VirtualKey;
use crate::error::PasteError;

pub trait Clipboard {
    fn get_text(&mut self) -> Result<Option<String>, PasteError>;
    fn set_text(&mut self, text: &str, exclude_from_history: bool) -> Result<(), PasteError>;
    fn clear(&mut self) -> Result<(), PasteError>;
}

pub trait Keyboard {
    fn send_paste(&mut self) -> Result<(), PasteError>;
    fn is_down(&self, key: VirtualKey) -> bool;
}

const RELEASE_POLL: Duration = Duration::from_millis(10);
const RELEASE_MAX_POLLS: u32 = 100;
const RESTORE_DELAY: Duration = Duration::from_millis(150);

fn wait_for_release(
    keyboard: &impl Keyboard,
    hotkey: VirtualKey,
    sleep: &dyn Fn(Duration),
) -> Result<(), PasteError> {
    for _ in 0..RELEASE_MAX_POLLS {
        if !keyboard.is_down(hotkey) {
            return Ok(());
        }
        sleep(RELEASE_POLL);
    }
    Err(PasteError::HotkeyStillHeld)
}

/// Leaves `text` on the clipboard as a normal entry so the user can paste it by hand.
/// Reports `cause` wrapped in a message saying so, or `cause` itself if even that fails.
fn keep_text(clipboard: &mut impl Clipboard, text: &str, cause: PasteError) -> PasteError {
    match clipboard.set_text(text, false) {
        Ok(()) => PasteError::TextOnClipboard(cause.to_string()),
        Err(_) => cause,
    }
}

/// Pastes `text` into the focused app via the clipboard, then restores the previous text
/// content. Empty text is a no-op that leaves the clipboard untouched. If the paste cannot
/// be completed, the dictated text is left on the clipboard instead, so it is never lost.
pub fn paste_text(
    clipboard: &mut impl Clipboard,
    keyboard: &mut impl Keyboard,
    hotkey: VirtualKey,
    text: &str,
    sleep: &dyn Fn(Duration),
) -> Result<(), PasteError> {
    if text.is_empty() {
        return Ok(());
    }
    if let Err(cause) = wait_for_release(keyboard, hotkey, sleep) {
        return Err(keep_text(clipboard, text, cause));
    }

    let previous = clipboard.get_text()?;
    clipboard.set_text(text, true)?;
    if let Err(cause) = keyboard.send_paste() {
        return Err(keep_text(clipboard, text, cause));
    }
    sleep(RESTORE_DELAY);
    match previous {
        Some(previous) => clipboard.set_text(&previous, false),
        None => clipboard.clear(),
    }
}
