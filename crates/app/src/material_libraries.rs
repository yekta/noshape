//! Material libraries added to the account (Onshape keeps them in account
//! preferences; here a preferences file): the Material panel's library
//! select lists the Onshape Material Library, these, and the open
//! document's Material Library tabs. The "Add material library" modal
//! (live dom/dialog-add-material-library.txt): an 800 px modal with a left
//! filter list (Onshape, Owned by me, Recently opened, Created by me, Shared
//! with me, Public) and a Library | Document | Owner table; a row selects,
//! [Add] (disabled until then) adds it; the footer links to the account
//! preferences.

use crate::os_ui as os;
use crate::theme::p;
use gpui_kit::component::WindowExt as _;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::document::TabContent;
use noshape_model::material_library::MaterialLibrary;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A Material Library tab of a local document, added to the account.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AddedLibrary {
    pub path: PathBuf,
    pub tab: u64,
    pub name: String,
    pub document: String,
    pub owner: String,
}

fn prefs_path() -> PathBuf {
    crate::files::config_dir().join("material_libraries.json")
}

/// The libraries added to the account, in the order they were added.
pub fn added() -> Vec<AddedLibrary> {
    std::fs::read(prefs_path()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

pub fn set_added(list: &[AddedLibrary]) {
    let _ = std::fs::write(prefs_path(), serde_json::to_vec_pretty(list).unwrap_or_default());
}

fn onshape_removed_path() -> PathBuf {
    crate::files::config_dir().join("material_library_onshape_removed")
}

/// Whether the Onshape Material Library is listed (Preferences › Material
/// libraries can remove it too, live prefs/05; Add material library… →
/// Onshape brings it back).
pub fn onshape_listed() -> bool {
    !onshape_removed_path().exists()
}

pub fn set_onshape_listed(on: bool) {
    let p = onshape_removed_path();
    if on {
        let _ = std::fs::remove_file(p);
    } else {
        let _ = std::fs::write(p, b"");
    }
}

/// The materials of an added library (its document's current state).
pub fn materials_of(lib: &AddedLibrary) -> MaterialLibrary {
    (|| -> anyhow::Result<MaterialLibrary> {
        let store = noshape_store::Document::open_read_only(&lib.path)?;
        let head = store.head(store.default_branch()?)?;
        let doc = noshape_model::Document::from_blobs(&store.read_all(head)?)?;
        Ok(doc.tabs.iter().find(|t| t.id.0 == lib.tab).and_then(|t| match &t.content {
            TabContent::MaterialLibrary(m) => Some(m.clone()),
            _ => None,
        }).unwrap_or_default())
    })()
    .unwrap_or_default()
}

/// Filters of the left list, in Onshape's order.
const FILTERS: [&str; 6] = ["Onshape", "Owned by me", "Recently opened", "Created by me", "Shared with me", "Public"];

/// The Onshape filter's mark, Shared with me and Public (the others are the
/// documents page's glyphs).
const GLYPH_ONSHAPE: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round"><path d="M10 1.8 17.1 5.9v8.2L10 18.2 2.9 14.1V5.9z"/><circle cx="10" cy="10" r="3.2"/></svg>"#;
const GLYPH_SHARED: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor"><circle cx="7.5" cy="6.5" r="3"/><path d="M1.8 16.5c.6-3.2 2.9-5 5.7-5s5.1 1.8 5.7 5z"/><path d="M13.2 4.2h4.8v4.8h-1.5V6.8l-3.3 3.3-1.1-1.1 3.3-3.3h-2.2z"/></svg>"#;
const GLYPH_PUBLIC: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.4"><circle cx="10" cy="10" r="8"/><path d="M2 10h16M10 2c2.4 2.3 3.6 5 3.6 8s-1.2 5.7-3.6 8c-2.4-2.3-3.6-5-3.6-8S7.6 4.3 10 2z"/></svg>"#;

/// The Material Library tabs of the local documents (name, owner, recent).
fn local_libraries() -> Vec<(AddedLibrary, bool)> {
    let input = crate::start_library::ScanInput::current();
    let scan = crate::start_library::Library::default().scan(&input);
    let mut out = Vec::new();
    for d in scan.docs.iter().filter(|d| !d.broken && !d.trashed) {
        let Ok(store) = noshape_store::Document::open_read_only(&d.path) else { continue };
        let Ok(head) = store.default_branch().and_then(|b| store.head(b)) else { continue };
        let Ok(blobs) = store.read_all(head) else { continue };
        let Ok(doc) = noshape_model::Document::from_blobs(&blobs) else { continue };
        for t in doc.tabs.iter().filter(|t| matches!(t.content, TabContent::MaterialLibrary(_))) {
            out.push((AddedLibrary { path: d.path.clone(), tab: t.id.0, name: t.name.clone(), document: d.name.clone(), owner: d.owner.clone() }, d.recent_rank.is_some()));
        }
    }
    out
}

pub struct AddLibraryDialog {
    filter: usize,
    /// Rows of the local filters (loaded once).
    local: Vec<(AddedLibrary, bool)>,
    me: String,
    selected: Option<usize>,
    on_add: OnAdd,
}

/// What [Add] does with the picked library (None = the Onshape library).
type OnAdd = std::rc::Rc<dyn Fn(Option<AddedLibrary>, &mut Window, &mut App)>;

/// Open the modal; `on_add` gets the picked library (None = the Onshape
/// Material Library).
pub fn open(on_add: impl Fn(Option<AddedLibrary>, &mut Window, &mut App) + 'static, window: &mut Window, cx: &mut App) {
    let me = crate::files::user_name_pref().or_else(|| std::env::var("USER").ok()).unwrap_or_default();
    let view = cx.new(|_| AddLibraryDialog { filter: 0, local: local_libraries(), me, selected: None, on_add: std::rc::Rc::new(on_add) });
    window.open_dialog(cx, move |dialog, _, _| os::modal(dialog, "Add material library", 800.).child(view.clone()));
}

impl AddLibraryDialog {
    /// Rows of the current filter: None = the Onshape Material Library.
    fn rows(&self) -> Vec<Option<AddedLibrary>> {
        let mine = |l: &AddedLibrary| l.owner == self.me || l.owner.is_empty();
        match self.filter {
            0 => vec![None],
            1 | 3 => self.local.iter().filter(|(l, _)| mine(l)).map(|(l, _)| Some(l.clone())).collect(),
            2 => self.local.iter().filter(|(_, recent)| *recent).map(|(l, _)| Some(l.clone())).collect(),
            4 => self.local.iter().filter(|(l, _)| !mine(l)).map(|(l, _)| Some(l.clone())).collect(),
            // Public: the Onshape library is the public std.mat document.
            _ => vec![None],
        }
    }
}

impl Render for AddLibraryDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = p();
        let nav = FILTERS.iter().enumerate().map(|(i, label)| {
            let on = self.filter == i;
            h_flex()
                .id(SharedString::from(format!("matlib-nav-{i}")))
                .h(px(32.))
                .px(px(6.))
                .gap(px(6.))
                .cursor_pointer()
                .child(os::glyph([GLYPH_ONSHAPE, os::GLYPH_OWNED, os::GLYPH_RECENT, os::GLYPH_CREATED, GLYPH_SHARED, GLYPH_PUBLIC][i], 20., if on { pal.accent } else { pal.text }))
                .child(div().text_size(px(14.67)).when(on, |d| d.font_weight(FontWeight::SEMIBOLD).text_color(pal.accent)).when(!on, |d| d.text_color(pal.text)).child(*label))
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.filter = i;
                    this.selected = None;
                    cx.notify();
                }))
        });
        let rows = self.rows();
        let cell = |t: String| div().w(px(181.)).pl(px(10.)).truncate().child(t);
        let table_rows = rows.iter().enumerate().map(|(i, r)| {
            let (lib, doc, owner) = match r {
                None => ("Onshape Material Library".to_string(), "std.mat".to_string(), "Onshape".to_string()),
                Some(l) => (l.name.clone(), l.document.clone(), l.owner.clone()),
            };
            let on = self.selected == Some(i);
            h_flex()
                .id(SharedString::from(format!("matlib-row-{i}")))
                .h(px(30.))
                .text_size(px(14.))
                .text_color(pal.text)
                .cursor_pointer()
                .when(on, |d| d.bg(gpui_kit::rgb(0xb5dcf5)))
                .when(!on, |d| d.hover(|d| d.bg(pal.row_hover)))
                .child(cell(lib))
                .child(cell(doc))
                .child(cell(owner))
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.selected = Some(i);
                    cx.notify();
                }))
        });
        let can_add = self.selected.is_some();
        v_flex()
            .w_full()
            .child(
                h_flex()
                    .h(px(271.))
                    .items_start()
                    .child(v_flex().w(px(195.)).h_full().pt(px(5.)).border_r_1().border_color(gpui_kit::rgb(0xd0dbe5)).children(nav))
                    .child(
                        v_flex()
                            .flex_1()
                            .pl(px(20.))
                            .pr(px(10.))
                            .pt(px(9.))
                            .child(h_flex().h(px(30.)).text_size(px(14.)).font_weight(FontWeight::BOLD).text_color(pal.text).child(cell("Library".into())).child(cell("Document".into())).child(cell("Owner".into())))
                            .child(div().id("matlib-rows").max_h(px(187.)).overflow_y_scroll().children(table_rows)),
                    ),
            )
            .child(
                h_flex()
                    .h(px(58.))
                    .px(px(13.))
                    .gap(px(8.))
                    .border_t_1()
                    .border_color(gpui_kit::rgb(0xdee2e6))
                    .child(
                        div()
                            .id("matlib-manage")
                            .flex_1()
                            .text_size(px(14.))
                            .text_color(gpui_kit::rgb(0x094174))
                            .cursor_pointer()
                            .hover(|d| d.underline())
                            .child("Manage my material libraries in account preferences")
                            .on_click(|_, window, cx| {
                                window.close_dialog(cx);
                                crate::preferences_window::show(crate::preferences_window::Page::Preferences, cx);
                            }),
                    )
                    .child(div().when(!can_add, |d| d.opacity(0.4)).child(os::primary_button("matlib-add", "Add").on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        let Some(i) = this.selected else { return };
                        let Some(r) = this.rows().get(i).cloned() else { return };
                        match &r {
                            Some(l) => {
                                let mut list = added();
                                if !list.contains(l) {
                                    list.push(l.clone());
                                    set_added(&list);
                                }
                            }
                            None => set_onshape_listed(true),
                        }
                        let f = this.on_add.clone();
                        window.close_dialog(cx);
                        f(r, window, cx);
                    }))))
                    .child(os::secondary_button("matlib-cancel", "Cancel", cx).on_click(|_, window, cx| window.close_dialog(cx))),
            )
    }
}
