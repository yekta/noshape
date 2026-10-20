//! Version and workspace "Properties…" (live document/72–73, 76, 78;
//! dom/version-properties*.txt, branch-edit-properties*.txt): Onshape's
//! 800 px metadata modal with the item tree at the left (300 px, `#f5f5f5`):
//! the version ⧫ / workspace ⚲ row, which expands to the document's tabs
//! at that point (thumbnail, name, italic type). The right side holds the
//! selected item's name over a `#ccc` rule, Name \*, Description and the
//! greyed Category ("Onshape Version" / "Onshape Workspace"); the footer
//! has form undo / redo and Apply, Save (disabled until edited), Close.

use crate::doc_model::{DocEvent, DocModel};
use crate::properties_dialog::{PROPS_MIN, bootstrap_modal, footer_button, gray_box, label, modal_header, props_size, resizable_props};
use crate::theme::p;
use gpui_kit::component::WindowExt as _;
use gpui_kit::component::input::{Input, InputEvent, InputState, Textarea, TextareaState};
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::document::{Document, TabContent, TabId};
use noshape_store::{BranchId, VersionId};
use std::collections::HashMap;
use std::sync::Arc;

/// A workspace's description (store meta; the branches table has none).
pub const WORKSPACE_DESCRIPTION_KEY: &str = "workspace_description:";

#[derive(Clone, Copy)]
pub enum Item {
    Version(VersionId),
    Workspace(BranchId),
}

type Listener = Box<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

/// A tab of the document at the item: id, name, type name, icon.
struct TabRow {
    id: TabId,
    name: String,
    kind: String,
    icon: &'static str,
}

pub struct ItemProperties {
    doc: Entity<DocModel>,
    item: Item,
    name: Entity<InputState>,
    description: Entity<TextareaState>,
    /// Saved (name, description): Apply / Save are enabled while the form differs.
    saved: (String, String),
    history: Vec<(String, String)>,
    at: usize,
    restoring: bool,
    expanded: bool,
    tabs: Vec<TabRow>,
    /// The selected tab (None: the version / workspace itself).
    selected: Option<TabId>,
    thumbs: HashMap<TabId, Arc<RenderImage>>,
    _subs: Vec<Subscription>,
}

/// Open the Properties modal of a version or workspace.
pub fn open(doc: Entity<DocModel>, item: Item, window: &mut Window, cx: &mut App) {
    let store = &doc.read(cx).session.store;
    let (name, desc, mv) = match item {
        Item::Version(id) => {
            let Ok(v) = store.version(id) else { return };
            (v.name, v.description, v.microversion)
        }
        Item::Workspace(b) => {
            let Ok(info) = store.branch(b) else { return };
            let desc = store.get_meta(&format!("{WORKSPACE_DESCRIPTION_KEY}{b}")).ok().flatten().map(|d| String::from_utf8_lossy(&d).into_owned()).unwrap_or_default();
            (info.name, desc, info.head)
        }
    };
    let at = store.read_all(mv).ok().and_then(|b| Document::from_blobs(&b).ok());
    let form = cx.new(|cx| ItemProperties::new(doc.clone(), item, name, desc, window, cx));
    if let Some(at) = at {
        form.update(cx, |f, cx| f.load_tabs(at, cx));
    }
    // The header's -8 px margin takes up the kit's gap under the title (the
    // body clips, so a negative top margin there would hide the tree's top).
    window.open_dialog(cx, move |dialog, _, _| resizable_props(bootstrap_modal(dialog, "Properties", props_size().0, 15.).title(modal_header("Properties", props_size().0).mb(px(-8.))).child(form.clone())));
}

fn tab_kind(c: &TabContent) -> (String, &'static str) {
    let (k, icon): (&str, &'static str) = match c {
        TabContent::PartStudio(_) => ("Part Studio", "part-studio"),
        TabContent::Assembly(_) => ("Assembly", "assembly"),
        TabContent::Drawing(_) => ("Drawing", "drawing"),
        TabContent::VariableStudio(_) => ("Variable Studio", "variable-studio"),
        TabContent::FeatureStudio(_) => ("Feature Studio", "feature-studio"),
        TabContent::MaterialLibrary(_) => ("Material Library", "material-library"),
        TabContent::File(f) => {
            use noshape_model::file_tab::FileKind;
            // Onshape names a file tab's type by its translator: the
            // extension for CAD, PDF, images and drawings ("X_T"), else
            // "Unknown" (a CSV; live document/73).
            let kind = match f.kind() {
                FileKind::Csv | FileKind::Other => "Unknown".to_string(),
                _ => f.ext().to_uppercase(),
            };
            return (kind, crate::file_tab_view::file_icon(f));
        }
    };
    (k.to_string(), icon)
}

impl ItemProperties {
    fn new(doc: Entity<DocModel>, item: Item, name: String, desc: String, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let n = name.clone();
        let name_st = cx.new(|cx| InputState::new(window, cx).default_value(n));
        let d = desc.clone();
        let description = cx.new(|cx| TextareaState::new(window, cx).rows(2).default_value(d));
        let subs = vec![
            cx.subscribe(&name_st, |this: &mut Self, _, ev: &InputEvent, cx| {
                if matches!(ev, InputEvent::Change) {
                    this.changed(cx);
                }
            }),
            cx.subscribe(&description, |this: &mut Self, _, ev: &InputEvent, cx| {
                if matches!(ev, InputEvent::Change) {
                    this.changed(cx);
                }
            }),
        ];
        Self {
            doc,
            item,
            name: name_st,
            description,
            saved: (name.clone(), desc.clone()),
            history: vec![(name, desc)],
            at: 0,
            restoring: false,
            expanded: false,
            tabs: Vec::new(),
            selected: None,
            thumbs: HashMap::new(),
            _subs: subs,
        }
    }

    /// The tabs of the document at the item, and their pictures (Part
    /// Studios, Assemblies and Drawings, rendered off the UI thread).
    fn load_tabs(&mut self, at: Document, cx: &mut Context<Self>) {
        self.tabs = at
            .ordered_tabs()
            .into_iter()
            .map(|t| {
                let (kind, icon) = tab_kind(&t.content);
                TabRow { id: t.id, name: t.name.clone(), kind, icon }
            })
            .collect();
        let todo: Vec<(TabId, bool)> = at
            .ordered_tabs()
            .into_iter()
            .filter_map(|t| match &t.content {
                TabContent::PartStudio(_) => Some((t.id, false)),
                TabContent::Assembly(_) | TabContent::Drawing(_) => Some((t.id, true)),
                _ => None,
            })
            .collect();
        if todo.is_empty() {
            return;
        }
        let gpu = cx.global::<crate::gpu::GpuGlobal>().0.clone();
        let versions = self.doc.read(cx).session.pinned_documents();
        cx.spawn(async move |this, cx| {
            let made = cx
                .background_spawn(async move {
                    let evaluator = parking_lot::Mutex::new(noshape_model::assembly::eval::AssemblyEvaluator::new());
                    let mut out = Vec::new();
                    for (tab, assembly) in todo {
                        if let Some(t) = at.tab(tab)
                            && let TabContent::Drawing(d) = &t.content
                        {
                            if let Some(img) = crate::tab_manager::drawing_thumbnail(&at, &versions, d, t.name.clone()) {
                                out.push((tab, img));
                            }
                            continue;
                        }
                        if let Some(img) = crate::tab_manager::tab_thumbnail(&at, tab, assembly, &evaluator, &gpu).await {
                            out.push((tab, img));
                        }
                    }
                    out
                })
                .await;
            let _ = this.update(cx, |this: &mut Self, cx| {
                this.thumbs.extend(made);
                cx.notify();
            });
        })
        .detach();
    }

    fn values(&self, cx: &App) -> (String, String) {
        (self.name.read(cx).value().trim().to_string(), self.description.read(cx).value().to_string())
    }

    fn changed(&mut self, cx: &mut Context<Self>) {
        if self.restoring {
            return;
        }
        let v = self.values(cx);
        if self.history.get(self.at) != Some(&v) {
            self.history.truncate(self.at + 1);
            self.history.push(v);
            self.at = self.history.len() - 1;
        }
        cx.notify();
    }

    fn undo(&mut self, redo: bool, window: &mut Window, cx: &mut Context<Self>) {
        let to = if redo { self.at + 1 } else { self.at.wrapping_sub(1) };
        let Some((n, d)) = self.history.get(to).cloned() else { return };
        self.at = to;
        self.restoring = true;
        self.name.update(cx, |s, cx| s.set_value(n, window, cx));
        self.description.update(cx, |s, cx| s.set_value(d, window, cx));
        self.restoring = false;
        cx.notify();
    }

    fn dirty(&self, cx: &App) -> bool {
        let v = self.values(cx);
        v != self.saved && !v.0.is_empty()
    }

    /// Apply (keep the modal open) or Save (close it).
    fn save(&mut self, close: bool, window: &mut Window, cx: &mut Context<Self>) {
        if !self.dirty(cx) {
            return;
        }
        let (n, d) = self.values(cx);
        let item = self.item;
        let r = self.doc.update(cx, |doc, _| -> anyhow::Result<()> {
            let store = &mut doc.session.store;
            match item {
                Item::Version(id) => store.update_version(id, &n, &d)?,
                Item::Workspace(b) => {
                    store.rename_branch(b, &n)?;
                    let key = format!("{WORKSPACE_DESCRIPTION_KEY}{b}");
                    store.set_meta(&key, (!d.is_empty()).then_some(d.as_bytes()))?;
                }
            }
            Ok(())
        });
        match r {
            Ok(()) => {
                self.saved = (n, d);
                self.doc.update(cx, |doc, cx| doc.reloaded(cx));
            }
            Err(e) => self.doc.update(cx, |_, cx| cx.emit(DocEvent::Error(format!("{e:#}")))),
        }
        if close {
            window.close_dialog(cx);
        }
        cx.notify();
    }

    /// A tree row: ▸ / ▾ (when it has children), a 70×40 picture, the name
    /// and the italic type; light blue when selected.
    fn tree_row(id: &'static str, ix: usize, indent: f32, chevron: Option<(bool, Listener)>, picture: AnyElement, (name, kind): (String, String), selected: bool) -> Stateful<Div> {
        h_flex()
            .id((id, ix))
            .w_full()
            .h(px(55.))
            .flex_none()
            .pl(px(12. + indent))
            .pr(px(8.))
            .border_b_1()
            .border_color(rgb(0xdddddd))
            .when(selected, |d| d.bg(rgb(0xb2ddf6)))
            .when(!selected, |d| d.hover(|d| d.bg(rgb(0xe5f3fb))))
            .child(div().id("chev").w(px(12.)).h_full().flex_none().flex().items_center().when_some(chevron, |d, (open, toggle)| {
                d.cursor_pointer()
                    .child(crate::os_ui::glyph(if open { crate::os_ui::GLYPH_EXPANDED } else { crate::os_ui::GLYPH_COLLAPSED }, 12., rgb(0x333333)))
                    .on_click(move |e, window, cx| {
                        cx.stop_propagation();
                        toggle(e, window, cx)
                    })
            }))
            .child(div().w(px(70.)).h(px(40.)).flex_none().flex().items_center().justify_center().child(picture))
            .child(
                v_flex()
                    .pl(px(4.))
                    .min_w_0()
                    .flex_1()
                    // The version / workspace name is 500, tab names regular (live 73).
                    .child(div().text_size(px(13.33)).line_height(px(21.)).font_weight(if indent > 0. { FontWeight::NORMAL } else { FontWeight::MEDIUM }).text_color(rgb(0x333333)).truncate().child(name))
                    .child(div().text_size(px(12.)).italic().text_color(rgb(0x666666)).truncate().child(kind)),
            )
    }
}

impl Render for ItemProperties {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = p();
        let dirty = self.dirty(cx);
        let (icon, kind, category) = match self.item {
            Item::Version(_) => ("legend-version", "Version", "Onshape Version"),
            Item::Workspace(_) => ("legend-workspace", "Workspace", "Onshape Workspace"),
        };
        let group = |l: Div, f: AnyElement| v_flex().w_full().child(l).child(div().p(px(1.)).child(f));
        let toggle: Listener = {
            let this = cx.entity();
            Box::new(move |_, _, cx| {
                this.update(cx, |this, cx| {
                    this.expanded = !this.expanded;
                    cx.notify();
                })
            })
        };
        let mut tree = v_flex().id("item-tree").size_full().overflow_y_scroll().child(
            Self::tree_row("item-row", 0, 0., Some((self.expanded, toggle)), crate::assets::icon(icon, 30., cx), (self.saved.0.clone(), kind.into()), self.selected.is_none())
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.selected = None;
                    cx.notify();
                })),
        );
        if self.expanded {
            for (ix, t) in self.tabs.iter().enumerate() {
                let picture = match self.thumbs.get(&t.id) {
                    Some(image) => img(ImageSource::Render(image.clone())).w(px(70.)).h(px(40.)).object_fit(ObjectFit::Contain).into_any_element(),
                    None => crate::assets::icon(t.icon, 30., cx),
                };
                let id = t.id;
                tree = tree.child(Self::tree_row("item-tab", ix, 20., None, picture, (t.name.clone(), t.kind.clone()), self.selected == Some(id)).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.selected = Some(id);
                    cx.notify();
                })));
            }
        }
        // A tab: its name and type, read-only here.
        let tab = self.selected.and_then(|id| self.tabs.iter().find(|t| t.id == id));
        let heading = tab.map(|t| t.name.clone()).unwrap_or_else(|| self.saved.0.clone());
        let form = match tab {
            Some(t) => v_flex()
                .w_full()
                .gap(px(15.))
                .child(group(label("Name", true), gray_box(t.name.clone(), false).into_any_element()))
                .child(group(label("Description", false), gray_box(String::new(), false).h(px(56.)).into_any_element()))
                .child(group(label("Category", false), gray_box(format!("Onshape {}", t.kind), true).into_any_element())),
            None => v_flex()
                .w_full()
                .gap(px(15.))
                .child(group(label("Name", true), crate::os_ui::input_h(Input::new(&self.name), 34.).text_size(px(14.)).into_any_element()))
                .child(group(label("Description", false), Textarea::new(&self.description).h(px(56.)).text_size(px(14.)).into_any_element()))
                .child(group(label("Category", false), gray_box(category.to_string(), true).into_any_element())),
        };
        let (can_undo, can_redo) = (self.at > 0, self.at + 1 < self.history.len());
        let undo_button = |id: &'static str, icon: &'static str, tip: &'static str, on: bool| {
            div()
                .id(id)
                .size(px(30.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(3.))
                .when(!on, |d| d.opacity(0.25))
                .when(on, |d| d.cursor_pointer().hover(|d| d.bg(rgb(0xeeeeee))))
                .tooltip(crate::ui::tooltip(tip))
                .child(svg().path(crate::assets::icon_path(icon)).size(px(20.)).text_color(rgb(0x333333)))
        };
        let body_h = 421. + props_size().1 - PROPS_MIN.1;
        v_flex()
            .w_full()
            .text_color(pal.text)
            .child(
                h_flex()
                    .w_full()
                    .h(px(body_h))
                    .items_start()
                    .child(div().w(px(300.)).h_full().flex_none().bg(rgb(0xf5f5f5)).border_r_1().border_color(rgb(0xcccccc)).child(tree))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .child(
                                div()
                                    .mt(px(10.))
                                    .h(px(31.))
                                    .flex_none()
                                    .px(px(15.))
                                    .border_b_1()
                                    .border_color(rgb(0xcccccc))
                                    .child(div().pl(px(4.)).text_size(px(18.)).font_weight(FontWeight::MEDIUM).text_color(rgb(0x333333)).truncate().child(heading)),
                            )
                            .child(div().id("item-form").flex_1().overflow_y_scroll().p(px(15.)).child(form)),
                    ),
            )
            .child(
                h_flex()
                    .h(px(62.))
                    .px(px(10.))
                    .border_t_1()
                    .border_color(rgb(0xdee2e6))
                    .child(
                        h_flex()
                            .gap(px(9.))
                            .pl(px(3.))
                            .child(undo_button("item-undo", "undo", "Undo", can_undo).on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.undo(false, window, cx))))
                            .child(undo_button("item-redo", "redo", "Redo", can_redo).on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.undo(true, window, cx)))),
                    )
                    .child(div().flex_1())
                    .child(
                        h_flex()
                            .gap(px(5.))
                            .pr(px(4.))
                            .child(footer_button("item-apply", "Apply", true, dirty).on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.save(false, window, cx))))
                            .child(footer_button("item-save", "Save", true, dirty).on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.save(true, window, cx))))
                            .child(footer_button("item-close", "Close", false, true).on_click(|_, window, cx| window.close_dialog(cx))),
                    ),
            )
    }
}
