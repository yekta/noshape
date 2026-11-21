//! T2 bench: a drawing of 20 views (docs/TESTS.md "UI benches"): opening it, its frames, Update, zoom and pan.

use crate::doc_model::DocModel;
use crate::drawing_view::DrawingEditor;
use crate::gpu::Gpu;
use gpui_kit::{AnyWindowHandle, AppContext as _, Bounds, Entity, Point, TestAppContext, WindowBounds, WindowOptions, px, size};
use noshape_ops::{Ctx, registry};
use noshape_ui_test_support::bench;
use serde_json::{Value, json};
use std::time::{Duration, Instant};

const CASE: &str = "drawing (20 views)";

fn call(ctx: &mut Ctx, tool: &str, args: Value) -> Value {
    registry::run(ctx, registry::by_tool_name(tool).unwrap(), &args).unwrap_or_else(|e| panic!("{tool}: {e:?}")).value
}

fn draw(cx: &mut TestAppContext, window: AnyWindowHandle) {
    cx.update_window(window, |_, w, cx| w.draw(cx).clear(cx)).unwrap();
}

/// Background work done (the test dispatcher runs it here), with frames as the app draws them.
fn settle(cx: &mut TestAppContext, window: AnyWindowHandle, editor: &Entity<DrawingEditor>) -> f64 {
    let t = Instant::now();
    for _ in 0..3000 {
        cx.run_until_parked();
        cx.executor().advance_clock(Duration::from_millis(50));
        draw(cx, window);
        if cx.update(|cx| editor.read(cx).real_sheet_in() && editor.read(cx).status.is_none()) {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    t.elapsed().as_secs_f64() * 1e3
}

/// A bracket (a plate with a boss and four holes) and a drawing of 20 views of it in a 5 × 4 grid on ISO A2.
fn drawing(cx: &mut TestAppContext) -> (AnyWindowHandle, Entity<DrawingEditor>) {
    cx.update(gpui_kit::init);
    cx.update(|cx| gpui_kit::component::Theme::global_mut(cx).font_family = "Roboto".into());
    crate::drawing_hooks::register_for_tests();
    let dir = std::env::temp_dir().join(format!("noshape-drawing-bench-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("views.noshape");
    for ext in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{ext}", path.display()));
    }
    let mut ctx = Ctx::new();
    call(&mut ctx, "document_create", json!({ "path": path.to_str().unwrap() }));
    let sk = call(&mut ctx, "sketch_create", json!({ "plane": "plane:top" }))["sketch"].clone();
    let d = call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "tool": "corner_rectangle", "points": [[0, 0], [80, 50]] }));
    call(&mut ctx, "feature_add", json!({ "type": "extrude", "params": { "entities": [d["regions"][0]["id"].clone()], "depth": "8 mm" } }));
    let sk = call(&mut ctx, "sketch_create", json!({ "plane": "plane:top" }))["sketch"].clone();
    let d = call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "tool": "circle", "points": [[40, 25], [52, 25]] }));
    call(&mut ctx, "feature_add", json!({ "type": "extrude", "params": { "entities": [d["regions"][0]["id"].clone()], "depth": "30 mm", "operationType": "ADD" } }));
    for (x, y) in [(8, 8), (72, 8), (8, 42), (72, 42)] {
        let sk = call(&mut ctx, "sketch_create", json!({ "plane": "plane:top" }))["sketch"].clone();
        let d = call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "tool": "circle", "points": [[x, y], [x + 3, y]] }));
        call(&mut ctx, "feature_add", json!({ "type": "extrude", "params": { "entities": [d["regions"][0]["id"].clone()], "endBound": "BLIND", "symmetric": true, "depth": "40 mm", "operationType": "REMOVE" } }));
    }
    let dr = call(&mut ctx, "drawing_create", json!({ "source": "Part Studio 1", "template": "ISO A2" }))["drawing"].clone();
    let orientations = ["front", "top", "right", "isometric", "back", "bottom", "left"];
    for i in 0..20 {
        let (col, row) = (i % 5, i / 5);
        call(&mut ctx, "drawing_insert_view", json!({ "drawing": dr, "source": "Part Studio 1", "orientation": orientations[i % orientations.len()], "position": [60 + col * 110, 70 + row * 100], "scale": 0.8 }));
    }
    drop(ctx);
    open(cx, &path)
}

/// The document's drawing in an editor (its views not made yet).
fn open(cx: &mut TestAppContext, path: &std::path::Path) -> (AnyWindowHandle, Entity<DrawingEditor>) {
    let session = crate::session::Session::open(path).unwrap();
    let tab = session.doc.tabs.iter().find(|t| matches!(t.content, noshape_model::TabContent::Drawing(_))).unwrap().id;
    let gpu = Gpu::headless().unwrap();
    let (window, editor) = cx.update(|cx| {
        let doc = cx.new(|_| DocModel::new(session));
        let bounds = Bounds { origin: Point::default(), size: size(px(1600.), px(1000.)) };
        let mut editor = None;
        let (w, _) = crate::os_menu::open_window(WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() }, cx, |window, cx| {
            let e = cx.new(|cx| DrawingEditor::new(doc, tab, gpu, window, cx));
            editor = Some(e.clone());
            e
        })
        .unwrap();
        (w, editor.unwrap())
    });
    (window, editor)
}

#[test]
#[ignore = "T2: bench, a drawing of 20 views (prints BENCH lines)"]
fn drawing_of_20_views() {
    bench::run(|cx| {
        let (window, editor) = drawing(cx);
        let t = Instant::now();
        draw(cx, window);
        println!("BENCH {CASE} | open (the editor and its first frame): {:.1} ms", t.elapsed().as_secs_f64() * 1e3);
        println!("BENCH {CASE} | its 20 views made (background): {:.0} ms", settle(cx, window, &editor));
        assert!(cx.update(|cx| editor.read(cx).built.as_ref().map_or(0, |b| b.placements.len())) >= 20, "20 views placed");
        let mut fits = Vec::new();
        let mut step = |name: &str, t: bench::Times| fits.push((name.to_string(), bench::report(CASE, name, &t)));
        step("a frame of the sheet", bench::time(20, || {
            cx.update(|cx| editor.update(cx, |_, cx| cx.notify()));
            draw(cx, window);
        }));
        let mut zoom_in = true;
        step("zoom (a wheel notch about the middle)", bench::time(20, || {
            cx.update(|cx| editor.update(cx, |e, cx| e.zoom_by(if zoom_in { 1.2 } else { 1.0 / 1.2 }, [800.0, 500.0], cx)));
            zoom_in = !zoom_in;
            draw(cx, window);
        }));
        let t = Instant::now();
        cx.run_until_parked();
        println!("BENCH {CASE} | the zoomed lines remade (background): {:.0} ms", t.elapsed().as_secs_f64() * 1e3);
        let mut k = 0;
        step("pan (a tenth of the canvas)", bench::time(20, || {
            k += 1;
            cx.update(|cx| editor.update(cx, |e, cx| e.pan_step(if k % 2 == 0 { 1.0 } else { -1.0 }, 0.0, cx)));
            draw(cx, window);
        }));
        step("Update: the click and its frame", bench::time(1, || {
            cx.update(|cx| editor.update(cx, |e, cx| e.update_drawing(cx)));
            draw(cx, window);
        }));
        assert!(cx.update(|cx| editor.read(cx).status.is_some()), "Update shows that it is working");
        println!("BENCH {CASE} | Update's 20 views remade (background): {:.0} ms", settle(cx, window, &editor));
        let slow: Vec<&str> = fits.iter().filter(|(_, ok)| !ok).map(|(n, _)| n.as_str()).collect();
        assert!(slow.is_empty(), "over a frame: {slow:?}");
    });
}

/// Live's "dx big asm" Part Studio 1 (behaviours 11-02: 1000 parts, its views 3.7–4.0 s each, the drawing redrawn
/// 6.4 s after Update): tests/perf/parts-1000.noshape with Create drawing's suggested views, opened cold (the Part
/// Studio regenerated by the drawing), then its pattern suppressed and Update.
#[test]
#[ignore = "T2: bench, a drawing of 1000 parts (prints BENCH lines)"]
fn drawing_of_1000_parts() {
    const CASE: &str = "drawing (1000 parts, 4 views)";
    bench::run(|cx| {
        cx.update(gpui_kit::init);
        cx.update(|cx| gpui_kit::component::Theme::global_mut(cx).font_family = "Roboto".into());
        crate::drawing_hooks::register_for_tests();
        let dir = std::env::temp_dir().join(format!("noshape-drawing-bench-1000-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("parts.noshape");
        std::fs::copy(concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/perf/parts-1000.noshape"), &path).unwrap();
        for ext in ["-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{ext}", path.display()));
        }
        {
            let mut ctx = Ctx::new();
            call(&mut ctx, "document_open", json!({ "path": path.to_str().unwrap() }));
            let d = call(&mut ctx, "drawing_create", json!({ "source": "Part Studio 1", "template": "ANSI_A_INCH.dwt", "suggested_views": true }));
            assert_eq!(d["views"].as_array().map_or(0, |v| v.len()), 4, "{d}");
        }
        noshape_ops::drawing::drop_cached_views();
        let at = Instant::now();
        let (window, editor) = open(cx, &path);
        draw(cx, window);
        println!("BENCH {CASE} | open (the editor and its first frame): {:.1} ms", at.elapsed().as_secs_f64() * 1e3);
        let made = settle(cx, window, &editor);
        let shown = at.elapsed().as_secs_f64() * 1e3;
        println!("BENCH {CASE} | its views made (background): {made:.0} ms; all views shown {shown:.0} ms after the open (live 3.7–4.0 s a view)");
        assert!(cx.update(|cx| editor.read(cx).built.as_ref().map_or(0, |b| b.placements.len())) >= 4, "4 views placed");
        let mut fits = Vec::new();
        let mut step = |name: &str, t: bench::Times| fits.push((name.to_string(), bench::report(CASE, name, &t)));
        step("a frame of the sheet", bench::time(20, || {
            cx.update(|cx| editor.update(cx, |_, cx| cx.notify()));
            draw(cx, window);
        }));
        let mut k = 0;
        step("pan (a tenth of the canvas)", bench::time(20, || {
            k += 1;
            cx.update(|cx| editor.update(cx, |e, cx| e.pan_step(if k % 2 == 0 { 1.0 } else { -1.0 }, 0.0, cx)));
            draw(cx, window);
        }));
        // An edit in the Part Studio (the pattern suppressed: one pin left), then Update.
        let doc = cx.update(|cx| editor.read(cx).doc.clone());
        let studio = cx.update(|cx| doc.read(cx).session.doc.tabs.iter().find(|t| matches!(t.content, noshape_model::TabContent::PartStudio(_))).unwrap().id);
        cx.update(|cx| doc.update(cx, |d, cx| d.edit_studio(studio, "Suppress Linear pattern 1", cx, |ps| {
            let f = ps.features.iter_mut().rev().find(|f| f.type_id == "linearPattern").unwrap();
            f.suppressed = true;
        })));
        let at = Instant::now();
        let click = bench::time(1, || {
            cx.update(|cx| editor.update(cx, |e, cx| e.update_drawing(cx)));
            draw(cx, window);
        });
        bench::report(CASE, "Update: the click and its frame", &click);
        let remade = settle(cx, window, &editor);
        println!("BENCH {CASE} | Update's views remade (background): {remade:.0} ms; shown {:.0} ms after the click (live 6.4 s)", at.elapsed().as_secs_f64() * 1e3);
        let slow: Vec<&str> = fits.iter().filter(|(_, ok)| !ok).map(|(n, _)| n.as_str()).collect();
        assert!(slow.is_empty(), "over a frame: {slow:?}");
        if !cfg!(debug_assertions) {
            // The click assembles the 1000-part sheet (its BOM, four views) on the UI thread: held to human scale
            // (DECISIONS: gaps from 100 ms), not to a frame.
            assert!(click.median() < 100., "Update's click {:.0} ms", click.median());
            assert!(shown < 8000. && remade < 12800., "all views shown {shown:.0} ms, Update {remade:.0} ms (live: 3.7–4.0 s a view, Update 6.4 s)");
        }
    });
}
