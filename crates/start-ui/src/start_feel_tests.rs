//! The documents page's interaction feel through the real pointer and keys (docs/parity/interaction-feel.md
//! "Documents page and tabs"; live2 feel/documents.json).

use super::{DocEntry, Source, StartView, ViewMode};
use gpui_kit::{AnyWindowHandle, AppContext as _, Bounds, Entity, Modifiers, MouseButton, Pixels, Point, TestAppContext, VisualTestContext, WindowBounds, WindowOptions, point, px, size};
use std::sync::atomic::{AtomicUsize, Ordering};

/// Documents opened through the app's hook.
static OPENED: AtomicUsize = AtomicUsize::new(0);

/// The app's side of the page for every test of the crate (the hooks are registered once a process).
pub(super) fn register_test_hooks() {
    crate::start_hooks::register(crate::start_hooks::Hooks {
        open_document_window: |_, _, _, _| {
            OPENED.fetch_add(1, Ordering::SeqCst);
            Ok(())
        },
        open_document_at: |_, _, _, _| Ok(()),
        versions_page: |_, name, _, cx| Ok(cx.new(|_| TestPage(name)).into()),
        build_label: String::new,
        import_dialog: |_, _, _, _| {},
        help_items: |m| m,
        rename_open_document: |path, name, _| {
            let mut store = noshape_store::Document::open(&path).expect("the test document");
            store.set_name(&name).expect("renamed");
        },
    });
}

/// The app's Versions and history page, as the tests stand it in: its document's name.
struct TestPage(String);

impl gpui_kit::Render for TestPage {
    fn render(&mut self, _: &mut gpui_kit::Window, _: &mut gpui_kit::Context<Self>) -> impl gpui_kit::IntoElement {
        use gpui_kit::{ParentElement as _, Styled as _};
        gpui_kit::div().size_full().child(self.0.clone())
    }
}

/// The page listing documents A to E (list view), drawn.
fn page(cx: &mut TestAppContext, name: &str) -> (AnyWindowHandle, Entity<StartView>) {
    cx.update(gpui_kit::init);
    cx.update(|cx| cx.bind_keys(super::key_bindings()));
    register_test_hooks();
    let gpu = crate::gpu::Gpu::headless().unwrap();
    cx.update(|cx| cx.set_global(crate::gpu::GpuGlobal(gpu)));
    let dir = std::env::temp_dir().join(format!("noshape-start-feel-{name}-{}", std::process::id()));
    let docs: Vec<DocEntry> = ["A", "B", "C", "D", "E"]
        .iter()
        .enumerate()
        .map(|(i, n)| DocEntry { path: dir.join(format!("{n}.noshape")), name: (*n).into(), modified: 1_760_000_000_000 - i as i64 * 60_000, created: 0, modified_by: "me".into(), owner: "me".into(), description: String::new(), workspace: "Main".into(), thumbnail: None, recent_rank: None, broken: false, trashed: false })
        .collect();
    let mut view = None;
    let (window, _) = cx
        .update(|cx| {
            crate::os_menu::open_window(WindowOptions { window_bounds: Some(WindowBounds::Windowed(Bounds { origin: Point::default(), size: size(px(1100.), px(700.)) })), ..Default::default() }, cx, |window, cx| {
                let v = cx.new(|cx| StartView::new(window, cx));
                view = Some(v.clone());
                v
            })
        })
        .unwrap();
    let view = view.unwrap();
    // The page's own scan of the documents folder lands first (it would replace the list).
    cx.run_until_parked();
    cx.update(|cx| {
        view.update(cx, |v, cx| {
            (v.source, v.view, v.loaded, v.docs, v.panel) = (Source::All, ViewMode::List, true, docs, None);
            // The default sort (another test's header clicks save theirs to the shared preferences).
            (v.sort, v.descending, v.sort_cleared) = (super::SortKey::Modified, true, true);
            v.refresh_body(cx);
        })
    });
    let w = window;
    // The first frame finds where the list is; the rows are built on the next.
    draw(cx, w);
    draw(cx, w);
    (w, view)
}

fn draw(cx: &mut TestAppContext, w: AnyWindowHandle) {
    cx.update_window(w, |_, window, cx| {
        window.refresh();
        window.draw(cx).clear(cx)
    })
    .unwrap();
    cx.run_until_parked();
}

fn bounds(cx: &mut TestAppContext, w: AnyWindowHandle, name: &str) -> Bounds<Pixels> {
    VisualTestContext::from_window(w, cx).debug_bounds(Box::leak(name.to_string().into_boxed_str())).unwrap_or_else(|| panic!("{name} not drawn"))
}

/// A row's middle, left of its name (a click there selects; one on the name opens).
fn row_at(cx: &mut TestAppContext, w: AnyWindowHandle, name: &str) -> Point<Pixels> {
    let b = bounds(cx, w, &format!("doc-row:{name}"));
    point(b.left() + px(500.), b.center().y)
}

fn click(cx: &mut TestAppContext, w: AnyWindowHandle, at: Point<Pixels>, m: Modifiers) {
    VisualTestContext::from_window(w, cx).simulate_click(at, m);
    draw(cx, w);
}

fn selected(cx: &mut TestAppContext, view: &Entity<StartView>) -> Vec<String> {
    view.read_with(cx, |v, _| v.selection_items().into_iter().map(|i| i.1).collect())
}

/// Live docs-import-feel-1118: the row under the pointer shows its Send to trash and labels icons ("Create, add or edit
/// labels") at its right end, no other row.
#[gpui::test]
fn the_hovered_row_shows_its_quick_actions(cx: &mut TestAppContext) {
    let (w, _view) = page(cx, "hover-icons");
    assert!(VisualTestContext::from_window(w, cx).debug_bounds("row-icon:Labels").is_none(), "none at rest");
    for name in ["C", "D"] {
        let at = row_at(cx, w, name);
        VisualTestContext::from_window(w, cx).simulate_mouse_move(at, None, Modifiers::none());
        draw(cx, w);
        let (row, icon) = (bounds(cx, w, &format!("doc-row:{name}")), bounds(cx, w, "row-icon:Labels"));
        assert!(row.contains(&icon.center()) && icon.left() > row.center().x, "{name}'s icons at its right end: {icon:?} {row:?}");
        assert!(row.contains(&bounds(cx, w, "row-icon:Send to trash").center()));
    }
    // Live docver-1121: with B selected and the pointer on D, only D has them; off the list, none.
    let at = row_at(cx, w, "B");
    click(cx, w, at, Modifiers::none());
    let at = row_at(cx, w, "D");
    VisualTestContext::from_window(w, cx).simulate_mouse_move(at, None, Modifiers::none());
    draw(cx, w);
    assert!(bounds(cx, w, "doc-row:D").contains(&bounds(cx, w, "row-icon:Labels").center()), "on the hovered row, not the selected one");
    VisualTestContext::from_window(w, cx).simulate_mouse_move(point(px(1090.), px(690.)), None, Modifiers::none());
    draw(cx, w);
    assert!(VisualTestContext::from_window(w, cx).debug_bounds("row-icon:Labels").is_none(), "pointer off the list: none, the selected row neither");
}

/// Live docs-import-feel-1118: the row's Send to trash opens its modal; Enter does nothing there, Esc closes it.
#[gpui::test]
fn enter_leaves_the_send_to_trash_modal_open(cx: &mut TestAppContext) {
    let (w, _view) = page(cx, "trash-enter");
    let at = row_at(cx, w, "C");
    VisualTestContext::from_window(w, cx).simulate_mouse_move(at, None, Modifiers::none());
    draw(cx, w);
    let icon = bounds(cx, w, "row-icon:Send to trash").center();
    click(cx, w, icon, Modifiers::none());
    let open = |cx: &mut TestAppContext| cx.update_window(w, |_, window, cx| gpui_kit::component::WindowExt::has_active_dialog(window, cx)).unwrap();
    assert!(open(cx), "the modal");
    VisualTestContext::from_window(w, cx).simulate_keystrokes("enter");
    draw(cx, w);
    assert!(open(cx), "Enter: still open");
    VisualTestContext::from_window(w, cx).simulate_keystrokes("escape");
    draw(cx, w);
    assert!(!open(cx), "Esc closes it");
}

/// Live docver-1121: Delete or Backspace on a selected row opens Send to trash; Esc closes it. Nothing selected: nothing.
#[gpui::test]
fn delete_or_backspace_sends_the_selected_rows_to_trash(cx: &mut TestAppContext) {
    let (w, view) = page(cx, "trash-key");
    let open = |cx: &mut TestAppContext| cx.update_window(w, |_, window, cx| gpui_kit::component::WindowExt::has_active_dialog(window, cx)).unwrap();
    VisualTestContext::from_window(w, cx).simulate_keystrokes("delete");
    draw(cx, w);
    assert!(!open(cx), "no row selected: no modal");
    let at = row_at(cx, w, "C");
    click(cx, w, at, Modifiers::none());
    assert_eq!(selected(cx, &view), ["C"]);
    for key in ["delete", "backspace"] {
        VisualTestContext::from_window(w, cx).simulate_keystrokes(key);
        draw(cx, w);
        assert!(open(cx), "{key}: Send to trash");
        VisualTestContext::from_window(w, cx).simulate_keystrokes("escape");
        draw(cx, w);
        assert!(!open(cx), "Esc closes it");
    }
}

/// Live docver-1121: a document open in another window renames inline (no dialog); the window's document takes the name
/// and its file stays where it is.
#[gpui::test]
fn an_open_document_renames_in_place(cx: &mut TestAppContext) {
    let (w, view) = page(cx, "rename-open");
    let entry = view.read_with(cx, |v, _| v.docs.iter().find(|d| d.name == "C").cloned().unwrap());
    std::fs::create_dir_all(entry.path.parent().unwrap()).unwrap();
    let _ = std::fs::remove_file(&entry.path);
    noshape_store::Document::create(&entry.path, &[("a", b"0")]).unwrap().set_name("C").unwrap();
    cx.update(|cx| {
        cx.set_global(crate::doc_files::OpenDocs::default());
        cx.global_mut::<crate::doc_files::OpenDocs>().0.insert(crate::doc_files::canonical(&entry.path), w);
    });
    cx.update_window(w, |_, window, cx| view.update(cx, |v, cx| v.rename(entry.clone(), window, cx))).unwrap();
    draw(cx, w);
    assert!(!cx.update_window(w, |_, window, cx| gpui_kit::component::WindowExt::has_active_dialog(window, cx)).unwrap(), "no dialog");
    VisualTestContext::from_window(w, cx).simulate_input("C3");
    VisualTestContext::from_window(w, cx).simulate_keystrokes("enter");
    draw(cx, w);
    assert_eq!(noshape_store::Document::open(&entry.path).unwrap().meta().unwrap().name, "C3", "the stored name");
    assert!(!entry.path.with_file_name("C3.noshape").exists(), "the file keeps its name while open");
    cx.update(|cx| cx.remove_global::<crate::doc_files::OpenDocs>());
    let _ = std::fs::remove_dir_all(entry.path.parent().unwrap());
}

/// Live docs-import-feel-1118: Name's header sorts A→Z, then Z→A, then back to the default order (newest first, no
/// arrow); Modified's the same: oldest first, newest first, no sort (live docver-1121).
#[gpui::test]
fn a_header_sorts_up_down_then_back_to_the_default(cx: &mut TestAppContext) {
    let (w, view) = page(cx, "sort-cycle");
    let state = |cx: &mut TestAppContext| view.read_with(cx, |v, _| (v.sort, v.descending, v.sort_cleared));
    let head = |cx: &mut TestAppContext, sel: &str| bounds(cx, w, sel).center();
    let at = head(cx, "doc-head-name");
    for want in [(super::SortKey::Name, false, false), (super::SortKey::Name, true, false), (super::SortKey::Modified, true, true)] {
        click(cx, w, at, Modifiers::none());
        assert_eq!(state(cx), want);
    }
    let at = head(cx, "doc-head-modified");
    click(cx, w, at, Modifiers::none());
    assert_eq!(state(cx), (super::SortKey::Modified, false, false), "oldest first");
    click(cx, w, at, Modifiers::none());
    assert_eq!(state(cx), (super::SortKey::Modified, true, false), "newest first");
    click(cx, w, at, Modifiers::none());
    assert_eq!(state(cx), (super::SortKey::Modified, true, true), "no sort");
}

/// Live docver-1121: in Recently opened no header sorts.
#[gpui::test]
fn recently_opened_headers_dont_sort(cx: &mut TestAppContext) {
    let (w, view) = page(cx, "sort-recent");
    cx.update(|cx| view.update(cx, |v, cx| {
        v.source = Source::Recent;
        v.refresh_body(cx);
    }));
    draw(cx, w);
    draw(cx, w);
    let state = |cx: &mut TestAppContext| view.read_with(cx, |v, _| (v.sort, v.descending, v.sort_cleared));
    for sel in ["doc-head-name", "doc-head-modified"] {
        let at = bounds(cx, w, sel).center();
        click(cx, w, at, Modifiers::none());
        assert_eq!(state(cx), (super::SortKey::Modified, true, true), "{sel}");
    }
}

/// Live2 feel/documents.json: Ctrl+A selects every row, Esc keeps the selection, a click on the list's empty space
/// clears it.
#[gpui::test]
fn ctrl_a_takes_every_row_esc_keeps_them_and_a_click_on_the_empty_list_clears(cx: &mut TestAppContext) {
    let (w, view) = page(cx, "select-all");
    let at = row_at(cx, w, "B");
    click(cx, w, at, Modifiers::none());
    assert_eq!(selected(cx, &view), ["B"]);
    let mut vcx = VisualTestContext::from_window(w, cx);
    vcx.simulate_keystrokes("secondary-a");
    draw(cx, w);
    assert_eq!(selected(cx, &view), ["A", "B", "C", "D", "E"], "Ctrl+A: every row");
    cx.update(|cx| view.update(cx, |v, cx| v.show_selection_panel(false, cx)));
    draw(cx, w);
    assert!(VisualTestContext::from_window(w, cx).debug_bounds("sel-trash").is_none(), "all rows: the panel lists none");
    VisualTestContext::from_window(w, cx).simulate_keystrokes("escape");
    draw(cx, w);
    assert_eq!(selected(cx, &view).len(), 5, "Esc keeps them");
    let (table, last) = (bounds(cx, w, "doc-table"), bounds(cx, w, "doc-row:E"));
    assert!(table.bottom() - last.bottom() > px(40.), "empty space under the rows: {table:?} {last:?}");
    click(cx, w, point(table.left() + px(200.), last.bottom() + px(30.)), Modifiers::none());
    assert!(selected(cx, &view).is_empty(), "a click on the empty list clears");
}

/// Live2 feel/documents.json `contextMenuForN` / `rightPanelForN` (docs-f1-shift-range): with several rows selected a
/// right-click on one of them keeps them and lists Details, Labels…, Move selected (N) to…, Send to trash; the panel
/// lists them with Trash and Deselect all, which clears them.
#[gpui::test]
fn several_selected_rows_get_their_own_menu_and_panel(cx: &mut TestAppContext) {
    let (w, view) = page(cx, "many");
    let at = row_at(cx, w, "B");
    click(cx, w, at, Modifiers::none());
    let at = row_at(cx, w, "D");
    click(cx, w, at, Modifiers::shift());
    assert_eq!(selected(cx, &view), ["B", "C", "D"]);
    let at = row_at(cx, w, "C");
    let mut vcx = VisualTestContext::from_window(w, cx);
    vcx.simulate_mouse_down(at, MouseButton::Right, Modifiers::none());
    vcx.simulate_mouse_up(at, MouseButton::Right, Modifiers::none());
    draw(cx, w);
    let menu = cx.read(crate::os_menu::ui_state).expect("a menu");
    assert_eq!(menu["rows"], serde_json::json!(["Details", "Labels…", "Move selected (3) to…", "Send to trash"]), "{menu}");
    assert_eq!(selected(cx, &view).len(), 3, "the right-click kept the selection");
    VisualTestContext::from_window(w, cx).simulate_keystrokes("escape");
    cx.update(|cx| view.update(cx, |v, cx| v.show_selection_panel(false, cx)));
    draw(cx, w);
    let (trash, deselect) = (bounds(cx, w, "sel-trash"), bounds(cx, w, "sel-deselect"));
    assert!(trash.right() < deselect.left(), "Trash, then Deselect all: {trash:?} {deselect:?}");
    click(cx, w, deselect.center(), Modifiers::none());
    assert!(selected(cx, &view).is_empty(), "Deselect all");
}

/// Live2 feel/documents.json `ghost`: a row moved 4 px is a click, 5 px drags it; the light-blue pill (48 px) sits 10 px
/// left of and 8 px above the pointer and the dragged rows fade.
#[gpui::test]
fn a_row_drags_from_5_px_under_a_pill_up_and_left_of_the_pointer(cx: &mut TestAppContext) {
    let (w, view) = page(cx, "drag");
    let at = row_at(cx, w, "C");
    let mut vcx = VisualTestContext::from_window(w, cx);
    vcx.simulate_mouse_move(at, None, Modifiers::none());
    vcx.simulate_mouse_down(at, MouseButton::Left, Modifiers::none());
    vcx.simulate_mouse_move(at + point(px(4.), px(0.)), Some(MouseButton::Left), Modifiers::none());
    draw(cx, w);
    assert!(!cx.update(|cx| cx.has_active_drag()), "4 px: a click");
    let to = at + point(px(5.), px(0.));
    VisualTestContext::from_window(w, cx).simulate_mouse_move(to, Some(MouseButton::Left), Modifiers::none());
    draw(cx, w);
    assert!(cx.update(|cx| cx.has_active_drag()), "5 px: a drag");
    let pill = bounds(cx, w, "docs-drag-ghost");
    let near = |a: Pixels, b: Pixels| (f32::from(a) - f32::from(b)).abs() < 0.5;
    assert!(near(pill.size.height, px(48.)) && near(pill.left(), to.x - px(10.)) && near(pill.top(), to.y - px(8.)), "{pill:?} at {to:?}");
    assert_eq!(view.read_with(cx, |v, _| v.dragging.iter().map(|p| p.file_stem().unwrap().to_string_lossy().to_string()).collect::<Vec<_>>()), ["C"], "the row fades");
    VisualTestContext::from_window(w, cx).simulate_mouse_up(to, MouseButton::Left, Modifiers::none());
    draw(cx, w);
}

/// Live2 11-13: a single click on a document's name opens it; Ctrl or Shift on the name select as on the row.
#[gpui::test]
fn a_click_on_a_documents_name_opens_it(cx: &mut TestAppContext) {
    let (w, view) = page(cx, "name-click");
    let name = bounds(cx, w, "doc-row:B");
    let before = OPENED.load(Ordering::SeqCst);
    // The name starts after the 64 px thumbnail (16 px in, a 14 px gap).
    let on_name = point(name.left() + px(16. + 64. + 14. + 4.), name.center().y);
    click(cx, w, on_name, Modifiers::secondary_key());
    assert_eq!((OPENED.load(Ordering::SeqCst) - before, selected(cx, &view)), (0, vec!["B".to_string()]), "Ctrl: selected, not opened");
    click(cx, w, on_name, Modifiers::none());
    assert_eq!(OPENED.load(Ordering::SeqCst) - before, 1, "a plain click opens it");
}

/// bm 20: a hovered row #def1fb, a selected one #b2ddf6.
#[test]
fn rows_light_and_select_in_lives_blues() {
    let pal = noshape_app_core::theme::PALETTE;
    assert_eq!((pal.row_hover, pal.accent_soft), (noshape_app_core::theme::rgb_const(0xdef1fb), noshape_app_core::theme::rgb_const(0xb2ddf6)));
}

/// Live2 feel/documents.json `trashForN`: the question and the toast for one or several rows.
#[test]
fn send_to_trash_words_count_the_rows() {
    assert_eq!(super::trash_question(1), "1 selection will be sent to the trash.");
    assert_eq!(super::trash_question(3), "3 selections will be sent to the trash.");
    assert_eq!(super::trashed_toast(3), "3 items have been sent to the trash and will be automatically deleted after 30 days.");
}

/// Live 2435: a row's Versions and history opens the document's own page in place of the list ("Versions and history
/// of <doc>", the top bar without its search), no document window; the logo goes back to the list.
#[gpui::test]
fn versions_and_history_opens_its_page_in_place_of_the_list(cx: &mut TestAppContext) {
    let (w, view) = page(cx, "versions-page");
    let path = view.read_with(cx, |v, _| v.docs.iter().find(|d| d.name == "C").unwrap().path.clone());
    cx.update_window(w, |_, window, cx| view.update(cx, |v, cx| v.open_history(path, "C".into(), window, cx))).unwrap();
    draw(cx, w);
    let mut vcx = VisualTestContext::from_window(w, cx);
    assert!(vcx.debug_bounds("versions-page").is_some(), "the page");
    assert!(vcx.debug_bounds("doc-row:C").is_none(), "no list under it");
    let logo = bounds(cx, w, "start-logo").center();
    VisualTestContext::from_window(w, cx).simulate_click(logo, Modifiers::none());
    draw(cx, w);
    draw(cx, w);
    assert!(VisualTestContext::from_window(w, cx).debug_bounds("versions-page").is_none(), "the logo goes back");
    assert!(VisualTestContext::from_window(w, cx).debug_bounds("doc-row:C").is_some());
}

/// Live docver-1121: emptying the search box (Backspace, no Enter) keeps the results; Enter on the empty box goes back
/// to the folder.
#[gpui::test]
fn an_emptied_search_box_keeps_the_results_until_enter(cx: &mut TestAppContext) {
    let (w, view) = page(cx, "search-empty");
    let search = view.read_with(cx, |v, _| v.search.clone());
    cx.update_window(w, |_, window, cx| search.update(cx, |s, cx| s.focus(window, cx))).unwrap();
    VisualTestContext::from_window(w, cx).simulate_input("B");
    VisualTestContext::from_window(w, cx).simulate_keystrokes("enter");
    draw(cx, w);
    let query = |cx: &mut TestAppContext| view.read_with(cx, |v, _| v.query.clone());
    assert_eq!(query(cx), "B", "searched");
    VisualTestContext::from_window(w, cx).simulate_keystrokes("backspace");
    draw(cx, w);
    assert_eq!(query(cx), "B", "the box emptied: still the results");
    VisualTestContext::from_window(w, cx).simulate_keystrokes("enter");
    draw(cx, w);
    assert_eq!(query(cx), "", "Enter on the empty box: the folder again");
    // Its ⊗ empties the box, keeps the results and leaves the box (focus on the page).
    VisualTestContext::from_window(w, cx).simulate_input("B");
    VisualTestContext::from_window(w, cx).simulate_keystrokes("enter");
    draw(cx, w);
    let at = bounds(cx, w, "input-clear").center();
    click(cx, w, at, Modifiers::none());
    assert_eq!(query(cx), "B", "⊗: still the results");
    assert_eq!(view.read_with(cx, |v, cx| v.search.read(cx).value().to_string()), "", "⊗ empties the box");
    let focused = cx.update_window(w, |_, window, cx| gpui_kit::Focusable::focus_handle(search.read(cx), cx).is_focused(window)).unwrap();
    assert!(!focused, "⊗: the box loses focus");
}

/// Live docs-page-gaps-1122: a document's menu is 167 px wide with no shadow; ↓ closes it and moves the list's selected
/// row down, ↑ (with a menu open again) up.
#[gpui::test]
fn arrows_close_a_documents_menu_and_move_the_selection(cx: &mut TestAppContext) {
    let (w, view) = page(cx, "menu-arrows");
    let open_menu = |cx: &mut TestAppContext, name: &str| {
        let at = row_at(cx, w, name);
        let mut vcx = VisualTestContext::from_window(w, cx);
        vcx.simulate_mouse_down(at, MouseButton::Right, Modifiers::none());
        vcx.simulate_mouse_up(at, MouseButton::Right, Modifiers::none());
        draw(cx, w);
    };
    let at = row_at(cx, w, "B");
    click(cx, w, at, Modifiers::none());
    open_menu(cx, "B");
    let menu = cx.read(crate::os_menu::ui_state).expect("a menu");
    assert_eq!(menu["rows"][0], "Open", "{menu}");
    let b = cx.read(crate::os_menu::open_bounds).expect("the menu's bounds");
    assert!(b.size.width >= px(167.), "167 px wide: {b:?}");
    VisualTestContext::from_window(w, cx).simulate_keystrokes("down");
    draw(cx, w);
    assert!(cx.read(crate::os_menu::ui_state).is_none(), "↓ closed the menu");
    assert_eq!(selected(cx, &view), ["C"], "↓ moved the selection down");
    open_menu(cx, "C");
    VisualTestContext::from_window(w, cx).simulate_keystrokes("up");
    draw(cx, w);
    assert!(cx.read(crate::os_menu::ui_state).is_none(), "↑ closed the menu");
    assert_eq!(selected(cx, &view), ["B"], "↑ moved the selection up");
}

/// Live docs-page-gaps-1122: over a document's name the tooltip "Click to open <name>." shows above it.
#[gpui::test]
fn a_documents_name_tips_click_to_open_above_it(cx: &mut TestAppContext) {
    let (w, _view) = page(cx, "name-tip");
    let name = bounds(cx, w, "doc-name:C");
    let mut vcx = VisualTestContext::from_window(w, cx);
    vcx.simulate_mouse_move(name.center(), None, Modifiers::none());
    for _ in 0..20 {
        vcx.executor().advance_clock(std::time::Duration::from_millis(50));
        draw(cx, w);
    }
    crate::ui_script::clear_tooltip();
    draw(cx, w);
    assert_eq!(crate::ui_script::shown_tooltip().as_deref(), Some("Click to open C."));
}

/// Live docs-page-gaps-1122: a hovered folder tile shows its Send to trash (in a white pill at its right end) and its
/// name tips "Click to open <name>." above it; another tile shows nothing.
#[gpui::test]
fn a_hovered_folder_tile_shows_its_trash_and_name_tip(cx: &mut TestAppContext) {
    let (w, view) = page(cx, "tile-hover");
    let dir = std::env::temp_dir().join(format!("noshape-start-feel-tiles-{}", std::process::id()));
    cx.update(|cx| {
        view.update(cx, |v, cx| {
            (v.view, v.folders_open, v.outside) = (ViewMode::Grid, true, vec![dir.join("Parts"), dir.join("Tools")]);
            v.refresh_body(cx);
        })
    });
    draw(cx, w);
    draw(cx, w);
    let tile = bounds(cx, w, "folder-tile:Parts");
    let mut vcx = VisualTestContext::from_window(w, cx);
    vcx.simulate_mouse_move(tile.center(), None, Modifiers::none());
    draw(cx, w);
    let trash = bounds(cx, w, "folder-trash");
    assert!(tile.contains(&trash.center()) && trash.center().x > tile.center().x, "the pill at the tile's right end: {trash:?} in {tile:?}");
    let name = bounds(cx, w, "folder-name:Parts");
    let mut vcx = VisualTestContext::from_window(w, cx);
    vcx.simulate_mouse_move(name.center(), None, Modifiers::none());
    for _ in 0..20 {
        vcx.executor().advance_clock(std::time::Duration::from_millis(50));
        draw(cx, w);
    }
    crate::ui_script::clear_tooltip();
    draw(cx, w);
    assert_eq!(crate::ui_script::shown_tooltip().as_deref(), Some("Click to open Parts."));
    let other = bounds(cx, w, "folder-tile:Tools");
    VisualTestContext::from_window(w, cx).simulate_mouse_move(other.center(), None, Modifiers::none());
    draw(cx, w);
    let trash = bounds(cx, w, "folder-trash");
    assert!(other.contains(&trash.center()), "only the hovered tile's pill: {trash:?} in {other:?}");
}

/// Live docs-page-gaps-1122: over a row's thumbnail a 302 x 183 popover shows the thumbnail large above the icon,
/// centred on it.
#[gpui::test]
fn a_rows_thumbnail_pops_up_large_above_it(cx: &mut TestAppContext) {
    let (w, _view) = page(cx, "thumb-pop");
    let thumb = bounds(cx, w, "doc-thumb:E");
    let mut vcx = VisualTestContext::from_window(w, cx);
    vcx.simulate_mouse_move(thumb.center(), None, Modifiers::none());
    for _ in 0..20 {
        vcx.executor().advance_clock(std::time::Duration::from_millis(50));
        draw(cx, w);
    }
    let pop = bounds(cx, w, "thumb-popover");
    assert_eq!((pop.size.width, pop.size.height), (px(302.), px(183.)), "{pop:?}");
    assert!(pop.bottom() < thumb.top() && thumb.top() - pop.bottom() <= px(10.), "just above the icon: {pop:?} {thumb:?}");
    assert!((pop.center().x - thumb.center().x).abs() < px(1.), "centred on it: {pop:?} {thumb:?}");
}
