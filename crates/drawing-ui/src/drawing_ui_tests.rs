//! UI tests of the Drawing editor on GPUI's test platform (docs/TESTS.md "UI keystroke tests"): the real editor in
//! a test window over a document made through the agent API, the GPU a no-op stand-in.

use crate::doc_model::DocModel;
use crate::drawing_view::{DrawingEditor, MenuEntry};
use crate::gpu::Gpu;
use gpui_kit::{AnyWindowHandle, AppContext as _, Bounds, Entity, Modifiers, Point, TestAppContext, WindowBounds, WindowOptions, px, size};
use noshape_ops::drawing_export::DrawingExportPrefs;
use noshape_ops::{Ctx, registry};
use serde_json::{Value, json};

fn call(ctx: &mut Ctx, tool: &str, args: Value) -> Value {
    let op = registry::by_tool_name(tool).unwrap_or_else(|| panic!("no tool {tool}"));
    registry::run(ctx, op, &args).unwrap_or_else(|e| panic!("{tool}: {e:?}")).value
}

/// Agent API calls on a test drawing (its tab's name) before the editor opens.
type Setup = fn(&mut Ctx, &str);

thread_local! {
    /// The next test drawing gets the source's suggested views.
    static SUGGESTED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// The next test drawing gets a dimension on its first view's longest edge.
    static DIMENSIONED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// The next test drawing's block has a Ø6 hole through it at (20, 10).
    static HOLED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// The next test drawing is on ANSI_A_MM.dwt (ANSI dimensions) instead of ISO A3.
    static ANSI: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// The next test drawing is ui-parity's start of this name (tests/ui-parity/starts.json) instead.
    static START: std::cell::Cell<Option<&'static str>> = const { std::cell::Cell::new(None) };
    /// The next test drawing is given more through the agent API (its tab's name) before the editor opens.
    static SETUP: std::cell::Cell<Option<Setup>> = const { std::cell::Cell::new(None) };
}

/// A drawing of a block (Drawing 1) in a test window.
fn drawing(cx: &mut TestAppContext, name: &str) -> (AnyWindowHandle, Entity<DrawingEditor>) {
    cx.update(gpui_kit::init);
    crate::drawing_hooks::register_for_tests();
    let dir = std::env::temp_dir().join(format!("noshape-drawing-ui-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("drawing.noshape");
    for ext in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{ext}", path.display()));
    }
    if let Some(name) = START.take() {
        let starts: Value = serde_json::from_str(&std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/ui-parity/starts.json")).unwrap()).unwrap();
        noshape_ops::ui_parity::build_start(&mut Ctx::new(), &path, &starts[name]).unwrap();
    } else {
        let mut ctx = Ctx::new();
        call(&mut ctx, "document_create", json!({ "path": path.to_str().unwrap() }));
        let sk = call(&mut ctx, "sketch_create", json!({ "plane": "plane:top" }))["sketch"].clone();
        let d = call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "tool": "corner_rectangle", "points": [[0, 0], [40, 20]] }));
        let region = d["regions"][0]["id"].clone();
        call(&mut ctx, "feature_add", json!({ "type": "extrude", "params": { "entities": [region], "depth": "10 mm" } }));
        if HOLED.get() {
            let sk = call(&mut ctx, "sketch_create", json!({ "plane": "plane:top" }))["sketch"].clone();
            let d = call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "tool": "circle", "points": [[20, 10], [23, 10]] }));
            let region = d["regions"][0]["id"].clone();
            call(&mut ctx, "feature_add", json!({ "type": "extrude", "params": { "entities": [region], "endBound": "BLIND", "symmetric": true, "depth": "40 mm", "operationType": "REMOVE" } }));
        }
        let template = if ANSI.replace(false) { "ANSI_A_MM.dwt" } else { "ISO A3" };
        let d = call(&mut ctx, "drawing_create", json!({ "source": "Part Studio 1", "template": template, "suggested_views": SUGGESTED.get() }));
        if let Some(f) = SETUP.take() {
            f(&mut ctx, d["drawing"].as_str().unwrap_or_default());
        }
        if DIMENSIONED.get() {
            let tab = d["drawing"].clone();
            let g = call(&mut ctx, "drawing_get", json!({ "drawing": tab }));
            let num = |v: &Value| v.as_f64().unwrap();
            let len = |e: &Value| (num(&e["b"][0]) - num(&e["a"][0])).hypot(num(&e["b"][1]) - num(&e["a"][1]));
            let edges = g["views"][0]["edges"].as_array().unwrap();
            let long = edges.iter().filter(|e| e["type"] == "line").max_by(|a, b| len(a).total_cmp(&len(b))).unwrap();
            let mid = [(num(&long["a"][0]) + num(&long["b"][0])) / 2.0, (num(&long["a"][1]) + num(&long["b"][1])) / 2.0];
            call(&mut ctx, "drawing_dimension", json!({ "drawing": tab, "picks": [mid], "place": [mid[0], mid[1] + 20.0] }));
        }
    }
    let session = crate::session::Session::open(&path).unwrap();
    let tab = session.doc.tabs.iter().find(|t| matches!(t.content, noshape_model::TabContent::Drawing(_))).unwrap().id;
    let gpu = Gpu::headless().unwrap();
    let (window, editor) = cx.update(|cx| {
        let doc = cx.new(|_| DocModel::new(session));
        let bounds = Bounds { origin: Point::default(), size: size(px(1200.), px(1200.)) };
        let mut editor = None;
        // The app's window layers: dialogs and the lists opened from them.
        let (w, _) = crate::os_menu::open_window(WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() }, cx, |window, cx| {
            let e = cx.new(|cx| DrawingEditor::new(doc, tab, gpu, window, cx));
            editor = Some(e.clone());
            e
        })
        .unwrap();
        (w, editor.unwrap())
    });
    draw(cx, window);
    (window, editor)
}

fn draw(cx: &mut TestAppContext, window: AnyWindowHandle) {
    cx.update_window(window, |_, w, cx| w.draw(cx).clear(cx)).unwrap();
    cx.run_until_parked();
}

/// A click at the element painted with debug selector `id`.
fn press(cx: &mut TestAppContext, window: AnyWindowHandle, id: &str) {
    let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
    let b = vcx.debug_bounds(Box::leak(id.to_string().into_boxed_str())).unwrap_or_else(|| panic!("no {id}"));
    vcx.simulate_click(b.center(), Modifiers::default());
    draw(cx, window);
}

/// A list's option as the user picks it: open the list, click the option.
fn pick(cx: &mut TestAppContext, window: AnyWindowHandle, id: &str, label: &str) {
    press(cx, window, id);
    press(cx, window, &format!("menu:{label}"));
    assert!(!cx.update(|cx| crate::os_menu::is_open(cx)), "{label} picked in {id}");
}

/// Live (the client's saveSettings / getExportDrawingOptions, user setting exportDrawingOptions): Export Drawing opens
/// on the last export's format with that format's choices; Cancel keeps nothing.
#[gpui::test]
fn export_drawing_opens_with_the_last_exports_choices(cx: &mut TestAppContext) {
    let (window, editor) = drawing(cx, "export-remember");
    cx.update_window(window, |_, w, _| w.activate_window()).unwrap();
    // One run's test binaries share the settings directory (tools/tiers): the remembered options go with the test,
    // or later exports through the agent API would take them.
    struct Forget(std::path::PathBuf);
    impl Drop for Forget {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let _forget = Forget(noshape_scene::config_dir().join("export_drawing.json"));
    let _ = std::fs::remove_file(noshape_scene::config_dir().join("export_drawing.json"));
    let open = |cx: &mut TestAppContext| {
        cx.update_window(window, |_, w, cx| editor.update(cx, |e, cx| e.open_export_dialog(w, cx))).unwrap();
        draw(cx, window);
    };
    open(cx);
    pick(cx, window, "ex-format", "DWG");
    pick(cx, window, "ex-sheets", "Current sheet");
    pick(cx, window, "ex-text", "Explode all into polylines");
    pick(cx, window, "ex-options", "Store file in a new tab");
    press(cx, window, "modal-ok");
    let saved = DrawingExportPrefs::load().expect("saved on Export");
    assert_eq!((saved.format.as_str(), saved.options, saved.dwg.sheets, saved.dwg.text), ("DWG", 2, 1, 2), "{saved:?}");

    open(cx);
    pick(cx, window, "ex-format", "PDF");
    press(cx, window, "modal-cancel");
    assert_eq!(DrawingExportPrefs::load().as_ref(), Some(&saved), "Cancel saves nothing");

    // Reopened on DWG with its choices: exporting again keeps the same ones.
    open(cx);
    press(cx, window, "modal-ok");
    assert_eq!(DrawingExportPrefs::load().as_ref(), Some(&saved));
}

/// qa DRW-EXPORT-HIDE-INSPECTION-SIZE (live drawing/1358-dk-export-default): Export Drawing's labels 75 px apart, and
/// "Hide inspection items" Chrome's 13 px box at the fields' left edge, 19 px under the Options select.
#[gpui::test]
fn export_drawing_rows_as_live(cx: &mut TestAppContext) {
    let (window, editor) = drawing(cx, "export-rows");
    cx.update_window(window, |_, w, cx| editor.update(cx, |e, cx| e.open_export_dialog(w, cx))).unwrap();
    draw(cx, window);
    let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
    let mut at = |id: &'static str| vcx.debug_bounds(id).unwrap_or_else(|| panic!("no {id}"));
    let (version, overridden, options, hide) = (at("ex-version"), at("ex-overridden"), at("ex-options"), at("ex-hide-insp"));
    assert_eq!(overridden.top() - version.top(), px(75.), "75 px from one select to the next");
    let box_top = hide.top() + (hide.size.height - px(13.)) / 2.;
    assert!((box_top - options.bottom() - px(19.)).abs() < px(1.), "the box 19 px under Options: {options:?} {hide:?}");
    assert_eq!(hide.left(), options.left(), "at the fields' left edge");
}

/// Lead 11-06, the display cache: a drawing's sheet comes back from the cache as it was built; while it stands in
/// (the sources regenerating) the sheet shows it, a tool asked for waits with the message at once, and the complete
/// real sheet replaces it and starts the tool.
#[gpui::test]
fn a_cached_sheet_stands_in_until_the_real_one_is_complete(cx: &mut TestAppContext) {
    SUGGESTED.set(true);
    let (window, editor) = drawing(cx, "display-cache");
    // The source's regeneration and the views' geometry run on background threads.
    for _ in 0..1500 {
        if cx.update(|cx| editor.read(cx).real_sheet_in()) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
        cx.executor().advance_clock(std::time::Duration::from_millis(50));
        draw(cx, window);
    }
    let real = cx.update(|cx| editor.read(cx).built.clone()).expect("the sheet built");
    assert!(cx.update(|cx| editor.read(cx).real_sheet_in()) && real.placements.len() >= 3, "the suggested views computed");
    let dir = std::env::temp_dir().join(format!("noshape-drawing-cache-ui-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    crate::drawing_open::store_sheet(&dir, "k", &real).unwrap();
    let cached = crate::drawing_open::load_sheet(&dir, "k").expect("the cached sheet");
    let _ = std::fs::remove_dir_all(&dir);
    let lines = |b: &noshape_model::drawing::build::BuiltSheet| b.geometry.lines.iter().map(|l| (l.points.clone(), l.pick)).collect::<Vec<_>>();
    assert_eq!(lines(&cached), lines(&real), "cached as it was");
    // Reopened, the sources not in yet: the cached sheet is shown, view-only.
    let empty = noshape_model::drawing::build::BuiltSheet { geometry: Default::default(), ..cached.clone() };
    let sheet = cx.update(|cx| editor.read(cx).sheet);
    cx.update(|cx| editor.update(cx, |e, cx| {
        e.stand_in(sheet, cached);
        let shown = e.sheet_to_show(sheet, empty, false, cx);
        assert_eq!(lines(&shown), lines(&real), "the cached sheet stands in");
        assert!(e.cached_view() && e.render_regenerating().is_some(), "view-only, Regenerating… shown");
        e.set_tool(noshape_scene::drawing_tools::Tool::Centermark, cx);
        assert!(matches!(e.tool, noshape_scene::drawing_tools::Tool::Select), "no tool on the cached sheet");
        assert_eq!(e.toast.as_ref().map(|t| t.to_string()).as_deref(), Some(crate::display_cache::WAITING));
    }));
    // The real sheet is complete: it replaces the cached one and the tool starts.
    cx.update(|cx| editor.update(cx, |e, cx| {
        let shown = e.sheet_to_show(sheet, (*real).clone(), true, cx);
        assert_eq!(lines(&shown), lines(&real));
        assert!(!e.cached_view(), "the sheet's own again");
    }));
    draw(cx, window);
    assert!(cx.update(|cx| matches!(editor.read(cx).tool, noshape_scene::drawing_tools::Tool::Centermark)), "the tool started once the real sheet was in");
}


/// Live2 (drw-view-copy-paste-keys, drw-datum-copy-paste): a paste puts the copied items' drawn box with its lower-left
/// corner at the point (a view's frame: its geometry and a 2 mm margin), not their centre.
#[gpui::test]
fn a_paste_puts_the_copied_box_lower_left_at_the_point(cx: &mut TestAppContext) {
    use noshape_scene::drawing_tools::Sel;
    SUGGESTED.set(true);
    let (window, editor) = drawing(cx, "paste-corner");
    let settle = |cx: &mut TestAppContext| {
        for _ in 0..1500 {
            if cx.update(|cx| editor.read(cx).real_sheet_in()) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
            cx.executor().advance_clock(std::time::Duration::from_millis(50));
            draw(cx, window);
        }
    };
    settle(cx);
    let before: Vec<_> = cx.update(|cx| editor.read(cx).current_sheet().views.iter().map(|v| v.id).collect());
    let front = before[0];
    cx.update(|cx| editor.update(cx, |e, cx| {
        e.selection = vec![Sel::View(front)];
        e.copy_views(None, cx);
        e.cursor = [230.0, 40.0];
        e.paste_views(cx);
    }));
    let pasted = cx.update(|cx| editor.read(cx).current_sheet().views.iter().map(|v| v.id).find(|v| !before.contains(v))).expect("a view pasted");
    let mut corner = None;
    for _ in 0..1500 {
        corner = cx.update(|cx| editor.read(cx).copy_corner(&[pasted], &[], &[]));
        if corner.is_some() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
        cx.executor().advance_clock(std::time::Duration::from_millis(50));
        draw(cx, window);
    }
    let corner = corner.expect("the pasted view drawn");
    assert!((corner[0] - 230.0).abs() < 0.5 && (corner[1] - 40.0).abs() < 0.5, "the frame's lower-left at the point: {corner:?}");
}

/// Lead 11-12 (drawing speed): a zoom's lines (pen widths are in pixels) are remade off the UI thread, the frame
/// drawing the old ones meanwhile (the parts drawing's 14-27 ms per wheel step on the UI thread).
#[gpui::test]
fn a_zoom_remakes_the_lines_off_the_ui_thread(cx: &mut TestAppContext) {
    SUGGESTED.set(true);
    let (window, editor) = drawing(cx, "zoom-lines");
    for _ in 0..1500 {
        if cx.update(|cx| editor.read(cx).real_sheet_in()) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
        cx.executor().advance_clock(std::time::Duration::from_millis(50));
        draw(cx, window);
    }
    let (before, _) = cx.update(|cx| editor.read(cx).lines_state());
    assert!(before > 0.0, "the sheet's lines made");
    cx.update(|cx| editor.update(cx, |e, cx| e.zoom_by(1.5, [600.0, 600.0], cx)));
    cx.update_window(window, |_, w, cx| w.draw(cx).clear(cx)).unwrap();
    let (drawn, remaking) = cx.update(|cx| editor.read(cx).lines_state());
    assert!(remaking && drawn == before, "the frame drew the old lines, the new ones being made");
    cx.run_until_parked();
    let zoom = cx.update(|cx| editor.read(cx).zoom);
    let (after, remaking) = cx.update(|cx| editor.read(cx).lines_state());
    assert!(!remaking && after == zoom && zoom > before * 1.4, "the lines made for the new zoom: {before} -> {after} ({zoom})");
}

/// Live2 box-select-rule (11-16) on a block's view: left to right a box takes the view only with a whole edge of it
/// inside (part of its bottom edge or a box past its corner: nothing), right to left when it meets the view's frame
/// (~7 px out); a box adds to what was selected (F3).
#[gpui::test]
fn a_box_takes_a_view_by_a_whole_edge_or_by_crossing_its_frame(cx: &mut TestAppContext) {
    use noshape_scene::drawing_tools::Sel;
    SUGGESTED.set(true);
    let (window, editor) = drawing(cx, "box-views");
    real_sheet(cx, window, &editor);
    let (pls, zoom) = cx.update(|cx| {
        let e = editor.read(cx);
        (e.built.as_ref().map(|b| b.placements.clone()).unwrap_or_default(), e.zoom)
    });
    let (pl, other) = (pls[0], pls[1]);
    let px = |n: f64| n / zoom;
    let w = pl.max[0] - pl.min[0];
    // (a) the middle third of the bottom edge, (b) the whole right edge, (d) past the corner, 4 to 20 px off.
    let part_of_bottom = ([pl.min[0] + w / 3.0, pl.min[1] - px(8.0)], [pl.max[0] - w / 3.0, pl.min[1] + px(3.0)]);
    let right_edge = ([pl.max[0] - px(3.0), pl.min[1] - px(5.0)], [pl.max[0] + px(10.0), pl.max[1] + px(5.0)]);
    let past_corner = ([pl.max[0] + px(4.0), pl.max[1] + px(4.0)], [pl.max[0] + px(20.0), pl.max[1] + px(20.0)]);
    for (name, (lo, hi), window_takes, crossing_takes) in [("part of the bottom edge", part_of_bottom, false, true), ("the whole right edge", right_edge, true, true), ("past the corner", past_corner, false, true)] {
        for (from, to, want) in [([lo[0], lo[1]], [hi[0], hi[1]], window_takes), ([hi[0], hi[1]], [lo[0], lo[1]], crossing_takes)] {
            let taken = cx.update(|cx| editor.update(cx, |e, cx| {
                e.selection = vec![Sel::View(other.id)];
                e.select_in_box(from, to, true, cx);
                (e.selection.contains(&Sel::View(pl.id)), e.selection.contains(&Sel::View(other.id)))
            }));
            assert_eq!(taken, (want, true), "{name}, {}: the view taken {want}, the other still selected", if from[0] < to[0] { "left to right" } else { "right to left" });
        }
    }
}

/// Live2 box-select-rule (11-16): left to right a box takes a dimension holding its whole text, an arrowhead or an
/// attachment point, not an extension line's foot alone; right to left it takes it when it touches it.
#[gpui::test]
fn a_box_takes_a_dimension_by_its_text_an_arrowhead_or_an_attachment_point(cx: &mut TestAppContext) {
    use noshape_scene::drawing_tools::Sel;
    SUGGESTED.set(true);
    DIMENSIONED.set(true);
    let (window, editor) = drawing(cx, "box-dimension");
    DIMENSIONED.set(false);
    real_sheet(cx, window, &editor);
    let id = cx.update(|cx| editor.read(cx).current_sheet().annotations.iter().find(|a| a.kind.is_dimension()).map(|a| a.id)).expect("a dimension");
    let grow = |pts: &[[f64; 2]], m: f64| {
        let lo = pts.iter().fold([f64::MAX; 2], |a, p| [a[0].min(p[0]), a[1].min(p[1])]);
        let hi = pts.iter().fold([f64::MIN; 2], |a, p| [a[0].max(p[0]), a[1].max(p[1])]);
        ([lo[0] - m, lo[1] - m], [hi[0] + m, hi[1] + m])
    };
    let (text, arrow, attach, foot) = cx.update(|cx| {
        let e = editor.read(cx);
        let (b, picks) = (e.built.clone().expect("built"), e.picks_of(Sel::Annotation(id)));
        let t = b.geometry.texts.iter().find(|t| picks.contains(&t.pick)).expect("the dimension's text");
        let text = grow(&noshape_model::drawing::font::text_box(t), 0.5);
        let fills = b.geometry.annotation_fills.iter().find(|(a, _)| *a == id).map(|(_, r)| r.clone()).expect("its arrowheads");
        let arrow = grow(&b.geometry.fills[fills.start].points, 0.3);
        let a = e.pk().resolved_annotation(id).expect("resolved");
        let p = e.pk().attach_points(&a)[0].1;
        let attach = grow(&[p], 0.5);
        // The extension line from that attachment point: a stretch of it from 5 % to 45 % of its length.
        let ext = b.geometry.lines.iter().filter(|l| picks.contains(&l.pick) && l.points.len() == 2).min_by(|l, m| {
            let d = |l: &&noshape_model::drawing::sheet::Polyline2| (l.points[0][0] - p[0]).hypot(l.points[0][1] - p[1]).min((l.points[1][0] - p[0]).hypot(l.points[1][1] - p[1]));
            d(l).total_cmp(&d(m))
        }).expect("an extension line");
        let (near, far) = if (ext.points[0][0] - p[0]).hypot(ext.points[0][1] - p[1]) < (ext.points[1][0] - p[0]).hypot(ext.points[1][1] - p[1]) { (ext.points[0], ext.points[1]) } else { (ext.points[1], ext.points[0]) };
        let at = |t: f64| [near[0] + (far[0] - near[0]) * t, near[1] + (far[1] - near[1]) * t];
        let foot = grow(&[at(0.05), at(0.45)], 0.5);
        (text, arrow, attach, foot)
    });
    for (name, (lo, hi), window_takes) in [("its whole text", text, true), ("an arrowhead", arrow, true), ("an attachment point", attach, true), ("an extension line's foot", foot, false)] {
        for (from, to, want) in [([lo[0], lo[1]], [hi[0], hi[1]], window_takes), ([hi[0], hi[1]], [lo[0], lo[1]], true)] {
            let taken = cx.update(|cx| editor.update(cx, |e, cx| {
                e.selection.clear();
                e.select_in_box(from, to, true, cx);
                e.selection.contains(&Sel::Annotation(id))
            }));
            assert_eq!(taken, want, "{name}, {}", if from[0] < to[0] { "left to right" } else { "right to left" });
        }
    }
}

/// Waits for the drawing's real sheet (the cached stand-in replaced).
fn real_sheet(cx: &mut TestAppContext, window: AnyWindowHandle, editor: &Entity<DrawingEditor>) {
    for _ in 0..1500 {
        if cx.update(|cx| editor.read(cx).real_sheet_in()) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
        cx.executor().advance_clock(std::time::Duration::from_millis(50));
        draw(cx, window);
    }
}

/// Live2 feel/drawing.json D3: a view's vertex snaps within 7 px on screen, not from 9, at any zoom (ours 8).
#[gpui::test]
fn a_views_vertex_snaps_within_8_px_on_screen(cx: &mut TestAppContext) {
    SUGGESTED.set(true);
    let (window, editor) = drawing(cx, "snap-reach");
    real_sheet(cx, window, &editor);
    for zoom_by in [1.0, 2.5] {
        let (corner, out, zoom) = cx.update(|cx| editor.update(cx, |e, cx| {
            e.zoom_by(zoom_by, [600.0, 600.0], cx);
            // A view's lower-left vertex, and the way out of the view from it.
            let pl = e.built.as_ref().expect("built").placements.first().cloned().expect("a view");
            let g = e.geoms.get(&pl.id).cloned().expect("geometry");
            let corner = g.snaps.iter().map(|s| pl.to_sheet(s.p)).min_by(|a, b| (a[0] + a[1]).total_cmp(&(b[0] + b[1]))).expect("a vertex");
            (corner, [-std::f64::consts::FRAC_1_SQRT_2, -std::f64::consts::FRAC_1_SQRT_2], e.zoom)
        }));
        let at = |px: f64| [corner[0] + out[0] * px / zoom, corner[1] + out[1] * px / zoom];
        let snapped = |cx: &mut TestAppContext, px: f64| cx.update(|cx| editor.read(cx).snap_at(at(px)).is_some());
        assert!(snapped(cx, 7.0), "7 px off: snaps (zoom {zoom})");
        assert!(!snapped(cx, 9.0), "9 px off: free (zoom {zoom})");
    }
}

/// Live2 (drw-dimension-tolerance-symmetrical; REQUESTS 11-14 00:43): Enter in the dimension palette makes the edit
/// and leaves the palette open; a click on the empty sheet closes it, the edit kept; Esc (`cancel`) closes it too.
#[gpui::test]
fn the_dimension_palette_stays_open_on_enter_and_closes_on_a_click_away(cx: &mut TestAppContext) {
    SUGGESTED.set(true);
    DIMENSIONED.set(true);
    let (window, editor) = drawing(cx, "palette-open");
    DIMENSIONED.set(false);
    real_sheet(cx, window, &editor);
    let id = cx.update(|cx| editor.read(cx).current_sheet().annotations.iter().find(|a| a.kind.is_dimension()).map(|a| a.id)).expect("a dimension");
    cx.update_window(window, |_, w, cx| editor.update(cx, |e, cx| e.open_dim_palette(id, w, cx))).unwrap();
    draw(cx, window);
    let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
    vcx.simulate_input("R");
    vcx.simulate_keystrokes("enter");
    draw(cx, window);
    let (open, prefix) = cx.update(|cx| {
        let e = editor.read(cx);
        (e.palette.is_some(), e.current_sheet().annotations.iter().find(|a| a.id == id).map(|a| a.format.prefix.clone()))
    });
    assert!(open, "Enter leaves the palette open");
    // Live2 dim-palette-halo: its left edge just right of the dimension's text, its main row (42 px down) starting at
    // the text's centre line.
    let (at, x_px, y_px) = cx.update(|cx| {
        let e = editor.read(cx);
        let (b, picks, s) = (e.built.clone().unwrap(), e.picks_of(crate::drawing_view::Sel::Annotation(id)), e.size_px());
        let t = b.geometry.texts.iter().find(|t| picks.contains(&t.pick)).expect("the dimension's text").clone();
        (e.palette_at(id), ((t.pos[0] - e.center[0]) * e.zoom + s[0] / 2.0) as f32, ((e.center[1] - t.pos[1]) * e.zoom + s[1] / 2.0) as f32)
    });
    assert!(at[0] > x_px, "right of the text: {at:?} vs x {x_px}");
    assert!((at[1] + 42. - y_px).abs() < 12., "main row at the text: top {} vs text y {y_px}", at[1]);
    assert_eq!(prefix.as_deref(), Some("R"), "Enter made the edit");
    let empty = empty_sheet_point(cx, window, &editor);
    click_sheet(cx, window, &editor, empty);
    let (open, prefix) = cx.update(|cx| {
        let e = editor.read(cx);
        (e.palette.is_some(), e.current_sheet().annotations.iter().find(|a| a.id == id).map(|a| a.format.prefix.clone()))
    });
    assert!(!open, "a click on the empty sheet closes it");
    assert_eq!(prefix.as_deref(), Some("R"), "the edit made with Enter stays");
    cx.update_window(window, |_, w, cx| editor.update(cx, |e, cx| e.open_dim_palette(id, w, cx))).unwrap();
    draw(cx, window);
    cx.update(|cx| editor.update(cx, |e, cx| e.cancel(cx)));
    draw(cx, window);
    assert!(cx.update(|cx| editor.read(cx).palette.is_none()), "Esc closes it");
}

/// Live2 drw-dimension-dblclick-precision: the palette's untyped value follows a precision picked (1.575 -> 1.6;
/// here 40,00 -> 40,0000),
/// and isn't made an override by it.
#[gpui::test]
fn the_dimension_palettes_value_follows_a_precision_picked(cx: &mut TestAppContext) {
    SUGGESTED.set(true);
    DIMENSIONED.set(true);
    let (window, editor) = drawing(cx, "palette-precision");
    DIMENSIONED.set(false);
    real_sheet(cx, window, &editor);
    let id = cx.update(|cx| editor.read(cx).current_sheet().annotations.iter().find(|a| matches!(a.kind, noshape_model::drawing::AnnotationKind::LinearDimension { .. })).map(|a| a.id)).expect("a linear dimension");
    // (Trailing zeros shown: a whole value reads its precision.)
    cx.update(|cx| editor.update(cx, |e, _| e.drawing.props.trailing_zeros = true));
    cx.update_window(window, |_, w, cx| editor.update(cx, |e, cx| e.open_dim_palette(id, w, cx))).unwrap();
    draw(cx, window);
    let shown = |cx: &mut TestAppContext| cx.update(|cx| editor.read(cx).palette.as_ref().map(|d| d.value.read(cx).value().to_string())).expect("the palette");
    let before = shown(cx);
    cx.update(|cx| editor.update(cx, |e, cx| e.edit_palette(|d| d.cur.format.precision = Some(4), cx)));
    draw(cx, window);
    let after = shown(cx);
    assert!(after != before && after.rsplit([',', '.']).next().map(str::len) == Some(4), "{before} at 0.0001: {after}");
    let over = cx.update(|cx| editor.read(cx).palette.as_ref().map(|d| d.cur.kind.clone()));
    assert!(matches!(over, Some(noshape_model::drawing::AnnotationKind::LinearDimension { override_text: None, .. })), "no override: {over:?}");
}

/// Live2 dim-palette-halo: a transparent halo (40 px left, 20 right, 30 above and below) around the dimension palette
/// closes a palette not yet clicked into, and keeps it open once it has been (a click into Prefix, a Precision pick).
#[gpui_kit::test]
fn the_dimension_palettes_halo_keeps_it_open_once_clicked_into(cx: &mut TestAppContext) {
    SUGGESTED.set(true);
    DIMENSIONED.set(true);
    let (window, editor) = drawing(cx, "palette-halo");
    DIMENSIONED.set(false);
    real_sheet(cx, window, &editor);
    let id = cx.update(|cx| editor.read(cx).current_sheet().annotations.iter().find(|a| a.kind.is_dimension()).map(|a| a.id)).expect("a dimension");
    let open = |cx: &mut TestAppContext| {
        cx.update_window(window, |_, w, cx| editor.update(cx, |e, cx| e.open_dim_palette(id, w, cx))).unwrap();
        draw(cx, window);
    };
    let click = |cx: &mut TestAppContext, dx: f32, dy: f32| {
        let p = cx.update(|cx| {
            let e = editor.read(cx);
            let at = e.palette_at(id);
            e.bounds.origin + Point::new(px(at[0] + dx), px(at[1] + dy))
        });
        gpui_kit::VisualTestContext::from_window(window, cx).simulate_click(p, Modifiers::default());
        draw(cx, window);
        cx.update(|cx| editor.read(cx).palette.is_some())
    };
    open(cx);
    assert!(!click(cx, -20., 60.), "a click in the halo closes a palette not clicked into");
    open(cx);
    assert!(click(cx, 40., 52.), "a click into Prefix keeps it open");
    assert!(click(cx, -20., 60.), "then a click in the halo keeps it open");
    assert!(click(cx, 200., -15.), "twice, above it too");
    assert!(!click(cx, 200., -60.), "a click past the halo closes it");
}

/// A sheet point (mm) with nothing under the pointer: no view, frame or annotation.
fn empty_sheet_point(cx: &mut TestAppContext, window: AnyWindowHandle, editor: &Entity<DrawingEditor>) -> [f64; 2] {
    use gpui_kit::MouseMoveEvent;
    let (w, h) = cx.update(|cx| editor.read(cx).current_sheet().size.size_mm());
    let candidates = (1..20).flat_map(|i| (1..20).map(move |j| [w * f64::from(i) / 20.0, h * f64::from(j) / 20.0]));
    for at in candidates {
        let empty = cx
            .update_window(window, |_, win, cx| editor.update(cx, |e, cx| {
                let l = e.sheet_to_local(at);
                let pos = e.bounds.origin + Point::new(px(l[0] as f32), px(l[1] as f32));
                e.on_mouse_move(&MouseMoveEvent { position: pos, pressed_button: None, modifiers: Modifiers::default() }, win, cx);
                e.hover.is_none()
            }))
            .unwrap();
        if empty {
            return at;
        }
    }
    panic!("no empty sheet point");
}

/// A press and release of the left button at sheet point `at` (mm), the pointer moved there first.
fn click_sheet(cx: &mut TestAppContext, window: AnyWindowHandle, editor: &Entity<DrawingEditor>, at: [f64; 2]) {
    use gpui_kit::{MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent};
    let at = cx.update(|cx| {
        let e = editor.read(cx);
        let l = e.sheet_to_local(at);
        e.bounds.origin + Point::new(px(l[0] as f32), px(l[1] as f32))
    });
    cx.update_window(window, |_, w, cx| editor.update(cx, |e, cx| {
        e.on_mouse_move(&MouseMoveEvent { position: at, pressed_button: None, modifiers: Modifiers::default() }, w, cx);
        e.on_mouse_down(&MouseDownEvent { button: MouseButton::Left, position: at, modifiers: Modifiers::default(), click_count: 1, first_mouse: false }, w, cx);
        e.on_mouse_up(&MouseUpEvent { button: MouseButton::Left, position: at, modifiers: Modifiers::default(), click_count: 1 }, w, cx);
    }))
    .unwrap();
    draw(cx, window);
}

/// Live2 drw-feel-q (a): a press on the empty sheet shows its box only past 7 px from the press: none at 7 px along
/// an axis, a box at (5, 5) px (7.07).
#[gpui::test]
fn a_box_on_the_empty_sheet_starts_past_7_px(cx: &mut TestAppContext) {
    use gpui_kit::{MouseButton, MouseDownEvent, MouseMoveEvent};
    SUGGESTED.set(true);
    let (window, editor) = drawing(cx, "box-start");
    real_sheet(cx, window, &editor);
    let empty = empty_sheet_point(cx, window, &editor);
    let at = cx.update(|cx| {
        let e = editor.read(cx);
        let l = e.sheet_to_local(empty);
        e.bounds.origin + Point::new(px(l[0] as f32), px(l[1] as f32))
    });
    let boxing = |cx: &mut TestAppContext, dx: f32, dy: f32| {
        cx.update_window(window, |_, w, cx| editor.update(cx, |e, cx| {
            let position = at + Point::new(px(dx), px(dy));
            e.on_mouse_move(&MouseMoveEvent { position, pressed_button: Some(MouseButton::Left), modifiers: Modifiers::default() }, w, cx);
            matches!(e.drag, crate::drawing_view::Drag::SelectBox { .. })
        }))
        .unwrap()
    };
    cx.update_window(window, |_, w, cx| editor.update(cx, |e, cx| {
        e.on_mouse_down(&MouseDownEvent { button: MouseButton::Left, position: at, modifiers: Modifiers::default(), click_count: 1, first_mouse: false }, w, cx);
    }))
    .unwrap();
    assert!(!boxing(cx, 4.0, 4.0), "5.7 px: no box");
    assert!(!boxing(cx, 7.0, 0.0), "7 px: no box");
    assert!(boxing(cx, 5.0, 5.0), "7.07 px: a box");
}

/// Live2 drw-view-copy-paste-keys: after Ctrl+C the next click on the empty sheet keeps the selection (it marks where
/// Ctrl+V pastes); a click there after that clears it, as always.
#[gpui::test]
fn the_first_click_on_the_empty_sheet_after_a_copy_keeps_the_selection(cx: &mut TestAppContext) {
    use noshape_scene::drawing_tools::Sel;
    SUGGESTED.set(true);
    let (window, editor) = drawing(cx, "copy-armed");
    real_sheet(cx, window, &editor);
    let view = cx.update(|cx| editor.read(cx).current_sheet().views.first().map(|v| v.id)).expect("a view");
    cx.update(|cx| editor.update(cx, |e, _| e.selection = vec![Sel::View(view)]));
    cx.update_window(window, |_, w, cx| {
        let focus = editor.read(cx).focus.clone();
        w.focus(&focus, cx);
        w.dispatch_action(Box::new(crate::actions::Copy), cx);
    })
    .unwrap();
    draw(cx, window);
    let selected = |cx: &mut TestAppContext| cx.update(|cx| editor.read(cx).selection.clone());
    let empty = empty_sheet_point(cx, window, &editor);
    click_sheet(cx, window, &editor, empty);
    assert_eq!(selected(cx), vec![Sel::View(view)], "the click after Ctrl+C keeps the view");
    click_sheet(cx, window, &editor, empty);
    assert!(selected(cx).is_empty(), "the next one clears");
}

/// Live2 feedback.json (documentLoad), the drawing as a Part Studio: no loading state in the first moment of a quick
/// open, nor once a view is drawn.
#[gpui::test]
fn an_opening_drawing_says_what_it_is_loading_until_a_view_is_drawn(cx: &mut TestAppContext) {
    SUGGESTED.set(true);
    let (window, editor) = drawing(cx, "loading");
    real_sheet(cx, window, &editor);
    assert_eq!(cx.update(|cx| editor.read(cx).loading_message()), None, "views drawn: nothing");
    cx.update(|cx| editor.update(cx, |e, _| e.opening_for_test()));
    assert_eq!(cx.update(|cx| editor.read(cx).loading_message()), Some("Loading graphics…"), "a second in, nothing drawn, sources in");
}

/// Live2 feel/drawing.json (tables_live2_1013 centerlines): with Centermark, a circle lights from 6 px outside it and
/// shows the preview cross; not from further; a circle that has a centermark gets nothing more.
#[gpui::test]
fn centermark_lights_a_circle_from_6_px_and_skips_a_marked_one(cx: &mut TestAppContext) {
    use noshape_model::drawing::AnnotationKind;
    use noshape_scene::drawing_tools::{Pick, Tool};
    SUGGESTED.set(true);
    HOLED.set(true);
    let (window, editor) = drawing(cx, "centermark-reach");
    HOLED.set(false);
    real_sheet(cx, window, &editor);
    // The hole's circle where a view shows it face on, in sheet mm.
    let (center, radius, zoom) = cx.update(|cx| {
        let e = editor.read(cx);
        let b = e.built.as_ref().expect("built");
        b.placements
            .iter()
            .find_map(|pl| {
                let g = e.geoms.get(&pl.id)?;
                let (c, r, _) = g.circles().into_iter().next()?;
                Some((pl.to_sheet(c), pl.len_to_sheet(r), e.zoom))
            })
            .expect("a view showing the hole")
    });
    cx.update(|cx| editor.update(cx, |e, cx| e.set_tool(Tool::Centermark, cx)));
    draw(cx, window);
    let at = |cx: &mut TestAppContext, out_px: f64| {
        cx.update(|cx| {
            editor.update(cx, |e, cx| {
                e.cursor = [center[0] + radius + out_px / zoom, center[1]];
                e.update_hover(cx);
                (matches!(e.circle_pick(e.cursor), Some(Pick::Circle { .. })), e.pending_annotation().is_some_and(|a| matches!(a.kind, AnnotationKind::Centermark { .. })))
            })
        })
    };
    assert_eq!(at(cx, 5.5), (true, true), "5.5 px out: the circle and the preview cross");
    assert!(!at(cx, 7.5).1, "7.5 px out: no preview");
    let marks = |cx: &mut TestAppContext| cx.update(|cx| editor.read(cx).current_sheet().annotations.iter().filter(|a| matches!(a.kind, AnnotationKind::Centermark { .. })).count());
    at(cx, 0.0);
    let before = marks(cx);
    cx.update(|cx| editor.update(cx, |e, cx| e.centermark_click(cx)));
    assert_eq!(marks(cx), before + 1, "a centermark on the circle");
    // The tool again: the marked circle gets nothing more.
    cx.update(|cx| editor.update(cx, |e, cx| {
        e.set_tool(Tool::Select, cx);
        e.set_tool(Tool::Centermark, cx);
    }));
    draw(cx, window);
    assert!(!at(cx, 0.0).1, "no preview on a marked circle");
    cx.update(|cx| editor.update(cx, |e, cx| e.centermark_click(cx)));
    assert_eq!(marks(cx), before + 1, "nothing added");
}

/// Live2 feel/drawing.json (tables_live2_1013 noteEditor): in a note, Tab inserts a tab.
#[gpui::test]
fn tab_in_a_note_inserts_a_tab(cx: &mut TestAppContext) {
    let (window, editor) = drawing(cx, "note-tab");
    real_sheet(cx, window, &editor);
    cx.update_window(window, |_, w, cx| editor.update(cx, |e, cx| e.open_note_panel(None, [100.0, 100.0], w, cx))).unwrap();
    draw(cx, window);
    let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
    vcx.simulate_keystrokes("a tab b");
    draw(cx, window);
    let text = cx.update(|cx| match &editor.read(cx).panel {
        Some(crate::drawing_view::Panel::Note { input, .. }) => Some(input.read(cx).value().to_string()),
        _ => None,
    });
    assert_eq!(text.as_deref(), Some("a\tb"));
}

/// Live 726: a double-clicked title field's Note editor shows the field's alignment (a centred one Center / Middle).
#[gpui::test]
fn a_title_field_edits_with_its_alignment(cx: &mut TestAppContext) {
    use noshape_model::drawing::{build::PickTarget, note::{NoteAlign, NoteVAlign}, sheet::Anchor};
    let (window, editor) = drawing(cx, "title-field-align");
    real_sheet(cx, window, &editor);
    let key = cx.update(|cx| {
        let e = editor.read(cx);
        let b = e.built.as_ref().expect("built");
        b.title_cells.iter().find_map(|(p, _)| match b.picks.get(*p as usize) {
            Some(PickTarget::TitleField(k)) if b.geometry.texts.iter().any(|t| t.pick == *p && t.anchor == Anchor::Center) => Some(k.clone()),
            _ => None,
        })
    });
    let key = key.expect("a centred title field");
    cx.update_window(window, |_, w, cx| editor.update(cx, |e, cx| e.open_title_field(key.clone(), w, cx))).unwrap();
    let style = cx.update(|cx| match &editor.read(cx).panel {
        Some(crate::drawing_view::Panel::Note { style, field: Some(_), .. }) => Some((style.align, style.valign)),
        _ => None,
    });
    assert_eq!(style, Some((NoteAlign::Center, NoteVAlign::Middle)), "{key}");
}

/// Live's note popups (dom drawing-note-popups, nt-pop-drawprop): Insert drawing property's 22 rows in a list 206 px
/// wide and at most 341 px tall (scrolled), the symbols in one 47 px wide, not the "+" menu's 253 px.
#[gpui::test]
fn the_note_insert_lists_are_lives_size(cx: &mut TestAppContext) {
    let (window, editor) = drawing(cx, "note-insert-lists");
    real_sheet(cx, window, &editor);
    cx.update_window(window, |_, w, cx| editor.update(cx, |e, cx| e.open_note_panel(None, [100.0, 100.0], w, cx))).unwrap();
    draw(cx, window);
    for (button, list, rows, w) in [("note-dprop", "note-dprop-dd", 22, 206.0), ("note-symbol", "note-symbol-dd", 15, 47.0)] {
        press(cx, window, button);
        let (labels, b) = cx.update(|cx| (noshape_app_core::os_ui::open_dropdown_labels(list, cx), noshape_app_core::os_ui::open_dropdown_bounds(list, cx)));
        assert_eq!(labels.map(|l| l.len()), Some(rows), "{list}'s rows");
        // (The list's bounds are inside its 1 px border.)
        let b = b.expect("open");
        let (outer_w, outer_h) = (f32::from(b.size.width) + 2.0, f32::from(b.size.height) + 2.0);
        assert!(outer_h <= 341.5, "{list}: {outer_h} px tall");
        assert!((w..w + 30.0).contains(&outer_w), "{list}: {outer_w} px wide");
        gpui_kit::VisualTestContext::from_window(window, cx).simulate_keystrokes("escape");
        draw(cx, window);
    }
}

/// Live2 tables_live2_1013 (dw4-resized): dragging a BOM's inner border moves that border only (the left column
/// widens, the right one narrows, the total stays); a general table's grows the column on the fixed corner's side
/// (live 1613 (b)).
#[::core::prelude::v1::test]
fn a_bom_border_drag_moves_only_that_border() {
    use crate::drawing_extra::divider_widths;
    let w = [20.0, 30.0, 40.0, 50.0];
    assert_eq!(divider_widths(&w, 3, 10.0, true, true), [20.0, 30.0, 50.0, 40.0]);
    assert_eq!(divider_widths(&w, 3, 60.0, true, true), [20.0, 30.0, 87.0, 3.0], "the right column keeps its minimum");
    assert_eq!(divider_widths(&w, 1, -30.0, true, true), [3.0, 47.0, 40.0, 50.0], "and the left one");
    assert_eq!(divider_widths(&w, 3, 10.0, true, false), [20.0, 30.0, 40.0, 40.0], "general, fixed right: the right column");
    assert_eq!(divider_widths(&w, 3, 10.0, false, false), [20.0, 30.0, 50.0, 50.0], "general, fixed left: the left column");
    assert_eq!(divider_widths(&w, 4, 10.0, true, true), [20.0, 30.0, 40.0, 60.0], "a BOM's outer border as before");
}

/// ui-parity drw-centermark: Centermark, then a click on the hole's circle (at 45°, through the mouse) makes a
/// centermark there.
#[gpui::test]
fn a_centermark_click_on_a_circle_marks_it(cx: &mut TestAppContext) {
    use noshape_model::drawing::AnnotationKind;
    use noshape_scene::drawing_tools::Tool;
    START.set(Some("hole-plate-drawing"));
    let (window, editor) = drawing(cx, "centermark-click");
    real_sheet(cx, window, &editor);
    let (center, radius) = cx.update(|cx| {
        let e = editor.read(cx);
        let b = e.built.as_ref().expect("built");
        b.placements.iter().find_map(|pl| {
            let g = e.geoms.get(&pl.id)?;
            let (c, r, _) = g.circles().into_iter().next()?;
            Some((pl.to_sheet(c), pl.len_to_sheet(r)))
        })
        .expect("a view showing the hole")
    });
    cx.update(|cx| editor.update(cx, |e, cx| e.set_tool(Tool::Centermark, cx)));
    draw(cx, window);
    let k = std::f64::consts::FRAC_1_SQRT_2;
    let on = [center[0] + radius * k, center[1] + radius * k];
    // The pointer arrives first and a frame draws (the preview cross), then the click, as in the app.
    let at = cx.update(|cx| {
        let e = editor.read(cx);
        let l = e.sheet_to_local(on);
        e.bounds.origin + Point::new(px(l[0] as f32), px(l[1] as f32))
    });
    cx.update_window(window, |_, w, cx| editor.update(cx, |e, cx| e.on_mouse_move(&gpui_kit::MouseMoveEvent { position: at, pressed_button: None, modifiers: Modifiers::default() }, w, cx))).unwrap();
    draw(cx, window);
    cx.update(|cx| editor.update(cx, |e, cx| e.update_preview(cx)));
    let previewed = cx.update(|cx| editor.read(cx).current_sheet().annotations.iter().any(|a| matches!(a.kind, AnnotationKind::Centermark { .. })));
    assert!(previewed, "the hovered circle's cross is in the displayed sheet");
    click_sheet(cx, window, &editor, on);
    let marks = cx.update(|cx| editor.read(cx).current_sheet().annotations.iter().filter(|a| matches!(a.kind, AnnotationKind::Centermark { .. })).count());
    assert_eq!(marks, 1, "a centermark on the circle");
}

/// Live drawing/289 and live2 (REQUESTS 11-14 00:48): a table's Bring to front is greyed only with no other
/// annotation-layer object on the sheet; views don't count.
#[gpui::test]
fn a_tables_bring_to_front_greys_only_without_other_annotations(cx: &mut TestAppContext) {
    let greyed = |dimensioned: bool, cx: &mut TestAppContext| {
        SUGGESTED.set(true);
        DIMENSIONED.set(dimensioned);
        let (window, editor) = drawing(cx, if dimensioned { "front-dim" } else { "front-views" });
        DIMENSIONED.set(false);
        real_sheet(cx, window, &editor);
        cx.update(|cx| {
            editor.update(cx, |e, cx| {
                let id = e.current_sheet().tables.iter().map(|t| t.id).max().unwrap_or(0) + 1;
                e.edit_sheet("Table", cx, |s| {
                    s.tables.push(noshape_model::drawing::Table::new(id, noshape_model::drawing::TableKind::General { cells: vec![vec!["a".into()]], title: None, header: false }, [200.0, 200.0]));
                });
                e.data_table_entries(id, cx).iter().find_map(|m| match m {
                    MenuEntry::Item { label, disabled, .. } if label.as_ref() == "Bring to front" => Some(*disabled),
                    _ => None,
                })
            })
        })
    };
    assert_eq!(greyed(false, cx), Some(true), "views only: greyed");
    assert_eq!(greyed(true, cx), Some(false), "with a dimension: enabled");
}

/// Live2's drw-revision-* (dx 11-14): a revision table's whole-table menu is its own (Revision table properties…,
/// Paste as table, Move to ▸, Bring to front, Send to back | Clear selection, Zoom to fit, Delete), none of a general
/// table's cell items; a selected cell's is Size ▸, Resize…, Copy.
#[gpui::test]
fn a_revision_tables_menus_are_lives(cx: &mut TestAppContext) {
    let (window, editor) = drawing(cx, "revision-menus");
    let id = cx.update(|cx| {
        editor.update(cx, |e, cx| {
            let id = e.current_sheet().next_table_id();
            let table = noshape_scene::drawing_tools::revision_table(id, &Default::default(), [300.0, 250.0]);
            e.edit_sheet("Revision table", cx, |s| s.tables.push(table));
            id
        })
    });
    draw(cx, window);
    let labels = |entries: Vec<MenuEntry>| -> Vec<String> {
        entries
            .into_iter()
            .filter_map(|m| match m {
                MenuEntry::Item { label, .. } | MenuEntry::Submenu(label, _) => Some(label.to_string()),
                MenuEntry::Separator => None,
            })
            .collect()
    };
    let whole = cx.update(|cx| labels(editor.read(cx).revision_table_entries(id, None)));
    assert_eq!(whole, ["Revision table properties...", "Paste as table", "Move to", "Bring to front", "Send to back", "Clear selection", "Zoom to fit", "Delete"]);
    let cells = cx.update(|cx| {
        let e = editor.read(cx);
        let t = e.current_sheet().table(id).unwrap().clone();
        labels(e.revision_cell_entries(&t, crate::drawing_cells::CellSel { table: id, a: (0, 0), b: (0, 0) }))
    });
    assert_eq!(cells, ["Size", "Resize...", "Copy"]);
}

/// Live2 drw-geometric-tolerance-dialog: one Esc, the Tolerance field focused, closes the Geometric tolerance
/// panel and ends the tool.
#[gpui::test]
fn esc_closes_the_geometric_tolerance_panel_and_its_tool(cx: &mut TestAppContext) {
    use noshape_scene::drawing_tools::Tool;
    let (window, editor) = drawing(cx, "gdt-esc");
    cx.update(|cx| editor.update(cx, |e, cx| e.set_tool(Tool::Gdt, cx)));
    draw(cx, window);
    draw(cx, window);
    assert!(cx.update(|cx| editor.read(cx).gdt_dlg.is_some()), "the panel opens with the tool");
    cx.update(|cx| cx.bind_keys(noshape_app_core::actions::default_key_bindings()));
    let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
    vcx.simulate_keystrokes("escape");
    draw(cx, window);
    cx.update(|cx| {
        let e = editor.read(cx);
        assert!(e.gdt_dlg.is_none() && matches!(e.tool, Tool::Select), "one Esc closes it and ends the tool");
    });
}

/// Live2 drw-note-editor: Esc in the note editor closes it and ends the Note tool.
#[gpui::test]
fn esc_in_the_note_editor_ends_the_note_tool(cx: &mut TestAppContext) {
    use noshape_scene::drawing_tools::Tool;
    let (window, editor) = drawing(cx, "note-esc");
    cx.update(|cx| editor.update(cx, |e, cx| e.set_tool(Tool::Note, cx)));
    draw(cx, window);
    cx.update_window(window, |_, w, cx| {
        editor.update(cx, |e, cx| {
            e.cursor = [60.0, 40.0];
            e.tool_click(w, cx);
        })
    })
    .unwrap();
    draw(cx, window);
    assert!(cx.update(|cx| matches!(editor.read(cx).panel, Some(crate::drawing_view::Panel::Note { .. }))), "the note editor opens");
    // (Esc in its text goes to `discard_inplace` or `cancel_panel`, elsewhere to `cancel`.)
    for close in [crate::drawing_view::DrawingEditor::discard_inplace, crate::drawing_view::DrawingEditor::cancel_panel, crate::drawing_view::DrawingEditor::cancel] {
        cx.update(|cx| editor.update(cx, |e, cx| e.set_tool(Tool::Note, cx)));
        cx.update_window(window, |_, w, cx| {
            editor.update(cx, |e, cx| {
                e.cursor = [60.0, 40.0];
                e.tool_click(w, cx);
            })
        })
        .unwrap();
        draw(cx, window);
        cx.update(|cx| editor.update(cx, close));
        let e = editor.read_with(cx, |e, _| (e.panel.is_none(), matches!(e.tool, Tool::Select)));
        assert_eq!(e, (true, true));
    }
    cx.update(|cx| {
        let e = editor.read(cx);
        assert!(e.panel.is_none() && matches!(e.tool, Tool::Select));
    });
}

/// Live2 drw-revision-table-properties: Esc closes a revision table's properties and drops its selection.
#[gpui::test]
fn esc_closes_revision_table_properties_and_drops_the_selection(cx: &mut TestAppContext) {
    use noshape_scene::drawing_tools::Sel;
    let (window, editor) = drawing(cx, "revision-props-esc");
    cx.update(|cx| {
        editor.update(cx, |e, cx| {
            let id = e.current_sheet().next_table_id();
            let table = noshape_scene::drawing_tools::revision_table(id, &Default::default(), [300.0, 250.0]);
            e.edit_sheet("Revision table", cx, |s| s.tables.push(table));
            e.selection = vec![Sel::Table(id)];
            e.open_revision_props(id, cx);
        })
    });
    draw(cx, window);
    assert!(cx.update(|cx| editor.read(cx).revision_dlg.is_some()));
    cx.update(|cx| editor.update(cx, |e, cx| e.cancel(cx)));
    cx.update(|cx| {
        let e = editor.read(cx);
        assert!(e.revision_dlg.is_none() && e.selection.is_empty());
    });
}

/// Live2 drw-section-view-section-depth: with Section depth on (Up to entity, nothing picked), a click off the views
/// once the cutting line is placed places the view without making it: the mode tabs go and ✓ waits; picking the
/// depth entity then makes the view there.
#[gpui::test]
fn a_section_view_waits_where_it_was_placed_for_its_depth_entity(cx: &mut TestAppContext) {
    use noshape_scene::drawing_tools::Tool;
    SUGGESTED.set(true);
    let (window, editor) = drawing(cx, "section-depth-placed");
    real_sheet(cx, window, &editor);
    // The document's views (the sheet shown has the pending one too).
    let views = |cx: &mut TestAppContext| cx.update(|cx| editor.read(cx).drawing.sheets[0].views.len());
    let before = views(cx);
    let (centre, corner) = cx.update(|cx| {
        let e = editor.read(cx);
        let pl = e.built.as_ref().expect("built").placements.first().cloned().expect("a view");
        let g = e.geoms.get(&pl.id).cloned().expect("geometry");
        let corner = g.snaps.iter().map(|s| pl.to_sheet(s.p)).min_by(|a, b| (a[0] + a[1]).total_cmp(&(b[0] + b[1]))).expect("a vertex");
        ([(pl.min[0] + pl.max[0]) / 2.0, (pl.min[1] + pl.max[1]) / 2.0], corner)
    });
    let click = |cx: &mut TestAppContext, at: [f64; 2]| {
        cx.update_window(window, |_, w, cx| {
            editor.update(cx, |e, cx| {
                e.cursor = at;
                e.update_preview(cx);
                e.tool_click(w, cx);
            })
        })
        .unwrap();
        draw(cx, window);
    };
    cx.update_window(window, |_, w, cx| {
        editor.update(cx, |e, cx| {
            e.set_tool(Tool::Section { parent: None, p0: None, line: None, partial: false, jogs: vec![] }, cx);
            e.sync_section_dlg(w, cx);
            e.set_section_mode(1, cx);
            e.toggle_section_depth(true, w, cx);
        })
    })
    .unwrap();
    draw(cx, window);
    // The first click picks the view; the cut is fixed on an inference point, the left edge at mid height (live2
    // place-1118).
    click(cx, centre);
    assert!(cx.update(|cx| matches!(editor.read(cx).tool, Tool::Section { parent: Some(_), line: None, .. })), "the view picked");
    click(cx, [corner[0], centre[1]]);
    assert!(cx.update(|cx| matches!(editor.read(cx).tool, Tool::Section { line: Some(_), .. })), "the cutting line is placed");
    // An empty corner of the sheet, inside its frame.
    click(cx, [30.0, 30.0]);
    assert_eq!(views(cx), before, "no view before its depth entity");
    assert_eq!(cx.update(|cx| editor.read(cx).section_dlg.as_ref().and_then(|d| d.placed)), Some([30.0, 30.0]));
    let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
    assert!(vcx.debug_bounds("ds-horiz").is_none(), "the mode tabs are gone");
    click(cx, corner);
    assert_eq!(views(cx), before + 1, "the depth entity makes the view");
    let placed = cx.update(|cx| editor.read(cx).drawing.sheets[0].views.last().map(|v| v.position)).unwrap();
    assert!((placed[1] - 30.0).abs() < 1e-6, "the view at the placement's height: {placed:?}");
}


/// live2 asm-feel (6), section-q (2): Angular with Cut angle unticked takes three clicks. The first (a view corner)
/// starts the line, which pivots about it toward the pointer; the second (the opposite corner) fixes it through that
/// point; the third places the view, which slides only along the cut's normal before it.
#[gpui::test]
fn an_angular_section_without_a_cut_angle_takes_its_line_from_two_clicks(cx: &mut TestAppContext) {
    use noshape_scene::drawing_tools::Tool;
    SUGGESTED.set(true);
    let (window, editor) = drawing(cx, "section-angular-free");
    real_sheet(cx, window, &editor);
    let views = |cx: &mut TestAppContext| cx.update(|cx| editor.read(cx).drawing.sheets[0].views.len());
    let before = views(cx);
    // The first view's lower-left and upper-right corners (sheet mm).
    let (lo, hi) = cx.update(|cx| {
        let e = editor.read(cx);
        let pl = e.built.as_ref().expect("built").placements.first().cloned().expect("a view");
        let g = e.geoms.get(&pl.id).cloned().expect("geometry");
        let pts: Vec<[f64; 2]> = g.snaps.iter().map(|s| pl.to_sheet(s.p)).collect();
        let lo = pts.iter().copied().min_by(|a, b| (a[0] + a[1]).total_cmp(&(b[0] + b[1]))).expect("a vertex");
        let hi = pts.iter().copied().max_by(|a, b| (a[0] + a[1]).total_cmp(&(b[0] + b[1]))).expect("a vertex");
        (lo, hi)
    });
    let click = |cx: &mut TestAppContext, at: [f64; 2]| {
        cx.update_window(window, |_, w, cx| {
            editor.update(cx, |e, cx| {
                e.cursor = at;
                e.update_preview(cx);
                e.tool_click(w, cx);
            })
        })
        .unwrap();
        draw(cx, window);
    };
    cx.update_window(window, |_, w, cx| {
        editor.update(cx, |e, cx| {
            e.set_tool(Tool::Section { parent: None, p0: None, line: None, partial: false, jogs: vec![] }, cx);
            e.sync_section_dlg(w, cx);
            e.set_section_mode(2, cx);
        })
    })
    .unwrap();
    draw(cx, window);
    click(cx, lo);
    assert!(cx.update(|cx| matches!(editor.read(cx).tool, Tool::Section { p0: Some(_), line: None, .. })), "the first click starts the line");
    click(cx, hi);
    let dir = cx.update(|cx| {
        let e = editor.read(cx);
        let Tool::Section { parent: Some(pid), line: Some((a, b)), .. } = &e.tool else { panic!("the second click fixes the line") };
        let pl = e.placement(*pid).unwrap();
        let (a, b) = (pl.to_sheet(*a), pl.to_sheet(*b));
        let l = ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt();
        [(b[0] - a[0]) / l, (b[1] - a[1]) / l]
    });
    let want = {
        let l = ((hi[0] - lo[0]).powi(2) + (hi[1] - lo[1]).powi(2)).sqrt();
        [(hi[0] - lo[0]) / l, (hi[1] - lo[1]) / l]
    };
    assert!((dir[0] * want[1] - dir[1] * want[0]).abs() < 1e-6, "the line runs corner to corner: {dir:?} vs {want:?}");
    // The view hangs on the pointer: a move along the cut doesn't move it.
    let pending = |cx: &mut TestAppContext, at: [f64; 2]| {
        cx.update_window(window, |_, _, cx| {
            editor.update(cx, |e, cx| {
                e.cursor = at;
                e.update_preview(cx);
            })
        })
        .unwrap();
        draw(cx, window);
        cx.update(|cx| editor.read(cx).pending_view().map(|v| v.position)).expect("the view on the pointer")
    };
    let off = [hi[0] + 60.0 * want[1], hi[1] - 60.0 * want[0]];
    let p1 = pending(cx, off);
    let p2 = pending(cx, [off[0] + 20.0 * want[0], off[1] + 20.0 * want[1]]);
    assert!((p1[0] - p2[0]).abs() < 1e-6 && (p1[1] - p2[1]).abs() < 1e-6, "it slides only along the cut's normal: {p1:?} {p2:?}");
    click(cx, off);
    assert_eq!(views(cx), before + 1, "the third click places the view");
}

/// Every right-click menu of ours, one entity of each kind, as live's dumps have them (live2 11-14,
/// dom/context-menus-drawing*.txt, nothing copied: no Paste; Add comment is cloud-only). `MENUDUMP <json>` prints them
/// all ({entity: [rows]}, separators "——", a submenu's label with " ▸") for docs/parity/drawing-matrix.md.
#[gpui::test]
fn drawing_context_menus_are_lives(cx: &mut TestAppContext) {
    use noshape_model::drawing::build::PickTarget;
    SUGGESTED.set(true);
    HOLED.set(true);
    SETUP.set(Some(|ctx, dw| {
        let g = call(ctx, "drawing_get", json!({ "drawing": dw }));
        let num = |v: &Value| v.as_f64().unwrap_or(0.0);
        let views = g["views"].as_array().cloned().unwrap_or_default();
        let front = views.iter().find(|v| v["orientation"] == "front").cloned().unwrap_or_else(|| views[0].clone());
        let len = |e: &Value| (num(&e["b"][0]) - num(&e["a"][0])).hypot(num(&e["b"][1]) - num(&e["a"][1]));
        let edges = front["edges"].as_array().cloned().unwrap_or_default();
        let long = edges.iter().filter(|e| e["type"] == "line").max_by(|a, b| len(a).total_cmp(&len(b))).cloned().unwrap_or_default();
        let (a, b) = ([num(&long["a"][0]), num(&long["a"][1])], [num(&long["b"][0]), num(&long["b"][1])]);
        let mid = [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0];
        let mut run = |tool: &str, args: Value| {
            if let Err(e) = registry::run(ctx, registry::by_tool_name(tool).unwrap(), &args) {
                eprintln!("MENUDUMP-SKIP {tool}: {}", e.message);
            }
        };
        run("drawing_dimension", json!({ "drawing": dw, "picks": [mid], "place": [mid[0], mid[1] + 15.0] }));
        run("drawing_inspection", json!({ "drawing": dw, "action": "automatic", "scope": "sheet" }));
        run("drawing_note", json!({ "drawing": dw, "text": "NOTE", "position": [30, 40] }));
        run("drawing_callout", json!({ "drawing": dw, "anchor": mid, "position": [mid[0] + 25.0, mid[1] + 25.0], "text": "1" }));
        run("drawing_datum", json!({ "drawing": dw, "anchor": a, "position": [a[0], a[1] - 15.0], "label": "A" }));
        run("drawing_gdt", json!({ "drawing": dw, "position": [60, 60], "rows": [{ "symbol": "position", "tolerance": "⌀0.1", "datums": ["A"] }] }));
        run("drawing_surface_finish", json!({ "drawing": dw, "position": [60, 80], "value": "Ra 1.6" }));
        run("drawing_surface_finish", json!({ "drawing": dw, "position": b, "value": "Ra 3.2" }));
        run("drawing_weld", json!({ "drawing": dw, "target": b, "position": [b[0] + 20.0, b[1] + 25.0], "spec": { "arrow": { "symbol": "fillet", "size": "5" } } }));
        run("drawing_edge_condition", json!({ "drawing": dw, "target": mid, "position": [mid[0] - 20.0, mid[1] + 20.0], "rows": [{ "sign": "+", "value": 0.3 }] }));
        run("drawing_ordinate", json!({ "drawing": dw, "origin": a, "side": [a[0], a[1] - 10.0], "points": [b] }));
        run("drawing_sketch", json!({ "drawing": dw, "tool": "line", "picks": [[20, 200], [60, 200]] }));
        run("drawing_sketch", json!({ "drawing": dw, "tool": "point", "picks": [[80, 200]] }));
        run("drawing_table", json!({ "drawing": dw, "position": [20, 280], "rows": 3, "columns": 2, "header_row": true }));
        run("drawing_revision_table", json!({ "drawing": dw, "position": [300, 290] }));
        run("drawing_sketch", json!({ "drawing": dw, "tool": "corner_rectangle", "picks": [[100, 240], [140, 270]] }));
        run("drawing_hatch_region", json!({ "drawing": dw, "points": [[120, 255]] }));
        run("drawing_centerline", json!({ "drawing": dw, "kind": "two_point", "picks": [a, [a[0], a[1] + 8.0]] }));
        // A 1×1 PNG.
        let png = std::env::temp_dir().join(format!("noshape-menu-dump-{}.png", std::process::id()));
        let bytes: &[u8] = &[0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 0x0d, 0x49, 0x48, 0x44, 0x52, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0, 0, 0, 0x1f, 0x15, 0xc4, 0x89, 0, 0, 0, 0x0d, 0x49, 0x44, 0x41, 0x54, 0x78, 0xda, 0x63, 0xfc, 0xff, 0x9f, 0xa1, 0x1e, 0, 0x07, 0x82, 0x02, 0x7f, 0x3d, 0xc8, 0x48, 0xef, 0, 0, 0, 0, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82];
        if std::fs::write(&png, bytes).is_ok() {
            run("drawing_insert_image", json!({ "drawing": dw, "path": png.to_string_lossy(), "position": [200, 230], "to": [230, 260] }));
        }
    }));
    let (window, editor) = drawing(cx, "menu-dump");
    real_sheet(cx, window, &editor);
    let entities: Vec<(String, u32, [f64; 2])> = cx.update(|cx| {
        let e = editor.read(cx);
        let b = e.built.as_ref().expect("built");
        let sheet = e.current_sheet();
        let kind = |id: u64| sheet.annotations.iter().find(|a| a.id == id).map(|a| format!("{:?}", a.kind).split([' ', '(', '{']).next().unwrap_or("").to_string()).unwrap_or_default();
        let on_edge = |id: u64| sheet.annotations.iter().any(|a| a.id == id && matches!(&a.kind, noshape_model::drawing::AnnotationKind::SurfaceFinish { on: Some(_), leaders, .. } if leaders.is_empty()));
        let mut out: Vec<(String, u32, [f64; 2])> = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for (pick, t) in b.picks.iter().enumerate() {
            let name = match t {
                PickTarget::Annotation(id) => match kind(*id).as_str() {
                    "Note" => "note".to_string(),
                    "Balloon" => "callout".into(),
                    "Datum" => "datum".into(),
                    "DatumTarget" => "datum-target".into(),
                    "Gdt" => "gdt".into(),
                    "SurfaceFinish" if on_edge(*id) => "surface-finish-on-edge".into(),
                    "SurfaceFinish" => "surface-finish".into(),
                    "Weld" => "weld".into(),
                    "EdgeCondition" => "edge-condition".into(),
                    "Centermark" => "centermark".into(),
                    k if k.contains("Centerline") => "centerline".into(),
                    k if k.starts_with("Ordinate") => "ordinate".into(),
                    k if k.ends_with("Dimension") => "dimension".into(),
                    k => k.to_lowercase(),
                },
                PickTarget::Inspection(_) => "inspection-item".into(),
                PickTarget::View(_) | PickTarget::Edge { .. } => "view".into(),
                PickTarget::TitleField(_) => "title-block".into(),
                PickTarget::Table(id) => match sheet.table(*id).map(|t| format!("{:?}", t.kind).split([' ', '(', '{']).next().unwrap_or("").to_string()).as_deref() {
                    Some("Revision") => "revision-table".into(),
                    Some(k) => format!("{}-table", k.to_lowercase()),
                    None => "table".into(),
                },
                PickTarget::SketchEntity(_) => "sketch".into(),
                PickTarget::Hatch(_) => "hatch".into(),
                PickTarget::Image(_) => "image".into(),
                t if format!("{t:?}").contains("Centermark") => "centermark".into(),
                _ => continue,
            };
            if seen.contains(&name) {
                continue;
            }
            // A text's middle (a callout's, away from its leader), else a line's.
            let at = b.geometry.texts.iter().find(|x| x.pick as usize == pick).map(|x| {
                let w = noshape_model::drawing::font::text_width(&x.text, x.height);
                [x.pos[0] + w / 2.0, x.pos[1] + x.height / 2.0]
            }).or_else(|| {
                b.geometry.lines.iter().find(|l| l.pick as usize == pick && l.points.len() >= 2).map(|l| [(l.points[0][0] + l.points[1][0]) / 2.0, (l.points[0][1] + l.points[1][1]) / 2.0])
            });
            // An image: the middle of its picture.
            let at = at.or_else(|| match t {
                PickTarget::Image(id) => sheet.images.iter().find(|i| i.id == *id).map(|i| [i.pos[0] + i.size[0] / 2.0, i.pos[1] + i.size[1] / 2.0]),
                _ => None,
            });
            if let Some(at) = at {
                seen.insert(name.clone());
                out.push((name, pick as u32, at));
            }
        }
        out
    });
    let flat = |entries: &[MenuEntry]| -> Vec<String> {
        entries
            .iter()
            .map(|m| match m {
                MenuEntry::Item { label, .. } => label.to_string(),
                MenuEntry::Separator => "——".into(),
                MenuEntry::Submenu(label, _) => format!("{label} ▸"),
            })
            .collect()
    };
    // The matrix's form: a submenu with its rows ("Insert ▸ Column Left | Column Right | …").
    fn full(entries: &[MenuEntry]) -> Vec<String> {
        entries
            .iter()
            .map(|m| match m {
                MenuEntry::Item { label, .. } => label.to_string(),
                MenuEntry::Separator => "——".into(),
                MenuEntry::Submenu(label, rows) => format!("{label} ▸ {}", full(rows).into_iter().filter(|r| r != "——").collect::<Vec<_>>().join(" | ")).trim_end().to_string(),
            })
            .collect()
    }
    let mut dump = serde_json::Map::new();
    let mut dump_full = serde_json::Map::new();
    let empty = cx.update(|cx| editor.update(cx, |e, cx| {
        e.cursor = [5.0, 5.0];
        e.hover = None;
        e.selection.clear();
        e.context_menu_entries(None, cx)
    }));
    dump.insert("empty-sheet".into(), json!(flat(&empty)));
    dump_full.insert("empty-sheet".into(), json!(full(&empty)));
    for (name, pick, at) in entities {
        let rows = cx.update(|cx| editor.update(cx, |e, cx| {
            e.cursor = at;
            e.hover = Some(pick);
            e.selection.clear();
            e.context_menu_entries(Some(pick), cx)
        }));
        dump.insert(name.clone(), json!(flat(&rows)));
        dump_full.insert(name, json!(full(&rows)));
    }
    // A cell of a general table and of a hole table, selected (click the table, then the cell again).
    let hole_table = cx.update(|cx| {
        editor.update(cx, |e, cx| {
            let view = e.built.as_ref()?.placements.first()?.id;
            let id = e.current_sheet().next_table_id();
            let kind = noshape_model::drawing::TableKind::Hole { view, origin: [0.0; 2], origin_ref: None, precision: None, flip_x: false, flip_y: false, show_location: true, collapse_tags: false, bottom_to_top: false, exclude: Vec::new(), x_dir: None, more: Vec::new() };
            e.edit_sheet("Hole table", cx, |s| s.tables.push(noshape_model::drawing::Table::new(id, kind, [300.0, 200.0])));
            Some(id)
        })
    });
    draw(cx, window);
    let general = cx.update(|cx| editor.read(cx).current_sheet().tables.iter().find(|t| matches!(t.kind, noshape_model::drawing::TableKind::General { .. })).map(|t| t.id));
    for (name, id) in [("table-cell", general), ("hole-table-cell", hole_table)] {
        let Some(id) = id else { continue };
        let rows = cx.update(|cx| editor.update(cx, |e, cx| {
            e.selection = vec![crate::drawing_view::Sel::Table(id)];
            e.cell_sel = Some(crate::drawing_cells::CellSel { table: id, a: (1, 0), b: (1, 0) });
            let rows = e.table_entries(id, cx);
            e.cell_sel = None;
            e.selection.clear();
            rows
        }));
        dump_full.insert(name.into(), json!(full(&rows)));
    }
    // A BOM table of the first view: its header's and row's menu (the whole table's), and a selected cell's.
    let bom = cx.update(|cx| {
        editor.update(cx, |e, cx| {
            let view = e.built.as_ref()?.placements.first()?.id;
            let id = e.current_sheet().next_table_id();
            let kind = noshape_model::drawing::TableKind::Bom { view, structured: false, top_level: false, bottom_to_top: false, overrides: Vec::new(), default_template: false, source: None };
            e.edit_sheet("BOM table", cx, |s| s.tables.push(noshape_model::drawing::Table::new(id, kind, [300.0, 120.0])));
            Some(id)
        })
    });
    draw(cx, window);
    if let Some(id) = bom {
        let (whole, cell) = cx.update(|cx| editor.update(cx, |e, cx| {
            e.selection = vec![crate::drawing_view::Sel::Table(id)];
            let whole = e.table_entries(id, cx);
            e.cell_sel = Some(crate::drawing_cells::CellSel { table: id, a: (1, 0), b: (1, 0) });
            let cell = e.table_entries(id, cx);
            e.cell_sel = None;
            e.selection.clear();
            (whole, cell)
        }));
        dump_full.insert("bom-table-header".into(), json!(full(&whole)));
        dump_full.insert("bom-table-row".into(), json!(full(&whole)));
        assert_eq!(full(&cell), ["Size ▸ Columns Equally | Rows Equally", "Resize...", "——", "Copy"], "live's bom-table-cell");
        dump_full.insert("bom-table-cell".into(), json!(full(&cell)));
    }
    // A revision's row of the revision table (row 1: the drawing's first revision).
    let revision_row = cx.update(|cx| {
        let e = editor.read(cx);
        let id = e.current_sheet().tables.iter().find(|t| matches!(t.kind, noshape_model::drawing::TableKind::Revision { .. }))?.id;
        Some(full(&e.revision_table_entries(id, Some((1, 0)))))
    });
    if let Some(rows) = revision_row {
        dump_full.insert("revision-table-row".into(), json!(rows));
    }
    // The sheet tree's reference row of the first view's source.
    let reference = cx.update(|cx| {
        let e = editor.read(cx);
        let sheet = e.current_sheet();
        let (key, _) = noshape_model::drawing::views::view_source(sheet, sheet.views.first()?.id)?;
        Some(full(&e.reference_menu(key.tab, None, None, cx)))
    });
    if let Some(rows) = reference {
        dump_full.insert("sheet-tree-part".into(), json!(rows));
    }
    let image_rows = cx.update(|cx| {
        let e = editor.read(cx);
        e.current_sheet().images.first().map(|i| full(&e.image_entries(i.id, cx)).into_iter().filter(|r| r != "Paste").collect::<Vec<_>>())
    });
    if let Some(rows) = image_rows {
        dump_full.insert("image".into(), json!(rows));
    }
    println!("MENUDUMP {}", Value::Object(dump_full));
    let tail = ["——", "Clear selection", "Zoom to fit", "Delete"];
    let with_tail = |rows: &[&str]| -> Vec<String> { rows.iter().chain(tail.iter()).map(|r| r.to_string()).collect() };
    let live: Vec<(&str, Vec<String>)> = vec![
        ("empty-sheet", ["Paste as table", "Sheet properties...", "Drawing properties...", "——", "Zoom to fit", "Zoom to window", "Zoom to sheet"].map(String::from).to_vec()),
        ("title-block", with_tail(&["Edit note...", "Explode note", "Copy", "Paste as table", "Add leader", "Move to ▸", "Bring to front", "Send to back"])),
        ("note", with_tail(&["Edit note...", "Explode note", "Copy", "Paste as table", "Add leader", "Move to ▸", "Bring to front", "Send to back"])),
        ("dimension", with_tail(&["Edit...", "Paste as table", "Unbreak dimension"])),
        ("inspection-item", ["Edit...", "Reorder inspection items...", "Remove inspection item", "Remove all inspection items", "——", "Clear selection", "Zoom to fit"].map(String::from).to_vec()),
        ("callout", with_tail(&["Edit...", "Copy", "Paste as table", "Add leader", "Remove leaders", "Move to ▸"])),
        ("datum", with_tail(&["Edit...", "Copy", "Paste as table"])),
        ("gdt", with_tail(&["Edit...", "Copy", "Paste as table", "Add leader", "Move to ▸"])),
        ("edge-condition", with_tail(&["Edit...", "Copy", "Paste as table", "Add leader", "Remove leaders", "Move to ▸"])),
        // On a value's text: Reset ordinate too (live2 drw-ordinate-value-rclick).
        ("ordinate", with_tail(&["Add to ordinate dimension group", "Edit...", "Paste as table", "Reset ordinate", "Unbreak dimension"])),
        ("revision-table", with_tail(&["Revision table properties...", "Paste as table", "Move to ▸", "Bring to front", "Send to back"])),
        ("surface-finish", with_tail(&["Edit...", "Copy", "Paste as table", "Add leader", "Move to ▸"])),
        // On an edge, no leader (live2 drw-surface-finish-flip 11-15): Flip where the free one has Add leader.
        ("surface-finish-on-edge", with_tail(&["Edit...", "Flip", "Copy", "Paste as table", "Move to ▸"])),
        ("weld", with_tail(&["Edit...", "Flip", "Copy", "Paste as table", "Move to ▸"])),
        // Live2's second pass (11-15).
        ("general-table", with_tail(&["Table properties...", "Copy", "Paste as table", "Move to ▸", "Bring to front", "Send to back", "Move to sheet..."])),
        ("centerline", ["Clear selection", "Zoom to fit", "Delete"].map(String::from).to_vec()),
        ("centermark", ["Paste as table", "Align centermark", "——", "Clear selection", "Zoom to fit"].map(String::from).to_vec()),
        ("hatch", with_tail(&["Copy", "Paste as table", "Edit hatch region..."])),
    ];
    for (entity, rows) in live {
        assert_eq!(dump.get(entity), Some(&json!(rows)), "{entity}'s menu");
    }
    // An image: its Switch to <file tab> row names the tab.
    let image: Vec<String> = cx.update(|cx| {
        let e = editor.read(cx);
        let id = e.current_sheet().images.first().map(|i| i.id).expect("an image");
        // (Nothing copied: the menu drops Paste, as `context_menu_entries` does.)
        flat(&e.image_entries(id, cx)).into_iter().filter(|r| r != "Paste").collect::<Vec<_>>()
    });
    let switch = image.iter().position(|r| r.starts_with("Switch to ")).expect("Switch to …");
    assert_eq!(&image[..switch], ["Image properties...", "Show image borders", "Copy", "Paste as table", "Move to ▸", "Bring to front", "Send to back", "——"]);
    assert_eq!(&image[switch + 1..], ["——", "Clear selection", "Zoom to fit", "Delete"]);
    // With a note copied every menu gains Paste (live2: after ctrl+c on a note): the note's after Explode note.
    let note = cx.update(|cx| editor.read(cx).current_sheet().annotations.iter().find(|a| matches!(a.kind, noshape_model::drawing::AnnotationKind::Note { .. })).map(|a| a.id)).expect("a note");
    let (pick, at) = cx.update(|cx| {
        let e = editor.read(cx);
        let b = e.built.as_ref().expect("built");
        let pick = b.picks.iter().position(|t| matches!(t, PickTarget::Annotation(id) if *id == note)).expect("the note's pick") as u32;
        let x = b.geometry.texts.iter().find(|x| x.pick == pick).expect("its text");
        (pick, [x.pos[0] + 1.0, x.pos[1] + x.height / 2.0])
    });
    let rows = cx.update(|cx| editor.update(cx, |e, cx| {
        e.selection = vec![noshape_scene::drawing_tools::Sel::Annotation(note)];
        e.copy_entities(cx);
        e.cursor = at;
        e.hover = Some(pick);
        e.context_menu_entries(Some(pick), cx)
    }));
    assert_eq!(&flat(&rows)[..4], ["Edit note...", "Explode note", "Paste", "Copy"]);
    // Move to ▸ lists every layer, the note's own greyed; with views behind it Send to back is never greyed (live2
    // drw-note-move-to-title-block).
    let state = |label: &str| rows.iter().find_map(|e| match e {
        MenuEntry::Item { label: l, disabled, .. } if l.as_ref() == label => Some(*disabled),
        _ => None,
    });
    assert_eq!(state("Send to back"), Some(false));
    assert!(cx.update(|cx| !editor.read(cx).current_sheet().views.is_empty()));
    let layers: Vec<(String, bool)> = rows.iter().find_map(|e| match e {
        MenuEntry::Submenu(l, items) if l.as_ref() == "Move to" => Some(items.iter().filter_map(|i| match i {
            MenuEntry::Item { label, disabled, .. } => Some((label.to_string(), *disabled)),
            _ => None,
        }).collect()),
        _ => None,
    }).expect("Move to ▸");
    assert_eq!(layers, [("Border frame".into(), false), ("Border zones".into(), false), ("Title block".into(), false), ("Drawing".into(), true)]);
    // The empty sheet with the note selected: the note's menu (live2 drw-note-copy-paste s8).
    let rows = cx.update(|cx| editor.update(cx, |e, cx| {
        e.cursor = [5.0, 5.0];
        e.hover = None;
        e.context_menu_entries(None, cx)
    }));
    assert_eq!(&flat(&rows)[..4], ["Edit note...", "Explode note", "Paste", "Copy"]);
    // A title block field right-clicked with the note selected: the field joins it, their rows (live2 rmb-probe).
    let (pick, at) = cx.update(|cx| {
        let e = editor.read(cx);
        let b = e.built.as_ref().expect("built");
        let pick = b.picks.iter().position(|t| matches!(t, PickTarget::TitleField(_))).expect("a title field") as u32;
        let x = b.geometry.texts.iter().find(|x| x.pick == pick).expect("its text");
        (pick, [x.pos[0] + 0.5, x.pos[1] + x.height / 2.0])
    });
    let rows = cx.update(|cx| editor.update(cx, |e, cx| {
        e.cursor = at;
        e.hover = Some(pick);
        e.context_menu_entries(Some(pick), cx)
    }));
    assert_eq!(flat(&rows), ["Paste", "Copy", "Paste as table", "Add leader", "Move to ▸", "——", "Clear selection", "Zoom to fit", "Delete"]);
}

/// Live drw-centermark (retake 996d6b8a): a Single centermark clicked on a hole that has an automatic mark replaces it
/// and stays after Esc.
#[gpui::test]
fn a_single_centermark_over_an_automatic_one_stays_after_escape(cx: &mut TestAppContext) {
    use noshape_model::drawing::views::EdgeGeom;
    use noshape_scene::drawing_tools::Tool;
    SUGGESTED.set(true);
    HOLED.set(true);
    let (window, editor) = drawing(cx, "centermark-over-auto");
    real_sheet(cx, window, &editor);
    let marks = |cx: &mut TestAppContext| cx.update(|cx| editor.read(cx).drawing.sheets[0].annotations.iter().filter(|a| matches!(a.kind, noshape_model::drawing::AnnotationKind::Centermark { .. })).count());
    let rim = cx.update(|cx| {
        let e = editor.read(cx);
        let b = e.built.as_ref().expect("built");
        b.placements.iter().find_map(|pl| {
            let g = e.geoms.get(&pl.id)?;
            g.model_edges.iter().find_map(|m| match m.geom {
                EdgeGeom::Circle { center, radius, face_on: true, .. } => Some(pl.to_sheet([center[0] + radius, center[1]])),
                _ => None,
            })
        })
    });
    let rim = rim.expect("a hole seen face on");
    let before = marks(cx);
    // The automatic marks' lines (a replaced one's lines pick as the Single mark).
    let autos = |cx: &mut TestAppContext| {
        cx.update(|cx| {
            let b = editor.read(cx).built.clone().expect("built");
            b.geometry.lines.iter().filter(|l| matches!(b.picks.get(l.pick as usize), Some(noshape_model::drawing::build::PickTarget::AutoCentermark { .. }))).count()
        })
    };
    let auto_before = autos(cx);
    assert!(auto_before > 0, "the hole has an automatic mark");
    cx.update(|cx| editor.update(cx, |e, cx| e.set_tool(Tool::Centermark, cx)));
    draw(cx, window);
    cx.update_window(window, |_, w, cx| {
        editor.update(cx, |e, cx| {
            e.cursor = rim;
            e.update_preview(cx);
            e.tool_click(w, cx);
        })
    })
    .unwrap();
    draw(cx, window);
    assert_eq!(marks(cx), before + 1, "the Single mark is drawn at once");
    cx.update(|cx| editor.update(cx, |e, cx| e.cancel(cx)));
    draw(cx, window);
    cx.update(|cx| editor.update(cx, |e, cx| e.cancel(cx)));
    draw(cx, window);
    assert_eq!(marks(cx), before + 1, "Esc keeps it");
    assert!(autos(cx) < auto_before, "in place of the automatic mark");
}

/// Live2 manual-4: ☰ › Print… in a drawing opens the drawing's PDF (live, pop-ups allowed: "<drawing>.pdf" in a new
/// tab ~1 s after the click), with no export banner; the bubble live shows when the browser blocks pop-ups is the
/// browser's.
#[gpui::test]
fn print_in_a_drawing_opens_its_pdf_without_a_banner(cx: &mut TestAppContext) {
    SUGGESTED.set(true);
    let (window, editor) = drawing(cx, "print");
    real_sheet(cx, window, &editor);
    cx.update(|cx| editor.update(cx, |e, cx| e.print(cx)));
    assert!(cx.update(|cx| editor.read(cx).toast.is_none()), "no banner");
    for _ in 0..500 {
        if cx.opened_url().is_some() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
        cx.run_until_parked();
    }
    let url = cx.opened_url().expect("the PDF opened");
    let path = url.strip_prefix("file://").expect("a file");
    assert!(path.ends_with("Drawing 1.pdf"), "{path}");
    let bytes = std::fs::read(path).expect("written");
    assert!(bytes.starts_with(b"%PDF"), "a PDF");
}

/// Interaction feel (pointer reach, click, drag): its own file.
mod feel;

/// Live drw-hcallout-fit 12: a hole callout lights every hole it counts (3X: the three like holes light blue), not
/// only the one it points at.
#[gpui::test]
fn a_hole_callout_lights_the_holes_it_counts(cx: &mut TestAppContext) {
    use noshape_model::drawing::views::EdgeGeom;
    use noshape_model::drawing::{Annotation, AnnotationKind, AnnotationRef, PointRef};
    START.set(Some("holes-plate-drawing"));
    let (window, editor) = drawing(cx, "hcallout-holes");
    real_sheet(cx, window, &editor);
    let id = cx.update(|cx| {
        editor.update(cx, |e, cx| {
            // A face-on bore with others of its radius in the same view.
            let (view, me, kin) = e.built.as_ref()?.placements.iter().find_map(|pl| {
                let g = e.geoms.get(&pl.id)?;
                let bores: Vec<_> = g.model_edges.iter().filter(|m| matches!(m.geom, EdgeGeom::Circle { face_on: true, bore: true, .. })).collect();
                let me = bores.first()?;
                let EdgeGeom::Circle { radius: r, .. } = me.geom else { return None };
                let kin = bores.iter().filter(|m| matches!(m.geom, EdgeGeom::Circle { radius, .. } if (radius - r).abs() < 1e-9)).count();
                (kin >= 3).then(|| (pl.id, (*me).clone(), kin))
            })?;
            let EdgeGeom::Circle { center, radius, .. } = me.geom else { return None };
            let id = e.current_sheet().next_annotation_id();
            let a = Annotation { id, kind: AnnotationKind::HoleCallout { view, center, radius, leaders: Vec::new() }, refs: vec![AnnotationRef { slot: 0, point: PointRef::Center { body: me.body, name: (*me.name).clone() } }], ..Default::default() };
            e.edit_sheet("Hole/thread callout", cx, |s| s.annotations.push(a));
            Some((id, kin))
        })
    });
    let (id, kin) = id.expect("a view with three like holes face on");
    draw(cx, window);
    let lit = cx.update(|cx| editor.read(cx).reference_edge_picks(id).len());
    assert!(lit >= kin, "the {kin} holes it counts light, not only its own: {lit} edges");

    // Live drw-hcallout-fit 26: Override › Tolerance › Fit starts a Clearance fit with no hole class (a dimension's
    // starts User defined).
    let fit = cx.update_window(window, |_, w, cx| {
        editor.update(cx, |e, cx| {
            e.open_hole_callout(id, w, cx);
            e.edit_palette(|d| d.cur.hole.get_or_insert_with(Default::default).override_tol = true, cx);
            e.edit_palette(|d| d.tol = "Fit", cx);
            e.palette.as_ref().map(|d| (d.cur.format.fit.fit_type, d.cur.format.tolerance.clone()))
        })
    });
    let (kind, tol) = fit.unwrap().expect("the palette");
    assert_eq!(kind, noshape_model::drawing::FitType::Clearance);
    assert!(matches!(&tol, noshape_model::drawing::Tolerance::Fit { hole, .. } if hole.is_empty()), "no hole class yet: {tol:?}");
}

/// Live drw-hole-table-cell-panel 13: a click in a size's merged SIZE cell outlines the whole cell (A1–A3), not the
/// row under the pointer.
#[gpui::test]
fn a_merged_hole_size_cell_is_outlined_whole(cx: &mut TestAppContext) {
    use noshape_model::drawing::views::EdgeGeom;
    use noshape_model::drawing::{Table, TableKind};
    START.set(Some("holes-plate-drawing"));
    let (window, editor) = drawing(cx, "hole-merged-cell");
    real_sheet(cx, window, &editor);
    let id = cx.update(|cx| {
        editor.update(cx, |e, cx| {
            let view = e.built.as_ref()?.placements.iter().find(|pl| {
                e.geoms.get(&pl.id).is_some_and(|g| g.model_edges.iter().filter(|m| matches!(m.geom, EdgeGeom::Circle { face_on: true, bore: true, .. })).count() >= 3)
            })?.id;
            let id = e.current_sheet().next_table_id();
            let kind = TableKind::Hole { view, origin: [0.0; 2], origin_ref: None, precision: None, flip_x: false, flip_y: false, show_location: true, collapse_tags: false, bottom_to_top: false, exclude: Vec::new(), x_dir: None, more: Vec::new() };
            e.edit_sheet("Hole table", cx, |s| s.tables.push(Table::new(id, kind, [300.0, 250.0])));
            Some(id)
        })
    });
    let id = id.expect("a view with holes face on");
    draw(cx, window);
    cx.update(|cx| {
        editor.update(cx, |e, _| {
            let l = e.built.as_ref().and_then(|b| b.table_layout(id)).cloned().expect("laid out");
            let m = *l.merges.iter().find(|m| m.rows > 1).expect("a size over two or more rows");
            e.selection = vec![crate::drawing_view::Sel::Table(id)];
            e.cell_sel = Some(crate::drawing_cells::CellSel { table: id, a: (m.row + 1, m.col), b: (m.row + 1, m.col) });
            let t0 = l.title as usize;
            let whole = ([l.xs[m.col], l.ys[t0 + m.row + m.rows]], [l.xs[m.col + m.cols], l.ys[t0 + m.row]]);
            assert_eq!(e.cell_sel_rect(), Some(whole), "the merged cell's outline, {m:?}");
        })
    });
}

/// Live drw-hole-table-cell-panel 13: the merged SIZE cell of A1–A3 lights those holes (their circles and centre
/// marks) and their three tags blue.
#[gpui::test]
fn a_hole_size_cell_lights_its_holes_and_tags(cx: &mut TestAppContext) {
    use noshape_model::drawing::views::EdgeGeom;
    use noshape_model::drawing::{Table, TableKind};
    START.set(Some("holes-plate-drawing"));
    let (window, editor) = drawing(cx, "hole-cell-lit");
    real_sheet(cx, window, &editor);
    let id = cx.update(|cx| {
        editor.update(cx, |e, cx| {
            let view = e.built.as_ref()?.placements.iter().find(|pl| {
                e.geoms.get(&pl.id).is_some_and(|g| g.model_edges.iter().filter(|m| matches!(m.geom, EdgeGeom::Circle { face_on: true, bore: true, .. })).count() >= 3)
            })?.id;
            let id = e.current_sheet().next_table_id();
            let kind = TableKind::Hole { view, origin: [0.0; 2], origin_ref: None, precision: None, flip_x: false, flip_y: false, show_location: true, collapse_tags: false, bottom_to_top: false, exclude: Vec::new(), x_dir: None, more: Vec::new() };
            e.edit_sheet("Hole table", cx, |s| s.tables.push(Table::new(id, kind, [300.0, 250.0])));
            Some(id)
        })
    });
    let id = id.expect("a view with holes face on");
    draw(cx, window);
    cx.update(|cx| {
        editor.update(cx, |e, _| {
            assert_eq!(e.hole_cells_lit(), (Vec::new(), Vec::new()), "nothing selected, nothing lit");
            let l = e.built.as_ref().and_then(|b| b.table_layout(id)).cloned().expect("laid out");
            let m = *l.merges.iter().find(|m| m.rows > 1).expect("a size over two or more rows");
            e.selection = vec![crate::drawing_view::Sel::Table(id)];
            e.cell_sel = Some(crate::drawing_cells::CellSel { table: id, a: (m.row, m.col), b: (m.row, m.col) });
            let (picks, texts) = e.hole_cells_lit();
            assert_eq!(texts.len(), m.rows, "one tag per row of the size");
            assert!(picks.len() >= m.rows, "each hole's circle (and centre mark): {} picks", picks.len());
            // One row's cell (TAG) lights its own hole alone.
            e.cell_sel = Some(crate::drawing_cells::CellSel { table: id, a: (m.row, 0), b: (m.row, 0) });
            assert_eq!(e.hole_cells_lit().1.len(), 1);
        })
    });
}

/// Live behaviours "feel coverage (11-20)" D2: a dimension dragged by its text from unselected ends unselected (tan
/// hover with grips while the pointer is on it), its move one edit; a selected one stays selected.
#[gpui::test]
fn a_dimension_dragged_from_unselected_ends_unselected(cx: &mut TestAppContext) {
    use gpui_kit::{MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent};
    use noshape_scene::drawing_tools::Sel;
    SUGGESTED.set(true);
    DIMENSIONED.set(true);
    let (window, editor) = drawing(cx, "dim-drag-unselected");
    DIMENSIONED.set(false);
    real_sheet(cx, window, &editor);
    let id = cx.update(|cx| editor.read(cx).current_sheet().annotations.iter().find(|a| a.kind.is_dimension()).map(|a| a.id)).expect("a dimension");
    let text_at = |cx: &mut TestAppContext| {
        cx.update(|cx| {
            let e = editor.read(cx);
            let (b, picks) = (e.built.clone().unwrap(), e.picks_of(Sel::Annotation(id)));
            let t = b.geometry.texts.iter().find(|t| picks.contains(&t.pick)).expect("the dimension's text").pos;
            let l = e.sheet_to_local(t);
            e.bounds.origin + Point::new(px(l[0] as f32), px(l[1] as f32))
        })
    };
    let drag = |cx: &mut TestAppContext, dy: f32| {
        let at = text_at(cx);
        let to = at + Point::new(px(0.), px(dy));
        cx.update_window(window, |_, w, cx| editor.update(cx, |e, cx| {
            e.on_mouse_move(&MouseMoveEvent { position: at, pressed_button: None, modifiers: Modifiers::default() }, w, cx);
            e.on_mouse_down(&MouseDownEvent { button: MouseButton::Left, position: at, modifiers: Modifiers::default(), click_count: 1, first_mouse: false }, w, cx);
            for k in 1..=4 {
                let p = at + Point::new(px(0.), px(dy * k as f32 / 4.));
                e.on_mouse_move(&MouseMoveEvent { position: p, pressed_button: Some(MouseButton::Left), modifiers: Modifiers::default() }, w, cx);
            }
            e.on_mouse_up(&MouseUpEvent { button: MouseButton::Left, position: to, modifiers: Modifiers::default(), click_count: 1 }, w, cx);
        }))
        .unwrap();
        draw(cx, window);
    };
    let place = |cx: &mut TestAppContext| cx.update(|cx| editor.read(cx).current_sheet().annotations.iter().find(|a| a.id == id).map(|a| format!("{:?}", a.kind))).unwrap();
    let before = place(cx);
    drag(cx, -30.);
    assert_ne!(place(cx), before, "moved");
    assert!(cx.update(|cx| editor.read(cx).selection.is_empty()), "not selected after the drag");
    cx.update(|cx| editor.update(cx, |e, _| e.selection = vec![Sel::Annotation(id)]));
    drag(cx, 20.);
    assert_eq!(cx.update(|cx| editor.read(cx).selection.clone()), vec![Sel::Annotation(id)], "a selected one stays selected");
}

/// Live parity-open-1120 dr2 (DRW-MULTI-VIEW-MENU): two views selected, the menu is Show/hide ▸ (hidden lines, shaded
/// view, threads, faulty parts, center of mass) · Tangent edges ▸ · Adjust linestyle... · View properties... · Move to
/// sheet... · Copy · Paste as table · Clear selection · Zoom to fit · Delete.
#[gpui::test]
fn two_views_menu_as_live(cx: &mut TestAppContext) {
    use noshape_scene::drawing_tools::Sel;
    SUGGESTED.set(true);
    let (window, editor) = drawing(cx, "multi-view-menu");
    real_sheet(cx, window, &editor);
    let rows = cx.update(|cx| {
        editor.update(cx, |e, cx| {
            e.selection = e.current_sheet().views.iter().take(2).map(|v| Sel::View(v.id)).collect();
            assert_eq!(e.selection.len(), 2, "two views");
            e.multi_selection_entries(cx)
        })
    });
    fn labels(entries: &[MenuEntry]) -> Vec<String> {
        entries
            .iter()
            .filter_map(|m| match m {
                MenuEntry::Item { label, .. } => Some(label.to_string()),
                MenuEntry::Separator => None,
                MenuEntry::Submenu(label, rows) => Some(format!("{label} ▸ {}", labels(rows).join(" | "))),
            })
            .collect()
    }
    // (Paste is there only while views are copied: the tests share the clipboard.)
    let got: Vec<String> = labels(&rows).into_iter().filter(|l| l != "Paste").collect();
    let show = "Show/hide ▸ Show hidden lines | Show shaded view | Hide threads | Show faulty parts | Show center of mass";
    assert_eq!(got.first().map(String::as_str), Some(show));
    assert!(got[1].starts_with("Tangent edges ▸"), "{got:?}");
    assert_eq!(got[2..], ["Adjust linestyle...", "View properties...", "Move to sheet...", "Copy", "Paste as table", "Clear selection", "Zoom to fit", "Delete"]);
}

/// Live ct-2-add (cfg-choosers-1120): the Custom table dialog's Add and Insert are 26 px, 2 px apart; the fixed corners
/// 26 px on a 52 px pitch, 15 px in.
#[gpui::test]
fn custom_table_dialog_rows_as_live(cx: &mut TestAppContext) {
    use noshape_scene::drawing_tools::{TableTool, Tool};
    SUGGESTED.set(true);
    let (window, editor) = drawing(cx, "ct-rows");
    real_sheet(cx, window, &editor);
    cx.update(|cx| editor.update(cx, |e, cx| e.set_tool(Tool::Table { kind: TableTool::Custom, view: None, origin: None }, cx)));
    draw(cx, window);
    let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
    let at = |vcx: &mut gpui_kit::VisualTestContext, id: &'static str| vcx.debug_bounds(id).unwrap_or_else(|| panic!("no {id}"));
    let (dlg, add, insert) = (at(&mut vcx, "dr-custom-dlg"), at(&mut vcx, "ct-add"), at(&mut vcx, "ct-insert"));
    assert_eq!((add.size.height, insert.size.height), (px(26.), px(26.)), "Add and Insert");
    assert_eq!(insert.origin.y - add.bottom(), px(2.), "2 px apart");
    let (c0, c1) = (at(&mut vcx, "ct-corner-0"), at(&mut vcx, "ct-corner-1"));
    assert_eq!(c0.size.width, px(26.), "a corner button");
    assert_eq!((c0.origin.x - dlg.origin.x, c1.origin.x - c0.origin.x), (px(15.), px(52.)), "15 px in, 52 px apart");
}

/// Live drawing/262 (dom drawing-custom-table-select-part-studio): Custom table › Add opens "Select a Part Studio",
/// the Insert view chooser's look (Current document | Other documents, the document, Part Studios only, no kinds,
/// filters or Import), 40 px right of the dialog; a pick is the table's Part Studio and closes it; Esc closes it too.
#[gpui::test]
fn custom_table_add_picks_a_part_studio_in_the_chooser(cx: &mut TestAppContext) {
    use noshape_scene::drawing_tools::{TableTool, Tool};
    SUGGESTED.set(true);
    let (window, editor) = drawing(cx, "ct-studio-chooser");
    real_sheet(cx, window, &editor);
    cx.update(|cx| editor.update(cx, |e, cx| e.set_tool(Tool::Table { kind: TableTool::Custom, view: None, origin: None }, cx)));
    draw(cx, window);
    press(cx, window, "ct-add");
    let state = |cx: &mut TestAppContext| cx.update(|cx| {
        let e = editor.read(cx);
        (e.chooser.studio, e.insert.chooser, e.custom_tables.dlg.as_ref().map(|d| (d.studio, d.version.clone(), d.picker.is_some())))
    });
    let (studio_mode, open, _) = state(cx);
    assert!(studio_mode && open == Some(0), "the chooser in its studio mode");
    let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
    for gone in ["dr-ch-ps", "dr-ch-import"] {
        assert!(vcx.debug_bounds(gone).is_none(), "no {gone}");
    }
    let (dlg, chooser) = (vcx.debug_bounds("dr-custom-dlg").expect("the dialog"), vcx.debug_bounds("dr-chooser").expect("the chooser"));
    assert!((chooser.left() - (dlg.right() + px(40.))).abs() < px(1.), "40 px right of the dialog: {dlg:?} {chooser:?}");
    let studio = cx.update(|cx| editor.read(cx).doc.read(cx).session.doc.ordered_tabs().into_iter().find(|t| matches!(t.content, noshape_model::TabContent::PartStudio(_))).map(|t| t.id)).unwrap();
    press(cx, window, &format!("dr-ch-ps-{studio}"));
    let (studio_mode, open, dlg) = state(cx);
    assert!(!studio_mode && open.is_none(), "a pick closes it");
    assert_eq!(dlg, Some((Some(studio), None, false)), "the table's Part Studio, this workspace's");
    press(cx, window, "ct-add");
    cx.update_window(window, |_, w, cx| w.dispatch_action(Box::new(gpui_kit::component::input::Escape), cx)).unwrap();
    draw(cx, window);
    let (studio_mode, open, dlg) = state(cx);
    assert!(!studio_mode && open.is_none() && dlg.as_ref().is_some_and(|d| !d.2), "Esc closes it, the dialog stays: {dlg:?}");
    // Esc reaching the drawing (the app's Cancel) closes it the same, and the next one ends the tool and its dialog
    // (live drw-custom-table-add 9-10).
    cx.update(|cx| cx.bind_keys([gpui_kit::KeyBinding::new("escape", noshape_app_core::actions::Cancel, Some("Drawing && !Input"))]));
    press(cx, window, "ct-add");
    cx.update_window(window, |_, w, cx| { let f = editor.read(cx).focus.clone(); w.focus(&f, cx) }).unwrap();
    gpui_kit::VisualTestContext::from_window(window, cx).simulate_keystrokes("escape");
    draw(cx, window);
    let (studio_mode, open, dlg) = state(cx);
    assert!(!studio_mode && open.is_none() && dlg.as_ref().is_some_and(|d| !d.2), "the first Esc closes the chooser only: {dlg:?}");
    gpui_kit::VisualTestContext::from_window(window, cx).simulate_keystrokes("escape");
    draw(cx, window);
    let (_, _, dlg) = state(cx);
    let select = cx.update(|cx| matches!(editor.read(cx).tool, Tool::Select));
    assert!(dlg.is_none() && select, "the second Esc closes Custom table: {dlg:?}");
}

/// Live iv-2 (DRW-CHOOSER-SORT): the chooser lists the studios by name, A → Z; its sort button's Name: Z → A
/// reverses them.
#[gpui::test]
fn the_chooser_sorts_studios_by_name(cx: &mut TestAppContext) {
    use noshape_scene::drawing_tools::{TableTool, Tool};
    SUGGESTED.set(true);
    SETUP.set(Some(|ctx, _| {
        call(ctx, "tab_create", json!({ "kind": "partstudio", "name": "Zed studio" }));
        call(ctx, "tab_create", json!({ "kind": "partstudio", "name": "a studio" }));
    }));
    let (window, editor) = drawing(cx, "chooser-sort");
    real_sheet(cx, window, &editor);
    cx.update(|cx| editor.update(cx, |e, cx| e.set_tool(Tool::Table { kind: TableTool::Custom, view: None, origin: None }, cx)));
    draw(cx, window);
    press(cx, window, "ct-add");
    let ids: Vec<(String, noshape_model::document::TabId)> = cx.update(|cx| {
        let doc = &editor.read(cx).doc.read(cx).session.doc;
        doc.ordered_tabs().into_iter().filter(|t| matches!(t.content, noshape_model::TabContent::PartStudio(_))).map(|t| (t.name.clone(), t.id)).collect()
    });
    let order = |cx: &mut TestAppContext| {
        let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
        let mut rows: Vec<(gpui_kit::Pixels, String)> = ids.iter().map(|(n, id)| (vcx.debug_bounds(Box::leak(format!("dr-ch-ps-{id}").into_boxed_str())).expect("a studio row").origin.y, n.clone())).collect();
        rows.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        rows.into_iter().map(|r| r.1).collect::<Vec<_>>()
    };
    assert_eq!(order(cx), ["a studio", "Part Studio 1", "Zed studio"], "A → Z");
    pick(cx, window, "dr-ch-sort", "Name: Z → A");
    assert_eq!(order(cx), ["Zed studio", "Part Studio 1", "a studio"], "Z → A");
}

/// Live cfg-choosers-1120: a configured studio's row in Select a Part Studio has a › that shows its inputs under it
/// (a list per input) and the configuration buttons; the studio picked takes the configuration set there.
#[gpui::test]
fn custom_table_add_takes_a_configured_studios_configuration(cx: &mut TestAppContext) {
    use noshape_scene::drawing_tools::{TableTool, Tool};
    SUGGESTED.set(true);
    SETUP.set(Some(|ctx, _| {
        call(ctx, "config_add_input", json!({ "tab": "Part Studio 1", "kind": "list", "name": "Size" }));
        call(ctx, "config_add_option", json!({ "tab": "Part Studio 1", "input": "Size", "name": "Large" }));
    }));
    let (window, editor) = drawing(cx, "ct-studio-config");
    real_sheet(cx, window, &editor);
    cx.update(|cx| editor.update(cx, |e, cx| e.set_tool(Tool::Table { kind: TableTool::Custom, view: None, origin: None }, cx)));
    draw(cx, window);
    press(cx, window, "ct-add");
    let (studio, input, large) = cx.update(|cx| {
        let doc = &editor.read(cx).doc.read(cx).session.doc;
        let tab = doc.ordered_tabs().into_iter().find(|t| matches!(t.content, noshape_model::TabContent::PartStudio(_))).unwrap();
        let noshape_model::TabContent::PartStudio(ps) = &tab.content else { unreachable!() };
        let i = ps.configuration.inputs.first().expect("Size").clone();
        let noshape_model::configuration::ConfigInputKind::List { options, .. } = &i.kind else { panic!("a list") };
        (tab.id, i.id.clone(), options.iter().find(|o| o.name == "Large").expect("Large").id.clone())
    });
    let list = format!("dr-ch-cfg-{studio}-{input}");
    let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
    assert!(vcx.debug_bounds(Box::leak(format!("dr-ch-cfg-last-{studio}").into_boxed_str())).is_some(), "its configuration buttons");
    assert!(vcx.debug_bounds(Box::leak(list.clone().into_boxed_str())).is_none(), "its inputs folded");
    press(cx, window, &format!("dr-ch-cfg-exp-{studio}"));
    assert!(cx.update(|cx| editor.read(cx).chooser.studio), "the › keeps the chooser open");
    pick(cx, window, &list, "Large");
    press(cx, window, &format!("dr-ch-ps-{studio}"));
    let config = cx.update(|cx| editor.read(cx).custom_tables.dlg.as_ref().map(|d| (d.studio, d.config.0.get(&input).cloned())));
    assert_eq!(config, Some((Some(studio), Some(large))), "the studio at Size Large");
}

/// Live cfg-choosers-1120 (iv-*): in Insert view's chooser a configured studio's row is 31 px with the configuration
/// buttons; opening it shows its inputs above its parts, and a part picked there still goes in (the inline value is
/// only browsed: live's view says "Size: Small").
#[gpui::test]
fn insert_view_lists_a_configured_studios_inputs_above_its_parts(cx: &mut TestAppContext) {
    SETUP.set(Some(|ctx, _| {
        call(ctx, "config_add_input", json!({ "tab": "Part Studio 1", "kind": "list", "name": "Size" }));
        call(ctx, "config_add_option", json!({ "tab": "Part Studio 1", "input": "Size", "name": "Large" }));
    }));
    let (window, editor) = drawing(cx, "iv-studio-config");
    real_sheet(cx, window, &editor);
    cx.update(|cx| editor.update(cx, |e, cx| e.open_insert_chooser(cx)));
    draw(cx, window);
    let (studio, input) = cx.update(|cx| {
        let doc = &editor.read(cx).doc.read(cx).session.doc;
        let tab = doc.ordered_tabs().into_iter().find(|t| matches!(t.content, noshape_model::TabContent::PartStudio(_))).unwrap();
        let noshape_model::TabContent::PartStudio(ps) = &tab.content else { unreachable!() };
        (tab.id, ps.configuration.inputs.first().expect("Size").id.clone())
    });
    let list = format!("dr-ch-cfg-{studio}-{input}");
    let row = format!("dr-ch-ps-{studio}");
    let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
    let r = vcx.debug_bounds(Box::leak(row.clone().into_boxed_str())).expect("the studio's row");
    assert_eq!(r.size.height, px(31.), "a configured studio's row");
    assert!(vcx.debug_bounds(Box::leak(format!("dr-ch-cfg-last-{studio}").into_boxed_str())).is_some(), "its configuration buttons");
    assert!(vcx.debug_bounds(Box::leak(list.clone().into_boxed_str())).is_none(), "its inputs folded");
    // A press in the list takes the focus from the search box (live drw-insert-view-top-1to1 s2).
    let search = cx.update(|cx| editor.read(cx).chooser.search.clone()).expect("the search box");
    cx.update_window(window, |_, w, cx| { let f = gpui_kit::Focusable::focus_handle(search.read(cx), cx); w.focus(&f, cx) }).unwrap();
    draw(cx, window);
    assert!(cx.update(|cx| editor.read(cx).chooser.search_focused), "focused: live's blue border");
    press(cx, window, &row);
    assert!(!cx.update_window(window, |_, w, cx| gpui_kit::Focusable::focus_handle(search.read(cx), cx).is_focused(w)).unwrap(), "the search box left");
    draw(cx, window);
    assert!(!cx.update(|cx| editor.read(cx).chooser.search_focused), "its border grey again");
    let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
    let l = vcx.debug_bounds(Box::leak(list.clone().into_boxed_str())).expect("its Size list once open");
    assert!(cx.update(|cx| editor.read(cx).insert.chooser.is_some()), "the name only opens it");
    pick(cx, window, &list, "Large");
    let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
    let part = vcx.debug_bounds(Box::leak(format!("dr-ch-c-{studio}-0").into_boxed_str())).expect("its first part row");
    assert!(part.origin.y > l.origin.y, "the inputs above the parts");
    press(cx, window, &format!("dr-ch-c-{studio}-0"));
    assert!(cx.update(|cx| editor.read(cx).insert.chooser.is_none()), "a part picked closes the chooser");
}

/// Live drw-datum-target-hide-dim s4–s5: a placed target's Origin field is active; picking a point keeps it active
/// (another point replaces it), and a click in space then starts the next target.
#[gpui::test]
fn a_datum_targets_origin_field_stays_active_once_picked(cx: &mut TestAppContext) {
    use noshape_model::drawing::build::PickTarget;
    use noshape_scene::drawing_tools::Tool;
    SUGGESTED.set(true);
    let (window, editor) = drawing(cx, "dt-origin");
    real_sheet(cx, window, &editor);
    let (a, b) = cx.update(|cx| {
        let e = editor.read(cx);
        let bt = e.built.as_ref().expect("built");
        let l = bt.geometry.lines.iter().find(|l| l.points.len() >= 2 && matches!(bt.picks.get(l.pick as usize), Some(PickTarget::Edge { .. }))).expect("a view edge");
        (l.points[0], l.points[l.points.len() - 1])
    });
    cx.update(|cx| editor.update(cx, |e, cx| e.set_tool(Tool::DatumTarget { anchor: None }, cx)));
    draw(cx, window);
    let click = |cx: &mut TestAppContext, at: [f64; 2]| {
        cx.update_window(window, |_, w, cx| editor.update(cx, |e, cx| {
            e.cursor = at;
            e.extra_tool_click(w, cx);
        }))
        .unwrap();
        draw(cx, window);
    };
    click(cx, a);
    click(cx, [a[0] + 15.0, a[1] + 15.0]);
    let picking = |cx: &mut TestAppContext| cx.update(|cx| editor.read(cx).datum_dlg.as_ref().and_then(|d| d.picking_origin));
    assert_eq!(picking(cx), Some(0), "the placed target's Origin field is active");
    click(cx, b);
    let origin = cx.update(|cx| editor.read(cx).datum_dlg.as_ref().and_then(|d| d.rows.first()).and_then(|r| r.ann.target.as_ref()).and_then(|t| t.origin));
    assert!(origin.is_some(), "the origin picked");
    assert_eq!(picking(cx), Some(0), "the Origin field stays active once picked");
    click(cx, [5.0, 5.0]);
    assert!(cx.update(|cx| matches!(editor.read(cx).tool, Tool::DatumTarget { anchor: Some(_) })), "a click in space starts the next target");
}
