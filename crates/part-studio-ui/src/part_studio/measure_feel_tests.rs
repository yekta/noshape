//! The measure matrix (docs/parity/interaction-feel.md "Measure"): live2's measure.json rows on live's own document
//! (Part 1 40 × 30 × 10 with an r4 hole at (20, 15), a 2 mm fillet on its x = 0 top edge and a 1 mm chamfer on its
//! x = 40 one; Part 2 x 60–80, y 0–20, z 0–15; Part 3 x 100–110, y 0–10, z 0–5): a selection, the summary bar's
//! readout (its names, values, units), at a device pixel ratio of 1 and 2. A row ours doesn't match yet is a gap; the
//! test fails when a row's status moves either way.

use super::*;
use gpui_kit::{MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, point};

fn document() -> serde_json::Value {
    let rect = |n: u32, a: [f64; 2], b: [f64; 2]| serde_json::json!({ "type": "newSketch", "name": format!("Sketch {n}"), "plane": "Top", "shapes": [{ "corner_rectangle": [a, b] }] });
    let extrude = |n: u32, depth: &str, op: &str| serde_json::json!({ "type": "extrude", "name": format!("Extrude {n}"), "params": { "entities": [format!("Sketch {n} region 0")], "endBound": "BLIND", "depth": depth, "operationType": op } });
    serde_json::json!({ "units": "mm", "features": [
        rect(1, [0.0, 0.0], [40.0, 30.0]), extrude(1, "10 mm", "NEW"),
        { "type": "newSketch", "name": "Sketch 2", "plane": "Top", "shapes": [{ "circle": [[20, 15], [24, 15]] }] },
        { "type": "extrude", "name": "Extrude 2", "params": { "entities": ["Sketch 2 region 0"], "endBound": "BLIND", "depth": "40 mm", "symmetric": true, "operationType": "REMOVE" } },
        { "type": "fillet", "name": "Fillet 1", "params": { "entities": ["edge through [0, 25, 10]"], "radius": "2 mm" } },
        { "type": "chamfer", "name": "Chamfer 1", "params": { "entities": ["edge through [40, 25, 10]"], "width": "1 mm" } },
        rect(3, [60.0, 0.0], [80.0, 20.0]), extrude(3, "15 mm", "NEW"),
        rect(4, [100.0, 0.0], [110.0, 10.0]), extrude(4, "5 mm", "NEW")
    ] })
}

/// An entity of the document by kind ("faces", "edges", "vertices") and a point on it (mm), or "plane:top".
#[derive(Clone, Copy, Debug)]
enum E {
    At(&'static str, [f64; 3]),
    Id(&'static str),
    Part(&'static str),
    /// The midpoint of the edge through this point.
    Mid([f64; 3]),
}

const V40: E = E::At("vertices", [40., 0., 0.]);
const V60: E = E::At("vertices", [60., 0., 0.]);
const FRONT_EDGE: E = E::At("edges", [20., 0., 0.]);
const LEFT_EDGE: E = E::At("edges", [0., 15., 0.]);
const P2_FRONT_EDGE: E = E::At("edges", [70., 0., 0.]);
const HOLE_TOP: E = E::At("edges", [24., 15., 10.]);
const HOLE_BOTTOM: E = E::At("edges", [24., 15., 0.]);
const FRONT: E = E::At("faces", [20., 0., 5.]);
const TOP: E = E::At("faces", [30., 25., 10.]);
const BOTTOM: E = E::At("faces", [5., 5., 0.]);
const P2_TOP: E = E::At("faces", [70., 10., 15.]);
/// Part 2's top front left corner: from V40, live's 25 mm (X 20, Y 0, Z 15).
const V60_TOP: E = E::At("vertices", [60., 0., 15.]);

struct Bench {
    window: AnyWindowHandle,
    view: Entity<PartStudioView>,
    path: std::path::PathBuf,
}

impl Bench {
    fn open(cx: &mut TestAppContext, name: &str, scale: f32) -> Self {
        Self::open_doc(cx, name, scale, document(), 3)
    }

    fn open_doc(cx: &mut TestAppContext, name: &str, scale: f32, doc: serde_json::Value, parts: usize) -> Self {
        let (window, view) = part_studio_from(cx, name, Some(doc));
        cx.update_window(window, |_, w, _| {
            w.set_scale_factor(scale);
            w.activate_window();
        })
        .unwrap();
        wait_until(cx, window, |cx| !view.read(cx).ui_busy() && view.read(cx).display.bodies.len() == parts);
        draw(cx, window);
        let failed = cx.update(|cx| {
            let v = view.read(cx);
            v.studio(cx).unwrap().features.iter().filter_map(|f| v.display.feature_state(f.id).filter(|s| s.status != noshape_model::display::FeatureStatus::Ok).map(|s| format!("{}: {:?}", f.name, s.message))).collect::<Vec<_>>()
        });
        assert!(failed.is_empty(), "live's measure document: {failed:?} didn't regenerate");
        let path = cx.update(|cx| view.read(cx).doc.read(cx).session.path.clone());
        Bench { window, view, path }
    }

    fn reference(&self, cx: &mut TestAppContext, e: E) -> Result<Reference, String> {
        let (kind, at) = match e {
            E::Id(id) => return noshape_ops::refs::parse_str(id).ok_or(format!("no reference {id}")),
            E::Part(name) => return cx.update(|cx| self.view.read(cx).display.bodies.iter().find(|b| b.name == name).map(|b| Reference::Body(b.id.clone()))).ok_or(format!("no part {name}")),
            E::At(kind, at) => (kind, at),
            E::Mid(at) => {
                let on = self.reference(cx, E::At("edges", at))?;
                return Ok(Reference::Implicit { on: Box::new(on), inference: noshape_model::reference::Inference::Midpoint });
            }
        };
        let mut ctx = noshape_ops::Ctx::new();
        let args = serde_json::json!({ "doc": self.path.to_str(), "kind": kind, "at": [at] });
        let op = noshape_ops::registry::by_tool_name("query_entities").ok_or("no query_entities")?;
        let got = noshape_ops::registry::run(&mut ctx, op, &args).map_err(|e| e.to_json().to_string())?.value;
        let id = got[kind].get(0).and_then(|v| v["id"].as_str()).ok_or(format!("nothing for {e:?}: {got}"))?;
        noshape_ops::refs::parse_str(id).ok_or(format!("no reference {id}"))
    }

    /// Selects `entities` (and `features` in the list), then the summary bar once its measurement is in.
    fn readout(&self, cx: &mut TestAppContext, entities: &[E], features: &[&str]) -> Result<Vec<String>, String> {
        let refs: Vec<Reference> = entities.iter().map(|e| self.reference(cx, *e)).collect::<Result<_, _>>()?;
        cx.update(|cx| {
            self.view.update(cx, |v, cx| {
                let ids: Vec<noshape_model::reference::FeatureId> = v.studio(cx).map(|s| s.features.iter().filter(|f| features.contains(&f.name.as_str())).map(|f| f.id).collect()).unwrap_or_default();
                v.selection = refs.clone();
                v.selected_features = ids;
                v.refresh_scene(false, cx);
                cx.notify();
            })
        });
        // The measurement comes from a background task; nothing may be the answer (wait it out then).
        let t = std::time::Instant::now();
        loop {
            draw(cx, self.window);
            let pending = cx.update(|cx| self.view.read(cx).measure_cache.as_ref().is_some_and(|m| m.results.is_none()));
            if !pending || t.elapsed() > std::time::Duration::from_secs(20) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
            cx.executor().advance_clock(std::time::Duration::from_millis(20));
        }
        draw(cx, self.window);
        Ok(cx.update(|cx| self.view.read(cx).ui.measure_readout.iter().map(|r| format!("{}: {} {}", r["name"].as_str().unwrap_or(""), r["value"].as_str().unwrap_or(""), r["units"].as_str().unwrap_or("")).trim().to_string()).collect()))
    }

/// The pointer onto element `sel` (`None`: off into empty graphics), then 500 ms of the clock.
    fn point_at(&self, cx: &mut TestAppContext, sel: Option<&str>) -> Result<(), String> {
        draw(cx, self.window);
        let at = match sel {
            Some(sel) => gpui_kit::VisualTestContext::from_window(self.window, cx).debug_bounds(Box::leak(sel.to_string().into_boxed_str())).ok_or(format!("no {sel}"))?.center(),
            None => point(px(600.), px(300.)),
        };
        gpui_kit::VisualTestContext::from_window(self.window, cx).simulate_event(MouseMoveEvent { position: at, pressed_button: None, modifiers: Modifiers::default() });
        for _ in 0..50 {
            cx.executor().advance_clock(std::time::Duration::from_millis(10));
            draw(cx, self.window);
        }
        Ok(())
    }

    /// The hovered measurement's lines, each named by its colour.
    fn lines(&self, cx: &mut TestAppContext) -> Vec<String> {
        cx.update(|cx| {
            self.view.read(cx).measure_hover_lines().segments.iter().map(|s| match s.color {
                [20, 20, 20, _] => "dashed black",
                [0xbd, 0x30, 0x39, _] => "X red",
                [0x00, 0x94, 0x00, _] => "Y green",
                [0x16, 0x51, 0xb0, _] => "Z blue",
                _ => "other",
            }.to_string()).collect()
        })
    }

    fn panel(&self, cx: &mut TestAppContext, open: bool) {
        if cx.update(|cx| self.view.read(cx).ui.measure_pinned) != open {
            let focus = cx.update(|cx| self.view.read(cx).focus.clone());
            cx.update_window(self.window, |_, w, cx| w.focus(&focus, cx)).unwrap();
            gpui_kit::VisualTestContext::from_window(self.window, cx).simulate_keystrokes("[");
            draw(cx, self.window);
        }
    }

    fn click_value(&self, cx: &mut TestAppContext, name: &str) -> Result<(), String> {
        let b = gpui_kit::VisualTestContext::from_window(self.window, cx).debug_bounds(Box::leak(format!("measure_value:{name}").into_boxed_str())).ok_or(format!("no value {name}"))?;
        let mut v = gpui_kit::VisualTestContext::from_window(self.window, cx);
        let at = point(b.center().x, b.center().y);
        v.simulate_event(MouseMoveEvent { position: at, pressed_button: None, modifiers: Modifiers::default() });
        v.simulate_event(MouseDownEvent { button: MouseButton::Left, position: at, modifiers: Modifiers::default(), click_count: 1, first_mouse: false });
        draw(cx, self.window);
        gpui_kit::VisualTestContext::from_window(self.window, cx).simulate_event(MouseUpEvent { button: MouseButton::Left, position: at, modifiers: Modifiers::default(), click_count: 1 });
        draw(cx, self.window);
        Ok(())
    }
}

type Check = fn(&Bench, &mut TestAppContext) -> Result<Vec<String>, String>;

struct Row {
    id: &'static str,
    /// Live's summary bar (measure.json "summary_bar"), each measurement "<name>: <value> <unit>".
    live: &'static [&'static str],
    check: Check,
    gap: bool,
}

fn rows() -> Vec<Row> {
    vec![
        Row { id: "one vertex", live: &["Point:", "svg-icon-XYZ-Icons-AlongX: 40.000 mm", "svg-icon-XYZ-Icons-AlongY: 0.000 mm", "svg-icon-XYZ-Icons-AlongZ: 0.000 mm"], gap: false, check: |b, cx| b.readout(cx, &[V40], &[]) },
        Row { id: "one straight edge", live: &["Length: 40.000 mm"], gap: false, check: |b, cx| b.readout(cx, &[FRONT_EDGE], &[]) },
        Row { id: "one circular edge", live: &["Diameter: 8.000 mm"], gap: false, check: |b, cx| b.readout(cx, &[HOLE_TOP], &[]) },
        // qa VER-COMPARE-READOUT (live ver-compare-hover-part 5, retake 24af0f0f2): Compare shows no readout.
        Row { id: "one circular edge in Versions and history › Compare", live: &[], gap: false, check: |b, cx| {
            crate::part_studio_hooks::TEST_COMPARE.with(|c| c.set(Some(0.5)));
            let got = b.readout(cx, &[HOLE_TOP], &[]);
            crate::part_studio_hooks::TEST_COMPARE.with(|c| c.set(None));
            got
        } },
        Row { id: "one face (the front: less the chamfer's and the fillet's corners)", live: &["Area: 398.642 mm2"], gap: false, check: |b, cx| b.readout(cx, &[FRONT], &[]) },
        Row { id: "two vertices", live: &["Min dist: 20.000 mm"], gap: false, check: |b, cx| b.readout(cx, &[V40, V60], &[]) },
        Row { id: "a vertex and a circle", live: &["Min dist: 39.990 mm", "Max dist: 47.778 mm", "Center dist: 43.875 mm"], gap: false, check: |b, cx| b.readout(cx, &[V60, HOLE_TOP], &[]) },
        Row { id: "two circles", live: &["Min dist: 10.000 mm", "Max dist: 12.806 mm", "Center dist: 10.000 mm"], gap: false, check: |b, cx| b.readout(cx, &[HOLE_TOP, HOLE_BOTTOM], &[]) },
        Row { id: "two parallel faces", live: &["Parallel dist: 5.000 mm"], gap: false, check: |b, cx| b.readout(cx, &[TOP, P2_TOP], &[]) },
        Row { id: "a face and a parallel line", live: &["Parallel dist: 10.000 mm"], gap: false, check: |b, cx| b.readout(cx, &[TOP, FRONT_EDGE], &[]) },
        Row { id: "two collinear edges", live: &["Parallel dist: 0.000 mm"], gap: false, check: |b, cx| b.readout(cx, &[FRONT_EDGE, P2_FRONT_EDGE], &[]) },
        Row { id: "two perpendicular edges", live: &["Perp lines angle: 90.000 deg"], gap: false, check: |b, cx| b.readout(cx, &[FRONT_EDGE, LEFT_EDGE], &[]) },
        Row { id: "two perpendicular faces", live: &["Perp planes angle: 90.000 deg"], gap: false, check: |b, cx| b.readout(cx, &[FRONT, BOTTOM], &[]) },
        Row { id: "a face and a perpendicular plane", live: &["Perp planes angle: 90.000 deg"], gap: false, check: |b, cx| b.readout(cx, &[FRONT, E::Id("plane:top")], &[]) },
        Row { id: "three vertices", live: &[], gap: false, check: |b, cx| b.readout(cx, &[V40, V60, E::At("vertices", [100., 0., 0.])], &[]) },
        Row { id: "a part", live: &[], gap: false, check: |b, cx| b.readout(cx, &[E::Part("Part 2")], &[]) },
        Row { id: "a feature", live: &[], gap: false, check: |b, cx| b.readout(cx, &[], &["Extrude 1"]) },
        Row { id: "the details panel open ([): the summary bar empty", live: &[], gap: false, check: |b, cx| {
            b.readout(cx, &[V40, V60], &[])?;
            let focus = cx.update(|cx| b.view.read(cx).focus.clone());
            cx.update_window(b.window, |_, w, cx| w.focus(&focus, cx)).unwrap();
            gpui_kit::VisualTestContext::from_window(b.window, cx).simulate_keystrokes("[");
            let open = b.readout(cx, &[V40, V60], &[])?;
            gpui_kit::VisualTestContext::from_window(b.window, cx).simulate_keystrokes("[");
            draw(cx, b.window);
            let pinned = cx.update(|cx| b.view.read(cx).ui.measure_pinned);
            if pinned {
                return Err("[ again left the panel open".into());
            }
            Ok(open)
        } },
        Row { id: "a value clicked: 5 decimals, again: 3", live: &["Min dist: 20.00000 mm", "Min dist: 20.000 mm"], gap: false, check: |b, cx| {
            b.readout(cx, &[V40, V60], &[])?;
            b.click_value(cx, "Min dist")?;
            let five = b.readout(cx, &[V40, V60], &[])?;
            b.click_value(cx, "Min dist")?;
            Ok(five.into_iter().chain(b.readout(cx, &[V40, V60], &[])?).collect())
        } },
        Row { id: "a value clicked: its text selected, Ctrl+C copies it", live: &["20.00000"], gap: false, check: |b, cx| {
            b.readout(cx, &[V40, V60], &[])?;
            b.click_value(cx, "Min dist")?;
            let focus = cx.update(|cx| b.view.read(cx).focus.clone());
            cx.update_window(b.window, |_, w, cx| w.focus(&focus, cx)).unwrap();
            gpui_kit::VisualTestContext::from_window(b.window, cx).simulate_keystrokes("secondary-c");
            let copied = cx.update(|cx| cx.read_from_clipboard().and_then(|c| c.text()));
            // Back to 3 decimals for the next rows.
            b.click_value(cx, "Min dist")?;
            Ok(copied.into_iter().collect())
        } },
        Row { id: "a value hovered: its 5 decimals' tooltip and its dashed line", live: &["Min dist: 20.000 mm", "20.00000", "dashed black", "left: no line"], gap: false, check: |b, cx| {
            b.panel(cx, false);
            let mut out = b.readout(cx, &[V40, V60], &[])?;
            // From empty space (an earlier row's click left the pointer on this value).
            b.point_at(cx, None)?;
            crate::ui_script::clear_tooltip();
            b.point_at(cx, Some("measure_value:Min dist"))?;
            out.push(crate::ui_script::shown_tooltip().unwrap_or_default());
            out.extend(b.lines(cx));
            b.point_at(cx, None)?;
            out.push(if b.lines(cx).is_empty() { "left: no line".into() } else { "left: a line".into() });
            Ok(out)
        } },
        Row { id: "the panel's rows hovered: the line, then the X / Y / Z legs", live: &["dashed black", "X red", "Z blue"], gap: false, check: |b, cx| {
            b.readout(cx, &[V40, V60_TOP], &[])?;
            b.panel(cx, true);
            b.point_at(cx, Some("measure-row-0"))?;
            let mut out = b.lines(cx);
            b.point_at(cx, Some("measure-row-1"))?;
            out.extend(b.lines(cx));
            b.point_at(cx, None)?;
            b.panel(cx, false);
            Ok(out)
        } },
        Row { id: "the details panel: 220 × 345 for a distance and its components", live: &["220 x 345"], gap: false, check: |b, cx| {
            b.readout(cx, &[V40, V60_TOP], &[])?;
            b.panel(cx, true);
            draw(cx, b.window);
            let size = gpui_kit::VisualTestContext::from_window(b.window, cx).debug_bounds("measure-panel").map(|r| r.size);
            b.panel(cx, false);
            let size = size.ok_or("no panel")?;
            Ok(vec![format!("{} x {}", f32::from(size.width).round(), f32::from(size.height).round())])
        } },
        Row { id: "a second entity hovered: no preview", live: &["Point:", "svg-icon-XYZ-Icons-AlongX: 40.000 mm", "svg-icon-XYZ-Icons-AlongY: 0.000 mm", "svg-icon-XYZ-Icons-AlongZ: 0.000 mm"], gap: false, check: |b, cx| {
            let other = b.reference(cx, V60)?;
            b.readout(cx, &[V40], &[])?;
            cx.update(|cx| b.view.update(cx, |v, cx| {
                v.hover = Some(other.clone());
                v.refresh_scene(false, cx);
            }));
            let out = b.readout(cx, &[V40], &[]);
            cx.update(|cx| b.view.update(cx, |v, _| v.hover = None));
            out
        } },
    ]
}

/// Every row at a pixel ratio of 1 and 2 on one document each. Prints matched / total (`MEASURE_ROW=<part of an id>`
/// runs only those rows).
#[gpui::test]
fn the_measure_matrix_matches_live(cx: &mut TestAppContext) {
    let rows = rows();
    let only = std::env::var("MEASURE_ROW").ok();
    let benches: Vec<Bench> = [1.0f32, 2.0].iter().map(|s| Bench::open(cx, &format!("measure-{s}"), *s)).collect();
    let mut report = Vec::new();
    let mut moved = Vec::new();
    for r in &rows {
        if only.as_deref().is_some_and(|o| !r.id.contains(o)) {
            continue;
        }
        let want: Vec<String> = r.live.iter().map(|s| s.to_string()).collect();
        let mut why = Vec::new();
        for (b, scale) in benches.iter().zip([1, 2]) {
            match (r.check)(b, cx) {
                Ok(got) if got == want => {}
                Ok(got) => why.push(format!("scale {scale}: ours {got:?}")),
                Err(e) => why.push(format!("scale {scale}: {e}")),
            }
        }
        let ok = why.is_empty();
        report.push(format!("{} {} (live: {:?}){}", if ok { "=" } else { "≠" }, r.id, r.live, if ok { String::new() } else { format!("\n    {}", why.join("\n    ")) }));
        if ok == r.gap {
            moved.push(r.id);
        }
    }
    let matched = report.iter().filter(|l| l.starts_with('=')).count();
    eprintln!("measure matrix: {matched} / {} rows match live\n{}", report.len(), report.join("\n"));
    assert!(moved.is_empty(), "rows whose status moved (update interaction-feel.md and the row's gap): {moved:?}");
}

/// live2 measure-mass-1118 (the bottom-right summary on its "l2 measure feel" studio: a 40 × 30 × 10 block, a wedge
/// (60,0)(90,0)(60,20) × 10, a cylinder r8 at (120,15) × 15, a wedge (60,40)(60,60)(90,60) × 10): readouts the matrix
/// didn't have, among them live's angle between two faces or edges, taken toward each one's middle from where they meet.
#[gpui::test]
fn measure_pairs_on_lives_wedges_and_cylinder(cx: &mut TestAppContext) {
    let poly = |n: u32, pts: &[[f64; 2]]| {
        let mut p = pts.to_vec();
        p.push(pts[0]);
        serde_json::json!({ "type": "newSketch", "name": format!("Sketch {n}"), "plane": "Top", "shapes": [{ "line": p }] })
    };
    let extrude = |n: u32, depth: &str| serde_json::json!({ "type": "extrude", "name": format!("Extrude {n}"), "params": { "entities": [format!("Sketch {n} region 0")], "endBound": "BLIND", "depth": depth, "operationType": "NEW" } });
    let doc = serde_json::json!({ "units": "mm", "features": [
        poly(1, &[[0., 0.], [40., 0.], [40., 30.], [0., 30.]]), extrude(1, "10 mm"),
        poly(2, &[[60., 0.], [90., 0.], [60., 20.]]), extrude(2, "10 mm"),
        { "type": "newSketch", "name": "Sketch 3", "plane": "Top", "shapes": [{ "circle": [[120, 15], [128, 15]] }] }, extrude(3, "15 mm"),
        poly(4, &[[60., 40.], [60., 60.], [90., 60.]]), extrude(4, "10 mm")
    ] });
    let b = Bench::open_doc(cx, "measure-pairs", 1.0, doc, 4);
    let hyp = E::At("edges", [75., 10., 10.]);
    let hyp4 = E::At("faces", [75., 50., 5.]);
    let cyl = E::At("faces", [128., 15., 7.]);
    let cases: [(&[E], &str); 22] = [
        (&[hyp], "Length:36.056mm"),
        (&[hyp, E::At("edges", [40., 15., 10.])], "Angle:56.310deg"),
        (&[hyp, E::At("edges", [60., 10., 10.])], "Angle:56.310deg"),
        (&[hyp, E::At("edges", [20., 30., 10.])], "Angle:146.310deg"),
        (&[hyp4], "Area:360.555mm2"),
        (&[hyp4, E::At("faces", [40., 15., 5.])], "Angle:123.690deg"),
        (&[E::At("faces", [40., 15., 5.]), hyp4], "Angle:123.690deg"),
        (&[hyp4, E::At("faces", [20., 0., 5.])], "Angle:33.690deg"),
        (&[E::At("faces", [20., 0., 5.]), hyp4], "Angle:33.690deg"),
        (&[hyp4, E::At("faces", [20., 15., 10.])], "Perp planes angle:90.000deg"),
        (&[cyl], "Diameter:16.000mm"),
        (&[cyl, E::At("faces", [20., 15., 10.])], "Perp axes angle:90.000deg"),
        (&[cyl, E::At("faces", [120., 15., 15.])], "Perp axes angle:90.000deg"),
        (&[cyl, E::Mid([20., 0., 0.])], "Min dist:93.119mm Center axes dist:101.119mm Max dist:110.145mm"),
        (&[E::At("vertices", [40., 0., 10.])], "Point: 40.000mm 0.000mm 10.000mm"),
        (&[E::At("vertices", [40., 0., 10.]), E::At("faces", [65., 5., 10.])], "Min dist:20.000mm"),
        (&[E::At("vertices", [40., 0., 10.]), E::At("faces", [75., 0., 5.])], "Min dist:20.000mm"),
        (&[E::At("faces", [20., 15., 10.]), E::At("faces", [65., 5., 10.])], "Parallel dist:0.000mm"),
        (&[E::At("faces", [20., 15., 10.]), hyp], "Parallel dist:0.000mm"),
        (&[E::At("faces", [120., 15., 15.]), E::At("edges", [40., 15., 10.])], "Parallel dist:5.000mm"),
        (&[E::At("edges", [128., 15., 15.]), hyp], "Min dist:26.026mm Max dist:68.391mm Center dist:29.548mm"),
        (&[E::At("edges", [128., 15., 15.]), E::At("faces", [120., 15., 15.])], "Face tangent angle:0.000deg Center dist:0.000mm Max dist:16.000mm Min dist:0.000mm"),
    ];
    let squash = |t: &str| t.split_whitespace().collect::<String>();
    let mut off = Vec::new();
    for (sel, live) in cases {
        let rows = b.readout(cx, sel, &[]).unwrap_or_else(|e| vec![e]);
        // ("svg-icon-XYZ-Icons-AlongX: 40.000 mm": the axis icon's name before the value.)
        let ours: String = rows.iter().map(|r| squash(r.rsplit_once("Icons-").map_or(r.as_str(), |(_, v)| v.split_once(':').map_or(v, |(_, x)| x)))).collect();
        if ours != squash(live) {
            off.push(format!("{sel:?}: ours {ours}, live {}", squash(live)));
        }
    }
    assert!(off.is_empty(), "measure pairs off live's:\n{}", off.join("\n"));
}

/// live2 measure-mass-1118: the Mass properties button again leaves the panel open; `[` over it closes it and opens
/// Measure empty, the measured parts deselected.
#[gpui::test]
fn mass_stays_open_and_bracket_opens_an_empty_measure(cx: &mut TestAppContext) {
    let b = Bench::open(cx, "mass-bracket", 1.0);
    let part = b.reference(cx, E::Part("Part 2")).unwrap();
    cx.update(|cx| b.view.update(cx, |v, cx| {
        v.selection = vec![part.clone()];
        v.open_mass(cx);
        v.open_mass(cx);
    }));
    draw(cx, b.window);
    assert!(cx.update(|cx| b.view.read(cx).show_mass), "the button again leaves it open");
    let focus = cx.update(|cx| b.view.read(cx).focus.clone());
    cx.update_window(b.window, |_, w, cx| w.focus(&focus, cx)).unwrap();
    gpui_kit::VisualTestContext::from_window(b.window, cx).simulate_keystrokes("[");
    draw(cx, b.window);
    let state = cx.update(|cx| {
        let v = b.view.read(cx);
        (v.show_mass, v.ui.measure_pinned, v.selection.len())
    });
    assert_eq!(state, (false, true, 0), "(Mass open, Measure open, selected) after [");
}

/// Live2 measure-mass-1118 (qa FEEL-MASS-PANEL-PLACE): Mass properties opens over the list's right part, 32 px left of
/// the graphics area (x 214 against 246), 3 px under the toolbar; it was clipped to the graphics area, under the list.
#[gpui::test]
fn mass_opens_over_the_lists_right_part(cx: &mut TestAppContext) {
    let b = Bench::open(cx, "mass-place", 1.0);
    let part = b.reference(cx, E::Part("Part 2")).unwrap();
    cx.update(|cx| b.view.update(cx, |v, cx| {
        v.selection = vec![part.clone()];
        v.open_mass(cx);
    }));
    draw(cx, b.window);
    let mut vcx = gpui_kit::VisualTestContext::from_window(b.window, cx);
    let (dialog, area) = (vcx.debug_bounds("mass-dialog").expect("the Mass dialog"), vcx.debug_bounds("viewport-area").expect("the graphics area"));
    assert_eq!((f32::from(area.left() - dialog.left()), f32::from(dialog.top() - area.top())), (32.0, 3.0), "(px over the list, px under the toolbar)");
}

/// Live (10-27, section-1118): the Section view dialog opens at (396, 179): 150 px right of and 100 px below the feature
/// dialogs' spot (246, 79), 3 px under the toolbar; not docked there.
#[gpui::test]
fn the_section_dialog_opens_right_of_and_below_the_feature_dialogs_spot(cx: &mut TestAppContext) {
    let b = Bench::open(cx, "section-place", 1.0);
    cx.update_window(b.window, |_, w, cx| b.view.update(cx, |v, cx| {
        v.selection.clear();
        v.toggle_section(w, cx);
    }))
    .unwrap();
    draw(cx, b.window);
    let mut vcx = gpui_kit::VisualTestContext::from_window(b.window, cx);
    let (dialog, area) = (vcx.debug_bounds("section-dialog-place").expect("the Section view dialog"), vcx.debug_bounds("viewport-area").expect("the graphics area"));
    assert_eq!((f32::from(dialog.left() - area.left()), f32::from(dialog.top() - area.top())), (150.0, 103.0), "(px right of, px under the graphics' corner)");
}

/// Live (behaviours "number and unit display rules", 11-18): a click on a Mass properties value shows it at the unit's
/// highest precision (mm 5, mm³ 8), a second click as before (3).
#[gpui::test]
fn a_clicked_mass_value_toggles_the_highest_precision(cx: &mut TestAppContext) {
    let b = Bench::open(cx, "mass-precise", 1.0);
    // Part 1 (its hole, fillet and chamfer): a volume with decimals (Part 2's 6000 mm³ has none to add).
    let part = b.reference(cx, E::Part("Part 1")).unwrap();
    cx.update(|cx| b.view.update(cx, |v, cx| {
        v.selection = vec![part.clone()];
        v.open_mass(cx);
    }));
    let texts = |cx: &mut TestAppContext| {
        draw(cx, b.window);
        let state = cx.update(|cx| b.view.read(cx).ui.mass_state.clone()).unwrap_or_default();
        let fields = state["fields"].as_array().cloned().unwrap_or_default();
        let text = fields.iter().find(|f| f["label"] == "Volume").and_then(|f| f["text"].as_str().map(str::to_string)).unwrap_or_default();
        let decimals = text.split_whitespace().next().and_then(|n| n.split_once('.')).map_or(0, |(_, d)| d.len());
        (decimals, text)
    };
    // The values are worked out off the UI thread: the panel blank until then.
    let volume = |cx: &gpui_kit::App| {
        let state = b.view.read(cx).ui.mass_state.clone().unwrap_or_default();
        state["fields"].as_array().is_some_and(|f| f.iter().any(|f| f["label"] == "Volume" && f["text"].as_str().is_some_and(|t| !t.is_empty())))
    };
    wait_until(cx, b.window, volume);
    let before = texts(cx);
    let click = |cx: &mut TestAppContext, key: &str| {
        let at = gpui_kit::VisualTestContext::from_window(b.window, cx).debug_bounds(Box::leak(format!("mass-value-{key}").into_boxed_str())).expect("the value").center();
        gpui_kit::VisualTestContext::from_window(b.window, cx).simulate_click(at, Modifiers::default());
    };
    click(cx, "volume");
    let precise = texts(cx);
    click(cx, "volume");
    let back = texts(cx);
    let state = cx.update(|cx| b.view.read(cx).ui.mass_state.clone());
    assert!(precise.0 > before.0 && precise.0 <= 8 && back == before, "the volume: {before:?}, clicked {precise:?}, again {back:?} (state {state:?})");
}

/// live2 measure-mass-1118: while Mass properties is open the bottom-right summary is gone, a click on empty graphics
/// keeps the measured parts, and Esc closes the panel keeping the selection.
#[gpui::test]
fn mass_hides_the_summary_keeps_its_parts_on_an_empty_click_and_esc_keeps_the_selection(cx: &mut TestAppContext) {
    let b = Bench::open(cx, "mass-open", 1.0);
    // A vertex's summary ("Point: …") with the panel closed, none with it open.
    let before = b.readout(cx, &[V40], &[]).unwrap();
    cx.update(|cx| b.view.update(cx, |v, cx| v.open_mass(cx)));
    let open_rows = b.readout(cx, &[V40], &[]).unwrap();
    assert!(!before.is_empty() && open_rows.is_empty(), "a vertex's summary, the panel closed: {before:?}; open: {open_rows:?}");
    let parts = vec![b.reference(cx, E::Part("Part 1")).unwrap(), b.reference(cx, E::Part("Part 2")).unwrap()];
    cx.update(|cx| b.view.update(cx, |v, cx| {
        v.selection = parts.clone();
        v.refresh_scene(false, cx);
    }));
    draw(cx, b.window);
    let state = |cx: &mut TestAppContext| cx.update(|cx| {
        let v = b.view.read(cx);
        (v.show_mass, v.selection.len(), v.ui.measure_readout.len())
    });
    let open = state(cx);
    cx.update_window(b.window, |_, w, cx| b.view.update(cx, |v, cx| {
        let vp = v.viewport.clone();
        v.on_viewport_event(&vp, &ViewportEvent::Click { pick: None, position: glam::DVec2::new(40., 40.), modifiers: Modifiers::default() }, w, cx);
    }))
    .unwrap();
    draw(cx, b.window);
    let clicked = state(cx);
    let focus = cx.update(|cx| b.view.read(cx).focus.clone());
    cx.update_window(b.window, |_, w, cx| w.focus(&focus, cx)).unwrap();
    gpui_kit::VisualTestContext::from_window(b.window, cx).simulate_keystrokes("escape");
    draw(cx, b.window);
    let closed = state(cx);
    assert_eq!((open, clicked, (closed.0, closed.1)), ((true, 2, 0), (true, 2, 0), (false, 2)), "(Mass open, selected, summary rows) when opened, after an empty click; (open, selected) after Esc");
}

/// Live2 mass-appear-1118: a part added to Mass properties keeps the old totals shown until the new ones come (no
/// blank, no spinner), then the sum.
#[gpui::test]
fn mass_keeps_the_old_totals_until_an_added_parts_come(cx: &mut TestAppContext) {
    let b = Bench::open(cx, "mass-add", 1.0);
    let (p1, p2) = (b.reference(cx, E::Part("Part 1")).unwrap(), b.reference(cx, E::Part("Part 2")).unwrap());
    cx.update(|cx| b.view.update(cx, |v, cx| {
        v.selection = vec![p1.clone()];
        v.open_mass(cx);
    }));
    let volume = |cx: &mut TestAppContext| {
        draw(cx, b.window);
        let state = cx.update(|cx| b.view.read(cx).ui.mass_state.clone()).unwrap_or_default();
        state["fields"].as_array().and_then(|f| f.iter().find(|f| f["label"] == "Volume").and_then(|f| f["text"].as_str().map(str::to_string))).unwrap_or_default()
    };
    wait_until(cx, b.window, |cx| {
        let state = b.view.read(cx).ui.mass_state.clone().unwrap_or_default();
        state["fields"].as_array().is_some_and(|f| f.iter().any(|f| f["label"] == "Volume" && f["text"].as_str().is_some_and(|t| !t.is_empty())))
    });
    let one = volume(cx);
    // Part 2 added: drawn before its measurement can have come back (the clock held, no task run in between).
    cx.update(|cx| b.view.update(cx, |v, cx| {
        v.selection = vec![p1.clone(), p2.clone()];
        v.refresh_scene(false, cx);
        cx.notify();
    }));
    // (One frame, no task run after it: the measurement is still out.)
    cx.update_window(b.window, |_, w, cx| w.draw(cx).clear(cx)).unwrap();
    let pending = cx.update(|cx| b.view.read(cx).ui.mass_state.clone()).unwrap_or_default()["fields"]
        .as_array()
        .and_then(|f| f.iter().find(|f| f["label"] == "Volume").and_then(|f| f["text"].as_str().map(str::to_string)))
        .unwrap_or_default();
    let still_out = cx.update(|cx| b.view.read(cx).ui.mass_job.as_ref().is_some_and(|j| j.value.is_none()));
    wait_until(cx, b.window, |cx| b.view.read(cx).ui.mass_job.as_ref().is_some_and(|j| j.value.is_some()));
    let both = volume(cx);
    assert!(still_out && !one.is_empty() && pending == one && both != one && !both.is_empty(), "Volume: Part 1 {one:?}, as Part 2 is added {pending:?} (measuring: {still_out}), then {both:?}");
}

/// Live measure-mass-1118: the bottom-right summary keeps its old text until the new selection's answer arrives, then
/// refills (no blank between).
#[gpui::test]
fn the_summary_keeps_its_old_text_until_the_answer(cx: &mut TestAppContext) {
    let b = Bench::open(cx, "summary-keeps", 1.0);
    let point = b.readout(cx, &[V40], &[]).unwrap();
    let edge = b.reference(cx, FRONT_EDGE).unwrap();
    cx.update(|cx| b.view.update(cx, |v, cx| {
        v.selection = vec![edge.clone()];
        v.refresh_scene(false, cx);
        cx.notify();
    }));
    // (One frame, no task run after it: the measurement is still out.)
    cx.update_window(b.window, |_, w, cx| w.draw(cx).clear(cx)).unwrap();
    let rows = |cx: &mut TestAppContext| cx.update(|cx| b.view.read(cx).ui.measure_readout.iter().map(|r| format!("{}: {} {}", r["name"].as_str().unwrap_or(""), r["value"].as_str().unwrap_or(""), r["units"].as_str().unwrap_or("")).trim().to_string()).collect::<Vec<_>>());
    let pending = rows(cx);
    let measuring = cx.update(|cx| b.view.read(cx).measure_cache.as_ref().is_some_and(|m| m.results.is_none()));
    let after = b.readout(cx, &[FRONT_EDGE], &[]).unwrap();
    assert!(measuring && pending == point && after != point && !after.is_empty(), "the summary: a vertex {point:?}; an edge clicked, measuring ({measuring}) {pending:?}; answered {after:?}");
}

/// Live measure-mass-1118: with Mass properties open a face click adds its part, a face of a measured part clicked
/// again takes the part out; the volume is the parts' sum.
#[gpui::test]
fn mass_adds_a_clicked_faces_part_and_a_second_click_takes_it_out(cx: &mut TestAppContext) {
    let b = Bench::open(cx, "mass-click", 1.0);
    let (p1, p2) = (b.reference(cx, E::Part("Part 1")).unwrap(), b.reference(cx, E::Part("Part 2")).unwrap());
    cx.update(|cx| b.view.update(cx, |v, cx| v.open_mass(cx)));
    draw(cx, b.window);
    let volume = |cx: &mut TestAppContext| {
        wait_until(cx, b.window, |cx| b.view.read(cx).ui.mass_job.as_ref().is_none_or(|j| j.value.is_some()));
        draw(cx, b.window);
        let state = cx.update(|cx| b.view.read(cx).ui.mass_state.clone()).unwrap_or_default();
        state["fields"].as_array().and_then(|f| f.iter().find(|f| f["label"] == "Volume").and_then(|f| f["text"].as_str().map(str::to_string))).unwrap_or_default()
    };
    let mut got = Vec::new();
    let mut volumes = Vec::new();
    for face in [TOP, P2_TOP, BOTTOM] {
        let r = b.reference(cx, face).unwrap();
        cx.update_window(b.window, |_, w, cx| b.view.update(cx, |v, cx| {
            let pick = v.built.as_ref().and_then(|bt| bt.picks.ids_for(&r).first().copied());
            let vp = v.viewport.clone();
            v.on_viewport_event(&vp, &ViewportEvent::Click { pick, position: glam::DVec2::new(500., 300.), modifiers: Modifiers::default() }, w, cx);
        }))
        .unwrap();
        draw(cx, b.window);
        got.push(cx.update(|cx| {
            let s = &b.view.read(cx).selection;
            (s.contains(&p1), s.contains(&p2), s.len())
        }));
        volumes.push(volume(cx));
    }
    assert_eq!(got, vec![(true, false, 1), (true, true, 2), (false, true, 1)], "(Part 1, Part 2 measured, selected) after Part 1's top, Part 2's top, Part 1's bottom");
    let mm3 = |t: &str| t.split_whitespace().next().and_then(|n| n.replace(',', "").parse::<f64>().ok());
    let v: Vec<Option<f64>> = volumes.iter().map(|t| mm3(t)).collect();
    let summed = matches!(v.as_slice(), [Some(a), Some(ab), Some(b)] if ((a + b) - ab).abs() <= 1e-3 * ab.abs());
    assert!(summed, "Volume: Part 1 {:?}, both {:?}, Part 2 {:?}: the two parts' sum", volumes[0], volumes[1], volumes[2]);
}

/// qa analysis-interference (live): a face's summary goes when Interference detection opens (it takes the face's
/// part); the other analysis tools keep it (qa ANALYSIS-READOUT-HIDDEN, live analysis-connection, -zebra,
/// -reflection, -deviation: Area 1120 mm² under their dialogs).
#[gpui::test]
fn only_interference_detection_hides_the_summary(cx: &mut TestAppContext) {
    use crate::part_studio::analysis_tools::ToolKind;
    let b = Bench::open(cx, "summary-analysis", 1.0);
    let toggle = |cx: &mut TestAppContext, kind: ToolKind| {
        cx.update_window(b.window, |_, w, cx| b.view.update(cx, |v, cx| v.toggle_analysis_tool(kind, w, cx))).unwrap();
        wait_until(cx, b.window, |cx| !b.view.read(cx).ui_busy());
        draw(cx, b.window);
        cx.update(|cx| (b.view.read(cx).ui.analysis_tool.is_some(), b.view.read(cx).ui.measure_readout.clone()))
    };
    for kind in [ToolKind::Interference, ToolKind::Connection, ToolKind::Zebra, ToolKind::Reflection, ToolKind::Deviation] {
        let face = b.readout(cx, &[TOP], &[]).unwrap();
        let open = toggle(cx, kind);
        let hides = matches!(kind, ToolKind::Interference);
        assert!(!face.is_empty() && open.0 && open.1.is_empty() == hides, "{kind:?}: the top face's summary {face:?}; with its dialog open: {:?}", open.1);
        toggle(cx, kind);
    }
}

/// qa ANALYSIS-READOUT-HIDDEN, live edge-menu-curve-analysis step 2: an edge's Curve/surface analysis from its context
/// menu keeps the edge selected and its "Length" readout under the panel.
#[gpui::test]
fn an_edges_curve_analysis_from_its_menu_keeps_its_length(cx: &mut TestAppContext) {
    use crate::part_studio::analysis_tools::ToolKind;
    let b = Bench::open(cx, "summary-curve-menu", 1.0);
    let edge = b.readout(cx, &[FRONT_EDGE], &[]).unwrap();
    cx.update_window(b.window, |_, w, cx| b.view.update(cx, |v, cx| { v.ui.analysis_menu_target = true; v.toggle_analysis_tool(ToolKind::CurveSurface, w, cx) })).unwrap();
    wait_until(cx, b.window, |cx| !b.view.read(cx).ui_busy());
    draw(cx, b.window);
    let open = cx.update(|cx| { let v = b.view.read(cx); (v.ui.analysis_tool.is_some(), v.selection.len(), v.ui.measure_readout.len()) });
    assert!(edge.first().is_some_and(|l| l.starts_with("Length")) && open == (true, 1, edge.len()), "the edge's readout {edge:?}; with Curve/surface open (open, selected, readout rows): {open:?}");
}

/// live2 mass-override-1119: on one part with a material the Center of mass and inertia Override boxes enable (a part
/// without one keeps them greyed); ticked, the values they had become editable. A centre's X typed moves the inertia
/// to the new centre (parallel axis); Lxy typed leaves Lyx as it was; ✓ stores both on the part, no feature added.
#[gpui::test]
fn mass_centre_and_inertia_overrides_on_a_part_with_a_material(cx: &mut TestAppContext) {
    let b = Bench::open(cx, "mass-com-override", 1.0);
    let state = |cx: &mut TestAppContext| {
        draw(cx, b.window);
        cx.update(|cx| b.view.read(cx).ui.mass_state.clone()).unwrap_or_default()
    };
    let field = |s: &serde_json::Value, label: &str| s["fields"].as_array().and_then(|f| f.iter().find(|f| f["label"].as_str().is_some_and(|l| l.starts_with(label))).cloned()).unwrap_or_default();
    let cell = |s: &serde_json::Value, label: &str| field(s, label)["text"].as_str().and_then(|t| t.split_whitespace().next()?.parse::<f64>().ok());
    let measured = |s: &serde_json::Value| cell(s, "Lyy").is_some();
    let open = |cx: &mut TestAppContext, part: &'static str| {
        let r = b.reference(cx, E::Part(part)).unwrap();
        cx.update(|cx| b.view.update(cx, |v, cx| {
            v.set_show_mass(false, cx);
            v.selection = vec![r.clone()];
            v.open_mass(cx);
        }));
        let window = b.window;
        wait_until(cx, window, |cx| b.view.read(cx).ui.mass_state.as_ref().is_some_and(|s| s["fields"].as_array().is_some_and(|f| f.iter().any(|f| f["label"] == "Volume" && f["text"].as_str().is_some_and(|t| !t.is_empty())))));
        state(cx)
    };
    let Reference::Body(p2) = b.reference(cx, E::Part("Part 2")).unwrap() else { panic!("Part 2") };
    // Part 1 has no material: greyed.
    let s = open(cx, "Part 1");
    assert_eq!(field(&s, "Center of mass")["state"], "disabled", "{s}");
    cx.update(|cx| b.view.update(cx, |v, cx| {
        let tab = v.tab;
        let body = p2.clone();
        v.doc.update(cx, |doc, cx| doc.edit_studio(tab, "Material", cx, |ps| ps.part_properties.entry(body).or_default().density = Some(8000.0)));
    }));
    let window = b.window;
    wait_until(cx, window, |cx| !b.view.read(cx).ui_busy());
    open(cx, "Part 2");
    let window = b.window;
    wait_until(cx, window, |cx| b.view.read(cx).ui.mass_state.as_ref().is_some_and(measured));
    let s = state(cx);
    assert_eq!(field(&s, "Center of mass")["state"], "ok", "{s}");
    assert_eq!(field(&s, "Mass moments of inertia")["state"], "ok", "{s}");
    let (lyy, lxy_before, lyx_before) = (cell(&s, "Lyy").unwrap(), cell(&s, "Lxy").unwrap(), cell(&s, "Lyx").unwrap());
    let click = |cx: &mut TestAppContext, sel: &str| {
        let at = gpui_kit::VisualTestContext::from_window(b.window, cx).debug_bounds(Box::leak(sel.to_string().into_boxed_str())).unwrap_or_else(|| panic!("no {sel}")).center();
        gpui_kit::VisualTestContext::from_window(b.window, cx).simulate_click(at, Modifiers::default());
        draw(cx, b.window);
    };
    let type_enter = |cx: &mut TestAppContext, text: &str| {
        gpui_kit::VisualTestContext::from_window(b.window, cx).simulate_input(text);
        gpui_kit::VisualTestContext::from_window(b.window, cx).simulate_keystrokes("enter");
        draw(cx, b.window);
    };
    // Ticked, the centre (70, 10, 7.5) is kept; X typed 10 mm moves the inertia 60 mm: Lyy and Lzz grow by m·0.06².
    assert_eq!(field(&s, "Center of mass")["checked"], false, "{s}");
    click(cx, "com-override");
    let s = state(cx);
    assert_eq!(field(&s, "Center of mass")["checked"], true, "{s}");
    assert_eq!(cell(&s, "Lyy"), Some(lyy), "ticking keeps the values: {s}");
    click(cx, "mass-override-cx");
    type_enter(cx, "10");
    let s = state(cx);
    let (m_kg, unit_per_kg) = cx.update(|cx| {
        let units = b.view.read(cx).doc.read(cx).session.doc.settings.units;
        (0.02 * 0.02 * 0.015 * 8000.0, crate::properties_dialog::mass_unit(&units).1)
    });
    let grow = m_kg * 0.06 * 0.06 * unit_per_kg * 1e6;
    assert!((cell(&s, "Lyy").unwrap() - (lyy + grow)).abs() < 1e-3 * grow, "Lyy {lyy} + {grow}: {s}");
    // Inertia ticked, Lxy typed 50: Lyx stays.
    click(cx, "moi-override");
    click(cx, "mass-override-Lxy");
    type_enter(cx, "50");
    let s = state(cx);
    assert_eq!(cell(&s, "Lxy"), Some(50.0), "{s}");
    assert_eq!(cell(&s, "Lyx"), Some(lyx_before), "Lxy was {lxy_before}: {s}");
    let features = cx.update(|cx| b.view.read(cx).studio(cx).unwrap().features.len());
    cx.update(|cx| b.view.update(cx, |v, cx| v.accept_mass_dialog(cx)));
    draw(cx, b.window);
    let (stored, after) = cx.update(|cx| {
        let v = b.view.read(cx);
        let ps = v.studio(cx).unwrap();
        (ps.part_properties.get(&p2).cloned().unwrap_or_default(), ps.features.len())
    });
    let com = stored.com_override.expect("the centre stored");
    assert!((com[0] - 0.01).abs() < 1e-9 && (com[1] - 0.01).abs() < 1e-9 && (com[2] - 0.0075).abs() < 1e-9, "{com:?}");
    let i = stored.inertia_override.expect("the inertia stored");
    assert!((i[3] * unit_per_kg * 1e6 - 50.0).abs() < 1e-6, "Lxy at index 3: {i:?}");
    assert_eq!(after, features, "no feature added");
}
