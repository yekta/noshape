//! Drawing editor: Onshape's Insert view panel (top left while a base view
//! follows the cursor: Insert button + source, View orientation, View
//! scale, View simplification, Show sketch appearances, Show annotations
//! from model) and the "Select a part or assembly" chooser its Insert
//! button (and the toolbar's Insert view) opens.

use crate::drawing_view::*;
use crate::theme::p;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
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
}

impl Default for InsertViewState {
    fn default() -> Self {
        Self { scale: None, simplification: Simplification::Auto, simp_value: None, sketch_appearances: false, model_annotations: true, chooser: None, sketch: None, flat: false, replace: None, replace_part: None }
    }
}

/// The five View simplification choices, with Onshape-like default values.
pub fn simplification_choices(cur: Simplification) -> [Simplification; 5] {
    let keep = |d: Simplification| if std::mem::discriminant(&d) == std::mem::discriminant(&cur) { cur } else { d };
    [Simplification::None, Simplification::Auto, keep(Simplification::Absolute(0.001)), keep(Simplification::RatioToStudio(0.01)), keep(Simplification::RatioToPart(0.01))]
}

/// The value field text of a simplification ("1 mm", "1 %").
pub fn simplification_text(s: Simplification, mm: bool) -> String {
    let trim = |v: f64| {
        let t = format!("{v:.4}");
        t.trim_end_matches('0').trim_end_matches('.').to_string()
    };
    match s {
        Simplification::Absolute(m) if mm => format!("{} mm", trim(m * 1000.0)),
        Simplification::Absolute(m) => format!("{} in", trim(m / 0.0254)),
        Simplification::RatioToStudio(r) | Simplification::RatioToPart(r) => format!("{} %", trim(r * 100.0)),
        _ => String::new(),
    }
}

/// Parse the value field into `kind` (lengths in the drawing units by default).
pub fn parse_simplification(kind: Simplification, text: &str, mm: bool) -> Option<Simplification> {
    match kind {
        Simplification::Absolute(_) => {
            let prefs = noshape_units::UnitPrefs { length: if mm { noshape_units::LengthUnit::Millimeter } else { noshape_units::LengthUnit::Inch }, ..Default::default() };
            let q = noshape_units::evaluate_as(text.trim(), noshape_units::Dim::LENGTH, &prefs, &noshape_units::EmptyScope).ok()?;
            (q.value > 0.0).then_some(Simplification::Absolute(q.value))
        }
        Simplification::RatioToStudio(_) | Simplification::RatioToPart(_) => {
            let v: f64 = text.trim().trim_end_matches('%').trim().parse().ok()?;
            let r = (v / 100.0).max(0.0);
            Some(if matches!(kind, Simplification::RatioToStudio(_)) { Simplification::RatioToStudio(r) } else { Simplification::RatioToPart(r) })
        }
        other => Some(other),
    }
}

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
        let st = cx.new(|cx| InputState::new(window, cx).default_value(simplification_text(s, mm)));
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
        self.doc.read(cx).session.doc.ordered_tabs().into_iter().filter(|t| matches!(t.content, TabContent::PartStudio(_))).map(|t| SourceKey::studio(t.id)).collect()
    }

    fn choose_source(&mut self, source: TabId, part: Option<noshape_model::BodyId>, cx: &mut Context<Self>) {
        self.insert.chooser = None;
        if let (Some(old), Some(old_part)) = (self.insert.replace, self.insert.replace_part.take()) {
            // Sheets panel reference › Replace: on every sheet (help H:sheets).
            self.insert.replace = None;
            self.edit("Replace", cx, |d| {
                for v in d.sheets.iter_mut().flat_map(|s| s.views.iter_mut()) {
                    if let ViewKind::Base { source: src, part: p, .. } = &mut v.kind
                        && *src == old
                        && *p == old_part
                    {
                        *src = source;
                        *p = part.clone();
                    }
                }
            });
            self.ensure_sources(false, cx);
            return;
        }
        if let Some(old) = self.insert.replace.take() {
            // Replace: every base view of the old source shows the new one.
            self.edit_sheet("Replace", cx, |s| {
                for v in s.views.iter_mut() {
                    if let ViewKind::Base { source: src, .. } = &mut v.kind
                        && *src == old
                    {
                        *src = source;
                    }
                }
            });
            self.ensure_sources(false, cx);
            return;
        }
        let orientation = match &self.tool {
            Tool::InsertView { orientation, .. } => *orientation,
            _ => ViewOrientation::Front,
        };
        self.tool = Tool::Select;
        self.insert.sketch = None;
        self.insert.flat = false;
        self.set_tool(Tool::InsertView { source, orientation, part }, cx);
    }

    fn set_insert_orientation(&mut self, o: ViewOrientation, cx: &mut Context<Self>) {
        if let Tool::InsertView { orientation, .. } = &mut self.tool {
            *orientation = o;
        }
        self.update_preview(cx);
        cx.notify();
    }

    /// Source label of the panel: "<Part Studio> <part>" (or the assembly).
    fn insert_source_label(&self, source: TabId, part: Option<&noshape_model::BodyId>, cx: &App) -> String {
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
        let select = |id: &'static str, current: String, items: Vec<(String, bool, Act)>| {
            let ed = ed.clone();
            Button::new(id)
                .label(current)
                .ghost()
                .xsmall()
                .w_full()
                .dropdown_caret(true)
                .border_b_1()
                .border_color(gpui_kit::rgb(0xd7d7d7))
                .rounded(px(0.))
                .os_dropdown_menu(SharedString::from(format!("{id}-dd")), true, move |mut menu, _, _| {
                    for (label, checked, f) in &items {
                        let (e, f) = (ed.clone(), f.clone());
                        menu = menu.item(crate::os_menu::OsMenuItem::new(label.clone()).checked(*checked).on_click(move |_, _, cx| e.update(cx, |this, cx| f(this, cx))));
                    }
                    menu
                })
                .into_any_element()
        };
        // Source dropdown: the whole Part Studio or one of its parts.
        let mut src_items: Vec<(String, bool, Act)> = Vec::new();
        let is_asm = matches!(self.doc.read(cx).session.doc.tab(source).map(|t| &t.content), Some(TabContent::Assembly(_)));
        if !is_asm {
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
        let orient_items: Vec<(String, bool, Act)> = ORIENTATIONS.iter().map(|&o| (format!("{o:?}"), o == orientation, std::rc::Rc::new(move |t: &mut DrawingEditor, cx: &mut Context<DrawingEditor>| t.set_insert_orientation(o, cx)) as Act)).collect();
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
                        .child(Button::new("div-close").ghost().child(crate::drawing_chrome::cancel_x(cx)).w(px(28.)).h(px(28.)).p_0().tooltip("Cancel (Esc)").on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
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
                        .child(div().flex_1().min_w_0().child(if src_items.is_empty() { div().px_1().truncate().text_size(px(13.33)).child(source_label).into_any_element() } else { select("div-source", source_label, src_items) })),
                )
                .child(row("View orientation", select("div-orient", format!("{orientation:?}"), orient_items)))
                .child(row("View scale", select("div-scale", format_scale(cur_scale.unwrap_or(sheet_scale)), scale_items)))
                .child(row("View simplification", select("div-simp", simp.name().to_string(), simp_items)))
                .children(self.insert.simp_value.as_ref().filter(|_| !simplification_text(simp, true).is_empty()).map(|(st, _)| row("", Input::new(st).small().into_any_element())))
                .child(
                    div().px(px(6.)).py(px(3.)).child(Checkbox::new("div-sketch-app").label("Show sketch appearances").small().checked(self.insert.sketch_appearances).on_click(cx.listener(|this, on: &bool, _, cx| {
                        this.insert.sketch_appearances = *on;
                        cx.notify();
                    }))),
                )
                .child(
                    div().px(px(6.)).pt(px(3.)).pb(px(6.)).child(Checkbox::new("div-model-ann").label("Show annotations from model").small().checked(self.insert.model_annotations).on_click(cx.listener(|this, on: &bool, _, cx| {
                        this.insert.model_annotations = *on;
                        cx.notify();
                    }))),
                )
                .into_any_element(),
        )
    }

    /// "Select a part or assembly": Part Studios (whole studio or one part)
    /// and assemblies of the document.
    pub fn render_insert_chooser(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let tab = self.insert.chooser?;
        let pal = p();
        // (tab, name, is assembly) of the insertable tabs, in tab order.
        let tabs: Vec<(TabId, String, bool)> = self
            .doc
            .read(cx)
            .session
            .doc
            .ordered_tabs()
            .into_iter()
            .filter_map(|t| match &t.content {
                TabContent::PartStudio(_) => Some((t.id, t.name.clone(), false)),
                TabContent::Assembly(_) => Some((t.id, t.name.clone(), true)),
                _ => None,
            })
            .collect();
        let dtab = |id: &'static str, label: &'static str, on: bool, value: u8, cx: &mut Context<Self>| {
            div()
                .id(id)
                .flex_1()
                .h(px(32.))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(12.))
                .cursor_pointer()
                .text_color(if on { pal.accent } else { pal.text })
                .when(on, |d| d.font_weight(FontWeight::MEDIUM).border_b_2().border_color(pal.accent))
                .child(label)
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.insert.chooser = Some(value);
                    cx.notify();
                }))
        };
        let row = |id: SharedString, indent: f32, icon: &'static str, name: String, cx: &App| {
            h_flex()
                .id(id)
                .h(px(32.))
                .pl(px(8. + indent))
                .gap(px(8.))
                .cursor_pointer()
                .hover(|d| d.bg(pal.row_hover))
                .child(crate::assets::icon(&crate::assets::icon_path(icon), 18., cx))
                .child(div().flex_1().min_w_0().truncate().text_size(px(13.33)).child(name))
        };
        let mut list = v_flex().id("dr-chooser-list").flex_1().min_h_0().overflow_y_scroll();
        let mut any = false;
        for (id, name, asm) in tabs {
            match (asm, tab) {
                (false, 0) => {
                    any = true;
                    list = list.child(row(SharedString::from(format!("dr-ch-ps-{id}")), 0., "part-studio", name, cx).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.choose_source(id, None, cx))));
                    if let Some(src) = self.sources.get(&SourceKey::studio(id)) {
                        for b in views::view_bodies(&src.context, None) {
                            // Sheet metal parts: "Flat pattern of <part>" above the part.
                            if src.context.sheet_metal.contains_key(&b.id) {
                                let bid = b.id.clone();
                                list = list.child(row(SharedString::from(format!("dr-ch-f-{id}-{}", b.id)), 20., "sheet-metal", format!("Flat pattern of {}", b.name), cx).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    this.choose_source(id, Some(bid.clone()), cx);
                                    this.insert.flat = true;
                                    this.set_insert_orientation(ViewOrientation::Top, cx);
                                })));
                            }
                            let bid = b.id.clone();
                            list = list.child(row(SharedString::from(format!("dr-ch-p-{id}-{}", b.id)), 20., "part", b.name.clone(), cx).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.choose_source(id, Some(bid.clone()), cx))));
                        }
                        // Sketches give sketch views.
                        for s in &src.context.sketches {
                            let f = s.feature;
                            list = list.child(row(SharedString::from(format!("dr-ch-s-{id}-{}", f.0)), 20., "sketch", s.name.clone(), cx).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                this.choose_source(id, None, cx);
                                this.insert_sketch(Some(f), cx);
                            })));
                        }
                    }
                }
                (true, 1) => {
                    any = true;
                    list = list.child(row(SharedString::from(format!("dr-ch-asm-{id}")), 0., "assembly", name, cx).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.choose_source(id, None, cx))));
                }
                _ => {}
            }
        }
        if !any {
            list = list.child(div().p_2().text_size(px(12.)).text_color(pal.text_muted).child(if tab == 0 { "No Part Studios in this document." } else { "No assemblies in this document." }));
        }
        let doc_name = self.doc.read(cx).name();
        Some(
            div()
                .id("dr-chooser-backdrop")
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .bg(pal.modal_backdrop)
                .occlude()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    v_flex()
                        .w(px(420.))
                        .h(px(460.))
                        .bg(pal.modal_bg)
                        .rounded(px(4.))
                        .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.25), offset: point(px(0.), px(6.)), blur_radius: px(18.), spread_radius: px(0.), inset: false }])
                        .text_color(pal.text)
                        .child(
                            h_flex()
                                .h(px(40.))
                                .px(px(10.))
                                .border_b_1()
                                .border_color(pal.border)
                                .child(div().flex_1().text_size(px(16.)).child("Select a part or assembly"))
                                .child(Button::new("dr-chooser-close").ghost().child(crate::ui::glyph("close", 14., pal.text_muted)).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.insert.chooser = None;
                                    cx.notify();
                                }))),
                        )
                        .child(h_flex().h(px(30.)).px(px(10.)).text_size(px(13.)).font_weight(FontWeight::MEDIUM).child(doc_name))
                        .child(h_flex().border_b_1().border_color(gpui_kit::rgb(0xeeeeee)).child(dtab("dr-ch-ps", "Part Studios", tab == 0, 0, cx)).child(dtab("dr-ch-asm", "Assemblies", tab == 1, 1, cx)))
                        .child(list),
                )
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

    /// The view frame of the sketch being inserted (looking at its plane).
    pub fn insert_sketch_frame(&self, source: noshape_model::document::TabId) -> Option<[[f64; 3]; 2]> {
        let f = self.insert.sketch.as_ref()?;
        let key = noshape_model::drawing::views::SourceKey { tab: source, config: Default::default(), explode: None, version: None };
        noshape_model::drawing::views::sketch_frame(&self.sources.get(&key)?.context, f)
    }
}
