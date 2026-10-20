//! Drawing editor: Onshape's Sheet properties panel (empty-sheet menu ›
//! Sheet properties…, sheet row › Properties…): Scale (editable, with a
//! list), Format (the sheet template) with the Select a DWT button, Size,
//! Reference (+ link to the reference tab).

use crate::drawing_view::*;
use crate::theme::p;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputState};
use crate::os_ui::OsDropdownExt as _;
use gpui_kit::component::{Sizable as _, h_flex, v_flex};
use gpui_kit::*;
use gpui_kit::prelude::FluentBuilder as _;
use noshape_model::drawing::sheet::format_scale;
use noshape_model::drawing::views::SourceKey;
use noshape_model::drawing::*;

/// Sheet properties panel state: the sheet and its scale field.
pub struct SheetPropsState {
    pub sheet: usize,
    pub scale: Entity<InputState>,
}

const SCALES: [f64; 12] = [10.0, 5.0, 4.0, 2.0, 1.0, 0.5, 0.25, 0.2, 0.1, 0.05, 0.02, 0.01];

type Act = std::rc::Rc<dyn Fn(&mut DrawingEditor, &mut Window, &mut Context<DrawingEditor>)>;

impl DrawingEditor {
    pub fn open_sheet_props(&mut self, sheet: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(s) = self.drawing.sheets.get(sheet) else { return };
        self.view_props = None;
        self.panel = None;
        let text = format_scale(s.scale);
        let scale = cx.new(|cx| InputState::new(window, cx).default_value(text));
        scale.update(cx, |st, cx| st.focus(window, cx));
        self.sheet_props = Some(SheetPropsState { sheet, scale });
        cx.notify();
    }

    /// ✓: apply the typed scale and close.
    fn accept_sheet_props(&mut self, cx: &mut Context<Self>) {
        let Some(st) = self.sheet_props.take() else { return };
        if let Some(s) = crate::drawing_ui::parse_scale(&st.scale.read(cx).value())
            && self.drawing.sheets.get(st.sheet).is_some_and(|x| (x.scale - s).abs() > 1e-12)
        {
            let i = st.sheet;
            self.edit("Sheet scale", cx, |d| d.sheets[i].scale = s);
        }
        cx.notify();
    }

    /// The source tab of the sheet's first base view (title block properties).
    fn sheet_reference(&self, sheet: &Sheet, cx: &App) -> Option<(noshape_model::document::TabId, String)> {
        sheet.views.iter().find_map(|v| match &v.kind {
            ViewKind::Base { source, part, .. } => {
                let tab = self.doc.read(cx).session.doc.tab(*source)?.name.clone();
                let part_name = part.as_ref().and_then(|b| self.sources.get(&SourceKey::studio(*source)).and_then(|s| s.context.bodies.iter().find(|x| &x.id == b).map(|x| x.name.clone())));
                Some((*source, part_name.map(|p| format!("{tab} {p}")).unwrap_or(tab)))
            }
            _ => None,
        })
    }

    pub fn render_sheet_props(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let st = self.sheet_props.as_ref()?;
        let si = st.sheet;
        let sheet = self.drawing.sheets.get(si)?.clone();
        let pal = p();
        let ed = cx.entity();
        let dropdown = |id: &'static str, current: String, items: Vec<(String, bool, Act)>| {
            let ed = ed.clone();
            Button::new(id)
                .label(current)
                .ghost()
                .small()
                .w_full()
                .dropdown_caret(true)
                .border_b_1()
                .border_color(gpui_kit::rgb(0xdddddd))
                .rounded(px(0.))
                .os_dropdown_menu(SharedString::from(format!("{id}-dd")), true, move |mut menu, _, _| {
                    for (label, checked, f) in &items {
                        let (e, f) = (ed.clone(), f.clone());
                        menu = menu.item(crate::os_menu::OsMenuItem::new(label.clone()).checked(*checked).on_click(move |_, w, cx| e.update(cx, |this, cx| f(this, w, cx))));
                    }
                    menu
                })
        };
        let row = |label: &'static str, body: AnyElement, extra: Option<AnyElement>| {
            h_flex()
                .h(px(28.))
                .child(div().w(px(85.)).flex_none().pl(px(5.)).text_size(px(12.)).text_color(pal.text).child(label))
                .child(div().flex_1().min_w_0().child(body))
                .child(div().w(px(26.)).ml(px(2.)).flex_none().children(extra))
        };
        // Scale: the typed text, plus Onshape's list.
        let scale_items: Vec<(String, bool, Act)> = SCALES
            .iter()
            .map(|&s| {
                let act: Act = std::rc::Rc::new(move |t: &mut DrawingEditor, w: &mut Window, cx: &mut Context<DrawingEditor>| {
                    t.edit("Sheet scale", cx, |d| d.sheets[si].scale = s);
                    if let Some(p) = &t.sheet_props {
                        p.scale.update(cx, |i, cx| i.set_value(format_scale(s), w, cx));
                    }
                });
                (format_scale(s), (sheet.scale - s).abs() < 1e-12, act)
            })
            .collect();
        let ed2 = ed.clone();
        let scale_menu = Button::new("sp-scale-list").ghost().small().w(px(20.)).p_0().dropdown_caret(true).os_dropdown_menu("sp-scale-list-dd", true, move |mut menu, _, _| {
            for (label, checked, f) in &scale_items {
                let (e, f) = (ed2.clone(), f.clone());
                menu = menu.item(crate::os_menu::OsMenuItem::new(label.clone()).checked(*checked).on_click(move |_, w, cx| e.update(cx, |this, cx| f(this, w, cx))));
            }
            menu
        });
        let scale_field = h_flex().border_b_1().border_color(pal.accent).child(div().flex_1().child(Input::new(&st.scale).appearance(false).small())).child(scale_menu).into_any_element();
        // Format: Onshape's template (DWT) names; "Custom" when none matches.
        let tmpl = templates::standard_templates();
        let cur_tmpl = tmpl.iter().find(|t| t.size == sheet.size && t.style == sheet.title_style && t.inches != self.drawing.units_mm).map(|t| t.name);
        let format_items: Vec<(String, bool, Act)> = tmpl
            .iter()
            .map(|&tp| {
                let act: Act = std::rc::Rc::new(move |t: &mut DrawingEditor, _: &mut Window, cx: &mut Context<DrawingEditor>| {
                    t.edit(&format!("Template {}", tp.name), cx, |d| templates::apply_template(d, si, &tp));
                    t.fitted = false;
                });
                (tp.name.replace(' ', "_") + ".dwt", Some(tp.name) == cur_tmpl, act)
            })
            .collect();
        let format_label = cur_tmpl.map(|n| n.replace(' ', "_") + ".dwt").unwrap_or_else(|| "Custom".into());
        let dwt = Button::new("sp-dwt").ghost().small().w(px(26.)).h(px(26.)).p_0().border_1().border_color(gpui_kit::rgb(0xd3d7d9)).rounded(px(0.)).child(crate::assets::icon(&crate::assets::icon_path("title-block"), 18., cx)).tooltip("Select a DWT").into_any_element();
        // Size: Onshape lists A, A Portrait, B... for ANSI, A4... for ISO.
        let size_items: Vec<(String, bool, Act)> = SheetSize::STANDARD
            .iter()
            .filter(|s| s.is_ansi() == sheet.size.is_ansi())
            .map(|&s| {
                let label = if s == SheetSize::AnsiAPortrait { "A Portrait".to_string() } else { s.label().to_string() };
                let act: Act = std::rc::Rc::new(move |t: &mut DrawingEditor, _: &mut Window, cx: &mut Context<DrawingEditor>| {
                    t.edit("Sheet size", cx, |d| d.sheets[si].size = s);
                    t.fitted = false;
                });
                (label, s == sheet.size, act)
            })
            .collect();
        let size_label = match sheet.size {
            SheetSize::AnsiAPortrait => "A Portrait".to_string(),
            s => s.label().to_string(),
        };
        let reference = self.sheet_reference(&sheet, cx);
        // Live 124: "Unassigned" in grey when the sheet has no reference.
        let unassigned = reference.is_none();
        let ref_label = reference.as_ref().map(|r| r.1.clone()).unwrap_or_else(|| "Unassigned".into());
        let ref_items: Vec<(String, bool, Act)> = vec![(ref_label.clone(), true, std::rc::Rc::new(|_: &mut DrawingEditor, _: &mut Window, _: &mut Context<DrawingEditor>| {}) as Act)];
        let link = Some(match reference {
            None => div()
                .size(px(26.))
                .flex()
                .items_center()
                .justify_center()
                .bg(gpui_kit::rgb(0xf0f0f0))
                .opacity(0.5)
                .child(crate::assets::icon(&crate::assets::icon_path("link"), 16., cx))
                .into_any_element(),
            Some((tab, _)) => Button::new("sp-ref-link")
                .ghost()
                .small()
                .w(px(26.))
                .h(px(26.))
                .p_0()
                .child(crate::assets::icon(&crate::assets::icon_path("link"), 16., cx))
                .tooltip("Switch to reference")
                .on_click(move |_, w, cx| crate::workspace::request_activate_tab(tab, w, cx))
                .into_any_element(),
        });
        Some(
            v_flex()
                .id("dr-sheet-props")
                .absolute()
                .top(px(DIALOG_TOP))
                .left(px(self.dialog_left()))
                .w(px(299.))
                .bg(pal.dialog_bg)
                .shadow(crate::drawing_chrome::dialog_shadow())
                .text_color(pal.text)
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    h_flex()
                        .h(px(28.))
                        .pl(px(11.))
                        .border_b_1()
                        .border_color(gpui_kit::rgb(0xd3d7d9))
                        .child(div().flex_1().text_size(px(13.)).font_weight(FontWeight::MEDIUM).child("Sheet properties"))
                        .child(
                            div()
                                .id("sp-ok")
                                .w(px(28.))
                                .h(px(27.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .bg(gpui_kit::rgb(0x009500))
                                .cursor_pointer()
                                .child(crate::ui::glyph("check", 18., gpui_kit::white()))
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.accept_sheet_props(cx))),
                        )
                        .child(
                            div()
                                .id("sp-cancel")
                                .w(px(28.))
                                .h(px(27.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .child(crate::drawing_chrome::cancel_x(cx))
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.sheet_props = None;
                                    cx.notify();
                                })),
                        ),
                )
                .child(
                    v_flex()
                        .p(px(3.))
                        .child(row("Scale", scale_field, None))
                        .child(row("Format", dropdown("sp-format", format_label, format_items).into_any_element(), Some(dwt)))
                        .child(row("Size", dropdown("sp-size", size_label, size_items).into_any_element(), None))
                        .child(row("Reference", div().when(unassigned, |d| d.text_color(gpui_kit::rgb(0xaaaaaa))).child(dropdown("sp-ref", ref_label, ref_items)).into_any_element(), link)),
                )
                .into_any_element(),
        )
    }
}
