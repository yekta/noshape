//! Feel scenarios of the Versions and history panel (docs/parity/interaction-feel.md, live2 feel/studios.json
//! `versionsPanel`).

use super::HistoryPanel;
use crate::doc_model::DocModel;
use gpui_kit::{AnyWindowHandle, AppContext as _, Bounds, Entity, IntoElement, Modifiers, MouseButton, MouseDownEvent, MouseUpEvent, ParentElement as _, PlatformInput, Point, Render, Styled as _, TestAppContext, VisualTestContext, Window, WindowBounds, WindowOptions, div, point, px, size};
use serde_json::json;

struct Host(Entity<HistoryPanel>);

impl Render for Host {
    fn render(&mut self, _: &mut Window, _: &mut gpui_kit::Context<Self>) -> impl IntoElement {
        div().size_full().flex().child(div().w(px(420.)).h_full().flex().flex_col().child(self.0.clone())).child(div().flex_1())
    }
}

fn draw(cx: &mut TestAppContext, window: AnyWindowHandle) {
    cx.update_window(window, |_, w, cx| w.draw(cx).clear(cx)).unwrap();
}

/// Main with V1 and V2, a change before each.
fn panel(cx: &mut TestAppContext, name: &str) -> (AnyWindowHandle, Entity<HistoryPanel>, Entity<DocModel>) {
    panel_with(cx, name, 1)
}

/// Main with V1 and V2, `edits` changes before each.
fn panel_with(cx: &mut TestAppContext, name: &str, edits: usize) -> (AnyWindowHandle, Entity<HistoryPanel>, Entity<DocModel>) {
    cx.update(gpui_kit::init);
    let dir = std::env::temp_dir().join(format!("noshape-history-feel-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("versions.noshape");
    for ext in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{ext}", path.display()));
    }
    let mut ctx = noshape_ops::Ctx::new();
    let mut call = |tool: &str, args: serde_json::Value| noshape_ops::registry::run(&mut ctx, noshape_ops::registry::by_tool_name(tool).unwrap(), &args).unwrap_or_else(|e| panic!("{tool}: {e:?}")).value;
    call("document_create", json!({ "path": path.to_str().unwrap() }));
    for k in 1..=2 {
        for e in 0..edits {
            call("variable_create", json!({ "name": format!("v{k}_{e}"), "value": format!("{k} mm") }));
        }
        call("history_create_version", json!({ "name": format!("V{k}") }));
    }
    let session = crate::session::Session::open(&path).unwrap();
    let (window, view, doc) = cx.update(|cx| {
        let doc = cx.new(|_| DocModel::new(session));
        let bounds = Bounds { origin: Point::default(), size: size(px(1200.), px(800.)) };
        let mut view = None;
        let d = doc.clone();
        let (w, _) = crate::os_menu::open_window(WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() }, cx, |window, cx| {
            let v = cx.new(|cx| HistoryPanel::new(d, window, cx));
            view = Some(v.clone());
            cx.new(|_| Host(v))
        })
        .unwrap();
        (w, view.unwrap(), doc)
    });
    draw(cx, window);
    cx.run_until_parked();
    draw(cx, window);
    (window, view, doc)
}

fn bounds(cx: &mut TestAppContext, window: AnyWindowHandle, selector: &'static str) -> Bounds<gpui_kit::Pixels> {
    VisualTestContext::from_window(window, cx).debug_bounds(selector).unwrap_or_else(|| panic!("{selector} drawn"))
}

/// A press and release at `at`, `clicks` times in a row (a double click: 2), then the version's read finished.
fn click(cx: &mut TestAppContext, window: AnyWindowHandle, at: Point<gpui_kit::Pixels>, clicks: usize) {
    for n in 1..=clicks {
        cx.update_window(window, |_, w, cx| {
            w.dispatch_event(PlatformInput::MouseDown(MouseDownEvent { button: MouseButton::Left, position: at, modifiers: Modifiers::default(), click_count: n, first_mouse: false }), cx);
            w.dispatch_event(PlatformInput::MouseUp(MouseUpEvent { button: MouseButton::Left, position: at, modifiers: Modifiers::default(), click_count: n }), cx);
        })
        .unwrap();
    }
    cx.run_until_parked();
    draw(cx, window);
}

/// Live2 feel/studios.json versionsPanel: a version row tooltips "Open version V1"; a single click on its name opens it
/// (a double click too); a click in the graph column opens nothing; "Show changes…" lists its changes without opening
/// the row; a click on Main goes back to the workspace.
#[gpui::test]
fn version_rows_open_on_a_click_of_their_name_and_the_graph_column_opens_nothing(cx: &mut TestAppContext) {
    let (window, view, doc) = panel(cx, "rows");
    let viewing = |cx: &mut TestAppContext| cx.update(|cx| doc.read(cx).session.viewing_version());
    let v1 = bounds(cx, window, "hist-row V1");
    let name = point(v1.origin.x + px(160.), v1.origin.y + px(15.));
    crate::ui_script::clear_tooltip();
    VisualTestContext::from_window(window, cx).simulate_mouse_move(name, None, Modifiers::default());
    cx.executor().advance_clock(std::time::Duration::from_millis(500));
    draw(cx, window);
    assert_eq!(crate::ui_script::shown_tooltip().as_deref(), Some("Open version V1"), "the row's tooltip");
    click(cx, window, point(v1.origin.x + px(40.), v1.origin.y + px(15.)), 1);
    assert_eq!(viewing(cx), None, "a click in the graph column opens nothing");
    click(cx, window, name, 1);
    assert_eq!(viewing(cx).as_deref(), Some("V1"), "a click on the name opens V1");
    let changes = bounds(cx, window, "hist-changes V2");
    click(cx, window, changes.center(), 1);
    assert_eq!(viewing(cx).as_deref(), Some("V1"), "Show changes… doesn't open V2");
    assert_eq!(cx.update(|cx| view.read(cx).expanded.len()), 1, "V2's changes listed");
    let v2 = bounds(cx, window, "hist-row V2");
    click(cx, window, point(v2.origin.x + px(160.), v2.origin.y + px(15.)), 2);
    assert_eq!(viewing(cx).as_deref(), Some("V2"), "a double click opens V2 as a click does");
    let main = bounds(cx, window, "hist-row Main");
    click(cx, window, point(main.origin.x + px(160.), main.origin.y + px(15.)), 1);
    assert_eq!(viewing(cx), None, "a click on Main goes back to the workspace");
}

/// Live2 feel/studios.json versionsPanel: Compare in a version row's menu enters compare mode at once (no dialog), the
/// workspace the base and the version the target.
#[gpui::test]
fn compare_from_a_version_rows_menu_starts_at_once_against_the_workspace(cx: &mut TestAppContext) {
    let (window, _view, doc) = panel(cx, "compare");
    let v1 = bounds(cx, window, "hist-row V1");
    let at = point(v1.origin.x + px(160.), v1.origin.y + px(15.));
    let mut vcx = VisualTestContext::from_window(window, cx);
    vcx.simulate_mouse_down(at, MouseButton::Right, Modifiers::default());
    vcx.simulate_mouse_up(at, MouseButton::Right, Modifiers::default());
    draw(cx, window);
    let item = bounds(cx, window, "menu:Compare");
    VisualTestContext::from_window(window, cx).simulate_click(item.center(), Modifiers::default());
    cx.run_until_parked();
    draw(cx, window);
    let (sides, base, v1_mv) = cx.update(|cx| {
        let session = &doc.read(cx).session;
        let v1_mv = session.store.list_versions().unwrap().into_iter().find(|v| v.name == "V1").map(|v| v.microversion);
        (crate::compare_mode::rows(&doc, cx), crate::compare_mode::base_label(&doc, cx), v1_mv)
    });
    let (base_side, target) = sides.expect("compare mode on, no dialog first");
    assert!(matches!(base_side, crate::compare_mode::Side::Workspace(_)), "the workspace is the base: {base_side:?} ({base:?})");
    assert_eq!(Some(target), v1_mv.map(crate::compare_mode::Side::Entry), "V1 is the target");
    assert_eq!(cx.update(|cx| doc.read(cx).session.viewing_version()), None, "the workspace stays open");
}

/// Live history-feel-1117 (qa HISTFEEL-CHANGE-CLICK): a click on a change row under an expanded version (not the
/// version's own point) opens that change.
#[gpui::test]
fn a_click_on_a_change_row_under_a_version_opens_that_change(cx: &mut TestAppContext) {
    let (window, view, doc) = panel_with(cx, "change-click", 3);
    let changes = bounds(cx, window, "hist-changes V2");
    click(cx, window, changes.center(), 1);
    assert_eq!(cx.update(|cx| view.read(cx).expanded.len()), 1, "V2's changes listed");
    // V2's second change: the one before its own point.
    let (mv, name) = cx.update(|cx| {
        let s = &doc.read(cx).session;
        let v2 = s.store.list_versions().expect("versions").into_iter().find(|v| v.name == "V2").expect("V2");
        let m = s.store.microversion(v2.microversion).expect("V2's point");
        let prev = s.store.microversion(m.parents[0]).expect("the change before");
        (prev.id, prev.description)
    });
    // (The three changes share a description: the row is found by its key.)
    let _ = name;
    let row = bounds(cx, window, Box::leak(format!("hist-change c{mv}").into_boxed_str()));
    VisualTestContext::from_window(window, cx).simulate_mouse_move(point(row.origin.x + px(240.), row.center().y), None, Modifiers::default());
    click(cx, window, point(row.origin.x + px(240.), row.center().y), 1);
    assert_eq!(cx.update(|cx| doc.read(cx).session.viewing), Some(mv), "that change opened");
}
