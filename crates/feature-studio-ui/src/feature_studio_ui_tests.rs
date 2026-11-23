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

/// Live tools-1119: a new error's squiggle and gutter marker show ~900 ms after the last key (858–946), never while
/// typing; the key fixing it clears it ~600 ms after (fv-1121 recheck: the underline 582–757 ms, the gutter 658–783).
#[gpui::test]
fn a_new_error_shows_800_ms_after_the_last_key_and_its_fix_clears_at_580_ms(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let dir = std::env::temp_dir().join(format!("noshape-ui-fs-check-{}", std::process::id()));
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
    pause(cx, window, 2000);
    // Drawn (squiggle, gutter) and counted (live's recorder reads the count right after the typing).
    let error = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).shown.iter().any(|n| n.error));
    let counted = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).notices.iter().any(|n| n.error));
    let squiggles = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).editor.read(cx).diagnostics().map_or(0, |d| d.len()));
    assert!(!error(cx), "the new studio has no error");
    let clean = squiggles(cx);
    for k in 0..3 {
        if k > 0 {
            pause(cx, window, 500);
        }
        type_key(cx, window, "x");
    }
    assert!(!error(cx), "no error while typing, a key every 500 ms");
    for _ in 0..75 {
        pause(cx, window, 10);
    }
    assert!(!error(cx), "none 750 ms after the last key");
    assert!(counted(cx), "but counted");
    assert!(cx.update(|cx| view.read(cx).ui_busy()), "the ui-parity driver waits for it");
    assert_eq!(squiggles(cx), clean, "no squiggle 750 ms after the last key");
    pause(cx, window, 100);
    assert!(error(cx), "the error shows by 850 ms");
    assert!(!cx.update(|cx| view.read(cx).ui_busy()), "and the driver reads it");
    assert!(squiggles(cx) > clean, "with its squiggle");
    for k in ["backspace", "backspace", "backspace"] {
        type_key(cx, window, k);
    }
    for _ in 0..55 {
        pause(cx, window, 10);
    }
    assert!(error(cx), "still showing 550 ms after the fix");
    pause(cx, window, 50);
    assert!(!error(cx), "gone 600 ms after the fix");
    assert_eq!(squiggles(cx), clean, "the squiggle too");
    let _ = (doc, tab);
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
    // Ctrl+\ from after a ')' to after its '(' (the caret at the text's end).
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_new(w, cx))).unwrap();
    for k in ["f", "(", "x", ")"] {
        type_key(cx, window, k);
    }
    type_key(cx, window, "secondary-\\");
    type_key(cx, window, "y");
    assert!(lines(cx).ends_with("f(yx)"), "{}", lines(cx));
    // Ctrl+Shift+↑ (a Mac: Option+Shift+↑) nudges the number at the caret; Alt+Shift+X (Ctrl+T) transposes.
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_new(w, cx))).unwrap();
    for k in [" ", "4", "1"] {
        type_key(cx, window, k);
    }
    type_key(cx, window, if cfg!(target_os = "macos") { "alt-shift-up" } else { "ctrl-shift-up" });
    assert!(lines(cx).ends_with(" 42"), "{}", lines(cx));
    type_key(cx, window, if cfg!(target_os = "macos") { "ctrl-t" } else { "alt-shift-x" });
    assert!(lines(cx).ends_with(" 24"), "{}", lines(cx));
    // ui-script's ace_word (live's recorder, qa UIS-EMPTY-NON-PS): a whole word of the code, at its middle.
    draw(cx, window);
    let at = |cx: &mut TestAppContext, t: serde_json::Value| cx.update(|cx| view.read(cx).ui_target_point(&t, cx));
    let import = at(cx, serde_json::json!({ "ace_word": "import" }));
    assert!(matches!(import, Some(Ok(p)) if p.x > px(0.) && p.y > px(0.)), "{import:?}");
    assert!(matches!(at(cx, serde_json::json!({ "ace_word": "mport" })), Some(Err(_))), "only whole words");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Live2 (REQUESTS 11-14 00:43) and live fsvs-feel-1118: Ace's list shows 232 ms (median) after a letter with matches, not on the keystroke.
#[gpui::test]
fn the_completion_list_shows_230_ms_after_the_key(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        super::bind_keys(cx);
    });
    let dir = std::env::temp_dir().join(format!("noshape-ui-fs-ac-{}", std::process::id()));
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
    let (window, view) = cx.update(|cx| {
        let doc = cx.new(|_| DocModel::new(session));
        let bounds = Bounds { origin: Point::default(), size: size(px(1600.), px(900.)) };
        let mut view = None;
        let (w, _) = gpui_kit::open_window(WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() }, cx, |window, cx| {
            let v = cx.new(|cx| FeatureStudioView::new(doc, tab, window, cx));
            view = Some(v.clone());
            cx.new(|_| Workspace(v))
        })
        .unwrap();
        (w, view.unwrap())
    });
    draw(cx, window);
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_new(w, cx))).unwrap();
    let open = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).editor.read(cx).completion_menu_state().open);
    type_key(cx, window, "enter");
    type_key(cx, window, "o");
    assert!(!open(cx), "not on the keystroke");
    pause(cx, window, 210);
    assert!(!open(cx), "not 210 ms after");
    pause(cx, window, 40);
    assert!(open(cx), "by 250 ms (live fsvs-feel-1118: 232 ms median)");
    // Live fs-editor-feel-1122: the pointer over the open list doesn't move its highlight; the wheel there leaves it open.
    {
        use gpui_kit::{Modifiers, ScrollDelta, ScrollWheelEvent, VisualTestContext, point};
        let row = || gpui_kit::component::input::COMPLETION_SELECTED_ROW.load(std::sync::atomic::Ordering::Relaxed);
        let caret = cx.update(|cx| {
            let ed = view.read(cx).editor.read(cx);
            let c = ed.cursor();
            ed.range_to_bounds(&(c - 1..c))
        }).expect("laid out");
        let before = row();
        let over = point(caret.left() + px(40.), caret.bottom() + px(100.));
        VisualTestContext::from_window(window, cx).simulate_mouse_move(over, None, Modifiers::default());
        pause(cx, window, 100);
        assert!(open(cx), "the pointer over the list: still open");
        assert_eq!(row(), before, "the hovered row isn't highlighted");
        VisualTestContext::from_window(window, cx).simulate_event(ScrollWheelEvent { position: over, delta: ScrollDelta::Lines(point(0., -3.)), wheel_ticks: Some(point(0., -1.)), ..Default::default() });
        pause(cx, window, 100);
        assert!(open(cx), "a wheel notch over the list: still open");
        assert_eq!(row(), before, "the wheel doesn't move the highlight");
    }
    // Live vs-fs-feel-1117: Tab doesn't accept (the list closes, a tab goes in); Esc closes it and keeps the text.
    let value = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).editor.read(cx).value().to_string());
    let bare = |s: &str| s.split_whitespace().collect::<String>();
    let before = value(cx);
    type_key(cx, window, "tab");
    pause(cx, window, 300);
    let after = value(cx);
    assert!(!open(cx), "Tab closes the list");
    assert!(after.len() > before.len() && bare(&after) == bare(&before), "Tab inserts only whitespace");
    type_key(cx, window, "o");
    pause(cx, window, 250);
    assert!(open(cx));
    let before = value(cx);
    type_key(cx, window, "escape");
    pause(cx, window, 300);
    assert!(!open(cx), "Esc closes the list");
    assert_eq!(value(cx), before, "Esc keeps the typed text");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Live2 manual-4: Ctrl (Cmd) over a defined identifier shows it as a link at once, in either order, and releasing
/// the key takes it away; std's gets an arrow; nothing over an import; a click on the link goes to the declaration.
#[gpui::test]
fn ctrl_over_a_defined_identifier_shows_a_link_that_goes_to_it(cx: &mut TestAppContext) {
    use gpui_kit::{Modifiers, VisualTestContext};
    cx.update(gpui_kit::init);
    let dir = std::env::temp_dir().join(format!("noshape-ui-fs-ctrl-link-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("ui.noshape");
    let _ = std::fs::remove_file(&path);
    let source = "FeatureScript 2581;\nimport(path : \"onshape/std/geometry.fs\", version : \"2581.0\");\n\nconst plateLength = 10;\n\nfunction doubled(n) { return 2 * n; }\n\nexport const f = function() { var x = doubled(3) * plateLength; opExtrude; };\n";
    {
        use serde_json::json;
        let mut ctx = noshape_ops::Ctx::new();
        let call = |ctx: &mut noshape_ops::Ctx, tool: &str, args: serde_json::Value| noshape_ops::registry::run(ctx, noshape_ops::registry::by_tool_name(tool).unwrap(), &args).unwrap_or_else(|e| panic!("{tool}: {e:?}")).value;
        call(&mut ctx, "document_create", json!({ "path": path.to_str().unwrap() }));
        call(&mut ctx, "tab_create", json!({ "kind": "featurestudio" }));
        call(&mut ctx, "featurestudio_edit", json!({ "tab": "Feature Studio 1", "source": source }));
    }
    let session = crate::session::Session::open(&path).unwrap();
    let tab = session.doc.tabs.iter().find(|t| matches!(t.content, noshape_model::TabContent::FeatureStudio(_))).unwrap().id;
    let (window, view) = cx.update(|cx| {
        let doc = cx.new(|_| DocModel::new(session));
        let bounds = Bounds { origin: Point::default(), size: size(px(1600.), px(900.)) };
        let mut view = None;
        let (w, _) = gpui_kit::open_window(WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() }, cx, |window, cx| {
            let v = cx.new(|cx| FeatureStudioView::new(doc, tab, window, cx));
            view = Some(v.clone());
            cx.new(|_| Workspace(v))
        })
        .unwrap();
        (w, view.unwrap())
    });
    draw(cx, window);
    let last = source.rfind("doubled(3)").unwrap();
    let at = |cx: &mut TestAppContext, needle_at: usize, len: usize| {
        cx.update(|cx| view.read(cx).editor.read(cx).range_to_bounds(&(needle_at..needle_at + len))).expect("laid out").center()
    };
    let (use_at, std_at, import_at) = (at(cx, last, 7), at(cx, source.find("opExtrude").unwrap(), 9), at(cx, source.find("geometry").unwrap(), 8));
    let link = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).ctrl_link.as_ref().map(|l| (l.name.clone(), l.std)));
    let ctrl = Modifiers::secondary_key();
    // (The code has the keyboard, as after a click in it.)
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.editor.update(cx, |e, cx| e.focus(w, cx)))).unwrap();
    let mut vcx = VisualTestContext::from_window(window, cx);
    vcx.simulate_mouse_move(use_at, None, Modifiers::default());
    draw(cx, window);
    assert_eq!(link(cx), None, "no Ctrl: no link");
    VisualTestContext::from_window(window, cx).simulate_modifiers_change(ctrl);
    draw(cx, window);
    assert_eq!(link(cx), Some(("doubled".into(), false)), "hover, then Ctrl: the link");
    VisualTestContext::from_window(window, cx).simulate_modifiers_change(Modifiers::default());
    draw(cx, window);
    assert_eq!(link(cx), None, "Ctrl released: gone");
    for (p, want) in [(std_at, Some(("opExtrude".to_string(), true))), (import_at, None), (use_at, Some(("doubled".to_string(), false)))] {
        VisualTestContext::from_window(window, cx).simulate_mouse_move(p, None, ctrl);
        draw(cx, window);
        assert_eq!(link(cx), want, "Ctrl, then hover");
    }
    let mut vcx = VisualTestContext::from_window(window, cx);
    let on = vcx.debug_bounds("fs-ctrl-link").expect("the link drawn").center();
    vcx.simulate_click(on, ctrl);
    draw(cx, window);
    let caret = cx.update(|cx| view.read(cx).editor.read(cx).cursor_position());
    assert_eq!((caret.line, caret.character), (5, 9), "at `doubled` in its declaration");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Live behaviours "feel coverage (11-20)" F2 / F3 (Ace): a "}" typed on an indented blank line outdents to its "{"
/// line's indent; a triple-click selects the line with its newline (the caret at the next line's start); the caret
/// beside a bracket boxes it and its partner (1 px rgb(192, 192, 192)), none away from brackets; a wheel notch scrolls
/// 72 px (Linux: Chrome's 120 px deltaY × Ace's 0.6).
#[gpui::test]
fn ace_feel_outdent_triple_click_bracket_boxes_and_wheel(cx: &mut TestAppContext) {
    use gpui_kit::{Modifiers, MouseButton, MouseDownEvent, MouseUpEvent, ScrollDelta, ScrollWheelEvent, VisualTestContext, point};
    cx.update(gpui_kit::init);
    let dir = std::env::temp_dir().join(format!("noshape-ui-fs-ace-feel-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("ui.noshape");
    let _ = std::fs::remove_file(&path);
    let head = "function f()\n{\n    if (true)\n    {\n        x = g(1);\n        ";
    let source = format!("{head}\n{}", "// filler\n".repeat(120));
    {
        use serde_json::json;
        let mut ctx = noshape_ops::Ctx::new();
        let call = |ctx: &mut noshape_ops::Ctx, tool: &str, args: serde_json::Value| noshape_ops::registry::run(ctx, noshape_ops::registry::by_tool_name(tool).unwrap(), &args).unwrap_or_else(|e| panic!("{tool}: {e:?}")).value;
        call(&mut ctx, "document_create", json!({ "path": path.to_str().unwrap() }));
        call(&mut ctx, "tab_create", json!({ "kind": "featurestudio" }));
        call(&mut ctx, "featurestudio_edit", json!({ "tab": "Feature Studio 1", "source": source }));
    }
    let session = crate::session::Session::open(&path).unwrap();
    let tab = session.doc.tabs.iter().find(|t| matches!(t.content, noshape_model::TabContent::FeatureStudio(_))).unwrap().id;
    let (window, view) = cx.update(|cx| {
        let doc = cx.new(|_| DocModel::new(session));
        let bounds = Bounds { origin: Point::default(), size: size(px(1600.), px(900.)) };
        let mut view = None;
        let (w, _) = gpui_kit::open_window(WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() }, cx, |window, cx| {
            let v = cx.new(|cx| FeatureStudioView::new(doc, tab, window, cx));
            view = Some(v.clone());
            cx.new(|_| Workspace(v))
        })
        .unwrap();
        (w, view.unwrap())
    });
    draw(cx, window);
    let value = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).editor.read(cx).value().to_string());
    let select = |cx: &mut TestAppContext, at: usize| cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.editor.update(cx, |e, cx| {
        e.focus(w, cx);
        e.set_selected_range(at..at, cx);
    }))).unwrap();
    let boxes = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).bracket_box.as_ref().map(|(c, _)| c.get_ranges(cx)).unwrap_or_default());
    // The "}" on the 8-space blank line goes to the inner "{" line's 4 spaces.
    select(cx, head.len());
    VisualTestContext::from_window(window, cx).simulate_input("}");
    draw(cx, window);
    assert!(value(cx).starts_with("function f()\n{\n    if (true)\n    {\n        x = g(1);\n    }\n"), "{:?}", &value(cx)[..80]);
    // The caret after it: both braces boxed (the "}" and the inner "{").
    let text = value(cx);
    let (inner_open, close) = (text.find("    {").unwrap() + 4, text.find("    }\n").unwrap() + 4);
    draw(cx, window);
    assert_eq!(boxes(cx), vec![close..close + 1, inner_open..inner_open + 1]);
    // Right after ")" and right before "(": the parentheses; on plain text: none.
    let open = text.find("g(1)").unwrap() + 1;
    select(cx, open + 3);
    draw(cx, window);
    assert_eq!(boxes(cx), vec![open + 2..open + 3, open..open + 1]);
    select(cx, open);
    draw(cx, window);
    assert_eq!(boxes(cx), vec![open..open + 1, open + 2..open + 3]);
    select(cx, 3);
    draw(cx, window);
    assert!(boxes(cx).is_empty(), "no bracket beside the caret");
    // "g" selected, the caret before "(": framed too (live fsm-1 f4, `import` double-clicked).
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.editor.update(cx, |e, cx| {
        e.focus(w, cx);
        e.set_selected_range(open - 1..open, cx);
    }))).unwrap();
    draw(cx, window);
    assert_eq!(boxes(cx), vec![open..open + 1, open + 2..open + 3], "a selection ending before a bracket");
    // Triple-click on line 3: that line and its newline, the caret on line 4.
    let line3 = text.find("    if").unwrap();
    let at = cx.update(|cx| view.read(cx).editor.read(cx).range_to_bounds(&(line3 + 4..line3 + 6))).expect("laid out").center();
    let mut vcx = VisualTestContext::from_window(window, cx);
    for n in 1..=3 {
        vcx.simulate_event(MouseDownEvent { button: MouseButton::Left, position: at, modifiers: Modifiers::default(), click_count: n, first_mouse: false });
        vcx.simulate_event(MouseUpEvent { button: MouseButton::Left, position: at, modifiers: Modifiers::default(), click_count: n });
    }
    draw(cx, window);
    let line4 = text[line3..].find('\n').unwrap() + line3 + 1;
    assert_eq!(cx.update(|cx| view.read(cx).editor.read(cx).selected_range()), line3..line4);
    // One wheel notch down.
    let before = cx.update(|cx| view.read(cx).editor.read(cx).scroll_offset().y);
    VisualTestContext::from_window(window, cx).simulate_event(ScrollWheelEvent { position: at, delta: ScrollDelta::Lines(point(0., -3.)), wheel_ticks: Some(point(0., -1.)), ..Default::default() });
    draw(cx, window);
    let moved = before - cx.update(|cx| view.read(cx).editor.read(cx).scroll_offset().y);
    let want = if cfg!(target_os = "linux") { 72. } else if cfg!(target_os = "windows") { 60. } else { f32::from(moved) };
    assert!((f32::from(moved) - want).abs() < 0.5, "one notch: {} px, want {want}", f32::from(moved));
    let _ = std::fs::remove_dir_all(&dir);
}

/// Live feel cov-1120 F1 / F2 (Ace's behaviours off): "(", "[" and '"' insert themselves alone, "(" over a selection
/// replaces it; Enter after "{" indents the new line 4 more and adds no "}".
#[gpui::test]
fn typing_pairs_nothing_and_enter_after_a_brace_indents_without_closing_it(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        super::bind_keys(cx);
    });
    let dir = std::env::temp_dir().join(format!("noshape-ui-fs-pairs-{}", std::process::id()));
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
    for k in ["enter", "(", "[", "\"", "backspace"] {
        type_key(cx, window, k);
    }
    assert_eq!(lines(cx), "\n([", "brackets and quotes alone; Backspace takes one character");
    for k in [" ", "a", "b", "c", "shift-left", "shift-left", "shift-left", "("] {
        type_key(cx, window, k);
    }
    assert_eq!(lines(cx), "\n([ (", "\"(\" replaces the selection");
    for k in ["secondary-a", "backspace", "{", "enter", "x", ";", "enter"] {
        type_key(cx, window, k);
    }
    // (Live's typed "}" then outdents to its "{" line: ticket FS-CLOSE-BRACE-OUTDENT.)
    assert_eq!(lines(cx), "{\n    x;\n    ", "Enter after an open brace: 4 more and no closing one; after a statement the same indent");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Live feel cov-1120 F3: a double-click selects the word under it, a triple-click its line.
#[gpui::test]
fn a_double_click_takes_the_word_and_a_triple_click_the_line(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        super::bind_keys(cx);
    });
    let dir = std::env::temp_dir().join(format!("noshape-ui-fs-clicks-{}", std::process::id()));
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
    for k in ["enter", "x", " ", "=", " ", "t", "r", "u", "e", ";", "enter", "y"] {
        type_key(cx, window, k);
    }
    pause(cx, window, 100);
    let word = start.len() + 5;
    let at = cx.update(|cx| view.read(cx).editor.read(cx).range_to_bounds(&(word..word + 4))).expect("laid out").center();
    let click = |cx: &mut TestAppContext, count: usize| {
        let mut v = gpui_kit::VisualTestContext::from_window(window, cx);
        let none = gpui_kit::Modifiers::default();
        v.simulate_event(gpui_kit::MouseDownEvent { button: gpui_kit::MouseButton::Left, position: at, modifiers: none, click_count: count, first_mouse: false });
        v.simulate_event(gpui_kit::MouseUpEvent { button: gpui_kit::MouseButton::Left, position: at, modifiers: none, click_count: count });
        draw(cx, window);
        cx.update(|cx| view.read(cx).editor.read(cx).selected_range())
    };
    click(cx, 1);
    assert_eq!(click(cx, 2), word..word + 4, "a double-click: \"true\"");
    // Live's triple-click takes the line's newline too (FS-TRIPLE-CLICK-NEWLINE).
    let line = start.len() + 1;
    assert_eq!(click(cx, 3), line..line + 10, "a triple-click: \"x = true;\\n\"");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Live workflow 26: New feature inserts its template as Ace's snippet: "My Feature" selected, it, `myFeature` and both
/// comments boxed; typed text stays in its box, Tab takes the next, the caret leaving them all ends the boxes.
#[gpui::test]
fn new_feature_boxes_its_tab_stops(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let dir = std::env::temp_dir().join(format!("noshape-ui-fs-new-stops-{}", std::process::id()));
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
    let (window, view) = cx.update(|cx| {
        let doc = cx.new(|_| DocModel::new(session));
        let bounds = Bounds { origin: Point::default(), size: size(px(1600.), px(900.)) };
        let mut view = None;
        let (w, _) = gpui_kit::open_window(WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() }, cx, |window, cx| {
            let v = cx.new(|cx| FeatureStudioView::new(doc, tab, window, cx));
            view = Some(v.clone());
            cx.new(|_| Workspace(v))
        })
        .unwrap();
        (w, view.unwrap())
    });
    draw(cx, window);
    // The workspace stars the tab on Edited ("*Feature Studio 1" from the first key, live workflow 26).
    let edited = std::rc::Rc::new(std::cell::Cell::new(0));
    let count = edited.clone();
    cx.update(|cx| {
        cx.subscribe(&view, move |_, e: &crate::feature_studio::FeatureStudioEvent, _| {
            if matches!(e, crate::feature_studio::FeatureStudioEvent::Edited) {
                count.set(count.get() + 1);
            }
        })
        .detach()
    });
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_new(w, cx))).unwrap();
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.insert_new_feature(w, cx))).unwrap();
    draw(cx, window);
    assert!(edited.get() > 0 && cx.update(|cx| view.read(cx).uncommitted(cx)), "uncommitted before any save: the tab starred");
    let boxed = |cx: &mut TestAppContext| {
        cx.update(|cx| {
            let v = view.read(cx);
            let text = v.editor.read(cx).value().to_string();
            v.snippet_boxes.as_ref().map(|(_, r)| r.iter().map(|r| text[r.clone()].to_string()).collect::<Vec<_>>()).unwrap_or_default()
        })
    };
    let selected = |cx: &mut TestAppContext| {
        cx.update(|cx| {
            let e = view.read(cx).editor.read(cx);
            e.value()[e.selected_range()].to_string()
        })
    };
    let comments = ["// Define the parameters of the feature type", "// Define the function's action"];
    assert_eq!(selected(cx), "My Feature");
    assert_eq!(boxed(cx), ["My Feature", "myFeature", comments[0], comments[1]]);
    for k in ["B", "o", "x"] {
        type_key(cx, window, k);
    }
    assert_eq!(boxed(cx), ["Box", "myFeature", comments[0], comments[1]], "typed into the first");
    type_key(cx, window, "tab");
    assert_eq!(selected(cx), "myFeature", "Tab takes the next");
    cx.update_window(window, |_, _, cx| view.update(cx, |v, cx| v.editor.update(cx, |e, cx| e.set_selected_range(0..0, cx)))).unwrap();
    draw(cx, window);
    assert!(boxed(cx).is_empty(), "the caret left them: no boxes");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Live fs-editor-feel-1122: hovering the underlined span shows its error at once under the line (22 px high: 14 px
/// text, padding 3); one character past the span, nothing.
#[gpui::test]
fn an_errors_tooltip_shows_under_its_line_only_over_the_underline(cx: &mut TestAppContext) {
    use gpui_kit::{Modifiers, VisualTestContext};
    cx.update(gpui_kit::init);
    let dir = std::env::temp_dir().join(format!("noshape-ui-fs-squiggle-tip-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("ui.noshape");
    let _ = std::fs::remove_file(&path);
    let source = "FeatureScript 2581;\nimport(path : \"onshape/std/geometry.fs\", version : \"2581.0\");\n\nexport const ee = sqrt(;\n";
    {
        use serde_json::json;
        let mut ctx = noshape_ops::Ctx::new();
        let call = |ctx: &mut noshape_ops::Ctx, tool: &str, args: serde_json::Value| noshape_ops::registry::run(ctx, noshape_ops::registry::by_tool_name(tool).unwrap(), &args).unwrap_or_else(|e| panic!("{tool}: {e:?}")).value;
        call(&mut ctx, "document_create", json!({ "path": path.to_str().unwrap() }));
        call(&mut ctx, "tab_create", json!({ "kind": "featurestudio" }));
        call(&mut ctx, "featurestudio_edit", json!({ "tab": "Feature Studio 1", "source": source }));
    }
    let session = crate::session::Session::open(&path).unwrap();
    let tab = session.doc.tabs.iter().find(|t| matches!(t.content, noshape_model::TabContent::FeatureStudio(_))).unwrap().id;
    let (window, view) = cx.update(|cx| {
        let doc = cx.new(|_| DocModel::new(session));
        let bounds = Bounds { origin: Point::default(), size: size(px(1600.), px(900.)) };
        let mut view = None;
        let (w, _) = gpui_kit::open_window(WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() }, cx, |window, cx| {
            let v = cx.new(|cx| FeatureStudioView::new(doc, tab, window, cx));
            view = Some(v.clone());
            cx.new(|_| Workspace(v))
        })
        .unwrap();
        (w, view.unwrap())
    });
    draw(cx, window);
    pause(cx, window, 2500);
    let range = cx.update(|cx| view.read(cx).editor.read(cx).diagnostics().and_then(|d| d.iter().next().map(|e| e.range.clone()))).expect("the error's underline");
    let span = cx.update(|cx| view.read(cx).editor.read(cx).range_to_bounds(&range)).expect("laid out");
    let mut v = VisualTestContext::from_window(window, cx);
    v.simulate_mouse_move(span.center(), None, Modifiers::default());
    draw(cx, window);
    let mut v = VisualTestContext::from_window(window, cx);
    let tip = v.debug_bounds("fs-squiggle-tip").expect("the tooltip at once");
    assert!((tip.top() - span.bottom()).abs() <= px(2.), "under the line: {tip:?} vs {span:?}");
    assert_eq!(tip.size.height, px(22.), "14 px text, padding 3");
    let past = cx.update(|cx| view.read(cx).editor.read(cx).range_to_bounds(&(range.end..range.end + 1))).expect("the next character");
    let mut v = VisualTestContext::from_window(window, cx);
    v.simulate_mouse_move(past.center(), None, Modifiers::default());
    draw(cx, window);
    let mut v = VisualTestContext::from_window(window, cx);
    assert!(v.debug_bounds("fs-squiggle-tip").is_none(), "nothing one character past the underline");
    // Live drw-matrix-1121 fsm-1: the tooltip shows over the underline while a text selection exists too.
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.editor.update(cx, |e, cx| {
        e.focus(w, cx);
        e.set_selected_range(0..12, cx);
    }))).unwrap();
    draw(cx, window);
    let mut v = VisualTestContext::from_window(window, cx);
    v.simulate_mouse_move(span.center(), None, Modifiers::default());
    draw(cx, window);
    let mut v = VisualTestContext::from_window(window, cx);
    assert!(v.debug_bounds("fs-squiggle-tip").is_some(), "with a selection: the tooltip still");
    assert_eq!(cx.update(|cx| view.read(cx).editor.read(cx).selected_range()), 0..12, "the hover keeps the selection");
    // A right-click on a word inside it: the selection stays; Copy / Paste / Cut at the pointer (fsm-1).
    let word = cx.update(|cx| view.read(cx).editor.read(cx).range_to_bounds(&(2..3))).expect("laid out").center();
    let mut v = VisualTestContext::from_window(window, cx);
    v.simulate_event(gpui_kit::MouseDownEvent { button: gpui_kit::MouseButton::Right, position: word, modifiers: Modifiers::default(), click_count: 1, first_mouse: false });
    v.simulate_event(gpui_kit::MouseUpEvent { button: gpui_kit::MouseButton::Right, position: word, modifiers: Modifiers::default(), click_count: 1 });
    draw(cx, window);
    assert_eq!(cx.update(|cx| view.read(cx).editor.read(cx).selected_range()), 0..12, "the right-click keeps the selection");
    let rows = cx.update(|cx| crate::os_menu::ui_state(cx)).map(|s| s["rows"].as_array().cloned().unwrap_or_default().iter().take(3).filter_map(|r| r.as_str().map(str::to_string)).collect::<Vec<_>>());
    assert_eq!(rows, Some(vec!["Copy".to_string(), "Paste".into(), "Cut".into()]), "Copy / Paste / Cut first");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Live workflows-1123 13 s3–s6: the Commit split button's ▾ is 18 px, against the main face with no divider.
#[gpui::test]
fn the_commit_caret_is_18_px_and_the_right_group_ends_at_the_edge(cx: &mut TestAppContext) {
    use gpui_kit::VisualTestContext;
    cx.update(gpui_kit::init);
    let dir = std::env::temp_dir().join(format!("noshape-ui-fs-commit-caret-{}", std::process::id()));
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
    let window = cx.update(|cx| {
        let doc = cx.new(|_| DocModel::new(session));
        let bounds = Bounds { origin: Point::default(), size: size(px(1600.), px(900.)) };
        let (w, _) = gpui_kit::open_window(WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() }, cx, |window, cx| {
            let v = cx.new(|cx| FeatureStudioView::new(doc, tab, window, cx));
            cx.new(|_| Workspace(v))
        })
        .unwrap();
        w
    });
    draw(cx, window);
    let mut vcx = VisualTestContext::from_window(window, cx);
    let main = vcx.debug_bounds("fs-commit").expect("the Commit button");
    let caret = vcx.debug_bounds("fs-commit-menu").expect("its ▾");
    assert_eq!((caret.size.width, caret.left()), (px(18.), main.right()), "18 px, no divider");
    // Live workflows-1123 12 s9: Monitor, Module outline, back / forward and Line, Column end at the window's right edge.
    let monitor = vcx.debug_bounds("fs-monitor").expect("Monitor");
    let line_col = vcx.debug_bounds("fs-line-column").expect("Line, Column");
    assert!(line_col.right() > px(1580.), "the right group ends at the edge: {line_col:?}");
    assert!(monitor.left() > caret.right() + px(300.), "a gap before Monitor: {monitor:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Live's Feature Studio editor (Ace) draws its caret as a 2 px bar.
#[gpui::test]
fn the_editors_caret_is_2_px_wide(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let dir = std::env::temp_dir().join(format!("noshape-ui-fs-caret-{}", std::process::id()));
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
    let (window, view) = cx.update(|cx| {
        let doc = cx.new(|_| DocModel::new(session));
        let bounds = Bounds { origin: Point::default(), size: size(px(1600.), px(900.)) };
        let mut view = None;
        let (w, _) = gpui_kit::open_window(WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() }, cx, |window, cx| {
            let v = cx.new(|cx| FeatureStudioView::new(doc, tab, window, cx));
            view = Some(v.clone());
            cx.new(|_| Workspace(v))
        })
        .unwrap();
        (w, view.unwrap())
    });
    draw(cx, window);
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.editor.update(cx, |e, cx| {
        e.focus(w, cx);
        e.set_selected_range(3..3, cx);
    }))).unwrap();
    draw(cx, window);
    let caret = cx.update(|cx| view.read(cx).editor.read(cx).prepaint_caret_geometry()).expect("a caret");
    assert_eq!(caret.cursor_bounds().size.width, px(2.));
    let _ = std::fs::remove_dir_all(&dir);
}
