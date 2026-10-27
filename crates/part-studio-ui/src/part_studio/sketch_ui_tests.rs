//! UI keystroke tests of sketch mode on GPUI's test platform (docs/TESTS.md
//! "UI keystroke tests"): the real Part Studio view in a test window, the GPU
//! a no-op stand-in, clicks as the viewport's tool events, keys through GPUI.

use super::super::PartStudioView;
use crate::doc_model::DocModel;
use crate::gpu::Gpu;
use crate::sketch_mode::Tool;
use crate::viewport::ViewportEvent;
use glam::DVec2;
use gpui_kit::{AnyWindowHandle, AppContext as _, Bounds, Entity, Keystroke, Modifiers, Point, TestAppContext, WindowBounds, WindowOptions, px, size};
use noshape_model::reference::{DatumPlane, Reference};

/// A fresh document's Part Studio in a test window, sketching on Top with `tool`.
fn sketch_with(cx: &mut TestAppContext, name: &str, tool: Tool) -> (AnyWindowHandle, Entity<PartStudioView>) {
    cx.update(gpui_kit::init);
    crate::part_studio_hooks::register_for_tests();
    let dir = std::env::temp_dir().join(format!("noshape-ui-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("ui.noshape");
    let _ = std::fs::remove_file(&path);
    let session = crate::session::Session::create(&path, crate::user_units::new_document()).unwrap();
    let tab = session.doc.tabs.iter().find(|t| matches!(t.content, noshape_model::TabContent::PartStudio(_))).unwrap().id;
    let gpu = Gpu::headless().unwrap();
    let (window, view) = cx.update(|cx| {
        let doc = cx.new(|_| DocModel::new(session));
        let bounds = Bounds { origin: Point::default(), size: size(px(1200.), px(800.)) };
        gpui_kit::open_window(WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() }, cx, |window, cx| cx.new(|cx| PartStudioView::new(doc, tab, gpu, window, cx))).unwrap()
    });
    draw(cx, window);
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            v.selection = vec![Reference::Plane(DatumPlane::Top)];
            v.start_feature("sketch", w, cx);
        });
    })
    .unwrap();
    // The window active, as the user's is (focus events need it); the
    // sketch editor comes up with the next frame.
    cx.update_window(window, |_, w, _| w.activate_window()).unwrap();
    draw(cx, window);
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| v.activate_tool(tool, w, cx));
        let focus = view.read(cx).focus.clone();
        w.focus(&focus, cx);
    })
    .unwrap();
    draw(cx, window);
    (window, view)
}

/// A frame (layout, paint: a focused input registers its text handler).
fn draw(cx: &mut TestAppContext, window: AnyWindowHandle) {
    cx.update_window(window, |_, w, cx| {
        w.draw(cx).clear(cx);
    })
    .unwrap();
    cx.run_until_parked();
}

/// A click of the tool at viewport pixel `at` (what the viewport sends for a
/// press and release without a drag).
fn click(cx: &mut TestAppContext, window: AnyWindowHandle, view: &Entity<PartStudioView>, at: (f64, f64)) {
    let at = DVec2::new(at.0, at.1);
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            let vp = v.viewport.clone();
            for ev in [
                ViewportEvent::PointerMove { position: at, modifiers: Modifiers::default() },
                ViewportEvent::ToolDown { position: at, modifiers: Modifiers::default(), click_count: 1 },
                ViewportEvent::ToolUp { position: at, modifiers: Modifiers::default() },
            ] {
                v.on_viewport_event(&vp, &ev, w, cx);
            }
        });
    })
    .unwrap();
    draw(cx, window);
}

/// Keys as the user types them (a character key carries its text), a frame
/// after each.
fn keys(cx: &mut TestAppContext, window: AnyWindowHandle, keys: &[&str]) {
    for k in keys {
        let mut ks = Keystroke::parse(k).unwrap();
        if k.chars().count() == 1 {
            ks.key_char = Some(k.to_string());
        }
        cx.update_window(window, |_, w, cx| {
            w.dispatch_keystroke(ks, cx);
        })
        .unwrap();
        draw(cx, window);
    }
}

/// The sketch's dimension values (metres), sorted.
fn dimensions(cx: &mut TestAppContext, view: &Entity<PartStudioView>) -> Vec<f64> {
    cx.update(|cx| {
        let data = view.read(cx).sketch_data(cx).expect("sketching");
        let mut v: Vec<f64> = data.sketch.constraints().filter(|c| c.driving).filter_map(|c| c.value()).collect();
        v.sort_by(f64::total_cmp);
        v
    })
}

fn near(a: &[f64], b: &[f64]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-9)
}

/// OTF-FIRST-KEY: Line, click, click, `5` `0` Enter: the value box takes the
/// typed digits from the first key, and Enter makes a 50 mm dimension (the
/// first digit used to come up selected, so `0` replaced it).
#[gpui::test]
fn line_value_box_takes_the_first_key(cx: &mut TestAppContext) {
    let (window, view) = sketch_with(cx, "line", Tool::Line);
    click(cx, window, &view, (500., 400.));
    click(cx, window, &view, (640., 360.));
    keys(cx, window, &["5", "0", "enter"]);
    assert!(near(&dimensions(cx, &view), &[0.05]), "{:?}", dimensions(cx, &view));
}

/// OTF-FIRST-KEY: corner rectangle, `30` Enter `20` Enter: 30 × 20 (the
/// width's first key used to be lost, and `20` went to the width).
#[gpui::test]
fn rectangle_value_boxes_take_width_then_height(cx: &mut TestAppContext) {
    let (window, view) = sketch_with(cx, "rect", Tool::CornerRect);
    click(cx, window, &view, (500., 300.));
    click(cx, window, &view, (650., 400.));
    keys(cx, window, &["3", "0", "enter", "2", "0", "enter"]);
    assert!(near(&dimensions(cx, &view), &[0.02, 0.03]), "{:?}", dimensions(cx, &view));
}
