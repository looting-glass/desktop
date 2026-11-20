//! Parse "Ctrl+Shift+D"-style hotkey strings into Win32 `RegisterHotKey` modifiers and
//! virtual-key codes. Platform-independent so it is unit-tested on every OS.

pub const MOD_ALT: u32 = 0x1;
pub const MOD_CONTROL: u32 = 0x2;
pub const MOD_SHIFT: u32 = 0x4;
pub const MOD_WIN: u32 = 0x8;

/// Returns `(modifiers, virtual_key)`, or `None` if the string isn't a valid hotkey.
pub fn parse(s: &str) -> Option<(u32, u32)> {
    let mut mods = 0;
    let mut key = None;
    for part in s.split('+').map(str::trim) {
        let p = part.to_ascii_lowercase();
        match p.as_str() {
            "ctrl" | "control" => mods |= MOD_CONTROL,
            "shift" => mods |= MOD_SHIFT,
            "alt" => mods |= MOD_ALT,
            "win" | "meta" | "super" => mods |= MOD_WIN,
            _ if key.is_none() => key = Some(vk(&p)?),
            _ => return None, // two non-modifier keys
        }
    }
    Some((mods, key?))
}

fn vk(k: &str) -> Option<u32> {
    let b = k.as_bytes();
    if b.len() == 1 && (b[0].is_ascii_alphanumeric()) {
        return Some(b[0].to_ascii_uppercase() as u32); // 'A'..'Z' and '0'..'9' are their VKs
    }
    if let Some(n) = k.strip_prefix('f').and_then(|n| n.parse::<u32>().ok()) {
        return (1..=24).contains(&n).then_some(0x6F + n); // F1 = 0x70
    }
    if let Some(n) = k.strip_prefix("num").and_then(|n| n.parse::<u32>().ok()) {
        return (n <= 9).then_some(0x60 + n);
    }
    Some(match k {
        "space" => 0x20,
        "tab" => 0x09,
        "enter" | "return" => 0x0D,
        "esc" | "escape" => 0x1B,
        "backspace" => 0x08,
        "insert" | "ins" => 0x2D,
        "delete" | "del" => 0x2E,
        "home" => 0x24,
        "end" => 0x23,
        "pageup" | "pgup" => 0x21,
        "pagedown" | "pgdown" => 0x22,
        "left" => 0x25,
        "up" => 0x26,
        "right" => 0x27,
        "down" => 0x28,
        "`" | "backquote" => 0xC0,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hotkeys() {
        assert_eq!(parse("Ctrl+D"), Some((MOD_CONTROL, 'D' as u32)));
        assert_eq!(parse("ctrl + shift + f7"), Some((MOD_CONTROL | MOD_SHIFT, 0x76)));
        assert_eq!(parse("Alt+W"), Some((MOD_ALT, 'W' as u32)));
        assert_eq!(parse("F12"), Some((0, 0x7B)));
        assert_eq!(parse("Ctrl+Space"), Some((MOD_CONTROL, 0x20)));
        assert_eq!(parse("Ctrl+1"), Some((MOD_CONTROL, '1' as u32)));
        assert_eq!(parse(""), None);
        assert_eq!(parse("Ctrl"), None);
        assert_eq!(parse("Ctrl+D+E"), None);
        assert_eq!(parse("Ctrl+F25"), None);
        assert_eq!(parse("Ctrl+Bogus"), None);
    }
}
