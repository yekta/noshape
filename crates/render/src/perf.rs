//! Lightweight frame-time instrumentation, enabled with `NOSHAPE_PERF=1`.
//!
//! Stages record durations into named accumulators; every [`REPORT_EVERY`]
//! samples of a stage a summary line (mean / p50 / p95 / max in ms) is logged
//! with target `perf`. When disabled, [`enabled`] is a cached `false` and the
//! callers skip all timing work.

use parking_lot::Mutex;
use std::collections::BTreeMap;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

/// Samples per stage between summary lines.
pub const REPORT_EVERY: usize = 120;

/// Whether `NOSHAPE_PERF` is set (to anything but `0`).
pub fn enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var("NOSHAPE_PERF").is_ok_and(|v| v != "0" && !v.is_empty()))
}

/// Process start reference for "time since launch" logs.
pub fn process_start() -> Instant {
    static T0: OnceLock<Instant> = OnceLock::new();
    *T0.get_or_init(Instant::now)
}

/// Log a one-off milestone (startup) with the time since [`process_start`].
pub fn milestone(what: &str) {
    if enabled() {
        log::info!(target: "perf", "{what}: {:.1} ms since start", process_start().elapsed().as_secs_f64() * 1000.0);
    }
}

#[derive(Default)]
struct Stage {
    samples: Vec<f32>,
}

fn stages() -> &'static Mutex<BTreeMap<&'static str, Stage>> {
    static S: OnceLock<Mutex<BTreeMap<&'static str, Stage>>> = OnceLock::new();
    S.get_or_init(Default::default)
}

/// Record one sample of `stage`.
pub fn record(stage: &'static str, d: Duration) {
    record_ms(stage, d.as_secs_f32() * 1000.0);
}

/// Record one sample of `stage` in milliseconds.
pub fn record_ms(stage: &'static str, ms: f32) {
    if !enabled() {
        return;
    }
    if period_due() {
        flush();
    }
    let mut s = stages().lock();
    let st = s.entry(stage).or_default();
    st.samples.push(ms);
    if st.samples.len() >= REPORT_EVERY {
        let line = summarize(stage, &mut st.samples);
        st.samples.clear();
        drop(s);
        log::info!(target: "perf", "{line}");
    }
}

/// `NOSHAPE_PERF_PERIOD=<seconds>`: also flush all stages periodically (for
/// interactive sessions where stages rarely reach [`REPORT_EVERY`] samples).
fn period_due() -> bool {
    static PERIOD: OnceLock<Option<Duration>> = OnceLock::new();
    static LAST: Mutex<Option<Instant>> = Mutex::new(None);
    let Some(p) = *PERIOD.get_or_init(|| std::env::var("NOSHAPE_PERF_PERIOD").ok().and_then(|v| v.parse::<f64>().ok()).map(Duration::from_secs_f64)) else { return false };
    let mut last = LAST.lock();
    let now = Instant::now();
    match *last {
        Some(t) if now - t < p => false,
        _ => {
            let due = last.is_some();
            *last = Some(now);
            due
        }
    }
}

/// Log and reset every stage that has samples (e.g. at the end of a benchmark).
pub fn flush() {
    if !enabled() {
        return;
    }
    let mut s = stages().lock();
    let lines: Vec<String> = s.iter_mut().filter(|(_, st)| !st.samples.is_empty()).map(|(k, st)| summarize(k, &mut st.samples)).collect();
    for st in s.values_mut() {
        st.samples.clear();
    }
    drop(s);
    for l in lines {
        log::info!(target: "perf", "{l}");
    }
}

/// Drop the collected per-frame samples (`r.`, `w.`, `g.`, `v.` stages)
/// without logging them; rare events (scene builds, regens) are kept.
pub fn reset() {
    for (k, st) in stages().lock().iter_mut() {
        if ["r.", "w.", "g.", "v."].iter().any(|p| k.starts_with(p)) {
            st.samples.clear();
        }
    }
}

/// Unit of a stage's samples: `*_mb` stages count megabytes, `*_count`
/// stages plain numbers, everything else milliseconds.
fn unit_of(stage: &str) -> &'static str {
    if stage.ends_with("_mb") {
        "MB"
    } else if stage.ends_with("_count") || stage.ends_with("instances") {
        ""
    } else {
        "ms"
    }
}

fn summarize(stage: &str, v: &mut [f32]) -> String {
    let s = Stats::of(v);
    format!("{stage:<22} n={:<4} mean {:7.2}  p50 {:7.2}  p95 {:7.2}  max {:7.2} {}", s.n, s.mean, s.p50, s.p95, s.max, unit_of(stage))
}

/// Mean and percentiles of a set of samples.
#[derive(Clone, Copy, Debug, Default)]
pub struct Stats {
    pub n: usize,
    pub mean: f32,
    pub p50: f32,
    pub p95: f32,
    pub p99: f32,
    pub max: f32,
}

impl Stats {
    /// Statistics of `v` (sorted in place); all zero when `v` is empty.
    pub fn of(v: &mut [f32]) -> Self {
        let n = v.len();
        if n == 0 {
            return Self::default();
        }
        v.sort_by(|a, b| a.total_cmp(b));
        let q = |p: f32| v[((n as f32 - 1.0) * p).round() as usize];
        Self { n, mean: v.iter().sum::<f32>() / n as f32, p50: q(0.5), p95: q(0.95), p99: q(0.99), max: v[n - 1] }
    }
}

/// CPU time consumed by the calling thread so far (`None` where the platform
/// has no per-thread clock). Unlike wall time it does not grow while the
/// thread waits or is preempted, so it measures the work itself on a busy
/// machine.
pub fn thread_cpu_time() -> Option<Duration> {
    #[cfg(unix)]
    {
        let mut ts = libc::timespec { tv_sec: 0, tv_nsec: 0 };
        // SAFETY: `ts` is a valid, writable timespec.
        let r = unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut ts) };
        (r == 0).then(|| Duration::new(ts.tv_sec as u64, ts.tv_nsec as u32))
    }
    #[cfg(not(unix))]
    {
        None
    }
}

/// Scoped timer: records on drop (no-op when disabled).
pub struct Scope {
    stage: &'static str,
    start: Option<Instant>,
}

impl Drop for Scope {
    fn drop(&mut self) {
        if let Some(t) = self.start {
            record(self.stage, t.elapsed());
        }
    }
}

/// Time the rest of the enclosing scope as `stage`.
pub fn scope(stage: &'static str) -> Scope {
    Scope { stage, start: enabled().then(Instant::now) }
}
