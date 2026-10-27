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

// The added libraries (a preferences file), shared with the Part Studio's Material panel (noshape-app-core).
pub use noshape_app_core::material_libraries::{AddedLibrary, added, onshape_listed, set_added, set_onshape_listed};

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
const GLYPH_ONSHAPE: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linejoin="round"><path d="M10 1.9 17.3 6v8L10 18.1 2.7 14V6zM10 10.1v8"/><path d="M10 1.9 17.3 6 10 10.1 2.7 6zm0 2.8c-1.4 0-2.5.6-2.5 1.3S8.6 7.3 10 7.3s2.5-.6 2.5-1.3-1.1-1.3-2.5-1.3z" fill="currentColor" fill-rule="evenodd"/></svg>"#;
const GLYPH_SHARED: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor"><path d="M1.7 1.7h12l4.6 4.5v12.1H1.7z" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linejoin="round"/><circle cx="11.5" cy="6.9" r="2.5"/><path d="M7.6 13.7c0-2.3 1.8-3.8 3.9-3.8s3.9 1.5 3.9 3.8v.9c0 .8-.6 1.4-1.4 1.4H9c-.8 0-1.4-.6-1.4-1.4z"/></svg>"#;
const GLYPH_PUBLIC: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor"><circle cx="10" cy="10" r="8.25" fill="none" stroke="currentColor" stroke-width="1.5"/><path d="M4.4 4 7 2.4l3-.4.4 2.4-.8 1.8-1.8.9-1.4.2-.3 1.2 1.2.6 1.4 1.6 1.3 1.6-1.1.2-1.6-1-1.8-1.3-1.6-1.4-.7-1.8z"/><path d="m11 4.4 1.6-.4-.6 1.8-1 .4z"/><path d="m10.6 11.5 2.2-.6 1.6.4 1.1 1.4-.6 1.8-1.6 1.6-.6 1.9-1.1-.3-.4-2.6-.7-1.8z"/><path d="m16.3 5.5 1.2 1.7.4 2-1.2-.6-.6-1.5z"/></svg>"#;

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
    let me = crate::doc_files::user_name_pref().or_else(|| std::env::var("USER").ok()).unwrap_or_default();
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
