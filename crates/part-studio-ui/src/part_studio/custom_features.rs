//! Add custom features (live behaviours.md "psui (2213)"): the toolbar's ⊞ opens a 300 × 581 select-item panel
//! right-aligned under the toolbar (Current document | Other documents, on Other documents with its search box
//! focused). A click on a feature of a document version adds it to the user's custom features at once, in every
//! document's toolbar; the panel stays open until its ✕ (Escape and clicks elsewhere leave it). With features added,
//! ⊞ is a split tool: its ▾ lists them, then Add custom features.

use super::PartStudioView;
use crate::select_item::{PanelMode, PanelSetup, SelectItemEvent, SelectItemPanel};
use crate::os_menu::OsContextMenuExt as _;
use gpui_kit::component::h_flex;
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_ops::prefs::CustomTool;

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
            SelectItemEvent::Picked(type_id) => this.add_my_feature(type_id, cx),
            SelectItemEvent::Close => {
                this.ui.add_features = None;
                cx.notify();
            }
            SelectItemEvent::CreateVersion(None) => crate::part_studio_hooks::create_version(this.doc.clone(), window, cx),
            SelectItemEvent::CreateVersion(Some(path)) => {
                let _ = crate::part_studio_hooks::open_document_window(path.clone(), this.gpu.clone(), cx);
            }
            SelectItemEvent::ImportFile | SelectItemEvent::Confirm | SelectItemEvent::LibraryImage(_) => {}
        });
        self.ui.add_features = Some(AddFeatures { panel, _sub: sub });
        cx.notify();
    }

    /// A feature picked in the panel joins the user's custom features (once).
    fn add_my_feature(&mut self, type_id: &str, cx: &mut Context<Self>) {
        let list = &mut self.ui.toolbar.custom_features;
        if list.iter().all(|f| f.type_id != type_id) {
            let Some(t) = noshape_model::features::get(type_id) else { return };
            list.push(CustomTool { type_id: type_id.to_string(), name: t.name.to_string() });
            self.ui.toolbar.save();
        }
        cx.notify();
    }

    /// Remove: an added custom feature off the toolbar, at once (live 2213).
    fn remove_my_feature(&mut self, type_id: &str, cx: &mut Context<Self>) {
        self.ui.toolbar.custom_features.retain(|f| f.type_id != type_id);
        self.ui.toolbar.save();
        self.ui.toolbar_faces.remove(ADD_CUSTOM_FACE);
        cx.notify();
    }

    /// The panel, right-aligned under the toolbar over the graphics.
    pub(super) fn render_add_custom_features(&self) -> Option<AnyElement> {
        let a = self.ui.add_features.as_ref()?;
        Some(div().absolute().top(px(3.)).right_0().child(a.panel.clone()).into_any_element())
    }

    /// ⊞ "Add custom features"; with the user's custom features a split tool: its face the one last picked from
    /// the ▾ (⊞ until then: live showed it after the panel's adds), the ▾ the features and then Add custom features.
    pub(super) fn add_custom_features_button(&mut self, cx: &mut Context<Self>) -> AnyElement {
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
                        menu = menu.item(
                            PopupMenuItem::element(move |_, _| {
                                let n = name.clone();
                                let row = h_flex().h(px(32.)).flex_1().gap(px(4.)).child(div().w(px(30.)).flex_none().flex().items_center().justify_center().child(crate::ui::initials_icon(&name))).child(div().flex_1().text_size(px(14.)).child(name.clone()));
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
