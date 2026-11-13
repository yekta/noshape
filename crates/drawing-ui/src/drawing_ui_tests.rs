//! UI tests of the Drawing editor on GPUI's test platform (docs/TESTS.md "UI keystroke tests"): the real editor in
//! a test window over a document made through the agent API, the GPU a no-op stand-in.

use crate::doc_model::DocModel;
use crate::drawing_view::DrawingEditor;
use crate::gpu::Gpu;
use gpui_kit::{AnyWindowHandle, AppContext as _, Bounds, Entity, Modifiers, Point, TestAppContext, WindowBounds, WindowOptions, px, size};
use noshape_ops::drawing_export::DrawingExportPrefs;
use noshape_ops::{Ctx, registry};
use serde_json::{Value, json};

fn call(ctx: &mut Ctx, tool: &str, args: Value) -> Value {
    let op = registry::by_tool_name(tool).unwrap_or_else(|| panic!("no tool {tool}"));
    registry::run(ctx, op, &args).unwrap_or_else(|e| panic!("{tool}: {e:?}")).value
}

thread_local! {
    /// The next test drawing gets the source's suggested views.
    static SUGGESTED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
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
    {
        let mut ctx = Ctx::new();
        call(&mut ctx, "document_create", json!({ "path": path.to_str().unwrap() }));
        let sk = call(&mut ctx, "sketch_create", json!({ "plane": "plane:top" }))["sketch"].clone();
        let d = call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "tool": "corner_rectangle", "points": [[0, 0], [40, 20]] }));
        let region = d["regions"][0]["id"].clone();
        call(&mut ctx, "feature_add", json!({ "type": "extrude", "params": { "entities": [region], "depth": "10 mm" } }));
        call(&mut ctx, "drawing_create", json!({ "source": "Part Studio 1", "template": "ISO A3", "suggested_views": SUGGESTED.get() }));
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

/// Lead 11-13 (interaction feel): a box dragged right to left selects what it crosses, left to right only what lies
/// wholly inside it (Onshape's window / crossing boxes, behaviours "box selection: window vs crossing").
#[gpui::test]
fn a_box_right_to_left_takes_what_it_crosses(cx: &mut TestAppContext) {
    use noshape_scene::drawing_tools::Sel;
    SUGGESTED.set(true);
    let (window, editor) = drawing(cx, "box-crossing");
    for _ in 0..1500 {
        if cx.update(|cx| editor.read(cx).real_sheet_in()) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
        cx.executor().advance_clock(std::time::Duration::from_millis(50));
        draw(cx, window);
    }
    let pl = cx.update(|cx| editor.read(cx).built.as_ref().and_then(|b| b.placements.first().cloned())).expect("a view placed");
    // From below-left of the view to its middle: half of it inside.
    let corner = [pl.min[0] - 3.0, pl.min[1] - 3.0];
    let middle = [(pl.min[0] + pl.max[0]) / 2.0, (pl.min[1] + pl.max[1]) / 2.0];
    let selected = |cx: &mut TestAppContext, from: [f64; 2], to: [f64; 2]| {
        cx.update(|cx| editor.update(cx, |e, cx| {
            e.select_in_box(from, to, false, cx);
            e.selection.contains(&Sel::View(pl.id))
        }))
    };
    assert!(!selected(cx, corner, middle), "left to right: the half-covered view isn't taken");
    assert!(selected(cx, middle, corner), "right to left: it is");
    let around = [pl.max[0] + 3.0, pl.max[1] + 3.0];
    assert!(selected(cx, corner, around), "left to right around it: taken");
}

/// Lead 11-13 (interaction feel): a free view dragged within 6 px of another view's centre line is held on it, the
/// placement inference's line shown; farther it moves freely.
#[gpui::test]
fn a_dragged_view_holds_on_another_views_centre_line(cx: &mut TestAppContext) {
    SUGGESTED.set(true);
    let (window, editor) = drawing(cx, "view-align");
    for _ in 0..1500 {
        if cx.update(|cx| editor.read(cx).real_sheet_in()) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
        cx.executor().advance_clock(std::time::Duration::from_millis(50));
        draw(cx, window);
    }
    let (a, b, zoom, start) = cx.update(|cx| {
        let e = editor.read(cx);
        let pls = e.built.as_ref().expect("built").placements.clone();
        let centre = |pl: &noshape_model::drawing::build::ViewPlacement| [(pl.min[0] + pl.max[0]) / 2.0, (pl.min[1] + pl.max[1]) / 2.0];
        let (pa, pb) = pls.iter().flat_map(|x| pls.iter().map(move |y| (x, y))).find(|(x, y)| (centre(x)[1] - centre(y)[1]).abs() > 20.0).expect("two views at different heights");
        let start = e.current_sheet().view(pa.id).expect("view").position;
        ((pa.id, centre(pa)), centre(pb), e.zoom, start)
    });
    let px_mm = 1.0 / zoom;
    let off = |px: f64| [15.0, b[1] - a.1[1] + px * px_mm];
    let moved = cx.update(|cx| editor.update(cx, |e, _| (e.align_view_drag(&[(a.0, start)], off(4.0)), e.infer_lines.len())));
    assert!((moved.0[1] - (b[1] - a.1[1])).abs() < 1e-9 && moved.1 >= 1, "4 px off: held on the line, its guide shown: {moved:?}");
    let free = cx.update(|cx| editor.update(cx, |e, _| {
        e.infer_lines.clear();
        e.align_view_drag(&[(a.0, start)], off(9.0))
    }));
    assert!((free[1] - off(9.0)[1]).abs() < 1e-9, "9 px off: free");
}
