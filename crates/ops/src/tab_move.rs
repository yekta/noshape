//! Tab › Move to document…: tabs and tab folders copied into a new or an
//! existing document, then deleted here (not undoable, as live's).

use crate::session::Session;
use anyhow::{Context as _, Result, ensure};
use noshape_model::document::{Document, DocumentSettings, META_BLOB, TabId};
use noshape_store::Document as Store;
use std::path::{Path, PathBuf};

/// Where Move to document… puts the tabs.
pub enum Destination<'a> {
    /// A new document of this name, next to the source document.
    New(&'a str),
    /// An existing document (not open in another window).
    Existing(&'a Path),
}

/// The tabs `tabs` reference (live 2323: the Variable Studio a moved Part Studio uses), the moved ones aside, in tab
/// order: Move to document… moves them along ("Move all referenced tabs").
pub fn referenced(doc: &Document, tabs: &[TabId]) -> Vec<TabId> {
    doc.ordered_tabs().into_iter().map(|t| t.id).filter(|x| !tabs.contains(x) && doc.tab_users(*x).iter().any(|u| tabs.contains(u))).collect()
}

/// Whether another tab uses one of `tabs` (live 2323: moving a Variable Studio the Part Studios use shows "One or more
/// tabs being moved contain workspace references. …").
pub fn used_elsewhere(doc: &Document, tabs: &[TabId]) -> bool {
    tabs.iter().any(|t| doc.tab_users(*t).iter().any(|u| !tabs.contains(u)))
}

/// Live's bubble after the move: "Part Studio 2, Variable Studio 1 have been moved to move 2323."
pub fn moved_message(names: &[String], to: &str) -> String {
    let verb = if names.len() == 1 { "has" } else { "have" };
    format!("{} {verb} been moved to {to}.", names.join(", "))
}

/// The new document's file: `<name>.noshape` in `dir`, `<name> (2)` and on when taken.
pub fn new_document_file(dir: &Path, name: &str) -> PathBuf {
    (1..)
        .map(|n| if n == 1 { dir.join(format!("{name}.noshape")) } else { dir.join(format!("{name} ({n}).noshape")) })
        .find(|p| !p.exists())
        .expect("a free name")
}

/// Moves `tabs` (tabs or folders, with their contents) into `dest`; returns the document they went to.
pub fn move_tabs(session: &mut Session, tabs: &[TabId], dest: Destination) -> Result<PathBuf> {
    let path = copy_out(&session.doc, &session.path, tabs, dest)?;
    let what = description(&session.doc, tabs);
    session.edit_document_not_undoable(&what, |d| remove(d, tabs))?;
    Ok(path)
}

/// The edit's description in this document's history.
pub fn description(src: &Document, tabs: &[TabId]) -> String {
    format!("Move tab to document : {}", labels(src, tabs).join(", "))
}

fn labels(src: &Document, tabs: &[TabId]) -> Vec<String> {
    tabs.iter().filter_map(|t| src.tab(*t).map(|x| x.name.clone()).or_else(|| src.folder(*t).map(|f| f.name.clone()))).collect()
}

/// The first half of [`move_tabs`]: `tabs` of `src` (the document at `src_path`) written into `dest`; the caller then
/// removes them with [`remove`] under [`description`].
pub fn copy_out(src: &Document, src_path: &Path, tabs: &[TabId], dest: Destination) -> Result<PathBuf> {
    let labels = labels(src, tabs);
    ensure!(!labels.is_empty() && labels.len() == tabs.len(), "Select a tab to move");
    let mut left = src.clone();
    remove(&mut left, tabs);
    ensure!(!left.tabs.is_empty(), "A document must have at least one tab");
    let path = match dest {
        Destination::New(name) => {
            let name = name.trim();
            ensure!(!name.is_empty(), "Enter a document name");
            let dir = src_path.parent().map(Path::to_path_buf).unwrap_or_default();
            let path = new_document_file(&dir, name);
            let mut other = Document { settings: DocumentSettings { units: src.settings.units, ..Default::default() }, tabs: Vec::new() };
            for t in tabs {
                copy(src, &mut other, *t, None);
            }
            let initial: Vec<(String, Vec<u8>)> = other.to_blobs().into_iter().collect();
            let mut store = Store::create(&path, &initial).with_context(|| format!("creating {}", path.display()))?;
            store.set_name(name)?;
            let _ = store.checkpoint();
            path
        }
        Destination::Existing(target) => {
            let mut store = Store::open(target).with_context(|| format!("opening {}", target.display()))?;
            let branch = store.default_branch()?;
            let head = store.head(branch)?;
            let mut other = Document::from_blobs(&store.read_all(head)?)?;
            let before: Vec<TabId> = other.tabs.iter().map(|t| t.id).collect();
            for t in tabs {
                copy(src, &mut other, *t, None);
            }
            let mut changes: Vec<(String, Option<Vec<u8>>)> = other.tabs.iter().filter(|t| !before.contains(&t.id)).map(|t| (Document::blob_name(t.id), other.tab_blob(t.id))).collect();
            changes.push((META_BLOB.to_string(), Some(other.settings_blob())));
            store.commit(branch, &changes, &format!("Insert tab : {}", labels.join(", ")))?;
            let _ = store.checkpoint();
            target.to_path_buf()
        }
    };
    Ok(path)
}

/// Copies `id` (a tab, or a folder and what it holds) into `parent` of `other`.
fn copy(src: &Document, other: &mut Document, id: TabId, parent: Option<TabId>) {
    if let Some(t) = src.tab(id) {
        // A name the other document has: the next free one of its kind ("Assembly 1" → "Assembly 2").
        let base = t.name.rsplit_once(' ').filter(|(_, n)| n.parse::<u32>().is_ok()).map_or(t.name.as_str(), |(b, _)| b);
        let name = if other.tabs.iter().any(|x| x.name == t.name) { other.next_tab_name(base) } else { t.name.clone() };
        let new = other.add_tab(&name, t.content.clone());
        other.set_parent(new, parent);
    } else if let Some(f) = src.folder(id) {
        let new = other.add_folder(&f.name, parent);
        for c in src.children(Some(id)) {
            copy(src, other, c, Some(new));
        }
    }
}

/// Deletes `tabs` (folders with what they hold) from `d`.
pub fn remove(d: &mut Document, tabs: &[TabId]) {
    for t in tabs {
        if d.is_folder(*t) {
            d.delete_folder_tree(*t);
        } else {
            d.remove_tab(*t);
        }
    }
}
