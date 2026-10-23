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
    /// Studios folded with their caret (live: they open expanded).
    collapsed: HashSet<(Option<std::path::PathBuf>, TabId)>,
    other: bool,
    /// Other documents: the chosen document and version (microversion).
    document: Option<std::path::PathBuf>,
    version: Option<String>,
    error: Option<String>,
    /// Configured studios: the configuration box open, and the values picked
    /// in it (help config-insert-vs-01).
    config_open: HashSet<(Option<std::path::PathBuf>, TabId)>,
    config_values: std::collections::HashMap<(Option<std::path::PathBuf>, TabId), noshape_model::configuration::ConfigValues>,
    /// The version graph is open; this document at a version (microversion,
    /// label) instead of Main.
    graph: bool,
    own_version: Option<(String, String)>,
}

/// .noshape documents besides `current`: recently opened, the documents
/// folder and the watched folders.
pub(crate) fn other_documents(current: &std::path::Path) -> Vec<std::path::PathBuf> {
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
    /// "Search Variable Studios" (lowercase): studios whose name or a
    /// variable matches.
    query: &'a str,
}

fn studio_rows(ctx: &Rows, src: &noshape_model::Document, external: Option<vars::ExternalStudio>, cx: &App, rows: &mut Vec<AnyElement>) {
    let Rows { doc, target, state, current, query } = *ctx;
    let pal = p();
    let own = match target {
        InsertTarget::Studio(t) if external.is_none() => Some(t),
        _ => None,
    };
    let studios: Vec<(TabId, String, Vec<String>, vars::VsConfiguration)> = src
        .ordered_tabs()
        .into_iter()
        .filter(|t| Some(t.id) != own)
        .filter_map(|t| match &t.content {
            TabContent::VariableStudio(v) => Some((t.id, t.name.clone(), v.variables.iter().map(|x| x.name.clone()).filter(|n| !n.is_empty()).collect::<Vec<_>>(), v.configuration.clone())),
            _ => None,
        })
        .filter(|(_, name, vars, _)| query.is_empty() || name.to_lowercase().contains(query) || vars.iter().any(|v| v.to_lowercase().contains(query)))
        .collect();
    if studios.is_empty() {
        let msg = if external.is_some() { "This version has no Variable Studios." } else { "There are no other Variable Studios in this document." };
        rows.push(os::info_box(msg).m(px(10.)).into_any_element());
        return;
    }
    let path = external.as_ref().map(|e| e.path.clone());
    for (id, name, names, cfg) in studios {
        let cur = current_ref(current, target, id, &external);
        let whole = cur.as_ref().is_some_and(|r| r.variables.is_none());
        let key = (path.clone(), id);
        let open = !state.borrow().collapsed.contains(&key);
        let cfg_key = key.clone();
        let (doc1, st, ext1) = (doc.clone(), state.clone(), external.clone());
        rows.push(
            h_flex()
                .id(SharedString::from(format!("ivs-{}-{id}", path.as_ref().map(|p| p.display().to_string()).unwrap_or_default())))
                // Live (dom/dialog-insert-variable-studio): 40 px rows; the
                // expander at x 4, the 50x30 studio icon at x 30, the name
                // (13.33 px, weight 300) at x 96.
                .h(px(40.))
                .flex_none()
                .pl(px(4.))
                .pr(px(4.))
                .text_size(px(13.33))
                .font_weight(FontWeight::LIGHT)
                .text_color(pal.text)
                .cursor_pointer()
                .relative()
                .group(SharedString::from(format!("ivs-row-{id}")))
                .tooltip(crate::ui::tooltip(name.clone()))
                // Live 159: the picked studio light blue from x 18 (not under
                // the expander), no check mark.
                .child(
                    div()
                        .absolute()
                        .left(px(18.))
                        .top_0()
                        .bottom_0()
                        .right_0()
                        .when(whole, |d| d.bg(pal.accent_soft))
                        .when(!whole, |d| d.group_hover(SharedString::from(format!("ivs-row-{id}")), |d| d.bg(rgb(0xdef1fb)))),
                )
                .child(
                    div()
                        .id(SharedString::from(format!("ivs-caret-{id}")))
                        .w(px(10.))
                        .h(px(22.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(crate::ui::glyph(if open { "ui-chevron-down" } else { "ui-chevron-right" }, 10., pal.text))
                        .on_click(move |_, window, cx| {
                            cx.stop_propagation();
                            let mut s = st.borrow_mut();
                            if !s.collapsed.remove(&key) {
                                s.collapsed.insert(key.clone());
                            }
                            window.refresh();
                        }),
                )
                .child(div().ml(px(16.)).w(px(50.)).h(px(30.)).flex_none().flex().items_center().justify_center().child(crate::assets::icon("variable-studio", 30., cx)))
                .child(div().ml(px(16.)).flex_1().min_w_0().truncate().child(name))
                // A configured studio: [▦ ▾] opens its configuration box.
                .when(!cfg.is_empty(), |d| {
                    let (st, key) = (state.clone(), cfg_key.clone());
                    d.child(
                        h_flex()
                            .id(SharedString::from(format!("ivs-cfg-{id}")))
                            .h(px(26.))
                            .rounded(px(3.))
                            .border_1()
                            .border_color(rgb(0xaaaaaa))
                            .bg(gpui_kit::white())
                            .child(div().id(SharedString::from(format!("ivs-cfg-open-{id}"))).px(px(5.)).h_full().flex().items_center().child(crate::assets::icon("configuration", 18., cx)).on_click({
                                let (st, key) = (st.clone(), key.clone());
                                move |_, window, cx| {
                                    cx.stop_propagation();
                                    let mut s = st.borrow_mut();
                                    if !s.config_open.remove(&key) {
                                        s.config_open.insert(key.clone());
                                    }
                                    window.refresh();
                                }
                            }))
                            // Live vs-926: Set to last viewed / Reset to default.
                            .child(div().id(SharedString::from(format!("ivs-cfg-more-{id}"))).w(px(20.)).h_full().flex().items_center().justify_center().border_l_1().border_color(rgb(0xaaaaaa)).child(crate::ui::glyph("drawing-tool-caret", 12., pal.text)).on_click({
                                let (st, key, last) = (st.clone(), key.clone(), cfg.active.clone());
                                move |e: &ClickEvent, window, cx| {
                                    cx.stop_propagation();
                                    let (st1, st2, k1, k2, last) = (st.clone(), st.clone(), key.clone(), key.clone(), last.clone());
                                    let menu = crate::os_menu::OsMenu::build(window, cx, move |m, _, _| {
                                        let (st1, st2, k1, k2, last) = (st1.clone(), st2.clone(), k1.clone(), k2.clone(), last.clone());
                                        m.item(crate::os_menu::OsMenuItem::new("Set to last viewed").on_click(move |_, window, _| {
                                            let mut s = st1.borrow_mut();
                                            s.config_values.insert(k1.clone(), last.clone());
                                            s.config_open.insert(k1.clone());
                                            window.refresh();
                                        }))
                                        .item(crate::os_menu::OsMenuItem::new("Reset to default").on_click(move |_, window, _| {
                                            let mut s = st2.borrow_mut();
                                            s.config_values.remove(&k2);
                                            s.config_open.insert(k2.clone());
                                            window.refresh();
                                        }))
                                    });
                                    crate::os_menu::open(e.position(), menu, window, cx);
                                }
                            })),
                    )
                })
                .on_click(move |_, window, cx| {
                    set_ref(&doc1, target, id, if whole { None } else { Some(StudioRef { studio: id, variables: None, external: ext1.clone(), config: Default::default() }) }, cx);
                    window.refresh();
                })
                .into_any_element(),
        );
        if state.borrow().config_open.contains(&cfg_key) {
            rows.push(config_box(ctx, id, &cfg_key, &cfg, external.clone()));
        }
        if open {
            for n in &names {
                let picked = cur.as_ref().is_some_and(|r| r.variables.as_ref().is_none_or(|v| v.contains(n)));
                let (doc2, cur2, n2, all, ext2) = (doc.clone(), cur.clone(), n.clone(), names.clone(), external.clone());
                rows.push(
                    h_flex()
                        .id(SharedString::from(format!("ivs-{id}-{n}")))
                        // Live: 40 px, the 20 px variable icon at x 80, the name at x 114.
                        .h(px(40.))
                        .flex_none()
                        .pl(px(80.))
                        .pr(px(4.))
                        .text_size(px(13.33))
                        .font_weight(FontWeight::LIGHT)
                        .text_color(pal.text)
                        .cursor_pointer()
                        .relative()
                        .group(SharedString::from(format!("ivs-var-{id}-{n}")))
                        // Live: the item (and its highlight) starts at x 40.
                        .child(
                            div()
                                .absolute()
                                .left(px(40.))
                                .top_0()
                                .bottom_0()
                                .right_0()
                                .when(picked, |d| d.bg(pal.accent_soft))
                                .when(!picked, |d| d.group_hover(SharedString::from(format!("ivs-var-{id}-{n}")), |d| d.bg(rgb(0xdef1fb)))),
                        )
                        // Live (part-studio/158, dom/dialog-insert-variable-studio):
                        // "(x) width", the 20 px variable icon 14 px before the name.
                        .child(div().flex_none().mr(px(14.)).child(crate::assets::icon("variable", 20., cx)))
                        .child(div().flex_1().truncate().child(n.clone()))
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
                            let next = (!list.is_empty()).then(|| StudioRef { studio: id, variables: Some(list), external: ext2.clone(), config: Default::default() });
                            set_ref(&doc2, target, id, next, cx);
                            window.refresh();
                        })
                        .into_any_element(),
                );
            }
        }
    }
}

/// A configured studio's inputs in the dialog (help config-insert-vs-01):
/// a select per list, a checkbox per checkbox, the value of a configuration
/// variable, then [Generate], which inserts the studio in that configuration.
fn config_box(ctx: &Rows, id: TabId, key: &(Option<std::path::PathBuf>, TabId), cfg: &vars::VsConfiguration, external: Option<vars::ExternalStudio>) -> AnyElement {
    use noshape_model::configuration::ConfigInputKind;
    let pal = p();
    let (doc, target, state) = (ctx.doc.clone(), ctx.target, ctx.state.clone());
    let values = state.borrow().config_values.get(key).cloned().unwrap_or_default();
    let mut col = v_flex().pl(px(46.)).pr(px(10.)).pb(px(8.)).gap(px(4.)).text_size(px(13.33)).text_color(pal.text);
    for input in &cfg.inputs {
        let chosen = cfg.value_of(&values, &input.id).unwrap_or_default();
        let (st, k, iid) = (state.clone(), key.clone(), input.id.clone());
        let row: AnyElement = match &input.kind {
            ConfigInputKind::List { options, .. } => {
                let label = options.iter().find(|o| o.id == chosen).map(|o| o.name.clone()).unwrap_or_default();
                let opts = options.clone();
                h_flex()
                    .id(SharedString::from(format!("ivs-in-{id}-{}", input.id)))
                    .h(px(28.))
                    .gap(px(6.))
                    .child(div().flex_none().text_color(pal.text_muted).child(input.name.clone()))
                    .child(
                        h_flex()
                            .flex_1()
                            .h(px(26.))
                            .border_b_1()
                            .border_color(rgb(0xcccccc))
                            .cursor_pointer()
                            .child(div().flex_1().text_size(px(14.)).child(label))
                            .child(crate::ui::glyph("ui-caret-down", 10., pal.text)),
                    )
                    .on_click(move |e: &ClickEvent, window, cx| {
                        let (st, k, iid, opts) = (st.clone(), k.clone(), iid.clone(), opts.clone());
                        let chosen = chosen.clone();
                        let menu = crate::os_menu::OsMenu::build(window, cx, move |mut m, _, _| {
                            m = m.select_list(200.);
                            for o in &opts {
                                let (st, k, iid, oid) = (st.clone(), k.clone(), iid.clone(), o.id.clone());
                                m = m.item(crate::os_menu::OsMenuItem::new(o.name.clone()).checked(o.id == chosen).on_click(move |_, window, _| {
                                    st.borrow_mut().config_values.entry(k.clone()).or_default().0.insert(iid.clone(), oid.clone());
                                    window.refresh();
                                }));
                            }
                            m
                        });
                        crate::os_menu::open(e.position(), menu, window, cx);
                    })
                    .into_any_element()
            }
            ConfigInputKind::Checkbox { .. } => {
                let on = chosen == "true";
                os::checkbox(SharedString::from(format!("ivs-in-{id}-{}", input.id)), input.name.clone(), on)
                    .on_click(move |_, window, _| {
                        st.borrow_mut().config_values.entry(k.clone()).or_default().0.insert(iid.clone(), (!on).to_string());
                        window.refresh();
                    })
                    .into_any_element()
            }
            _ => h_flex().h(px(28.)).child(div().flex_1().text_color(pal.text_muted).child(input.name.clone())).child(chosen).into_any_element(),
        };
        col = col.child(row);
    }
    let k = key.clone();
    // Disabled until an input changes (live vs-925).
    let changed = state.borrow().config_values.get(key).is_some_and(|v| !v.is_empty());
    col.child(
        div().pt(px(4.)).child(
            div()
                .id(SharedString::from(format!("ivs-generate-{id}")))
                .when(!changed, |d| d.opacity(0.5))
                .h(px(30.))
                .px(px(10.))
                .flex()
                .items_center()
                .rounded(px(3.))
                .border_1()
                .border_color(rgb(0xaaaaaa))
                .bg(gpui_kit::white())
                .cursor_pointer()
                .hover(|d| d.bg(pal.panel_hover))
                .text_size(px(14.))
                .child("Generate")
                .on_click(move |_, window, cx| {
                    if !changed {
                        return;
                    }
                    let config = state.borrow().config_values.get(&k).cloned().unwrap_or_default();
                    set_ref(&doc, target, id, Some(StudioRef { studio: id, variables: None, external: external.clone(), config }), cx);
                    window.refresh();
                }),
        ),
    )
    .into_any_element()
}

/// Open the dialog for `target`.
pub fn open(doc: Entity<DocModel>, target: InsertTarget, window: &mut Window, cx: &mut App) {
    open_with(doc, target, DialogState::default(), window, cx);
}

fn open_with(doc: Entity<DocModel>, target: InsertTarget, state: DialogState, window: &mut Window, cx: &mut App) {
    let state: Rc<RefCell<DialogState>> = Rc::new(RefCell::new(state));
    // ✕ (or Esc) puts the target's references back as they were.
    let before = snapshot(&doc.read(cx).session.doc, target);
    let search = cx.new(|cx| gpui_kit::component::input::InputState::new(window, cx).select_on_focus(false).placeholder("Search Variable Studios"));
    window
        .subscribe(&search, cx, |_, ev: &gpui_kit::component::input::InputEvent, window, _| {
            if let gpui_kit::component::input::InputEvent::Change = ev {
                window.refresh();
            }
        })
        .detach();
    let (doc_c, before_c) = (doc.clone(), before.clone());
    // Live (part-studio/158, document/47, dom/dialog-insert-variable-studio):
    // the same 300 px feature-style dialog at the top left, from a Part
    // Studio, an Assembly or a Variable Studio; not modal.
    window.open_dialog(cx, move |dialog, _, cx| {
        let pal = p();
        let (current, path, doc_name, branch) = {
            let d = doc.read(cx);
            let s = &d.session;
            (s.doc.clone(), s.path.clone(), d.name(), s.store.branch(s.branch).map(|b| b.name).unwrap_or_else(|_| "Main".into()))
        };
        let other = state.borrow().other;
        let graph = state.borrow().graph && !other;
        let query = search.read(cx).value().trim().to_lowercase();
        let changed = before.is_some() && snapshot(&current, target) != before;
        let tab = |id: &'static str, label: &'static str, on: bool, st: Rc<RefCell<DialogState>>| {
            div()
                .id(id)
                .w(px(150.))
                .h(px(32.))
                .relative()
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(12.))
                .cursor_pointer()
                .when(on, |d| d.text_color(rgb(0x1651b0)).font_weight(FontWeight::MEDIUM).child(div().absolute().left_0().right_0().bottom_0().h(px(2.)).bg(rgb(0x1651b0))))
                .when(!on, |d| d.text_color(pal.text))
                .child(label)
                .on_click(move |_, window, _| {
                    st.borrow_mut().other = id == "ivs-other";
                    window.refresh();
                })
        };
        let mut rows: Vec<AnyElement> = Vec::new();
        if graph {
            version_rows(&state, &path, &mut rows);
        } else if !other {
            // This document, at Main or at a version picked in the graph.
            let at = state.borrow().own_version.clone();
            match at {
                None => studio_rows(&Rows { doc: &doc, target, state: &state, current: &current, query: &query }, &current, None, cx, &mut rows),
                Some((mv, label)) => {
                    let ext = vars::ExternalStudio { path: path.clone(), microversion: mv, version: label };
                    match vars::external_document(&ext) {
                        Some(src) => studio_rows(&Rows { doc: &doc, target, state: &state, current: &current, query: &query }, &src, Some(ext), cx, &mut rows),
                        None => rows.push(os::info_box("This version could not be read.").m(px(10.)).into_any_element()),
                    }
                }
            }
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
                            Some(src) => studio_rows(&Rows { doc: &doc, target, state: &state, current: &current, query: &query }, &src, Some(ext), cx, &mut rows),
                            None => rows.push(os::info_box("This version could not be read.").m(px(10.)).into_any_element()),
                        }
                    }
                }
            }
        }
        let square = |id: &'static str, icon: &'static str, tip: &'static str, on: bool| {
            div()
                .id(id)
                .size(px(30.))
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .rounded(px(3.))
                .border_1()
                .border_color(rgb(0xaaaaaa))
                .cursor_pointer()
                .when(on, |d| d.bg(rgb(0xeaeaea)))
                .hover(|d| d.bg(pal.panel_hover))
                .tooltip(crate::ui::tooltip(tip))
                .child(crate::assets::icon(icon, 20., cx))
        };
        let own_label = state.borrow().own_version.as_ref().map(|(_, l)| l.clone()).unwrap_or(branch);
        let (st_g, doc_v) = (state.clone(), doc.clone());
        let document_row = h_flex()
            .h(px(40.))
            .flex_none()
            .pl(px(6.))
            .pr(px(5.))
            .gap(px(6.))
            .border_b_1()
            .border_color(rgb(0xeeeeee))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .child(div().truncate().text_size(px(14.67)).font_weight(FontWeight::MEDIUM).line_height(px(19.)).text_color(pal.text).child(doc_name))
                    .child(h_flex().gap_1().opacity(0.5).text_size(px(12.)).line_height(px(15.6)).text_color(pal.text).child(crate::ui::glyph("branch", 10., pal.text)).child(own_label)),
            )
            .child(square("ivs-create-version", "version", "Create version…", false).on_click(move |_, window, cx| crate::history_panel::open_create_version_in(doc_v.clone(), window, cx)))
            .child(square("ivs-version-graph", "version-graph", "Version graph", graph).on_click(move |_, window, _| {
                let mut s = st_g.borrow_mut();
                s.graph = !s.graph;
                window.refresh();
            }));
        let (doc_ok, doc_x, before_x, st_ok) = (doc.clone(), doc.clone(), before.clone(), state.clone());
        let header = h_flex()
            .h(px(28.))
            .flex_none()
            .bg(gpui_kit::white())
            .border_b_1()
            .border_color(rgb(0xd7d7d7))
            .child(
                // Red until a studio is picked (live: "… did not regenerate properly").
                div()
                    .id("ivs-title")
                    .flex_1()
                    .pl(px(6.))
                    .text_size(px(13.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(if changed { pal.text } else { rgb(0xaa252d) })
                    .when(!changed, |d| d.tooltip(crate::ui::tooltip("Insert Variable Studio did not regenerate properly: Error regenerating.")))
                    .child("Insert Variable Studio"),
            )
            .child(
                div()
                    .id("ivs-ok")
                    .size(px(28.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(pal.accept).when(!changed, |d| d.opacity(0.2))
                    .child(crate::assets::icon("dialog-ok", 24., cx))
                    .on_click(move |_, window, cx| {
                        let _ = (&doc_ok, &st_ok);
                        if changed {
                            window.close_dialog(cx);
                        }
                    }),
            )
            .child(
                div()
                    .id("ivs-cancel")
                    .size(px(28.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(crate::assets::icon("dialog-cancel", 24., cx))
                    .on_click(move |_, window, cx| {
                        if let Some(b) = before_x.clone() {
                            restore(&doc_x, target, b, cx);
                        }
                        window.close_dialog(cx);
                    }),
            );
        // Live: a 42 px search container (6 px above and below the box).
        let search_row = div().flex_none().py(px(6.)).px(px(6.)).child(
            h_flex()
                .h(px(30.))
                .rounded(px(3.))
                .border_1()
                .border_color(rgb(0xaaaaaa))
                .bg(gpui_kit::white())
                .child(gpui_kit::component::input::Input::new(&search).appearance(false).text_size(px(13.33)).pl(px(5.))),
        );
        let (doc_e, before_e) = (doc_c.clone(), before_c.clone());
        dialog
            .w(px(300.))
            .margin_left(px(246.))
            .margin_top(px(79.))
            .overlay(false)
            .close_button(false)
            .p_0()
            .gap_0()
            .rounded(px(0.))
            .border_0()
            .bg(gpui_kit::rgba(0xfffffff2))
            .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(5.)), blur_radius: px(10.), spread_radius: px(0.), inset: false }])
            .on_cancel(move |_, _, cx| {
                if let Some(b) = before_e.clone() {
                    restore(&doc_e, target, b, cx);
                }
                true
            })
            .child(
                v_flex()
                    .h(px(581.))
                    .child(header)
                    .child(h_flex().flex_none().border_b_1().border_color(rgb(0xeeeeee)).child(tab("ivs-current", "Current document", !other, state.clone())).child(tab("ivs-other", "Other documents", other, state.clone())))
                    .when(!other, |d| d.child(document_row).child(search_row))
                    .child(v_flex().id("ivs-rows").flex_1().min_h_0().overflow_y_scroll().pt(px(1.)).children(rows))
                    // Live: a 22 px footer under a #eaeaea rule with the ? at the right.
                    .child(
                        h_flex().h(px(22.)).flex_none().border_t_1().border_color(rgb(0xeaeaea)).justify_end().pr(px(4.)).child(
                            // Live: opens help "Variable Studios" (same from every tab).
                            div().id("ivs-help").cursor_pointer().child(crate::assets::icon("help-faint", 14., cx)).on_click(|_, _, cx| cx.open_url("https://cad.onshape.com/help/Content/VariableStudio/variable_studio.htm?cshid=variable_studios")),
                        ),
                    ),
            )
    });
}

/// The target's references (what ✕ puts back).
#[derive(Clone, PartialEq)]
enum RefsSnapshot {
    Studio(Vec<StudioRef>),
    Element(vars::VariableStudioRefs),
}

fn snapshot(doc: &noshape_model::Document, target: InsertTarget) -> Option<RefsSnapshot> {
    match target {
        InsertTarget::Studio(t) => match &doc.tab(t)?.content {
            TabContent::VariableStudio(v) => Some(RefsSnapshot::Studio(v.inserted.clone())),
            _ => None,
        },
        InsertTarget::Element(e) => match &doc.tab(e)?.content {
            TabContent::PartStudio(ps) => Some(RefsSnapshot::Element(ps.variable_studios.clone())),
            TabContent::Assembly(a) => Some(RefsSnapshot::Element(a.variable_studios.clone())),
            _ => None,
        },
    }
}

/// Cancel: the target's references as they were when the dialog opened.
fn restore(doc: &Entity<DocModel>, target: InsertTarget, snap: RefsSnapshot, cx: &mut App) {
    if snapshot(&doc.read(cx).session.doc, target).as_ref() == Some(&snap) {
        return;
    }
    let tab = match target {
        InsertTarget::Studio(t) | InsertTarget::Element(t) => t,
    };
    doc.update(cx, |d, cx| {
        d.edit_tab(tab, "Remove reference", cx, |c| match (c, &snap) {
            (TabContent::VariableStudio(v), RefsSnapshot::Studio(r)) => v.inserted = r.clone(),
            (TabContent::PartStudio(ps), RefsSnapshot::Element(r)) => ps.variable_studios = r.clone(),
            (TabContent::Assembly(a), RefsSnapshot::Element(r)) => a.variable_studios = r.clone(),
            _ => {}
        });
    });
}

/// The version graph of this document: Main, then its versions; picking
/// one lists that version's Variable Studios.
fn version_rows(state: &Rc<RefCell<DialogState>>, path: &std::path::Path, rows: &mut Vec<AnyElement>) {
    let pal = p();
    let versions: Vec<crate::derived::VersionChoice> = crate::derived::external_doc_info(path).map(|i| i.versions.into_iter().filter(|v| v.label != "Current state").collect()).unwrap_or_default();
    let chosen = state.borrow().own_version.as_ref().map(|(mv, _)| mv.clone());
    let row = |id: SharedString, label: String, on: bool, pick: Option<(String, String)>| {
        let st = state.clone();
        h_flex()
            .id(id)
            .h(px(36.))
            .flex_none()
            .pl(px(12.))
            .text_size(px(14.))
            .text_color(pal.text)
            .cursor_pointer()
            .when(on, |d| d.bg(rgb(0xb2ddf6)))
            .hover(|d| d.bg(rgb(0xdef1fb)))
            .child(label)
            .on_click(move |_, window, _| {
                let mut s = st.borrow_mut();
                s.own_version = pick.clone();
                s.graph = false;
                window.refresh();
            })
            .into_any_element()
    };
    rows.push(row("ivs-v-main".into(), "Main".into(), chosen.is_none(), None));
    for v in versions {
        let mv = v.microversion.to_string();
        rows.push(row(SharedString::from(format!("ivs-v-{mv}")), v.label.clone(), chosen.as_deref() == Some(mv.as_str()), Some((mv, v.label))));
    }
}

/// "+" Create Variable Studio from a Part Studio's / Assembly's Variable
/// table: a new studio tab right after `element`, its table shown here at
/// once, ready for input (the first studio is inserted into all; a later
/// one, unticked, is referenced by `element`).
pub fn create_studio(doc: &Entity<DocModel>, element: TabId, cx: &mut App) {
    doc.update(cx, |d, cx| {
        d.edit_document("Insert tab : Variable Studio", cx, |doc| {
            let name = doc.next_tab_name("Variable Studio");
            let studio = vars::new_studio(doc);
            let all = studio.insert_into_all;
            let id = doc.add_tab(&name, TabContent::VariableStudio(studio));
            if !all {
                let r = StudioRef { studio: id, variables: None, external: None, config: Default::default() };
                match doc.tab_mut(element).map(|t| &mut t.content) {
                    Some(TabContent::PartStudio(ps)) => vars::insert_studio(&mut ps.variable_studios, r),
                    Some(TabContent::Assembly(a)) => vars::insert_studio(&mut a.variable_studios, r),
                    _ => {}
                }
            }
            let order = &mut doc.settings.tab_order;
            order.retain(|t| *t != id);
            let at = order.iter().position(|t| *t == element).map(|i| i + 1).unwrap_or(order.len());
            order.insert(at, id);
        });
    });
}

/// The Variable table footer of a Part Studio / Assembly: a split button
/// whose main part is "Create Variable Studio" while the document has no
/// Variable Studio (live part-studio/77) and "Insert Variable Studio" once
/// it has one (156); the ▴ menu has Insert Variable Studio and Create
/// Variable Studio (157).
pub fn panel_footer(doc: Entity<DocModel>, element: TabId, cx: &App) -> AnyElement {
    use gpui_kit::component::button::{Button, ButtonCustomVariant, ButtonVariants as _};
    let pal = p();
    let variant = ButtonCustomVariant::new(cx).color(rgb(0xffffff).into()).foreground(pal.text.into()).hover(pal.panel_hover.into()).active(pal.panel_active.into()).shadow(false);
    let d1 = doc.clone();
    let any_studio = doc.read(cx).session.doc.tabs.iter().any(|t| matches!(t.content, TabContent::VariableStudio(_)));
    let (icon, label) = if any_studio { ("insert-variable", "Insert Variable Studio") } else { ("create-variable", "Create Variable Studio") };
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
                .child(h_flex().gap(px(3.)).child(crate::assets::icon(icon, 20., cx)).child(div().text_size(px(14.)).child(label)))
                .on_click(move |_, window, cx| if any_studio { open(d1.clone(), InsertTarget::Element(element), window, cx) } else { create_studio(&d1, element, cx) }),
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
                .on_click(move |e, window, cx| {
                    let (d2, d3) = (doc.clone(), doc.clone());
                    let menu = crate::os_menu::OsMenu::build(window, cx, move |m, _, _| {
                        m.dropdown()
                            .item(crate::os_menu::OsMenuItem::new("Insert Variable Studio").icon_name("insert-variable").on_click(move |_, window, cx| open(d3.clone(), InsertTarget::Element(element), window, cx)))
                            .item(crate::os_menu::OsMenuItem::new("Create Variable Studio").icon_name("create-variable").on_click(move |_, _, cx| create_studio(&d2, element, cx)))
                    });
                    crate::os_menu::open_anchored(point(e.position().x + px(13.), e.position().y - px(18.)), Anchor::BottomRight, menu, window, cx);
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
                Some(vars::VarKind::FromImport) => "From import",
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
        .child(div().mt(px(3.)).child(os::glyph(os::GLYPH_OVERFLOW, 20., pal.text)))
        .on_click(move |e, window, cx| {
            let (d, ext, name) = (d.clone(), ext.clone(), name.clone());
            let menu = crate::os_menu::OsMenu::build(window, cx, move |m, _, _| {
            let (d1, d2, d3, d4) = (d.clone(), d.clone(), d.clone(), d.clone());
            let m = m.dropdown();
            let m = match &ext {
                // Onshape: Open linked document / Update linked document.
                Some(e) => {
                    let (p1, e2) = (e.path.clone(), e.clone());
                    m.item(crate::os_menu::OsMenuItem::new("Open linked document").on_click(move |_, _, cx| {
                        let gpu = cx.global::<crate::gpu::GpuGlobal>().0.clone();
                        let _ = crate::files::open_document_window(p1.clone(), None, gpu, cx);
                    }))
                        .item(crate::os_menu::OsMenuItem::new("Update linked document").on_click(move |_, window, cx| open_at_document(d4.clone(), target, e2.path.clone(), window, cx)))
                }
                None => m.item(crate::os_menu::OsMenuItem::new(format!("Switch to {name}")).on_click(move |_, window, cx| crate::workspace::request_activate_tab(studio_id, window, cx))),
            };
            let m = if k > 0 { m.item(crate::os_menu::OsMenuItem::new("Move up").on_click(move |_, _, cx| move_ref(&d1, target, studio_id, false, cx))) } else { m };
            let m = if k + 1 < n { m.item(crate::os_menu::OsMenuItem::new("Move down").on_click(move |_, _, cx| move_ref(&d2, target, studio_id, true, cx))) } else { m };
            m.item(crate::os_menu::OsMenuItem::new("Remove reference").on_click(move |_, _, cx| set_ref(&d3, target, studio_id, None, cx)))
        });
            crate::os_menu::open_anchored(point(e.position().x + px(12.), e.position().y + px(12.)), Anchor::TopRight, menu, window, cx);
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
