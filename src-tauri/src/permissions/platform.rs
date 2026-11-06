use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum PermissionStatus {
    Granted,
    Denied,
    NotDetermined,
    Restricted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionsState {
    pub microphone: PermissionStatus,
    pub accessibility: PermissionStatus,
}

#[cfg(target_os = "macos")]
pub mod macos {
    use super::*;
    use std::process::Command;

    pub fn check_microphone() -> PermissionStatus {
        // Use tccutil to check microphone permission status
        let output = Command::new("sqlite3")
            .args([
                &format!(
                    "{}/Library/Application Support/com.apple.TCC/TCC.db",
                    std::env::var("HOME").unwrap_or_default()
                ),
                "SELECT auth_value FROM access WHERE service='kTCCServiceMicrophone' AND client='com.typeurvoice.app' LIMIT 1;"
            ])
            .output();

        match output {
            Ok(out) => {
                let s = String::from_utf8_lossy(&out.stdout);
                match s.trim() {
                    "2" => PermissionStatus::Granted,
                    "0" => PermissionStatus::Denied,
                    _ => PermissionStatus::NotDetermined,
                }
            }
            _ => PermissionStatus::NotDetermined,
        }
    }

    pub fn check_accessibility() -> PermissionStatus {
        // AXIsProcessTrusted equivalent — check via osascript
        let output = Command::new("osascript")
            .args(["-e", "tell application \"System Events\" to return true"])
            .output();

        match output {
            Ok(out) if out.status.success() => PermissionStatus::Granted,
            _ => PermissionStatus::NotDetermined,
        }
    }

    pub fn open_microphone_settings() {
        let _ = Command::new("open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone")
            .spawn();
    }

    pub fn open_accessibility_settings() {
        let _ = Command::new("open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
            .spawn();
    }
}

#[cfg(target_os = "windows")]
pub mod windows {
    use super::*;

    pub fn check_microphone() -> PermissionStatus {
        // On Windows, we try to enumerate audio devices; if empty, likely denied
        PermissionStatus::NotDetermined
    }

    pub fn check_accessibility() -> PermissionStatus {
        // Windows doesn't require accessibility permission for global hotkeys
        PermissionStatus::Granted
    }

    pub fn open_microphone_settings() {
        let _ = std::process::Command::new("ms-settings:privacy-microphone").spawn();
    }

    pub fn open_accessibility_settings() {
        // Not needed on Windows
    }
}

pub fn check_all_permissions() -> PermissionsState {
    #[cfg(target_os = "macos")]
    return PermissionsState {
        microphone: macos::check_microphone(),
        accessibility: macos::check_accessibility(),
    };

    #[cfg(target_os = "windows")]
    return PermissionsState {
        microphone: windows::check_microphone(),
        accessibility: windows::check_accessibility(),
    };

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    PermissionsState {
        microphone: PermissionStatus::NotDetermined,
        accessibility: PermissionStatus::NotDetermined,
    }
}

pub fn open_permission_settings(permission: &str) {
    #[cfg(target_os = "macos")]
    match permission {
        "microphone" => macos::open_microphone_settings(),
        "accessibility" => macos::open_accessibility_settings(),
        _ => {}
    }

    #[cfg(target_os = "windows")]
    match permission {
        "microphone" => windows::open_microphone_settings(),
        _ => {}
    }
}
