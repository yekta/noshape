//! Quitting commits every open document's pending edit (an assembly Insert session, the appearance editor's colour),
//! also while something else still holds the document (logout and shutdown quit the app the same way).

use crate::doc_model::DocModel;
use gpui_kit::{AppContext as _, TestAppContext};
use noshape_model::Document;

#[gpui::test]
fn quitting_commits_a_pending_edit(cx: &mut TestAppContext) {
    let dir = std::env::temp_dir().join(format!("noshape-quit-commit-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("q.noshape");
    for ext in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{ext}", path.display()));
    }
    let session = crate::session::Session::create(&path, Document::new()).unwrap();
    let tab = session.doc.tabs[0].id;
    let doc = cx.new(|_| DocModel::new(session));
    doc.update(cx, |d, cx| {
        d.edit_tab_deferred(tab, "Insert", cx, |c| {
            if let noshape_model::TabContent::PartStudio(ps) = c {
                ps.rollback = Some(0);
            }
        });
        assert!(d.has_pending());
    });
    let held = doc.clone();
    cx.quit();
    assert!(!held.read_with(cx, |d, _| d.has_pending()));
    let reopened = crate::session::Session::open(&path).unwrap();
    assert!(matches!(&reopened.doc.tab(tab).unwrap().content, noshape_model::TabContent::PartStudio(ps) if ps.rollback == Some(0)));
    drop((held, doc));
    let _ = std::fs::remove_dir_all(&dir);
}
