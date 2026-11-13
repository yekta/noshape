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
    /// The next test drawing gets a dimension on its first view's longest edge.
    static DIMENSIONED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
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
        let d = call(&mut ctx, "drawing_create", json!({ "source": "Part Studio 1", "template": "ISO A3", "suggested_views": SUGGESTED.get() }));
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

/// Live2 feel/drawing.json F3: a box takes a view it reaches in either direction (half of it covered), and it adds to
/// what was selected.
#[gpui::test]
fn a_box_takes_the_views_it_reaches_and_adds_them(cx: &mut TestAppContext) {
    use noshape_scene::drawing_tools::Sel;
    SUGGESTED.set(true);
    let (window, editor) = drawing(cx, "box-views");
    for _ in 0..1500 {
        if cx.update(|cx| editor.read(cx).real_sheet_in()) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
        cx.executor().advance_clock(std::time::Duration::from_millis(50));
        draw(cx, window);
    }
    let pls = cx.update(|cx| editor.read(cx).built.as_ref().map(|b| b.placements.clone()).unwrap_or_default());
    let (pl, other) = (pls[0], pls[1]);
    let corner = [pl.min[0] - 3.0, pl.min[1] - 3.0];
    let middle = [(pl.min[0] + pl.max[0]) / 2.0, (pl.min[1] + pl.max[1]) / 2.0];
    for (from, to) in [(corner, middle), (middle, corner)] {
        let taken = cx.update(|cx| editor.update(cx, |e, cx| {
            e.selection = vec![Sel::View(other.id)];
            e.select_in_box(from, to, true, cx);
            (e.selection.contains(&Sel::View(pl.id)), e.selection.contains(&Sel::View(other.id)))
        }));
        assert_eq!(taken, (true, true), "half covered: taken, and the other view still selected ({from:?} -> {to:?})");
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

/// Live2 (REQUESTS 2578 (4), drw-dimension-tolerance-symmetrical): Enter in the dimension palette makes the edit and
/// leaves the palette open; a click on the empty sheet leaves it open too; Esc (`cancel`) closes it, the edit kept.
#[gpui::test]
fn the_dimension_palette_stays_open_on_enter_and_on_the_empty_sheet(cx: &mut TestAppContext) {
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
    assert_eq!(prefix.as_deref(), Some("R"), "Enter made the edit");
    let empty = empty_sheet_point(cx, window, &editor);
    click_sheet(cx, window, &editor, empty);
    assert!(cx.update(|cx| editor.read(cx).palette.is_some()), "a click on the empty sheet leaves it open");
    cx.update(|cx| editor.update(cx, |e, cx| e.cancel(cx)));
    draw(cx, window);
    let (open, prefix) = cx.update(|cx| {
        let e = editor.read(cx);
        (e.palette.is_some(), e.current_sheet().annotations.iter().find(|a| a.id == id).map(|a| a.format.prefix.clone()))
    });
    assert!(!open, "Esc closes it");
    assert_eq!(prefix.as_deref(), Some("R"), "the edit made with Enter stays");
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
