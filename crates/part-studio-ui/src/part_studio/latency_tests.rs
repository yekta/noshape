//! Speed of everyday Part Studio interactions on live's 30-feature part (tests/perf/live-30-features.noshape), with
//! real frames and the GPU's own pick (docs/PLAN.md 2.7: at least twice as fast as live where the work is local):
//! a face lighting under the pointer, a click's selection, a feature dialog opening on its preview, a dragged
//! dimension's preview, the part redrawn after ✓, a context menu, a sketch entered and left, and Undo. Wall-clock
//! medians in a release build: T2.

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

const TARGETS: [Target; 9] = [
    // "Pre-highlight": on a hardware-GL desktop ≈ 10–50 ms + one pick (2–10 ms) after the pointer stops.
    Target { what: "hover: a face lit after a jump", live: "10-50 ms + pick (2-10)", max_ms: 30.0 },
    // Live psui/timings-1117 (software GL): pointerup → the selected face drawn 57–67 ms; held to one 60 Hz frame.
    Target { what: "click: the face selected and drawn", live: "57-67 ms", max_ms: 16.7 },
    // Live psui/timings-1117: double-click → the dialog 322–448 ms, its preview drawn 0.73–2.2 s; "speed: core edit
    // commit": Enter → the preview's regeneration reply 0.2–0.3 s. Live ui-speed-1118 (medians of 10): the toolbar's
    // Extrude dialog shown 221 ms, settled 1.3 s; the first preview after a face pick settled 2.0 s.
    Target { what: "dialog: Extrude 1 open on its preview", live: "322-448 ms (preview 0.7-2.2 s)", max_ms: 100.0 },
    // interaction-feel "preview timing while dragging (every ~0.4 s)".
    Target { what: "drag: a depth step's preview drawn", live: "~400 ms", max_ms: 100.0 },
    // "speed: core edit commit": ✓ → the new tessellation 0.31–0.35 s. Live ui-speed-1118: ✓ → regeneration shown
    // 107 ms, settled 1.1 s; sketch enter / exit and undo settled 0.9–1.3 s; a tab switch's list 1.5–1.7 s; a
    // document opened: the part shaded 7.6 s, the feature list 11.4 s.
    Target { what: "commit: the part redrawn after ✓", live: "310-350 ms", max_ms: 155.0 },
    // Live ui-speed-1118 (block, input → 60 fps screen frames): a face's context menu shown 117 ms after the
    // right-click; a sketch entered on a selected face 226 ms (first change), left with ✓ 117 ms (the list), its
    // Undo 107 ms (the list).
    Target { what: "menu: the context menu shown", live: "117 ms", max_ms: 58.5 },
    Target { what: "sketch: entered on a selected plane", live: "226 ms", max_ms: 113.0 },
    Target { what: "sketch: left with ✓, the part redrawn", live: "117 ms", max_ms: 58.5 },
    Target { what: "undo: the sketch undone and redrawn", live: "107 ms", max_ms: 53.5 },
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
    let mut ms: [Vec<f64>; 9] = Default::default();
    // A while of slow hovering over the part first, as anyone's session starts (the pick's running average settles).
    for k in 0..40 {
        move_to(cx, window, &pointer, a + DVec2::new(k as f64 * 1.5, 0.0));
        pump(cx, window, &view, Duration::from_millis(16));
    }

    // Hover and click, alternating between the two points (the first place something lit is kept for the menu).
    let mut lit_at = None;
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
        if lit_at.is_none() && lit.is_some() {
            lit_at = Some(at);
        }
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

    // The context menu of what is under the pointer, a sketch on Top, ✓, and the sketch's Undo.
    let at = lit_at.expect("something lit under the pointer");
    move_to(cx, window, &pointer, at);
    pump(cx, window, &view, Duration::from_millis(300));
    let win = pointer.window_point(cx, at);
    for _ in 0..RUNS {
        let t = timed(cx, window, &view, |cx| {
            let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
            vcx.simulate_mouse_down(win, gpui_kit::MouseButton::Right, Modifiers::default());
            vcx.simulate_mouse_up(win, gpui_kit::MouseButton::Right, Modifiers::default());
        }, |cx| view.read(cx).context_menu.is_some());
        ms[5].push(t);
        cx.update(|cx| view.update(cx, |v, cx| v.close_context_menu(cx)));
        pump(cx, window, &view, Duration::from_millis(50));

        let t = timed(cx, window, &view, |cx| {
            cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| {
                v.selection = vec![Reference::Plane(DatumPlane::Top)];
                v.start_feature("sketch", w, cx);
            }))
            .unwrap();
        }, |cx| view.read(cx).sketch.is_some() && !view.read(cx).ui_busy());
        ms[6].push(t);
        let s0 = seq(cx);
        let t = timed(cx, window, &view, |cx| {
            cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| {
                let _ = v.finish_sketch(w, cx);
            })).unwrap();
        }, |cx| view.read(cx).sketch.is_none() && view.read(cx).display_seq > s0 && !view.read(cx).ui_busy());
        ms[7].push(t);
        let count = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).studio(cx).map_or(0, |s| s.features.len()));
        let (n, s0) = (count(cx), seq(cx));
        let t = timed(cx, window, &view, |cx| {
            cx.update(|cx| view.read(cx).doc.clone()).update(cx, |d, cx| d.undo(cx));
        }, |cx| view.read(cx).display_seq > s0 && !view.read(cx).ui_busy());
        ms[8].push(t);
        assert_eq!(count(cx), n - 1, "Undo took the sketch out");
        cx.update(|cx| view.update(cx, |v, _| v.selection.clear()));
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
