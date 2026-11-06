//! Short system sounds for recording start and stop.

use windows::Win32::System::Diagnostics::Debug::MessageBeep;
use windows::Win32::UI::WindowsAndMessaging::{MB_ICONASTERISK, MB_ICONHAND, MB_OK};

use crate::app::Cue;

pub fn play_cue(cue: Cue) {
    let style = match cue {
        Cue::Start => MB_ICONASTERISK,
        Cue::Stop => MB_OK,
        Cue::Error => MB_ICONHAND,
    };
    // SAFETY: MessageBeep has no preconditions; failure only means no sound.
    unsafe {
        let _ = MessageBeep(style);
    }
}
