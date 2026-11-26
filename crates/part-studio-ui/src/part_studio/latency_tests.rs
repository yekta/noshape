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
    perf_document(cx, "live-30-features")
}

/// tests/perf/<name>.noshape in a test window drawing real frames, regenerated and shown.
fn perf_document(cx: &mut TestAppContext, name: &str) -> (AnyWindowHandle, Entity<PartStudioView>) {
    perf_document_at(cx, &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../tests/perf/{name}.noshape")))
}

/// A copy of document `src` in a test window drawing real frames, regenerated and shown.
fn perf_document_at(cx: &mut TestAppContext, src: &std::path::Path) -> (AnyWindowHandle, Entity<PartStudioView>) {
    RENDERING.set(true);
    let (window, view) = part_studio_with(cx, "latency", |path| {
        std::fs::copy(src, path).unwrap();
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

/// [`timed`] to the moment `done` first held (the state applied; its frame follows within one): the test runs
/// background tasks on its own thread, so a slow one started by the same change (std's editing logic, a selection's
/// measurement), which the app runs beside it, would otherwise count.
fn timed_applied(cx: &mut TestAppContext, window: AnyWindowHandle, view: &Entity<PartStudioView>, go: impl FnOnce(&mut TestAppContext), done: impl Fn(&gpui_kit::App) -> bool + 'static) -> f64 {
    let done = std::rc::Rc::new(done);
    let seen = std::rc::Rc::new(std::cell::Cell::new(None::<Instant>));
    let t0 = Instant::now();
    let _watch = cx.update(|cx| {
        let (done, seen) = (done.clone(), seen.clone());
        cx.observe(view, move |_, cx| {
            if seen.get().is_none() && done(cx) {
                seen.set(Some(Instant::now()));
            }
        })
    });
    let t = timed(cx, window, view, go, |cx| done(cx));
    seen.get().map_or(t, |at| (at - t0).as_secs_f64() * 1000.0)
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
    let mut ms: [Vec<f64>; 9] = Default::default();
    let points = two_points_on_the_part(cx, &view);
    let (hover, click, lit_at) = hover_and_click(cx, window, &view, &pointer, points);
    (ms[0], ms[1]) = (hover, click);

    // Extrude 1 (the plate, under all 29 others): open, depth steps as a drag gives them, ✓.
    let extrude = cx.update(|cx| view.read(cx).studio(cx).and_then(|s| s.features.iter().find(|f| f.type_id == "extrude").map(|f| f.id))).expect("an Extrude");
    let [open, step, commit, _] = edit_runs(cx, window, &view, extrude, 10.0, false);
    (ms[2], ms[3], ms[4]) = (open, step, commit);
    let seq = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).display_seq);

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

/// The everyday interactions on a mid-size corpus part (NOSHAPE_LATENCY_DOC, e.g. c2-impeller-loft's document: live's
/// "Impellergehaeuse", 48 features) against live's recorded times: slower only where ours is ≥ 100 ms and ≥ 2× live's
/// (lead 11-24). The edited feature is the first blind Extrude (the most features after it).
#[gpui::test]
#[ignore = "T2: wall-clock timing; a corpus document (NOSHAPE_LATENCY_DOC) in a release build"]
fn everyday_interactions_on_a_corpus_part(cx: &mut TestAppContext) {
    let Some(doc) = std::env::var_os("NOSHAPE_LATENCY_DOC") else {
        eprintln!("latency: NOSHAPE_LATENCY_DOC unset");
        return;
    };
    let Some(_gpu) = gpu_pointer() else { return };
    let (window, view) = perf_document_at(cx, std::path::Path::new(&doc));
    let pointer = Pointer::new(cx, window, &view);
    let points = two_lit_points(cx, window, &view, &pointer);
    let (hover, click, _) = hover_and_click(cx, window, &view, &pointer, points);
    let blind = |f: &noshape_model::Feature| f.type_id == "extrude" && !f.suppressed && matches!(f.params.get("endBound"), Some(noshape_model::ParamValue::Enum(b)) if b == "BLIND");
    let (extrude, name, after, depth) = cx
        .update(|cx| {
            let v = view.read(cx);
            let s = v.studio(cx)?;
            let k = s.features.iter().position(blind)?;
            let f = &s.features[k];
            // (A plain "<n> mm"; anything else steps from 10 mm.)
            let depth = f.params.text("depth").strip_suffix(" mm").and_then(|n| n.trim().parse::<f64>().ok());
            Some((f.id, f.name.clone(), s.features.len() - k - 1, depth.unwrap_or(10.0)))
        })
        .expect("a blind Extrude");
    let [open, step, commit, undo] = edit_runs(cx, window, &view, extrude, depth, true);
    // Live (behaviours): pre-highlight 10–50 ms; click → selected 57–67 ms (timings-1117); the dialog shown 322–448 ms
    // (timings-1117); a field's preview: Enter → the regeneration's reply 0.2–0.3 s (core edit commit, 30 features);
    // ✓ → regenerated 3.2 s and unsuppress (the same rebuild of the features after one) 1.4 s on this part (timing-1123).
    let rows = [("hover: a face lit", 50.0, hover), ("click: selected and drawn", 67.0, click), ("dialog: open on its preview", 448.0, open), ("field edit: its preview drawn", 300.0, step), ("Enter: the part rebuilt", 3239.0, commit), ("undo: rebuilt again", 1361.0, undo)];
    let failing = cx.update(|cx| {
        let v = view.read(cx);
        v.studio(cx).map_or(0, |s| s.features.iter().filter(|f| v.display.feature_state(f.id).is_some_and(|st| st.status != noshape_model::display::FeatureStatus::Ok)).count())
    });
    let mut report = vec![format!("{name} ({after} features after it, {failing} not OK), depth from {depth:.3} mm"), format!("{:<32} {:>9} {:>9}", "", "ours", "live")];
    let mut slower = Vec::new();
    for (what, live, runs) in rows {
        let m = median(runs.clone());
        report.push(format!("{what:<32} {m:>6.1} ms {live:>6.0} ms  (runs: {})", runs.iter().map(|x| format!("{x:.0}")).collect::<Vec<_>>().join(" ")));
        if m >= 100.0 && m >= 2.0 * live {
            slower.push(what);
        }
    }
    eprintln!("LATENCY\n{}", report.join("\n"));
    assert!(slower.is_empty(), "≥ 100 ms and ≥ 2× live: {slower:?}");
}

/// The hover-to-lit and click-to-selected times, alternating between two points on the part (after a while of slow
/// hovering, as anyone's session starts: the pick's running average settles), and the first point something lit at.
fn hover_and_click(cx: &mut TestAppContext, window: AnyWindowHandle, view: &Entity<PartStudioView>, pointer: &Pointer, (a, b): (DVec2, DVec2)) -> (Vec<f64>, Vec<f64>, Option<DVec2>) {
    for k in 0..40 {
        move_to(cx, window, pointer, a + DVec2::new(k as f64 * 1.5, 0.0));
        pump(cx, window, view, Duration::from_millis(16));
    }
    let (mut hover, mut click) = (Vec::new(), Vec::new());
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
        move_to(cx, window, pointer, from);
        pump(cx, window, view, Duration::from_millis(200));
        let before = cx.update(|cx| view.read(cx).hover.clone());
        for k in 1..8 {
            move_to(cx, window, pointer, from + (at - from) * (k as f64 / 8.0));
            pump(cx, window, view, Duration::from_millis(16));
        }
        let win = pointer.window_point(cx, at);
        let t = timed(cx, window, view, |cx| gpui_kit::VisualTestContext::from_window(window, cx).simulate_mouse_move(win, None, Modifiers::default()), |cx| {
            let v = view.read(cx);
            v.hover.is_some() && v.hover != before && v.ui.scene_task.is_none() && pointer.vp.read(cx).hover_settled()
        });
        hover.push(t);
        let lit = cx.update(|cx| view.read(cx).hover.clone());
        if lit_at.is_none() && lit.is_some() {
            lit_at = Some(at);
        }
        let v = view.clone();
        let t = timed_applied(cx, window, view, |cx| gpui_kit::VisualTestContext::from_window(window, cx).simulate_click(win, Modifiers::default()), move |cx| {
            let v = v.read(cx);
            v.selection.first() == lit.as_ref() && v.ui.scene_task.is_none()
        });
        click.push(t);
    }
    (hover, click, lit_at)
}

/// Feature `id`'s dialog opened on its preview, its depth stepped from `depth` mm as a drag gives it (each step's
/// preview), ✓ (the part rebuilt) and, with `undo`, the edit undone (rebuilt again): the times of each, ms.
fn edit_runs(cx: &mut TestAppContext, window: AnyWindowHandle, view: &Entity<PartStudioView>, extrude: noshape_model::FeatureId, depth: f64, undo: bool) -> [Vec<f64>; 4] {
    let mut ms: [Vec<f64>; 4] = Default::default();
    // (Each waits for a display applied after it: a preview or the regeneration's, `display_seq`.)
    let seq = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).display_seq);
    for run in 0..RUNS {
        cx.update(|cx| view.update(cx, |v, _| v.selection.clear()));
        let s0 = seq(cx);
        let t = timed(cx, window, view, |cx| {
            cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.edit_feature(extrude, w, cx))).unwrap();
        }, |cx| view.read(cx).edit.is_some() && view.read(cx).display_seq > s0 && !view.read(cx).ui_busy());
        ms[0].push(t);
        for step in 0..3 {
            let text = format!("{:.3} mm", depth + (run * 3 + step) as f64 * 0.137);
            let s0 = seq(cx);
            let v = view.clone();
            let t = timed_applied(cx, window, view, |cx| {
                cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| {
                    if let Some(e) = &v.edit {
                        e.dialog.update(cx, |d, cx| d.set_expr_text("depth", &text, w, cx));
                    }
                }))
                .unwrap();
            }, move |cx| {
                let v = v.read(cx);
                v.display_seq > s0 && v.pending.is_none() && v.ui.scene_task.is_none()
            });
            // (Then whatever else the change started, std's editing logic, before the next one.)
            let _ = timed(cx, window, view, |_| {}, |cx| !view.read(cx).ui_busy());
            ms[1].push(t);
        }
        let s0 = seq(cx);
        let t = timed(cx, window, view, |cx| {
            cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.accept_open_dialog(w, cx))).unwrap();
        }, |cx| view.read(cx).edit.is_none() && view.read(cx).display_seq > s0 && !view.read(cx).ui_busy());
        ms[2].push(t);
        if undo {
            let s0 = seq(cx);
            let t = timed(cx, window, view, |cx| {
                cx.update(|cx| view.read(cx).doc.clone()).update(cx, |d, cx| d.undo(cx));
            }, |cx| view.read(cx).display_seq > s0 && !view.read(cx).ui_busy());
            ms[3].push(t);
        }
    }
    ms
}

/// Two points of the view lighting different things: tried on a grid round the view's centre, each after a rest.
fn two_lit_points(cx: &mut TestAppContext, window: AnyWindowHandle, view: &Entity<PartStudioView>, pointer: &Pointer) -> (DVec2, DVec2) {
    let size = cx.update(|cx| pointer.vp.read(cx).size());
    let mut first: Option<(DVec2, _)> = None;
    for (i, j) in [(0, 0), (1, 0), (0, 1), (-1, 0), (0, -1), (2, 1), (-2, -1), (1, 2), (-1, -2), (3, 0), (0, 3)] {
        let at = size / 2.0 + DVec2::new(i as f64, j as f64) * (size.min_element() / 10.0);
        move_to(cx, window, pointer, at);
        pump(cx, window, view, Duration::from_millis(400));
        let Some(lit) = cx.update(|cx| view.read(cx).hover.clone()) else { continue };
        match &first {
            None => first = Some((at, lit)),
            Some((a, h)) if *h != lit => return (*a, at),
            Some(_) => {}
        }
    }
    panic!("no two points lighting different things");
}

/// Live's pre-highlight on heavy Part Studios (live 11-21 "qa 20:18 (1)", software GL Chrome): the 806-face plate's top
/// face 42-53 ms, a side face 299-361 ms; the 1000-part Part Studio's face ~266 ms. Ours: a face lit after live's
/// approach (8 moves 16 ms apart), the same as on the 30-feature part, at points across the part.
#[gpui::test]
#[ignore = "T2: wall-clock timing; a release build on a quiet machine (the Mac)"]
fn a_face_lights_on_heavy_part_studios(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    let mut report = Vec::new();
    for (name, live_ms) in [("big-plate", 53.0), ("parts-1000", 266.0)] {
        let (window, view) = perf_document(cx, name);
        let pointer = Pointer::new(cx, window, &view);
        let (a, b) = two_lit_points(cx, window, &view, &pointer);
        for k in 0..40 {
            move_to(cx, window, &pointer, a + DVec2::new(k as f64 * 1.5, 0.0));
            pump(cx, window, &view, Duration::from_millis(16));
        }
        let mut ms = Vec::new();
        for i in 0..RUNS * 2 {
            let (from, at) = if i % 2 == 0 { (b, a) } else { (a, b) };
            pump(cx, window, &view, Duration::from_millis(200));
            let before = cx.update(|cx| view.read(cx).hover.clone());
            for k in 1..8 {
                move_to(cx, window, &pointer, from + (at - from) * (k as f64 / 8.0));
                pump(cx, window, &view, Duration::from_millis(16));
            }
            let win = pointer.window_point(cx, at);
            ms.push(timed(cx, window, &view, |cx| gpui_kit::VisualTestContext::from_window(window, cx).simulate_mouse_move(win, None, Modifiers::default()), |cx| {
                let v = view.read(cx);
                v.hover.is_some() && v.hover != before && v.ui.scene_task.is_none() && pointer.vp.read(cx).hover_settled()
            }));
        }
        let m = median(ms.clone());
        report.push((name, m, live_ms, format!("{name}: a face lit {m:.1} ms median (live {live_ms} ms), average pick {:.1} ms (runs: {})", cx.update(|cx| pointer.vp.read(cx).average_pick_ms()), ms.iter().map(|x| format!("{x:.1}")).collect::<Vec<_>>().join(" "))));
        cx.update_window(window, |_, w, _| w.remove_window()).unwrap();
    }
    eprintln!("{}", report.iter().map(|r| r.3.as_str()).collect::<Vec<_>>().join("\n"));
    let slower: Vec<_> = report.iter().filter(|r| r.1 > r.2 / 2.0).map(|r| r.0).collect();
    assert!(slower.is_empty(), "slower than half of live's time: {slower:?}");
}

/// Lead 11-24, sketch speed as a person feels it, on live's S300 (behaviours "Sketch S300 timings", 11-01): 80
/// rectangles with their width and height dimensions (320 lines, ~800 constraints). Live (software GL): open for edit
/// 1.2-1.9 s, a dimension edit to the redrawn sketch ~1.1 s, a drag at its renderer's ~4 fps (median 270 ms a frame),
/// ✓ 0.24 s; qa sellat-1118 (one line): a dimension edit 584-743 ms. Ours: each to the frame that shows it, a drag
/// step to the frame with the corner moved, and a value the sketch can't take (no stall, "Sketch could not be
/// solved."). T2.
#[gpui::test]
#[ignore = "T2: wall-clock timing; a release build on a quiet machine (the Mac)"]
fn a_300_line_sketch_keeps_up_with_the_hand(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    RENDERING.set(true);
    let (window, view) = part_studio_with(cx, "s300", |path| crate::session::Session::create(path, crate::user_units::new_document()).unwrap());
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| {
        v.selection = vec![Reference::Plane(DatumPlane::Top)];
        v.start_feature("sketch", w, cx);
    }))
    .unwrap();
    wait_until(cx, window, |cx| view.read(cx).sketch.is_some() && !view.read(cx).ui_busy());
    // 20 x 4 rectangles 10 x 6 mm on a 15 mm grid, each with its width and height, and one more line whose length
    // the sketch can't take other than 30 mm (its horizontal distance fixes it).
    let (corner, width, len) = cx
        .update(|cx| view.update(cx, |v, cx| v.with_sketch(cx, |_, data| {
            use noshape_sketch::ConstraintKind;
            let (mut corner, mut width) = (None, None);
            for i in 0..80 {
                let p0 = DVec2::new(-0.15 + 0.015 * (i % 20) as f64, -0.03 + 0.015 * (i / 20) as f64);
                let s = data.sketch.corner_rectangle(p0, p0 + DVec2::new(0.01, 0.006));
                corner.get_or_insert(s.points[0]);
                let w = crate::sketch_mode::add_dimension(data, ConstraintKind::Length { curve: s.curves[0], value: 0.0 }, Some(p0 + DVec2::new(0.005, -0.003)), None);
                width = width.or(w);
                let _ = crate::sketch_mode::add_dimension(data, ConstraintKind::Length { curve: s.curves[3], value: 0.0 }, Some(p0 + DVec2::new(-0.003, 0.003)), None);
            }
            let a = data.sketch.add_point(DVec2::new(-0.015, 0.045));
            let b = data.sketch.add_point(DVec2::new(0.015, 0.045));
            let line = data.sketch.add_line_between(a, b).unwrap();
            data.sketch.add_constraint(ConstraintKind::Horizontal { a: line, b: None }).unwrap();
            data.sketch.add_constraint(ConstraintKind::HorizontalDistance { a, b, value: 0.03 }).unwrap();
            let len = data.sketch.add_reference(ConstraintKind::Length { curve: line, value: 0.03 }).unwrap();
            let _ = data.sketch.set_driving(len, true);
            (corner.unwrap(), width.unwrap(), len)
        })))
        .unwrap();
    let sizes = cx.update(|cx| view.read(cx).sketch_data(cx).map(|d| (d.sketch.entities().filter(|e| e.kind() == noshape_sketch::EntityKind::Line).count(), d.sketch.constraint_count())).unwrap());
    let seq = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).display_seq);
    let s0 = seq(cx);
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| { let _ = v.finish_sketch(w, cx); })).unwrap();
    wait_until(cx, window, |cx| view.read(cx).sketch.is_none() && view.read(cx).display_seq > s0 && !view.read(cx).ui_busy());
    let sketch = cx.update(|cx| view.read(cx).studio(cx).unwrap().features.iter().find(|f| f.name == "Sketch 1").unwrap().id);

    let mut ms: [Vec<f64>; 7] = Default::default();
    let pointer = Pointer::new(cx, window, &view);
    for run in 0..3 {
        ms[0].push(timed(cx, window, &view, |cx| {
            cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.edit_feature(sketch, w, cx))).unwrap();
        }, |cx| view.read(cx).sketch.is_some() && !view.read(cx).ui_busy()));
        if run == 0 {
            // Zoomed to the first rectangle, as anyone dragging its corner would (the whole sketch is ~2.6 px/mm).
            let vp = cx.update(|cx| view.read(cx).viewport.clone());
            vp.update(cx, |vp, cx| vp.zoom_to_bounds(glam::DVec3::new(-0.165, -0.045, 0.0), glam::DVec3::new(-0.125, -0.01, 0.0), cx));
            pump(cx, window, &view, Duration::from_millis(1500));
        }
        // A drag of the first rectangle's lower left corner, 2 px a step.
        let at = |cx: &mut TestAppContext| cx.update(|cx| {
            let p = view.read(cx).sketch_data(cx).unwrap().sketch.point(corner).unwrap();
            (p, view.read(cx).viewport.read(cx).project(glam::DVec3::new(p.x, p.y, 0.0)).unwrap())
        });
        let (p0, s0px) = at(cx);
        move_to(cx, window, &pointer, s0px);
        pump(cx, window, &view, Duration::from_millis(300));
        let hovered = cx.update(|cx| view.read(cx).sketch.as_ref().and_then(|e| e.hover));
        assert_eq!(hovered, Some(corner), "the corner lit under the pointer at {s0px:?}");
        let left = gpui_kit::MouseButton::Left;
        gpui_kit::VisualTestContext::from_window(window, cx).simulate_mouse_down(pointer.window_point(cx, s0px), left, Modifiers::default());
        for k in 1..=20 {
            let to = pointer.window_point(cx, s0px + DVec2::new(2.0 * k as f64, -(k as f64)));
            let mut handled = 0.0;
            ms[1].push(timed(cx, window, &view, |cx| {
                let t0 = Instant::now();
                gpui_kit::VisualTestContext::from_window(window, cx).simulate_mouse_move(to, Some(left), Modifiers::default());
                handled = t0.elapsed().as_secs_f64() * 1e3;
            }, |_| true));
            ms[2].push(handled);
        }
        let up = pointer.window_point(cx, s0px + DVec2::new(40.0, -20.0));
        ms[3].push(timed(cx, window, &view, |cx| gpui_kit::VisualTestContext::from_window(window, cx).simulate_mouse_up(up, left, Modifiers::default()), |cx| !view.read(cx).ui_busy()));
        let (p1, _) = at(cx);
        assert!((p1 - p0).length() > 1e-4, "the corner followed the drag: {p0:?} → {p1:?}");
        // A width typed, to the redrawn sketch.
        let text = if run % 2 == 0 { "12 mm" } else { "10 mm" };
        ms[4].push(timed(cx, window, &view, |cx| {
            cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.commit_dimension(width, text, w, cx))).unwrap();
        }, |cx| !view.read(cx).ui_busy()));
        // A length the sketch can't take: no stall, live's warning, the value back after.
        ms[5].push(timed(cx, window, &view, |cx| {
            cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.commit_dimension(len, "10 mm", w, cx))).unwrap();
        }, |cx| !view.read(cx).ui_busy()));
        let warned = cx.update(|cx| view.read(cx).message.as_ref().map(|(_, m)| m.clone()));
        assert_eq!(warned.as_deref(), Some(crate::sketch_mode::SKETCH_NOT_SOLVED), "the warning");
        cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.commit_dimension(len, "30 mm", w, cx))).unwrap();
        pump(cx, window, &view, Duration::from_millis(100));
        let s0 = seq(cx);
        ms[6].push(timed(cx, window, &view, |cx| {
            cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| { let _ = v.finish_sketch(w, cx); })).unwrap();
        }, |cx| view.read(cx).sketch.is_none() && view.read(cx).display_seq > s0 && !view.read(cx).ui_busy()));
    }
    let rows = [
        ("open for edit", "1.2-1.9 s"),
        // (Its worst is a hover pick's GPU readback: slow on a software renderer, off the UI thread.)
        ("drag: a step to its frame", "~270 ms a frame (renderer)"),
        ("drag: the move handled (UI thread)", "-"),
        ("drag: release, regenerated", "-"),
        ("dimension edit, redrawn", "~1.1 s (one line 584-743 ms)"),
        ("a value it can't take, warned", "-"),
        ("✓, the part studio redrawn", "0.24 s"),
    ];
    let mut report = vec![format!("S300: {} lines, {} constraints", sizes.0, sizes.1), format!("{:<34} {:>9} {:>9}  live", "", "median", "worst")];
    for ((what, live), runs) in rows.iter().zip(&ms) {
        let worst = runs.iter().cloned().fold(0.0, f64::max);
        report.push(format!("{what:<34} {:>6.1} ms {:>6.1} ms  {live}", median(runs.clone()), worst));
    }
    eprintln!("{}", report.join("\n"));
    let slow: Vec<&str> = rows.iter().zip(&ms).filter(|(_, runs)| median((*runs).clone()) >= 100.0).map(|((w, _), _)| *w).collect();
    assert!(slow.is_empty(), "100 ms or more: {slow:?}");
    let stall = ms[2].iter().cloned().fold(0.0, f64::max);
    assert!(stall < 100.0, "a drag step held the UI thread {stall:.0} ms");
}

/// A slow sketch's dimension edit (fuzz fz211: ~0.3 s a solve, ~0.4 s its regeneration) leaves the UI thread free: the
/// sketch is held while its solve runs off it, pending (Onshape's working overlay past 0.9 s), then shows the new size;
/// a value it can't take warns as on a quick sketch, its last solved shape kept.
#[gpui::test]
fn a_slow_sketch_solves_a_dimension_off_the_ui_thread(cx: &mut TestAppContext) {
    let (window, view) = part_studio_with(cx, "slow-solve", |path| crate::session::Session::create(path, crate::user_units::new_document()).unwrap());
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| {
        v.selection = vec![Reference::Plane(DatumPlane::Top)];
        v.start_feature("sketch", w, cx);
    }))
    .unwrap();
    wait_until(cx, window, |cx| view.read(cx).sketch.is_some() && !view.read(cx).ui_busy());
    let (bottom, width, line, len) = cx
        .update(|cx| view.update(cx, |v, cx| v.with_sketch(cx, |_, data| {
            use noshape_sketch::ConstraintKind;
            let s = data.sketch.corner_rectangle(DVec2::ZERO, DVec2::new(0.01, 0.006));
            let width = crate::sketch_mode::add_dimension(data, ConstraintKind::Length { curve: s.curves[0], value: 0.0 }, Some(DVec2::new(0.005, -0.003)), None).unwrap();
            // A 30-40-50 triangle, each side dimensioned (live errors-1118: its 50 typed longer than the other two).
            let (a, b, c) = (data.sketch.add_point(DVec2::new(0.0, 0.02)), data.sketch.add_point(DVec2::new(0.04, 0.02)), data.sketch.add_point(DVec2::new(0.0, 0.05)));
            let sides = [data.sketch.add_line_between(a, b).unwrap(), data.sketch.add_line_between(b, c).unwrap(), data.sketch.add_line_between(c, a).unwrap()];
            let lens: Vec<_> = sides.iter().map(|l| crate::sketch_mode::add_dimension(data, ConstraintKind::Length { curve: *l, value: 0.0 }, None, None).unwrap()).collect();
            (s.curves[0], width, sides[1], lens[1])
        })))
        .unwrap();
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| {
        v.regen_sync(w, cx);
        v.ui.slow_sketch = true;
    }))
    .unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let length = |cx: &mut TestAppContext, curve| cx.update(|cx| view.read(cx).sketch_data(cx).and_then(|d| d.sketch.curve(curve)).map(|c| c.length() * 1e3).unwrap());

    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.commit_dimension(width, "12 mm", w, cx))).unwrap();
    let held = cx.update(|cx| view.update(cx, |v, cx| (v.ui.sketch_solve.is_some(), v.pending.is_some(), v.with_sketch(cx, |_, _| ()).is_none())));
    assert_eq!(held, (true, true, true), "solving off the UI thread: pending, the sketch held");
    assert!((length(cx, bottom) - 10.0).abs() < 1e-6, "the old shape shown meanwhile");
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    assert!((length(cx, bottom) - 12.0).abs() < 1e-6, "the new width: {}", length(cx, bottom));
    let after = cx.update(|cx| (view.read(cx).ui.sketch_solve.is_none(), view.read(cx).message.clone()));
    assert_eq!(after, (true, None), "done, no message");

    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.commit_dimension(len, "100 mm", w, cx))).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let warned = cx.update(|cx| view.read(cx).message.clone().map(|m| m.1));
    assert_eq!(warned.as_deref(), Some(crate::sketch_mode::SKETCH_NOT_SOLVED), "the warning");
    assert!((length(cx, line) - 50.0).abs() < 1e-6, "the last solved shape kept: {}", length(cx, line));
    assert!((length(cx, bottom) - 12.0).abs() < 1e-6, "the rest as it was");
}

/// A slow sketch's drag (fuzz fz211: steps up to ~0.2 s) keeps the UI thread free: each step solves off it, the last
/// solved shape shown meanwhile; the pointer's moves during a solve make one next step, its release waits for it, and
/// the point ends under the pointer as on a quick sketch.
#[gpui::test]
fn a_slow_sketch_drags_off_the_ui_thread(cx: &mut TestAppContext) {
    let (window, view) = part_studio_with(cx, "slow-drag", |path| crate::session::Session::create(path, crate::user_units::new_document()).unwrap());
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| {
        v.selection = vec![Reference::Plane(DatumPlane::Top)];
        v.start_feature("sketch", w, cx);
    }))
    .unwrap();
    wait_until(cx, window, |cx| view.read(cx).sketch.is_some() && !view.read(cx).ui_busy());
    let end = cx
        .update(|cx| view.update(cx, |v, cx| v.with_sketch(cx, |_, data| {
            let (a, b) = (data.sketch.add_point(DVec2::new(0.02, 0.01)), data.sketch.add_point(DVec2::new(0.04, 0.01)));
            data.sketch.add_line_between(a, b).unwrap();
            b
        })))
        .unwrap();
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| {
        v.regen_sync(w, cx);
        v.viewport.update(cx, |vp, cx| vp.zoom_to_fit(true, cx));
        v.ui.slow_sketch = true;
    }))
    .unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    draw(cx, window);
    let event = |cx: &mut TestAppContext, ev: ViewportEvent| {
        cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| {
            let vp = v.viewport.clone();
            v.on_viewport_event(&vp, &ev, w, cx);
        }))
        .unwrap();
    };
    event(cx, ViewportEvent::Hover(None));
    let (o, dx, dy) = cx.update(|cx| view.read(cx).sketch.as_ref().unwrap().screen_map.unwrap());
    let m = glam::DMat2::from_cols(dx, dy);
    let px = |q: DVec2| (m.inverse() * (q - o)).round();
    let plane = |p: DVec2| o + m * (p + DVec2::splat(0.5));
    let point = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).sketch_data(cx).unwrap().sketch.point(end).unwrap());
    let start = point(cx);
    let from = px(start);
    // (Upward: down it would end level with the origin, which snaps.)
    let to = from + DVec2::new(30.0, -20.0);
    let none = Modifiers::default();
    event(cx, ViewportEvent::PointerMove { position: from, modifiers: none });
    event(cx, ViewportEvent::ToolDown { position: from, modifiers: none, click_count: 1 });
    event(cx, ViewportEvent::ToolDrag { position: from + DVec2::new(10.0, -5.0) });
    let held = cx.update(|cx| view.read(cx).ui.sketch_drag.is_some());
    assert!(held, "the step solves off the UI thread");
    assert_eq!(point(cx), start, "the last solved shape shown meanwhile");
    event(cx, ViewportEvent::ToolDrag { position: from + DVec2::new(20.0, -10.0) });
    event(cx, ViewportEvent::ToolDrag { position: to });
    event(cx, ViewportEvent::ToolUp { position: to, modifiers: none });
    let dragging = cx.update(|cx| view.read(cx).sketch.as_ref().unwrap().geometry_dragging());
    assert!(dragging, "the release waits for the solve");
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    let done = cx.update(|cx| (view.read(cx).ui.sketch_drag.is_none(), view.read(cx).sketch.as_ref().unwrap().geometry_dragging()));
    assert_eq!(done, (true, false), "solved, the drag ended");
    let at = point(cx);
    // It keeps its offset from the press (live sketch-drag-1125: the grab offset).
    let want = plane(to) + (start - plane(from));
    assert!((at - want).length() < 1e-9, "the point under the pointer, grab offset kept: {at} vs {want}");
}

/// A sketch on Top laid out as live's speed-1125 grid (qa/speed-1125 raw/ts1.b.js): `rects` rectangles 10 x 6 mm at a
/// 15 mm pitch in `cols` columns, each with its 10 mm width; S50 adds a circle and a line. Open for edit, zoomed to
/// live's 0.188 mm/px about the first rectangle; its lower left corner and width dimension.
fn speed_grid(cx: &mut TestAppContext, rects: usize, cols: usize, extras: bool) -> (AnyWindowHandle, Entity<PartStudioView>, noshape_sketch::EntityId, noshape_sketch::ConstraintId) {
    let (window, view) = sketch_on_top(cx);
    let (corner_before, width_before) = cx
        .update(|cx| view.update(cx, |v, cx| v.with_sketch(cx, |_, data| {
            use noshape_sketch::ConstraintKind;
            let (mut corner, mut width) = (None, None);
            for i in 0..rects {
                let p0 = DVec2::new(0.015 * (i % cols) as f64, 0.015 * (i / cols) as f64);
                let s = data.sketch.corner_rectangle(p0, p0 + DVec2::new(0.01, 0.006));
                // Live's q0 corner (10, 6) (its (0, 0) corner is on the Origin).
                let far = s.points.iter().copied().find(|q| data.sketch.point(*q).is_some_and(|q| (q - p0 - DVec2::new(0.01, 0.006)).length() < 1e-9));
                corner = corner.or(far);
                let w = crate::sketch_mode::add_dimension(data, ConstraintKind::Length { curve: s.curves[0], value: 0.0 }, Some(p0 + DVec2::new(0.005, -0.003)), None);
                width = width.or(w);
            }
            if extras {
                data.sketch.add_circle(DVec2::new(-0.015, 0.003), 0.003);
                let a = data.sketch.add_point(DVec2::new(-0.02, 0.01));
                let b = data.sketch.add_point(DVec2::new(-0.01, 0.015));
                data.sketch.add_line_between(a, b).unwrap();
            }
            (corner.unwrap(), width.unwrap())
        })))
        .unwrap();
    // Closed and opened again, as a user's saved sketch.
    let s0 = cx.update(|cx| view.read(cx).display_seq);
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| { let _ = v.finish_sketch(w, cx); })).unwrap();
    wait_until(cx, window, |cx| view.read(cx).sketch.is_none() && view.read(cx).display_seq > s0 && !view.read(cx).ui_busy());
    let sketch = cx.update(|cx| view.read(cx).studio(cx).unwrap().features.iter().find(|f| f.name == "Sketch 1").unwrap().id);
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.edit_feature(sketch, w, cx))).unwrap();
    wait_until(cx, window, |cx| view.read(cx).sketch.is_some() && !view.read(cx).ui_busy());
    // (Its ids as read back.)
    let (corner, width) = cx.update(|cx| {
        let d = view.read(cx).sketch_data(cx).unwrap();
        let at = DVec2::new(0.01, 0.006);
        let corner = d.sketch.entities().filter(|e| e.kind() == noshape_sketch::EntityKind::Point).map(|e| e.id).find(|q| d.sketch.point(*q).is_some_and(|q| (q - at).length() < 1e-9));
        let width = d.sketch.constraints().find(|c| matches!(c.kind, noshape_sketch::ConstraintKind::Length { .. })).map(|c| c.id);
        let points: Vec<_> = d.sketch.entities().filter(|e| e.kind() == noshape_sketch::EntityKind::Point).take(8).map(|e| (e.id, d.sketch.point(e.id))).collect();
        assert!(corner.is_some(), "no point at (10, 6) mm after reopening (was {corner_before:?}, {} entities): {points:?}", d.sketch.entities().count());
        (corner.unwrap(), width.unwrap_or(width_before))
    });
    let vp = cx.update(|cx| view.read(cx).viewport.clone());
    // 0.188 mm/px over a ~1000 px wide viewport: ~190 mm across, about the first rectangle.
    vp.update(cx, |vp, cx| vp.zoom_to_bounds(glam::DVec3::new(-0.08, -0.05, 0.0), glam::DVec3::new(0.11, 0.06, 0.0), cx));
    (window, view, corner, width)
}

/// Live's user-action rows (qa/speed-1125, software GL, medians of 5): a rectangle corner dragged in a 50- and a
/// 200-entity sketch (release to settled 654 / 1049 ms, each move drawn in <= 54 ms), its width typed + Enter
/// (667 / 758 ms), and Insert DXF's ✓ of a 72-entity gear (802 ms). Ours on the same kind of host, printed with live's.
#[gpui::test]
#[ignore = "T2: wall-clock timing; a release build, compared with live's software-GL rows"]
fn live_speed_1125_sketch_rows(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    RENDERING.set(true);
    let mut report = vec![format!("{:<40} {:>9} {:>9}  live", "", "median", "worst")];
    let mut notes = vec![];
    let mut row = |what: &str, runs: &[f64], live: &str| {
        if runs.is_empty() {
            return;
        }
        let worst = runs.iter().cloned().fold(0.0, f64::max);
        report.push(format!("{what:<40} {:>6.1} ms {:>6.1} ms  {live}", median(runs.to_vec()), worst));
    };
    for (name, rects, cols, extras, live) in [("S50", 12, 4, true, ["654", "<= 54", "667"]), ("S200", 50, 10, false, ["1049", "continuous", "758"])] {
        let (window, view, mut corner, width) = speed_grid(cx, rects, cols, extras);
        pump(cx, window, &view, Duration::from_millis(1500));
        let pointer = Pointer::new(cx, window, &view);
        let mut ms: [Vec<f64>; 3] = Default::default();
        let mut jumps = vec![];
        let mut home = cx.update(|cx| view.read(cx).sketch_data(cx).unwrap().sketch.point(corner)).unwrap_or(DVec2::new(0.01, 0.006));
        for run in 0..5 {
            // The point nearest where the corner was (its id as the sketch now reads back).
            let nearest = |cx: &mut TestAppContext, home: DVec2, within: f64| cx.update(|cx| {
                let d = view.read(cx).sketch_data(cx).unwrap();
                let near = d.sketch.entities().filter(|e| e.kind() == noshape_sketch::EntityKind::Point).filter_map(|e| Some((e.id, (d.sketch.point(e.id)? - home).length())));
                let (id, off) = near.min_by(|a, b| a.1.total_cmp(&b.1)).unwrap();
                assert!(off < within, "{name} run {run}: no point within {within} m of {home:?} ({off})");
                (id, off)
            });
            // (Moved up to 2 mm since by the width typed.)
            corner = nearest(cx, home, if run == 0 { 1e-4 } else { 5e-3 }).0;
            let at = |cx: &mut TestAppContext| cx.update(|cx| {
                let p = view.read(cx).sketch_data(cx).unwrap().sketch.point(corner).unwrap();
                (p, view.read(cx).viewport.read(cx).project(glam::DVec3::new(p.x, p.y, 0.0)).unwrap())
            });
            let (p0, s0px) = at(cx);
            move_to(cx, window, &pointer, s0px);
            pump(cx, window, &view, Duration::from_millis(900));
            let hovered = cx.update(|cx| view.read(cx).sketch.as_ref().and_then(|e| e.hover));
            assert_eq!(hovered, Some(corner), "{name}: the corner lit under the pointer");
            let left = gpui_kit::MouseButton::Left;
            gpui_kit::VisualTestContext::from_window(window, cx).simulate_mouse_down(pointer.window_point(cx, s0px), left, Modifiers::default());
            // +60/-40 px and back on the next run, 20 moves 16 ms apart.
            let dir = if run % 2 == 0 { DVec2::new(3.0, 2.0) } else { DVec2::new(-3.0, -2.0) };
            for k in 1..=20 {
                let to = pointer.window_point(cx, s0px + dir * k as f64);
                let t0 = Instant::now();
                ms[1].push(timed(cx, window, &view, |cx| {
                    gpui_kit::VisualTestContext::from_window(window, cx).simulate_mouse_move(to, Some(left), Modifiers::default());
                }, |_| true));
                if let Some(rest) = Duration::from_millis(16).checked_sub(t0.elapsed()) {
                    pump(cx, window, &view, rest);
                }
            }
            let up = pointer.window_point(cx, s0px + dir * 20.0);
            let (dragged, _) = at(cx);
            ms[0].push(timed(cx, window, &view, |cx| gpui_kit::VisualTestContext::from_window(window, cx).simulate_mouse_up(up, left, Modifiers::default()), |cx| !view.read(cx).ui_busy()));
            // (The released sketch reads back with new ids.)
            let (released, jump) = nearest(cx, dragged, 5e-3);
            jumps.push(jump * 1000.0);
            let p1 = cx.update(|cx| view.read(cx).sketch_data(cx).unwrap().sketch.point(released)).unwrap();
            if (p1 - p0).length() < 1e-3 {
                notes.push(format!("{name} run {run}: the corner did not follow the drag (at {p0:?})"));
            }
            home = p1;
            let text = if run % 2 == 0 { "12 mm" } else { "10 mm" };
            if std::env::var_os("NOSHAPE_SPEED_NO_DIM").is_some() {
                continue;
            }
            ms[2].push(timed(cx, window, &view, |cx| {
                cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.commit_dimension(width, text, w, cx))).unwrap();
            }, |cx| !view.read(cx).ui_busy()));
            pump(cx, window, &view, Duration::from_millis(300));
        }
        row(&format!("{name} drag: release, settled"), &ms[0], live[0]);
        row(&format!("{name} drag: a move drawn"), &ms[1], live[1]);
        row(&format!("{name} width typed + Enter, settled"), &ms[2], live[2]);
        notes.push(format!("{name}: the corner from its last drawn drag place to its released one: {jumps:.3?} mm"));
    }
    let dxf = std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/perf/gear-logo.dxf")).unwrap();
    let mut ms = vec![];
    for _ in 0..5 {
        let (window, view) = sketch_on_top(cx);
        ms.push(timed(cx, window, &view, |cx| {
            cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.insert_dxf_bytes(&dxf, w, cx))).unwrap();
        }, |cx| !view.read(cx).ui_busy()));
        let n = cx.update(|cx| view.read(cx).sketch_data(cx).unwrap().sketch.entities().filter(|e| e.kind() != noshape_sketch::EntityKind::Point).count());
        assert!(n >= 72, "the gear's 72 entities: {n}");
    }
    row("Insert DXF (gear, 72 entities) ✓, settled", &ms, "802");
    report.extend(notes);
    eprintln!("{}", report.join("\n"));
}

/// An empty sketch on Top of a new document, open for edit.
fn sketch_on_top(cx: &mut TestAppContext) -> (AnyWindowHandle, Entity<PartStudioView>) {
    let (window, view) = part_studio_with(cx, "speed-grid", |path| crate::session::Session::create(path, crate::user_units::new_document()).unwrap());
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| {
        v.selection = vec![Reference::Plane(DatumPlane::Top)];
        v.start_feature("sketch", w, cx);
    }))
    .unwrap();
    wait_until(cx, window, |cx| view.read(cx).sketch.is_some() && !view.read(cx).ui_busy());
    (window, view)
}
