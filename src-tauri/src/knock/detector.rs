//! Knock pattern detection on raw accelerometer samples. Pure and
//! platform-independent so it can be unit-tested and replayed offline.
//!
//! Tuned on recorded MacBook Pro data. Any bump — knock, keystroke, desk
//! thump — sets the whole chassis ringing at ~14 Hz for ~150 ms, and that
//! ringing looks the same whatever caused it. What marks a fresh strike is a
//! burst of 40–150 Hz energy, which the ringing lacks. So:
//!
//! 1. A 40 Hz high-pass filter per axis drops gravity and the ringing.
//! 2. An *impact* is that filtered magnitude crossing the sensitivity
//!    threshold; its peak is taken over a short hold window, a refractory
//!    period follows, and a much weaker impact right after a strong one is
//!    treated as its echo and ignored.
//! 3. A *pattern* is N impacts with knock-like spacing and similar strength,
//!    preceded by a moment of quiet and followed by a confirmation wait,
//!    longer than the pattern's own spacing, in which nothing else hits
//!    (steady bumps — typing, music, walking — never stay quiet afterwards).
//!    Impacts the caller vetoes (typing, clicking) break the pattern.

use crate::state::KnockSensitivity;

/// An impact within this many seconds of a key press or click is typing.
pub const INPUT_GUARD_SECS: f64 = 0.2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Config {
    /// Knocks in a pattern. With `max_knocks` above it, any count in
    /// `knocks..=max_knocks` is a pattern and `Pattern::knocks` says which.
    pub knocks: usize,
    pub max_knocks: usize,
    pub sensitivity: KnockSensitivity,
}

impl Config {
    /// Exactly `knocks` knocks.
    pub fn exactly(knocks: usize, sensitivity: KnockSensitivity) -> Self {
        Self { knocks, max_knocks: knocks, sensitivity }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pattern {
    /// How many knocks.
    pub knocks: usize,
    /// Sample time of the first and last knock, seconds.
    pub first: f64,
    pub last: f64,
    /// Strongest knock in the pattern (high-passed), g.
    pub peak_g: f32,
}

/// The SPU accelerometer streams at ~800 Hz (1 ms report interval).
const SAMPLE_RATE_HZ: f64 = 800.0;
const HIGHPASS_HZ: f64 = 40.0;
/// Window after a trigger in which the impact's peak is collected.
const HOLD: f64 = 0.02;
/// No new trigger this soon after the previous one.
const REFRACTORY: f64 = 0.12;
/// An impact this soon after a stronger one, and MAX_PEAK_RATIO times
/// weaker, is that one's echo.
const ECHO_WINDOW: f64 = 0.3;
/// Loudest / softest knock within one pattern.
const MAX_PEAK_RATIO: f32 = 2.5;
/// Longest pause between consecutive knocks of one pattern.
const MAX_GAP: f64 = 0.6;
/// A pattern may only start after this long without any impact.
const QUIET_BEFORE: f64 = 0.5;
/// After the last knock, nothing may hit for CONFIRM_FACTOR × the
/// pattern's longest gap (at least MIN_CONFIRM) before it fires — so a
/// steady rhythm always shows its next beat first. This is the delay
/// between the last knock and recording starting: 0.35 s for a quick
/// knock-knock.
const MIN_CONFIRM: f64 = 0.35;
const CONFIRM_FACTOR: f64 = 1.3;
/// A pattern that one more knock would extend (double when triples count
/// too) waits a little longer, so a third knock in an uneven rhythm still
/// lands inside the wait.
const MIN_CONFIRM_EXTENDABLE: f64 = 0.45;
const CONFIRM_FACTOR_EXTENDABLE: f64 = 1.7;

/// Minimum high-passed impact, g. Recorded knuckle knocks peak at
/// 0.019–0.042 g in this band; a still laptop stays under ~0.006 g.
fn threshold(s: KnockSensitivity) -> f32 {
    match s {
        KnockSensitivity::High => 0.010,
        KnockSensitivity::Medium => 0.013,
        KnockSensitivity::Low => 0.017,
    }
}

/// Second-order Butterworth high-pass (RBJ cookbook), one per axis.
#[derive(Clone, Copy)]
struct HighPass {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
}

impl HighPass {
    fn new(cutoff_hz: f64, rate_hz: f64) -> Self {
        let w0 = std::f64::consts::TAU * cutoff_hz / rate_hz;
        let alpha = w0.sin() / (2.0 * std::f64::consts::FRAC_1_SQRT_2);
        let cos = w0.cos();
        let a0 = 1.0 + alpha;
        Self {
            b0: ((1.0 + cos) / 2.0 / a0) as f32,
            b1: (-(1.0 + cos) / a0) as f32,
            b2: ((1.0 + cos) / 2.0 / a0) as f32,
            a1: (-2.0 * cos / a0) as f32,
            a2: ((1.0 - alpha) / a0) as f32,
            x1: 0.0,
            x2: 0.0,
            y1: 0.0,
            y2: 0.0,
        }
    }

    /// Start from a resting input so the first samples don't ring.
    fn prime(&mut self, x: f32) {
        self.x1 = x;
        self.x2 = x;
    }

    fn step(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.b1 * self.x1 + self.b2 * self.x2 - self.a1 * self.y1 - self.a2 * self.y2;
        self.x2 = self.x1;
        self.x1 = x;
        self.y2 = self.y1;
        self.y1 = y;
        y
    }
}

pub struct Detector {
    knocks: usize,
    max_knocks: usize,
    threshold: f32,
    filters: Option<[HighPass; 3]>,
    /// Trigger time and running peak of the impact being held.
    holding: Option<(f64, f32)>,
    last_trigger: f64,
    /// Latest non-echo impact (time, peak).
    prev_impact: Option<(f64, f32)>,
    /// Time of the latest non-echo impact, accepted or not.
    last_impact: f64,
    /// Accepted impacts of the pattern in progress: (time, peak).
    pending: Vec<(f64, f32)>,
    /// Complete pattern and the time it fires unless something hits first.
    armed: Option<(Pattern, f64)>,
}

impl Detector {
    pub fn new(config: Config) -> Self {
        Self {
            knocks: config.knocks.max(1),
            max_knocks: config.max_knocks.max(config.knocks.max(1)),
            threshold: threshold(config.sensitivity),
            filters: None,
            holding: None,
            last_trigger: f64::NEG_INFINITY,
            prev_impact: None,
            last_impact: f64::NEG_INFINITY,
            pending: Vec::new(),
            armed: None,
        }
    }

    /// Feed one sample (`t` seconds, acceleration in g). `accept(t_impact)`
    /// is asked once per impact and may veto it (typing, clicking).
    pub fn push(&mut self, t: f64, x: f32, y: f32, z: f32, accept: impl FnOnce(f64) -> bool) -> Option<Pattern> {
        let a = [x, y, z];
        let Some(filters) = self.filters.as_mut() else {
            let mut f = [HighPass::new(HIGHPASS_HZ, SAMPLE_RATE_HZ); 3];
            for i in 0..3 {
                f[i].prime(a[i]);
            }
            self.filters = Some(f);
            return None;
        };
        let mut mag_sq = 0.0;
        for i in 0..3 {
            let v = filters[i].step(a[i]);
            mag_sq += v * v;
        }
        let mag = mag_sq.sqrt();

        if let Some((start, peak)) = self.holding {
            if t - start < HOLD {
                self.holding = Some((start, peak.max(mag)));
                return None;
            }
            self.holding = None;
            self.on_impact(start, peak, accept);
        }
        if let Some((pattern, fire_at)) = self.armed {
            if t >= fire_at && self.holding.is_none() {
                // QUIET_BEFORE (counted from the last knock) keeps the next
                // pattern from starting straight away.
                self.armed = None;
                self.pending.clear();
                return Some(pattern);
            }
        }
        if mag >= self.threshold && t - self.last_trigger >= REFRACTORY {
            self.last_trigger = t;
            self.holding = Some((t, mag));
        }
        None
    }

    fn on_impact(&mut self, t: f64, peak: f32, accept: impl FnOnce(f64) -> bool) {
        if self
            .prev_impact
            .is_some_and(|(pt, pp)| t - pt < ECHO_WINDOW && peak * MAX_PEAK_RATIO < pp)
        {
            return;
        }
        self.prev_impact = Some((t, peak));
        let since_last = t - self.last_impact;
        self.last_impact = t;

        // Anything hitting during the confirmation wait means this was not
        // a clean pattern — unless it is the next knock of a longer one
        // (`pending` still holds the armed pattern's knocks then).
        let armed = self.armed.take();
        if !accept(t) {
            self.pending.clear();
            return;
        }
        if let Some((p, _)) = armed {
            let extends = p.knocks < self.max_knocks && self.pending.len() == p.knocks && t - p.last <= MAX_GAP;
            if !extends {
                self.pending.clear();
                return;
            }
        }
        if self.pending.last().is_some_and(|&(p, _)| t - p > MAX_GAP) {
            self.pending.clear();
        }
        if self.pending.is_empty() && since_last < QUIET_BEFORE {
            return;
        }
        self.pending.push((t, peak));
        if self.pending.len() < self.knocks {
            return;
        }
        let (lo, hi) = self.pending.iter().fold((f32::MAX, 0f32), |(lo, hi), p| (lo.min(p.1), hi.max(p.1)));
        let knocks = self.pending.len();
        if hi <= lo * MAX_PEAK_RATIO {
            let longest_gap = self.pending.windows(2).map(|w| w[1].0 - w[0].0).fold(0.0, f64::max);
            let extendable = knocks < self.max_knocks;
            let wait = if extendable {
                (CONFIRM_FACTOR_EXTENDABLE * longest_gap).max(MIN_CONFIRM_EXTENDABLE)
            } else {
                (CONFIRM_FACTOR * longest_gap).max(MIN_CONFIRM)
            };
            self.armed = Some((Pattern { knocks, first: self.pending[0].0, last: t, peak_g: hi }, t + wait));
            if extendable {
                return; // keep the knocks: a next one may extend the pattern
            }
        }
        self.pending.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f64 = 800.0;

    /// Gravity on -Z plus faint sensor noise, with knocks mixed in. Each
    /// knock is a short 120 Hz strike on top of 14 Hz chassis ringing, like
    /// the recorded ones.
    struct Signal {
        knocks: Vec<(f64, f32)>,
    }

    impl Signal {
        fn sample(&self, t: f64) -> [f32; 3] {
            let noise = ((t * 12_345.678).sin() * 0.0008) as f32;
            let mut a = [noise, -noise, -1.0 + noise];
            for &(at, amp) in &self.knocks {
                let dt = t - at;
                if (0.0..0.2).contains(&dt) {
                    let strike = amp * (-(dt as f32) / 0.01).exp() * ((dt * 120.0 * std::f64::consts::TAU).sin() as f32);
                    let ring = 2.0 * amp * (-(dt as f32) / 0.07).exp() * ((dt * 14.0 * std::f64::consts::TAU).sin() as f32);
                    a[2] += strike + ring;
                    a[1] += 0.4 * (strike + ring);
                }
            }
            a
        }
    }

    fn run(config: Config, knocks: &[(f64, f32)], secs: f64, mut accept: impl FnMut(f64) -> bool) -> Vec<Pattern> {
        let signal = Signal { knocks: knocks.to_vec() };
        let mut d = Detector::new(config);
        let mut out = Vec::new();
        for i in 0..(secs * RATE) as usize {
            let t = i as f64 / RATE;
            let [x, y, z] = signal.sample(t);
            if let Some(p) = d.push(t, x, y, z, |ti| accept(ti)) {
                out.push(p);
            }
        }
        out
    }

    fn double(s: KnockSensitivity) -> Config {
        Config::exactly(2, s)
    }

    #[test]
    fn double_knock_fires_once() {
        let p = run(double(KnockSensitivity::Medium), &[(1.0, 0.04), (1.17, 0.035)], 3.0, |_| true);
        assert_eq!(p.len(), 1, "{p:?}");
        assert!((p[0].first - 1.0).abs() < 0.01 && (p[0].last - 1.17).abs() < 0.01, "{p:?}");
    }

    #[test]
    fn slow_double_knock_fires() {
        let p = run(double(KnockSensitivity::Medium), &[(1.0, 0.04), (1.5, 0.04)], 3.0, |_| true);
        assert_eq!(p.len(), 1, "{p:?}");
    }

    #[test]
    fn single_knock_does_nothing() {
        assert!(run(double(KnockSensitivity::High), &[(1.0, 0.05)], 3.0, |_| true).is_empty());
    }

    #[test]
    fn knocks_too_far_apart_do_nothing() {
        assert!(run(double(KnockSensitivity::Medium), &[(1.0, 0.04), (1.8, 0.04)], 3.0, |_| true).is_empty());
    }

    #[test]
    fn faint_taps_below_threshold_do_nothing() {
        assert!(run(double(KnockSensitivity::Low), &[(1.0, 0.004), (1.2, 0.004)], 3.0, |_| true).is_empty());
    }

    #[test]
    fn typing_vetoes_the_pattern() {
        assert!(run(double(KnockSensitivity::Medium), &[(1.0, 0.04), (1.2, 0.04)], 3.0, |_| false).is_empty());
        // A vetoed bump right before a real pair also blocks it (no quiet before).
        let knocks = [(0.8, 0.04), (1.1, 0.04), (1.3, 0.04)];
        let p = run(double(KnockSensitivity::Medium), &knocks, 3.0, |t| t > 1.0);
        assert!(p.is_empty(), "{p:?}");
    }

    #[test]
    fn steady_bumps_never_fire() {
        // Hard typing, music through the speakers, walking: bumps keep coming.
        for spacing in [0.15, 0.25, 0.4, 0.55] {
            let knocks: Vec<_> = (0..12).map(|i| (1.0 + i as f64 * spacing, 0.04)).collect();
            let p = run(double(KnockSensitivity::Medium), &knocks, 8.0, |_| true);
            assert!(p.is_empty(), "spacing {spacing}: {p:?}");
        }
    }

    #[test]
    fn triple_needs_three() {
        let triple = Config::exactly(3, KnockSensitivity::Medium);
        assert!(run(triple, &[(1.0, 0.04), (1.2, 0.04)], 3.0, |_| true).is_empty());
        assert_eq!(run(triple, &[(1.0, 0.04), (1.2, 0.04), (1.4, 0.04)], 3.0, |_| true).len(), 1);
    }

    #[test]
    fn triple_knock_in_double_mode_does_nothing() {
        let p = run(double(KnockSensitivity::Medium), &[(1.0, 0.04), (1.2, 0.04), (1.4, 0.04)], 3.0, |_| true);
        assert!(p.is_empty(), "{p:?}");
    }

    fn double_or_triple() -> Config {
        Config { knocks: 2, max_knocks: 3, sensitivity: KnockSensitivity::Medium }
    }

    #[test]
    fn split_mode_tells_double_from_triple() {
        let p = run(double_or_triple(), &[(1.0, 0.04), (1.17, 0.035)], 3.0, |_| true);
        assert_eq!(p.iter().map(|p| p.knocks).collect::<Vec<_>>(), vec![2], "{p:?}");
        let p = run(double_or_triple(), &[(1.0, 0.04), (1.17, 0.035), (1.34, 0.04)], 3.0, |_| true);
        assert_eq!(p.iter().map(|p| p.knocks).collect::<Vec<_>>(), vec![3], "{p:?}");
        assert!((p[0].first - 1.0).abs() < 0.01 && (p[0].last - 1.34).abs() < 0.01, "{p:?}");
        // Uneven rhythm: third knock a bit slower than the second.
        let p = run(double_or_triple(), &[(1.0, 0.04), (1.2, 0.04), (1.5, 0.04)], 3.0, |_| true);
        assert_eq!(p.iter().map(|p| p.knocks).collect::<Vec<_>>(), vec![3], "{p:?}");
    }

    #[test]
    fn split_mode_rejects_four_knocks_and_steady_bumps() {
        let p = run(double_or_triple(), &[(1.0, 0.04), (1.2, 0.04), (1.4, 0.04), (1.6, 0.04)], 3.0, |_| true);
        assert!(p.is_empty(), "{p:?}");
        for spacing in [0.15, 0.25, 0.4, 0.55] {
            let knocks: Vec<_> = (0..12).map(|i| (1.0 + i as f64 * spacing, 0.04)).collect();
            let p = run(double_or_triple(), &knocks, 8.0, |_| true);
            assert!(p.is_empty(), "spacing {spacing}: {p:?}");
        }
    }

    #[test]
    fn two_patterns_in_a_row() {
        let knocks = [(1.0, 0.04), (1.2, 0.04), (3.0, 0.04), (3.2, 0.04)];
        assert_eq!(run(double(KnockSensitivity::Medium), &knocks, 4.0, |_| true).len(), 2);
    }

    #[test]
    fn mismatched_peaks_are_not_a_pattern() {
        let p = run(double(KnockSensitivity::High), &[(1.0, 0.2), (1.2, 0.02)], 3.0, |_| true);
        assert!(p.is_empty(), "{p:?}");
    }

    #[test]
    fn slow_tilt_is_not_a_knock() {
        let mut d = Detector::new(double(KnockSensitivity::High));
        for i in 0..(3.0 * RATE) as usize {
            let t = i as f64 / RATE;
            // Laptop tilted by ~30° and back over a second.
            let angle = (0.5 * (1.0 - (t * std::f64::consts::PI * 2.0).cos())) as f32 * 0.5;
            assert!(d.push(t, angle.sin(), 0.0, -angle.cos(), |_| true).is_none(), "fired at {t}");
        }
    }

    /// Replays a recording from the calibration recorder and prints what
    /// fires per labelled phase:
    /// TYPEURVOICE_KNOCK_CSV=samples.csv [TYPEURVOICE_KNOCK_EVENTS=events.csv] \
    ///   cargo test --release --lib knock_replay -- --ignored --nocapture
    #[test]
    #[ignore]
    fn knock_replay() {
        let Ok(path) = std::env::var("TYPEURVOICE_KNOCK_CSV") else { return };
        let text = std::fs::read_to_string(path).unwrap();
        let samples: Vec<[f64; 4]> = text
            .lines()
            .skip(1)
            .filter_map(|l| {
                let v: Vec<f64> = l.split(',').filter_map(|s| s.parse().ok()).collect();
                (v.len() == 4).then(|| [v[0], v[1], v[2], v[3]])
            })
            .collect();
        let mut phases: Vec<(String, f64, f64)> = Vec::new();
        if let Some(events) = std::env::var("TYPEURVOICE_KNOCK_EVENTS").ok().and_then(|p| std::fs::read_to_string(p).ok()) {
            let mut start = 0.0;
            for l in events.lines().skip(1) {
                let f: Vec<&str> = l.split(',').collect();
                match f.get(1) {
                    Some(&"phase_start") => start = f[0].parse().unwrap(),
                    Some(&"phase_end") => phases.push((f[2].to_string(), start, f[0].parse().unwrap())),
                    _ => {}
                }
            }
        }
        for sensitivity in [KnockSensitivity::High, KnockSensitivity::Medium, KnockSensitivity::Low] {
            for (knocks, max_knocks) in [(2, 2), (3, 3), (2, 3)] {
                let mut d = Detector::new(Config { knocks, max_knocks, sensitivity });
                let fired: Vec<Pattern> = samples
                    .iter()
                    .filter_map(|s| d.push(s[0], s[1] as f32, s[2] as f32, s[3] as f32, |_| true))
                    .collect();
                let first_phase = phases.first().map(|p| p.1).unwrap_or(f64::INFINITY);
                let mut row = format!("pre={}", fired.iter().filter(|p| p.last < first_phase).count());
                for (name, a, b) in &phases {
                    row += &format!(" {}={}", name, fired.iter().filter(|p| (*a..*b).contains(&p.last)).count());
                }
                let kinds: Vec<usize> = fired.iter().map(|p| p.knocks).collect();
                println!("{:?} x{}-{}: total={} {} kinds={:?}", sensitivity, knocks, max_knocks, fired.len(), row, kinds);
            }
        }
    }
}
