use wdys::keys::key_name;

#[test]
fn function_keys_are_named() {
    assert_eq!(key_name(0x70), "F1");
    assert_eq!(key_name(0x78), "F9");
    assert_eq!(key_name(0x7B), "F12");
    assert_eq!(key_name(0x87), "F24");
}

#[test]
fn letters_and_digits_are_named() {
    assert_eq!(key_name(0x41), "A");
    assert_eq!(key_name(0x5A), "Z");
    assert_eq!(key_name(0x30), "0");
    assert_eq!(key_name(0x39), "9");
}

#[test]
fn modifiers_say_which_side() {
    assert_eq!(key_name(0xA2), "Left Ctrl");
    assert_eq!(key_name(0xA3), "Right Ctrl");
    assert_eq!(key_name(0xA4), "Left Alt");
    assert_eq!(key_name(0xA5), "Right Alt");
    assert_eq!(key_name(0xA0), "Left Shift");
    assert_eq!(key_name(0xA1), "Right Shift");
}

#[test]
fn common_special_keys_are_named() {
    assert_eq!(key_name(0x91), "Scroll Lock");
    assert_eq!(key_name(0x13), "Pause");
    assert_eq!(key_name(0x14), "Caps Lock");
    assert_eq!(key_name(0x2D), "Insert");
    assert_eq!(key_name(0x1B), "Esc");
    assert_eq!(key_name(0x20), "Space");
}

#[test]
fn unknown_keys_fall_back_to_their_code() {
    assert_eq!(key_name(0xE7), "key 0xE7");
}
