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
use gpui_kit::component::input::{Input, InputEvent, InputState};
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
        let revert = cx.listener(|this, _: &ClickEvent, _, cx| {
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
        });
        let rows = v_flex()
            .child(row("Hatch pattern", select("st-h-group", GROUPS[h.standard.min(2)].0.to_string(), false, groups)))
            .child(row("", select("st-h-pattern", pat_label, true, patterns)))
            .when(!general, |e| e.child(row("Scale", select("st-h-scale", scale_label, false, scales))).child(row("Angle", select("st-h-angle", angle_label, false, angles))))
            .child(row("Color", select("st-h-color", color_label, false, colors)));
        Self::styles_with_footer(rows.into_any_element(), "st-h-revert", revert, cx)
    }

    pub fn render_styles_panel(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let sel = self.styled_selection();
        let hatch = self.selection.iter().find_map(|s| if let Sel::Hatch(id) = s { self.current_sheet().sketch.hatches.iter().find(|h| h.id == *id).cloned() } else { None });
        if sel.is_empty()
            && let Some(h) = hatch
        {
            return self.render_hatch_styles(h, cx);
        }
        if sel.is_empty() {
            // Live drawing/95-panel-styles (IntimationMessageheadingStyle /
            // IntimationMessageStyle): 4 px in and below the header, 7 px
            // right, padding 15,
            // #bce8f1 border, #d9edf7 fill, #31708f text; a bold 14 px heading
            // and a 13 px body on 20 px lines.
            let info = div()
                .ml(px(4.))
                .mt(px(4.))
                .mr(px(7.))
                .p(px(15.))
                .rounded(px(4.))
                .border_1()
                .border_color(gpui_kit::rgb(0xbce8f1))
                .bg(gpui_kit::rgb(0xd9edf7))
                .text_color(gpui_kit::rgb(0x31708f))
                .line_height(px(20.))
                .child(div().text_size(px(14.)).font_weight(FontWeight::BOLD).child("Select a dimension or annotation or hatch region to change its style"))
                .child(div().text_size(px(13.)).child("Select any dimension or annotation or hatch region to apply a different style from the drawing property."));
            return Self::styles_with_footer(info.into_any_element(), "st-revert", |_, _, _| {}, cx);
        }
        let dims = sel.iter().all(|a| a.kind.is_dimension());
        let props = self.drawing.props.clone();
        let first = sel[0].clone();
        let own = first.style.clone().map(|b| *b).unwrap_or_default();
        let ed = cx.entity();
        let ink = gpui_kit::rgb(0x333333);
        // Live 474 (dom/drawing-styles-*): label left, the control at 145 px; a
        // select is underlined with a ▾; sizes are a number ▾ then the unit.
        let select = |id: &'static str, label: String, w: f32, items: Vec<(String, bool, Edit)>| -> AnyElement {
            let ed = ed.clone();
            Button::new(id).label(label).ghost().small().w(px(w)).dropdown_caret(true).border_b_1().border_color(gpui_kit::rgb(0xdddddd)).rounded(px(0.)).os_dropdown_menu(SharedString::from(format!("{id}-dd")), true, move |mut m, _, _| {
                for (l, on, f) in &items {
                    let (e, f) = (ed.clone(), f.clone());
                    m = m.item(crate::os_menu::OsMenuItem::new(l.clone()).checked(*on).on_click(move |_, _, cx| e.update(cx, |t, cx| t.edit_styles(f.clone(), cx))));
                }
                m
            })
            .into_any_element()
        };
        let row = |label: &'static str, body: AnyElement| h_flex().h(px(33.)).px(px(12.)).child(div().w(px(122.)).flex_none().text_size(px(13.33)).text_color(ink).child(label)).child(body).into_any_element();
        let st = |f: fn(&mut AnnStyle)| -> Edit {
            std::rc::Rc::new(move |a: &mut Annotation| f(a.style.get_or_insert_with(Default::default)))
        };
        let drawing = |s: String, inherited: bool| if inherited { format!("{s} (Drawing)") } else { s };
        let u = noshape_model::drawing::format::length_units(&self.drawing, &noshape_model::drawing::DimFormat::default());
        let digits = if self.drawing.units_mm { 2 } else { 4 };
        let num = |mm: f64| noshape_model::drawing::format::format_number(mm / 1000.0 / u.meters(), Some(digits), 4);
        let unit = |e: AnyElement| h_flex().gap(px(10.)).child(e).child(div().text_size(px(13.33)).text_color(ink).child(u.symbol())).into_any_element();
        // Font (plain value).
        let font_items: Vec<(String, bool, Edit)> = FONT_FAMILIES
            .iter()
            .map(|f| {
                let fam = f.to_string();
                (fam.clone(), own.font.as_ref().map_or(fam == "Noto Sans", |x| x.family == fam), std::rc::Rc::new(move |a: &mut Annotation| a.style.get_or_insert_with(Default::default).font = Some(TextFont { family: fam.clone(), bold: false, italic: false })) as Edit)
            })
            .collect();
        let font_label = own.font.as_ref().map(|f| f.family.clone()).unwrap_or_else(|| "Noto Sans".into());
        // Text height and arrowhead: preset lists, the value shown plain.
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
                    (num(h), cur.map_or(inherited, |c| (c - h).abs() < 1e-6), f)
                })
                .collect()
        };
        let set_height: fn(&mut AnnStyle, f64) = |s, h| s.height = (!h.is_nan()).then_some(h);
        let set_arrow: fn(&mut AnnStyle, f64) = |s, h| s.arrow = (!h.is_nan()).then_some(h);
        let h_label = num(own.height.unwrap_or(base_h));
        let a_label = num(own.arrow.unwrap_or(base_arrow));
        // Color: the swatch opens the color list.
        let colors: Vec<MenuEntry> = crate::drawing_drawprops::COLORS
            .iter()
            .map(|(n, c)| {
                let c = *c;
                MenuEntry::check(*n, own.color == c, move |t, _, cx| t.edit_styles(std::rc::Rc::new(move |a: &mut Annotation| a.style.get_or_insert_with(Default::default).color = c), cx))
            })
            .collect();
        let c = own.color.unwrap_or([0, 0, 0]);
        let swatch = {
            let ed = ed.clone();
            div()
                .id("st-color")
                .size(px(26.))
                .bg(gpui_kit::rgb(((c[0] as u32) << 16) | ((c[1] as u32) << 8) | c[2] as u32))
                .cursor_pointer()
                .on_click(move |e, w, cx| open_tool_menu(&colors, &ed, e, w, cx))
                .into_any_element()
        };
        // Arrowhead type: the arrow picker beside the size (live 477).
        let base_type = if dims { props.dim_arrow_type } else { props.ann_arrow_type };
        let cur_type = own.arrow_type.unwrap_or(base_type);
        let glyph = |t: ArrowType| match t {
            ArrowType::None => " ",
            ArrowType::Filled => "◄",
            ArrowType::Unfilled => "◁",
            ArrowType::Open => "<",
            ArrowType::Oblique => "/",
            ArrowType::FilledDot => "•",
        };
        let arrow_types: Vec<(String, bool, Edit)> = ArrowType::ALL
            .iter()
            .map(|t| {
                let t = *t;
                (format!("{}  {}", glyph(t), t.name()), cur_type == t, std::rc::Rc::new(move |a: &mut Annotation| a.style.get_or_insert_with(Default::default).arrow_type = Some(t)) as Edit)
            })
            .collect();
        let arrow_picker = {
            let ed = ed.clone();
            Button::new("st-arrow-type").ghost().small().dropdown_caret(true).child(div().text_size(px(12.)).text_color(ink).child(format!("—{}", glyph(cur_type)))).os_dropdown_menu("st-arrow-type-dd", true, move |mut m, _, _| {
                for (l, on, f) in &arrow_types {
                    let (e, f) = (ed.clone(), f.clone());
                    m = m.item(crate::os_menu::OsMenuItem::new(l.clone()).checked(*on).on_click(move |_, _, cx| e.update(cx, |t, cx| t.edit_styles(f.clone(), cx))));
                }
                m
            })
            .into_any_element()
        };
        let mut body = v_flex()
            .pt(px(14.))
            .child(row("Font", select("st-font", font_label, 220., font_items)))
            .child(row("Text height", unit(select("st-height", h_label, 80., presets(own.height, base_h, set_height)))))
            .child(row("Color", swatch))
            .child(row("Arrowhead", h_flex().gap(px(10.)).child(unit(select("st-arrow", a_label, 80., presets(own.arrow, base_arrow, set_arrow)))).child(arrow_picker).into_any_element()));
        if dims {
            // Text justification keeps "(Drawing)" on the inherited value
            // (live 474: "Center (Drawing)"); alignment shows plain values.
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
                    (n.to_string(), own.horizontal.map_or(inherited, |x| x == v), f)
                })
                .collect();
            let al = if own.horizontal.unwrap_or(horiz_base) { "Horizontal" } else { "Align with dimension line" }.to_string();
            // Text gap: a spinbox (type a value, or step it).
            let gap = own.text_gap.unwrap_or(props.dim_text_gap);
            let gap_text = num(gap);
            let fresh = !self.styles_gap.as_ref().is_some_and(|(id, _, _)| *id == first.id);
            if fresh {
                let stx = cx.new(|cx| InputState::new(window, cx).select_on_focus(false).revert_on_escape(false).default_value(gap_text.clone()));
                let meters = u.meters();
                let sub = cx.subscribe(&stx, move |this: &mut Self, stx, ev: &InputEvent, cx| {
                    if !matches!(ev, InputEvent::PressEnter { .. } | InputEvent::Blur) {
                        return;
                    }
                    if let Ok(v) = stx.read(cx).value().trim().parse::<f64>() {
                        let mm = (v * meters * 1000.0).max(0.0);
                        this.edit_styles(std::rc::Rc::new(move |a: &mut Annotation| a.style.get_or_insert_with(Default::default).text_gap = Some(mm)), cx);
                    }
                });
                self.styles_gap = Some((first.id, stx, sub));
            }
            let gap_input = self.styles_gap.as_ref().map(|g| g.1.clone());
            let step = if self.drawing.units_mm { 0.25 } else { 0.254 };
            let stepper = |id: &'static str, glyph: &'static str, d: f64, cx: &mut Context<Self>| {
                div().id(id).h(px(11.)).w(px(14.)).flex().items_center().justify_center().cursor_pointer().text_size(px(8.)).text_color(ink).child(glyph).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    let next = (gap + d).max(0.0);
                    this.styles_gap = None;
                    this.edit_styles(std::rc::Rc::new(move |a: &mut Annotation| a.style.get_or_insert_with(Default::default).text_gap = Some(next)), cx);
                }))
            };
            let spin = h_flex()
                .w(px(200.))
                .h(px(26.))
                .border_b_1()
                .border_color(gpui_kit::rgb(0xdddddd))
                .children(gap_input.map(|g| div().flex_1().child(Input::new(&g).appearance(false).small())))
                .child(v_flex().child(stepper("st-gap-up", "▲", step, cx)).child(stepper("st-gap-down", "▼", -step, cx)));
            let broken = !first.unbroken;
            let radial = sel.iter().all(|a| matches!(a.kind, AnnotationKind::RadialDimension { diameter: true, .. }));
            let dim_line = matches!(first.kind, AnnotationKind::RadialDimension { dim_line: true, .. });
            body = body
                .child(row("Text justification", select("st-just", jl, 220., justs)))
                .child(row("Text alignment", select("st-align", al, 220., aligns)))
                .child(row("Text gap", h_flex().gap(px(10.)).child(spin).child(div().text_size(px(13.33)).text_color(ink).child(u.symbol())).into_any_element()))
                .child(div().h(px(33.)).px(px(10.)).flex().items_center().child(crate::drawing_chrome::DrCheckbox::new("st-break").native().label("Break dimension").checked(broken).on_click(cx.listener(move |this, v: &bool, _, cx| {
                    let v = *v;
                    this.edit_styles(std::rc::Rc::new(move |a: &mut Annotation| a.unbroken = !v), cx);
                }))))
                .child(div().h(px(33.)).px(px(10.)).flex().items_center().child(crate::drawing_chrome::DrCheckbox::new("st-dimline").native().label("Show dimension line").checked(dim_line).on_click(cx.listener(move |this, v: &bool, _, cx| {
                    if !radial {
                        return;
                    }
                    let v = *v;
                    this.edit_styles(std::rc::Rc::new(move |a: &mut Annotation| if let AnnotationKind::RadialDimension { dim_line, .. } = &mut a.kind { *dim_line = v }), cx);
                }))));
        }
        let revert = cx.listener(|this, _: &ClickEvent, _, cx| this.edit_styles(std::rc::Rc::new(|a: &mut Annotation| a.style = None), cx));
        Self::styles_with_footer(body.into_any_element(), "st-revert", revert, cx)
    }

    /// The panel body over its footer (live drawing/95, 474): pinned to the
    /// bottom, a #dcdcdc top line on #f9f9f9; a 28 px white button with the
    /// refresh icon, "Revert to drawing properties" (13.33 px #666) and a
    /// help icon at the right.
    fn styles_with_footer(body: AnyElement, id: &'static str, revert: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static, cx: &mut Context<Self>) -> AnyElement {
        v_flex()
            .size_full()
            .child(div().flex_1().min_h_0().child(body))
            .child(
                h_flex()
                    .flex_none()
                    .h(px(40.))
                    .pl(px(4.))
                    .pr(px(9.))
                    .gap(px(5.))
                    .border_t_1()
                    .border_color(gpui_kit::rgb(0xdcdcdc))
                    .bg(gpui_kit::rgb(0xf9f9f9))
                    .child(div().id(id).size(px(28.)).flex().items_center().justify_center().rounded(px(3.)).bg(crate::theme::light().panel).cursor_pointer().child(crate::assets::icon("refresh", 20., cx)).on_click(revert))
                    .child(div().flex_1().text_size(px(13.33)).text_color(gpui_kit::rgb(0x666666)).child("Revert to drawing properties"))
                    .child(div().size(px(14.)).child(crate::assets::icon("help-filled", 14., cx))),
            )
            .into_any_element()
    }
}
