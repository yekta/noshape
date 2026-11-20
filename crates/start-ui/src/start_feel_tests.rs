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
        open_document_history: |_, _, _| Ok(()),
        build_label: String::new,
        import_dialog: |_, _, _, _| {},
        help_items: |m| m,
    });
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
        .map(|(i, n)| DocEntry { path: dir.join(format!("{n}.noshape")), name: (*n).into(), modified: 1_760_000_000_000 - i as i64 * 60_000, modified_by: "me".into(), owner: "me".into(), description: String::new(), workspace: "Main".into(), thumbnail: None, recent_rank: None, broken: false, trashed: false })
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
