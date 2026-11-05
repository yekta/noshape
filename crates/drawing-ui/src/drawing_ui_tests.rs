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
        call(&mut ctx, "drawing_create", json!({ "source": "Part Studio 1", "template": "ISO A3" }));
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
