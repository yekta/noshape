//! The document ☰ menu's dialogs (Onshape, live document/32): Rename
//! document, Move to…, Document details…, Restore deleted workspaces…, Copy
//! workspace…, Update workspace…, Workspace properties…, and the Workspace
//! units panel.

use crate::doc_model::{DocEvent, DocModel};
use crate::os_ui as os;
use crate::theme::p;
use gpui_kit::component::WindowExt as _;
use gpui_kit::component::input::{InputState, Textarea, TextareaState};
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::TabContent;
use std::rc::Rc;

fn report(doc: &Entity<DocModel>, r: anyhow::Result<()>, cx: &mut App) -> bool {
    match r {
        Ok(()) => {
            doc.update(cx, |d, cx| d.reloaded(cx));
            true
        }
        Err(e) => {
            doc.update(cx, |_, cx| cx.emit(DocEvent::Error(format!("{e:#}"))));
            false
        }
    }
}

/// The document's "Not revision managed" (store meta).
pub const DOC_NOT_REVISION_MANAGED: &str = "doc_not_revision_managed";

/// ☰ › Document details… (live 1738, 600 px): "Enter a document
/// description:" (Maximum of 10000 characters), the textarea, ☐ Not
/// revision managed; [Update] (pale until something changes) [Cancel].
pub fn document_details(doc: Entity<DocModel>, window: &mut Window, cx: &mut App) {
    let path = doc.read(cx).session.path.clone();
    let details = crate::start_library::doc_details(&path).unwrap_or_default();
    let nrm_saved = doc.read(cx).session.store.get_meta(DOC_NOT_REVISION_MANAGED).ok().flatten().is_some_and(|b| b == b"1");
    let nrm = Rc::new(std::cell::Cell::new(nrm_saved));
    let desc_saved = details.description.clone();
    let desc_st = cx.new(|cx| TextareaState::new(window, cx).rows(4).default_value(details.description.clone()));
    // Typing re-renders the dialog (Update turns on).
    window.subscribe(&desc_st, cx, |_, _: &gpui_kit::component::input::InputEvent, window, _| window.refresh()).detach();
    let focus = desc_st.clone();
    window.open_dialog(cx, move |dialog, _, cx| {
        let pal = p();
        let (d, doc, nrm2, saved) = (desc_st.clone(), doc.clone(), nrm.clone(), desc_saved.clone());
        let changed = desc_st.read(cx).value().as_ref() != desc_saved.as_str() || nrm.get() != nrm_saved;
        os::modal(dialog, "Document details", 600.)
            .child(
                os::modal_body()
                    .child(h_flex().gap(px(6.)).child(div().text_size(px(14.)).font_weight(FontWeight::SEMIBOLD).text_color(pal.text).child("Enter a document description:")).child(div().text_size(px(12.)).text_color(pal.text_muted).child("Maximum of 10000 characters")))
                    .child({
                        os::note_modal_input(Some("Enter a document description"), &desc_st.read(cx).value());
                        Textarea::new(&desc_st).h(px(96.)).text_size(px(14.))
                    })
                    .child({
                        let nrm = nrm.clone();
                        os::modal_checkbox("doc-nrm", "Not revision managed", nrm.get()).on_click(move |_, window, _| {
                            nrm.set(!nrm.get());
                            window.refresh();
                        })
                    }),
            )
            .footer(os::modal_footer().child(os::modal_ok(div().when(!changed, |d| d.opacity(0.4)).child(os::primary_button("ok", "Update")))).child(os::modal_cancel(os::cancel_button("cancel", "Cancel", cx))))
            .on_ok(move |_, _, cx| {
                let desc = d.read(cx).value().to_string();
                if desc == saved && nrm2.get() == nrm_saved {
                    return false;
                }
                let on = nrm2.get();
                let r = doc.update(cx, |dm, _| -> anyhow::Result<()> {
                    dm.session.store.set_description(&desc)?;
                    dm.session.store.set_meta(DOC_NOT_REVISION_MANAGED, on.then_some(&b"1"[..]))?;
                    Ok(())
                });
                report(&doc, r, cx)
            })
    });
    // Live (doc-menu-details): the description has the keyboard when the modal opens.
    window.defer(cx, move |window, cx| focus.update(cx, |s, cx| s.focus(window, cx)));
}

/// ☰ › Restore deleted workspaces…: deleted branches with a Restore button.
/// ☰ › Restore deleted workspaces… (live doc-menu-restore-workspaces): the deleted workspaces to tick, [Restore
/// selected] (pale until one is ticked) [Cancel]; none: "No deleted workspaces found in this document."
pub fn restore_deleted_workspaces(doc: Entity<DocModel>, window: &mut Window, cx: &mut App) {
    let picked: Rc<std::cell::RefCell<Vec<noshape_store::BranchId>>> = Rc::default();
    window.open_dialog(cx, move |dialog, _, cx| {
        let pal = p();
        let deleted: Vec<noshape_store::BranchInfo> = doc.read(cx).session.store.graph().map(|g| g.branches.into_iter().filter(|b| b.deleted).collect()).unwrap_or_default();
        let rows: Vec<AnyElement> = if deleted.is_empty() {
            vec![os::info_box("No deleted workspaces found in this document.").into_any_element()]
        } else {
            deleted
                .into_iter()
                .map(|b| {
                    let (picked, id) = (picked.clone(), b.id);
                    let on = picked.borrow().contains(&id);
                    h_flex()
                        .h(px(36.))
                        .gap(px(8.))
                        .child(os::user_checkbox(SharedString::from(format!("restore-{}", id.0)), b.name.clone(), on).flex_1().on_click(move |_, window, _| {
                            let mut p = picked.borrow_mut();
                            if let Some(i) = p.iter().position(|x| *x == id) {
                                p.remove(i);
                            } else {
                                p.push(id);
                            }
                            window.refresh();
                        }))
                        .child(div().text_size(px(13.)).text_color(pal.text_muted).child(crate::start_library::format_onshape_time(b.created)))
                        .into_any_element()
                })
                .collect()
        };
        let any = !picked.borrow().is_empty();
        let (doc, picked) = (doc.clone(), picked.clone());
        os::modal(dialog, "Restore deleted workspaces", 460.)
            .child(os::modal_body().children(rows))
            .footer(os::modal_footer().child(os::modal_ok(div().when(!any, |d| d.opacity(0.4)).child(os::primary_button("ok", "Restore selected")))).child(os::modal_cancel(os::cancel_button("cancel", "Cancel", cx))))
            .on_ok(move |_, _, cx| {
                let ids = picked.borrow().clone();
                if ids.is_empty() {
                    return false;
                }
                let r = doc.update(cx, |d, _| ids.iter().try_for_each(|id| d.session.store.restore_branch(*id)).map_err(Into::into));
                report(&doc, r, cx)
            })
    });
}

/// ☰ › Copy workspace…: a new document from this workspace's state, opened.
pub fn copy_workspace(doc: Entity<DocModel>, window: &mut Window, cx: &mut App) {
    copy_dialog(doc, None, window, cx);
}

/// ☰ › Copy version… (a window on a version): a new document holding the
/// version's state, opened.
pub fn copy_version(doc: Entity<DocModel>, mv: noshape_store::MicroversionId, window: &mut Window, cx: &mut App) {
    copy_dialog(doc, Some(mv), window, cx);
}

/// Live 1738: "Copy the active workspace (version) to a new document":
/// Document name ("<name> - Copy", selected), "Copy document to" with the
/// folder browser, [Create document] [Cancel].
fn copy_dialog(doc: Entity<DocModel>, mv: Option<noshape_store::MicroversionId>, window: &mut Window, cx: &mut App) {
    let (path, name) = {
        let d = doc.read(cx);
        (d.session.path.clone(), d.name())
    };
    let start = path.parent().map(|p| p.to_path_buf()).unwrap_or_else(crate::files::documents_dir);
    let location = Rc::new(crate::start_window::Location::new(start).with_new_folder());
    let name_st = cx.new(|cx| InputState::new(window, cx).default_value(format!("{name} - Copy")));
    let focus = name_st.clone();
    let title = if mv.is_some() { "Copy the active version to a new document" } else { "Copy the active workspace to a new document" };
    window.open_dialog(cx, move |dialog, _, cx| {
        let (loc, path, n, doc) = (location.clone(), path.clone(), name_st.clone(), doc.clone());
        os::modal(dialog, title, 598.)
            .child(
                os::modal_body()
                    .child(os::field_label("Document name"))
                    .child(os::input_h(os::modal_input(Some("Document name"), &name_st, cx), 32.).text_size(px(14.)))
                    .child(os::field_label("Copy document to"))
                    .child(location.render(cx)),
            )
            .footer(os::modal_footer().child(os::modal_ok(os::primary_button("ok", location.ok_label("Create document")))).child(os::modal_cancel(os::cancel_button("cancel", "Cancel", cx))))
            .on_ok(move |_, window, cx| {
                // "Open": into the folder selected in the box (or the new one named), the dialog stays.
                if loc.open_picked(cx) {
                    window.refresh();
                    return false;
                }
                let new_name = n.read(cx).value().trim().to_string();
                if new_name.is_empty() {
                    return false;
                }
                let dir = loc.chosen.borrow().clone();
                let _ = doc.update(cx, |d, _| d.session.store.checkpoint());
                let r = match mv {
                    Some(mv) => crate::start_library::copy_version_named(&path, mv, &dir, &new_name),
                    None => crate::start_library::copy_document_named(&path, &dir, &new_name),
                };
                match r {
                    Ok(dest) => {
                        let gpu = cx.global::<crate::gpu::GpuGlobal>().0.clone();
                        if let Err(e) = crate::files::open_document_window(dest, None, gpu, cx) {
                            doc.update(cx, |_, cx| cx.emit(DocEvent::Error(format!("{e:#}"))));
                        }
                        true
                    }
                    Err(e) => {
                        doc.update(cx, |_, cx| cx.emit(DocEvent::Error(format!("{e:#}"))));
                        false
                    }
                }
            })
    });
    window.defer(cx, move |window, cx| {
        focus.update(cx, |s, cx| {
            s.focus(window, cx);
            s.select_all(window, cx);
        })
    });
}

/// ☰ › Workspace properties…: the workspace's metadata modal, as the
/// Versions and history panel's Properties… on the workspace row (live
/// navbar-version-v1: the same edit-current-workspace-properties command).
pub fn workspace_properties(doc: Entity<DocModel>, window: &mut Window, cx: &mut App) {
    let branch = doc.read(cx).session.branch;
    noshape_history_ui::version_properties::open(doc, noshape_history_ui::version_properties::Item::Workspace(branch), window, cx);
}

/// ☰ › Move to…: the folder browser; the document closes, moves and reopens.
pub fn move_to(doc: Entity<DocModel>, window: &mut Window, cx: &mut App) {
    let path = doc.read(cx).session.path.clone();
    let start = path.parent().map(|p| p.to_path_buf()).unwrap_or_else(crate::files::documents_dir);
    let location = Rc::new(crate::start_window::Location::new(start));
    window.open_dialog(cx, move |dialog, _, cx| {
        let (loc, path) = (location.clone(), path.clone());
        os::modal(dialog, "Move documents and folders to", 460.)
            .child(os::modal_body().child(os::field_label("Location")).child(location.render(cx)))
            .footer(os::modal_footer().child(os::modal_ok(os::primary_button("ok", "Move here"))).child(os::modal_cancel(os::cancel_button("cancel", "Cancel", cx))))
            .on_ok(move |_, window, cx| {
                let dir = loc.chosen.borrow().clone();
                if path.parent() == Some(dir.as_path()) {
                    return true;
                }
                let path = path.clone();
                // Close this window (releasing the store), then move and reopen.
                window.remove_window();
                cx.spawn(async move |cx| {
                    cx.background_executor().timer(std::time::Duration::from_millis(300)).await;
                    cx.update(|cx| {
                        if let Ok(dest) = crate::start_library::move_document(&path, &dir) {
                            crate::files::replace_recent(&path, &dest);
                            let gpu = cx.global::<crate::gpu::GpuGlobal>().0.clone();
                            let _ = crate::files::open_document_window(dest, None, gpu, cx);
                        }
                    });
                })
                .detach();
                true
            })
    });
}

/// One reference to another document that has a newer version.
#[derive(Clone)]
struct Outdated {
    label: String,
    from: String,
    to: crate::derived::VersionChoice,
    kind: OutdatedKind,
}

#[derive(Clone)]
enum OutdatedKind {
    /// A Derived feature (tab, feature id) pinned to `ExternalRef`.
    Derived(noshape_model::document::TabId, noshape_model::reference::FeatureId, crate::derived::ExternalRef),
    /// A Variable Studio table from another document (element / studio tab, studio id).
    Variables(noshape_model::document::TabId, noshape_model::document::TabId),
}

/// References of this workspace to other documents' versions with a newer
/// version available.
fn outdated_references(doc: &noshape_model::Document) -> Vec<Outdated> {
    use noshape_model::variables::VariableStudioRefs;
    let mut out = Vec::new();
    for t in &doc.tabs {
        let mut studio_refs = |owner: noshape_model::document::TabId, refs: &[noshape_model::variables::StudioRef]| {
            for r in refs {
                let Some(e) = &r.external else { continue };
                let Ok(mv) = e.microversion.parse() else { continue };
                let x = crate::derived::ExternalRef { path: e.path.clone(), microversion: mv, tab: String::new() };
                if let Some(v) = crate::derived::newer_version(&x) {
                    let doc_name = crate::derived::external_doc_info(&e.path).map(|i| i.name).unwrap_or_default();
                    out.push(Outdated { label: format!("{doc_name} (Variable Studio) in {}", t.name), from: e.version.clone(), to: v, kind: OutdatedKind::Variables(owner, r.studio) });
                }
            }
        };
        match &t.content {
            TabContent::PartStudio(ps) => {
                let VariableStudioRefs { inserted, .. } = &ps.variable_studios;
                studio_refs(t.id, inserted);
                for f in ps.features.iter().filter(|f| f.type_id == "importDerived") {
                    let Some(x) = crate::derived::ExternalRef::parse(f.params.text("partStudio")) else { continue };
                    if let Some(v) = crate::derived::newer_version(&x) {
                        out.push(Outdated { label: format!("{} in {}", crate::derived::describe_external(&x), t.name), from: String::new(), to: v, kind: OutdatedKind::Derived(t.id, f.id, x) });
                    }
                }
            }
            TabContent::Assembly(a) => studio_refs(t.id, &a.variable_studios.inserted),
            TabContent::VariableStudio(v) => studio_refs(t.id, &v.inserted),
            _ => {}
        }
    }
    out
}

/// Point one outdated reference at its newest version.
fn apply_update(o: &Outdated, doc: &mut noshape_model::Document) {
    match &o.kind {
        OutdatedKind::Derived(tab, fid, x) => {
            if let Some(TabContent::PartStudio(ps)) = doc.tab_mut(*tab).map(|t| &mut t.content)
                && let Some(f) = ps.feature_mut(*fid)
            {
                let key = crate::derived::ExternalRef { microversion: o.to.microversion, ..x.clone() }.key();
                f.params.set("partStudio", noshape_model::params::ParamValue::Text(key));
            }
        }
        OutdatedKind::Variables(owner, studio) => {
            let refs: Option<&mut Vec<noshape_model::variables::StudioRef>> = match doc.tab_mut(*owner).map(|t| &mut t.content) {
                Some(TabContent::PartStudio(ps)) => Some(&mut ps.variable_studios.inserted),
                Some(TabContent::Assembly(a)) => Some(&mut a.variable_studios.inserted),
                Some(TabContent::VariableStudio(v)) => Some(&mut v.inserted),
                _ => None,
            };
            if let Some(r) = refs.and_then(|refs| refs.iter_mut().find(|r| r.studio == *studio))
                && let Some(e) = &mut r.external
            {
                e.microversion = o.to.microversion.to_string();
                e.version = o.to.label.clone();
            }
        }
    }
}

/// ☰ › Update workspace…: the workspace's feature library goes to the latest; noshape ships one, so live's toast says
/// it already is.
pub fn update_workspace(doc: Entity<DocModel>, cx: &mut App) {
    doc.update(cx, |_, cx| cx.emit(DocEvent::Info(noshape_ops::workspace_update::LATEST_MESSAGE.into())));
}

/// The Reference manager (a tab's "Newer versions of linked documents available" icon): references to other
/// documents with newer versions, updated together (Update to latest).
pub fn reference_manager(doc: Entity<DocModel>, window: &mut Window, cx: &mut App) {
    let outdated = outdated_references(&doc.read(cx).session.doc);
    window.open_dialog(cx, move |dialog, _, cx| {
        let pal = p();
        let rows: Vec<AnyElement> = if outdated.is_empty() {
            vec![os::info_box("All references to other documents are up to date.").into_any_element()]
        } else {
            let n = outdated.len();
            std::iter::once(os::info_box(format!("Newer versions available for {n} {}", if n == 1 { "reference" } else { "references" })).into_any_element()).chain(
            outdated
                .iter()
                .map(|o| {
                    h_flex()
                        .h(px(30.))
                        .gap(px(8.))
                        .text_size(px(14.))
                        .child(div().flex_1().truncate().text_color(pal.text).child(o.label.clone()))
                        .child(div().text_color(pal.text_muted).child(if o.from.is_empty() { format!("→ {}", o.to.label) } else { format!("{} → {}", o.from, o.to.label) }))
                        .into_any_element()
                }))
                .collect()
        };
        let (doc, list) = (doc.clone(), outdated.clone());
        let can = !list.is_empty();
        os::modal(dialog, "Reference manager", 560.)
            .child(os::modal_body().children(rows))
            .footer(
                os::modal_footer()
                    .when(can, |f| f.child(os::modal_ok(os::primary_button("ok", "Update all"))))
                    .child(os::modal_cancel(os::cancel_button("cancel", if can { "Cancel" } else { "Close" }, cx))),
            )
            .on_ok(move |_, _, cx| {
                let list = list.clone();
                doc.update(cx, |d, cx| d.edit_document(noshape_ops::workspace_update::DESCRIPTION, cx, |document| list.iter().for_each(|o| apply_update(o, document))));
                true
            })
    });
}

/// Onshape's "Workspace units" panel (live, document/40): a 338 px
/// feature-dialog-style panel (title, green ✓, red ✕) with collapsible
/// Length / Angle / Mechanical groups; each quantity has a default unit and
/// display decimals.
pub struct WorkspaceUnits {
    doc: Entity<DocModel>,
    draft: noshape_units::UnitPrefs,
    collapsed: [bool; 3],
}

pub enum WorkspaceUnitsEvent {
    Close,
}

impl EventEmitter<WorkspaceUnitsEvent> for WorkspaceUnits {}

use noshape_app_core::user_units::{DECIMALS, OTHER_DECIMALS};

impl WorkspaceUnits {
    pub fn new(doc: Entity<DocModel>, cx: &mut Context<Self>) -> Self {
        let draft = doc.read(cx).session.doc.settings.units;
        Self { doc, draft, collapsed: [false; 3] }
    }

    fn apply(&mut self, cx: &mut Context<Self>) {
        let units = self.draft;
        let old = self.doc.read(cx).session.doc.settings.units;
        self.doc.update(cx, |doc, cx| {
            // Unitless sketch dimensions keep their size (session.set_units).
            if let Err(e) = doc.session.set_units(units, "Change workspace units") {
                cx.emit(DocEvent::Error(format!("{e:#}")));
            }
            cx.emit(DocEvent::UnitsChanged { old });
        });
        cx.emit(WorkspaceUnitsEvent::Close);
    }

    /// An underlined select (feature dialogs): `options`, the current one,
    /// and what picking option `i` does to the draft.
    fn select(&self, label: String, w: f32, options: Vec<String>, current: usize, set: impl Fn(&mut noshape_units::UnitPrefs, usize) + 'static, cx: &mut Context<Self>) -> AnyElement {
        use gpui_kit::component::button::{Button, ButtonCustomVariant, ButtonVariants as _};
        use crate::os_ui::OsDropdownExt as _;
        let pal = p();
        let view = cx.entity();
        let set = Rc::new(set);
        let plain = ButtonCustomVariant::new(cx).color(gpui_kit::transparent_white()).foreground(pal.text.into()).hover(pal.panel_hover.into()).active(pal.panel_active.into()).shadow(false);
        let shown = options.get(current).cloned().unwrap_or_default();
        let id = SharedString::from(format!("wu-{label}"));
        let id2 = id.clone();
        noshape_app_core::ui_script::note_field(|| serde_json::json!({ "label": label, "kind": "enum", "text": shown }));
        div()
            .w(px(w))
            .map(|d| noshape_app_core::ui_script::mark(d, || format!("field:{label}")))
            .border_b_1()
            .border_color(rgb(0xd7d7d7))
            .child(
                Button::new(id)
                    .custom(plain)
                    .w_full()
                    .h(px(26.))
                    .rounded_none()
                    .justify_start()
                    .pl(px(6.))
                    .child(h_flex().w(px(w - 12.)).child(div().flex_1().text_size(px(13.33)).text_color(pal.text).child(shown)).child(os::glyph(os::GLYPH_CARET, 7., pal.text)))
                    .os_dropdown_menu(SharedString::from(format!("{id2}-dd")), true, move |mut m, _, _| {
                        for (i, o) in options.iter().enumerate() {
                            let (view, set) = (view.clone(), set.clone());
                            m = m.item(crate::os_menu::OsMenuItem::new(o.clone()).checked(i == current).on_click(move |_, _, cx| {
                                view.update(cx, |this, cx| {
                                    set(&mut this.draft, i);
                                    cx.notify();
                                })
                            }));
                        }
                        m
                    }),
            )
            .into_any_element()
    }

    fn row(&self, key: &'static str, label: String, unit: AnyElement, decimals: AnyElement) -> AnyElement {
        let pal = p();
        h_flex()
            .id(key)
            .px(px(10.))
            .pb(px(10.))
            .gap(px(3.))
            .items_end()
            .child(v_flex().child(div().pl(px(5.)).pt(px(5.)).text_size(px(12.)).text_color(pal.text_muted).child(label)).child(unit))
            .child(v_flex().child(div().pl(px(5.)).pt(px(5.)).text_size(px(12.)).text_color(pal.text_muted).child("Display decimals")).child(decimals))
            .into_any_element()
    }
}

impl Render for WorkspaceUnits {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        use noshape_units::{AngleUnit, LENGTH_UNITS_SHOWN, OTHER_UNITS, OTHER_UNITS_SHOWN};
        let pal = p();
        let d = self.draft;
        // Live: each quantity has its own Display decimals list (Length 6
        // options, Linear acceleration / Angle / Angular velocity 7, Mass and
        // Moment 8, the rest 9).
        let dec = |cur: u8, n: usize| (DECIMALS[..n].iter().map(|s| s.to_string()).collect::<Vec<_>>(), (cur as usize).min(n - 1));
        let mut groups: [Vec<AnyElement>; 3] = [Vec::new(), Vec::new(), Vec::new()];
        // Each group's row labels, for the ui-parity state (live records the panel's texts).
        let mut labels: [Vec<String>; 3] = [Vec::new(), Vec::new(), Vec::new()];
        // Length and angle (the units expressions use), in Onshape's lists.
        let lu = LENGTH_UNITS_SHOWN.iter().position(|u| *u == d.length).unwrap_or(0);
        let (dl, dc) = dec(d.length_precision, 6);
        let unit = self.select("Length default unit".into(), 210., LENGTH_UNITS_SHOWN.iter().map(|u| u.name().to_string()).collect(), lu, |p, i| p.length = LENGTH_UNITS_SHOWN[i], cx);
        let decimals = self.select("Length display decimals".into(), 100., dl, dc, |p, i| p.length_precision = i as u8, cx);
        groups[0].push(self.row("wu-row-length", "Length default unit".into(), unit, decimals));
        labels[0].push("Length default unit".into());
        let au = AngleUnit::ALL.iter().position(|u| *u == d.angle).unwrap_or(0);
        let (al, ac) = dec(d.angle_precision, 7);
        let unit = self.select("Angle default unit".into(), 210., AngleUnit::ALL.iter().map(|u| u.name().to_string()).collect(), au, |p, i| p.angle = AngleUnit::ALL[i], cx);
        let decimals = self.select("Angle display decimals".into(), 100., al, ac, |p, i| p.angle_precision = i as u8, cx);
        groups[1].push(self.row("wu-row-angle", "Angle default unit".into(), unit, decimals));
        labels[1].push("Angle default unit".into());
        for (k, (label, group, names, _, _)) in OTHER_UNITS.iter().enumerate() {
            let (dl, dc) = dec(d.other[k].1, OTHER_DECIMALS[k]);
            let shown = OTHER_UNITS_SHOWN[k];
            let at = shown.iter().position(|&i| i == d.other[k].0).unwrap_or(0);
            let unit = self.select(format!("{label} default unit"), 210., shown.iter().map(|&i| names[i as usize].to_string()).collect(), at, move |p, i| p.other[k].0 = shown[i], cx);
            let decimals = self.select(format!("{label} display decimals"), 100., dl, dc, move |p, i| p.other[k].1 = i as u8, cx);
            let key: &'static str = ["wu-r0", "wu-r1", "wu-r2", "wu-r3", "wu-r4", "wu-r5", "wu-r6", "wu-r7", "wu-r8"][k];
            groups[*group as usize].push(self.row(key, format!("{label} default unit"), unit, decimals));
            labels[*group as usize].push(format!("{label} default unit"));
        }
        // Onshape's order inside a group: the base quantity first.
        let titles = ["Length", "Angle", "Mechanical"];
        // Live's recorder reads the panel's texts in order: a group's title, then per row its label and "Display decimals".
        noshape_app_core::ui_script::note_dialog(|| {
            let fields = noshape_app_core::ui_script::take_fields();
            let text: Vec<String> = (0..3).flat_map(|g| std::iter::once(titles[g].to_string()).chain(if self.collapsed[g] { Vec::new() } else { labels[g].iter().flat_map(|l| [l.clone(), "Display decimals".to_string()]).collect() })).collect();
            serde_json::json!({ "kind": "panel", "title": "Workspace units", "text": text, "tabs": [], "fields": fields, "error": null, "ok": "enabled" })
        });
        let mut body = v_flex().id("wu-body").max_h(px(760.)).overflow_y_scroll();
        for (g, rows) in groups.into_iter().enumerate() {
            let open = !self.collapsed[g];
            body = body.child(
                h_flex()
                    .id(("wu-group", g))
                    .h(px(32.))
                    .px(px(4.))
                    .gap(px(6.))
                    .cursor_pointer()
                    .child(crate::ui::glyph(if open { "ui-chevron-down" } else { "ui-chevron-right" }, 10., pal.text))
                    .child(div().text_size(px(16.)).text_color(pal.text).child(titles[g]))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.collapsed[g] = !this.collapsed[g];
                        cx.notify();
                    })),
            );
            if open {
                body = body.children(rows);
            }
        }
        v_flex()
            .id("workspace-units")
            .occlude()
            .w(px(338.))
            .bg(gpui_kit::rgba(0xfffffff2))
            .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(5.)), blur_radius: px(10.), spread_radius: px(0.), inset: false }])
            .child(
                h_flex()
                    .h(px(29.))
                    .bg(rgb(0xffffff))
                    .border_b_1()
                    .border_color(rgb(0xd7d7d7))
                    .child(div().flex_1().pl(px(6.)).text_size(px(13.)).font_weight(FontWeight::MEDIUM).text_color(pal.text).child("Workspace units"))
                    .child(
                        div()
                            .id("wu-ok")
                            .map(|d| noshape_app_core::ui_script::mark(d, || "button:ok".into()))
                            .size(px(28.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .bg(rgb(0x009400))
                            .cursor_pointer()
                            .child(crate::ui::glyph("check", 18., gpui_kit::rgb_static(0xffffff)))
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.apply(cx))),
                    )
                    .child(
                        div()
                            .id("wu-cancel")
                            .map(|d| noshape_app_core::ui_script::mark(d, || "button:cancel".into()))
                            .size(px(28.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .child(os::glyph(os::GLYPH_CLOSE, 16., rgb(0xd9534f)))
                            .on_click(cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(WorkspaceUnitsEvent::Close))),
                    ),
            )
            .child(body)
    }
}

// ------------------------------------------------------------------ element tabs

/// An element copied with tab › Copy to clipboard (pasted from "+").
pub struct CopiedTab(pub Option<noshape_model::document::Tab>);

impl Global for CopiedTab {}

/// Tab › Properties…: the element's name and description; a drawing's is
/// the metadata Properties dialog (live 2159, 3021: Part number and
/// Revision fill its title block's DWG NO. and REV; drawing.md S15).
/// `thumb`: the tab's picture (the Part Studio card in the left tree).
/// An assembly's computed mass ("12.3456 kg") for its Properties opened from the tab's menu, the assembly perhaps
/// never opened: solved off the UI thread in a session of its own on the saved file (as an agent's
/// assembly_mass_properties). None without materials or when it can't be solved.
fn assembly_mass_later(doc: &Entity<DocModel>, tab: String, cx: &mut App) -> Task<Option<String>> {
    let path = doc.read(cx).session.path.to_string_lossy().to_string();
    cx.background_spawn(async move {
        let mut ctx = noshape_ops::Ctx::new();
        let op = noshape_ops::registry::by_tool_name("assembly_mass_properties")?;
        let r = noshape_ops::registry::run(&mut ctx, op, &serde_json::json!({ "doc": path, "tab": tab })).ok()?.value;
        let kg = r["mass_kg"].as_f64()?;
        Some(format!("{} kg", noshape_units::to_fixed(kg, 4)))
    })
}

/// "2026-11-09" as the Properties dialog shows it, "11/09/2026".
fn us_date(iso: &str) -> String {
    let p: Vec<&str> = iso.split('-').collect();
    match p.as_slice() {
        [y, m, d] => format!("{m}/{d}/{y}"),
        _ => iso.to_string(),
    }
}

/// "11/09/2026" (as typed in the dialog) as the drawing keeps it, "2026-11-09"; `None` for anything else.
fn iso_date(us: &str) -> Option<String> {
    let p: Vec<u32> = us.trim().split('/').map(|x| x.parse().ok()).collect::<Option<_>>()?;
    match p.as_slice() {
        [m, d, y] if (1..=12).contains(m) && (1..=31).contains(d) => Some(format!("{y:04}-{m:02}-{d:02}")),
        _ => None,
    }
}

/// A store timestamp (ms since the epoch, UTC) as MM/DD/YYYY.
fn us_date_of_millis(ms: i64) -> String {
    let (y, m, d) = noshape_model::drawing::format::civil_from_days(ms.div_euclid(86_400_000));
    format!("{m:02}/{d:02}/{y:04}")
}

pub fn tab_properties(doc: Entity<DocModel>, tab: noshape_model::document::TabId, thumb: Option<std::sync::Arc<RenderImage>>, window: &mut Window, cx: &mut App) {
    let drawing = {
        let session = &doc.read(cx).session;
        session.doc.tab(tab).and_then(|t| match &t.content {
            noshape_model::document::TabContent::Drawing(dr) => {
                // Live doc-tab-properties-drawing: Drawn by, Approver, the dates (MM/DD/YYYY), and the shown Last
                // changed date (the document's latest change), Last changed by and Inspection count.
                let changed = session.head().ok().and_then(|h| session.store.microversion(h).ok()).map(|m| us_date_of_millis(m.created)).unwrap_or_default();
                let inspections = dr.sheets.iter().flat_map(|s| s.annotations.iter()).filter(|a| a.inspection.is_some()).count();
                let fields = [dr.drawn_by.clone().unwrap_or_default(), dr.meta.approver.clone(), dr.meta.created.as_deref().map(us_date).unwrap_or_default(), dr.meta.approved.as_deref().map(us_date).unwrap_or_default()];
                Some((t.name.clone(), (*dr.meta).clone(), fields, [changed, String::new(), inspections.to_string()]))
            }
            _ => None,
        })
    };
    if let Some((name, m, fields, info)) = drawing {
        use noshape_app_core::properties_dialog::{MetaSpec, MetaValues};
        let mut item = m.item.clone();
        (item.title1, item.title2, item.title3) = (Some(m.title1.clone()), Some(m.title2.clone()), Some(m.title3.clone()));
        let spec = MetaSpec {
            category: "Onshape Drawing",
            material: None,
            computed_mass: None,
            bom_options: None,
            drawing_info: Some(info),
            values: MetaValues { name, description: m.description.clone(), part_number: m.number.clone(), revision: m.revision.clone(), meta: item, mass_override: None, bom: 0, drawing: Some(fields) },
        };
        noshape_app_core::properties_dialog::open_properties(
            spec,
            move |v, _, cx| {
                doc.update(cx, |d, cx| {
                    d.edit_document(&format!("Properties : {}", v.name), cx, |document| {
                        let Some(t) = document.tab_mut(tab) else { return };
                        t.name = v.name.clone();
                        if let noshape_model::document::TabContent::Drawing(dr) = &mut t.content {
                            let mut item = v.meta.clone();
                            let title = |s: Option<String>| s.unwrap_or_default();
                            let (t1, t2, t3) = (title(item.title1.take()), title(item.title2.take()), title(item.title3.take()));
                            let m = &mut dr.meta;
                            (m.number, m.revision, m.description) = (v.part_number.clone(), v.revision.clone(), v.description.clone());
                            (m.title1, m.title2, m.title3) = (t1, t2, t3);
                            m.item = item;
                            if let Some([by, approver, drawn, approved]) = &v.drawing {
                                m.approver = approver.clone();
                                m.created = iso_date(drawn).or(m.created.take());
                                m.approved = iso_date(approved);
                                dr.drawn_by = Some(by.clone()).filter(|b| !b.is_empty());
                            }
                        }
                    })
                });
            },
            window,
            cx,
        );
        return;
    }
    // Live 1738: every tab kind has the metadata modal, with its own fields
    // (properties_dialog picks them by the category).
    let (name, category) = {
        let d = &doc.read(cx).session.doc;
        let Some(t) = d.tab(tab) else { return };
        let category = match &t.content {
            TabContent::Assembly(_) => {
                let mass = assembly_mass_later(&doc, t.name.clone(), cx);
                return crate::assembly_view::open_assembly_metadata_mass_later(doc, tab, mass, window, cx);
            }
            TabContent::PartStudio(_) => "Onshape Part Studio",
            TabContent::VariableStudio(_) => "Onshape Variable Studio",
            TabContent::FeatureStudio(_) => "Onshape Feature Studio",
            _ => "Onshape File",
        };
        (t.name.clone(), category)
    };
    // Other tabs keep their metadata in the store, beside the history (the
    // keys release management reads too).
    let values = {
        let store = &doc.read(cx).session.store;
        let text = |k: &str| store.get_meta(&format!("{k}:{}", tab.0)).ok().flatten().map(|b| String::from_utf8_lossy(&b).to_string()).unwrap_or_default();
        let mut meta: noshape_model::partstudio::ItemMeta = serde_json::from_str(&text("tab_item_meta")).unwrap_or_default();
        meta.not_revision_managed = text("tab_not_revision_managed") == "1";
        noshape_app_core::properties_dialog::MetaValues { name, description: text("tab_description"), part_number: text("tab_part_number"), revision: text("tab_revision"), meta, ..Default::default() }
    };
    let spec = noshape_app_core::properties_dialog::MetaSpec { category, material: None, computed_mass: None, bom_options: None, drawing_info: None, values };
    let save = move |v: noshape_app_core::properties_dialog::MetaValues, _: &mut Window, cx: &mut App| {
            let r = doc.update(cx, |d, _| -> anyhow::Result<()> {
                let store = &mut d.session.store;
                let mut set = |k: &str, v: &str| store.set_meta(&format!("{k}:{}", tab.0), Some(v.as_bytes()).filter(|b| !b.is_empty()));
                set("tab_description", &v.description)?;
                set("tab_part_number", &v.part_number)?;
                set("tab_revision", &v.revision)?;
                set("tab_not_revision_managed", if v.meta.not_revision_managed { "1" } else { "" })?;
                set("tab_item_meta", &serde_json::to_string(&v.meta).unwrap_or_default())?;
                Ok(())
            });
            report(&doc, r, cx);
            let renamed = doc.read(cx).session.doc.tab(tab).is_some_and(|t| t.name != v.name);
            if renamed {
                doc.update(cx, |d, cx| {
                    d.edit_document(&noshape_ops::docedit::rename_tab_description(&v.name), cx, |document| {
                        if let Some(t) = document.tab_mut(tab) {
                            t.name = v.name.clone();
                        }
                    })
                });
            }
    };
    // Live 1738: only a Part Studio's modal has the left tree (its card).
    if category == "Onshape Part Studio" {
        let card = noshape_app_core::properties_dialog::ItemCard { kind: "Part Studio", thumb };
        noshape_app_core::properties_dialog::open_properties_with_card(spec, card, save, window, cx);
    } else {
        noshape_app_core::properties_dialog::open_properties(spec, save, window, cx);
    }
}

/// Tab › Move to document… (live doc-tab-menu-move-to-document): New document (its name, "Untitled document",
/// selected) | Other documents (pick one), the tabs being moved, [Move] [Cancel]. The tabs go to the other document
/// and are deleted here (op `tab_move_to_document`).
pub fn move_to_document(doc: Entity<DocModel>, tabs: Vec<noshape_model::document::TabId>, window: &mut Window, cx: &mut App) {
    struct State {
        other: bool,
        picked: Option<std::path::PathBuf>,
        /// "Move all referenced tabs" (on) and its Show details list (off).
        all_refs: bool,
        details: bool,
    }
    let path = doc.read(cx).session.path.clone();
    let docs = crate::doc_files::other_documents(&path);
    let state = Rc::new(std::cell::RefCell::new(State { other: false, picked: None, all_refs: true, details: false }));
    let name_st = cx.new(|cx| InputState::new(window, cx).default_value("Untitled document"));
    let focus = name_st.clone();
    // Live 2323: the tabs these use go along ("Move all referenced tabs"); moving a tab others use says so.
    let (names, refs, ref_names, used_elsewhere) = {
        let d = &doc.read(cx).session.doc;
        let name_of = |t: &noshape_model::document::TabId| d.tab(*t).map(|x| x.name.clone()).or_else(|| d.folder(*t).map(|f| f.name.clone()));
        let refs = noshape_ops::tab_move::referenced(d, &tabs);
        let ref_names: Vec<String> = refs.iter().filter_map(name_of).collect();
        (tabs.iter().filter_map(name_of).collect::<Vec<String>>(), refs, ref_names, noshape_ops::tab_move::used_elsewhere(d, &tabs))
    };
    window.open_dialog(cx, move |dialog, _, cx| {
        let pal = p();
        let other = state.borrow().other;
        let tab = |id: &'static str, label: &'static str, on: bool, to_other: bool| {
            let st = state.clone();
            div()
                .id(id)
                .px(px(12.))
                .py(px(8.))
                .text_size(px(14.))
                .cursor_pointer()
                .text_color(if on { pal.accent } else { pal.text })
                .when(on, |d| d.border_b_2().border_color(pal.accent))
                .map(|d| noshape_app_core::ui_script::mark(d, || format!("button:{label}")))
                .child(label)
                .on_click(move |_, window, _| {
                    st.borrow_mut().other = to_other;
                    window.refresh();
                })
        };
        let picked = state.borrow().picked.clone();
        let rows: Vec<AnyElement> = if docs.is_empty() {
            vec![os::info_box("No other documents.").into_any_element()]
        } else {
            docs.iter()
                .enumerate()
                .map(|(k, target)| {
                    let name = crate::derived::external_doc_info(target).map(|i| i.name).unwrap_or_default();
                    let (st, target) = (state.clone(), target.clone());
                    let on = picked.as_ref() == Some(&target);
                    h_flex()
                        .id(("mtd", k))
                        .h(px(32.))
                        .px(px(10.))
                        .gap(px(6.))
                        .text_size(px(14.))
                        .cursor_pointer()
                        .when(on, |d| d.bg(pal.accent_soft))
                        .when(!on, |d| d.hover(|d| d.bg(rgb(0xdef1fb))))
                        .child(os::glyph(os::GLYPH_DOCUMENT, 18., pal.text_muted))
                        .child(div().flex_1().truncate().child(crate::ui::user_text(name.clone())))
                        .map(|d| noshape_app_core::ui_script::mark(d, move || format!("list_row:{name}")))
                        .on_click(move |_, window, _| {
                            st.borrow_mut().picked = Some(target.clone());
                            window.refresh();
                        })
                        .into_any_element()
                })
                .collect()
        };
        let count = match names.len() {
            1 => "1 tab selected".to_string(),
            n => format!("{n} tabs selected"),
        };
        let (all_refs, details) = {
            let s = state.borrow();
            (s.all_refs, s.details)
        };
        let moved = v_flex().pt(px(8.)).gap(px(2.)).child(div().text_size(px(13.)).text_color(pal.text_muted).child(count)).children(names.iter().map(|n| div().pl(px(8.)).text_size(px(14.)).child(crate::ui::user_text(n.clone()))));
        let references: Option<AnyElement> = if !ref_names.is_empty() {
            let n = ref_names.len();
            let line = if n == 1 { "1 referenced tab will be moved".to_string() } else { format!("{n} referenced tabs will be moved") };
            let (st_d, st_c) = (state.clone(), state.clone());
            Some(
                v_flex()
                    .pt(px(8.))
                    .gap(px(2.))
                    .child(
                        h_flex()
                            .gap(px(6.))
                            .text_size(px(13.))
                            .child(div().text_color(pal.text_muted).child(line))
                            .child(
                                div()
                                    .id("mtd-details")
                                    .cursor_pointer()
                                    .text_color(pal.accent)
                                    .child(if details { "Hide details" } else { "Show details" })
                                    .map(|d| noshape_app_core::ui_script::mark(d, || if details { "button:Hide details".into() } else { "button:Show details".into() }))
                                    .on_click(move |_, window, _| {
                                        let d = st_d.borrow().details;
                                        st_d.borrow_mut().details = !d;
                                        window.refresh();
                                    }),
                            ),
                    )
                    .when(details, |d| d.children(ref_names.iter().map(|n| div().pl(px(8.)).text_size(px(14.)).child(crate::ui::user_text(n.clone())))))
                    .child(os::checkbox("mtd-all-refs", "Move all referenced tabs", all_refs).on_click(move |_, window, _| {
                        let on = st_c.borrow().all_refs;
                        st_c.borrow_mut().all_refs = !on;
                        window.refresh();
                    }))
                    .into_any_element(),
            )
        } else if used_elsewhere {
            Some(os::info_box("One or more tabs being moved contain workspace references. All referenced tabs will be moved as part of this operation.").mt(px(8.)).into_any_element())
        } else {
            None
        };
        let (doc, tabs, n, st, refs) = (doc.clone(), tabs.clone(), name_st.clone(), state.clone(), refs.clone());
        os::modal(dialog, "Move to document", 460.)
            .child(
                os::modal_body()
                    .child(h_flex().border_b_1().border_color(rgb(0xeeeeee)).child(tab("mtd-new", "New document", !other, false)).child(tab("mtd-other", "Other documents", other, true)))
                    .when(!other, |d| d.child(os::field_label("Document name")).child(os::input_h(os::modal_input(Some("Document name"), &name_st, cx), 32.).text_size(px(14.))))
                    .when(other, |d| d.child(v_flex().id("mtd-rows").max_h(px(300.)).overflow_y_scroll().children(rows)))
                    .child(moved)
                    // (Noted here: the block was built before the modal.)
                    .when(!ref_names.is_empty(), |d| {
                        noshape_app_core::ui_script::note_modal_field(|| serde_json::json!({ "label": "Move all referenced tabs", "kind": "checkbox", "value": all_refs }));
                        d
                    })
                    .children(references),
            )
            .footer(os::modal_footer().child(os::modal_ok(os::primary_button("ok", "Move"))).child(os::modal_cancel(os::cancel_button("cancel", "Cancel", cx))))
            .on_ok(move |_, _, cx| {
                let (to_other, picked, all_refs) = {
                    let s = st.borrow();
                    (s.other, s.picked.clone(), s.all_refs)
                };
                let mut tabs = tabs.clone();
                if all_refs {
                    tabs.extend(refs.iter().copied());
                }
                let name = n.read(cx).value().trim().to_string();
                let r = match (to_other, picked) {
                    (true, None) => return false,
                    (true, Some(target)) if crate::files::is_document_open(&target, cx) => Err(anyhow::anyhow!("Close the other document's window first")),
                    (true, Some(target)) => move_out(&doc, &tabs, noshape_ops::tab_move::Destination::Existing(&target), cx),
                    (false, _) if name.is_empty() => return false,
                    (false, _) => move_out(&doc, &tabs, noshape_ops::tab_move::Destination::New(&name), cx),
                };
                report(&doc, r, cx)
            })
    });
    window.defer(cx, move |window, cx| {
        focus.update(cx, |s, cx| {
            s.focus(window, cx);
            s.select_all(window, cx);
        })
    });
}

fn move_out(doc: &Entity<DocModel>, tabs: &[noshape_model::document::TabId], dest: noshape_ops::tab_move::Destination, cx: &mut App) -> anyhow::Result<()> {
    let (src, path) = {
        let s = &doc.read(cx).session;
        (s.doc.clone(), s.path.clone())
    };
    let to = noshape_ops::tab_move::copy_out(&src, &path, tabs, dest)?;
    let what = noshape_ops::tab_move::description(&src, tabs);
    doc.update(cx, |d, cx| d.edit_document_not_undoable(&what, cx, |document| noshape_ops::tab_move::remove(document, tabs)));
    // Live 2323: the document stays open, a bubble names what went where.
    let names: Vec<String> = tabs.iter().filter_map(|t| src.tab(*t).map(|x| x.name.clone()).or_else(|| src.folder(*t).map(|f| f.name.clone()))).collect();
    let to_name = noshape_store::Document::open_read_only(&to).ok().and_then(|d| d.meta().ok()).map(|m| m.name).unwrap_or_default();
    doc.update(cx, |_, cx| cx.emit(DocEvent::Info(noshape_ops::tab_move::moved_message(&names, &to_name))));
    Ok(())
}
