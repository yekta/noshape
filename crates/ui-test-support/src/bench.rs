//! T2 benches of the studios' views at scale (docs/TESTS.md "UI benches"): the real view in a test window whose text
//! is shaped as the app's (Linux: cosmic-text with the app's Roboto; elsewhere GPUI's no-op text system), each step
//! timed on the UI thread: a frame (`Window::draw`: render, layout, prepaint, paint) or a handler and the frame after
//! it. Work a step hands to a background task isn't counted: the step must show it is busy (a spinner, a progress row).

use std::time::Instant;

/// The app's UI fonts (crates/app/src/main.rs `load_fonts`).
fn app_fonts() -> Vec<std::borrow::Cow<'static, [u8]>> {
    vec![
        std::borrow::Cow::Borrowed(include_bytes!("../../app/assets/fonts/Roboto-Regular.ttf")),
        std::borrow::Cow::Borrowed(include_bytes!("../../app/assets/fonts/Roboto-Medium.ttf")),
        std::borrow::Cow::Borrowed(include_bytes!("../../app/assets/fonts/Roboto-Bold.ttf")),
        std::borrow::Cow::Borrowed(include_bytes!("../../app/assets/fonts/Roboto-Italic.ttf")),
        std::borrow::Cow::Borrowed(include_bytes!("../../app/assets/fonts/Inconsolata-Regular.ttf")),
    ]
}

/// Runs `f` with a test context that shapes text as the app does, then tears it down as `#[gpui::test]` does.
pub fn run(f: impl FnOnce(&mut gpui::TestAppContext)) {
    let dispatcher = gpui::TestDispatcher::new(0);
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    let text: std::sync::Arc<dyn gpui::PlatformTextSystem> = std::sync::Arc::new(gpui_wgpu::CosmicTextSystem::new("Roboto"));
    #[cfg(not(any(target_os = "linux", target_os = "freebsd")))]
    let text: std::sync::Arc<dyn gpui::PlatformTextSystem> = std::sync::Arc::new(gpui::NoopTextSystem);
    let mut cx = gpui::TestAppContext::build_with_text_system(dispatcher.clone(), None, text);
    cx.update(|cx| cx.text_system().add_fonts(app_fonts())).expect("the app's fonts");
    f(&mut cx);
    cx.run_until_parked();
    cx.update(|cx| {
        cx.background_executor().forbid_parking();
        cx.quit();
    });
    cx.run_until_parked();
    drop(cx);
    dispatcher.drain_tasks();
}

/// A step's times (ms).
pub struct Times(Vec<f64>);

impl Times {
    /// Times taken by hand (ms), for a step that times only part of what it does.
    pub fn from_ms(ms: Vec<f64>) -> Self {
        Self(ms)
    }

    pub fn median(&self) -> f64 {
        self.at(0.5)
    }

    pub fn p95(&self) -> f64 {
        self.at(0.95)
    }

    fn at(&self, q: f64) -> f64 {
        let mut v = self.0.clone();
        v.sort_by(f64::total_cmp);
        v.get(((v.len() as f64 - 1.0) * q).round() as usize).copied().unwrap_or(0.0)
    }
}

/// `n` runs of `step`, each timed (`NOSHAPE_BENCH_REPEAT=<k>`: k times as many, for a profiler to see the steps).
pub fn time(n: usize, mut step: impl FnMut()) -> Times {
    let repeat = std::env::var("NOSHAPE_BENCH_REPEAT").ok().and_then(|r| r.parse::<usize>().ok()).unwrap_or(1).max(1);
    Times((0..n * repeat).map(|_| {
        let t = Instant::now();
        step();
        t.elapsed().as_secs_f64() * 1e3
    }).collect())
}

/// One timed step's line ("BENCH <case> | <step>: median, p95 ms"), printed, and whether it fits a frame (16 ms). Only
/// an optimised build (`--profile ui-bench`) is held to the frame: a debug build's times are several times the app's.
pub fn report(case: &str, step: &str, t: &Times) -> bool {
    println!("BENCH {case} | {step}: median {:.2} ms, p95 {:.2} ms ({} runs)", t.median(), t.p95(), t.0.len());
    t.median() <= FRAME_MS || cfg!(debug_assertions)
}

/// One frame at 60 Hz.
pub const FRAME_MS: f64 = 16.0;
