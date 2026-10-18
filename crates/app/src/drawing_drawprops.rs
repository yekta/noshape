//! Drawing editor: Onshape's Drawing properties panel (empty-sheet menu ›
//! Drawing properties…), docked at the right: eight tab icons; Units and
//! precision (Primary units, decimal separator, precisions; leading and
//! trailing zeros) edits [`DrawingProps`].

use crate::drawing_view::*;
use crate::theme::p;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::{Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::drawing::format::default_digits;
use noshape_model::drawing::*;

/// Drawing properties panel: open and the selected tab.
#[derive(Clone, Copy, Debug, Default)]
pub struct DrawingPropsUi {
    pub open: bool,
    pub tab: usize,
}

/// Onshape's tabs (tooltip, icon).
const TABS: [(&str, &str); 8] = [
    ("Units and precision", "linear-dim"),
    ("Dimensions", "dimension"),
    ("Annotations", "text"),
    ("Views", "base-view"),
    ("Construction geometry", "centerline"),
    ("Formats", "title-block"),
    ("Tables", "table"),
    ("Inspection", "measure"),
];

/// "0", "0.1", "0.12"... for 0..=6 decimals.
fn precision_label(digits: usize) -> String {
    if digits == 0 { "0".into() } else { format!("0.{}", &"123456"[..digits]) }
}

type Act = std::rc::Rc<dyn Fn(&mut DrawingProps)>;

impl DrawingEditor {
    pub fn open_drawing_props(&mut self, cx: &mut Context<Self>) {
        self.drawing_props.open = true;
        cx.notify();
    }

    fn set_drawing_prop(&mut self, f: &dyn Fn(&mut DrawingProps), cx: &mut Context<Self>) {
        self.edit("Drawing properties", cx, |d| {
            f(&mut d.props);
            // Keep the metric flag in step with the chosen units.
            if let Some(u) = d.props.units {
                d.units_mm = !matches!(u, DimUnits::Inch | DimUnits::Foot);
            }
        });
    }

    pub fn render_drawing_props(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.drawing_props.open {
            return None;
        }
        let pal = p();
        let ed = cx.entity();
        let props = self.drawing.props.clone();
        let units = props.units.unwrap_or(if self.drawing.units_mm { DimUnits::Millimeter } else { DimUnits::Inch });
        let select = |id: &'static str, current: String, items: Vec<(String, bool, Act)>| {
            let ed = ed.clone();
            Button::new(id)
                .label(current)
                .ghost()
                .small()
                .w(px(160.))
                .dropdown_caret(true)
                .border_b_1()
                .border_color(gpui_kit::rgb(0xdddddd))
                .rounded(px(0.))
                .dropdown_menu(move |mut menu, _, _| {
                    for (label, checked, f) in &items {
                        let (e, f) = (ed.clone(), f.clone());
                        menu = menu.item(PopupMenuItem::new(label.clone()).checked(*checked).on_click(move |_, _, cx| e.update(cx, |this, cx| this.set_drawing_prop(&*f, cx))));
                    }
                    menu
                })
                .into_any_element()
        };
        let row = |label: &'static str, body: AnyElement| h_flex().h(px(33.)).px(px(12.)).child(div().flex_1().text_size(px(13.33)).text_color(pal.text_muted).child(label)).child(body).into_any_element();
        let section = |title: &'static str| div().h(px(30.)).px(px(12.)).flex().items_center().bg(gpui_kit::rgb(0xefefef)).border_t_1().border_color(pal.border).text_size(px(13.33)).font_weight(FontWeight::SEMIBOLD).child(title).into_any_element();
        let precision_items = |cur: Option<u8>, max: usize, set: fn(&mut DrawingProps, Option<u8>)| -> Vec<(String, bool, Act)> {
            (0..=max).map(|k| (precision_label(k), cur == Some(k as u8), std::rc::Rc::new(move |p: &mut DrawingProps| set(p, Some(k as u8))) as Act)).collect()
        };
        let shown = |cur: Option<u8>, default: usize| precision_label(cur.map(|c| c as usize).unwrap_or(default));
        let mut body = v_flex().id("dr-dprops-body").flex_1().min_h_0().overflow_y_scroll();
        if self.drawing_props.tab == 0 {
            let unit_items: Vec<(String, bool, Act)> = DimUnits::ALL
                .iter()
                .map(|&u| {
                    let label = match u {
                        DimUnits::Millimeter => "Millimeters",
                        DimUnits::Centimeter => "Centimeters",
                        DimUnits::Meter => "Meters",
                        DimUnits::Inch => "Inches",
                        DimUnits::Foot => "Feet",
                    };
                    (label.to_string(), u == units, std::rc::Rc::new(move |p: &mut DrawingProps| p.units = Some(u)) as Act)
                })
                .collect();
            let unit_label = unit_items.iter().find(|i| i.1).map(|i| i.0.clone()).unwrap_or_default();
            let sep_items: Vec<(String, bool, Act)> = vec![
                ("Period".into(), !props.decimal_comma, std::rc::Rc::new(|p: &mut DrawingProps| p.decimal_comma = false) as Act),
                ("Comma".into(), props.decimal_comma, std::rc::Rc::new(|p: &mut DrawingProps| p.decimal_comma = true) as Act),
            ];
            let check = |id: &'static str, label: &'static str, on: bool, f: fn(&mut DrawingProps, bool), cx: &mut Context<Self>| {
                div().h(px(37.)).px(px(10.)).flex().items_center().child(Checkbox::new(id).label(label).checked(on).on_click(cx.listener(move |this, v: &bool, _, cx| {
                    let v = *v;
                    this.set_drawing_prop(&move |p| f(p, v), cx);
                })))
            };
            body = body
                .child(div().h(px(32.)).px(px(12.)).flex().items_center().text_size(px(14.67)).font_weight(FontWeight::SEMIBOLD).child("Units and precision"))
                .child(section("Primary"))
                .child(row("Units", select("dp-units", unit_label, unit_items)))
                .child(row("Decimal separator", select("dp-sep", if props.decimal_comma { "Comma".into() } else { "Period".into() }, sep_items)))
                .child(row("Precision", select("dp-prec", shown(props.precision, default_digits(units)), precision_items(props.precision, 6, |p, v| p.precision = v))))
                .child(row("Tolerance precision", select("dp-tprec", shown(props.tolerance_precision.or(props.precision), default_digits(units)), precision_items(props.tolerance_precision, 6, |p, v| p.tolerance_precision = v))))
                .child(row("Angular precision", select("dp-aprec", shown(props.angular_precision, 2), precision_items(props.angular_precision, 4, |p, v| p.angular_precision = v))))
                .child(row("Angular tolerance precision", select("dp-atprec", shown(props.angular_tolerance_precision, 2), precision_items(props.angular_tolerance_precision, 4, |p, v| p.angular_tolerance_precision = v))))
                .child(section("Leading and trailing zeros"))
                .child(check("dp-lz", "Length leading zeros", props.leading_zeros, |p, v| p.leading_zeros = v, cx))
                .child(check("dp-tz", "Length trailing zeros", props.trailing_zeros, |p, v| p.trailing_zeros = v, cx))
                .child(check("dp-tlz", "Length tolerance leading zeros", props.tolerance_leading_zeros, |p, v| p.tolerance_leading_zeros = v, cx))
                .child(check("dp-ttz", "Length tolerance trailing zeros", props.tolerance_trailing_zeros, |p, v| p.tolerance_trailing_zeros = v, cx));
        } else {
            body = body
                .child(div().h(px(32.)).px(px(12.)).flex().items_center().text_size(px(14.67)).font_weight(FontWeight::SEMIBOLD).child(TABS[self.drawing_props.tab].0))
                .child(div().px(px(12.)).py_2().text_size(px(12.)).text_color(pal.text_muted).child("These properties are set per item for now (select a dimension, note or table)."));
        }
        let mut tabs = h_flex().px(px(8.)).gap(px(1.));
        for (i, (tip, icon)) in TABS.iter().enumerate() {
            let on = i == self.drawing_props.tab;
            tabs = tabs.child(
                div()
                    .id(("dr-dprops-tab", i))
                    .size(px(32.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .border_1()
                    .border_color(pal.border)
                    .cursor_pointer()
                    .when(on, |d| d.bg(gpui_kit::white()).border_b_2().border_color(pal.accent))
                    .when(!on, |d| d.bg(gpui_kit::rgb(0xf2f2f2)).hover(|d| d.bg(pal.panel_hover)))
                    .tooltip(crate::ui::tooltip(*tip))
                    .child(crate::assets::icon(&crate::assets::icon_path(icon), 20., cx))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.drawing_props.tab = i;
                        cx.notify();
                    })),
            );
        }
        Some(
            v_flex()
                .id("dr-drawing-props")
                .absolute()
                .top_0()
                .right_0()
                .bottom_0()
                .w(px(370.))
                .bg(gpui_kit::rgb(0xfafafa))
                .border_l_1()
                .border_color(pal.border)
                .text_color(pal.text)
                .occlude()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    h_flex()
                        .h(px(40.))
                        .px(px(8.))
                        .child(div().flex_1().text_center().text_size(px(18.)).font_weight(FontWeight::MEDIUM).text_color(pal.accent).child("Drawing properties"))
                        .child(Button::new("dp-close").ghost().child(crate::ui::glyph("close", 14., pal.text_muted)).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.drawing_props.open = false;
                            cx.notify();
                        }))),
                )
                .child(tabs)
                .child(body)
                .into_any_element(),
        )
    }
}
