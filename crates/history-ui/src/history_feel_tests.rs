//! Feel scenarios of the Versions and history panel (docs/parity/interaction-feel.md, live2 feel/studios.json
//! `versionsPanel`).

use super::HistoryPanel;
use crate::doc_model::DocModel;
use gpui_kit::{AnyWindowHandle, AppContext as _, Bounds, Entity, IntoElement, Modifiers, MouseButton, MouseDownEvent, MouseUpEvent, ParentElement as _, PlatformInput, Point, Render, Styled as _, TestAppContext, VisualTestContext, Window, WindowBounds, WindowOptions, div, point, px, size};
use serde_json::json;

/// The panel 420 px wide at the window's left, or (`.1`) the page over the whole window.
struct Host(Entity<HistoryPanel>, bool);

impl Render for Host {
    fn render(&mut self, _: &mut Window, _: &mut gpui_kit::Context<Self>) -> impl IntoElement {
        let w = if self.1 { px(1200.) } else { px(420.) };
        div().size_full().flex().child(div().w(w).h_full().flex().flex_col().child(self.0.clone())).child(div().flex_1())
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
    panel_versions(cx, name, edits, ["V1", "V2"])
}

/// Main with two versions named `versions`, `edits` changes before each.
fn panel_versions(cx: &mut TestAppContext, name: &str, edits: usize, versions: [&str; 2]) -> (AnyWindowHandle, Entity<HistoryPanel>, Entity<DocModel>) {
    history_view(cx, name, edits, versions, false)
}

/// [`panel_versions`] as the panel, or as the documents page's Versions and history page (`page`).
fn history_view(cx: &mut TestAppContext, name: &str, edits: usize, versions: [&str; 2], page: bool) -> (AnyWindowHandle, Entity<HistoryPanel>, Entity<DocModel>) {
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
        let description = if k == 1 { "block and routing curve" } else { "" };
        call("history_create_version", json!({ "name": versions[k - 1], "description": description }));
    }
    let session = crate::session::Session::open(&path).unwrap();
    let (window, view, doc) = cx.update(|cx| {
        let doc = cx.new(|_| DocModel::new(session));
        let bounds = Bounds { origin: Point::default(), size: size(px(1200.), px(800.)) };
        let mut view = None;
        let d = doc.clone();
        let (w, _) = crate::os_menu::open_window(WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() }, cx, |window, cx| {
            let v = cx.new(|cx| if page { HistoryPanel::page(d, "versions".into(), std::rc::Rc::new(|_, _, _| {}), window, cx) } else { HistoryPanel::new(d, window, cx) });
            view = Some(v.clone());
            cx.new(|_| Host(v, page))
        })
        .unwrap();
        (w, view.unwrap(), doc)
    });
    draw(cx, window);
    cx.run_until_parked();
    draw(cx, window);
    (window, view, doc)
}

/// A page row's ••• (drawn only on the row under the pointer): the pointer over the row, then the dots' bounds.
fn hovered_dots(cx: &mut TestAppContext, window: AnyWindowHandle, row: &'static str, dots: &'static str) -> Bounds<gpui_kit::Pixels> {
    let r = bounds(cx, window, row);
    VisualTestContext::from_window(window, cx).simulate_mouse_move(point(r.origin.x + px(40.), r.center().y), None, Modifiers::default());
    draw(cx, window);
    bounds(cx, window, dots)
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

/// Live2 feel/studios.json versionsPanel: a version row tooltips "Open version V1" on one line at 130 ms, grown into its
/// description at ~2.3 s and still centred on the row (live2 versions-1119); a single click on its name opens it
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
    cx.executor().advance_clock(std::time::Duration::from_millis(100));
    draw(cx, window);
    assert_eq!(crate::ui_script::shown_tooltip(), None, "not yet at 100 ms (live2 versions-1119: 130 ms)");
    cx.executor().advance_clock(std::time::Duration::from_millis(60));
    draw(cx, window);
    assert_eq!(crate::ui_script::shown_tooltip().as_deref(), Some("Open version V1"), "the one-line tip first");
    // Beside the panel's right edge (the row's), not under the pointer (qa HISTFEEL-CHANGE-TOOLTIP).
    let tip = VisualTestContext::from_window(window, cx).debug_bounds("placed-tooltip").expect("the tooltip");
    assert!(tip.left() >= v1.right() && tip.left() - v1.right() < px(6.), "right of the row: {tip:?} by {v1:?}");
    assert!(tip.size.height < px(30.), "one line: {tip:?}");
    cx.executor().advance_clock(std::time::Duration::from_millis(2200));
    draw(cx, window);
    draw(cx, window);
    assert_eq!(crate::ui_script::shown_tooltip().as_deref(), Some("Open version V1 block and routing curve"), "grown at ~2.3 s");
    let grown = VisualTestContext::from_window(window, cx).debug_bounds("placed-tooltip").expect("the tooltip");
    assert!((f32::from(grown.size.width) - 300.).abs() < 0.5, "300 px wide: {grown:?}");
    assert!((f32::from(grown.top()) - (f32::from(tip.top()) - 13.)).abs() < 0.5, "its top 13 px up: {tip:?} -> {grown:?}");
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

/// Live history-feel-1117 (qa HISTFEEL-SCROLL, -EXPANDED): the search box and rows scroll as one under the pinned
/// Name | Modified header, and change rows are 30 px.
#[gpui::test]
fn the_history_scrolls_as_one_under_a_pinned_header(cx: &mut TestAppContext) {
    let (window, view, _doc) = panel_with(cx, "scroll", 12);
    for v in ["hist-changes V2", "hist-changes V1"] {
        let at = bounds(cx, window, v).center();
        click(cx, window, at, 1);
    }
    assert_eq!(cx.update(|cx| view.read(cx).expanded.len()), 2, "both lists open");
    let (search, v1) = (bounds(cx, window, "hist-search"), bounds(cx, window, "hist-row V1"));
    let wheel = gpui_kit::ScrollWheelEvent { position: point(px(200.), px(400.)), delta: gpui_kit::ScrollDelta::Pixels(point(px(0.), px(-200.))), ..Default::default() };
    VisualTestContext::from_window(window, cx).simulate_event(wheel);
    draw(cx, window);
    draw(cx, window);
    let (search2, v12) = (bounds(cx, window, "hist-search"), bounds(cx, window, "hist-row V1"));
    let moved = f32::from(search.origin.y - search2.origin.y);
    assert!(moved > 100., "the search box scrolls away: {moved}");
    assert_eq!(f32::from(v1.origin.y - v12.origin.y), moved, "with the rows");
    assert!(cx.update(|cx| view.read(cx).header_out.get()), "the header is pinned");
    let change = cx.update(|cx| view.read(cx).rows.as_ref().and_then(|r| r.iter().find(|r| r.kind == super::RowKind::Version).and_then(|r| r.changes.first().copied())));
    if let Some(mv) = change {
        let row = VisualTestContext::from_window(window, cx).debug_bounds(Box::leak(format!("hist-change c{mv}").into_boxed_str()));
        if let Some(row) = row {
            assert_eq!(f32::from(row.size.height), 30., "a change row is 30 px");
        }
    }
}

/// qa HISTORY-ROW-NAME-OVERLAP: a name wider than the Name column ends at it (cut with an ellipsis), never over the
/// Modified column.
#[gpui::test]
fn a_long_row_name_ends_at_the_name_column(cx: &mut TestAppContext) {
    let long = "Part Studio 2 :: Edit : QA Box 1";
    let (window, _view, _doc) = panel_versions(cx, "long-name", 1, ["V1", long]);
    let (row, name) = (bounds(cx, window, Box::leak(format!("hist-row {long}").into_boxed_str())), bounds(cx, window, Box::leak(format!("hist-name {long}").into_boxed_str())));
    let short = bounds(cx, window, "hist-name V1");
    assert_eq!(name.origin.x, short.origin.x, "the name starts where the others do");
    let end = f32::from(name.origin.x + name.size.width - row.origin.x);
    let column = f32::from(short.origin.x - row.origin.x) + 115.;
    assert!(end <= column + 0.5, "the name ends at the Name column ({end} > {column})");
}

/// Live ws-1118: the default workspace's row has no Delete; the current non-default one's has, and deleting it lands
/// viewing the version it was branched from, with "Deleted workspace B1." and its Restore link for ~30 s.
#[gpui::test]
fn deleting_the_workspace_you_are_in_lands_on_its_version(cx: &mut TestAppContext) {
    let (window, view, doc) = panel(cx, "delete-current");
    let v1 = cx.update(|cx| doc.read(cx).session.store.list_versions().expect("versions").into_iter().find(|v| v.name == "V1").expect("V1"));
    let b1 = cx.update(|cx| doc.update(cx, |d, cx| {
        let b = d.session.branch_from(v1.microversion, "B1").expect("B1");
        d.reloaded(cx);
        b
    }));
    cx.run_until_parked();
    draw(cx, window);
    draw(cx, window);
    let menu = |cx: &mut TestAppContext, row: &'static str| {
        let b = bounds(cx, window, row);
        let at = point(b.origin.x + px(160.), b.origin.y + px(15.));
        let mut vcx = VisualTestContext::from_window(window, cx);
        vcx.simulate_mouse_down(at, MouseButton::Right, Modifiers::default());
        vcx.simulate_mouse_up(at, MouseButton::Right, Modifiers::default());
        draw(cx, window);
        let labels = cx.update(|cx| crate::os_menu::open_labels(cx));
        cx.update_window(window, |_, w, cx| w.dispatch_keystroke(gpui_kit::Keystroke::parse("escape").unwrap(), cx)).unwrap();
        draw(cx, window);
        labels
    };
    assert!(menu(cx, "hist-row B1").iter().any(|l| l == "Delete"), "the current workspace's row has Delete");
    assert!(!menu(cx, "hist-row Main").iter().any(|l| l == "Delete"), "the default one's has none");
    view.update(cx, |v, cx| v.delete_workspace(b1, "B1".into(), cx));
    cx.run_until_parked();
    let (viewing, link) = cx.update(|cx| {
        let d = doc.read(cx);
        (d.session.viewing_version(), d.notice_link("Deleted workspace B1.").map(|l| (l.label.to_string(), l.stays.as_secs())))
    });
    assert_eq!(viewing.as_deref(), Some("V1"), "viewing V1");
    assert_eq!(link, Some(("Restore".into(), 30)), "its Restore link, the bubble gone after 30 s (live: 30.8 s)");
}

/// Live ws-1118 b2: the Branch dialog refuses an empty name or one over 256 characters (its red line, Create off).
#[test]
fn a_workspace_name_is_required_and_at_most_256_characters() {
    assert!(super::workspace_name_bad("") && super::workspace_name_bad("   "));
    assert!(!super::workspace_name_bad("B2") && !super::workspace_name_bad(&"x".repeat(256)));
    assert!(super::workspace_name_bad(&"x".repeat(257)));
}

/// Live docver-1121: Create version refuses an empty name, spaces, and 256 or 257 characters; 255 are taken.
#[test]
fn a_version_name_is_required_and_at_most_255_characters() {
    assert!(super::version_name_bad("") && super::version_name_bad("   "));
    assert!(!super::version_name_bad("V2") && !super::version_name_bad(&"x".repeat(255)));
    assert!(super::version_name_bad(&"x".repeat(256)) && super::version_name_bad(&"x".repeat(257)));
}


struct ComparePanel(Entity<DocModel>, noshape_model::document::TabId);

impl Render for ComparePanel {
    fn render(&mut self, _: &mut Window, cx: &mut gpui_kit::Context<Self>) -> impl IntoElement {
        div().size_full().flex().children(crate::compare_mode::render_panel(&self.0, self.1, cx))
    }
}

/// Live fc-1120 (10): a face of Main's Extrude 2 (not in V1) clicked in Compare selects its row, whose parameters show
/// beside "Feature does not exist in target."; a second click deselects it ("Select a feature to compare its
/// parameters." again).
#[gpui::test]
fn a_feature_only_in_the_base_reads_does_not_exist_in_target_beside_its_parameters(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    // (Feature icons only: nothing else above the panel exists here.)
    crate::history_hooks::register(crate::history_hooks::Hooks {
        feature_icon: |_| "".into(),
        workspace_protection: |_, _, _, _| {},
        open_new_window: |_, _, _, _| Ok(()),
        open_workspace_window: |_, _, _| Ok(()),
        open_compare_window: |_, _, _, _| Ok(()),
    });
    let dir = std::env::temp_dir().join(format!("noshape-history-feel-cmp-missing-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("versions.noshape");
    for ext in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{ext}", path.display()));
    }
    let mut ctx = noshape_ops::Ctx::new();
    let mut call = |tool: &str, args: serde_json::Value| noshape_ops::registry::run(&mut ctx, noshape_ops::registry::by_tool_name(tool).unwrap(), &args).unwrap_or_else(|e| panic!("{tool}: {e:?}")).value;
    call("document_create", json!({ "path": path.to_str().unwrap() }));
    for (k, points) in [(1, [[0, 0], [40, 30]]), (2, [[15, 10], [25, 20]])] {
        call("sketch_create", json!({ "plane": "plane:top" }));
        let sketch = format!("Sketch {k}");
        call("sketch_draw", json!({ "sketch": sketch, "tool": "corner_rectangle", "points": points }));
        let region = call("sketch_get", json!({ "sketch": sketch }))["regions"][0]["id"].as_str().unwrap().to_string();
        call("feature_add", json!({ "type": "extrude", "params": { "entities": [region], "depth": "10 mm", "operationType": "NEW" } }));
        if k == 1 {
            call("history_create_version", json!({ "name": "V1" }));
        }
    }
    let session = crate::session::Session::open(&path).unwrap();
    let (window, doc, tab, extrude2) = cx.update(|cx| {
        let doc = cx.new(|_| DocModel::new(session));
        let (tab, extrude2) = {
            let d = &doc.read(cx).session.doc;
            let t = d.tabs.iter().find(|t| t.name == "Part Studio 1").unwrap();
            let noshape_model::document::TabContent::PartStudio(ps) = &t.content else { panic!("a Part Studio") };
            (t.id, ps.features.iter().find(|f| f.name == "Extrude 2").unwrap().id)
        };
        let (branch, v1) = {
            let s = &doc.read(cx).session;
            (s.branch, s.store.list_versions().unwrap().into_iter().find(|v| v.name == "V1").unwrap().microversion)
        };
        crate::compare_mode::compare(&doc, (crate::compare_mode::Side::Workspace(branch), "Main".into()), (crate::compare_mode::Side::Entry(v1), "V1".into()), cx).unwrap();
        let bounds = Bounds { origin: Point::default(), size: size(px(1200.), px(800.)) };
        let d = doc.clone();
        let (w, _) = crate::os_menu::open_window(WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() }, cx, |_, cx| cx.new(|_| ComparePanel(d, tab))).unwrap();
        (w, doc, tab, extrude2)
    });
    draw(cx, window);
    let missing = |cx: &mut TestAppContext| VisualTestContext::from_window(window, cx).debug_bounds("compare-missing-target");
    assert!(missing(cx).is_none(), "nothing selected yet");
    cx.update(|cx| crate::compare_mode::select_feature(&doc, Some(extrude2), cx));
    draw(cx, window);
    let selected = cx.update(|cx| crate::compare_mode::ui_state(&doc, Some(tab), cx).unwrap()["selected"].clone());
    assert_eq!(selected, json!("Extrude 2"), "Extrude 2's row selected");
    let text = missing(cx).expect("the target's column says the feature is missing");
    assert!(text.origin.x >= px(198.) && text.size.width > px(150.), "in the right-hand column (after the base's 197 px and the 1 px rule): {text:?}");
    cx.update(|cx| crate::compare_mode::select_feature(&doc, None, cx));
    draw(cx, window);
    assert!(missing(cx).is_none(), "deselected");
    cx.update(|cx| crate::compare_mode::stop(&doc, cx));
}

/// Live fv-1121 (4): hovering a version graph node tooltips its branch ("Main"), not the row's "Open version …", and
/// doesn't tint the row; a click on it opens nothing.
#[gpui::test]
fn a_graph_nodes_tooltip_is_its_branch(cx: &mut TestAppContext) {
    let (window, view, doc) = panel(cx, "node-tip");
    let node = bounds(cx, window, "hist-node-start");
    crate::ui_script::clear_tooltip();
    VisualTestContext::from_window(window, cx).simulate_mouse_move(node.center(), None, Modifiers::default());
    cx.executor().advance_clock(std::time::Duration::from_millis(800));
    draw(cx, window);
    assert_eq!(crate::ui_script::shown_tooltip().as_deref(), Some("Main"));
    assert_eq!(cx.update(|cx| view.read(cx).node_hover.clone()).as_deref(), Some("start"), "its row not tinted (only the tooltip)");
    let name = bounds(cx, window, "hist-row Start");
    VisualTestContext::from_window(window, cx).simulate_mouse_move(point(name.origin.x + px(160.), name.center().y), None, Modifiers::default());
    draw(cx, window);
    assert_eq!(cx.update(|cx| view.read(cx).node_hover.clone()), None, "off the node: the row tints again");
    VisualTestContext::from_window(window, cx).simulate_mouse_move(node.center(), None, Modifiers::default());
    draw(cx, window);
    click(cx, window, node.center(), 1);
    assert_eq!(cx.update(|cx| doc.read(cx).session.viewing_version()), None, "the node opens nothing");
}

/// Live 2435 (q2435-vh-30s, -click-start, -dblclick-start, -main-rclick) and docver-1121: the documents page's Versions
/// and history page arrives with Main selected; a click (or a double-click) on another row only selects it, opening
/// nothing; a right-click shows no menu; the row under the pointer shows its ••• (none at rest): Main's lists Open ·
/// Open in new browser tab · Workspace protections… · Properties…, a version's Open · Open in new browser tab · Branch
/// to create workspace… · Restore to Main · Properties…; the legend is one row right under the table.
#[gpui::test]
fn the_versions_page_selects_on_click_and_acts_from_its_dots(cx: &mut TestAppContext) {
    let (w, view, doc) = history_view(cx, "page", 1, ["V1", "V2"], true);
    assert!(view.read_with(cx, |v, _| v.page_selected().is_some_and(|k| k.starts_with('w'))), "Main selected on arrival");
    assert!(VisualTestContext::from_window(w, cx).debug_bounds("hist-page-title").is_some());
    let (legend, last) = (bounds(cx, w, "hist-page-legend"), bounds(cx, w, "hist-row Start"));
    let gap = f32::from(legend.top() - last.bottom());
    assert!((0. ..60.).contains(&gap), "the legend right under the last row (live ~36 px), not at the page's foot: {gap}");
    assert!(VisualTestContext::from_window(w, cx).debug_bounds("hist-page-more Main").is_none(), "no ••• at rest");
    let main_dots = hovered_dots(cx, w, "hist-row Main", "hist-page-more Main");
    assert!(bounds(cx, w, "hist-row Main").contains(&main_dots.center()), "Main's ••• at its row's end");
    let start = bounds(cx, w, "hist-row Start").center();
    click(cx, w, start, 1);
    draw(cx, w);
    assert_eq!(view.read_with(cx, |v, _| v.page_selected().map(str::to_string)), Some("start".into()));
    let v1 = bounds(cx, w, "hist-row V1").center();
    click(cx, w, v1, 2);
    draw(cx, w);
    assert!(view.read_with(cx, |v, _| v.page_selected().is_some_and(|k| k.starts_with('v'))), "V1 selected");
    assert!(doc.read_with(cx, |d, _| d.session.viewing.is_none()), "nothing opened");
    let v1 = bounds(cx, w, "hist-row V1");
    VisualTestContext::from_window(w, cx).simulate_mouse_down(v1.center(), MouseButton::Right, Modifiers::default());
    draw(cx, w);
    assert!(!cx.update(|cx| crate::os_menu::is_open(cx)), "no right-click menu");
    VisualTestContext::from_window(w, cx).simulate_mouse_up(v1.center(), MouseButton::Right, Modifiers::default());
    let dots = hovered_dots(cx, w, "hist-row V1", "hist-page-more V1").center();
    VisualTestContext::from_window(w, cx).simulate_mouse_move(dots, None, Modifiers::default());
    draw(cx, w);
    VisualTestContext::from_window(w, cx).simulate_click(dots, Modifiers::default());
    draw(cx, w);
    assert_eq!(cx.update(|cx| crate::os_menu::open_labels(cx)), ["Open", "Open in new browser tab", "Branch to create workspace…", "Restore to Main", "Properties…"]);
    VisualTestContext::from_window(w, cx).simulate_keystrokes("escape");
    draw(cx, w);
    let main_dots = hovered_dots(cx, w, "hist-row Main", "hist-page-more Main").center();
    VisualTestContext::from_window(w, cx).simulate_mouse_move(main_dots, None, Modifiers::default());
    draw(cx, w);
    VisualTestContext::from_window(w, cx).simulate_click(main_dots, Modifiers::default());
    draw(cx, w);
    assert_eq!(cx.update(|cx| crate::os_menu::open_labels(cx)), ["Open", "Open in new browser tab", "Workspace protections…", "Properties…"]);
    // Start is a version too (live docver-1121 vhpage-dots-Start): Properties… as V1's.
    VisualTestContext::from_window(w, cx).simulate_keystrokes("escape");
    draw(cx, w);
    let start_dots = hovered_dots(cx, w, "hist-row Start", "hist-page-more Start").center();
    VisualTestContext::from_window(w, cx).simulate_mouse_move(start_dots, None, Modifiers::default());
    draw(cx, w);
    VisualTestContext::from_window(w, cx).simulate_click(start_dots, Modifiers::default());
    draw(cx, w);
    assert_eq!(cx.update(|cx| crate::os_menu::open_labels(cx)), ["Open", "Open in new browser tab", "Branch to create workspace…", "Restore to Main", "Properties…"]);
}

/// Live docver-1121 (studios 11-21 13:11): the Start row is a real version: its menu ends with Properties… and Publish
/// FeatureScript…, and once renamed the row, the view-only bar and other documents' version lists read the new name.
#[gpui::test]
fn the_start_row_is_a_version_with_properties_and_publish(cx: &mut TestAppContext) {
    let (window, _view, doc) = panel(cx, "start-version");
    let start = bounds(cx, window, "hist-row Start");
    let at = point(start.origin.x + px(160.), start.origin.y + px(10.));
    let mut vcx = VisualTestContext::from_window(window, cx);
    vcx.simulate_mouse_down(at, MouseButton::Right, Modifiers::default());
    vcx.simulate_mouse_up(at, MouseButton::Right, Modifiers::default());
    draw(cx, window);
    let labels = cx.update(|cx| crate::os_menu::open_labels(cx));
    assert!(labels.ends_with(&["Properties…".to_string(), "Publish FeatureScript…".to_string()]), "{labels:?}");
    assert!(labels.iter().any(|l| l == "Restore to Main…"), "{labels:?}");
    VisualTestContext::from_window(window, cx).simulate_keystrokes("escape");
    // Properties… › Apply, as the dialog saves it.
    cx.update(|cx| doc.update(cx, |d, cx| {
        d.session.store.update_start_version("Start x", "the first state").unwrap();
        d.reloaded(cx);
    }));
    cx.run_until_parked();
    draw(cx, window);
    bounds(cx, window, "hist-row Start x");
    let path = cx.update(|cx| {
        let d = doc.read(cx);
        assert_eq!(d.session.store.start_version().unwrap(), ("Start x".to_string(), "the first state".to_string()));
        d.session.path.clone()
    });
    let (_, versions) = crate::derived::document_versions(&path).unwrap();
    assert_eq!(versions.last().map(|v| v.label.as_str()), Some("Start x"), "the Insert dialog's version list");
    let root = cx.update(|cx| doc.read(cx).session.store.graph().unwrap().microversions.iter().find(|m| m.parents.is_empty()).unwrap().id);
    cx.update(|cx| doc.update(cx, |d, cx| {
        d.session.view_microversion(Some(root)).unwrap();
        d.reloaded(cx);
    }));
    assert_eq!(cx.update(|cx| doc.read(cx).session.viewing_version()), Some("Start x".into()), "Versions are view only. Viewing Start x.");
}

/// Live hist-filter-1122: Filter branches shows only while the document has a second workspace (43 x 28, right of the
/// magnifier); Show current branch lists the current branch's rows alone and "Branched from V1 on Main", whose link
/// goes back to every branch; with parent branches Main's rows come back; deleting B1 takes the button away.
#[gpui::test]
fn the_branch_filter_shows_with_a_second_workspace_and_narrows_the_rows(cx: &mut TestAppContext) {
    let (window, view, doc) = panel(cx, "branch-filter");
    let drawn = |cx: &mut TestAppContext, selector: &'static str| VisualTestContext::from_window(window, cx).debug_bounds(selector).is_some();
    assert!(!drawn(cx, "hist-branch-filter"), "no button with Main alone");
    let v1 = cx.update(|cx| doc.read(cx).session.store.list_versions().expect("versions").into_iter().find(|v| v.name == "V1").expect("V1"));
    let b1 = cx.update(|cx| doc.update(cx, |d, cx| {
        let b = d.session.branch_from(v1.microversion, "B1").expect("B1");
        d.edit_document("Rename tab", cx, |doc| doc.tabs[0].name = "Studio B1".into());
        d.session.create_version("V3", "").expect("V3");
        d.reloaded(cx);
        b
    }));
    cx.run_until_parked();
    draw(cx, window);
    draw(cx, window);
    let button = bounds(cx, window, "hist-branch-filter");
    assert_eq!((f32::from(button.size.width), f32::from(button.size.height)), (43., 28.));
    assert_eq!(view.read_with(cx, |v, _| v.ui_branch_filter()), "Show all branches");
    let pick = |cx: &mut TestAppContext, item: &'static str| {
        let at = bounds(cx, window, "hist-branch-filter").center();
        click(cx, window, at, 1);
        assert!(view.read_with(cx, |v, _| v.branch_button_look().0), "its menu open: the blue outline");
        let row = bounds(cx, window, item);
        assert!(row.top() > button.bottom() && (f32::from(row.right()) - f32::from(button.right())).abs() <= 2., "the menu hangs right-aligned under the button (rows inside its border): {row:?} {button:?}");
        click(cx, window, row.center(), 1);
        draw(cx, window);
        assert!(!drawn(cx, item), "one click picks and closes");
    };
    pick(cx, "hist-branch-item Show current branch");
    assert_eq!(view.read_with(cx, |v, _| v.ui_branch_filter()), "Show current branch");
    assert_eq!(view.read_with(cx, |v, _| v.branch_button_look()), (false, true), "a filter on, the menu closed: filled blue, no outline");
    for row in ["hist-row B1", "hist-row V3", "hist-branched-from Branched from V1 on Main"] {
        assert!(drawn(cx, row), "{row} shown");
    }
    for row in ["hist-row Main", "hist-row V2", "hist-row V1", "hist-row Start"] {
        assert!(!drawn(cx, row), "{row} hidden");
    }
    let link = bounds(cx, window, "hist-branch-history").center();
    click(cx, window, link, 1);
    assert_eq!(view.read_with(cx, |v, _| v.ui_branch_filter()), "Show all branches");
    assert_eq!(view.read_with(cx, |v, _| v.branch_button_look()), (false, false), "back on all branches: grey");
    assert!(drawn(cx, "hist-row Main") && drawn(cx, "hist-row V1") && !drawn(cx, "hist-branched-from Branched from V1 on Main"));
    pick(cx, "hist-branch-item Show current branch with parent branches");
    for row in ["hist-row B1", "hist-row V3", "hist-row Main", "hist-row V2", "hist-row V1", "hist-row Start"] {
        assert!(drawn(cx, row), "{row} shown");
    }
    view.update(cx, |v, cx| v.delete_workspace(b1, "B1".into(), cx));
    cx.run_until_parked();
    draw(cx, window);
    draw(cx, window);
    assert!(!drawn(cx, "hist-branch-filter"), "deleting B1 takes the button away");
    assert_eq!(view.read_with(cx, |v, _| v.ui_branch_filter()), "Show all branches");
}

/// Live history-gaps-1122: in version › Restore to Main… Enter does nothing (the dialog stays, nothing restored); Esc
/// closes it.
#[gpui::test]
fn the_restore_dialog_ignores_enter_and_closes_on_esc(cx: &mut TestAppContext) {
    let (window, _view, doc) = panel(cx, "restore-keys");
    let v1 = cx.update(|cx| doc.read(cx).session.store.list_versions().expect("versions").into_iter().find(|v| v.name == "V1").expect("V1"));
    let head = |cx: &mut TestAppContext| cx.update(|cx| doc.read(cx).session.store.graph().unwrap().microversions.len());
    let before = head(cx);
    cx.update_window(window, |_, w, cx| crate::merge_dialog::open_restore(doc.clone(), v1.microversion, "V1".into(), w, cx)).unwrap();
    draw(cx, window);
    let shown = |cx: &mut TestAppContext| cx.update_window(window, |_, w, cx| gpui_kit::component::WindowExt::has_active_dialog(w, cx)).unwrap();
    assert!(shown(cx), "Restoring changes into Main");
    VisualTestContext::from_window(window, cx).simulate_keystrokes("enter");
    draw(cx, window);
    assert!(shown(cx), "Enter leaves it open");
    assert_eq!(head(cx), before, "nothing restored");
    VisualTestContext::from_window(window, cx).simulate_keystrokes("escape");
    draw(cx, window);
    assert!(!shown(cx), "Esc closes it");
    assert_eq!(head(cx), before, "still nothing restored");
}

/// Live workflows-1123 19 s21: B1 merged into Main, a version "Merged" made there, then B1 deleted: Main's history keeps
/// "Merge from B1" under "Merged" and B1's merged change on a lane of its own.
#[test]
fn a_deleted_merged_workspace_keeps_its_merge_rows() {
    let dir = std::env::temp_dir().join(format!("noshape-history-deleted-merge-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("merge.noshape");
    for ext in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{ext}", path.display()));
    }
    let mut ctx = noshape_ops::Ctx::new();
    let mut call = |tool: &str, args: serde_json::Value| noshape_ops::registry::run(&mut ctx, noshape_ops::registry::by_tool_name(tool).unwrap(), &args).unwrap_or_else(|e| panic!("{tool}: {e:?}")).value;
    call("document_create", json!({ "path": path.to_str().unwrap() }));
    call("variable_create", json!({ "name": "a", "value": "1 mm" }));
    call("history_create_version", json!({ "name": "V1" }));
    call("history_create_branch", json!({ "name": "B1", "version": "V1" }));
    call("history_switch_branch", json!({ "branch": "B1" }));
    call("variable_create", json!({ "name": "b", "value": "2 mm" }));
    call("history_switch_branch", json!({ "branch": "Main" }));
    call("history_merge", json!({ "branch": "B1" }));
    call("history_create_version", json!({ "name": "Merged" }));
    call("history_delete_branch", json!({ "branch": "B1" }));
    let session = crate::session::Session::open(&path).unwrap();
    let graph = session.store.graph().unwrap();
    let mut lanes: Vec<noshape_store::BranchId> = graph.branches.iter().filter(|b| !b.deleted).map(|b| b.id).collect();
    let live = lanes.len();
    lanes.extend(super::merged_deleted(&graph));
    assert_eq!(lanes.len(), live + 1, "B1 keeps a lane");
    let rows = super::build_rows(&graph, &lanes, ("Start".into(), String::new()));
    let kinds: Vec<(super::RowKind, String)> = rows.iter().map(|r| (r.kind, r.name.clone())).collect();
    let at = |k: super::RowKind| rows.iter().position(|r| r.kind == k);
    let (merged, merge, change) = (rows.iter().position(|r| r.kind == super::RowKind::Version && r.name == "Merged"), at(super::RowKind::Merge), at(super::RowKind::Merged));
    assert!(merged.is_some() && merge.is_some() && change.is_some(), "Merged, the merge row and B1's change: {:?}", kinds.iter().map(|k| &k.1).collect::<Vec<_>>());
    assert!(merged < merge && merge < change, "in that order");
    assert_eq!(rows[change.unwrap()].lane, live, "B1's change on its own lane");
    assert!(rows[merged.unwrap()].changes.is_empty(), "the version shows no changes of its own");
}
