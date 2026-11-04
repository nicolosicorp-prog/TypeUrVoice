//! Knock-to-record: a double (or triple) knock on the MacBook chassis toggles
//! recording, like pressing the primary shortcut in Toggle mode.
//!
//! `sensor` streams the built-in accelerometer on a dedicated thread,
//! `detector` turns samples into knock patterns, and impacts that coincide
//! with typing or clicking are dropped. A completed pattern is handed to
//! `crate::on_knock` on a fresh thread so the sensor run loop never blocks on
//! overlay/window work. macOS only — elsewhere the listener never starts.

pub mod detector;
#[cfg(target_os = "macos")]
mod sensor;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager};

#[derive(Default)]
pub struct KnockListener {
    running: Mutex<Option<Running>>,
}

struct Running {
    config: detector::Config,
    stop: Arc<AtomicBool>,
}

pub fn is_available() -> bool {
    #[cfg(target_os = "macos")]
    return sensor::is_available();
    #[cfg(not(target_os = "macos"))]
    return false;
}

/// Start, restart or stop the listener so it matches the current settings.
/// Cheap when nothing changed — called on every settings save.
pub fn sync(app: &AppHandle) {
    let wanted = crate::with_settings(app, |s| {
        s.knock_enabled.then(|| detector::Config {
            knocks: s.knock_pattern.count(),
            sensitivity: s.knock_sensitivity,
        })
    });
    let listener = app.state::<KnockListener>();
    let mut running = listener.running.lock().unwrap_or_else(|e| e.into_inner());
    if running.as_ref().map(|r| r.config) == wanted {
        return;
    }
    if let Some(old) = running.take() {
        // The sensor thread notices within one 250 ms run-loop slice.
        old.stop.store(true, Ordering::Relaxed);
        crate::log_to_file("Knock: listener stopped");
    }
    if let Some(config) = wanted {
        *running = start(app.clone(), config);
    }
}

#[cfg(target_os = "macos")]
fn start(app: AppHandle, config: detector::Config) -> Option<Running> {
    use std::time::{Duration, Instant};

    if !sensor::is_available() {
        crate::log_to_file("Knock: enabled, but this Mac has no accelerometer");
        return None;
    }
    let stop = Arc::new(AtomicBool::new(false));
    let thread_stop = Arc::clone(&stop);
    let spawned = std::thread::Builder::new()
        .name("knock-sensor".into())
        .spawn(move || {
            let mut detector = detector::Detector::new(config);
            let started = Instant::now();
            let result = sensor::run(thread_stop, |t, x, y, z| {
                // Reject an impact if a key or click happened just around it
                // — typing and trackpad clicks shake the chassis too.
                let not_typing = |impact_t: f64| {
                    let age = started.elapsed().as_secs_f64() - impact_t;
                    sensor::seconds_since_input() > age + detector::INPUT_GUARD_SECS
                };
                if let Some(pattern) = detector.push(t, x, y, z, not_typing) {
                    crate::log_to_file(&format!(
                        "Knock: {} knocks over {}ms, peak {:.2}g",
                        config.knocks,
                        ((pattern.last - pattern.first) * 1000.0) as u32,
                        pattern.peak_g,
                    ));
                    let since_first = started.elapsed().saturating_sub(Duration::from_secs_f64(pattern.first));
                    let app = app.clone();
                    std::thread::spawn(move || crate::on_knock(&app, since_first));
                }
            });
            match result {
                Ok(()) => {}
                Err(e) => crate::log_to_file(&format!("Knock: listener failed: {}", e)),
            }
        });
    match spawned {
        Ok(_) => {
            crate::log_to_file(&format!(
                "Knock: listening ({} knocks, {:?} sensitivity)",
                config.knocks, config.sensitivity
            ));
            Some(Running { config, stop })
        }
        Err(e) => {
            crate::log_to_file(&format!("Knock: could not spawn sensor thread: {}", e));
            None
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn start(_app: AppHandle, _config: detector::Config) -> Option<Running> {
    None
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;
    use crate::state::KnockSensitivity;

    /// Live check against the real sensor, with the same typing veto as the
    /// app. Prints every impact and every recognised pattern:
    /// VOICEINK_KNOCK_SECS=120 VOICEINK_KNOCK_SENS=medium \
    ///   cargo test --release --lib knock_live -- --ignored --nocapture
    #[test]
    #[ignore]
    fn knock_live() {
        let secs: u64 = std::env::var("VOICEINK_KNOCK_SECS").ok().and_then(|s| s.parse().ok()).unwrap_or(60);
        let sensitivity = match std::env::var("VOICEINK_KNOCK_SENS").as_deref() {
            Ok("high") => KnockSensitivity::High,
            Ok("low") => KnockSensitivity::Low,
            _ => KnockSensitivity::Medium,
        };
        let knocks = if std::env::var("VOICEINK_KNOCK_TRIPLE").is_ok() { 3 } else { 2 };
        println!("\nListening for {secs} s — {knocks} knocks, {sensitivity:?} sensitivity. Ctrl-C to stop.\n");
        let stop = Arc::new(AtomicBool::new(false));
        let timer_stop = Arc::clone(&stop);
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(secs));
            timer_stop.store(true, Ordering::Relaxed);
        });
        let started = std::time::Instant::now();
        let mut detector = detector::Detector::new(detector::Config { knocks, sensitivity });
        let mut count = 0;
        sensor::run(stop, |t, x, y, z| {
            let accept = |impact_t: f64| {
                let age = started.elapsed().as_secs_f64() - impact_t;
                let typing = sensor::seconds_since_input() <= age + detector::INPUT_GUARD_SECS;
                println!("  {:7.2}s bump{}", impact_t, if typing { "  (typing/click — ignored)" } else { "" });
                !typing
            };
            if let Some(p) = detector.push(t, x, y, z, accept) {
                count += 1;
                println!(">>> {:7.2}s KNOCK PATTERN #{count}  ({} ms between first and last, peak {:.3} g)",
                    p.last, ((p.last - p.first) * 1000.0) as u32, p.peak_g);
            }
        })
        .expect("sensor");
        println!("\nDone: {count} pattern(s).");
    }
}
