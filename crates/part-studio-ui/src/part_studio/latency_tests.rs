//! Speed of everyday Part Studio interactions on live's 30-feature part (tests/perf/live-30-features.noshape), with
//! real frames and the GPU's own pick (docs/PLAN.md 2.7: at least twice as fast as live where the work is local):
//! a face lighting under the pointer, a click's selection, a feature dialog opening on its preview, a dragged
//! dimension's preview, and the part redrawn after ✓. Wall-clock medians in a release build: T2.

use super::feel::{Pointer, gpu_pointer};
use super::*;
use std::time::{Duration, Instant};

/// Repeats of each interaction.
const RUNS: usize = 7;

/// Live's times (behaviours.md) and ours at most half of them (PLAN 2.7), ms.
struct Target {
    what: &'static str,
    live: &'static str,
    max_ms: f64,
}

const TARGETS: [Target; 5] = [
    // "Pre-highlight": on a hardware-GL desktop ≈ 10–50 ms + one pick (2–10 ms) after the pointer stops.
    Target { what: "hover: a face lit after a jump", live: "10-50 ms + pick (2-10)", max_ms: 30.0 },
    // Not timed live; one frame of a 60 Hz display.
    Target { what: "click: the face selected and drawn", live: "(a frame)", max_ms: 16.7 },
    // "speed: core edit commit": Enter → the preview's regeneration reply 0.2–0.3 s.
    Target { what: "dialog: Extrude 1 open on its preview", live: "200-300 ms", max_ms: 100.0 },
    // interaction-feel "preview timing while dragging (every ~0.4 s)".
    Target { what: "drag: a depth step's preview drawn", live: "~400 ms", max_ms: 100.0 },
    // "speed: core edit commit": ✓ → the new tessellation 0.31–0.35 s.
    Target { what: "commit: the part redrawn after ✓", live: "310-350 ms", max_ms: 155.0 },
];

/// The 30-feature part in a test window drawing real frames, regenerated and shown.
fn thirty_features(cx: &mut TestAppContext) -> (AnyWindowHandle, Entity<PartStudioView>) {
    RENDERING.set(true);
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/perf/live-30-features.noshape");
    let (window, view) = part_studio_with(cx, "latency", |path| {
        std::fs::copy(&src, path).unwrap();
        crate::session::Session::open(path).unwrap()
    });
    wait_until(cx, window, |cx| !view.read(cx).ui_busy() && view.read(cx).display.bodies.iter().any(|b| b.kind == noshape_model::display::BodyKind::Solid));
    (window, view)
}

/// A display's frame interval (120 Hz, a ProMotion Mac): the window is laid out and drawn on this tick, as the app's.
const FRAME: Duration = Duration::from_micros(8333);

/// Runs `go`, then until `done` holds at a frame: GPU results delivered and tasks run continuously, the window drawn
/// on the frame tick, the test clock kept with the real one (the hover's pause and a commit's burst wait are timers).
/// The time to that frame, ms.
fn timed(cx: &mut TestAppContext, window: AnyWindowHandle, view: &Entity<PartStudioView>, go: impl FnOnce(&mut TestAppContext), done: impl Fn(&gpui_kit::App) -> bool) -> f64 {
    let t0 = Instant::now();
    go(cx);
    let (mut last, mut drawn) = (t0, t0);
    let vp = cx.update(|cx| view.read(cx).viewport.clone());
    loop {
        cx.update(|cx| view.read(cx).gpu.deliver_frames());
        cx.run_until_parked();
        let now = Instant::now();
        cx.executor().advance_clock(now - last);
        last = now;
        if now - drawn >= FRAME {
            cx.update_window(window, |_, w, cx| w.draw(cx).clear(cx)).unwrap();
            drawn = now;
            if cx.update(|cx| done(cx) && !vp.read(cx).pick_pending()) {
                return t0.elapsed().as_secs_f64() * 1000.0;
            }
        }
        assert!(t0.elapsed() < Duration::from_secs(60), "timed out");
        std::thread::sleep(Duration::from_micros(100));
    }
}

/// Frames and tasks run for `dur` of real time (a hand resting, the time between two mouse moves).
fn pump(cx: &mut TestAppContext, window: AnyWindowHandle, view: &Entity<PartStudioView>, dur: Duration) {
    let t0 = Instant::now();
    let _ = timed(cx, window, view, |_| {}, |_| t0.elapsed() >= dur);
}

/// A pointer move to viewport point `at`.
fn move_to(cx: &mut TestAppContext, window: AnyWindowHandle, pointer: &Pointer, at: DVec2) {
    let win = pointer.window_point(cx, at);
    gpui_kit::VisualTestContext::from_window(window, cx).simulate_mouse_move(win, None, Modifiers::default());
}

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(f64::total_cmp);
    v[v.len() / 2]
}

/// Two points on the part's screen image a little apart (its centre and a third of the way to its first vertex).
fn two_points_on_the_part(cx: &mut TestAppContext, view: &Entity<PartStudioView>) -> (DVec2, DVec2) {
    cx.update(|cx| {
        let v = view.read(cx);
        let b = v.display.bodies.iter().find(|b| b.kind == noshape_model::display::BodyKind::Solid).unwrap();
        let p = |q: &[f32; 3]| glam::DVec3::new(q[0] as f64, q[1] as f64, q[2] as f64);
        let c = b.mesh.positions.iter().fold(glam::DVec3::ZERO, |c, q| c + p(q)) / b.mesh.positions.len() as f64;
        let vp = v.viewport.read(cx);
        let (sc, s0) = (vp.project(c).unwrap(), vp.project(p(&b.mesh.positions[0])).unwrap());
        (sc, sc + (s0 - sc) / 3.0)
    })
}

#[gpui::test]
#[ignore = "T2: wall-clock timing; a release build on a quiet machine (the Mac)"]
fn everyday_interactions_beat_live_on_the_30_feature_part(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let (window, view) = thirty_features(cx);
    let pointer = Pointer::new(cx, window, &view);
    let (a, b) = two_points_on_the_part(cx, &view);
    let mut ms: [Vec<f64>; 5] = Default::default();
    // A while of slow hovering over the part first, as anyone's session starts (the pick's running average settles).
    for k in 0..40 {
        move_to(cx, window, &pointer, a + DVec2::new(k as f64 * 1.5, 0.0));
        pump(cx, window, &view, Duration::from_millis(16));
    }

    // Hover and click, alternating between the two points.
    for i in 0..RUNS * 2 {
        let (from, at) = if i % 2 == 0 { (b, a) } else { (a, b) };
        cx.update(|cx| view.update(cx, |v, cx| {
            v.selection.clear();
            v.ui.last_click = None;
            v.rebuild_scene(cx);
        }));
        // A hand resting, then live's approach (behaviours "pre-highlight": 8 moves 16 ms apart, the last on the face),
        // timed from the last move.
        pump(cx, window, &view, Duration::from_millis(200));
        let before = cx.update(|cx| view.read(cx).hover.clone());
        for k in 1..8 {
            move_to(cx, window, &pointer, from + (at - from) * (k as f64 / 8.0));
            pump(cx, window, &view, Duration::from_millis(16));
        }
        let win = pointer.window_point(cx, at);
        let t = timed(cx, window, &view, |cx| gpui_kit::VisualTestContext::from_window(window, cx).simulate_mouse_move(win, None, Modifiers::default()), |cx| {
            let v = view.read(cx);
            v.hover.is_some() && v.hover != before && v.ui.scene_task.is_none() && pointer.vp.read(cx).hover_settled()
        });
        ms[0].push(t);
        let lit = cx.update(|cx| view.read(cx).hover.clone());
        let t = timed(cx, window, &view, |cx| gpui_kit::VisualTestContext::from_window(window, cx).simulate_click(win, Modifiers::default()), |cx| {
            let v = view.read(cx);
            v.selection.first() == lit.as_ref() && v.ui.scene_task.is_none()
        });
        ms[1].push(t);
    }

    // Extrude 1 (the plate, under all 29 others): open, depth steps as a drag gives them, ✓.
    let extrude = cx.update(|cx| view.read(cx).studio(cx).and_then(|s| s.features.iter().find(|f| f.type_id == "extrude").map(|f| f.id))).expect("an Extrude");
    // (Each waits for a display applied after it: a preview or the regeneration's, `display_seq`.)
    let seq = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).display_seq);
    for run in 0..RUNS {
        cx.update(|cx| view.update(cx, |v, _| v.selection.clear()));
        let s0 = seq(cx);
        let t = timed(cx, window, &view, |cx| {
            cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.edit_feature(extrude, w, cx))).unwrap();
        }, |cx| view.read(cx).edit.is_some() && view.read(cx).display_seq > s0 && !view.read(cx).ui_busy());
        ms[2].push(t);
        for step in 0..3 {
            let text = format!("{:.3} mm", 10.0 + (run * 3 + step) as f64 * 0.137);
            let s0 = seq(cx);
            let t = timed(cx, window, &view, |cx| {
                cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| {
                    if let Some(e) = &v.edit {
                        e.dialog.update(cx, |d, cx| d.set_expr_text("depth", &text, w, cx));
                    }
                }))
                .unwrap();
            }, |cx| view.read(cx).display_seq > s0 && !view.read(cx).ui_busy());
            ms[3].push(t);
        }
        let s0 = seq(cx);
        let t = timed(cx, window, &view, |cx| {
            cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.accept_open_dialog(w, cx))).unwrap();
        }, |cx| view.read(cx).edit.is_none() && view.read(cx).display_seq > s0 && !view.read(cx).ui_busy());
        ms[4].push(t);
    }

    let mut report = vec![format!("{:<40} {:>9} {:>9}  live", "", "median", "target")];
    let mut slower = Vec::new();
    for (target, runs) in TARGETS.iter().zip(ms) {
        let m = median(runs.clone());
        report.push(format!("{:<40} {:>6.1} ms {:>6.1} ms  {}  (runs: {})", target.what, m, target.max_ms, target.live, runs.iter().map(|x| format!("{x:.1}")).collect::<Vec<_>>().join(" ")));
        if m > target.max_ms {
            slower.push(target.what);
        }
    }
    report.push(format!("average pick: {:.1} ms", cx.update(|cx| pointer.vp.read(cx).average_pick_ms())));
    eprintln!("{}", report.join("\n"));
    assert!(slower.is_empty(), "slower than half of live's time: {slower:?}");
}
