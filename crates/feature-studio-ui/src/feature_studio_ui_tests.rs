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

/// Live 2480: the Feature Studio's undo steps are its saves; a run of typing up to a pause is one, and at a key a
/// second each key is one.
#[gpui::test]
fn a_typed_run_undoes_in_one_step(cx: &mut TestAppContext) {
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
    assert_eq!(text(cx), start, "the whole run is one step");
    // Ctrl+Z before the pause: the run is saved first, then undone whole.
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_new(w, cx))).unwrap();
    for k in ["p", "q"] {
        type_key(cx, window, k);
    }
    type_key(cx, window, "secondary-z");
    pause(cx, window, 2000);
    assert_eq!(text(cx), start, "Ctrl+Z right after typing");
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

/// Live2 studios.json featureStudioEditor (Ace's keys, fs0.json): Ctrl+/ comments the line with `// ` and takes it
/// away again, Alt+↑ moves the line up, Ctrl+Shift+D duplicates it, Ctrl+D removes it; each one undo step.
#[gpui::test]
fn ace_line_keys_comment_move_duplicate_and_remove(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        super::bind_keys(cx);
    });
    let dir = std::env::temp_dir().join(format!("noshape-ui-fs-lines-{}", std::process::id()));
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
    let lines = |cx: &mut TestAppContext| {
        pause(cx, window, 2000);
        let t = cx.update(|cx| text_of(&doc, tab, cx)).unwrap();
        t.strip_prefix(start.as_str()).map(str::to_string).unwrap_or(t)
    };
    for k in ["\n", "a", "\n", "b"] {
        type_key(cx, window, if k == "\n" { "enter" } else { k });
    }
    assert_eq!(lines(cx), "\na\nb");
    type_key(cx, window, "secondary-/");
    assert_eq!(lines(cx), "\na\n// b");
    type_key(cx, window, "secondary-/");
    assert_eq!(lines(cx), "\na\nb");
    type_key(cx, window, "alt-up");
    assert_eq!(lines(cx), "\nb\na");
    type_key(cx, window, "secondary-shift-d");
    assert_eq!(lines(cx), "\nb\nb\na");
    type_key(cx, window, "secondary-d");
    assert_eq!(lines(cx), "\nb\na");
    cx.update(|cx| doc.update(cx, |d, cx| d.undo(cx)));
    assert_eq!(lines(cx), "\nb\nb\na", "Ctrl+D is one undo step");
    let _ = std::fs::remove_dir_all(&dir);
}
