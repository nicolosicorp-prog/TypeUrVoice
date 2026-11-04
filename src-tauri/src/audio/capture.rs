use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// Maximum recording duration (5 minutes) — prevents runaway recordings
const MAX_RECORDING_SECS: u64 = 300;

/// Start recording audio into `samples_out` on a background thread.
/// The thread runs until `stop_flag` is set to true or MAX_RECORDING_SECS elapses.
/// Call this from within a `std::thread::spawn` so cpal::Stream lives on that thread.
///
/// `device_name` is the user-selected input device (`Some(name)`) or `None`
/// for the system default. Falls back to default if the named device is not
/// found (logs a warning to crash.log).
pub fn record_until_stopped(
    samples_out: Arc<Mutex<Vec<f32>>>,
    sample_rate_out: Arc<Mutex<u32>>,
    stop_flag: Arc<AtomicBool>,
    device_name: Option<String>,
) -> Result<()> {
    let host = cpal::default_host();
    let device = pick_device(&host, device_name.as_deref())
        .context("No microphone found. Please connect a microphone and try again.")?;

    let config = best_config(&device)?;
    let sample_rate = config.sample_rate().0;

    // Use poison-safe locking everywhere — a poisoned mutex must never crash the app
    *sample_rate_out.lock().unwrap_or_else(|e| e.into_inner()) = sample_rate;

    {
        samples_out.lock().unwrap_or_else(|e| e.into_inner()).clear();
    }

    let channels = config.channels() as usize;
    let samples_clone = Arc::clone(&samples_out);
    let stop_flag_clone = Arc::clone(&stop_flag);
    let max_samples = (sample_rate as usize) * (MAX_RECORDING_SECS as usize);

    let stream = device.build_input_stream(
        &config.into(),
        move |data: &[f32], _| {
            // CRITICAL: This callback runs on a system audio thread.
            // A panic here would SIGABRT the entire process.
            // Use unwrap_or_else to survive poisoned mutex, and guard against panics.
            let Ok(mut buf) = samples_clone.lock().or_else(|e| Ok::<_, ()>(e.into_inner())) else {
                return;
            };
            // Hard cap in samples — regardless of channel config, the mono buffer
            // cannot exceed MAX_RECORDING_SECS. Stop the recorder cleanly on overflow.
            if buf.len() >= max_samples {
                stop_flag_clone.store(true, Ordering::Relaxed);
                return;
            }
            let remaining = max_samples - buf.len();
            if channels == 1 {
                let take = data.len().min(remaining);
                buf.extend_from_slice(&data[..take]);
            } else {
                let chunks = remaining;
                for chunk in data.chunks(channels).take(chunks) {
                    let mono = chunk.iter().sum::<f32>() / channels as f32;
                    buf.push(mono);
                }
            }
        },
        |err| eprintln!("[VoiceInk] Audio stream error: {}", err),
        None,
    )?;

    stream.play()?;

    // Block thread until stop signal or timeout
    let start = Instant::now();
    while !stop_flag.load(Ordering::Relaxed) {
        if start.elapsed().as_secs() >= MAX_RECORDING_SECS {
            eprintln!("[VoiceInk] Recording hit {}s timeout — stopping", MAX_RECORDING_SECS);
            stop_flag.store(true, Ordering::Relaxed);
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    // Stream drops here → recording stops
    drop(stream);
    Ok(())
}

/// Pick the input device matching `name`, or return the system default.
/// If `name` is provided but no device matches, logs a warning and falls back.
fn pick_device(host: &cpal::Host, name: Option<&str>) -> Option<cpal::Device> {
    if let Some(target) = name {
        if let Ok(devices) = host.input_devices() {
            for device in devices {
                if let Ok(dn) = device.name() {
                    if dn == target {
                        return Some(device);
                    }
                }
            }
        }
        crate::log_to_file(&format!(
            "Audio: input device {:?} not found — falling back to system default",
            target
        ));
    }
    host.default_input_device()
}

/// List all available input devices for the UI selector.
pub fn list_input_devices() -> Vec<String> {
    let host = cpal::default_host();
    let mut names = Vec::new();
    if let Ok(devices) = host.input_devices() {
        for device in devices {
            if let Ok(name) = device.name() {
                names.push(name);
            }
        }
    }
    names
}

fn best_config(device: &cpal::Device) -> Result<cpal::SupportedStreamConfig> {
    let supported: Vec<_> = device.supported_input_configs()?.collect();

    // Try 16kHz mono first (Whisper optimal)
    for range in &supported {
        if range.channels() == 1 {
            let min = range.min_sample_rate().0;
            let max = range.max_sample_rate().0;
            if min <= 16000 && 16000 <= max {
                return Ok(range.clone().with_sample_rate(cpal::SampleRate(16000)));
            }
        }
    }

    // Fall back to device default
    device.default_input_config().map_err(|e| e.into())
}

