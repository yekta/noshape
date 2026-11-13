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
        let (window, view) = part_studio_from(cx, name, Some(document()));
        cx.update_window(window, |_, w, _| {
            w.set_scale_factor(scale);
            w.activate_window();
        })
        .unwrap();
        wait_until(cx, window, |cx| !view.read(cx).ui_busy() && view.read(cx).display.bodies.len() == 3);
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
