//! UI keystroke tests of the Feature Studio on GPUI's test platform (docs/TESTS.md "UI keystroke tests"): the real
//! view in a test window, keys through the window.

use super::{FeatureStudioView, text_of};
use crate::doc_model::DocModel;
use gpui_kit::{AnyWindowHandle, AppContext as _, Bounds, Entity, IntoElement, Keystroke, ParentElement as _, Point, Render, Styled as _, TestAppContext, Window, WindowBounds, WindowOptions, div, px, size};
use std::time::Duration;

struct Workspace(Entity<FeatureStudioView>);

impl Render for Workspace {
    fn render(&mut self, _: &mut Window, _: &mut gpui_kit::Context<Self>) -> impl IntoElement {
        div().size_full().child(self.0.clone())
    }
}

fn draw(cx: &mut TestAppContext, window: AnyWindowHandle) {
    cx.update_window(window, |_, w, cx| {
        w.draw(cx).clear(cx);
    })
    .unwrap();
    cx.run_until_parked();
}

fn type_key(cx: &mut TestAppContext, window: AnyWindowHandle, key: &str) {
    cx.update_window(window, |_, w, cx| {
        w.dispatch_keystroke(Keystroke::parse(key).unwrap(), cx);
    })
    .unwrap();
    cx.run_until_parked();
}

fn pause(cx: &mut TestAppContext, window: AnyWindowHandle, ms: u64) {
    cx.executor().advance_clock(Duration::from_millis(ms));
    draw(cx, window);
}

/// Live 2480: the Feature Studio's undo steps are its saves; a fast run of typing is two (its first character, then
/// the rest), and at a key a second each key is one.
#[gpui::test]
fn a_typed_run_undoes_to_its_first_character_then_away(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let dir = std::env::temp_dir().join(format!("noshape-ui-fs-undo-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("ui.noshape");
    let _ = std::fs::remove_file(&path);
    {
        use serde_json::json;
        let mut ctx = noshape_ops::Ctx::new();
        let call = |ctx: &mut noshape_ops::Ctx, tool: &str, args: serde_json::Value| noshape_ops::registry::run(ctx, noshape_ops::registry::by_tool_name(tool).unwrap(), &args).unwrap_or_else(|e| panic!("{tool}: {e:?}")).value;
        call(&mut ctx, "document_create", json!({ "path": path.to_str().unwrap() }));
        call(&mut ctx, "tab_create", json!({ "kind": "featurestudio" }));
    }
    let session = crate::session::Session::open(&path).unwrap();
    let tab = session.doc.tabs.iter().find(|t| matches!(t.content, noshape_model::TabContent::FeatureStudio(_))).unwrap().id;
    let (window, view, doc) = cx.update(|cx| {
        let doc = cx.new(|_| DocModel::new(session));
        let bounds = Bounds { origin: Point::default(), size: size(px(1600.), px(900.)) };
        let mut view = None;
        let d = doc.clone();
        let (w, _) = gpui_kit::open_window(WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() }, cx, |window, cx| {
            let v = cx.new(|cx| FeatureStudioView::new(d, tab, window, cx));
            view = Some(v.clone());
            cx.new(|_| Workspace(v))
        })
        .unwrap();
        (w, view.unwrap(), doc)
    });
    draw(cx, window);
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_new(w, cx))).unwrap();
    let start = cx.update(|cx| text_of(&doc, tab, cx)).unwrap();
    pause(cx, window, 2000);
    for k in ["a", "b", "c"] {
        type_key(cx, window, k);
    }
    pause(cx, window, 2000);
    let text = |cx: &mut TestAppContext| cx.update(|cx| text_of(&doc, tab, cx)).unwrap();
    assert_eq!(text(cx), format!("{start}abc"));
    cx.update(|cx| doc.update(cx, |d, cx| d.undo(cx)));
    draw(cx, window);
    assert_eq!(text(cx), format!("{start}a"), "the first character is a step of its own");
    cx.update(|cx| doc.update(cx, |d, cx| d.undo(cx)));
    draw(cx, window);
    assert_eq!(text(cx), start);
    // A key a second: each its own step.
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_new(w, cx))).unwrap();
    for k in ["x", "y"] {
        type_key(cx, window, k);
        pause(cx, window, 1000);
    }
    assert_eq!(text(cx), format!("{start}xy"));
    cx.update(|cx| doc.update(cx, |d, cx| d.undo(cx)));
    draw(cx, window);
    assert_eq!(text(cx), format!("{start}x"));
    let _ = std::fs::remove_dir_all(&dir);
}
