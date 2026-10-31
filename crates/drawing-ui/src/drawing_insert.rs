//! Drawing editor: Onshape's Insert view panel (top left while a base view
//! follows the cursor: Insert button + source, View orientation, View
//! scale, View simplification, Show sketch appearances, Show annotations
//! from model) and the "Select a part or assembly" chooser its Insert
//! button (and the toolbar's Insert view) opens.

use crate::drawing_view::*;
use crate::theme::p;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use crate::os_ui::OsDropdownExt as _;
use gpui_kit::component::{Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::document::{TabContent, TabId};
use noshape_model::drawing::sheet::format_scale;
use noshape_model::drawing::views::{self, SourceKey};
use noshape_model::drawing::*;

/// Insert view panel settings (kept between inserts, like Onshape).
pub struct InsertViewState {
    /// View scale (None: the sheet scale).
    pub scale: Option<f64>,
    /// View simplification of new views and its value field (Absolute /
    /// Ratio to studio / Ratio to part).
    pub simplification: Simplification,
    pub simp_value: Option<(Entity<InputState>, Subscription)>,
    pub sketch_appearances: bool,
    pub model_annotations: bool,
    /// "Select a part or assembly" chooser: open, and its tab (0 Part
    /// Studios, 1 Assemblies).
    pub chooser: Option<u8>,
    /// The Insert view tool's empty panel is open without the chooser
    /// (live drawing/122, 170a: [Insert] opens the chooser).
    pub panel: bool,
    /// Insert a sketch of the source (a sketch view).
    pub sketch: Option<noshape_model::FeatureId>,
    /// Insert the chosen sheet metal part's flat pattern.
    pub flat: bool,
    /// The chooser picks a replacement for this source (Sheets panel ›
    /// BOM › Replace): its views (and so its BOM) switch to the choice.
    pub replace: Option<TabId>,
    /// Sheets panel › reference › Replace...: the replaced reference's part
    /// (`None` inside: the whole source); its views on every sheet switch.
    pub replace_part: Option<Option<noshape_model::BodyId>>,
    /// View orientation › one of the source's named views and its frame
    /// (replaces the standard orientation while set).
    pub named: Option<(String, [[f64; 3]; 2], Vec<[f64; 4]>)>,
    /// Assemblies: Explode/Position and Display state (None: Show all) of
    /// new views (prefilled from the assembly's last applied ones).
    pub asm_position: AsmPosition,
    pub display_state: Option<String>,
    /// Other documents: the chosen document version (the source's reference
    /// key as the pinned id; [`DrawingEditor::version_doc`] loads it).
    pub pinned: Option<PinnedVersion>,
}

impl Default for InsertViewState {
    fn default() -> Self {
        Self { scale: None, simplification: Simplification::Auto, simp_value: None, sketch_appearances: false, model_annotations: true, chooser: None, panel: false, sketch: None, flat: false, replace: None, replace_part: None, named: None, asm_position: AsmPosition::Default, display_state: None, pinned: None }
    }
}




pub use noshape_ops::drawing::{asm_position_name, asm_positions, named_view_frame, named_view_section, named_views, parse_simplification, simplification_choices, simplification_text};

type Act = std::rc::Rc<dyn Fn(&mut DrawingEditor, &mut Context<DrawingEditor>)>;





const ORIENTATIONS: [ViewOrientation; 7] = [ViewOrientation::Top, ViewOrientation::Left, ViewOrientation::Right, ViewOrientation::Front, ViewOrientation::Back, ViewOrientation::Bottom, ViewOrientation::Isometric];
const SCALES: [f64; 12] = [10.0, 5.0, 4.0, 2.0, 1.0, 0.5, 0.25, 0.2, 0.1, 0.05, 0.02, 0.01];

impl DrawingEditor {
    /// The Insert view panel's simplification value field (made from render).
    pub fn sync_insert_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let s = self.insert.simplification;
        let mm = self.drawing.units_mm;
        if !matches!(self.tool, Tool::InsertView { .. }) || self.insert.simp_value.is_some() || simplification_text(s, mm).is_empty() {
            return;
        }
        let st = cx.new(|cx| InputState::new(window, cx).select_on_focus(false).revert_on_escape(false).default_value(simplification_text(s, mm)));
        let sub = cx.subscribe(&st, |this: &mut Self, st, ev: &InputEvent, cx| {
            if matches!(ev, InputEvent::Change)
                && let Some(n) = parse_simplification(this.insert.simplification, &st.read(cx).value(), this.drawing.units_mm)
            {
                this.insert.simplification = n;
                this.update_preview(cx);
            }
        });
        self.insert.simp_value = Some((st, sub));
    }

    /// Toolbar Insert view / the panel's Insert button: the chooser.
    /// The Insert view tool: its panel, empty until [Insert] picks a source
    /// (live drawing/122).
    pub fn open_insert_panel(&mut self, cx: &mut Context<Self>) {
        if self.insert.chooser.is_none() && !matches!(self.tool, Tool::InsertView { .. }) {
            self.insert.panel = true;
        }
        cx.notify();
    }

    pub fn open_insert_chooser(&mut self, cx: &mut Context<Self>) {
        self.insert.chooser = Some(self.insert.chooser.unwrap_or(0));
        self.ensure_sources(false, cx);
        cx.notify();
    }

    /// Part Studios whose parts the chooser lists (regenerated on demand).
    pub fn chooser_sources(&self, cx: &App) -> Vec<SourceKey> {
        if self.insert.chooser.is_none() {
            return Vec::new();
        }
        // Other documents: the opened document's Part Studios at its version.
        if self.chooser.other {
            let Some(d) = &self.chooser.other_doc else { return Vec::new() };
            return d.tabs.iter().filter(|t| !t.2).filter_map(|t| Some(SourceKey { version: Some(d.key(t.0)?.0), ..SourceKey::studio(t.0) })).collect();
        }
        self.doc.read(cx).session.doc.ordered_tabs().into_iter().filter(|t| matches!(t.content, TabContent::PartStudio(_))).map(|t| SourceKey::studio(t.id)).collect()
    }

    /// Other documents: a Part Studio (or one of its parts) or an assembly
    /// of another document's version (`key`: its `ExternalRef` key, `label`:
    /// "<document> <version>").
    pub fn choose_external(&mut self, key: String, tab: TabId, part: Option<noshape_model::BodyId>, label: String, cx: &mut Context<Self>) {
        self.choose_source(tab, part, cx);
        self.insert.pinned = Some(PinnedVersion { id: key, name: label });
        self.ensure_sources(false, cx);
        self.update_preview(cx);
    }

    pub fn choose_source(&mut self, source: TabId, part: Option<noshape_model::BodyId>, cx: &mut Context<Self>) {
        self.insert.chooser = None;
        self.insert.panel = false;
        self.insert.pinned = None;
        if let (Some(old), Some(old_part)) = (self.insert.replace, self.insert.replace_part.take()) {
            // Sheets panel reference › Replace: on every sheet (help H:sheets).
            self.insert.replace = None;
            self.edit("Replace", cx, |d| noshape_scene::drawing_tools::view_edit::replace_reference(d, old, &old_part, source, part.clone()));
            self.ensure_sources(false, cx);
            return;
        }
        if let Some(old) = self.insert.replace.take() {
            // Replace: every base view of the old source shows the new one.
            self.edit_sheet("Replace", cx, |s| noshape_scene::drawing_tools::view_edit::replace_sheet_source(s, old, source));
            self.ensure_sources(false, cx);
            return;
        }
        let orientation = match &self.tool {
            Tool::InsertView { orientation, source: old, .. } => {
                // Named views and assembly states belong to their source.
                if *old != source {
                    self.insert.named = None;
                    self.insert.asm_position = AsmPosition::Default;
                    self.insert.display_state = None;
                }
                *orientation
            }
            _ => {
                self.insert.named = None;
                self.insert.asm_position = AsmPosition::Default;
                self.insert.display_state = None;
                ViewOrientation::Front
            }
        };
        self.tool = Tool::Select;
        self.insert.sketch = None;
        self.insert.flat = false;
        self.set_tool(Tool::InsertView { source, orientation, part }, cx);
    }

    pub fn set_insert_orientation(&mut self, o: ViewOrientation, cx: &mut Context<Self>) {
        if let Tool::InsertView { orientation, .. } = &mut self.tool {
            *orientation = o;
        }
        self.insert.named = None;
        self.update_preview(cx);
        cx.notify();
    }

    /// View orientation › a named view of the source.
    fn set_insert_named(&mut self, name: String, cx: &mut Context<Self>) {
        let Tool::InsertView { source, .. } = &self.tool else { return };
        let doc = &self.doc.read(cx).session.doc;
        self.insert.named = named_view_frame(doc, *source, &name).map(|f| (name.clone(), f, named_view_section(doc, *source, &name)));
        self.update_preview(cx);
        cx.notify();
    }

    /// Source label of the panel: "<Part Studio> <part>" (or the assembly).
    fn insert_source_label(&self, source: TabId, part: Option<&noshape_model::BodyId>, cx: &App) -> String {
        // Another document's tab: its name from the regenerated source.
        if let Some(p) = &self.insert.pinned {
            return self.sources.get(&SourceKey { version: Some(p.id.clone()), ..SourceKey::studio(source) }).map(|s| s.name.clone()).unwrap_or_default();
        }
        let tab_name = self.doc.read(cx).session.doc.tab(source).map(|t| t.name.clone()).unwrap_or_default();
        let part_name = part.and_then(|b| self.sources.get(&SourceKey::studio(source)).and_then(|s| s.context.bodies.iter().find(|x| &x.id == b).map(|x| x.name.clone())));
        match part_name {
            Some(p) => format!("{tab_name} {p}"),
            None => tab_name,
        }
    }

    /// The Insert view panel while a base view follows the cursor.
    pub fn render_insert_view_panel(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let Tool::InsertView { source, orientation, part } = self.tool.clone() else { return None };
        let pal = p();
        let ed = cx.entity();
        let sheet_scale = self.current_sheet().scale;
        let label = |text: &'static str| div().w(px(105.)).flex_none().text_size(px(12.)).text_color(pal.text).child(text);
        // Items from `user_from` on are user content (parts, named views,
        // positions, display states): never translated.
        let select = |id: &'static str, current: String, items: Vec<(String, bool, Act)>, user_from: usize| {
            let ed = ed.clone();
            let user = items.iter().position(|i| i.1).is_some_and(|i| i >= user_from);
            Button::new(id)
                .when(!user, |b| b.label(current.clone()))
                .when(user, |b| b.child(div().min_w_0().whitespace_nowrap().text_ellipsis().child(crate::ui::user_text(current))))
                .ghost()
                .xsmall()
                .w_full()
                .dropdown_caret(true)
                .border_b_1()
                .border_color(gpui_kit::rgb(0xd7d7d7))
                .rounded(px(0.))
                .os_dropdown_menu(SharedString::from(format!("{id}-dd")), true, move |mut menu, _, _| {
                    for (i, (label, checked, f)) in items.iter().enumerate() {
                        let (e, f) = (ed.clone(), f.clone());
                        let item = crate::os_menu::OsMenuItem::new(label.clone());
                        let item = if i >= user_from { item.user() } else { item };
                        menu = menu.item(item.checked(*checked).on_click(move |_, _, cx| e.update(cx, |this, cx| f(this, cx))));
                    }
                    menu
                })
                .into_any_element()
        };
        // Source dropdown: the whole Part Studio or one of its parts.
        let mut src_items: Vec<(String, bool, Act)> = Vec::new();
        let is_asm = matches!(self.doc.read(cx).session.doc.tab(source).map(|t| &t.content), Some(TabContent::Assembly(_)));
        if !is_asm && self.insert.pinned.is_none() {
            let tab_name = self.doc.read(cx).session.doc.tab(source).map(|t| t.name.clone()).unwrap_or_default();
            src_items.push((format!("{tab_name} (all parts)"), part.is_none(), std::rc::Rc::new(move |t: &mut DrawingEditor, cx: &mut Context<DrawingEditor>| t.choose_source(source, None, cx))));
            if let Some(src) = self.sources.get(&SourceKey::studio(source)) {
                for b in views::view_bodies(&src.context, None) {
                    let id = b.id.clone();
                    let on = part.as_ref() == Some(&b.id);
                    src_items.push((b.name.clone(), on, std::rc::Rc::new(move |t: &mut DrawingEditor, cx: &mut Context<DrawingEditor>| t.choose_source(source, Some(id.clone()), cx))));
                }
            }
        }
        // The standard orientations, then the source's named views (help H:views).
        let named = self.insert.named.as_ref().map(|n| n.0.clone());
        let mut orient_items: Vec<(String, bool, Act)> = ORIENTATIONS.iter().map(|&o| (format!("{o:?}"), named.is_none() && o == orientation, std::rc::Rc::new(move |t: &mut DrawingEditor, cx: &mut Context<DrawingEditor>| t.set_insert_orientation(o, cx)) as Act)).collect();
        for nv in named_views(&self.doc.read(cx).session.doc, source) {
            let name = nv.name.clone();
            orient_items.push((nv.name.clone(), named.as_ref() == Some(&nv.name), std::rc::Rc::new(move |t: &mut DrawingEditor, cx: &mut Context<DrawingEditor>| t.set_insert_named(name.clone(), cx)) as Act));
        }
        let orient_label = named.clone().unwrap_or_else(|| format!("{orientation:?}"));
        let cur_scale = self.insert.scale;
        let scale_items: Vec<(String, bool, Act)> = SCALES
            .iter()
            .map(|&s| {
                let on = cur_scale.unwrap_or(sheet_scale) == s;
                (
                    format_scale(s),
                    on,
                    std::rc::Rc::new(move |t: &mut DrawingEditor, cx: &mut Context<DrawingEditor>| {
                        // The sheet scale itself means "follow the sheet".
                        t.insert.scale = (s != t.current_sheet().scale).then_some(s);
                        t.update_preview(cx);
                        cx.notify();
                    }) as Act,
                )
            })
            .collect();
        let simp = self.insert.simplification;
        let simp_items: Vec<(String, bool, Act)> = simplification_choices(simp)
            .into_iter()
            .map(|c| {
                (
                    c.name().to_string(),
                    std::mem::discriminant(&c) == std::mem::discriminant(&simp),
                    std::rc::Rc::new(move |t: &mut DrawingEditor, cx: &mut Context<DrawingEditor>| {
                        t.insert.simplification = c;
                        t.insert.simp_value = None;
                        cx.notify();
                    }) as Act,
                )
            })
            .collect();
        let row = |l: &'static str, body: AnyElement| h_flex().h(px(28.)).px(px(6.)).child(label(l)).child(div().flex_1().min_w_0().child(body));
        // Assemblies: Explode/Position and Display state (live drawing/1106).
        let asm_rows: Vec<AnyElement> = match self.doc.read(cx).session.doc.tab(source).map(|t| &t.content) {
            Some(TabContent::Assembly(asm)) => {
                let cur = self.insert.asm_position;
                let mut pos_items: Vec<(String, bool, Act)> = Vec::new();
                for (name, p) in asm_positions(asm) {
                    pos_items.push((name, p == cur, std::rc::Rc::new(move |t: &mut DrawingEditor, cx: &mut Context<DrawingEditor>| {
                        t.insert.asm_position = p;
                        t.update_preview(cx);
                        cx.notify();
                    }) as Act));
                }
                let cur_d = self.insert.display_state.clone();
                let mut ds_items: Vec<(String, bool, Act)> = vec![("Show all".into(), cur_d.is_none(), std::rc::Rc::new(|t: &mut DrawingEditor, cx: &mut Context<DrawingEditor>| {
                    t.insert.display_state = None;
                    t.update_preview(cx);
                    cx.notify();
                }) as Act)];
                for d in &asm.display_states {
                    let n = d.name.clone();
                    ds_items.push((d.name.clone(), cur_d.as_ref() == Some(&d.name), std::rc::Rc::new(move |t: &mut DrawingEditor, cx: &mut Context<DrawingEditor>| {
                        t.insert.display_state = Some(n.clone());
                        t.update_preview(cx);
                        cx.notify();
                    }) as Act));
                }
                let pos_label = asm_position_name(asm, cur);
                vec![
                    row("Explode/Position", select("div-explode", pos_label, pos_items, 1)).into_any_element(),
                    row("Display state", select("div-display", cur_d.unwrap_or_else(|| "Show all".into()), ds_items, 1)).into_any_element(),
                ]
            }
            _ => Vec::new(),
        };
        let source_label = self.insert_source_label(source, part.as_ref(), cx);
        Some(
            v_flex()
                .id("dr-insert-view")
                .absolute()
                .top(px(DIALOG_TOP))
                .left(px(self.dialog_left()))
                .w(px(247.))
                .bg(pal.dialog_bg)
                .shadow(crate::drawing_chrome::dialog_shadow())
                .text_color(pal.text)
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    h_flex()
                        .h(px(28.))
                        .pl(px(6.))
                        .child(div().flex_1().text_size(px(13.)).font_weight(FontWeight::MEDIUM).child("Insert view"))
                        .child(Button::new("div-close").ghost().child(crate::drawing_chrome::cancel_x(cx)).w(px(28.)).h(px(28.)).p_0().on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.insert.chooser = None;
                            this.cancel(cx);
                        }))),
                )
                .child(
                    h_flex()
                        .h(px(30.))
                        .px(px(4.))
                        .gap(px(3.))
                        .child(Button::new("div-insert").label("Insert").icon(gpui_kit::component::Icon::empty().path(crate::assets::icon_path("insert"))).outline().small().w(px(104.)).on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.open_insert_chooser(cx))))
                        .child(div().flex_1().min_w_0().child(if src_items.is_empty() { div().px_1().truncate().text_size(px(13.33)).child(crate::ui::user_text(source_label)).into_any_element() } else { select("div-source", source_label, src_items, 0) })),
                )
                .child(row("View orientation", select("div-orient", orient_label, orient_items, ORIENTATIONS.len())))
                .children(asm_rows)
                .child(row("View scale", select("div-scale", format_scale(cur_scale.unwrap_or(sheet_scale)), scale_items, usize::MAX)))
                .child(row("View simplification", select("div-simp", simp.name().to_string(), simp_items, usize::MAX)))
                .children(self.insert.simp_value.as_ref().filter(|_| !simplification_text(simp, true).is_empty()).map(|(st, _)| row("", Input::new(st).small().into_any_element())))
                .child(
                    div().px(px(6.)).py(px(3.)).child(crate::drawing_chrome::DrCheckbox::new("div-sketch-app").label("Show sketch appearances").checked(self.insert.sketch_appearances).on_click(cx.listener(|this, on: &bool, _, cx| {
                        this.insert.sketch_appearances = *on;
                        cx.notify();
                    }))),
                )
                // Assemblies have no model annotations (live drawing/1106).
                .when(is_asm, |d| d.child(div().h(px(3.))))
                .when(!is_asm, |d| d.child(
                    div().px(px(6.)).pt(px(3.)).pb(px(6.)).child(crate::drawing_chrome::DrCheckbox::new("div-model-ann").label("Show annotations from model").checked(self.insert.model_annotations).on_click(cx.listener(|this, on: &bool, _, cx| {
                        this.insert.model_annotations = *on;
                        cx.notify();
                    }))),
                ))
                .into_any_element(),
        )
    }
}

impl DrawingEditor {
    /// Insert view of a sketch ("Create Drawing of Sketch N…" with No views).
    pub fn insert_sketch(&mut self, sketch: Option<noshape_model::FeatureId>, cx: &mut Context<Self>) {
        self.insert.sketch = sketch;
        self.update_preview(cx);
    }

}
