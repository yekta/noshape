//! Drawing editor: Onshape's Drawing properties panel (empty-sheet menu ›
//! Drawing properties…), docked at the right: eight tab icons; Units and
//! precision (Primary units, decimal separator, precisions; leading and
//! trailing zeros), Dimensions / Annotations (font, text, color,
//! arrowheads; gaps and extensions), Views (insert view defaults), Tables
//! (cell padding) and Inspection (item parsing, text height, border shape
//! and size, exclude callouts) edit [`DrawingProps`].

use crate::drawing_view::*;
use crate::theme::p;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use crate::os_ui::OsDropdownExt as _;
use gpui_kit::component::{Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::drawing::format::default_digits;
use noshape_model::drawing::sheet::AnnType;
use noshape_model::drawing::*;

/// A number field and its subscription.
pub type NumberField = (Entity<InputState>, Subscription);

/// Drawing properties panel: open, the selected tab and the Inspection
/// tab's number fields (text height, border size).
#[derive(Default)]
pub struct DrawingPropsUi {
    pub open: bool,
    pub tab: usize,
    /// The open tab's number fields: (tab, fields).
    pub inspection_inputs: Option<(usize, Vec<NumberField>)>,
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

/// Drawing color choices (None = black, the default ink).
pub const COLORS: [(&str, Option<[u8; 3]>); 9] = [
    ("Black", None),
    ("Red", Some([220, 40, 40])),
    ("Orange", Some([240, 140, 20])),
    ("Yellow", Some([230, 200, 0])),
    ("Green", Some([30, 160, 60])),
    ("Blue", Some([22, 81, 176])),
    ("Purple", Some([130, 60, 180])),
    ("Gray", Some([128, 128, 128])),
    ("White", Some([255, 255, 255])),
];

/// "0", "0.1", "0.12"... for 0..=6 decimals.
fn precision_label(digits: usize) -> String {
    if digits == 0 { "0".into() } else { format!("0.{}", &"123456"[..digits]) }
}

type Act = std::rc::Rc<dyn Fn(&mut DrawingProps)>;

/// Which text a Font row sets.
#[derive(Clone, Copy)]
enum FontOf {
    Dims,
    Annotations,
    Inspection,
    Type(AnnType),
}

fn font_of(p: &mut DrawingProps, w: FontOf) -> &mut Option<noshape_model::drawing::sheet::TextFont> {
    match w {
        FontOf::Dims => &mut p.dim_font,
        FontOf::Annotations => &mut p.ann_font,
        FontOf::Inspection => &mut p.inspection_font,
        FontOf::Type(t) => &mut p.ann_type_mut(t).font,
    }
}

/// Element ids of the per-type rows (font select, bold, italic, color).
const TYPE_IDS: [[&str; 4]; 6] = [
    ["dp-t0-font", "dp-t0-b", "dp-t0-i", "dp-t0-color"],
    ["dp-t1-font", "dp-t1-b", "dp-t1-i", "dp-t1-color"],
    ["dp-t2-font", "dp-t2-b", "dp-t2-i", "dp-t2-color"],
    ["dp-t3-font", "dp-t3-b", "dp-t3-i", "dp-t3-color"],
    ["dp-t4-font", "dp-t4-b", "dp-t4-i", "dp-t4-color"],
    ["dp-t5-font", "dp-t5-b", "dp-t5-i", "dp-t5-color"],
];

impl DrawingEditor {
    pub fn open_drawing_props(&mut self, cx: &mut Context<Self>) {
        self.drawing_props.open = true;
        self.right_panel = None;
        cx.notify();
    }

    /// The open tab's number fields (made from render; Enter / blur
    /// applies): Dimensions and Annotations text height and arrowhead,
    /// Inspection text height and border size.
    pub fn sync_drawprops_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let tab = self.drawing_props.tab;
        if !self.drawing_props.open || ![1, 2, 3, 4, 6, 7].contains(&tab) {
            self.drawing_props.inspection_inputs = None;
            return;
        }
        if self.drawing_props.inspection_inputs.as_ref().is_some_and(|i| i.0 == tab) {
            return;
        }
        let p = &self.drawing.props;
        let field = |v: f64, set: fn(&mut DrawingProps, f64), window: &mut Window, cx: &mut Context<Self>| {
            let st = cx.new(|cx| InputState::new(window, cx).default_value(format!("{} mm", noshape_model::drawing::format::format_number(v, None, 2))));
            let sub = cx.subscribe(&st, move |this: &mut Self, st, ev: &InputEvent, cx| {
                if matches!(ev, InputEvent::PressEnter { .. } | InputEvent::Blur)
                    && let Some(v) = st.read(cx).value().trim().trim_end_matches("mm").trim().parse::<f64>().ok().filter(|v| *v > 0.0)
                {
                    this.set_drawing_prop(&move |p| set(p, v), cx);
                }
            });
            (st, sub)
        };
        // Text fields (label prefixes): applied on Enter / blur.
        let text_field = |v: String, set: fn(&mut DrawingProps, String), window: &mut Window, cx: &mut Context<Self>| {
            let st = cx.new(|cx| InputState::new(window, cx).default_value(v));
            let sub = cx.subscribe(&st, move |this: &mut Self, st, ev: &InputEvent, cx| {
                if matches!(ev, InputEvent::PressEnter { .. } | InputEvent::Blur) {
                    let v = st.read(cx).value().to_string();
                    this.set_drawing_prop(&move |p| set(p, v.clone()), cx);
                }
            });
            (st, sub)
        };
        let vt = p.view_tab.clone();
        let fields = match tab {
            3 => vec![
                field(vt.hatch_spacing, |p, v| p.view_tab.hatch_spacing = v, window, cx),
                text_field(vt.section_prefix, |p, v| p.view_tab.section_prefix = v, window, cx),
                text_field(vt.detail_prefix, |p, v| p.view_tab.detail_prefix = v, window, cx),
                text_field(vt.aux_prefix, |p, v| p.view_tab.aux_prefix = v, window, cx),
                text_field(vt.aux_suffix, |p, v| p.view_tab.aux_suffix = v, window, cx),
            ],
            4 => vec![field(vt.centermark_extension, |p, v| p.view_tab.centermark_extension = v, window, cx)],
            1 => {
                let (h, a, tg, gg, ex) = (p.dim_text_height, p.dim_arrow, p.dim_text_gap, p.dim_geometry_gap, p.dim_extension);
                vec![
                    field(h, |p, v| p.dim_text_height = v, window, cx),
                    field(a, |p, v| p.dim_arrow = v, window, cx),
                    field(tg, |p, v| p.dim_text_gap = v, window, cx),
                    field(gg, |p, v| p.dim_geometry_gap = v, window, cx),
                    field(ex, |p, v| p.dim_extension = v, window, cx),
                ]
            }
            2 => {
                let (h, a) = (p.ann_text_height, p.ann_arrow);
                let mut f = vec![field(h, |p, v| p.ann_text_height = v, window, cx), field(a, |p, v| p.ann_arrow = v, window, cx)];
                // Per type text heights (shown: the type's, else All annotations'), then Datum size.
                let setters: [fn(&mut DrawingProps, f64); 6] = [
                    |p, v| p.ann_type_mut(AnnType::ALL[0]).height = Some(v),
                    |p, v| p.ann_type_mut(AnnType::ALL[1]).height = Some(v),
                    |p, v| p.ann_type_mut(AnnType::ALL[2]).height = Some(v),
                    |p, v| p.ann_type_mut(AnnType::ALL[3]).height = Some(v),
                    |p, v| p.ann_type_mut(AnnType::ALL[4]).height = Some(v),
                    |p, v| p.ann_type_mut(AnnType::ALL[5]).height = Some(v),
                ];
                for (t, set) in AnnType::ALL.iter().zip(setters) {
                    f.push(field(p.ann_type(*t).height.unwrap_or(h), set, window, cx));
                }
                let ds = p.datum_size;
                f.push(field(ds, |p, v| p.datum_size = v, window, cx));
                let cd = p.com_diameter;
                f.push(field(cd, |p, v| p.com_diameter = v, window, cx));
                f
            }
            6 => {
                let (h, v) = (p.table_h_padding, p.table_v_padding);
                vec![field(h, |p, v| p.table_h_padding = v, window, cx), field(v, |p, v| p.table_v_padding = v, window, cx)]
            }
            _ => {
                let (h, b) = (p.inspection_text_height, p.inspection_border_size);
                vec![field(h, |p, v| p.inspection_text_height = v, window, cx), field(b, |p, v| p.inspection_border_size = v, window, cx)]
            }
        };
        self.drawing_props.inspection_inputs = Some((tab, fields));
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
                .os_dropdown_menu(SharedString::from(format!("{id}-dd")), true, move |mut menu, _, _| {
                    for (label, checked, f) in &items {
                        let (e, f) = (ed.clone(), f.clone());
                        menu = menu.item(crate::os_menu::OsMenuItem::new(label.clone()).checked(*checked).on_click(move |_, _, cx| e.update(cx, |this, cx| this.set_drawing_prop(&*f, cx))));
                    }
                    menu
                })
                .into_any_element()
        };
        // Color swatch: its color and name; a click opens the color dialog.
        let swatch = |id: &'static str, cur: Option<[u8; 3]>, set: crate::drawing_color::ColorSet| -> AnyElement {
            let ed = ed.clone();
            let c = cur.unwrap_or([0, 0, 0]);
            let name = COLORS.iter().find(|x| x.1 == cur).map(|x| x.0.to_string()).unwrap_or_else(|| format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2]));
            h_flex()
                .id(id)
                .w(px(160.))
                .gap(px(6.))
                .cursor_pointer()
                .child(div().w(px(28.)).h(px(16.)).bg(gpui_kit::rgb(((c[0] as u32) << 16) | ((c[1] as u32) << 8) | c[2] as u32)).border_1().border_color(gpui_kit::rgb(0x999999)))
                .child(div().text_size(px(12.)).child(name))
                .on_click(move |_, w, cx| {
                    let set = set.clone();
                    ed.update(cx, |t, cx| t.open_color_dialog(cur, set, w, cx))
                })
                .into_any_element()
        };
        // Font row: family, Bold, Italic (the default is the drawing sans).
        let font_row = |w: FontOf, cur: Option<noshape_model::drawing::sheet::TextFont>, cx: &mut Context<Self>| -> AnyElement {
            use noshape_model::drawing::sheet::{FONT_FAMILIES, TextFont};
            let label = cur.as_ref().map(|f| f.family.clone()).unwrap_or_else(|| "Default".into());
            let mut items: Vec<(String, bool, Act)> = vec![("Default".into(), cur.is_none(), std::rc::Rc::new(move |p: &mut DrawingProps| *font_of(p, w) = None) as Act)];
            for fam in FONT_FAMILIES {
                let on = cur.as_ref().is_some_and(|f| f.family == fam);
                items.push((fam.to_string(), on, std::rc::Rc::new(move |p: &mut DrawingProps| {
                    let f = font_of(p, w);
                    let (b, i) = f.as_ref().map(|f| (f.bold, f.italic)).unwrap_or((false, false));
                    *f = Some(TextFont { family: fam.into(), bold: b, italic: i });
                }) as Act));
            }
            let toggle = |id: &'static str, glyph: &'static str, on: bool, f: fn(&mut TextFont), cx: &mut Context<Self>| {
                div()
                    .id(id)
                    .size(px(24.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(13.))
                    .font_weight(FontWeight::BOLD)
                    .cursor_pointer()
                    .rounded(px(2.))
                    .when(on, |d| d.bg(gpui_kit::rgb(0xcfdbe5)))
                    .child(glyph)
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.set_drawing_prop(&move |p| {
                            let slot = font_of(p, w);
                            let mut font = slot.clone().unwrap_or_else(|| TextFont { family: FONT_FAMILIES[0].into(), bold: false, italic: false });
                            f(&mut font);
                            *slot = Some(font);
                        }, cx)
                    }))
            };
            let (b, i) = cur.as_ref().map(|f| (f.bold, f.italic)).unwrap_or((false, false));
            let id = match w {
                FontOf::Dims => ("dp-dfont", "dp-dfont-b", "dp-dfont-i"),
                FontOf::Annotations => ("dp-afont", "dp-afont-b", "dp-afont-i"),
                FontOf::Inspection => ("dp-ifont", "dp-ifont-b", "dp-ifont-i"),
                FontOf::Type(t) => {
                    let ids = TYPE_IDS[AnnType::ALL.iter().position(|x| *x == t).unwrap_or(0)];
                    (ids[0], ids[1], ids[2])
                }
            };
            h_flex()
                .gap(px(2.))
                .child(select(id.0, label, items))
                .child(toggle(id.1, "B", b, |f| f.bold = !f.bold, cx))
                .child(toggle(id.2, "I", i, |f| f.italic = !f.italic, cx))
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
        } else if self.drawing_props.tab == 1 || self.drawing_props.tab == 2 {
            // Dimensions › All dimensions / Annotations › All annotations.
            let dims = self.drawing_props.tab == 1;
            let field = |i: usize| match self.drawing_props.inspection_inputs.as_ref().and_then(|(_, f)| f.get(i)) {
                Some(f) => div().w(px(160.)).child(Input::new(&f.0).small()).into_any_element(),
                None => div().into_any_element(),
            };
            let (color, arrow) = if dims { (props.dim_color, props.dim_arrow_type) } else { (props.ann_color, props.ann_arrow_type) };
            let arrow_items: Vec<(String, bool, Act)> = noshape_model::drawing::sheet::ArrowType::ALL
                .iter()
                .map(|&t| (t.name().to_string(), t == arrow, std::rc::Rc::new(move |p: &mut DrawingProps| if dims { p.dim_arrow_type = t } else { p.ann_arrow_type = t }) as Act))
                .collect();
            body = body
                .child(div().h(px(32.)).px(px(12.)).flex().items_center().text_size(px(14.67)).font_weight(FontWeight::SEMIBOLD).child(if dims { "Dimensions" } else { "Annotations" }))
                .child(section(if dims { "All dimensions" } else { "All annotations" }))
                .child(row("Font", font_row(if dims { FontOf::Dims } else { FontOf::Annotations }, if dims { props.dim_font.clone() } else { props.ann_font.clone() }, cx)))
                .child(row("Text height", field(0)))
                .child(row("Color", swatch(if dims { "dp-dcolor" } else { "dp-acolor" }, color, std::rc::Rc::new(move |p: &mut DrawingProps, c| if dims { p.dim_color = c } else { p.ann_color = c }))))
                .child(row("Arrowhead", field(1)))
                .child(row("Arrowhead type", select(if dims { "dp-darrow" } else { "dp-aarrow" }, arrow.name().into(), arrow_items)));
            if !dims {
                for (i, t) in AnnType::ALL.iter().copied().enumerate() {
                    let ts = props.ann_type(t);
                    let ids = TYPE_IDS[i];
                    let cur = ts.color;
                    body = body
                        .child(section(t.name()))
                        .child(row("Font", font_row(FontOf::Type(t), ts.font.clone(), cx)))
                        .child(row("Text height", field(2 + i)))
                        .child(row("Color", swatch(ids[3], cur, std::rc::Rc::new(move |p: &mut DrawingProps, c| p.ann_type_mut(t).color = c))));
                    if t == AnnType::Notes {
                        let hide = props.hide_placeholder;
                        body = body.child(div().h(px(37.)).px(px(10.)).flex().items_center().child(Checkbox::new("dp-hide-ph").label("Hide property placeholder").checked(hide).on_click(cx.listener(|this, v: &bool, _, cx| {
                            let v = *v;
                            this.set_drawing_prop(&move |p| p.hide_placeholder = v, cx);
                        }))));
                    }
                    if t == AnnType::Datums {
                        body = body.child(row("Datum size", field(8)));
                    }
                    if t == AnnType::Callouts {
                        body = body.child(section("Center of mass")).child(row("Indicator diameter", field(9)));
                    }
                }
            }
            if dims {
                let horiz = props.dim_text_horizontal;
                let align_items: Vec<(String, bool, Act)> = vec![
                    ("With dimension line".into(), !horiz, std::rc::Rc::new(|p: &mut DrawingProps| p.dim_text_horizontal = false) as Act),
                    ("Horizontal".into(), horiz, std::rc::Rc::new(|p: &mut DrawingProps| p.dim_text_horizontal = true) as Act),
                ];
                body = body
                    .child(section("Gaps and extensions"))
                    .child(row("Text alignment", select("dp-dalign", if horiz { "Horizontal".into() } else { "With dimension line".into() }, align_items)))
                    .child(row("Text gap", field(2)))
                    .child(row("Geometry gap", field(3)))
                    .child(row("Extension past line", field(4)));
            }
        } else if self.drawing_props.tab == 3 {
            let check = |id: &'static str, label: &'static str, on: bool, f: fn(&mut DrawingProps, bool), cx: &mut Context<Self>| {
                div().h(px(37.)).px(px(10.)).flex().items_center().child(Checkbox::new(id).label(label).checked(on).on_click(cx.listener(move |this, v: &bool, _, cx| {
                    let v = *v;
                    this.set_drawing_prop(&move |p| f(p, v), cx);
                })))
            };
            let vd = props.view_defaults.clone();
            let proj = self.drawing.projection;
            let ed2 = ed.clone();
            let proj_btn = Button::new("dp-proj")
                .label(if proj == Projection::ThirdAngle { "Third angle" } else { "First angle" })
                .ghost()
                .small()
                .w(px(160.))
                .dropdown_caret(true)
                .border_b_1()
                .border_color(gpui_kit::rgb(0xdddddd))
                .rounded(px(0.))
                .os_dropdown_menu("dp-projection-dd", true, move |m, _, _| {
                    let (a, b) = (ed2.clone(), ed2.clone());
                    m.item(crate::os_menu::OsMenuItem::new("Third angle").checked(proj == Projection::ThirdAngle).on_click(move |_, _, cx| a.update(cx, |t, cx| t.edit("Projection angle", cx, |d| d.projection = Projection::ThirdAngle))))
                        .item(crate::os_menu::OsMenuItem::new("First angle").checked(proj == Projection::FirstAngle).on_click(move |_, _, cx| b.update(cx, |t, cx| t.edit("Projection angle", cx, |d| d.projection = Projection::FirstAngle))))
                })
                .into_any_element();
            let simp: [(&str, Simplification); 5] = [("None", Simplification::None), ("Auto", Simplification::Auto), ("Absolute", Simplification::Absolute(0.001)), ("Ratio to studio", Simplification::RatioToStudio(0.01)), ("Ratio to part", Simplification::RatioToPart(0.01))];
            let simp_label = simp.iter().find(|s| std::mem::discriminant(&s.1) == std::mem::discriminant(&vd.simplification)).map(|s| s.0).unwrap_or("Auto");
            let simp_items: Vec<(String, bool, Act)> = simp.iter().map(|&(n, s)| (n.to_string(), n == simp_label, std::rc::Rc::new(move |p: &mut DrawingProps| p.view_defaults.simplification = s) as Act)).collect();
            let hidden_items: Vec<(String, bool, Act)> = vec![
                ("Hidden".into(), !vd.hidden_lines, std::rc::Rc::new(|p: &mut DrawingProps| p.view_defaults.hidden_lines = false) as Act),
                ("Shown".into(), vd.hidden_lines, std::rc::Rc::new(|p: &mut DrawingProps| p.view_defaults.hidden_lines = true) as Act),
            ];
            let tangent_items: Vec<(String, bool, Act)> = ["Hidden", "Solid", "Phantom"].iter().enumerate().map(|(i, n)| (n.to_string(), vd.tangent == i as u8, std::rc::Rc::new(move |p: &mut DrawingProps| p.view_defaults.tangent = i as u8) as Act)).collect();
            body = body
                .child(div().h(px(32.)).px(px(12.)).flex().items_center().text_size(px(14.67)).font_weight(FontWeight::SEMIBOLD).child("Views"))
                .child(section("All views"))
                .child(row("Projection angle", proj_btn))
                .child(section("Insert view defaults"))
                .child(row("View simplification", select("dp-vsimp", simp_label.into(), simp_items)))
                .child(check("dp-vcm", "Auto centermarks (Parts)", vd.auto_centermarks, |p, v| p.view_defaults.auto_centermarks = v, cx))
                .child(check("dp-vcl", "Auto centerlines (Parts)", vd.auto_centerlines, |p, v| p.view_defaults.auto_centerlines = v, cx))
                .child(row("Hidden lines", select("dp-vhidden", if vd.hidden_lines { "Shown".into() } else { "Hidden".into() }, hidden_items)))
                .child(row("Tangent edges", select("dp-vtan", ["Hidden", "Solid", "Phantom"][vd.tangent.min(2) as usize].into(), tangent_items)))
                .child(check("dp-vscale", "Scale label", vd.scale_label, |p, v| p.view_defaults.scale_label = v, cx));
            // Per view type.
            let vt = props.view_tab.clone();
            let field = |i: usize| match self.drawing_props.inspection_inputs.as_ref().and_then(|(_, f)| f.get(i)) {
                Some(f) => div().w(px(160.)).child(Input::new(&f.0).small()).into_any_element(),
                None => div().into_any_element(),
            };
            let detail_lines: Vec<(String, bool, Act)> = ["Phantom", "Dashed", "Solid"].iter().enumerate().map(|(i, n)| (n.to_string(), vt.detail_line == i as u8, std::rc::Rc::new(move |p: &mut DrawingProps| p.view_tab.detail_line = i as u8) as Act)).collect();
            let detail_styles: Vec<(String, bool, Act)> = ["In line", "Connected", "Leader"].iter().enumerate().map(|(i, n)| (n.to_string(), vt.detail_style == i as u8, std::rc::Rc::new(move |p: &mut DrawingProps| p.view_tab.detail_style = i as u8) as Act)).collect();
            let aux_styles: Vec<(String, bool, Act)> = vec![
                ("A".into(), !vt.aux_double_label, std::rc::Rc::new(|p: &mut DrawingProps| p.view_tab.aux_double_label = false) as Act),
                ("A-A".into(), vt.aux_double_label, std::rc::Rc::new(|p: &mut DrawingProps| p.view_tab.aux_double_label = true) as Act),
            ];
            body = body
                .child(section("Hidden edges"))
                .child(row("Color", swatch("dp-hcolor", vt.hidden_color, std::rc::Rc::new(|p: &mut DrawingProps, c| p.view_tab.hidden_color = c))))
                .child(section("Tangent edges"))
                .child(row("Color", swatch("dp-tcolor", vt.tangent_color, std::rc::Rc::new(|p: &mut DrawingProps, c| p.view_tab.tangent_color = c))))
                .child(section("Section views"))
                .child(row("Hatch spacing", field(0)))
                .child(row("Label prefix", field(1)))
                .child(section("Detail views"))
                .child(row("Profile lines", select("dp-dline", ["Phantom", "Dashed", "Solid"][vt.detail_line.min(2) as usize].into(), detail_lines)))
                .child(row("Label prefix", field(2)))
                .child(row("Style", select("dp-dstyle", ["In line", "Connected", "Leader"][vt.detail_style.min(2) as usize].into(), detail_styles)))
                .child(check("dp-dprofile", "Show detail view profile", vt.detail_show_profile, |p, v| p.view_tab.detail_show_profile = v, cx))
                .child(check("dp-dclip", "Clip border to geometry", vt.detail_clip_border, |p, v| p.view_tab.detail_clip_border = v, cx))
                .child(section("Auxiliary views"))
                .child(check("dp-aind", "Show view reference indicator", vt.aux_indicator, |p, v| p.view_tab.aux_indicator = v, cx))
                .child(row("View label prefix", field(3)))
                .child(row("View label style", select("dp-alabel", if vt.aux_double_label { "A-A".into() } else { "A".into() }, aux_styles)))
                .child(row("View label suffix", field(4)));
        } else if self.drawing_props.tab == 4 {
            let vt = props.view_tab.clone();
            let field = |i: usize| match self.drawing_props.inspection_inputs.as_ref().and_then(|(_, f)| f.get(i)) {
                Some(f) => div().w(px(160.)).child(Input::new(&f.0).small()).into_any_element(),
                None => div().into_any_element(),
            };
            body = body
                .child(div().h(px(32.)).px(px(12.)).flex().items_center().text_size(px(14.67)).font_weight(FontWeight::SEMIBOLD).child("Construction geometry"))
                .child(section("Centermarks"))
                .child(row("Color", swatch("dp-cmcolor", vt.centermark_color, std::rc::Rc::new(|p: &mut DrawingProps, c| p.view_tab.centermark_color = c))))
                .child(row("Mark size", field(0)))
                .child(section("Centerlines"))
                .child(row("Color", swatch("dp-clcolor", vt.centerline_color, std::rc::Rc::new(|p: &mut DrawingProps, c| p.view_tab.centerline_color = c))));
        } else if self.drawing_props.tab == 5 {
            let color_select = |id: &'static str, cur: Option<[u8; 3]>, set: fn(&mut DrawingProps, Option<[u8; 3]>)| swatch(id, cur, std::rc::Rc::new(move |p: &mut DrawingProps, c| set(p, c)));
            let locked = props.formats_locked;
            body = body
                .child(div().h(px(32.)).px(px(12.)).flex().items_center().text_size(px(14.67)).font_weight(FontWeight::SEMIBOLD).child("Formats"))
                .child(div().h(px(37.)).px(px(10.)).flex().items_center().child(Checkbox::new("dp-flock").label("Locked").checked(locked).on_click(cx.listener(|this, v: &bool, _, cx| {
                    let v = *v;
                    this.set_drawing_prop(&move |p| p.formats_locked = v, cx);
                }))))
                .child(section("Border frame"))
                .child(row("Color", color_select("dp-fborder", props.border_color, |p, c| p.border_color = c)))
                .child(section("Border zone"))
                .child(row("Color", color_select("dp-fzone", props.zone_color, |p, c| p.zone_color = c)))
                .child(section("Title block"))
                .child(row("Color", color_select("dp-ftitle", props.title_block_color, |p, c| p.title_block_color = c)));
        } else if self.drawing_props.tab == 6 {
            let field = |i: usize| match self.drawing_props.inspection_inputs.as_ref().and_then(|(_, f)| f.get(i)) {
                Some(f) => div().w(px(160.)).child(Input::new(&f.0).small()).into_any_element(),
                None => div().into_any_element(),
            };
            body = body
                .child(div().h(px(32.)).px(px(12.)).flex().items_center().text_size(px(14.67)).font_weight(FontWeight::SEMIBOLD).child("Tables"))
                .child(section("All tables"))
                .child(row("Horizontal padding", field(0)))
                .child(row("Vertical padding", field(1)));
        } else if self.drawing_props.tab == 7 {
            let check = |id: &'static str, label: &'static str, on: bool, f: fn(&mut DrawingProps, bool), cx: &mut Context<Self>| {
                div().h(px(37.)).px(px(10.)).flex().items_center().child(Checkbox::new(id).label(label).checked(on).on_click(cx.listener(move |this, v: &bool, _, cx| {
                    let v = *v;
                    this.set_drawing_prop(&move |p| f(p, v), cx);
                })))
            };
            let parse_items: Vec<(String, bool, Act)> = vec![
                ("Parameter".into(), !props.inspection_row_parsing, std::rc::Rc::new(|p: &mut DrawingProps| p.inspection_row_parsing = false) as Act),
                ("Row".into(), props.inspection_row_parsing, std::rc::Rc::new(|p: &mut DrawingProps| p.inspection_row_parsing = true) as Act),
            ];
            let shape_items: Vec<(String, bool, Act)> = InspectionShape::ALL.iter().map(|&s| (s.name().to_string(), s == props.inspection_shape, std::rc::Rc::new(move |p: &mut DrawingProps| p.inspection_shape = s) as Act)).collect();
            let field = |i: usize| match self.drawing_props.inspection_inputs.as_ref().and_then(|(_, f)| f.get(i)) {
                Some(f) => div().w(px(160.)).child(Input::new(&f.0).small()).into_any_element(),
                None => div().into_any_element(),
            };
            body = body
                .child(div().h(px(32.)).px(px(12.)).flex().items_center().text_size(px(14.67)).font_weight(FontWeight::SEMIBOLD).child("Inspection"))
                .child(row("Item parsing", select("dp-iparse", if props.inspection_row_parsing { "Row".into() } else { "Parameter".into() }, parse_items)))
                .child(section("Text"))
                .child(row("Font", font_row(FontOf::Inspection, props.inspection_font.clone(), cx)))
                .child(row("Text height", field(0)))
                .child(section("Border"))
                .child(row("Border shape", select("dp-ishape", props.inspection_shape.name().into(), shape_items)))
                .child(row("Border size", field(1)))
                .child(row(
                    "Border color",
                    swatch("dp-icolor", props.inspection_color, std::rc::Rc::new(|p: &mut DrawingProps, c| p.inspection_color = c)),
                ))
                .child(check("dp-isolid", "Solid fill", props.inspection_solid_fill, |p, v| p.inspection_solid_fill = v, cx))
                .child(section("Automatic"))
                .child(check("dp-iexcl", "Exclude callouts", props.inspection_exclude_callouts, |p, v| p.inspection_exclude_callouts = v, cx));
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
