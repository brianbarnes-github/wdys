/// Human-readable name for a Windows virtual-key code, for messages like "Hotkey set to F9".
pub fn key_name(vk: u32) -> String {
    match vk {
        0x70..=0x87 => format!("F{}", vk - 0x6F),
        0x41..=0x5A | 0x30..=0x39 => char::from_u32(vk).map_or_else(String::new, String::from),
        _ => named(vk).map_or_else(|| format!("key 0x{vk:02X}"), String::from),
    }
}

fn named(vk: u32) -> Option<&'static str> {
    Some(match vk {
        0x08 => "Backspace",
        0x09 => "Tab",
        0x0D => "Enter",
        0x13 => "Pause",
        0x14 => "Caps Lock",
        0x1B => "Esc",
        0x20 => "Space",
        0x21 => "Page Up",
        0x22 => "Page Down",
        0x23 => "End",
        0x24 => "Home",
        0x2C => "Print Screen",
        0x2D => "Insert",
        0x2E => "Delete",
        0x5B => "Left Windows",
        0x5C => "Right Windows",
        0x5D => "Menu",
        0x90 => "Num Lock",
        0x91 => "Scroll Lock",
        0xA0 => "Left Shift",
        0xA1 => "Right Shift",
        0xA2 => "Left Ctrl",
        0xA3 => "Right Ctrl",
        0xA4 => "Left Alt",
        0xA5 => "Right Alt",
        _ => return None,
    })
}
