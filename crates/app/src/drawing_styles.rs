//! Drawing editor: the Styles panel (right strip "Styles"; live
//! drawing/474–477, dom/drawing-styles-*): with nothing selected a blue
//! hint; with dimensions selected Font, Text height, Color, Arrowhead +
//! type, Text justification, Text alignment, Text gap, Break dimension and
//! Show dimension line; with annotations Font, Text height, Color and
//! Arrowhead; with hatch regions Hatch pattern (group, pattern), Scale,
//! Angle and Color. Inherited values read "(Drawing)"; "Revert to drawing
//! properties" clears the selection's own styles.

use crate::drawing_view::*;
use crate::os_ui::OsDropdownExt as _;
use crate::theme::p;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::{Sizable as _, h_flex, v_flex};
use gpui_kit::*;
use noshape_model::drawing::sheet::{ArrowType, FONT_FAMILIES, TextFont};
use gpui_kit::prelude::FluentBuilder as _;
use noshape_model::drawing::sheet_sketch::HatchRegion;
use noshape_model::drawing::{AnnStyle, Annotation, AnnotationKind, TextJustify};

type Edit = std::rc::Rc<dyn Fn(&mut Annotation)>;
type HatchEdit = std::rc::Rc<dyn Fn(&mut HatchRegion)>;

impl DrawingEditor {
    fn styled_selection(&self) -> Vec<Annotation> {
        let sheet = self.current_sheet();
        self.selection.iter().filter_map(|s| if let Sel::Annotation(id) = s { sheet.annotations.iter().find(|a| a.id == *id).cloned() } else { None }).collect()
    }

    /// Apply `f` to every selected annotation (one undo step).
    fn edit_styles(&mut self, f: Edit, cx: &mut Context<Self>) {
        let ids: Vec<u64> = self.selection.iter().filter_map(|s| if let Sel::Annotation(id) = s { Some(*id) } else { None }).collect();
        self.edit_sheet("Style", cx, |s| {
            for a in s.annotations.iter_mut().filter(|a| ids.contains(&a.id)) {
                f(a);
                if a.style.as_ref().is_some_and(|st| st.is_empty()) {
                    a.style = None;
                }
            }
        });
    }

    /// Apply `f` to every selected hatch region (one undo step).
    fn edit_hatch_styles(&mut self, f: HatchEdit, cx: &mut Context<Self>) {
        let ids: Vec<u64> = self.selection.iter().filter_map(|s| if let Sel::Hatch(id) = s { Some(*id) } else { None }).collect();
        self.edit_sheet("Style", cx, |s| {
            for h in s.sketch.hatches.iter_mut().filter(|h| ids.contains(&h.id)) {
                f(h);
            }
        });
    }

    /// Hatch regions selected: Hatch pattern (group and pattern), Scale,
    /// Angle, Color; Revert puts the drawing's ANSI31 and color back.
    fn render_hatch_styles(&mut self, h: HatchRegion, cx: &mut Context<Self>) -> AnyElement {
        use noshape_model::drawing::hatch_pat::{self, GROUPS};
        let pal = p();
        let ed = cx.entity();
        let select = |id: &'static str, label: String, icons: bool, items: Vec<(String, bool, HatchEdit, Option<String>)>| -> AnyElement {
            let ed = ed.clone();
            Button::new(id).label(label).ghost().small().w(px(160.)).dropdown_caret(true).border_b_1().border_color(gpui_kit::rgb(0xdddddd)).rounded(px(0.)).os_dropdown_menu(SharedString::from(format!("{id}-dd")), true, move |mut m, _, _| {
                if icons {
                    m = m.select_list(160.);
                }
                for (l, on, f, icon) in &items {
                    let (e, f) = (ed.clone(), f.clone());
                    let mut it = crate::os_menu::OsMenuItem::new(l.clone()).checked(*on);
                    if let Some(i) = icon {
                        it = it.icon_name(i);
                    }
                    m = m.item(it.on_click(move |_, _, cx| e.update(cx, |t, cx| t.edit_hatch_styles(f.clone(), cx))));
                }
                m
            })
            .into_any_element()
        };
        let row = |label: &'static str, body: AnyElement| h_flex().h(px(33.)).px(px(12.)).child(div().flex_1().text_size(px(13.33)).text_color(pal.text_muted).child(label)).child(body).into_any_element();
        let groups: Vec<(String, bool, HatchEdit, Option<String>)> = GROUPS
            .iter()
            .enumerate()
            .map(|(g, (n, _))| {
                (n.to_string(), g == h.standard, std::rc::Rc::new(move |x: &mut HatchRegion| {
                    if x.standard != g {
                        x.standard = g;
                        x.pattern = hatch_pat::default_pattern(g);
                        x.angle = 0.0;
                    }
                }) as HatchEdit, None)
            })
            .collect();
        let std = h.standard;
        let patterns: Vec<(String, bool, HatchEdit, Option<String>)> = hatch_pat::display_order(std)
            .into_iter()
            .filter_map(|i| {
                let p = hatch_pat::pattern(std, i)?;
                Some((p.name.to_string(), i == h.pattern, std::rc::Rc::new(move |x: &mut HatchRegion| x.pattern = i) as HatchEdit, Some(hatch_pat::icon_name(std, i))))
            })
            .collect();
        let scales: Vec<(String, bool, HatchEdit, Option<String>)> = std::iter::once(("Auto".to_string(), 1.0))
            .chain((1..=10).map(|k| (format!("{k}.0"), k as f64)))
            .enumerate()
            .map(|(i, (l, v))| (l, (i == 0 && (h.scale - 1.0).abs() < 1e-9) || (i > 1 && (h.scale - v).abs() < 1e-9), std::rc::Rc::new(move |x: &mut HatchRegion| x.scale = v) as HatchEdit, None))
            .collect();
        let base = hatch_pat::base_angle(h.standard, h.pattern);
        let angles: Vec<(String, bool, HatchEdit, Option<String>)> = std::iter::once(("Auto".to_string(), None))
            .chain((0..=12).map(|k| (format!("{}.0°", k * 15), Some((k * 15) as f64))))
            .map(|(l, a)| {
                let on = match a {
                    None => h.angle.abs() < 1e-9,
                    Some(a) => h.angle.abs() > 1e-9 && ((h.angle + base) - a).abs() < 1e-6,
                };
                (l, on, std::rc::Rc::new(move |x: &mut HatchRegion| x.angle = a.map(|a| a - hatch_pat::base_angle(x.standard, x.pattern)).unwrap_or(0.0)) as HatchEdit, None)
            })
            .collect();
        let colors: Vec<(String, bool, HatchEdit, Option<String>)> = crate::drawing_drawprops::COLORS
            .iter()
            .map(|(n, c)| {
                let c = *c;
                (n.to_string(), h.color == c, std::rc::Rc::new(move |x: &mut HatchRegion| x.color = c) as HatchEdit, None)
            })
            .collect();
        let pat_label = hatch_pat::pattern(h.standard, h.pattern).map(|p| p.name.to_string()).unwrap_or_default();
        let scale_label = if (h.scale - 1.0).abs() < 1e-9 { "Auto".to_string() } else { format!("{:.1}", h.scale) };
        let angle_label = if h.angle.abs() < 1e-9 { "Auto".to_string() } else { format!("{:.1}°", h.angle + base) };
        let color_label = crate::drawing_drawprops::COLORS.iter().find(|x| x.1 == h.color).map(|x| x.0.to_string()).unwrap_or_else(|| "Custom".into());
        let general = h.standard == 2;
        v_flex()
            .child(row("Hatch pattern", select("st-h-group", GROUPS[h.standard.min(2)].0.to_string(), false, groups)))
            .child(row("", select("st-h-pattern", pat_label, true, patterns)))
            .when(!general, |e| e.child(row("Scale", select("st-h-scale", scale_label, false, scales))).child(row("Angle", select("st-h-angle", angle_label, false, angles))))
            .child(row("Color", select("st-h-color", color_label, false, colors)))
            .child(
                h_flex()
                    .id("st-h-revert")
                    .h(px(36.))
                    .px(px(12.))
                    .gap(px(6.))
                    .cursor_pointer()
                    .text_size(px(13.))
                    .text_color(gpui_kit::rgb(0x1651b0))
                    .child("↺ Revert to drawing properties")
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.edit_hatch_styles(
                            std::rc::Rc::new(|x: &mut HatchRegion| {
                                x.standard = 0;
                                x.pattern = hatch_pat::default_pattern(0);
                                x.scale = 1.0;
                                x.angle = 0.0;
                                x.color = None;
                            }),
                            cx,
                        )
                    })),
            )
            .into_any_element()
    }

    pub fn render_styles_panel(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let sel = self.styled_selection();
        let hatch = self.selection.iter().find_map(|s| if let Sel::Hatch(id) = s { self.current_sheet().sketch.hatches.iter().find(|h| h.id == *id).cloned() } else { None });
        if sel.is_empty()
            && let Some(h) = hatch
        {
            return self.render_hatch_styles(h, cx);
        }
        if sel.is_empty() {
            return div()
                .m(px(8.))
                .p(px(8.))
                .rounded(px(4.))
                .border_1()
                .border_color(gpui_kit::rgb(0xb2ddf6))
                .bg(gpui_kit::rgb(0xdef1fb))
                .text_size(px(13.))
                .text_color(gpui_kit::rgb(0x094174))
                .child("Select a dimension or annotation or hatch region to change its style")
                .child(div().pt(px(6.)).child("Select any dimension or annotation or hatch region to apply a different style from the drawing property."))
                .into_any_element();
        }
        let dims = sel.iter().all(|a| a.kind.is_dimension());
        let props = self.drawing.props.clone();
        let first = sel[0].clone();
        let own = first.style.clone().map(|b| *b).unwrap_or_default();
        let ed = cx.entity();
        // A select: label ▾ with (label, checked, edit) items.
        let select = |id: &'static str, label: String, items: Vec<(String, bool, Edit)>| -> AnyElement {
            let ed = ed.clone();
            Button::new(id).label(label).ghost().small().w(px(160.)).dropdown_caret(true).border_b_1().border_color(gpui_kit::rgb(0xdddddd)).rounded(px(0.)).os_dropdown_menu(SharedString::from(format!("{id}-dd")), true, move |mut m, _, _| {
                for (l, on, f) in &items {
                    let (e, f) = (ed.clone(), f.clone());
                    m = m.item(crate::os_menu::OsMenuItem::new(l.clone()).checked(*on).on_click(move |_, _, cx| e.update(cx, |t, cx| t.edit_styles(f.clone(), cx))));
                }
                m
            })
            .into_any_element()
        };
        let row = |label: &'static str, body: AnyElement| h_flex().h(px(33.)).px(px(12.)).child(div().flex_1().text_size(px(13.33)).text_color(pal.text_muted).child(label)).child(body).into_any_element();
        let st = |f: fn(&mut AnnStyle)| -> Edit {
            std::rc::Rc::new(move |a: &mut Annotation| f(a.style.get_or_insert_with(Default::default)))
        };
        let drawing = |s: String, inherited: bool| if inherited { format!("{s} (Drawing)") } else { s };
        let u = noshape_model::drawing::format::length_units(&self.drawing, &noshape_model::drawing::DimFormat::default());
        let len = |mm: f64| format!("{} {}", noshape_model::drawing::format::format_number(mm / 1000.0 / u.meters(), Some(4), 4), u.symbol());
        // Font.
        let font_items: Vec<(String, bool, Edit)> = FONT_FAMILIES
            .iter()
            .map(|f| {
                let fam = f.to_string();
                (fam.clone(), own.font.as_ref().is_some_and(|x| x.family == fam), std::rc::Rc::new(move |a: &mut Annotation| a.style.get_or_insert_with(Default::default).font = Some(TextFont { family: fam.clone(), bold: false, italic: false })) as Edit)
            })
            .collect();
        let font_label = own.font.as_ref().map(|f| f.family.clone()).unwrap_or_else(|| drawing("Noto Sans".into(), true));
        // Text height and arrowhead: inch / mm presets.
        let (base_h, base_arrow) = if dims { (props.dim_text_height, props.dim_arrow) } else { (props.ann_text_height, props.ann_arrow) };
        let presets = |cur: Option<f64>, base: f64, set: fn(&mut AnnStyle, f64)| -> Vec<(String, bool, Edit)> {
            let mut v: Vec<f64> = [2.0, 2.5, 3.0, 3.05, 3.5, 4.0, 4.57, 5.0, 6.0, 7.0].to_vec();
            if !v.iter().any(|x| (x - base).abs() < 1e-6) {
                v.push(base);
                v.sort_by(f64::total_cmp);
            }
            v.into_iter()
                .map(|h| {
                    let inherited = (h - base).abs() < 1e-6;
                    let f: Edit = if inherited {
                        std::rc::Rc::new(move |a: &mut Annotation| if let Some(s) = a.style.as_mut() { set(s, f64::NAN) })
                    } else {
                        std::rc::Rc::new(move |a: &mut Annotation| set(a.style.get_or_insert_with(Default::default), h))
                    };
                    (drawing(len(h), inherited), cur.map_or(inherited, |c| (c - h).abs() < 1e-6), f)
                })
                .collect()
        };
        let set_height: fn(&mut AnnStyle, f64) = |s, h| s.height = (!h.is_nan()).then_some(h);
        let set_arrow: fn(&mut AnnStyle, f64) = |s, h| s.arrow = (!h.is_nan()).then_some(h);
        let set_gap: fn(&mut AnnStyle, f64) = |s, h| s.text_gap = (!h.is_nan()).then_some(h);
        let h_label = own.height.map(len).unwrap_or_else(|| drawing(len(base_h), true));
        let a_label = own.arrow.map(len).unwrap_or_else(|| drawing(len(base_arrow), true));
        // Color presets.
        let colors: Vec<(String, bool, Edit)> = crate::drawing_drawprops::COLORS
            .iter()
            .map(|(n, c)| {
                let c = *c;
                (n.to_string(), own.color == c, std::rc::Rc::new(move |a: &mut Annotation| a.style.get_or_insert_with(Default::default).color = c) as Edit)
            })
            .collect();
        let color_label = crate::drawing_drawprops::COLORS.iter().find(|x| x.1 == own.color).map(|x| x.0.to_string()).unwrap_or_else(|| "Custom".into());
        // Arrow type (live: filled, open, open-thin, dot).
        let arrow_types: Vec<(String, bool, Edit)> = ArrowType::ALL
            .iter()
            .map(|t| {
                let t = *t;
                (t.name().to_string(), own.arrow_type == Some(t), std::rc::Rc::new(move |a: &mut Annotation| a.style.get_or_insert_with(Default::default).arrow_type = Some(t)) as Edit)
            })
            .collect();
        let base_type = if dims { props.dim_arrow_type } else { props.ann_arrow_type };
        let type_label = own.arrow_type.map(|t| t.name().to_string()).unwrap_or_else(|| drawing(base_type.name().into(), true));
        // Live 474: the rows start 14 px under the header.
        let mut body = v_flex().pt(px(14.))
            .child(row("Font", select("st-font", font_label, font_items)))
            .child(row("Text height", select("st-height", h_label, presets(own.height, base_h, set_height))))
            .child(row("Color", select("st-color", color_label, colors)))
            .child(row("Arrowhead", h_flex().gap(px(4.)).child(select("st-arrow", a_label, presets(own.arrow, base_arrow, set_arrow))).into_any_element()))
            .child(row("Arrowhead type", select("st-arrow-type", type_label, arrow_types)));
        if dims {
            // Text justification: Left, Center, Right, Auto ("(Drawing)" on
            // the drawing's own value), Text alignment, Text gap.
            let justs: Vec<(String, bool, Edit)> = TextJustify::ALL
                .iter()
                .map(|j| {
                    let j = *j;
                    let inherited = j == props.dim_justify;
                    let f: Edit = if inherited { st(|s| s.justify = None) } else { std::rc::Rc::new(move |a: &mut Annotation| a.style.get_or_insert_with(Default::default).justify = Some(j)) };
                    (drawing(j.name().into(), inherited), own.justify.map_or(inherited, |x| x == j), f)
                })
                .collect();
            let jl = own.justify.map(|j| j.name().to_string()).unwrap_or_else(|| drawing(props.dim_justify.name().into(), true));
            let horiz_base = props.dim_text_horizontal;
            let aligns: Vec<(String, bool, Edit)> = [("Align with dimension line", false), ("Horizontal", true)]
                .iter()
                .map(|&(n, v)| {
                    let inherited = v == horiz_base;
                    let f: Edit = if inherited { st(|s| s.horizontal = None) } else { std::rc::Rc::new(move |a: &mut Annotation| a.style.get_or_insert_with(Default::default).horizontal = Some(v)) };
                    (drawing(n.into(), inherited), own.horizontal.map_or(inherited, |x| x == v), f)
                })
                .collect();
            let al = match own.horizontal {
                Some(true) => "Horizontal".to_string(),
                Some(false) => "Align with dimension line".to_string(),
                None => drawing(if horiz_base { "Horizontal" } else { "Align with dimension line" }.into(), true),
            };
            let gl = own.text_gap.map(len).unwrap_or_else(|| drawing(len(props.dim_text_gap), true));
            let broken = !first.unbroken;
            let radial = sel.iter().all(|a| matches!(a.kind, AnnotationKind::RadialDimension { diameter: true, .. }));
            let dim_line = matches!(first.kind, AnnotationKind::RadialDimension { dim_line: true, .. });
            body = body
                .child(row("Text justification", select("st-just", jl, justs)))
                .child(row("Text alignment", select("st-align", al, aligns)))
                .child(row("Text gap", select("st-gap", gl, presets(own.text_gap, props.dim_text_gap, set_gap))))
                .child(div().h(px(33.)).px(px(10.)).flex().items_center().child(Checkbox::new("st-break").label("Break dimension").checked(broken).on_click(cx.listener(move |this, v: &bool, _, cx| {
                    let v = *v;
                    this.edit_styles(std::rc::Rc::new(move |a: &mut Annotation| a.unbroken = !v), cx);
                }))))
                .child(div().h(px(33.)).px(px(10.)).flex().items_center().opacity(if radial { 1.0 } else { 0.4 }).child(Checkbox::new("st-dimline").label("Show dimension line").checked(dim_line).on_click(cx.listener(move |this, v: &bool, _, cx| {
                    if !radial {
                        return;
                    }
                    let v = *v;
                    this.edit_styles(std::rc::Rc::new(move |a: &mut Annotation| if let AnnotationKind::RadialDimension { dim_line, .. } = &mut a.kind { *dim_line = v }), cx);
                }))));
        }
        v_flex()
            .child(body)
            .child(
                h_flex()
                    .id("st-revert")
                    .h(px(36.))
                    .px(px(12.))
                    .gap(px(6.))
                    .cursor_pointer()
                    .text_size(px(13.))
                    .text_color(gpui_kit::rgb(0x1651b0))
                    .child("↺ Revert to drawing properties")
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.edit_styles(std::rc::Rc::new(|a: &mut Annotation| a.style = None), cx))),
            )
            .into_any_element()
    }
}
