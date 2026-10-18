//! Onshape's "Insert Variable Studio" dialog: pick a whole Variable Studio
//! table or single variables to insert into a Variable Studio or into a Part
//! Studio / Assembly Variable table. Clicking a row inserts it right away
//! (Onshape: "your selection is made available immediately"); inserted
//! entries show a check.

use crate::doc_model::DocModel;
use crate::os_ui as os;
use crate::theme::p;
use gpui_kit::component::WindowExt as _;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::TabContent;
use noshape_model::document::TabId;
use noshape_model::variables::{self as vars, StudioRef};
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

/// Where the chosen tables go.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum InsertTarget {
    /// Another Variable Studio (its "Insert Variable Studio" button).
    Studio(TabId),
    /// A Part Studio's or Assembly's Variable table.
    Element(TabId),
}

/// Apply `r` (or remove the reference when None) to the target.
pub fn set_ref(doc: &Entity<DocModel>, target: InsertTarget, studio: TabId, r: Option<StudioRef>, cx: &mut App) {
    let (tab, desc) = match target {
        InsertTarget::Studio(t) => (t, if r.is_some() { "Insert Variable Studio" } else { "Remove reference" }),
        InsertTarget::Element(e) => (e, if r.is_some() { "Insert Variable Studio" } else { "Remove reference" }),
    };
    doc.update(cx, |d, cx| {
        d.edit_tab(tab, desc, cx, |c| match (c, &r) {
            (TabContent::VariableStudio(v), Some(r)) => vars::insert_into_studio(v, r.clone()),
            (TabContent::VariableStudio(v), None) => v.inserted.retain(|x| x.studio != studio),
            (TabContent::PartStudio(ps), Some(r)) => vars::insert_studio(&mut ps.variable_studios, r.clone()),
            (TabContent::PartStudio(ps), None) => vars::remove_studio(&mut ps.variable_studios, studio),
            (TabContent::Assembly(a), Some(r)) => vars::insert_studio(&mut a.variable_studios, r.clone()),
            (TabContent::Assembly(a), None) => vars::remove_studio(&mut a.variable_studios, studio),
            _ => {}
        });
    });
}

/// Move `studio`'s table in `element`'s panel up or down (its explicit
/// references are reordered; auto-inserted tables become explicit).
pub fn move_table(doc: &Entity<DocModel>, element: TabId, studio: TabId, down: bool, cx: &mut App) {
    let list = vars::element_studios(&doc.read(cx).session.doc, element);
    let Some(k) = list.iter().position(|r| r.studio == studio) else { return };
    let j = if down { k + 1 } else { k.wrapping_sub(1) };
    if j >= list.len() {
        return;
    }
    let mut order = list.clone();
    order.swap(k, j);
    doc.update(cx, |d, cx| {
        d.edit_tab(element, if down { "Move table down" } else { "Move table up" }, cx, |c| {
            let refs = match c {
                TabContent::PartStudio(ps) => &mut ps.variable_studios,
                TabContent::Assembly(a) => &mut a.variable_studios,
                _ => return,
            };
            // Pin the whole order explicitly so it survives.
            refs.inserted = order.clone();
            refs.removed.retain(|t| !order.iter().any(|r| r.studio == *t));
        });
    });
}

/// Dialog state kept across re-renders.
#[derive(Default)]
struct DialogState {
    expanded: HashSet<(Option<std::path::PathBuf>, TabId)>,
    other: bool,
    /// Other documents: the chosen document and version (microversion).
    document: Option<std::path::PathBuf>,
    version: Option<String>,
    error: Option<String>,
}

/// .noshape documents besides `current`: recently opened, the documents
/// folder and the watched folders.
fn other_documents(current: &std::path::Path) -> Vec<std::path::PathBuf> {
    let canon = |p: &std::path::Path| p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
    let me = canon(current);
    let mut out: Vec<std::path::PathBuf> = Vec::new();
    let mut push = |p: std::path::PathBuf| {
        let c = canon(&p);
        if c != me && c.extension().is_some_and(|e| e == "noshape") && c.exists() && !out.contains(&c) {
            out.push(c);
        }
    };
    for p in crate::files::recent() {
        push(p);
    }
    for dir in std::iter::once(crate::files::documents_dir()).chain(crate::files::watched_folders()) {
        if let Ok(rd) = std::fs::read_dir(dir) {
            for e in rd.flatten() {
                push(e.path());
            }
        }
    }
    out
}

/// Whether `r` points at `studio` (of `external`'s document, or this one).
fn same_source(r: &StudioRef, studio: TabId, external: &Option<vars::ExternalStudio>) -> bool {
    r.studio == studio && r.external.as_ref().map(|e| &e.path) == external.as_ref().map(|e| &e.path)
}

/// The current reference to a studio in the target, if any.
fn current_ref(doc: &noshape_model::Document, target: InsertTarget, studio: TabId, external: &Option<vars::ExternalStudio>) -> Option<StudioRef> {
    match target {
        InsertTarget::Studio(t) => match &doc.tab(t)?.content {
            TabContent::VariableStudio(v) => v.inserted.iter().find(|r| same_source(r, studio, external)).cloned(),
            _ => None,
        },
        InsertTarget::Element(e) => vars::element_studios(doc, e).into_iter().find(|r| same_source(r, studio, external)),
    }
}

/// Rows for the studios of `src` (this document, or another document's
/// version when `external` is set): a studio row inserts or removes the
/// whole table; its caret lists the variables to insert singly.
#[derive(Clone, Copy)]
struct Rows<'a> {
    doc: &'a Entity<DocModel>,
    target: InsertTarget,
    state: &'a Rc<RefCell<DialogState>>,
    /// This document (where the target is).
    current: &'a noshape_model::Document,
}

fn studio_rows(ctx: &Rows, src: &noshape_model::Document, external: Option<vars::ExternalStudio>, cx: &App, rows: &mut Vec<AnyElement>) {
    let Rows { doc, target, state, current } = *ctx;
    let pal = p();
    let own = match target {
        InsertTarget::Studio(t) if external.is_none() => Some(t),
        _ => None,
    };
    let studios: Vec<(TabId, String, Vec<String>)> = src
        .ordered_tabs()
        .into_iter()
        .filter(|t| Some(t.id) != own)
        .filter_map(|t| match &t.content {
            TabContent::VariableStudio(v) => Some((t.id, t.name.clone(), v.variables.iter().map(|x| x.name.clone()).filter(|n| !n.is_empty()).collect())),
            _ => None,
        })
        .collect();
    if studios.is_empty() {
        let msg = if external.is_some() { "This version has no Variable Studios." } else { "There are no other Variable Studios in this document." };
        rows.push(os::info_box(msg).m(px(10.)).into_any_element());
        return;
    }
    let path = external.as_ref().map(|e| e.path.clone());
    for (id, name, names) in studios {
        let cur = current_ref(current, target, id, &external);
        let whole = cur.as_ref().is_some_and(|r| r.variables.is_none());
        let key = (path.clone(), id);
        let open = state.borrow().expanded.contains(&key);
        let (doc1, st, ext1) = (doc.clone(), state.clone(), external.clone());
        rows.push(
            h_flex()
                .id(SharedString::from(format!("ivs-{}-{id}", path.as_ref().map(|p| p.display().to_string()).unwrap_or_default())))
                .h(px(32.))
                .px(px(6.))
                .gap(px(4.))
                .text_size(px(14.))
                .cursor_pointer()
                .when(whole, |d| d.bg(pal.accent_soft))
                .hover(|d| d.bg(rgb(0xdef1fb)))
                .child(
                    div()
                        .id(SharedString::from(format!("ivs-caret-{id}")))
                        .size(px(16.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(crate::ui::glyph(if open { "ui-chevron-down" } else { "ui-chevron-right" }, 10., pal.text))
                        .on_click(move |_, window, cx| {
                            cx.stop_propagation();
                            let mut s = st.borrow_mut();
                            if !s.expanded.remove(&key) {
                                s.expanded.insert(key.clone());
                            }
                            window.refresh();
                        }),
                )
                .child(crate::assets::icon("variable-studio", 20., cx))
                .child(div().flex_1().truncate().child(name))
                .when(whole, |d| d.child(crate::ui::glyph("check", 14., pal.accent)))
                .on_click(move |_, window, cx| {
                    set_ref(&doc1, target, id, if whole { None } else { Some(StudioRef { studio: id, variables: None, external: ext1.clone() }) }, cx);
                    window.refresh();
                })
                .into_any_element(),
        );
        if open {
            for n in &names {
                let picked = cur.as_ref().is_some_and(|r| r.variables.as_ref().is_none_or(|v| v.contains(n)));
                let (doc2, cur2, n2, all, ext2) = (doc.clone(), cur.clone(), n.clone(), names.clone(), external.clone());
                rows.push(
                    h_flex()
                        .id(SharedString::from(format!("ivs-{id}-{n}")))
                        .h(px(28.))
                        .pl(px(48.))
                        .pr(px(6.))
                        .text_size(px(13.))
                        .cursor_pointer()
                        .when(picked, |d| d.bg(pal.accent_soft))
                        .hover(|d| d.bg(rgb(0xdef1fb)))
                        .child(div().flex_1().truncate().child(format!("#{n}")))
                        .when(picked, |d| d.child(crate::ui::glyph("check", 14., pal.accent)))
                        .on_click(move |_, window, cx| {
                            // Toggle one variable; a whole-table reference
                            // becomes the list of the others.
                            let list = match &cur2 {
                                Some(StudioRef { variables: None, .. }) => all.iter().filter(|x| **x != n2).cloned().collect(),
                                Some(StudioRef { variables: Some(v), .. }) => {
                                    let mut v = v.clone();
                                    match v.iter().position(|x| *x == n2) {
                                        Some(i) => {
                                            v.remove(i);
                                        }
                                        None => v.push(n2.clone()),
                                    }
                                    v
                                }
                                None => vec![n2.clone()],
                            };
                            let next = (!list.is_empty()).then(|| StudioRef { studio: id, variables: Some(list), external: ext2.clone() });
                            set_ref(&doc2, target, id, next, cx);
                            window.refresh();
                        })
                        .into_any_element(),
                );
            }
        }
    }
}

/// Open the dialog for `target`.
pub fn open(doc: Entity<DocModel>, target: InsertTarget, window: &mut Window, cx: &mut App) {
    open_with(doc, target, DialogState::default(), window, cx);
}

fn open_with(doc: Entity<DocModel>, target: InsertTarget, state: DialogState, window: &mut Window, cx: &mut App) {
    let state: Rc<RefCell<DialogState>> = Rc::new(RefCell::new(state));
    window.open_dialog(cx, move |dialog, _, cx| {
        let pal = p();
        let (current, path) = {
            let s = &doc.read(cx).session;
            (s.doc.clone(), s.path.clone())
        };
        let other = state.borrow().other;
        let tab = |id: &'static str, label: &'static str, on: bool, st: Rc<RefCell<DialogState>>| {
            div()
                .id(id)
                .flex_1()
                .h(px(32.))
                .relative()
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(14.))
                .cursor_pointer()
                .when(on, |d| d.text_color(pal.accent).font_weight(FontWeight::MEDIUM).child(div().absolute().left_0().right_0().bottom_0().h(px(2.)).bg(pal.accent)))
                .when(!on, |d| d.text_color(pal.text).child(div().absolute().left_0().right_0().bottom_0().h(px(1.)).bg(pal.border)))
                .child(label)
                .on_click(move |_, window, _| {
                    st.borrow_mut().other = id == "ivs-other";
                    window.refresh();
                })
        };
        let mut rows: Vec<AnyElement> = Vec::new();
        if !other {
            studio_rows(&Rows { doc: &doc, target, state: &state, current: &current }, &current, None, cx, &mut rows);
        } else {
            let chosen = state.borrow().document.clone();
            match chosen {
                None => {
                    let docs = other_documents(&path);
                    if docs.is_empty() {
                        rows.push(os::info_box("No other documents.").m(px(10.)).into_any_element());
                    }
                    for (k, d) in docs.into_iter().enumerate() {
                        let name = crate::derived::external_doc_info(&d).map(|i| i.name).unwrap_or_else(|_| d.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default());
                        let st = state.clone();
                        rows.push(
                            h_flex()
                                .id(("ivs-doc", k))
                                .h(px(32.))
                                .px(px(10.))
                                .gap(px(6.))
                                .text_size(px(14.))
                                .cursor_pointer()
                                .hover(|d| d.bg(rgb(0xdef1fb)))
                                .child(os::glyph(os::GLYPH_DOCUMENT, 18., pal.text_muted))
                                .child(div().flex_1().truncate().child(name))
                                .on_click(move |_, window, _| {
                                    let mut s = st.borrow_mut();
                                    s.document = Some(d.clone());
                                    s.version = None;
                                    s.error = None;
                                    window.refresh();
                                })
                                .into_any_element(),
                        );
                    }
                }
                Some(d) => {
                    let info = crate::derived::external_doc_info(&d).ok();
                    let name = info.as_ref().map(|i| i.name.clone()).unwrap_or_default();
                    // Onshape inserts from a version; "Current state" is not offered.
                    let versions: Vec<crate::derived::VersionChoice> = info.map(|i| i.versions.into_iter().filter(|v| v.label != "Current state").collect()).unwrap_or_default();
                    let st = state.clone();
                    rows.push(
                        h_flex()
                            .id("ivs-back")
                            .h(px(32.))
                            .px(px(6.))
                            .gap(px(4.))
                            .text_size(px(14.))
                            .font_weight(FontWeight::MEDIUM)
                            .cursor_pointer()
                            .child(os::glyph(os::GLYPH_BACK, 16., pal.text))
                            .child(name)
                            .on_click(move |_, window, _| {
                                st.borrow_mut().document = None;
                                window.refresh();
                            })
                            .into_any_element(),
                    );
                    if versions.is_empty() {
                        // Onshape: "If no version exists, you are prompted to create one."
                        let (st, d2) = (state.clone(), d.clone());
                        rows.push(os::info_box("This document has no versions. Create a version to insert its Variable Studios.").m(px(10.)).into_any_element());
                        rows.push(
                            div()
                                .px(px(10.))
                                .child(os::primary_button("ivs-create-version", "Create version").on_click(move |_, window, _| {
                                    let r = (|| -> anyhow::Result<()> {
                                        let mut store = noshape_store::Document::open(&d2)?;
                                        let head = store.head(store.default_branch()?)?;
                                        store.create_version(head, "V1", "")?;
                                        Ok(())
                                    })();
                                    st.borrow_mut().error = r.err().map(|e| format!("{e:#}"));
                                    window.refresh();
                                }))
                                .into_any_element(),
                        );
                        if let Some(e) = state.borrow().error.clone() {
                            rows.push(div().p(px(10.)).text_size(px(12.)).text_color(pal.error).child(e).into_any_element());
                        }
                    } else {
                        let chosen_mv = state.borrow().version.clone().unwrap_or_else(|| versions[0].microversion.to_string());
                        let label = versions.iter().find(|v| v.microversion.to_string() == chosen_mv).map(|v| v.label.clone()).unwrap_or_default();
                        let mut vrow = h_flex().px(px(10.)).py(px(4.)).gap(px(4.)).flex_wrap().text_size(px(13.)).child(div().text_color(pal.text_muted).child("Version:"));
                        for v in &versions {
                            let (st, mv) = (state.clone(), v.microversion.to_string());
                            let on = mv == chosen_mv;
                            vrow = vrow.child(
                                div()
                                    .id(SharedString::from(format!("ivs-v-{mv}")))
                                    .px(px(6.))
                                    .rounded(px(3.))
                                    .cursor_pointer()
                                    .when(on, |d| d.bg(pal.accent_soft).font_weight(FontWeight::MEDIUM))
                                    .hover(|d| d.bg(rgb(0xdef1fb)))
                                    .child(v.label.clone())
                                    .on_click(move |_, window, _| {
                                        st.borrow_mut().version = Some(mv.clone());
                                        window.refresh();
                                    }),
                            );
                        }
                        rows.push(vrow.into_any_element());
                        let ext = vars::ExternalStudio { path: d.clone(), microversion: chosen_mv.clone(), version: label };
                        match vars::external_document(&ext) {
                            Some(src) => studio_rows(&Rows { doc: &doc, target, state: &state, current: &current }, &src, Some(ext), cx, &mut rows),
                            None => rows.push(os::info_box("This version could not be read.").m(px(10.)).into_any_element()),
                        }
                    }
                }
            }
        }
        let st = state.clone();
        os::modal(dialog, "Insert Variable Studio", 460.)
            .child(
                v_flex()
                    .child(h_flex().px(px(10.)).child(tab("ivs-current", "Current document", !other, st.clone())).child(tab("ivs-other", "Other documents", other, st)))
                    .child(v_flex().id("ivs-rows").max_h(px(420.)).min_h(px(160.)).overflow_y_scroll().py(px(6.)).children(rows)),
            )
            .footer(os::modal_footer().child(os::modal_cancel(os::cancel_button("close", "Close", cx))))
    });
}

/// "+" Create Variable Studio from a Part Studio's / Assembly's Variable
/// table: a new studio tab right after `element` (checked "Insert into
/// all…", so its table shows here at once, ready for input).
pub fn create_studio(doc: &Entity<DocModel>, element: TabId, cx: &mut App) {
    doc.update(cx, |d, cx| {
        d.edit_document("Insert tab : Variable Studio", cx, |doc| {
            let name = doc.next_tab_name("Variable Studio");
            let id = doc.add_tab(&name, TabContent::VariableStudio(vars::VariableStudio::default()));
            let order = &mut doc.settings.tab_order;
            order.retain(|t| *t != id);
            let at = order.iter().position(|t| *t == element).map(|i| i + 1).unwrap_or(order.len());
            order.insert(at, id);
        });
    });
}

/// The Variable table footer of a Part Studio / Assembly (live,
/// part-studio/77): a split button "Create Variable Studio | ▴"; the menu
/// has Create Variable Studio and Insert Variable Studio.
pub fn panel_footer(doc: Entity<DocModel>, element: TabId, cx: &App) -> AnyElement {
    use gpui_kit::component::button::{Button, ButtonCustomVariant, ButtonVariants as _};
    use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
    let pal = p();
    let variant = ButtonCustomVariant::new(cx).color(rgb(0xffffff).into()).foreground(pal.text.into()).hover(pal.panel_hover.into()).active(pal.panel_active.into()).shadow(false);
    let d1 = doc.clone();
    h_flex()
        .child(
            Button::new("vt-create")
                .custom(variant)
                .h(px(35.))
                .px(px(10.5))
                .rounded_l(px(5.25))
                .rounded_r(px(0.))
                .border_1()
                .border_color(rgb(0xaaaaaa))
                .child(h_flex().gap(px(3.)).child(crate::assets::icon("variable-studio", 20., cx)).child(div().text_size(px(14.)).child("Create Variable Studio")))
                .on_click(move |_, _, cx| create_studio(&d1, element, cx)),
        )
        .child(
            Button::new("vt-more")
                .custom(variant)
                .h(px(35.))
                .w(px(26.))
                .rounded_r(px(5.25))
                .rounded_l(px(0.))
                .border_1()
                .border_l_0()
                .border_color(rgb(0xaaaaaa))
                .child(os::glyph(os::GLYPH_CARET_UP, 7., pal.text))
                .dropdown_menu_with_anchor(Anchor::BottomRight, move |m, _, _| {
                    let (d2, d3) = (doc.clone(), doc.clone());
                    m.item(PopupMenuItem::new("Create Variable Studio").on_click(move |_, _, cx| create_studio(&d2, element, cx)))
                        .item(PopupMenuItem::new("Insert Variable Studio").on_click(move |_, window, cx| open(d3.clone(), InsertTarget::Element(element), window, cx)))
                }),
        )
        .into_any_element()
}

/// A read-only Variable Studio table in `target` (a studio inserted into
/// another studio, or one from another document in a Part Studio /
/// Assembly panel): its title ("Variable Studio 1", plus the version for
/// another document's), Name | Variable type | Value | Description rows,
/// and the reference's "…" menu.
pub fn readonly_table(doc: &Entity<DocModel>, target: InsertTarget, r: &StudioRef, k: usize, n: usize, cx: &App) -> Option<AnyElement> {
    use gpui_kit::component::button::{Button, ButtonCustomVariant, ButtonVariants as _};
    use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
    const COL_W: f32 = 140.;
    const RULE: u32 = 0xaaaaaa;
    const HEAD_BG: u32 = 0xeaeaea;
    let pal = p();
    let here = doc.read(cx).session.doc.clone();
    let src = match &r.external {
        Some(e) => vars::external_document(e)?,
        None => std::sync::Arc::new(here.clone()),
    };
    let tab = src.tab(r.studio)?;
    let TabContent::VariableStudio(studio) = &tab.content else { return None };
    let title = match &r.external {
        Some(e) => format!("{} {}", tab.name, e.version),
        None => tab.name.clone(),
    };
    let values = vars::studio_variables(&src, r.studio);
    let units = here.settings.units;
    let cell = || div().w(px(COL_W)).h(px(34.)).flex_none().flex().items_center().px(px(7.)).border_r_1().border_b_1().border_color(rgb(RULE)).truncate();
    let rows: Vec<AnyElement> = studio
        .variables
        .iter()
        .filter(|v| r.variables.as_ref().is_none_or(|l| l.contains(&v.name)))
        .map(|v| {
            let value = values.get(&v.name).map(|q| noshape_units::format_quantity(*q, &units)).unwrap_or_default();
            let kind = match v.kind {
                Some(vars::VarKind::Length) => "Length",
                Some(vars::VarKind::Angle) => "Angle",
                Some(vars::VarKind::Number) => "Number",
                _ => "Any",
            };
            h_flex()
                .border_l_1()
                .border_color(rgb(RULE))
                .bg(rgb(0xffffff))
                .text_size(px(14.))
                .text_color(pal.text_muted)
                .child(cell().child(v.name.clone()))
                .child(cell().child(kind))
                .child(cell().child(value))
                .child(cell().child(v.description.clone()))
                .into_any_element()
        })
        .collect();
    let plain = ButtonCustomVariant::new(cx).color(gpui_kit::transparent_white()).foreground(pal.text.into()).hover(gpui_kit::transparent_white()).active(gpui_kit::transparent_white()).shadow(false);
    let (d, studio_id, name, ext) = (doc.clone(), r.studio, tab.name.clone(), r.external.clone());
    let more = Button::new(SharedString::from(format!("ro-more-{studio_id}")))
        .custom(plain)
        .w(px(25.))
        .h(px(22.))
        .p_0()
        .child(div().text_size(px(13.)).font_weight(FontWeight::BOLD).text_color(pal.info_text).child("•••"))
        .dropdown_menu_with_anchor(Anchor::TopRight, move |m, _, _| {
            let (d1, d2, d3, d4) = (d.clone(), d.clone(), d.clone(), d.clone());
            let m = match &ext {
                // Onshape: Open linked document / Update linked document.
                Some(e) => {
                    let (p1, e2) = (e.path.clone(), e.clone());
                    m.item(PopupMenuItem::new("Open linked document").on_click(move |_, _, cx| {
                        let gpu = cx.global::<crate::gpu::GpuGlobal>().0.clone();
                        let _ = crate::files::open_document_window(p1.clone(), None, gpu, cx);
                    }))
                        .item(PopupMenuItem::new("Update linked document").on_click(move |_, window, cx| open_at_document(d4.clone(), target, e2.path.clone(), window, cx)))
                }
                None => m.item(PopupMenuItem::new(format!("Switch to {name}")).on_click(move |_, window, cx| crate::workspace::request_activate_tab(studio_id, window, cx))),
            };
            let m = if k > 0 { m.item(PopupMenuItem::new("Move up").on_click(move |_, _, cx| move_ref(&d1, target, studio_id, false, cx))) } else { m };
            let m = if k + 1 < n { m.item(PopupMenuItem::new("Move down").on_click(move |_, _, cx| move_ref(&d2, target, studio_id, true, cx))) } else { m };
            m.item(PopupMenuItem::new("Remove reference").on_click(move |_, _, cx| set_ref(&d3, target, studio_id, None, cx)))
        });
    Some(
        v_flex()
            .w_full()
            .child(
                h_flex()
                    .h(px(35.))
                    .px(px(6.))
                    .gap(px(3.))
                    .bg(rgb(HEAD_BG))
                    .border_1()
                    .border_color(rgb(RULE))
                    .child(crate::ui::glyph("ui-chevron-down", 11., pal.text))
                    .child(div().text_size(px(16.)).font_weight(FontWeight::MEDIUM).text_color(pal.text).child(title))
                    .child(div().flex_1())
                    .child(more),
            )
            .child(
                h_flex()
                    .border_l_1()
                    .border_color(rgb(RULE))
                    .bg(rgb(HEAD_BG))
                    .text_size(px(14.))
                    .font_weight(FontWeight::BOLD)
                    .text_color(pal.text)
                    .children(["Name", "Variable type", "Value", "Description"].map(|h| cell().justify_center().child(h))),
            )
            .children(rows)
            .into_any_element(),
    )
}

/// Move a reference up or down in its target.
pub fn move_ref(doc: &Entity<DocModel>, target: InsertTarget, studio: TabId, down: bool, cx: &mut App) {
    match target {
        InsertTarget::Element(e) => move_table(doc, e, studio, down, cx),
        InsertTarget::Studio(t) => doc.update(cx, |d, cx| {
            d.edit_tab(t, if down { "Move table down" } else { "Move table up" }, cx, |c| {
                if let TabContent::VariableStudio(v) = c
                    && let Some(k) = v.inserted.iter().position(|r| r.studio == studio)
                {
                    let j = if down { k + 1 } else { k.wrapping_sub(1) };
                    if j < v.inserted.len() {
                        v.inserted.swap(k, j);
                    }
                }
            });
        }),
    }
}

/// The dialog opened on another document (its "Update linked document").
fn open_at_document(doc: Entity<DocModel>, target: InsertTarget, path: std::path::PathBuf, window: &mut Window, cx: &mut App) {
    open_with(doc, target, DialogState { other: true, document: Some(path), ..Default::default() }, window, cx);
}
