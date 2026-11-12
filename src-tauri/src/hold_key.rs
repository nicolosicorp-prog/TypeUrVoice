//! Physical state of the Hold-mode shortcut key.
//!
//! The Carbon hot-key events behind `tauri-plugin-global-shortcut` on macOS
//! are not the whole truth: a Released can arrive while the key is still
//! held (the recording stops mid-sentence), and a real release can be lost
//! (e.g. while secure input is on, the recording never stops). Reading the
//! key's actual state lets Hold mode ignore the first and recover from the
//! second. `CGEventSourceKeyState` needs no extra permission.

/// macOS virtual key code of the shortcut's main (non-modifier) key, or
/// `None` when it can't be resolved (callers then trust the events alone).
pub fn key_code(shortcut: &str) -> Option<u16> {
    let parsed = shortcut.parse::<tauri_plugin_global_shortcut::Shortcut>().ok()?;
    mac_key_code(&format!("{:?}", parsed.key))
}

/// `keyboard_types::Code` debug name → macOS ANSI virtual key code.
fn mac_key_code(code: &str) -> Option<u16> {
    let k = match code {
        "KeyA" => 0x00, "KeyS" => 0x01, "KeyD" => 0x02, "KeyF" => 0x03,
        "KeyH" => 0x04, "KeyG" => 0x05, "KeyZ" => 0x06, "KeyX" => 0x07,
        "KeyC" => 0x08, "KeyV" => 0x09, "KeyB" => 0x0B, "KeyQ" => 0x0C,
        "KeyW" => 0x0D, "KeyE" => 0x0E, "KeyR" => 0x0F, "KeyY" => 0x10,
        "KeyT" => 0x11, "Digit1" => 0x12, "Digit2" => 0x13, "Digit3" => 0x14,
        "Digit4" => 0x15, "Digit6" => 0x16, "Digit5" => 0x17, "Equal" => 0x18,
        "Digit9" => 0x19, "Digit7" => 0x1A, "Minus" => 0x1B, "Digit8" => 0x1C,
        "Digit0" => 0x1D, "BracketRight" => 0x1E, "KeyO" => 0x1F, "KeyU" => 0x20,
        "BracketLeft" => 0x21, "KeyI" => 0x22, "KeyP" => 0x23, "Enter" => 0x24,
        "KeyL" => 0x25, "KeyJ" => 0x26, "Quote" => 0x27, "KeyK" => 0x28,
        "Semicolon" => 0x29, "Backslash" => 0x2A, "Comma" => 0x2B, "Slash" => 0x2C,
        "KeyN" => 0x2D, "KeyM" => 0x2E, "Period" => 0x2F, "Tab" => 0x30,
        "Space" => 0x31, "Backquote" => 0x32, "Backspace" => 0x33, "Escape" => 0x35,
        "F1" => 0x7A, "F2" => 0x78, "F3" => 0x63, "F4" => 0x76, "F5" => 0x60,
        "F6" => 0x61, "F7" => 0x62, "F8" => 0x64, "F9" => 0x65, "F10" => 0x6D,
        "F11" => 0x67, "F12" => 0x6F, "Home" => 0x73, "PageUp" => 0x74,
        "Delete" => 0x75, "End" => 0x77, "PageDown" => 0x79,
        "ArrowLeft" => 0x7B, "ArrowRight" => 0x7C, "ArrowDown" => 0x7D, "ArrowUp" => 0x7E,
        _ => return None,
    };
    Some(k)
}

/// True while the key is physically held down.
#[cfg(target_os = "macos")]
pub fn is_down(code: u16) -> bool {
    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGEventSourceKeyState(state_id: i32, key: u16) -> bool;
    }
    const HID_SYSTEM_STATE: i32 = 1; // kCGEventSourceStateHIDSystemState
    unsafe { CGEventSourceKeyState(HID_SYSTEM_STATE, code) }
}

/// Elsewhere the key state isn't read: report "up" so the hot-key events
/// alone decide (the behaviour before this module existed).
#[cfg(not(target_os = "macos"))]
pub fn is_down(_code: u16) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_common_shortcuts() {
        assert_eq!(key_code("Control+Q"), Some(0x0C));
        assert_eq!(key_code("Control+W"), Some(0x0D));
        assert_eq!(key_code("Alt+Space"), Some(0x31));
        assert_eq!(key_code("Control+Shift+Space"), Some(0x31));
        assert_eq!(key_code("Super+F5"), Some(0x60));
    }

    #[test]
    fn unknown_shortcut_is_none() {
        assert_eq!(key_code("not a shortcut"), None);
    }
}
