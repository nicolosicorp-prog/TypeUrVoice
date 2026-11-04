//! Built-in MacBook accelerometer, read through IOKit HID.
//!
//! Apple Silicon MacBooks carry an IMU (Bosch BMI286) behind the Sensor
//! Processing Unit. It is not documented by Apple: it shows up as an
//! `AppleSPUHIDDevice` on vendor usage page 0xFF00, usage 3, and only streams
//! after `SensorPropertyReportingState` / `SensorPropertyPowerState` are set
//! on its `AppleSPUHIDDriver`. Reports are 22 bytes with X/Y/Z as
//! little-endian Q16 int32 (g) at offsets 6/10/14, ~800 Hz.
//!
//! Every step fails soft: no device, open refused or silent sensor all come
//! back as `Err` for the caller to log — nothing here can take the app down.
//!
//! Other apps (e.g. Knock) read the same sensor, so we never power it down or
//! change its rate away from the 1 ms everyone else requests.

use std::ffi::{c_void, CString};
use std::os::raw::c_char;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

type IoObject = u32;
type CFTypeRef = *const c_void;
type CFIndex = isize;
type IOHIDDeviceRef = *mut c_void;

type ReportCallback = extern "C" fn(
    context: *mut c_void,
    result: i32,
    sender: *mut c_void,
    report_type: u32,
    report_id: u32,
    report: *mut u8,
    report_length: CFIndex,
);

#[link(name = "IOKit", kind = "framework")]
extern "C" {
    fn IOServiceMatching(name: *const c_char) -> *mut c_void;
    fn IOServiceGetMatchingServices(main_port: u32, matching: *mut c_void, existing: *mut IoObject) -> i32;
    fn IOIteratorNext(iterator: IoObject) -> IoObject;
    fn IOObjectRelease(object: IoObject) -> i32;
    fn IOObjectConformsTo(object: IoObject, class_name: *const c_char) -> i32;
    fn IORegistryEntryCreateCFProperty(entry: IoObject, key: CFTypeRef, allocator: CFTypeRef, options: u32) -> CFTypeRef;
    fn IORegistryEntrySetCFProperty(entry: IoObject, name: CFTypeRef, value: CFTypeRef) -> i32;
    fn IORegistryEntryCreateIterator(entry: IoObject, plane: *const c_char, options: u32, iterator: *mut IoObject) -> i32;
    fn IOHIDDeviceCreate(allocator: CFTypeRef, service: IoObject) -> IOHIDDeviceRef;
    fn IOHIDDeviceOpen(device: IOHIDDeviceRef, options: u32) -> i32;
    fn IOHIDDeviceClose(device: IOHIDDeviceRef, options: u32) -> i32;
    fn IOHIDDeviceRegisterInputReportCallback(
        device: IOHIDDeviceRef,
        report: *mut u8,
        report_length: CFIndex,
        callback: Option<ReportCallback>,
        context: *mut c_void,
    );
    fn IOHIDDeviceScheduleWithRunLoop(device: IOHIDDeviceRef, run_loop: *mut c_void, mode: CFTypeRef);
    fn IOHIDDeviceUnscheduleFromRunLoop(device: IOHIDDeviceRef, run_loop: *mut c_void, mode: CFTypeRef);
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    static kCFRunLoopDefaultMode: CFTypeRef;
    fn CFStringCreateWithCString(allocator: CFTypeRef, c_str: *const c_char, encoding: u32) -> CFTypeRef;
    fn CFNumberCreate(allocator: CFTypeRef, the_type: CFIndex, value_ptr: *const c_void) -> CFTypeRef;
    fn CFNumberGetValue(number: CFTypeRef, the_type: CFIndex, value_ptr: *mut c_void) -> u8;
    fn CFGetTypeID(cf: CFTypeRef) -> usize;
    fn CFNumberGetTypeID() -> usize;
    fn CFRelease(cf: CFTypeRef);
    fn CFRunLoopGetCurrent() -> *mut c_void;
    fn CFRunLoopRunInMode(mode: CFTypeRef, seconds: f64, return_after_source_handled: u8) -> i32;
}

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGEventSourceSecondsSinceLastEventType(state_id: i32, event_type: u32) -> f64;
}

const UTF8: u32 = 0x0800_0100;
const CF_NUMBER_SINT32: CFIndex = 3;
const CF_NUMBER_SINT64: CFIndex = 4;
const ITERATE_RECURSIVELY: u32 = 1;
const VENDOR_USAGE_PAGE: i64 = 0xFF00;
const USAGE_ACCELEROMETER: i64 = 3;
const Q16: f32 = 65536.0;
/// Re-send the wake properties if the sensor goes quiet this long (it stops
/// streaming across system sleep).
const SILENCE_REWAKE: Duration = Duration::from_secs(2);

/// Owned CoreFoundation reference, released on drop.
struct Cf(CFTypeRef);

impl Cf {
    fn string(s: &str) -> Cf {
        let c = CString::new(s).expect("static key without NUL");
        Cf(unsafe { CFStringCreateWithCString(std::ptr::null(), c.as_ptr(), UTF8) })
    }

    fn i32(v: i32) -> Cf {
        Cf(unsafe { CFNumberCreate(std::ptr::null(), CF_NUMBER_SINT32, &v as *const i32 as *const c_void) })
    }
}

impl Drop for Cf {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { CFRelease(self.0) }
        }
    }
}

/// Owned IOKit object, released on drop.
struct Io(IoObject);

impl Drop for Io {
    fn drop(&mut self) {
        if self.0 != 0 {
            unsafe { IOObjectRelease(self.0) };
        }
    }
}

fn int_property(entry: IoObject, key: &str) -> Option<i64> {
    let key = Cf::string(key);
    let value = Cf(unsafe { IORegistryEntryCreateCFProperty(entry, key.0, std::ptr::null(), 0) });
    if value.0.is_null() || unsafe { CFGetTypeID(value.0) != CFNumberGetTypeID() } {
        return None;
    }
    let mut out: i64 = 0;
    let ok = unsafe { CFNumberGetValue(value.0, CF_NUMBER_SINT64, &mut out as *mut i64 as *mut c_void) };
    (ok != 0).then_some(out)
}

/// The accelerometer's `AppleSPUHIDDevice` service, if this Mac has one.
fn find_accelerometer() -> Option<Io> {
    let class = CString::new("AppleSPUHIDDevice").ok()?;
    let matching = unsafe { IOServiceMatching(class.as_ptr()) };
    if matching.is_null() {
        return None;
    }
    let mut iter: IoObject = 0;
    // Consumes `matching`.
    if unsafe { IOServiceGetMatchingServices(0, matching, &mut iter) } != 0 {
        return None;
    }
    let iter = Io(iter);
    loop {
        let service = Io(unsafe { IOIteratorNext(iter.0) });
        if service.0 == 0 {
            return None;
        }
        if int_property(service.0, "PrimaryUsagePage") == Some(VENDOR_USAGE_PAGE)
            && int_property(service.0, "PrimaryUsage") == Some(USAGE_ACCELEROMETER)
        {
            return Some(service);
        }
    }
}

/// Ask the accelerometer's own `AppleSPUHIDDriver` (a descendant of the
/// device) to power up and stream at 1 ms. Leaves the gyro and other SPU
/// sensors alone.
fn wake(device: IoObject) {
    let (Ok(plane), Ok(driver_class)) = (CString::new("IOService"), CString::new("AppleSPUHIDDriver")) else {
        return;
    };
    let mut iter: IoObject = 0;
    if unsafe { IORegistryEntryCreateIterator(device, plane.as_ptr(), ITERATE_RECURSIVELY, &mut iter) } != 0 {
        return;
    }
    let iter = Io(iter);
    loop {
        let entry = Io(unsafe { IOIteratorNext(iter.0) });
        if entry.0 == 0 {
            return;
        }
        if unsafe { IOObjectConformsTo(entry.0, driver_class.as_ptr()) } == 0 {
            continue;
        }
        for (key, value) in [
            ("SensorPropertyReportingState", 1),
            ("SensorPropertyPowerState", 1),
            ("ReportInterval", 1000),
        ] {
            let (key, value) = (Cf::string(key), Cf::i32(value));
            unsafe { IORegistryEntrySetCFProperty(entry.0, key.0, value.0) };
        }
    }
}

/// True when this Mac has the accelerometer (Apple Silicon MacBooks).
pub fn is_available() -> bool {
    find_accelerometer().is_some()
}

/// Seconds since the last key press/release or trackpad/mouse click,
/// system-wide. Needs no permission. Used to reject the chassis vibration
/// that typing and clicking produce.
pub fn seconds_since_input() -> f64 {
    const HID_SYSTEM_STATE: i32 = 1;
    // kCGEventLeftMouseDown/Up, RightMouseDown/Up, KeyDown/Up, OtherMouseDown/Up
    const EVENTS: [u32; 8] = [1, 2, 3, 4, 10, 11, 25, 26];
    EVENTS
        .iter()
        .map(|&e| unsafe { CGEventSourceSecondsSinceLastEventType(HID_SYSTEM_STATE, e) })
        .fold(f64::INFINITY, f64::min)
}

struct Context<F> {
    started: Instant,
    on_sample: F,
    last_sample: Instant,
    samples: u64,
}

extern "C" fn on_report<F: FnMut(f64, f32, f32, f32)>(
    context: *mut c_void,
    _result: i32,
    _sender: *mut c_void,
    _report_type: u32,
    _report_id: u32,
    report: *mut u8,
    report_length: CFIndex,
) {
    if context.is_null() || report.is_null() || report_length < 18 {
        return;
    }
    // A panic must not unwind into IOKit's C frames.
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let ctx = unsafe { &mut *(context as *mut Context<F>) };
        let bytes = unsafe { std::slice::from_raw_parts(report, report_length as usize) };
        let axis = |o: usize| i32::from_le_bytes([bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]]) as f32 / Q16;
        let now = Instant::now();
        ctx.last_sample = now;
        ctx.samples += 1;
        (ctx.on_sample)(now.duration_since(ctx.started).as_secs_f64(), axis(6), axis(10), axis(14));
    }));
}

/// Stream accelerometer samples `(seconds since start, x, y, z in g)` into
/// `on_sample` on the calling thread until `stop` is set. Blocks; run it on
/// a dedicated thread.
pub fn run<F: FnMut(f64, f32, f32, f32)>(stop: Arc<AtomicBool>, on_sample: F) -> Result<(), String> {
    let service = find_accelerometer().ok_or("no built-in accelerometer on this Mac")?;
    wake(service.0);

    let device = unsafe { IOHIDDeviceCreate(std::ptr::null(), service.0) };
    if device.is_null() {
        return Err("IOHIDDeviceCreate failed".into());
    }
    let device_ref = Cf(device as CFTypeRef);
    let opened = unsafe { IOHIDDeviceOpen(device, 0) };
    if opened != 0 {
        return Err(format!("could not open the accelerometer (IOReturn {:#x})", opened as u32));
    }
    // Waking again after open matches what every working reader does —
    // some machines ignore the first request until a client is attached.
    wake(service.0);

    let mut report = vec![0u8; 64];
    let started = Instant::now();
    // Owned through a raw pointer for the whole session: the callback and
    // this loop both touch it, but only ever on this thread.
    let ctx = Box::into_raw(Box::new(Context { started, on_sample, last_sample: started, samples: 0 }));
    let ctx_ptr = ctx as *mut c_void;
    let run_loop = unsafe { CFRunLoopGetCurrent() };
    unsafe {
        IOHIDDeviceRegisterInputReportCallback(device, report.as_mut_ptr(), report.len() as CFIndex, Some(on_report::<F>), ctx_ptr);
        IOHIDDeviceScheduleWithRunLoop(device, run_loop, kCFRunLoopDefaultMode);
    }

    let mut last_wake = Instant::now();
    let mut warned = false;
    while !stop.load(Ordering::Relaxed) {
        unsafe { CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.25, 0) };
        let (last_sample, samples) = unsafe { ((*ctx).last_sample, (*ctx).samples) };
        if last_sample.elapsed() > SILENCE_REWAKE && last_wake.elapsed() > SILENCE_REWAKE {
            if !warned {
                crate::log_to_file(&format!(
                    "Knock: accelerometer silent ({} samples so far) — re-waking",
                    samples
                ));
                warned = true;
            }
            wake(service.0);
            last_wake = Instant::now();
        } else if last_sample.elapsed() < SILENCE_REWAKE {
            warned = false;
        }
    }

    unsafe {
        IOHIDDeviceUnscheduleFromRunLoop(device, run_loop, kCFRunLoopDefaultMode);
        IOHIDDeviceRegisterInputReportCallback(device, report.as_mut_ptr(), report.len() as CFIndex, None, std::ptr::null_mut());
        IOHIDDeviceClose(device, 0);
    }
    drop(device_ref);
    // `report` and `ctx` outlive every callback: the device is unscheduled
    // and closed above, on this same thread that ran the callbacks.
    drop(unsafe { Box::from_raw(ctx) });
    drop(report);
    Ok(())
}
