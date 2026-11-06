use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use wdys::config::VirtualKey;
use wdys::error::PasteError;
use wdys::paste::{paste_text, Clipboard, Keyboard};

const HOTKEY: VirtualKey = VirtualKey(0xA3);

type Shared<T> = Rc<RefCell<T>>;

struct FakeClipboard {
    content: Shared<Option<String>>,
    writes: Shared<Vec<(String, bool)>>,
}

impl Clipboard for FakeClipboard {
    fn get_text(&mut self) -> Result<Option<String>, PasteError> {
        Ok(self.content.borrow().clone())
    }
    fn set_text(&mut self, text: &str, exclude: bool) -> Result<(), PasteError> {
        self.writes.borrow_mut().push((text.to_string(), exclude));
        *self.content.borrow_mut() = Some(text.to_string());
        Ok(())
    }
    fn clear(&mut self) -> Result<(), PasteError> {
        *self.content.borrow_mut() = None;
        Ok(())
    }
}

struct FakeKeyboard {
    clipboard: Shared<Option<String>>,
    pasted: Shared<Vec<Option<String>>>,
    hotkey_down_polls: Cell<u32>,
    fail_paste: bool,
}

impl Keyboard for FakeKeyboard {
    fn send_paste(&mut self) -> Result<(), PasteError> {
        if self.fail_paste {
            return Err(PasteError::Input("boom".into()));
        }
        self.pasted.borrow_mut().push(self.clipboard.borrow().clone());
        Ok(())
    }
    fn is_down(&self, _key: VirtualKey) -> bool {
        let remaining = self.hotkey_down_polls.get();
        if remaining == 0 {
            return false;
        }
        self.hotkey_down_polls.set(remaining.saturating_sub(1));
        true
    }
}

struct Rig {
    clipboard: FakeClipboard,
    keyboard: FakeKeyboard,
    content: Shared<Option<String>>,
    writes: Shared<Vec<(String, bool)>>,
    pasted: Shared<Vec<Option<String>>>,
}

fn rig(initial: Option<&str>, down_polls: u32, fail_paste: bool) -> Rig {
    let content = Rc::new(RefCell::new(initial.map(String::from)));
    let writes = Rc::new(RefCell::new(Vec::new()));
    let pasted = Rc::new(RefCell::new(Vec::new()));
    Rig {
        clipboard: FakeClipboard { content: content.clone(), writes: writes.clone() },
        keyboard: FakeKeyboard {
            clipboard: content.clone(),
            pasted: pasted.clone(),
            hotkey_down_polls: Cell::new(down_polls),
            fail_paste,
        },
        content,
        writes,
        pasted,
    }
}

fn no_sleep(_: Duration) {}

#[test]
fn pastes_and_restores_previous_text() {
    let mut r = rig(Some("previous"), 0, false);
    paste_text(&mut r.clipboard, &mut r.keyboard, HOTKEY, "hello", &no_sleep).unwrap();
    assert_eq!(*r.pasted.borrow(), vec![Some("hello".to_string())]);
    assert_eq!(*r.content.borrow(), Some("previous".to_string()));
    assert_eq!(r.writes.borrow()[0], ("hello".to_string(), true));
}

#[test]
fn empty_text_touches_nothing() {
    let mut r = rig(Some("previous"), 0, false);
    paste_text(&mut r.clipboard, &mut r.keyboard, HOTKEY, "", &no_sleep).unwrap();
    assert!(r.writes.borrow().is_empty());
    assert!(r.pasted.borrow().is_empty());
    assert_eq!(*r.content.borrow(), Some("previous".to_string()));
}

#[test]
fn empty_clipboard_is_cleared_afterward() {
    let mut r = rig(None, 0, false);
    paste_text(&mut r.clipboard, &mut r.keyboard, HOTKEY, "hello", &no_sleep).unwrap();
    assert_eq!(*r.pasted.borrow(), vec![Some("hello".to_string())]);
    assert_eq!(*r.content.borrow(), None);
}

#[test]
fn waits_for_hotkey_release() {
    let mut r = rig(None, 3, false);
    let sleeps = Cell::new(0u32);
    paste_text(&mut r.clipboard, &mut r.keyboard, HOTKEY, "hi", &|_| sleeps.set(sleeps.get() + 1)).unwrap();
    assert!(sleeps.get() >= 3);
    assert_eq!(r.pasted.borrow().len(), 1);
}

#[test]
fn held_hotkey_times_out() {
    let mut r = rig(Some("previous"), u32::MAX, false);
    let result = paste_text(&mut r.clipboard, &mut r.keyboard, HOTKEY, "hi", &no_sleep);
    assert!(matches!(result, Err(PasteError::HotkeyStillHeld)));
    assert!(r.pasted.borrow().is_empty());
    assert_eq!(*r.content.borrow(), Some("previous".to_string()));
}

#[test]
fn restores_even_when_paste_fails() {
    let mut r = rig(Some("previous"), 0, true);
    let result = paste_text(&mut r.clipboard, &mut r.keyboard, HOTKEY, "hi", &no_sleep);
    assert!(matches!(result, Err(PasteError::Input(_))));
    assert_eq!(*r.content.borrow(), Some("previous".to_string()));
}
