use anyhow::Result;
use arboard::Clipboard;

pub fn paste_text(text: &str) -> Result<()> {
    // Step A: write text to clipboard
    write_clipboard(text).map_err(|e| anyhow::anyhow!("Clipboard write failed: {}. Text could not be pasted.", e))?;

    // Give clipboard time to settle before simulating paste
    std::thread::sleep(std::time::Duration::from_millis(100));

    // Step B: simulate Cmd+V / Ctrl+V to paste into the focused app.
    // A blocked Cmd+V is an error for the caller to surface: the text only
    // sits on the clipboard and the user has to paste it themselves.
    simulate_paste().map_err(|e| {
        crate::log_to_file(&format!("Paste: Cmd+V simulation failed: {}", e));
        anyhow::anyhow!(
            "Text copied — press Cmd+V. Auto-paste was blocked: allow TypeUrVoice in \
             System Settings → Privacy & Security → Accessibility."
        )
    })?;
    eprintln!("[TypeUrVoice] Paste: simulated Cmd+V");
    Ok(())
}

pub fn copy_to_clipboard(text: &str) -> Result<()> {
    write_clipboard(text).map_err(|e| anyhow::anyhow!("Clipboard write failed: {}. Text could not be pasted.", e))
}

/// Write text to the system clipboard. On macOS we try `arboard` first; if it
/// fails (rare, but seen on some macs where NSPasteboard access from a worker
/// thread returns an error), we fall back to `pbcopy`, which is a subprocess
/// and can't fail for permission reasons.
fn write_clipboard(text: &str) -> Result<()> {
    match Clipboard::new().and_then(|mut c| c.set_text(text.to_string())) {
        Ok(_) => {
            eprintln!("[TypeUrVoice] Clipboard: wrote {} chars via arboard", text.len());
            Ok(())
        }
        Err(e) => {
            eprintln!("[TypeUrVoice] Clipboard: arboard failed ({}), trying pbcopy fallback", e);
            #[cfg(target_os = "macos")]
            {
                pbcopy(text)?;
                eprintln!("[TypeUrVoice] Clipboard: wrote {} chars via pbcopy", text.len());
                Ok(())
            }
            #[cfg(not(target_os = "macos"))]
            Err(anyhow::anyhow!("Clipboard write failed: {}", e))
        }
    }
}

#[cfg(target_os = "macos")]
fn pbcopy(text: &str) -> Result<()> {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let mut child = Command::new("pbcopy")
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|e| anyhow::anyhow!("Failed to launch pbcopy: {}", e))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(text.as_bytes())
            .map_err(|e| anyhow::anyhow!("Failed to write to pbcopy stdin: {}", e))?;
    }
    let status = child
        .wait()
        .map_err(|e| anyhow::anyhow!("Failed to wait for pbcopy: {}", e))?;
    if !status.success() {
        return Err(anyhow::anyhow!("pbcopy exited with status {}", status));
    }
    Ok(())
}

/// macOS: post Cmd+V straight from TypeUrVoice with CoreGraphics.
///
/// Earlier builds shelled out to `osascript … keystroke`, which macOS
/// attributes to osascript/System Events and which kept failing with
/// "osascript is not allowed to send keystrokes" even with TypeUrVoice listed
/// under Accessibility. Posting the event ourselves makes TypeUrVoice's own
/// Accessibility grant the only thing that matters. (The original enigo
/// crash was a NULL CGEvent being dereferenced without permission — here
/// trust is checked first and every pointer is null-checked.)
#[cfg(target_os = "macos")]
mod mac {
    use std::ffi::c_void;

    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        pub fn AXIsProcessTrusted() -> bool;
        pub fn CGEventSourceCreate(state_id: i32) -> *mut c_void;
        pub fn CGEventCreateKeyboardEvent(source: *mut c_void, keycode: u16, key_down: bool) -> *mut c_void;
        pub fn CGEventSetFlags(event: *mut c_void, flags: u64);
        pub fn CGEventPost(tap: u32, event: *mut c_void);
        pub fn CGEventKeyboardSetUnicodeString(event: *mut c_void, length: usize, string: *const u16);
    }
    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        pub fn CFRelease(cf: *const c_void);
    }

    pub const HID_SYSTEM_STATE: i32 = 1;
    pub const HID_EVENT_TAP: u32 = 0;
    /// kVK_ANSI_V — physical key, same position on QWERTY/QWERTZ layouts.
    pub const KEY_V: u16 = 9;
    pub const FLAG_COMMAND: u64 = 0x0010_0000;
}

/// Type `text` into the focused app as keyboard input — used for text
/// streamed while the user is still speaking, so the clipboard isn't
/// overwritten for every sentence. Multi-line text is pasted instead: a
/// typed Return would send the message in chat apps.
pub fn type_text(text: &str) -> Result<()> {
    if text.contains('\n') {
        return paste_text(text);
    }
    #[cfg(target_os = "macos")]
    unsafe {
        use mac::*;
        if !AXIsProcessTrusted() {
            return Err(anyhow::anyhow!("TypeUrVoice is not allowed under Accessibility"));
        }
        let source = CGEventSourceCreate(HID_SYSTEM_STATE);
        let units: Vec<u16> = text.encode_utf16().collect();
        let mut result = Ok(());
        let mut i = 0;
        'outer: while i < units.len() {
            // ≤16 UTF-16 units per event (macOS drops longer strings), never
            // splitting a surrogate pair.
            let mut end = (i + 16).min(units.len());
            if end < units.len() && (0xD800..0xDC00).contains(&units[end - 1]) {
                end -= 1;
            }
            let piece = &units[i..end];
            for key_down in [true, false] {
                let event = CGEventCreateKeyboardEvent(source, 0, key_down);
                if event.is_null() {
                    result = Err(anyhow::anyhow!("CGEventCreateKeyboardEvent returned NULL"));
                    break 'outer;
                }
                CGEventKeyboardSetUnicodeString(event, piece.len(), piece.as_ptr());
                // Explicitly no modifiers: in Hold mode the user is still holding
                // the shortcut's Option/Control, which must not turn our text
                // into shortcuts.
                CGEventSetFlags(event, 0);
                CGEventPost(HID_EVENT_TAP, event);
                CFRelease(event);
            }
            i = end;
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        if !source.is_null() {
            CFRelease(source);
        }
        result?;
    }

    #[cfg(target_os = "windows")]
    {
        use enigo::{Enigo, Keyboard, Settings as EnigoSettings};
        let mut enigo = Enigo::new(&EnigoSettings::default())?;
        enigo.text(text)?;
    }

    Ok(())
}

/// Whether macOS lets TypeUrVoice post keystrokes (logged at launch).
#[cfg(target_os = "macos")]
pub fn accessibility_trusted() -> bool {
    unsafe { mac::AXIsProcessTrusted() }
}

fn simulate_paste() -> Result<()> {
    #[cfg(target_os = "macos")]
    unsafe {
        use mac::*;
        if !AXIsProcessTrusted() {
            // No prompt here — launch already asked once; the notification
            // the caller shows explains where to allow it.
            return Err(anyhow::anyhow!("TypeUrVoice is not allowed under Accessibility"));
        }
        let source = CGEventSourceCreate(HID_SYSTEM_STATE); // NULL is a valid source
        let mut result = Ok(());
        for key_down in [true, false] {
            let event = CGEventCreateKeyboardEvent(source, KEY_V, key_down);
            if event.is_null() {
                result = Err(anyhow::anyhow!("CGEventCreateKeyboardEvent returned NULL"));
                break;
            }
            CGEventSetFlags(event, FLAG_COMMAND);
            CGEventPost(HID_EVENT_TAP, event);
            CFRelease(event);
        }
        if !source.is_null() {
            CFRelease(source);
        }
        result?;
    }

    #[cfg(target_os = "windows")]
    {
        use enigo::{Enigo, Key, Keyboard, Settings as EnigoSettings};
        let mut enigo = Enigo::new(&EnigoSettings::default())?;
        enigo.key(Key::Control, enigo::Direction::Press)?;
        enigo.key(Key::Unicode('v'), enigo::Direction::Click)?;
        enigo.key(Key::Control, enigo::Direction::Release)?;
    }

    Ok(())
}
