//! Quiet the Mac while recording — `Settings::media_while_recording`.
//!
//! * **Mute**: mutes the default output device (CoreAudio), unmutes it when
//!   the recording stops. Left alone if it was already muted.
//! * **Pause**: if the output device is playing something, presses the
//!   system Play/Pause key (the one on the keyboard — it reaches Music,
//!   Spotify, YouTube in Safari/Chrome and anything else that shows in
//!   Now Playing) and presses it again when the recording stops. Nothing
//!   playing ⇒ no key press, so a paused player is never started. Posting
//!   the key needs Accessibility (the same grant as auto-paste); without it
//!   we mute instead.
//!
//! `quiet` runs when a recording starts (before the mic opens), `restore`
//! when it stops and on Quit. Only what `quiet` changed is undone.

use crate::state::MediaWhileRecording;

#[cfg(target_os = "macos")]
pub use mac::{quiet, recover, restore};

pub fn is_available() -> bool {
    cfg!(target_os = "macos")
}

#[cfg(not(target_os = "macos"))]
pub fn quiet(_mode: &MediaWhileRecording) {}

#[cfg(not(target_os = "macos"))]
pub fn restore() {}

#[cfg(not(target_os = "macos"))]
pub fn recover() {}

#[cfg(target_os = "macos")]
mod mac {
    use super::MediaWhileRecording;
    use crate::log_to_file;
    use std::ffi::{c_char, c_void};
    use std::sync::Mutex;

    /// What `restore` has to undo.
    enum Undo {
        Unmute { device: u32, elements: Vec<u32> },
        Resume,
    }

    static UNDO: Mutex<Option<Undo>> = Mutex::new(None);

    /// While the Mac is muted by us, `media_mute.txt` says which device and
    /// elements — so a crash or forced quit mid-recording is undone at the
    /// next launch (`recover`) instead of leaving the Mac silent.
    fn marker_path() -> Option<std::path::PathBuf> {
        crate::storage::settings::typeurvoice_data_dir().ok().map(|d| d.join("media_mute.txt"))
    }

    fn write_marker(device: u32, elements: &[u32]) {
        if let Some(path) = marker_path() {
            let list: Vec<String> = elements.iter().map(u32::to_string).collect();
            let _ = std::fs::write(path, format!("{} {}", device, list.join(",")));
        }
    }

    fn remove_marker() {
        if let Some(path) = marker_path() {
            let _ = std::fs::remove_file(path);
        }
    }

    /// Unmute what a previous run muted and never got to unmute.
    pub fn recover() {
        let Some(path) = marker_path() else { return };
        let Ok(text) = std::fs::read_to_string(&path) else { return };
        let mut parts = text.split_whitespace();
        let device = parts.next().and_then(|d| d.parse::<u32>().ok());
        let elements: Vec<u32> = parts
            .next()
            .map(|l| l.split(',').filter_map(|e| e.parse().ok()).collect())
            .unwrap_or_default();
        if let Some(device) = device {
            for e in &elements {
                set_u32(device, &mute_address(*e), 0);
            }
            log_to_file("Media: unmuted the sound a previous run left muted (it quit mid-recording)");
        }
        let _ = std::fs::remove_file(path);
    }

    pub fn quiet(mode: &MediaWhileRecording) {
        let mut undo = UNDO.lock().unwrap_or_else(|e| e.into_inner());
        if undo.is_some() {
            return; // already quiet (a previous recording wasn't restored yet)
        }
        *undo = match mode {
            MediaWhileRecording::Off => None,
            MediaWhileRecording::Mute => mute(),
            MediaWhileRecording::Pause => pause(),
        };
    }

    pub fn restore() {
        let undo = UNDO.lock().unwrap_or_else(|e| e.into_inner()).take();
        match undo {
            None => {}
            Some(Undo::Unmute { device, elements }) => {
                for e in &elements {
                    set_u32(device, &mute_address(*e), 0);
                }
                remove_marker();
                log_to_file("Media: sound unmuted");
            }
            Some(Undo::Resume) => {
                post_media_key(NX_KEYTYPE_PLAY);
                log_to_file("Media: playback resumed");
            }
        }
    }

    fn mute() -> Option<Undo> {
        let Some(device) = default_output() else {
            log_to_file("Media: no output device — nothing to mute");
            return None;
        };
        let elements = mute_elements(device);
        if elements.is_empty() {
            log_to_file("Media: this output device can't be muted");
            return None;
        }
        // Only touch elements that are currently audible, so a device the
        // user had muted stays muted after the recording.
        let changed: Vec<u32> = elements
            .into_iter()
            .filter(|e| get_u32(device, &mute_address(*e)) == Some(0))
            .filter(|e| set_u32(device, &mute_address(*e), 1))
            .collect();
        if changed.is_empty() {
            return None;
        }
        write_marker(device, &changed);
        log_to_file("Media: sound muted while recording");
        Some(Undo::Unmute { device, elements: changed })
    }

    fn pause() -> Option<Undo> {
        let Some(device) = default_output() else { return None };
        if !is_running(device) {
            // Pressing Play/Pause now would *start* the last player.
            return None;
        }
        if !accessibility_trusted() {
            log_to_file("Media: pausing needs Accessibility permission — muting instead");
            return mute();
        }
        post_media_key(NX_KEYTYPE_PLAY);
        log_to_file("Media: playback paused while recording");
        Some(Undo::Resume)
    }

    // ── CoreAudio ───────────────────────────────────────────────────────────

    #[repr(C)]
    struct PropertyAddress {
        selector: u32,
        scope: u32,
        element: u32,
    }

    const fn fourcc(s: &[u8; 4]) -> u32 {
        ((s[0] as u32) << 24) | ((s[1] as u32) << 16) | ((s[2] as u32) << 8) | (s[3] as u32)
    }

    const SYSTEM_OBJECT: u32 = 1; // kAudioObjectSystemObject
    const DEFAULT_OUTPUT_DEVICE: u32 = fourcc(b"dOut");
    const RUNNING_SOMEWHERE: u32 = fourcc(b"gone"); // kAudioDevicePropertyDeviceIsRunningSomewhere
    const MUTE: u32 = fourcc(b"mute");
    const SCOPE_GLOBAL: u32 = fourcc(b"glob");
    const SCOPE_OUTPUT: u32 = fourcc(b"outp");

    #[link(name = "CoreAudio", kind = "framework")]
    extern "C" {
        fn AudioObjectHasProperty(id: u32, address: *const PropertyAddress) -> u8;
        fn AudioObjectIsPropertySettable(id: u32, address: *const PropertyAddress, settable: *mut u8) -> i32;
        fn AudioObjectGetPropertyData(
            id: u32,
            address: *const PropertyAddress,
            qualifier_size: u32,
            qualifier: *const c_void,
            data_size: *mut u32,
            data: *mut c_void,
        ) -> i32;
        fn AudioObjectSetPropertyData(
            id: u32,
            address: *const PropertyAddress,
            qualifier_size: u32,
            qualifier: *const c_void,
            data_size: u32,
            data: *const c_void,
        ) -> i32;
    }

    fn get_u32(id: u32, address: &PropertyAddress) -> Option<u32> {
        let mut value: u32 = 0;
        let mut size = std::mem::size_of::<u32>() as u32;
        let status = unsafe {
            AudioObjectGetPropertyData(id, address, 0, std::ptr::null(), &mut size, &mut value as *mut u32 as *mut c_void)
        };
        (status == 0).then_some(value)
    }

    fn set_u32(id: u32, address: &PropertyAddress, value: u32) -> bool {
        let status = unsafe {
            AudioObjectSetPropertyData(
                id, address, 0, std::ptr::null(),
                std::mem::size_of::<u32>() as u32, &value as *const u32 as *const c_void,
            )
        };
        status == 0
    }

    fn default_output() -> Option<u32> {
        let address = PropertyAddress { selector: DEFAULT_OUTPUT_DEVICE, scope: SCOPE_GLOBAL, element: 0 };
        get_u32(SYSTEM_OBJECT, &address).filter(|id| *id != 0)
    }

    /// Some app has the output device running — i.e. something is playing.
    fn is_running(device: u32) -> bool {
        let address = PropertyAddress { selector: RUNNING_SOMEWHERE, scope: SCOPE_GLOBAL, element: 0 };
        get_u32(device, &address).is_some_and(|v| v != 0)
    }

    fn mute_address(element: u32) -> PropertyAddress {
        PropertyAddress { selector: MUTE, scope: SCOPE_OUTPUT, element }
    }

    /// The main element when it has a settable mute, otherwise the first two
    /// channels (some USB devices only mute per channel).
    fn mute_elements(device: u32) -> Vec<u32> {
        let settable = |e: u32| unsafe {
            let address = mute_address(e);
            let mut ok: u8 = 0;
            AudioObjectHasProperty(device, &address) != 0
                && AudioObjectIsPropertySettable(device, &address, &mut ok) == 0
                && ok != 0
        };
        if settable(0) {
            vec![0]
        } else {
            [1, 2].into_iter().filter(|e| settable(*e)).collect()
        }
    }

    // ── Play/Pause key ──────────────────────────────────────────────────────

    const NX_KEYTYPE_PLAY: i64 = 16;

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct CGPoint {
        x: f64,
        y: f64,
    }

    #[link(name = "AppKit", kind = "framework")]
    extern "C" {}

    #[link(name = "objc")]
    extern "C" {
        fn objc_getClass(name: *const c_char) -> *mut c_void;
        fn sel_registerName(name: *const c_char) -> *mut c_void;
        fn objc_msgSend();
        fn objc_autoreleasePoolPush() -> *mut c_void;
        fn objc_autoreleasePoolPop(pool: *mut c_void);
    }

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGEventPost(tap: u32, event: *mut c_void);
    }

    use crate::paste::accessibility_trusted;

    /// Press and release a media key, as the keyboard's own key does:
    /// an `NSSystemDefined` event (subtype 8, aux control buttons) posted to
    /// the HID event tap.
    fn post_media_key(key: i64) {
        type OtherEvent = unsafe extern "C" fn(
            *mut c_void, *mut c_void, u64, CGPoint, u64, f64, i64, *mut c_void, i16, i64, i64,
        ) -> *mut c_void;
        type GetCgEvent = unsafe extern "C" fn(*mut c_void, *mut c_void) -> *mut c_void;

        unsafe {
            let class = objc_getClass(c"NSEvent".as_ptr());
            if class.is_null() {
                log_to_file("Media: NSEvent unavailable — can't press Play/Pause");
                return;
            }
            let make: OtherEvent = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
            let to_cg: GetCgEvent = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
            let sel_make = sel_registerName(
                c"otherEventWithType:location:modifierFlags:timestamp:windowNumber:context:subtype:data1:data2:".as_ptr(),
            );
            let sel_cg = sel_registerName(c"CGEvent".as_ptr());

            let pool = objc_autoreleasePoolPush();
            for down in [true, false] {
                let state: i64 = if down { 0xA } else { 0xB };
                let flags: u64 = if down { 0xA00 } else { 0xB00 };
                let event = make(
                    class, sel_make,
                    14, // NSEventTypeSystemDefined
                    CGPoint { x: 0.0, y: 0.0 },
                    flags, 0.0, 0, std::ptr::null_mut(),
                    8, // NX_SUBTYPE_AUX_CONTROL_BUTTONS
                    (key << 16) | (state << 8),
                    -1,
                );
                if event.is_null() {
                    continue;
                }
                let cg = to_cg(event, sel_cg);
                if !cg.is_null() {
                    CGEventPost(0 /* kCGHIDEventTap */, cg);
                }
            }
            objc_autoreleasePoolPop(pool);
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn fourcc_matches_coreaudio() {
            assert_eq!(MUTE, 0x6D75_7465);
            assert_eq!(DEFAULT_OUTPUT_DEVICE, 0x644F_7574);
        }

        /// Reads the real output device without changing anything.
        /// `cargo test --lib media_probe -- --ignored --nocapture`
        /// (run `afplay /System/Library/Sounds/Submarine.aiff` meanwhile to see `running: true`).
        #[test]
        #[ignore]
        fn media_probe() {
            let device = default_output().expect("default output device");
            println!(
                "output device {}: running: {}, mute elements: {:?}, muted: {:?}, accessibility: {}",
                device,
                is_running(device),
                mute_elements(device),
                mute_elements(device).first().and_then(|e| get_u32(device, &mute_address(*e))),
                accessibility_trusted(),
            );
        }
    }
}
