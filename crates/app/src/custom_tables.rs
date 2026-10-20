//! FeatureScript custom tables (`defineTable`) offered to the Part Studio
//! Custom tables panel and to drawing custom tables (live behaviours.md
//! "Custom tables (defineTable)", help Drawing__custom_table): the
//! workspace's own tables ("Workspace custom tables") and the tables added
//! from a document version through Add custom tables ("My custom tables",
//! kept per user, available in every document).

use crate::theme::p;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::TabContent;
use noshape_model::custom::table::TableResult;
use noshape_model::document::{Document, TabId};
use noshape_model::regen::context::Context as RegenContext;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// A custom table: Feature Studio `fs`'s exported constant `name`, from the
/// workspace (`version` None) or a document version (`version`: an
/// `ext:` key of [`crate::derived::ExternalRef`] naming the document,
/// microversion and Feature Studio).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TableRef {
    pub fs: TabId,
    pub name: String,
    /// "Table Type Name".
    pub display: String,
    #[serde(default)]
    pub version: Option<String>,
    /// The version's name ("V3"), for the Feature Studio link.
    #[serde(default)]
    pub version_label: Option<String>,
    /// The Feature Studio tab's name at that version.
    #[serde(default)]
    pub fs_name: String,
}

fn my_tables_path() -> std::path::PathBuf {
    crate::files::config_dir().join("custom_tables.json")
}

/// "My custom tables": the tables the user added (newest last).
pub fn my_tables() -> Vec<TableRef> {
    std::fs::read(my_tables_path()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

/// Add a table to "My custom tables" (once).
pub fn add_my_table(t: TableRef) {
    let mut list = my_tables();
    if !list.contains(&t) {
        list.push(t);
        let _ = std::fs::create_dir_all(crate::files::config_dir());
        let _ = std::fs::write(my_tables_path(), serde_json::to_vec_pretty(&list).unwrap_or_default());
    }
}

/// The tables defined by the document's own Feature Studios (live: tables
/// without a precondition are not offered; the model leaves them out).
pub fn workspace_tables(doc: &Document) -> Vec<TableRef> {
    let scripts = noshape_model::custom::DocumentScripts::from_document(doc);
    scripts
        .studios()
        .flat_map(|(tab, name, c)| c.tables.iter().map(move |t| TableRef { fs: tab, name: t.name.clone(), display: t.display_name.clone(), version: None, version_label: None, fs_name: name.to_string() }))
        .collect()
}

/// The tables of the Feature Studios of a document snapshot (a version),
/// grouped by studio: (studio tab, studio name, tables).
pub fn version_tables(doc: &Document, path: &std::path::Path, mv: noshape_store::MicroversionId, label: &str) -> Vec<(TabId, String, Vec<TableRef>)> {
    doc.ordered_tabs()
        .into_iter()
        .filter_map(|t| match &t.content {
            TabContent::FeatureStudio(f) => {
                let c = noshape_model::custom::compile(t.id, &f.source);
                let key = crate::derived::ExternalRef { path: path.to_path_buf(), microversion: mv, tab: t.id.to_string() }.key();
                let tables: Vec<TableRef> = c.tables.iter().map(|d| TableRef { fs: t.id, name: d.name.clone(), display: d.display_name.clone(), version: Some(key.clone()), version_label: Some(label.to_string()), fs_name: t.name.clone() }).collect();
                (!tables.is_empty()).then(|| (t.id, t.name.clone(), tables))
            }
            _ => None,
        })
        .collect()
}

/// The document a table's Feature Studio comes from: the workspace, or
/// the snapshot at its version.
pub fn source_document(t: &TableRef, workspace: &Document) -> Option<Arc<Document>> {
    match &t.version {
        None => Some(Arc::new(workspace.clone())),
        Some(key) => {
            let r = crate::derived::ExternalRef::parse(key)?;
            crate::derived::external_snapshot(&r.path, r.microversion).ok()
        }
    }
}

/// Run a table on a Part Studio's final context (call off the main thread).
pub fn evaluate(t: &TableRef, fs_doc: &Document, context: &RegenContext) -> TableResult {
    noshape_model::drawing::tables::evaluate_custom(fs_doc, t.fs, &t.name, t.version.is_some(), context)
}

/// The two-letter badge of a table (live "Fs" for "FS sizes", "Ft" for
/// "FS two tables"): the first letters of the first two words.
pub fn initials(name: &str) -> String {
    let mut words = name.split_whitespace();
    let first = words.next().and_then(|w| w.chars().next()).map(|c| c.to_uppercase().collect::<String>()).unwrap_or_default();
    let second = match words.next() {
        Some(w) => w.chars().next().map(|c| c.to_lowercase().collect::<String>()).unwrap_or_default(),
        None => name.chars().nth(1).map(|c| c.to_lowercase().collect::<String>()).unwrap_or_default(),
    };
    first + &second
}

/// The badge (live `div.tool-initials-icon`: 20 px, 1 px #999 border,
/// radius 3, 11 px text).
pub fn badge(name: &str) -> Div {
    div()
        .size(px(20.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(3.))
        .border_1()
        .border_color(gpui_kit::rgb(0x999999))
        .text_size(px(11.))
        .text_color(gpui_kit::rgb(0x333333))
        .child(initials(name))
}

/// The table list of a selector (live 636/646): "My custom tables" then
/// "Workspace custom tables" (grey 13 px headings), items with their badge;
/// the chosen one light blue. `pick` gets the clicked table.
pub fn table_list(id: &'static str, mine: &[TableRef], workspace: &[TableRef], chosen: Option<&TableRef>, width: f32, pick: impl Fn(TableRef, &mut Window, &mut App) + 'static) -> AnyElement {
    let pal = p();
    let pick = std::rc::Rc::new(pick);
    let mut list = v_flex().id(id).w(px(width)).max_h(px(400.)).overflow_y_scroll().py(px(2.)).bg(gpui_kit::white()).border_1().border_color(gpui_kit::rgb(0xcccccc)).shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(3.)), blur_radius: px(6.), spread_radius: px(0.), inset: false }]);
    for (heading, items) in [("My custom tables", mine), ("Workspace custom tables", workspace)] {
        if items.is_empty() {
            continue;
        }
        list = list.child(div().h(px(24.)).px(px(6.)).flex().items_center().text_size(px(13.)).text_color(gpui_kit::rgb(0x888888)).border_b_1().border_color(gpui_kit::rgb(0xe6e6e6)).child(heading));
        for (i, t) in items.iter().enumerate() {
            let on = chosen == Some(t);
            let (t2, pick) = (t.clone(), pick.clone());
            list = list.child(
                h_flex()
                    .id(SharedString::from(format!("{id}-{heading}-{i}")))
                    .h(px(30.))
                    .px(px(6.))
                    .gap(px(5.))
                    .cursor_pointer()
                    .text_size(px(14.))
                    .text_color(pal.text)
                    .when(on, |d| d.bg(gpui_kit::rgb(0xdef1fb)))
                    .hover(|d| d.bg(gpui_kit::rgb(0xdef1fb)))
                    .child(badge(&t.display))
                    .child(div().truncate().child(t.display.clone()))
                    .on_click(move |_, w, cx| pick(t2.clone(), w, cx)),
            );
        }
    }
    list.into_any_element()
}

/// A custom table's group in the FeatureScript notices pane, shown while
/// the table throws (live 652: "FS sizes (pvz 2parts)" with the error, its
/// stack and the table's println output).
#[derive(Clone, Debug, PartialEq)]
pub struct NoticeGroup {
    pub title: String,
    pub entries: Vec<noshape_model::custom::run::MonitorEntry>,
    pub output: String,
}

/// Groups by (document, Part Studio).
#[derive(Default)]
struct TableNotices(std::collections::HashMap<(EntityId, TabId), NoticeGroup>);

impl Global for TableNotices {}

/// Set (or clear) the notices group of a Part Studio's shown table.
pub fn set_notices(doc: EntityId, studio: TabId, group: Option<NoticeGroup>, cx: &mut App) {
    let map = &mut cx.default_global::<TableNotices>().0;
    let changed = match group {
        Some(g) => map.insert((doc, studio), g.clone()).as_ref() != Some(&g),
        None => map.remove(&(doc, studio)).is_some(),
    };
    if changed {
        cx.refresh_windows();
    }
}

/// The custom table groups of a document, by Part Studio.
pub fn notice_groups(doc: EntityId, cx: &App) -> Vec<(TabId, NoticeGroup)> {
    let mut out: Vec<(TabId, NoticeGroup)> = cx.try_global::<TableNotices>().map(|m| m.0.iter().filter(|((d, _), _)| *d == doc).map(|((_, t), g)| (*t, g.clone())).collect()).unwrap_or_default();
    out.sort_by_key(|(t, _)| t.0);
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn badges_take_the_first_letters() {
        assert_eq!(super::initials("FS sizes"), "Fs");
        assert_eq!(super::initials("FS two tables"), "Ft");
        assert_eq!(super::initials("Masses"), "Ma");
    }
}
