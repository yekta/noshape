//! Drawing editor: view menu › Adjust linestyle… (live drawing/443–446,
//! dom/drawing-adjust-linestyle.txt): a floating dialog with the Edges
//! field ("Select edges in a view to change linestyle"; picked edges show
//! as "Top : entity" ×), Linestyle (Solid, Phantom, Center, Dashed,
//! Dotted), Dashes (greyed for Solid), Thickness (0.13–2.00 mm) and the
//! color button, and "Revert to drawing properties"; ✓ applies, ✗ / Esc
//! cancels. The edges preview live.

use crate::drawing_view::*;
use crate::os_ui::OsDropdownExt as _;
use crate::theme::p;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::drawing::{EdgeStyle, LINE_THICKNESSES, LinePattern, ViewId};
use noshape_model::reference::{BodyId, TopoName};

/// The open Adjust linestyle dialog.
pub struct LinestyleDlg {
    pub view: ViewId,
    pub edges: Vec<(BodyId, TopoName)>,
    pub pattern: LinePattern,
    pub dashes: Entity<InputState>,
    pub thickness: f64,
    pub color: Option<[u8; 3]>,
    /// Picked edges whose styles go back to the drawing's.
    pub revert: bool,
}

impl DrawingEditor {
    pub fn open_linestyle(&mut self, view: ViewId, window: &mut Window, cx: &mut Context<Self>) {
        let dashes = cx.new(|cx| InputState::new(window, cx).select_on_focus(false).revert_on_escape(false).default_value("1.00"));
        self.linestyle = Some(LinestyleDlg { view, edges: Vec::new(), pattern: LinePattern::Solid, dashes, thickness: 0.35, color: None, revert: false });
        cx.notify();
    }

    /// The drawing with the dialog's styles on its edges.
    fn linestyled(&self, cx: &App) -> Option<noshape_model::drawing::Drawing> {
        let d = self.linestyle.as_ref()?;
        let k = d.dashes.read(cx).value().trim().parse::<f64>().unwrap_or(1.0);
        let mut dr = self.drawing.clone();
        let v = dr.sheets.get_mut(self.sheet)?.view_mut(d.view)?;
        for (body, name) in &d.edges {
            v.edge_styles.retain(|s| !(s.body == *body && s.name == *name));
            if !d.revert {
                v.edge_styles.push(EdgeStyle { body: body.clone(), name: name.clone(), pattern: d.pattern, dashes: k, thickness: d.thickness, color: d.color });
            }
        }
        Some(dr)
    }

    fn preview_linestyle(&mut self, cx: &mut Context<Self>) {
        let dr = self.linestyled(cx);
        self.set_preview(dr, cx);
    }

    /// A click with the dialog open: add / remove the hovered edge of its
    /// view. True when used.
    pub fn linestyle_click(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(view) = self.linestyle.as_ref().map(|d| d.view) else { return false };
        if let Some((v, me)) = self.hovered_edge()
            && v == view
            && let Some(d) = &mut self.linestyle
        {
            let key = (me.body.clone(), (*me.name).clone());
            match d.edges.iter().position(|e| *e == key) {
                Some(i) => {
                    d.edges.remove(i);
                }
                None => d.edges.push(key),
            }
            self.preview_linestyle(cx);
        }
        true
    }

    pub fn close_linestyle(&mut self, accept: bool, cx: &mut Context<Self>) {
        let dr = if accept { self.linestyled(cx) } else { None };
        self.linestyle = None;
        self.set_preview(None, cx);
        if let Some(dr) = dr {
            self.commit("Adjust linestyle", dr, cx);
        }
        cx.notify();
    }

    pub fn render_linestyle(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let d = self.linestyle.as_ref()?;
        let pal = p();
        let ink = gpui_kit::rgb(0x333333);
        let ed = cx.entity();
        let view_name = self.current_sheet().view(d.view).map(|v| v.name.clone().unwrap_or_else(|| crate::drawing_props::default_view_name(&self.drawing, self.current_sheet(), v))).unwrap_or_default();
        type Set = std::rc::Rc<dyn Fn(&mut LinestyleDlg)>;
        let select = |id: &'static str, label: String, w: f32, items: Vec<(String, bool, Set)>| {
            let ed = ed.clone();
            Button::new(id).label(label).ghost().xsmall().w(px(w)).dropdown_caret(true).border_b_1().border_color(gpui_kit::rgb(0xdddddd)).rounded(px(0.)).os_dropdown_menu(SharedString::from(format!("{id}-dd")), true, move |mut m, _, _| {
                for (l, on, f) in &items {
                    let (e, f) = (ed.clone(), f.clone());
                    m = m.item(crate::os_menu::OsMenuItem::new(l.clone()).checked(*on).on_click(move |_, _, cx| {
                        e.update(cx, |t, cx| {
                            if let Some(d) = &mut t.linestyle {
                                f(d);
                                d.revert = false;
                            }
                            t.preview_linestyle(cx);
                        })
                    }));
                }
                m
            })
        };
        let patterns: Vec<(String, bool, Set)> = LinePattern::ALL.iter().map(|p| {
            let p = *p;
            (p.name().to_string(), d.pattern == p, std::rc::Rc::new(move |d: &mut LinestyleDlg| d.pattern = p) as Set)
        }).collect();
        let thicknesses: Vec<(String, bool, Set)> = LINE_THICKNESSES.iter().map(|t| {
            let t = *t;
            (format!("{t:.2} mm"), (d.thickness - t).abs() < 1e-9, std::rc::Rc::new(move |d: &mut LinestyleDlg| d.thickness = t) as Set)
        }).collect();
        let colors: Vec<(String, bool, Set)> = crate::drawing_drawprops::COLORS.iter().map(|(n, c)| {
            let c = *c;
            (n.to_string(), d.color == c, std::rc::Rc::new(move |d: &mut LinestyleDlg| d.color = c) as Set)
        }).collect();
        let c = d.color.unwrap_or([0, 0, 0]);
        let swatch = div().size(px(18.)).border_1().border_color(gpui_kit::rgb(0x999999)).bg(gpui_kit::rgb(((c[0] as u32) << 16) | ((c[1] as u32) << 8) | c[2] as u32));
        let chips: Vec<AnyElement> = d.edges.iter().enumerate().map(|(i, _)| {
            h_flex().id(("ls-edge", i)).h(px(22.)).px(px(4.)).gap(px(4.)).bg(gpui_kit::rgb(0xeeeeee)).rounded(px(2.)).text_size(px(12.))
                .child(format!("{view_name} : entity"))
                .child(div().id(("ls-edge-x", i)).cursor_pointer().child("×").on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    if let Some(d) = &mut this.linestyle && i < d.edges.len() {
                        d.edges.remove(i);
                    }
                    this.preview_linestyle(cx);
                })))
                .into_any_element()
        }).collect();
        let empty = chips.is_empty();
        let row = |label: &'static str, body: AnyElement| h_flex().h(px(30.)).px(px(10.)).child(div().w(px(80.)).text_size(px(12.)).child(label)).child(body).into_any_element();
        let solid = d.pattern == LinePattern::Solid;
        Some(
            v_flex()
                .id("dr-linestyle")
                .absolute()
                .top(px(DIALOG_TOP))
                .left(px(self.dialog_left()))
                .w(px(260.))
                .pb(px(6.))
                .bg(pal.dialog_bg)
                .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(5.)), blur_radius: px(10.), spread_radius: px(0.), inset: false }])
                .text_color(ink)
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    h_flex()
                        .h(px(28.))
                        .pl(px(10.))
                        .child(div().flex_1().text_size(px(13.)).font_weight(FontWeight::MEDIUM).child("Adjust linestyle"))
                        .child(div().id("ls-ok").w(px(28.)).h(px(27.)).flex().items_center().justify_center().bg(pal.accept).cursor_pointer().child(crate::ui::glyph("check", 16., gpui_kit::white())).on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.close_linestyle(true, cx))))
                        .child(div().id("ls-close").w(px(28.)).h(px(27.)).flex().items_center().justify_center().cursor_pointer().child(crate::ui::glyph("close", 14., pal.cancel)).on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.close_linestyle(false, cx)))),
                )
                .child(div().px(px(10.)).pt(px(4.)).text_size(px(12.)).child("Edges"))
                .child(
                    div()
                        .mx(px(10.))
                        .min_h(px(30.))
                        .p(px(4.))
                        .flex()
                        .flex_wrap()
                        .gap(px(4.))
                        .border_1()
                        .border_color(gpui_kit::rgb(0xabcbff))
                        .when(empty, |b| b.child(div().text_size(px(12.)).italic().text_color(pal.text_muted).child("Select edges in a view to change linestyle")))
                        .children(chips),
                )
                .child(row("Linestyle", select("ls-pattern", d.pattern.name().into(), 150., patterns).into_any_element()))
                .child(row("Dashes", div().w(px(150.)).when(solid, |b| b.opacity(0.4)).child(Input::new(&d.dashes).text_size(px(13.33)).text_color(gpui_kit::rgb(0x555555)).small()).into_any_element()))
                .child(row("Thickness", h_flex().gap(px(6.)).child(select("ls-thick", format!("{:.2} mm", d.thickness), 110., thicknesses)).child(Button::new("ls-color").ghost().xsmall().child(swatch).os_dropdown_menu("ls-color-dd", true, {
                    let ed = ed.clone();
                    move |mut m, _, _| {
                        for (l, on, f) in &colors {
                            let (e, f) = (ed.clone(), f.clone());
                            m = m.item(crate::os_menu::OsMenuItem::new(l.clone()).checked(*on).on_click(move |_, _, cx| e.update(cx, |t, cx| {
                                if let Some(d) = &mut t.linestyle {
                                    f(d);
                                }
                                t.preview_linestyle(cx);
                            })));
                        }
                        m
                    }
                })).into_any_element()))
                .child(
                    div()
                        .id("ls-revert")
                        .px(px(10.))
                        .pt(px(4.))
                        .text_size(px(12.))
                        .text_color(gpui_kit::rgb(0x1651b0))
                        .cursor_pointer()
                        .child("↺ Revert to drawing properties")
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            if let Some(d) = &mut this.linestyle {
                                d.revert = true;
                            }
                            this.preview_linestyle(cx);
                        })),
                )
                .into_any_element(),
        )
    }
}
