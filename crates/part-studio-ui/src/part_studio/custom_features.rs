//! Add custom features (live behaviours.md "psui (2213)", help "Custom features"): the toolbar's ⊞ opens a 300 ×
//! 581 select-item panel right-aligned under the toolbar (Current document | Other documents, on Other documents with
//! its search box focused). A click on a feature of a document version adds it to the user's custom features at once
//! (an account setting: every document's toolbar), a second click takes it off again; a Feature Studio's row adds all
//! its features. The panel stays open until its ✕ (Escape and clicks elsewhere leave it). With features added, ⊞ is
//! a split tool: its ▾ lists them alphabetically (right-click › Remove), then Add custom features; no button of their
//! own (lead's live check 11-09: Onshape's help is out of date there). Features the Part Studio uses that aren't on
//! the toolbar: "Linked custom features in this Part Studio" ▾.

use super::PartStudioView;
use crate::select_item::{PanelMode, PanelSetup, SelectItemEvent, SelectItemPanel};
use crate::os_menu::OsContextMenuExt as _;
use gpui_kit::component::h_flex;
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_ops::prefs::CustomTool;

/// The user's custom features changed (added, removed): every open Part Studio reloads them (an account setting).
#[derive(Default)]
pub struct MyCustomFeaturesRev(pub u64);

impl Global for MyCustomFeaturesRev {}

/// The open panel.
pub struct AddFeatures {
    pub(super) panel: Entity<SelectItemPanel>,
    _sub: Subscription,
}

impl PartStudioView {
    /// ⊞: open the panel (a second click leaves it as it is).
    pub(super) fn open_add_custom_features(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.ui.add_features.is_some() {
            return;
        }
        let setup = PanelSetup {
            mode: PanelMode::CustomFeatures,
            sources: Vec::new(),
            doc_name: self.doc.read(cx).name(),
            doc_path: Some(self.doc.read(cx).session.path.clone()),
            except: String::new(),
            gpu: None,
            value: String::new(),
            libraries: false,
            frame_profiles: false,
        };
        let panel = cx.new(|cx| SelectItemPanel::new(setup, window, cx));
        let sub = cx.subscribe_in(&panel, window, |this: &mut Self, _, ev: &SelectItemEvent, window, cx| match ev {
            SelectItemEvent::Picked(type_id) => {
                let version = this.ui.add_features.as_ref().and_then(|a| a.panel.read(cx).version_phrase());
                this.toggle_my_feature(type_id, version, cx)
            }
            SelectItemEvent::PickedAll(ids) => {
                let version = this.ui.add_features.as_ref().and_then(|a| a.panel.read(cx).version_phrase());
                this.add_all_my_features(ids, version, cx)
            }
            SelectItemEvent::Close => {
                this.ui.add_features = None;
                // Its "was added to your toolbar" toast goes with it (live fs-ps-acf-click-adds).
                if this.message.as_ref().is_some_and(|(_, m)| m.starts_with("Custom feature \"")) {
                    this.message = None;
                }
                cx.notify();
            }
            SelectItemEvent::CreateVersion(None) => crate::part_studio_hooks::create_version(this.doc.clone(), window, cx),
            SelectItemEvent::CreateVersion(Some(path)) => {
                let _ = crate::part_studio_hooks::open_document_window(path.clone(), this.gpu.clone(), cx);
            }
            SelectItemEvent::ImportFile | SelectItemEvent::Confirm | SelectItemEvent::LibraryImage(_) => {}
        });
        self.ui.add_features = Some(AddFeatures { panel, _sub: sub });
        self.sync_added(cx);
        cx.notify();
    }

    /// The panel shows the features on the toolbar as added.
    fn sync_added(&mut self, cx: &mut Context<Self>) {
        let added: std::collections::HashSet<String> = self.ui.toolbar.custom_features.iter().map(|f| f.type_id.clone()).collect();
        if let Some(a) = &self.ui.add_features {
            a.panel.update(cx, |p, cx| p.set_added(added, cx));
        }
    }

    /// The user's custom features changed here: saved (an account setting), and every Part Studio told.
    fn my_features_changed(&mut self, cx: &mut Context<Self>) {
        let list = &mut self.ui.toolbar.custom_features;
        // Onshape lists custom features alphabetically (help "Custom features").
        list.sort_by_key(|f| f.name.to_lowercase());
        self.ui.toolbar.save();
        let rev = &mut cx.default_global::<MyCustomFeaturesRev>().0;
        *rev += 1;
        self.ui.my_custom_rev = *rev;
        self.sync_added(cx);
        cx.refresh_windows();
    }

    /// Another Part Studio changed the user's custom features: this one reloads them.
    pub(super) fn reload_my_features_if_changed(&mut self, cx: &mut Context<Self>) {
        let rev = cx.try_global::<MyCustomFeaturesRev>().map_or(0, |r| r.0);
        if rev != self.ui.my_custom_rev {
            self.ui.my_custom_rev = rev;
            self.ui.toolbar.custom_features = super::toolbar_prefs::ToolbarPrefs::load().custom_features;
            self.sync_added(cx);
        }
    }

    /// A feature row clicked in the panel: added (with live's toast), or, already on the toolbar, taken off
    /// ("This toggles the custom feature in and out of the toolbar", help "Custom features").
    pub(super) fn toggle_my_feature(&mut self, type_id: &str, version: Option<String>, cx: &mut Context<Self>) {
        if self.ui.toolbar.custom_features.iter().any(|f| f.type_id == type_id) {
            self.remove_my_feature(type_id, cx);
            return;
        }
        self.add_my_feature(type_id, version, cx);
    }

    /// A Feature Studio's row: all its features added ("each represented by its own icon on your toolbar"), or,
    /// all already on it, all taken off.
    pub(super) fn add_all_my_features(&mut self, ids: &[String], version: Option<String>, cx: &mut Context<Self>) {
        let on: Vec<&String> = ids.iter().filter(|t| self.ui.toolbar.custom_features.iter().any(|f| f.type_id == **t)).collect();
        if on.len() == ids.len() {
            for t in ids {
                self.remove_my_feature(t, cx);
            }
            return;
        }
        for t in ids {
            self.add_my_feature(t, version.clone(), cx);
        }
    }

    /// A feature picked in the panel joins the user's custom features (once), with live's green toast (2361:
    /// "Custom feature "uip Feature" was added to your toolbar from version V1.").
    fn add_my_feature(&mut self, type_id: &str, version: Option<String>, cx: &mut Context<Self>) {
        let list = &mut self.ui.toolbar.custom_features;
        if list.iter().all(|f| f.type_id != type_id) {
            let Some(t) = noshape_model::features::get(type_id) else { return };
            list.push(CustomTool { type_id: type_id.to_string(), name: t.name.to_string() });
            self.my_features_changed(cx);
            // `version`: "version V1", "published version V4".
            let from = version.map(|v| format!(" from {v}")).unwrap_or_default();
            self.flash_success(format!("Custom feature \"{}\" was added to your toolbar{from}.", t.name), cx);
        }
        cx.notify();
    }

    /// Remove: an added custom feature off the toolbar, at once (live 2213); its "was added" toast goes.
    fn remove_my_feature(&mut self, type_id: &str, cx: &mut Context<Self>) {
        self.ui.toolbar.custom_features.retain(|f| f.type_id != type_id);
        self.ui.toolbar_faces.remove(ADD_CUSTOM_FACE);
        if self.message.as_ref().is_some_and(|(_, m)| m.starts_with("Custom feature \"")) {
            self.message = None;
        }
        self.my_features_changed(cx);
        cx.notify();
    }

    /// "Linked custom features in this Part Studio" ▾ (live's toolbar `part-studio-linked-feature-tools`, icon
    /// external-fs-feature, before Custom features in this workspace): the Part Studio's features from other
    /// documents or versions that aren't on the user's toolbar (removed, or a shared document's), so they stay
    /// usable; alphabetical. None when there are none.
    pub(super) fn linked_custom_tools(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let mine: std::collections::HashSet<&str> = self.ui.toolbar.custom_features.iter().map(|f| f.type_id.as_str()).collect();
        let studio = self.studio(cx)?;
        let mut linked: Vec<(String, String)> = Vec::new();
        for f in &studio.features {
            let Some((key, _)) = noshape_model::custom::parse_type_id(&f.type_id) else { continue };
            if !key.starts_with("ext:") || mine.contains(f.type_id.as_str()) || linked.iter().any(|(t, _)| *t == f.type_id) {
                continue;
            }
            let Some(t) = noshape_model::features::get(&f.type_id) else { continue };
            linked.push((f.type_id.clone(), t.name.to_string()));
        }
        if linked.is_empty() {
            return None;
        }
        linked.sort_by_key(|(_, n)| n.to_lowercase());
        let view = cx.entity();
        let button: std::rc::Rc<std::cell::Cell<Bounds<Pixels>>> = Default::default();
        let b2 = button.clone();
        Some(
            crate::ui::tool_button("tb-linked-custom-features", "linked-custom-features", false, cx)
                .w(px(crate::ui::TOOL_CELL + crate::ui::TOOL_CARET_W))
                .child(crate::ui::glyph("tool-caret", 10., crate::theme::p().text))
                .child(canvas(move |b, _, _| b2.set(b), |_, _, _, _| {}).absolute().size_full())
                .tooltip("Linked custom features in this Part Studio")
                .map(|d| crate::ui_script::mark(d, || "button:Linked custom features in this Part Studio".into()))
                .on_click(move |_: &ClickEvent, window, cx| {
                    let v = view.clone();
                    let b = button.get();
                    let linked = linked.clone();
                    let menu = crate::os_menu::OsMenu::build(window, cx, move |m, _, _| {
                        let mut m = m.dropdown().min_width(178.);
                        for (id, name) in &linked {
                            let (v, id) = (v.clone(), id.clone());
                            m = m.item(crate::os_menu::OsMenuItem::new(name.clone()).user().initials(name).on_click(move |_, window, cx| v.update(cx, |this, cx| this.start_feature(&id, window, cx))));
                        }
                        m
                    });
                    crate::os_menu::open_anchored(point(b.left(), b.bottom() + px(4.)), Anchor::TopLeft, menu, window, cx);
                })
                .into_any_element(),
        )
    }

    /// The panel, right-aligned under the toolbar over the graphics.
    pub(super) fn render_add_custom_features(&self) -> Option<AnyElement> {
        let a = self.ui.add_features.as_ref()?;
        Some(div().absolute().top(px(3.)).right_0().child(a.panel.clone()).into_any_element())
    }

    /// ⊞ "Add custom features"; with the user's custom features a split tool: its face the one last picked from
    /// the ▾ (⊞ until then: live showed it after the panel's adds), the ▾ the features and then Add custom features.
    pub(super) fn add_custom_features_button(&mut self, cx: &mut Context<Self>) -> AnyElement {
        self.reload_my_features_if_changed(cx);
        let mine = self.ui.toolbar.custom_features.clone();
        let face = self.ui.toolbar_faces.get(ADD_CUSTOM_FACE).and_then(|id| mine.iter().find(|f| f.type_id == *id)).cloned();
        let open = self.ui.add_features.is_some();
        let main = match &face {
            Some(f) => {
                let id = f.type_id.clone();
                let btn = crate::ui::split_tool_button_with("tb-add-custom", crate::ui::initials_icon(&f.name), false, cx).on_click(cx.listener(move |this, _: &ClickEvent, window, cx| this.start_feature(&id, window, cx)));
                let (name, id) = (f.name.clone(), f.type_id.clone());
                let view = cx.entity();
                let btn = btn.os_context_menu(move |m, _, _| my_feature_menu(m, view.clone(), id.clone()));
                crate::ui_script::wrap(crate::ui::with_tooltip("tt-add-custom", name.clone(), btn).into_any_element(), move || format!("tool:{name}"))
            }
            None => {
                let btn = if mine.is_empty() { crate::ui::tool_button("tb-add-custom", "add-custom-features", open, cx) } else { crate::ui::split_tool_button("tb-add-custom", "add-custom-features", open, cx) };
                let btn = btn.on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.open_add_custom_features(window, cx)));
                crate::ui_script::wrap(crate::ui::with_tooltip("tt-add-custom", "Add custom features", btn).into_any_element(), || "tool:Add custom features".into())
            }
        };
        if mine.is_empty() {
            return main;
        }
        let view = cx.entity();
        crate::ui::tool_group(
            "tg-add-custom",
            main,
            crate::ui::tool_caret("tbm-add-custom", cx)
                .dropdown_menu(move |mut menu, window, _| {
                    menu = crate::ui::fit_tool_menu(menu.min_w(px(180.)), window);
                    for f in mine.iter() {
                        let v = view.clone();
                        let (id, name) = (f.type_id.clone(), f.name.clone());
                        let (rv, rid) = (view.clone(), f.type_id.clone());
                        menu = menu.item(
                            PopupMenuItem::element(move |_, _| {
                                let n = name.clone();
                                let (rv, rid) = (rv.clone(), rid.clone());
                                // Right-click › Remove (lead's live check 11-09), as on the face.
                                let row = h_flex()
                                    .id(SharedString::from(format!("my-custom-{rid}")))
                                    .h(px(32.))
                                    .flex_1()
                                    .gap(px(4.))
                                    .child(div().w(px(30.)).flex_none().flex().items_center().justify_center().child(crate::ui::initials_icon(&name)))
                                    .child(div().flex_1().text_size(px(14.)).child(name.clone()))
                                    .os_context_menu(move |m, _, _| my_feature_menu(m, rv.clone(), rid.clone()));
                                crate::ui_script::mark(row, move || format!("menu:{n}"))
                            })
                                .on_click(move |_, window, cx| {
                                    v.update(cx, |this, cx| {
                                        if let Some(t) = noshape_model::features::get(&id) {
                                            this.ui.toolbar_faces.insert(ADD_CUSTOM_FACE, t.id);
                                        }
                                        this.start_feature(&id, window, cx)
                                    })
                                }),
                        );
                    }
                    let v = view.clone();
                    menu.item(crate::ui::menu_tool_item("Add custom features", "add-custom-features", None).on_click(move |_, window, cx| {
                        v.update(cx, |this, cx| {
                            this.ui.toolbar_faces.remove(ADD_CUSTOM_FACE);
                            this.open_add_custom_features(window, cx)
                        })
                    }))
                })
                .map(|c| crate::ui_script::wrap(c.into_any_element(), || "caret:Add custom features".into())),
        )
        .into_any_element()
    }
}

/// The toolbar faces' key of the ⊞ group.
const ADD_CUSTOM_FACE: &str = "add-custom";

/// An added feature's right-click menu (live 2213): Open linked document (the version it comes from), Remove (off
/// the toolbar at once, no prompt).
fn my_feature_menu(m: crate::os_menu::OsMenu, view: Entity<PartStudioView>, type_id: String) -> crate::os_menu::OsMenu {
    let linked = noshape_model::custom::parse_type_id(&type_id).and_then(|(studio, _)| crate::derived::ExternalRef::parse(studio));
    let v = view.clone();
    let id = type_id.clone();
    m.list_no_icons()
        .item(crate::os_menu::OsMenuItem::new("Open linked document").disabled(linked.is_none()).on_click(move |_, _, cx| {
            let Some(r) = linked.clone() else { return };
            v.update(cx, |this, cx| {
                let _ = crate::part_studio_hooks::open_document_window(r.path.clone(), this.gpu.clone(), cx);
            })
        }))
        .item(crate::os_menu::OsMenuItem::new("Remove").on_click(move |_, _, cx| view.update(cx, |this, cx| this.remove_my_feature(&id, cx))))
}
