//! The document ☰ menu's dialogs (Onshape, live document/32): Rename
//! document, Move to…, Document details…, Restore deleted workspaces…, Copy
//! workspace…, Update workspace…, Workspace properties…, and the Workspace
//! units panel.

use crate::doc_model::{DocEvent, DocModel};
use crate::os_ui as os;
use crate::theme::p;
use gpui_kit::component::WindowExt as _;
use gpui_kit::component::input::{Input, InputState, Textarea, TextareaState};
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::TabContent;
use std::rc::Rc;

/// A modal with one name field (and an optional description), OK runs `then`.
fn name_modal(title: &'static str, ok: &'static str, name: String, description: Option<String>, window: &mut Window, cx: &mut App, then: impl Fn(String, Option<String>, &mut Window, &mut App) -> bool + 'static) {
    let name_st = cx.new(|cx| InputState::new(window, cx).default_value(name));
    let desc_st = description.map(|d| cx.new(|cx| TextareaState::new(window, cx).rows(4).placeholder("Description").default_value(d)));
    let then = Rc::new(then);
    let focus = name_st.clone();
    window.open_dialog(cx, move |dialog, _, cx| {
        let (n, d, then) = (name_st.clone(), desc_st.clone(), then.clone());
        os::modal(dialog, title, 460.)
            .child(
                os::modal_body()
                    .child(os::field_label("Name"))
                    .child(os::input_h(Input::new(&name_st), 32.).text_size(px(14.)))
                    .when_some(desc_st.clone(), |b, d| b.child(os::field_label("Description")).child(Textarea::new(&d).h(px(96.)).text_size(px(14.)))),
            )
            .footer(os::modal_footer().child(os::modal_ok(os::primary_button("ok", ok))).child(os::modal_cancel(os::cancel_button("cancel", "Cancel", cx))))
            .on_ok(move |_, window, cx| {
                let name = n.read(cx).value().trim().to_string();
                let desc = d.as_ref().map(|d| d.read(cx).value().to_string());
                !name.is_empty() && then(name, desc, window, cx)
            })
    });
    window.defer(cx, move |window, cx| {
        focus.update(cx, |s, cx| {
            s.focus(window, cx);
            s.select_all(window, cx);
        })
    });
}

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

/// ☰ › Rename document.
pub fn rename_document(doc: Entity<DocModel>, window: &mut Window, cx: &mut App) {
    let name = doc.read(cx).name();
    name_modal("Rename document", "Rename", name, None, window, cx, move |name, _, _, cx| {
        let r = doc.update(cx, |d, _| d.session.store.set_name(&name).map_err(Into::into));
        report(&doc, r, cx)
    });
}

/// ☰ › Document details…: name and description (stored with the document),
/// owner and history stamps.
pub fn document_details(doc: Entity<DocModel>, window: &mut Window, cx: &mut App) {
    let path = doc.read(cx).session.path.clone();
    let details = crate::start_library::doc_details(&path).unwrap_or_default();
    let name = doc.read(cx).name();
    let name_st = cx.new(|cx| InputState::new(window, cx).default_value(name));
    let desc_st = cx.new(|cx| TextareaState::new(window, cx).rows(4).placeholder("Description").default_value(details.description.clone()));
    window.open_dialog(cx, move |dialog, _, cx| {
        let pal = p();
        let (n, d, doc) = (name_st.clone(), desc_st.clone(), doc.clone());
        let stamp = |label: &'static str, who: &str, at: i64| {
            v_flex()
                .pt(px(6.))
                .child(os::field_label(label))
                .child(div().text_size(px(13.33)).text_color(pal.text).child(format!("{who} · {}", crate::start_library::format_onshape_time(at))))
        };
        os::modal(dialog, "Document details", 520.)
            .child(
                os::modal_body()
                    .child(os::field_label("Name"))
                    .child(os::input_h(Input::new(&name_st), 32.).text_size(px(14.)))
                    .child(os::field_label("Description"))
                    .child(Textarea::new(&desc_st).h(px(96.)).text_size(px(14.)))
                    .child(v_flex().pt(px(6.)).child(os::field_label("Owner")).child(div().text_size(px(13.33)).child("me")))
                    .child(stamp("Created by", &details.created_by, details.created))
                    .child(stamp("Last modified by", &details.modified_by, details.modified)),
            )
            .footer(os::modal_footer().child(os::modal_ok(os::primary_button("ok", "Save"))).child(os::modal_cancel(os::cancel_button("cancel", "Cancel", cx))))
            .on_ok(move |_, _, cx| {
                let name = n.read(cx).value().trim().to_string();
                let desc = d.read(cx).value().to_string();
                if name.is_empty() {
                    return false;
                }
                let r = doc.update(cx, |dm, _| -> anyhow::Result<()> {
                    dm.session.store.set_name(&name)?;
                    dm.session.store.set_description(&desc)?;
                    Ok(())
                });
                report(&doc, r, cx)
            })
    });
}

/// ☰ › Restore deleted workspaces…: deleted branches with a Restore button.
pub fn restore_deleted_workspaces(doc: Entity<DocModel>, window: &mut Window, cx: &mut App) {
    window.open_dialog(cx, move |dialog, _, cx| {
        let pal = p();
        let deleted: Vec<noshape_store::BranchInfo> = doc.read(cx).session.store.graph().map(|g| g.branches.into_iter().filter(|b| b.deleted).collect()).unwrap_or_default();
        let rows: Vec<AnyElement> = if deleted.is_empty() {
            vec![os::info_box("No deleted workspaces found in this document.").into_any_element()]
        } else {
            deleted
                .into_iter()
                .map(|b| {
                    let (doc, id) = (doc.clone(), b.id);
                    h_flex()
                        .h(px(36.))
                        .gap(px(8.))
                        .child(div().flex_1().text_size(px(14.)).text_color(pal.text).child(crate::ui::user_text(b.name.clone())))
                        .child(div().text_size(px(13.)).text_color(pal.text_muted).child(crate::start_library::format_onshape_time(b.created)))
                        .child(os::secondary_button(SharedString::from(format!("restore-{}", id.0)), "Restore", cx).on_click(move |_, window, cx| {
                            let r = doc.update(cx, |d, _| d.session.store.restore_branch(id).map_err(Into::into));
                            report(&doc, r, cx);
                            window.refresh();
                        }))
                        .into_any_element()
                })
                .collect()
        };
        os::modal(dialog, "Restore deleted workspaces", 460.)
            .child(os::modal_body().children(rows))
            .footer(os::modal_footer().child(os::modal_cancel(os::cancel_button("close", "Close", cx))))
    });
}

/// ☰ › Copy workspace…: a new document from this workspace's state, opened.
pub fn copy_workspace(doc: Entity<DocModel>, window: &mut Window, cx: &mut App) {
    let (path, name) = {
        let d = doc.read(cx);
        (d.session.path.clone(), d.name())
    };
    name_modal("Copy workspace", "Copy", format!("Copy of {name}"), None, window, cx, move |new_name, _, window, cx| {
        let _ = doc.update(cx, |d, _| d.session.store.checkpoint());
        match crate::start_library::copy_document_named(&path, &new_name) {
            Ok(dest) => {
                let gpu = cx.global::<crate::gpu::GpuGlobal>().0.clone();
                if let Err(e) = crate::files::open_document_window(dest, None, gpu, cx) {
                    doc.update(cx, |_, cx| cx.emit(DocEvent::Error(format!("{e:#}"))));
                }
                let _ = window;
                true
            }
            Err(e) => {
                doc.update(cx, |_, cx| cx.emit(DocEvent::Error(format!("{e:#}"))));
                false
            }
        }
    });
}

/// ☰ › Workspace properties…: the workspace's (branch's) name and
/// description.
pub fn workspace_properties(doc: Entity<DocModel>, window: &mut Window, cx: &mut App) {
    let (branch, name, desc) = {
        let s = &doc.read(cx).session;
        let name = s.store.branch(s.branch).map(|b| b.name).unwrap_or_else(|_| "Main".into());
        let desc = s.store.get_meta(&format!("workspace_description:{}", s.branch.0)).ok().flatten().map(|b| String::from_utf8_lossy(&b).to_string()).unwrap_or_default();
        (s.branch, name, desc)
    };
    name_modal("Workspace properties", "Save", name, Some(desc), window, cx, move |name, desc, _, cx| {
        let r = doc.update(cx, |d, _| -> anyhow::Result<()> {
            d.session.store.rename_branch(branch, &name)?;
            d.session.store.set_meta(&format!("workspace_description:{}", branch.0), desc.as_deref().map(str::as_bytes))?;
            Ok(())
        });
        report(&doc, r, cx)
    });
}

/// ☰ › Move to…: the folder browser; the document closes, moves and reopens.
pub fn move_to(doc: Entity<DocModel>, window: &mut Window, cx: &mut App) {
    let path = doc.read(cx).session.path.clone();
    let start = path.parent().map(|p| p.to_path_buf()).unwrap_or_else(crate::files::documents_dir);
    let location = Rc::new(crate::start_window::Location::new(start));
    window.open_dialog(cx, move |dialog, _, cx| {
        let (loc, path) = (location.clone(), path.clone());
        os::modal(dialog, "Move to", 460.)
            .child(os::modal_body().child(os::field_label("Location")).child(location.render()))
            .footer(os::modal_footer().child(os::modal_ok(os::primary_button("ok", "Move"))).child(os::modal_cancel(os::cancel_button("cancel", "Cancel", cx))))
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

/// ☰ › Update workspace…: references to other documents with newer
/// versions, updated together.
pub fn update_workspace(doc: Entity<DocModel>, window: &mut Window, cx: &mut App) {
    let outdated = outdated_references(&doc.read(cx).session.doc);
    window.open_dialog(cx, move |dialog, _, cx| {
        let pal = p();
        let rows: Vec<AnyElement> = if outdated.is_empty() {
            vec![os::info_box("All references to other documents are up to date.").into_any_element()]
        } else {
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
                })
                .collect()
        };
        let (doc, list) = (doc.clone(), outdated.clone());
        let can = !list.is_empty();
        os::modal(dialog, "Update workspace", 560.)
            .child(os::modal_body().children(rows))
            .footer(
                os::modal_footer()
                    .when(can, |f| f.child(os::modal_ok(os::primary_button("ok", "Update"))))
                    .child(os::modal_cancel(os::cancel_button("cancel", if can { "Cancel" } else { "Close" }, cx))),
            )
            .on_ok(move |_, _, cx| {
                let list = list.clone();
                doc.update(cx, |d, cx| d.edit_document("Update workspace", cx, |document| list.iter().for_each(|o| apply_update(o, document))));
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

/// Display decimals options ("0", "0.1", … "0.12345678").
pub(crate) const DECIMALS: [&str; 9] = ["0", "0.1", "0.12", "0.123", "0.1234", "0.12345", "0.123456", "0.1234567", "0.12345678"];
/// How many [`DECIMALS`] options each of `OTHER_UNITS` offers (live).
pub(crate) const OTHER_DECIMALS: [usize; 9] = [7, 7, 8, 9, 9, 9, 8, 9, 9];

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
    fn select(&self, id: SharedString, w: f32, options: Vec<String>, current: usize, set: impl Fn(&mut noshape_units::UnitPrefs, usize) + 'static, cx: &mut Context<Self>) -> AnyElement {
        use gpui_kit::component::button::{Button, ButtonCustomVariant, ButtonVariants as _};
        use crate::os_ui::OsDropdownExt as _;
        let pal = p();
        let view = cx.entity();
        let set = Rc::new(set);
        let plain = ButtonCustomVariant::new(cx).color(gpui_kit::transparent_white()).foreground(pal.text.into()).hover(pal.panel_hover.into()).active(pal.panel_active.into()).shadow(false);
        let label = options.get(current).cloned().unwrap_or_default();
        let id2 = id.clone();
        div()
            .w(px(w))
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
                    .child(h_flex().w(px(w - 12.)).child(div().flex_1().text_size(px(13.33)).text_color(pal.text).child(label)).child(os::glyph(os::GLYPH_CARET, 7., pal.text)))
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
        // Length and angle (the units expressions use), in Onshape's lists.
        let lu = LENGTH_UNITS_SHOWN.iter().position(|u| *u == d.length).unwrap_or(0);
        let (dl, dc) = dec(d.length_precision, 6);
        let unit = self.select("wu-length".into(), 210., LENGTH_UNITS_SHOWN.iter().map(|u| u.name().to_string()).collect(), lu, |p, i| p.length = LENGTH_UNITS_SHOWN[i], cx);
        let decimals = self.select("wu-length-dec".into(), 100., dl, dc, |p, i| p.length_precision = i as u8, cx);
        groups[0].push(self.row("wu-row-length", "Length default unit".into(), unit, decimals));
        let au = AngleUnit::ALL.iter().position(|u| *u == d.angle).unwrap_or(0);
        let (al, ac) = dec(d.angle_precision, 7);
        let unit = self.select("wu-angle".into(), 210., AngleUnit::ALL.iter().map(|u| u.name().to_string()).collect(), au, |p, i| p.angle = AngleUnit::ALL[i], cx);
        let decimals = self.select("wu-angle-dec".into(), 100., al, ac, |p, i| p.angle_precision = i as u8, cx);
        groups[1].push(self.row("wu-row-angle", "Angle default unit".into(), unit, decimals));
        for (k, (label, group, names, _, _)) in OTHER_UNITS.iter().enumerate() {
            let (dl, dc) = dec(d.other[k].1, OTHER_DECIMALS[k]);
            let shown = OTHER_UNITS_SHOWN[k];
            let at = shown.iter().position(|&i| i == d.other[k].0).unwrap_or(0);
            let unit = self.select(SharedString::from(format!("wu-{k}")), 210., shown.iter().map(|&i| names[i as usize].to_string()).collect(), at, move |p, i| p.other[k].0 = shown[i], cx);
            let decimals = self.select(SharedString::from(format!("wu-{k}-dec")), 100., dl, dc, move |p, i| p.other[k].1 = i as u8, cx);
            let key: &'static str = ["wu-r0", "wu-r1", "wu-r2", "wu-r3", "wu-r4", "wu-r5", "wu-r6", "wu-r7", "wu-r8"][k];
            groups[*group as usize].push(self.row(key, format!("{label} default unit"), unit, decimals));
        }
        // Onshape's order inside a group: the base quantity first.
        let titles = ["Length", "Angle", "Mechanical"];
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

/// Tab › Properties…: the element's name and description.
pub fn tab_properties(doc: Entity<DocModel>, tab: noshape_model::document::TabId, window: &mut Window, cx: &mut App) {
    let (name, desc) = {
        let s = &doc.read(cx).session;
        let name = s.doc.tab(tab).map(|t| t.name.clone()).unwrap_or_default();
        let desc = s.store.get_meta(&format!("tab_description:{}", tab.0)).ok().flatten().map(|b| String::from_utf8_lossy(&b).to_string()).unwrap_or_default();
        (name, desc)
    };
    name_modal("Properties", "Save", name, Some(desc), window, cx, move |name, desc, _, cx| {
        let ok = doc.update(cx, |d, _| d.session.store.set_meta(&format!("tab_description:{}", tab.0), desc.as_deref().map(str::as_bytes)));
        doc.update(cx, |d, cx| {
            d.edit_document(&format!("Rename tab : {name}"), cx, |document| {
                if let Some(t) = document.tab_mut(tab) {
                    t.name = name.clone();
                }
            })
        });
        report(&doc, ok.map_err(Into::into), cx)
    });
}

/// Tab › Show code: the element as FeatureScript-style code (read-only).
pub fn show_code(doc: Entity<DocModel>, tab: noshape_model::document::TabId, window: &mut Window, cx: &mut App) {
    let (title, code) = {
        let d = &doc.read(cx).session.doc;
        let t = d.tab(tab);
        (t.map(|t| t.name.clone()).unwrap_or_default(), t.map(|t| element_code(&t.name, &t.content)).unwrap_or_default())
    };
    let st = cx.new(|cx| TextareaState::new(window, cx).rows(30).default_value(code));
    window.open_dialog(cx, move |dialog, _, cx| {
        os::modal(dialog, title.clone(), 820.)
            .child(os::modal_body().child(div().font_family("monospace").child(Textarea::new(&st).h(px(560.)).text_size(px(13.)))))
            .footer(os::modal_footer().child(os::modal_cancel(os::cancel_button("close", "Close", cx))))
    });
}

/// FeatureScript-style text of a Part Studio: one call per feature with its
/// parameters.
fn element_code(name: &str, content: &TabContent) -> String {
    let mut out = String::from("FeatureScript 2384;\nimport(path : \"onshape/std/common.fs\", version : \"2384.0\");\n\n");
    let TabContent::PartStudio(ps) = content else { return out };
    let ident: String = name.chars().filter(|c| c.is_alphanumeric()).collect();
    out.push_str(&format!("export const {} = defineFeature(function(context is Context, id is Id, definition is map)\n    precondition\n    {{\n    }}\n    {{\n", if ident.is_empty() { "partStudio".into() } else { format!("{}{}", ident[..1].to_lowercase(), &ident[1..]) }));
    for f in &ps.features {
        let params = serde_json::to_value(&f.params).unwrap_or_default();
        let fields: Vec<String> = match params {
            serde_json::Value::Object(m) => m.into_iter().map(|(k, v)| format!("\"{k}\" : {v}")).collect(),
            other => vec![other.to_string()],
        };
        let call = if f.suppressed { "// suppressed: " } else { "" };
        out.push_str(&format!("        {call}{}(context, id + \"{}\", {{ {} }});\n", f.type_id, f.id.0, fields.join(", ")));
    }
    out.push_str("    });\n");
    out
}

/// Tab › Move to document…: another document receives the element (as a new
/// tab) and it is deleted here.
pub fn move_to_document(doc: Entity<DocModel>, tabs: Vec<noshape_model::document::TabId>, window: &mut Window, cx: &mut App) {
    let path = doc.read(cx).session.path.clone();
    let docs = crate::doc_files::other_documents(&path);
    window.open_dialog(cx, move |dialog, _, cx| {
        let pal = p();
        let rows: Vec<AnyElement> = if docs.is_empty() {
            vec![os::info_box("No other documents.").into_any_element()]
        } else {
            docs.iter()
                .enumerate()
                .map(|(k, target)| {
                    let name = crate::derived::external_doc_info(target).map(|i| i.name).unwrap_or_default();
                    let (doc, target, tabs) = (doc.clone(), target.clone(), tabs.clone());
                    h_flex()
                        .id(("mtd", k))
                        .h(px(32.))
                        .px(px(10.))
                        .gap(px(6.))
                        .text_size(px(14.))
                        .cursor_pointer()
                        .hover(|d| d.bg(rgb(0xdef1fb)))
                        .child(os::glyph(os::GLYPH_DOCUMENT, 18., pal.text_muted))
                        .child(div().flex_1().truncate().child(crate::ui::user_text(name)))
                        .on_click(move |_, window, cx| {
                            let r = tabs.iter().try_for_each(|t| move_tab_into(&doc, *t, &target, cx));
                            if report(&doc, r, cx) {
                                window.close_dialog(cx);
                            }
                        })
                        .into_any_element()
                })
                .collect()
        };
        os::modal(dialog, "Move to document", 460.)
            .child(os::modal_body().child(os::field_label("Document")).child(v_flex().id("mtd-rows").max_h(px(360.)).overflow_y_scroll().children(rows)))
            .footer(os::modal_footer().child(os::modal_cancel(os::cancel_button("cancel", "Cancel", cx))))
    });
}

/// Move a tab, or a folder with its contents (Onshape: "tabs and folders
/// within the folder remain within it"), into the other document's main
/// workspace.
fn move_tab_into(doc: &Entity<DocModel>, tab: noshape_model::document::TabId, target: &std::path::Path, cx: &mut App) -> anyhow::Result<()> {
    use noshape_model::document::TabId;
    anyhow::ensure!(!crate::files::is_document_open(target, cx), "Close the other document's window first");
    let src = doc.read(cx).session.doc.clone();
    let label = match (src.tab(tab), src.folder(tab)) {
        (Some(t), _) => t.name.clone(),
        (None, Some(f)) => f.name.clone(),
        _ => return Ok(()),
    };
    let mut store = noshape_store::Document::open(target)?;
    let branch = store.default_branch()?;
    let head = store.head(branch)?;
    let mut other = noshape_model::Document::from_blobs(&store.read_all(head)?)?;
    let mut added = Vec::new();
    // Copy `id` (tab or folder) into `parent` of the other document.
    fn copy(src: &noshape_model::Document, other: &mut noshape_model::Document, id: TabId, parent: Option<TabId>, added: &mut Vec<TabId>) {
        if let Some(t) = src.tab(id) {
            let name = if other.tabs.iter().any(|x| x.name == t.name) { other.next_tab_name(&t.name) } else { t.name.clone() };
            let new = other.add_tab(&name, t.content.clone());
            other.set_parent(new, parent);
            added.push(new);
        } else if let Some(f) = src.folder(id) {
            let new = other.add_folder(&f.name, parent);
            for c in src.children(Some(id)) {
                copy(src, other, c, Some(new), added);
            }
        }
    }
    copy(&src, &mut other, tab, None, &mut added);
    let mut changes: Vec<(String, Option<Vec<u8>>)> = added.iter().map(|id| (noshape_model::Document::blob_name(*id), other.tab_blob(*id))).collect();
    changes.push((noshape_model::document::META_BLOB.to_string(), Some(other.settings_blob())));
    store.commit(branch, &changes, &format!("Insert tab : {label}"))?;
    let _ = store.checkpoint();
    doc.update(cx, |d, cx| {
        d.edit_document_not_undoable(&format!("Move tab to document : {label}"), cx, |document| {
            if document.is_folder(tab) {
                document.delete_folder_tree(tab);
            } else {
                document.remove_tab(tab);
            }
        })
    });
    Ok(())
}
