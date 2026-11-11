//! Drawing editor: Onshape's Reference manager (Drawing tab › Change to
//! version…, Sheets panel › Reference manager). Live (behaviours "Drawing
//! tab › Change to version…", drawing/292–303): a 330 × 430 panel at the
//! top left over the sheet, tabs Update to latest | Selective update, "ⓘ
//! Newer versions available for N documents", one row per referenced
//! document with newer versions ("Main ⇨ V3", "V1 ⇨ V3"); Update all moves
//! each to its latest version; Selective update ticks rows and picks their
//! versions (Version graph; this document's Main too) for Update selected.
//! The panel stays open after an update, listing what is still behind.

use crate::drawing_view::*;
use crate::theme::p;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::drawing::{PinnedVersion, TableKind, ViewKind};
use std::collections::{HashMap, HashSet};

/// Where a referenced document's views go.
#[derive(Clone, Debug, PartialEq)]
pub enum RefTarget {
    /// This document's workspace.
    Main,
    /// This document at a version.
    Version(PinnedVersion),
    /// Another document at a microversion.
    External(noshape_store::MicroversionId),
    /// This document's first state (Change to version…'s Start, live doc-tab-menu-change-to-version).
    Start(noshape_store::MicroversionId),
}

/// A referenced document with newer versions.
#[derive(Clone)]
pub struct RefDoc {
    pub name: String,
    /// Another document's file; None: this document.
    pub path: Option<std::path::PathBuf>,
    /// What its views show now ("Main", "V1").
    pub current: String,
    /// Its choices (label, target): this document's Main first, then the
    /// versions newest first.
    pub choices: Vec<(String, RefTarget)>,
    /// The choice Update all takes (the latest version).
    pub latest: usize,
    pub thumbnail: Option<std::sync::Arc<RenderImage>>,
    /// A newer version is there (the "Newer versions available for n documents" count): not this document's own row
    /// that Change to version… lists anyway.
    pub newer: bool,
}

/// Reference manager state: tab (0 Update to latest, 1 Selective update),
/// the documents, ticked rows and per-row chosen versions.
pub struct RefManager {
    pub tab: u8,
    /// Opened by the tab's Change to version…: this document's row stays listed.
    pub change_to_version: bool,
    pub docs: Vec<RefDoc>,
    pub checked: HashSet<usize>,
    pub chosen: HashMap<usize, usize>,
}

/// A base view's own reference: this document (workspace or a version id)
/// or another document's (its path).
fn other_path(pin: Option<&PinnedVersion>) -> Option<std::path::PathBuf> {
    pin.and_then(|p| crate::derived::ExternalRef::parse(&p.id)).map(|r| r.path)
}

/// [`DrawingEditor::ext_info`]'s key of another document at a microversion.
pub fn ext_info_key(r: &crate::derived::ExternalRef) -> String {
    format!("{}|{}", r.microversion, r.path.display())
}

impl DrawingEditor {
    /// Read off the UI thread the version names of the drawing's other-document references and whether a newer
    /// version exists (the Sheets panel row's ⧫ V1 or blue update icon); `force`: all again.
    pub fn check_ext_refs(&mut self, force: bool, cx: &mut Context<Self>) {
        if self.ext_checking {
            return;
        }
        let mut todo: Vec<crate::derived::ExternalRef> = Vec::new();
        for p in self.drawing.sheets.iter().flat_map(|s| &s.views).filter_map(|v| v.pinned.as_ref()) {
            let Some(r) = crate::derived::ExternalRef::parse(&p.id) else { continue };
            if (force || !self.ext_info.contains_key(&ext_info_key(&r))) && !todo.iter().any(|t| t.path == r.path && t.microversion == r.microversion) {
                todo.push(r);
            }
        }
        if todo.is_empty() {
            return;
        }
        self.ext_checking = true;
        cx.spawn(async move |this, cx| {
            let got: Vec<(String, (String, bool))> = cx
                .background_spawn(async move {
                    todo.iter()
                        .map(|r| {
                            let label = noshape_ops::workspace_update::version_label(r).unwrap_or_else(|| format!("state {}", r.microversion.short()));
                            (ext_info_key(r), (label, crate::derived::newer_version(r).is_some()))
                        })
                        .collect()
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.ext_checking = false;
                this.ext_info.extend(got);
                cx.notify();
            });
        })
        .detach();
    }

    /// A pinned reference's version name and whether a newer one exists (another document's), once read.
    pub fn ext_info_of(&self, pin: &PinnedVersion) -> Option<(String, bool)> {
        let r = crate::derived::ExternalRef::parse(&pin.id)?;
        self.ext_info.get(&ext_info_key(&r)).cloned()
    }

    pub fn open_ref_manager(&mut self, cx: &mut Context<Self>) {
        let docs = self.referenced_docs(false, cx);
        self.ref_manager = Some(RefManager { tab: 0, change_to_version: false, docs, checked: HashSet::new(), chosen: HashMap::new() });
        cx.notify();
    }

    /// The tab's Change to version… (live doc-tab-menu-change-to-version): on Selective update, this document's row
    /// listed whether or not a newer version is there ("Main ⇨ Start" with no versions).
    pub fn open_change_to_version(&mut self, cx: &mut Context<Self>) {
        let docs = self.referenced_docs(true, cx);
        let tab = u8::from(docs.iter().all(|d| !d.newer));
        self.ref_manager = Some(RefManager { tab, change_to_version: true, docs, checked: HashSet::new(), chosen: HashMap::new() });
        cx.notify();
    }

    /// The referenced documents that have newer versions than their views
    /// show (live: a workspace reference counts while a version exists).
    fn referenced_docs(&self, change_to_version: bool, cx: &App) -> Vec<RefDoc> {
        let dm = self.doc.read(cx);
        let bases: Vec<Option<PinnedVersion>> = self.drawing.sheets.iter().flat_map(|s| &s.views).filter(|v| matches!(v.kind, ViewKind::Base { .. })).map(|v| v.pinned.clone()).collect();
        let mut out = Vec::new();
        // This document: its views on Main or a version of it.
        let own: Vec<&Option<PinnedVersion>> = bases.iter().filter(|p| other_path(p.as_ref()).is_none()).collect();
        if let Some(first) = own.first() {
            let versions = dm.session.store.list_versions().unwrap_or_default();
            let behind = versions.last().is_some_and(|newest| own.iter().any(|p| p.as_ref().is_none_or(|p| p.id != newest.id.to_string())));
            if behind || change_to_version {
                let branch = dm.session.store.branch(dm.session.branch).map(|b| b.name).unwrap_or_else(|_| "Main".into());
                let current = first.as_ref().map_or(branch.clone(), |p| p.name.clone());
                let mut choices = vec![(branch, RefTarget::Main)];
                choices.extend(versions.iter().rev().map(|v| (v.name.clone(), RefTarget::Version(PinnedVersion { locked: false, id: v.id.to_string(), name: v.name.clone() }))));
                // Change to version… offers the document's Start too (as the assembly's, live asm-menu-change-to-version).
                if change_to_version && let Some(root) = dm.session.store.graph().ok().and_then(|g| g.microversions.iter().find(|m| m.parents.is_empty()).map(|m| m.id)) {
                    choices.push(("Start".into(), RefTarget::Start(root)));
                }
                if choices.len() > 1 {
                    let thumbnail = dm.session.store.get_meta("thumbnail").ok().flatten().as_deref().and_then(crate::library::decode_thumbnail);
                    out.push(RefDoc { name: dm.name(), path: None, current, choices, latest: 1, thumbnail, newer: behind });
                }
            }
        }
        // Other documents (Insert view › Other documents), by file.
        let mut seen: Vec<std::path::PathBuf> = Vec::new();
        // (Pinned ones, Pin reference, stay at their version: live 2303.)
        for p in bases.iter().flatten().filter(|p| !p.locked) {
            let Some(r) = crate::derived::ExternalRef::parse(&p.id) else { continue };
            if seen.contains(&r.path) {
                continue;
            }
            seen.push(r.path.clone());
            let Ok(info) = crate::derived::external_doc_info(&r.path) else { continue };
            let choices: Vec<(String, RefTarget)> = info.versions.iter().filter(|v| v.label != "Current state").map(|v| (v.label.clone(), RefTarget::External(v.microversion))).collect();
            let Some((_, RefTarget::External(newest))) = choices.first() else { continue };
            if *newest == r.microversion {
                continue;
            }
            let current = choices.iter().find(|(_, t)| *t == RefTarget::External(r.microversion)).map(|(l, _)| l.clone()).unwrap_or_else(|| format!("state {}", r.microversion.short()));
            let thumbnail = crate::start_library::file_thumbnail(&r.path);
            out.push(RefDoc { name: info.name, path: Some(r.path), current, choices, latest: 0, thumbnail, newer: true });
        }
        out
    }

    /// Move one document's views (and BOM tables of it) to `target`.
    fn update_reference(&mut self, d: &RefDoc, target: &RefTarget, label: &str, cx: &mut Context<Self>) {
        let (path, name) = (d.path.clone(), d.name.clone());
        let target = target.clone();
        let own = self.doc.read(cx).session.path.clone();
        self.edit(&format!("Change to version : {label}"), cx, |dr| {
            for v in dr.sheets.iter_mut().flat_map(|s| s.views.iter_mut()).filter(|v| matches!(v.kind, ViewKind::Base { .. })) {
                match (&path, &target) {
                    (None, RefTarget::Main) if other_path(v.pinned.as_ref()).is_none() => v.pinned = None,
                    (None, RefTarget::Version(pin)) if other_path(v.pinned.as_ref()).is_none() => v.pinned = Some(pin.clone()),
                    // This document's Start: its state there, by reference.
                    (None, RefTarget::Start(mv)) if other_path(v.pinned.as_ref()).is_none() => {
                        if let ViewKind::Base { source, .. } = &v.kind {
                            v.pinned = Some(PinnedVersion { locked: false, id: crate::derived::ExternalRef { path: own.clone(), microversion: *mv, tab: source.to_string() }.key(), name: "Start".into() });
                        }
                    }
                    (Some(path), RefTarget::External(mv)) => {
                        if let Some(r) = v.pinned.as_ref().filter(|p| !p.locked).and_then(|p| crate::derived::ExternalRef::parse(&p.id)).filter(|r| r.path == *path) {
                            v.pinned = Some(PinnedVersion { locked: false, id: crate::derived::ExternalRef { microversion: *mv, ..r }.key(), name: format!("{name} {label}") });
                        }
                    }
                    _ => {}
                }
            }
            // BOM tables of another document's assembly (drawing.md TB14).
            if let (Some(path), RefTarget::External(mv)) = (&path, &target) {
                for t in dr.sheets.iter_mut().flat_map(|s| s.tables.iter_mut()) {
                    // (And cut lists of its composite parts.)
                    let version = match &mut t.kind {
                        TableKind::Bom { source: Some(s), .. } => &mut s.version,
                        TableKind::CutList { source: Some(s), .. } => &mut s.version,
                        _ => continue,
                    };
                    if let Some(r) = version.as_deref().and_then(crate::derived::ExternalRef::parse).filter(|r| r.path == *path) {
                        *version = Some(crate::derived::ExternalRef { microversion: *mv, ..r }.key());
                    }
                }
            }
        });
    }

    /// Update all / Update selected: no message; the panel stays open and
    /// lists what is still behind (live 297, assembly 606–607).
    fn apply_ref_manager(&mut self, cx: &mut Context<Self>) {
        let Some(m) = self.ref_manager.take() else { return };
        for (i, d) in m.docs.iter().enumerate() {
            if m.tab == 1 && !m.checked.contains(&i) {
                continue;
            }
            let k = m.chosen.get(&i).copied().unwrap_or(d.latest);
            if let Some((label, target)) = d.choices.get(k) {
                self.update_reference(d, target, label, cx);
            }
        }
        self.ensure_sources(false, cx);
        let docs = self.referenced_docs(m.change_to_version, cx);
        self.ref_manager = Some(RefManager { tab: m.tab, change_to_version: m.change_to_version, docs, checked: HashSet::new(), chosen: HashMap::new() });
        cx.notify();
    }

    pub fn render_ref_manager(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let m = self.ref_manager.as_ref()?;
        let pal = p();
        let tab = m.tab;
        let blue = gpui_kit::rgb(0x1651b0);
        let ink = gpui_kit::rgb(0x333333);
        let tab_button = |id: &'static str, label: &'static str, ix: u8| {
            let on = tab == ix;
            div()
                .id(id)
                .flex_1()
                .h(px(32.))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(12.))
                .cursor_pointer()
                .when(on, |d| d.font_weight(FontWeight::MEDIUM).text_color(blue).border_b_2().border_color(blue))
                .when(!on, |d| d.text_color(ink))
                .child(label)
        };
        let n = m.docs.iter().filter(|d| d.newer).count();
        let mut list = v_flex().flex_1().min_h_0();
        for (i, d) in m.docs.iter().enumerate() {
            let chosen = m.chosen.get(&i).copied().unwrap_or(d.latest);
            let to = d.choices.get(chosen).map(|c| c.0.clone()).unwrap_or_default();
            let checked = m.checked.contains(&i);
            let labels: Vec<String> = d.choices.iter().map(|c| c.0.clone()).collect();
            list = list.child(
                h_flex()
                    .h(px(40.))
                    .my(px(2.))
                    .when(tab == 1, |r| {
                        r.child(div().w(px(19.)).pl(px(4.)).child(crate::drawing_chrome::DrCheckbox::new(("rm-check", i)).native().checked(checked).on_click(cx.listener(move |this, v: &bool, _, cx| {
                            if let Some(m) = &mut this.ref_manager {
                                if *v {
                                    m.checked.insert(i);
                                } else {
                                    m.checked.remove(&i);
                                }
                            }
                            cx.notify();
                        }))))
                    })
                    .child(div().w(px(70.)).h(px(40.)).px(px(2.)).flex_none().flex().items_center().justify_center().child(match d.thumbnail.clone() {
                        Some(img) => gpui_kit::img(img).w(px(66.)).h(px(40.)).object_fit(ObjectFit::Contain).into_any_element(),
                        None => crate::assets::icon("document", 32., cx).into_any_element(),
                    }))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .pl(px(4.))
                            .child(div().text_size(px(14.67)).font_weight(FontWeight::MEDIUM).text_color(ink).truncate().child(crate::ui::user_text(d.name.clone())))
                            .child(div().text_size(px(12.)).opacity(0.5).text_color(ink).truncate().child(format!("{} ⇨ {to}", d.current))),
                    )
                    .when(tab == 1, |r| {
                        r.child(
                            div()
                                .id(SharedString::from(format!("rm-graph-{i}")))
                                .mr(px(21.))
                                .size(px(30.))
                                .flex()
                                .flex_none()
                                .items_center()
                                .justify_center()
                                .rounded(px(3.))
                                .border_1()
                                .border_color(gpui_kit::rgb(0xaaaaaa))
                                .cursor_pointer()
                                .tooltip(crate::ui::tooltip("Version graph"))
                                .child(crate::ui::glyph("version-graph", 20., ink))
                                .on_click(cx.listener(move |_, ev: &ClickEvent, window, cx| {
                                    // Picking a version ticks the row (live 296).
                                    let entries: Vec<MenuEntry> = labels
                                        .iter()
                                        .enumerate()
                                        .map(|(k, label)| {
                                            MenuEntry::check(label.clone(), k == chosen, move |t, _, cx| {
                                                if let Some(m) = &mut t.ref_manager {
                                                    m.chosen.insert(i, k);
                                                    m.checked.insert(i);
                                                }
                                                cx.notify();
                                            })
                                        })
                                        .collect();
                                    let ed = cx.entity();
                                    let menu = crate::os_menu::OsMenu::build(window, cx, move |m, window, cx| fill_os_menu(m, &entries, &ed, window, cx));
                                    crate::os_menu::open(ev.position(), menu, window, cx);
                                })),
                        )
                    }),
            );
        }
        let enabled = n > 0 && (tab == 0 || !m.checked.is_empty());
        let info = format!("Newer versions available for {n} document{}", if n == 1 { "" } else { "s" });
        // Its texts for the ui-parity runner (live 2303: a flyout of the drawing).
        // (As live's reader reads it since 11-10: a panel of texts, a selective row's box first.)
        let mut text: Vec<String> = vec!["Update to latest".into(), "Selective update".into(), info.clone()];
        for (i, d) in m.docs.iter().enumerate() {
            let to = d.choices.get(m.chosen.get(&i).copied().unwrap_or(d.latest)).map(|c| c.0.clone()).unwrap_or_default();
            if tab == 1 {
                text.push(if m.checked.contains(&i) { "[x] ".into() } else { "[ ] ".into() });
            }
            text.extend([d.name.clone(), d.current.clone(), "⇨".into(), to]);
        }
        text.push(if tab == 0 { "Update all".into() } else { "Update selected".into() });
        crate::ui_script::note_dialog(|| serde_json::json!({ "kind": "panel", "title": "Reference manager", "text": text }));
        Some(
            v_flex()
                .id("dr-ref-manager")
                .absolute()
                .top(px(DIALOG_TOP))
                .left(px(self.dialog_left()))
                .w(px(330.))
                .h(px(430.))
                .bg(pal.dialog_bg)
                .text_color(ink)
                .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(5.)), blur_radius: px(10.), spread_radius: px(0.), inset: false }])
                .occlude()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    h_flex()
                        .h(px(29.))
                        .bg(gpui_kit::rgb(0xffffff))
                        .border_b_1()
                        .border_color(gpui_kit::rgb(0xd7d7d7))
                        .child(div().flex_1().pl(px(6.)).text_size(px(13.)).font_weight(FontWeight::MEDIUM).child("Reference manager"))
                        .child(
                            div()
                                .id("rm-close")
                                .size(px(28.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .child(crate::ui::glyph("close", 12., pal.text_muted))
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.ref_manager = None;
                                    cx.notify();
                                })),
                        ),
                )
                .child(
                    h_flex()
                        .bg(gpui_kit::rgb(0xffffff))
                        .border_b_1()
                        .border_color(gpui_kit::rgb(0xeeeeee))
                        .child(tab_button("rm-tab-latest", "Update to latest", 0).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            if let Some(m) = &mut this.ref_manager {
                                m.tab = 0;
                            }
                            cx.notify();
                        })))
                        .child(tab_button("rm-tab-selective", "Selective update", 1).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            if let Some(m) = &mut this.ref_manager {
                                m.tab = 1;
                            }
                            cx.notify();
                        }))),
                )
                .child(
                    v_flex()
                        .flex_1()
                        .min_h_0()
                        .bg(gpui_kit::rgb(0xffffff))
                        .pt(px(6.))
                        .child(
                            h_flex()
                                .h(px(30.))
                                .px(px(3.))
                                .gap(px(4.))
                                .border_b_1()
                                .border_color(gpui_kit::rgb(0xeaeaea))
                                .child(crate::assets::icon("inserted-version-update", 20., cx))
                                .child(div().text_size(px(12.)).child(info)),
                        )
                        .child(list),
                )
                .child(
                    h_flex().h(px(46.)).px(px(5.)).justify_end().border_t_1().border_color(gpui_kit::rgb(0xeaeaea)).bg(gpui_kit::rgb(0xffffff)).child(
                        div()
                            .id("rm-update")
                            .h(px(34.))
                            .px(px(10.5))
                            .flex()
                            .items_center()
                            .rounded(px(5.25))
                            .bg(blue)
                            .text_size(px(14.))
                            .text_color(gpui_kit::rgb_static(0xffffff))
                            .when(!enabled, |d| d.opacity(0.4))
                            .when(enabled, |d| d.cursor_pointer())
                            .child(if tab == 0 { "Update all" } else { "Update selected" })
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                if enabled {
                                    this.apply_ref_manager(cx);
                                }
                            })),
                    ),
                )
                .into_any_element(),
        )
    }
}
