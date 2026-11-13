//! The Tab manager's drag and drop (lead 11-13, interaction feel): rows dropped on a folder go into it; rows dropped
//! on a tab row land before or after it, in its folder. Neither is an undo step (Onshape's tab operations, store's
//! NOT_UNDOABLE_PREFIXES).

use crate::doc_model::DocModel;
use crate::tab_manager::{TabManager, TmDrag, TmDrop, tm_zone};
use gpui_kit::{AppContext as _, Bounds, Entity, Point, TestAppContext, WeakEntity, WindowBounds, WindowOptions, px, size};
use noshape_model::document::TabId;
use noshape_ops::{Ctx, registry};
use serde_json::{Value, json};

fn call(ctx: &mut Ctx, tool: &str, args: Value) -> Value {
    let op = registry::by_tool_name(tool).unwrap_or_else(|| panic!("no tool {tool}"));
    registry::run(ctx, op, &args).unwrap_or_else(|e| panic!("{tool}: {e:?}")).value
}

/// The new document's first tab, A, B, C and an empty Folder 1 at the root, in a Tab manager.
fn manager(cx: &mut TestAppContext, name: &str) -> (Entity<DocModel>, Entity<TabManager>, [TabId; 4], TabId) {
    cx.update(gpui_kit::init);
    let gpu = crate::gpu::Gpu::headless().unwrap();
    cx.update(|cx| cx.set_global(crate::gpu::GpuGlobal(gpu)));
    let dir = std::env::temp_dir().join(format!("noshape-tab-manager-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("tabs.noshape");
    for ext in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{ext}", path.display()));
    }
    {
        let mut ctx = Ctx::new();
        call(&mut ctx, "document_create", json!({ "path": path.to_str().unwrap() }));
        for name in ["A", "B", "C"] {
            call(&mut ctx, "tab_create", json!({ "kind": "partstudio", "name": name }));
        }
    }
    let session = crate::session::Session::open(&path).unwrap();
    let doc = cx.new(|_| DocModel::new(session));
    let folder = doc.update(cx, |d, cx| {
        let mut id = None;
        d.edit_document("New folder", cx, |doc| id = Some(doc.add_folder("Folder 1", None)));
        id.unwrap()
    });
    let ids = doc.read_with(cx, |d, _| {
        let tabs = d.session.doc.ordered_tabs();
        let named = |n: &str| tabs.iter().find(|t| t.name == n).map(|t| t.id).unwrap_or_else(|| panic!("no tab {n}"));
        [tabs[0].id, named("A"), named("B"), named("C")]
    });
    let bounds = Bounds { origin: Point::default(), size: size(px(400.), px(800.)) };
    let mut tm = None;
    let d2 = doc.clone();
    cx.update(|cx| {
        cx.open_window(WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() }, |window, cx| {
            let e = cx.new(|cx| TabManager::new(d2, None, WeakEntity::new_invalid(), window, cx));
            tm = Some(e.clone());
            e
        })
        .unwrap();
    });
    (doc, tm.unwrap(), ids, folder)
}

fn drag(ids: Vec<TabId>) -> TmDrag {
    TmDrag { label: format!("{} items", ids.len()), ids, offset: Point::default() }
}

/// Live2 (feel/documents.json `folderRowDropZones`, rows ~53 px): over a folder row, the top third drops above it
/// (to 12–18 px below its top), the middle into it (to 32–36 px), the rest below it; a tab row by its halves.
#[test]
fn a_folder_row_takes_drops_in_its_middle_third() {
    let h = 54.;
    assert_eq!(tm_zone(true, 11., h), TmDrop::Before);
    assert_eq!(tm_zone(true, 19., h), TmDrop::Into);
    assert_eq!(tm_zone(true, 31., h), TmDrop::Into);
    assert_eq!(tm_zone(true, 37., h), TmDrop::After);
    assert_eq!(tm_zone(false, 26., h), TmDrop::Before);
    assert_eq!(tm_zone(false, 28., h), TmDrop::After);
}

#[gpui::test]
fn rows_dropped_beside_a_folder_land_before_or_after_it(cx: &mut TestAppContext) {
    let (doc, tm, [_, a, _, _], folder) = manager(cx, "beside-folder");
    let at = |cx: &mut TestAppContext, t: TabId| doc.read_with(cx, |d, _| d.session.doc.children(None).iter().position(|x| *x == t).unwrap());
    tm.update(cx, |t, cx| {
        t.drop_at = Some((folder, TmDrop::Before));
        t.drop_by_row(&drag(vec![a]), folder, cx);
    });
    assert_eq!(at(cx, a) + 1, at(cx, folder), "A right before the folder");
    assert_eq!(doc.read_with(cx, |d, _| d.session.doc.parent_of(a)), None, "and not in it");
}

#[gpui::test]
fn rows_dropped_on_a_folder_go_into_it(cx: &mut TestAppContext) {
    let (doc, tm, [_, a, b, _], folder) = manager(cx, "into-folder");
    let drag = drag(vec![a, b]);
    tm.update(cx, |t, cx| t.drop_into_folder(&drag, folder, cx));
    let parents = |cx: &mut TestAppContext| doc.read_with(cx, |d, _| [a, b].map(|t| d.session.doc.parent_of(t)));
    assert_eq!(parents(cx), [Some(folder); 2], "both in the folder");
    doc.update(cx, |d, cx| d.undo(cx));
    assert_eq!(parents(cx), [Some(folder); 2], "not undone (a tab operation)");
}

#[gpui::test]
fn rows_dropped_on_a_tab_row_land_before_or_after_it(cx: &mut TestAppContext) {
    let (doc, tm, [first, a, b, c], _) = manager(cx, "by-row");
    let at = |cx: &mut TestAppContext, t: TabId| doc.read_with(cx, |d, _| d.session.doc.children(None).iter().position(|x| *x == t).unwrap());
    // C onto A's upper half: just before A.
    tm.update(cx, |t, cx| {
        t.drop_at = Some((a, TmDrop::Before));
        t.drop_by_row(&drag(vec![c]), a, cx);
    });
    assert_eq!(at(cx, c) + 1, at(cx, a), "C right before A");
    // The first tab onto B's lower half: just after B.
    tm.update(cx, |t, cx| {
        t.drop_at = Some((b, TmDrop::After));
        t.drop_by_row(&drag(vec![first]), b, cx);
    });
    assert_eq!(at(cx, b) + 1, at(cx, first), "the first tab right after B");
    // A row dropped on itself stays.
    let before = at(cx, a);
    tm.update(cx, |t, cx| t.drop_by_row(&drag(vec![a]), a, cx));
    assert_eq!(at(cx, a), before);
}
