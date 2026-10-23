//! Drawing editor: Onshape's Section view dialog (while the Section view
//! tool is active): Vertical / Horizontal / Angular, the section label,
//! the cutting-line pick button and Flip, "Show cut geometry only" and
//! "Section depth". Every mode takes one click: vertical, horizontal or
//! at the Angular mode's Cut angle (Opposite direction mirrors it).

use crate::drawing_view::*;
use crate::theme::p;
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

/// Section view dialog state.
pub struct SectionDlg {
    /// 0 Vertical, 1 Horizontal, 2 Angular.
    pub mode: u8,
    pub label: Entity<InputState>,
    /// "Show cut geometry only".
    pub cut_only: bool,
    /// "Section depth" (Blind): the field and its value in meters.
    pub depth: Option<(Entity<InputState>, Subscription)>,
    pub depth_value: Option<f64>,
    /// Section depth "Up to entity" (else Blind) and the picked point.
    pub up_to: bool,
    pub depth_ref: Option<noshape_model::drawing::PointRef>,
    /// "Exclude from cut" (assembly views): the parts picked.
    pub exclude: Vec<noshape_model::BodyId>,
    /// Which field takes clicks: 1 = depth entity, 2 = excluded parts.
    pub picking: u8,
    /// Angular: "Cut angle" (degrees from horizontal) and its field,
    /// "Opposite direction".
    pub angle: f64,
    pub angle_input: Option<(Entity<InputState>, Subscription)>,
    pub opposite: bool,
    /// Flip: look the other way across the cutting line.
    pub flip: bool,
    /// "Cut from section" (sectioning a section view; live default on).
    pub cut_from: bool,
}

/// "10 mm", "0.5 in", "12" (drawing units) → meters.
pub fn parse_depth(s: &str, mm: bool) -> Option<f64> {
    let s = s.trim();
    let (num, k) = if let Some(n) = s.strip_suffix("mm") {
        (n, 0.001)
    } else if let Some(n) = s.strip_suffix("cm") {
        (n, 0.01)
    } else if let Some(n) = s.strip_suffix("in") {
        (n, 0.0254)
    } else if let Some(n) = s.strip_suffix('m') {
        (n, 1.0)
    } else {
        (s, if mm { 0.001 } else { 0.0254 })
    };
    num.trim().parse::<f64>().ok().filter(|v| *v > 0.0).map(|v| v * k)
}

impl DrawingEditor {
    /// Create / drop the dialog with the Section view tool (called from render).
    pub fn sync_section_dlg(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let active = matches!(self.tool, Tool::Section { partial: false, .. });
        match (&self.section_dlg, active) {
            (None, true) => {
                let next = self.current_sheet().next_label();
                let label = cx.new(|cx| InputState::new(window, cx).select_on_focus(false).revert_on_escape(false).placeholder("Section id").default_value(next));
                self.section_dlg = Some(SectionDlg { mode: 0, label, cut_only: false, depth: None, depth_value: None, up_to: false, depth_ref: None, exclude: Vec::new(), picking: 0, angle: 45.0, angle_input: None, opposite: false, flip: false, cut_from: true });
            }
            (Some(_), false) => self.section_dlg = None,
            _ => {}
        }
        // Angular: the Cut angle field.
        if let Some(d) = &mut self.section_dlg
            && d.mode == 2
            && d.angle_input.is_none()
        {
            let st = cx.new(|cx| InputState::new(window, cx).select_on_focus(false).revert_on_escape(false).default_value(format!("{} deg", d.angle)));
            let sub = cx.subscribe(&st, |this: &mut Self, st, ev: &InputEvent, cx| {
                if matches!(ev, InputEvent::Change)
                    && let Ok(a) = st.read(cx).value().trim().trim_end_matches("deg").trim_end_matches('°').trim().parse::<f64>()
                {
                    if let Some(d) = &mut this.section_dlg {
                        d.angle = a;
                    }
                    this.update_preview(cx);
                }
            });
            d.angle_input = Some((st, sub));
        }
    }

    /// The cutting line direction (sheet) of the dialog's mode: vertical,
    /// horizontal, or at the Cut angle (Opposite direction mirrors it).
    pub fn section_axis(&self) -> Option<[f64; 2]> {
        let d = self.section_dlg.as_ref()?;
        match d.mode {
            0 => Some([0.0, 1.0]),
            1 => Some([1.0, 0.0]),
            _ => {
                let a = d.angle.to_radians() * if d.opposite { -1.0 } else { 1.0 };
                Some([a.cos(), a.sin()])
            }
        }
    }

    /// The label typed in the dialog (else the sheet's next free letter).
    pub fn section_label(&self, cx: &App) -> Option<String> {
        let t = self.section_dlg.as_ref()?.label.read(cx).value().trim().to_string();
        (!t.is_empty()).then_some(t)
    }

    /// A click while a dialog field takes picks: the depth point (Up to
    /// entity) or a part to exclude from the cut. True when used.
    pub fn section_field_click(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(picking) = self.section_dlg.as_ref().map(|d| d.picking).filter(|p| *p != 0) else { return false };
        if picking == 1 {
            let Some(r) = self.snap_at(self.cursor).and_then(|p| p.r) else {
                self.status = Some("Select a vertex or circle center for the section depth".into());
                cx.notify();
                return true;
            };
            if let Some(d) = &mut self.section_dlg {
                d.depth_ref = Some(r);
                d.picking = 0;
            }
        } else if let Some(body) = self.edge_body_at(self.cursor)
            && let Some(d) = &mut self.section_dlg
        {
            match d.exclude.iter().position(|b| *b == body) {
                Some(i) => {
                    d.exclude.remove(i);
                }
                None => d.exclude.push(body),
            }
        }
        self.update_preview(cx);
        cx.notify();
        true
    }

    /// The part whose view edge is under a sheet point.
    fn edge_body_at(&self, c: [f64; 2]) -> Option<noshape_model::BodyId> {
        use noshape_model::drawing::build::PickTarget;
        let b = self.built.as_ref()?;
        let PickTarget::Edge { view, edge } = b.picks.get(b.hit(c, self.tol_mm(5.0))? as usize)?.clone() else { return None };
        let g = self.geoms.get(&view)?;
        let m = g.edges.get(edge as usize)?.model?;
        Some(g.model_edges.get(m as usize)?.body.clone())
    }

    fn toggle_section_depth(&mut self, on: bool, window: &mut Window, cx: &mut Context<Self>) {
        let mm = self.drawing.units_mm;
        let Some(d) = &mut self.section_dlg else { return };
        if !on {
            d.depth = None;
        } else if d.depth.is_none() {
            let text = if mm { "10 mm" } else { "0.5 in" };
            d.depth_value = parse_depth(text, mm);
            let st = cx.new(|cx| InputState::new(window, cx).select_on_focus(false).revert_on_escape(false).default_value(text));
            let sub = cx.subscribe(&st, move |this: &mut Self, st, ev: &InputEvent, cx| {
                if matches!(ev, InputEvent::Change) {
                    let v = parse_depth(&st.read(cx).value(), mm);
                    if let Some(d) = &mut this.section_dlg {
                        d.depth_value = v;
                    }
                    this.update_preview(cx);
                }
            });
            d.depth = Some((st, sub));
        }
        self.update_preview(cx);
        cx.notify();
    }

    fn set_section_mode(&mut self, mode: u8, cx: &mut Context<Self>) {
        if let Some(d) = &mut self.section_dlg {
            d.mode = mode;
        }
        // Start the cut again in the new mode (the view stays chosen).
        if let Tool::Section { parent, partial, .. } = self.tool.clone() {
            self.tool = Tool::Section { parent, p0: None, line: None, partial, jogs: vec![] };
        }
        self.update_preview(cx);
        cx.notify();
    }

    pub fn render_section_dlg(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let d = self.section_dlg.as_ref()?;
        let pal = p();
        let mode = d.mode;
        let (opp, flip) = (d.opposite, d.flip);
        // Exclude from cut is for assembly views.
        let assembly = match &self.tool {
            Tool::Section { parent: Some(p), .. } => noshape_model::drawing::views::view_source(self.current_sheet(), *p).and_then(|(k, _)| self.sources.get(&k)).is_some_and(|s| s.assembly),
            _ => self.current_sheet().views.iter().any(|v| noshape_model::drawing::views::view_source(self.current_sheet(), v.id).and_then(|(k, _)| self.sources.get(&k)).is_some_and(|s| s.assembly)),
        };
        let line_done = matches!(self.tool, Tool::Section { line: Some(_), .. });
        // "Cut from section": sectioning a section view (not a broken-out one).
        let of_section = match &self.tool {
            Tool::Section { parent: Some(p), .. } => self.current_sheet().view(*p).is_some_and(|v| matches!(v.kind, noshape_model::drawing::ViewKind::Section { .. }) && v.broken_out.is_empty()),
            _ => false,
        };
        let tab = |id: &'static str, label: &'static str, m: u8, cx: &mut Context<Self>| {
            let on = mode == m;
            div()
                .id(id)
                .h(px(26.))
                .px(px(7.))
                .flex()
                .items_center()
                .text_size(px(13.33))
                .cursor_pointer()
                .border_b_1()
                .when(on, |d| d.border_b_2().border_color(pal.accent).text_color(pal.emphasis).font_weight(FontWeight::MEDIUM))
                .when(!on, |d| d.border_color(gpui_kit::rgb(0xdddddd)).text_color(pal.text))
                .child(label)
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.set_section_mode(m, cx)))
        };
        Some(
            v_flex()
                .id("dr-section-dlg")
                .absolute()
                .top(px(DIALOG_TOP))
                .left(px(self.dialog_left()))
                .w(px(200.))
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
                        .child(div().flex_1().text_size(px(13.)).font_weight(FontWeight::MEDIUM).child("Section view"))
                        // ✓ is enabled once the section view can be placed.
                        .child(div().w(px(28.)).h(px(27.)).flex().items_center().justify_center().bg(gpui_kit::rgb(0x009500)).when(!line_done, |d| d.bg(gpui_kit::rgb(crate::drawing_chrome::OK_DISABLED))).child(crate::ui::glyph("check", 18., gpui_kit::white())))
                        .child(
                            div()
                                .id("ds-cancel")
                                .w(px(28.))
                                .h(px(27.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .child(crate::drawing_chrome::cancel_x(cx))
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.cancel(cx))),
                        ),
                )
                .child(
                    v_flex()
                        .p(px(3.))
                        .child(h_flex().child(tab("ds-vert", "Vertical", 0, cx)).child(tab("ds-horiz", "Horizontal", 1, cx)).child(tab("ds-ang", "Angular", 2, cx)))
                        .child(gpui_kit::base::input::input_frame(div().mt(px(1.)).h(px(26.)).flex().items_end().border_b_1().border_color(pal.accent).child(Input::new(&d.label).text_size(px(13.33)).text_color(gpui_kit::rgb(0x555555)).appearance(false).small())))
                        .child(
                            h_flex()
                                .mt(px(5.))
                                .h(px(30.))
                                .child(
                                    // The cutting line pick (active until the line is placed; click to pick again).
                                    div()
                                        .id("ds-line")
                                        .size(px(30.))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .rounded(px(2.))
                                        .border_1()
                                        .border_color(gpui_kit::rgb(0x9aa5af))
                                        .when(!line_done, |d| d.bg(gpui_kit::rgb(0xcfdbe5)))
                                        .cursor_pointer()
                                        .tooltip(crate::ui::tooltip("Cutting line"))
                                        .child(crate::assets::icon("section-cut-line", 20., cx))
                                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                            let m = this.section_dlg.as_ref().map(|d| d.mode).unwrap_or(0);
                                            this.set_section_mode(m, cx);
                                        })),
                                )
                                .child(div().flex_1())
                                .child(
                                    div()
                                        .id("ds-flip")
                                        .size(px(26.))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .border_1()
                                        .border_color(gpui_kit::rgb(0xd3d7d9))
                                        .when(flip, |d| d.bg(gpui_kit::rgb(0xcfdbe5)))
                                        .cursor_pointer()
                                        .tooltip(crate::ui::tooltip("Flip direction"))
                                        .child(crate::assets::icon("section-flip", 20., cx))
                                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                            if let Some(d) = &mut this.section_dlg {
                                                d.flip = !d.flip;
                                            }
                                            this.update_preview(cx);
                                            cx.notify();
                                        })),
                                ),
                        )
                        .when_some(d.angle_input.as_ref().filter(|_| mode == 2).map(|x| x.0.clone()), |e, st| {
                            e.child(
                                h_flex()
                                    .h(px(30.))
                                    .px(px(6.))
                                    .gap(px(6.))
                                    .child(div().text_size(px(12.)).text_color(pal.text_muted).child("Cut angle"))
                                    .child(div().flex_1().child(Input::new(&st).text_size(px(13.33)).text_color(gpui_kit::rgb(0x555555)).small()))
                                    .child(
                                        div()
                                            .id("ds-opposite")
                                            .size(px(24.))
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .border_1()
                                            .border_color(gpui_kit::rgb(0xd3d7d9))
                                            .when(opp, |d| d.bg(gpui_kit::rgb(0xcfdbe5)))
                                            .cursor_pointer()
                                            .tooltip(crate::ui::tooltip("Opposite direction"))
                                            .child(crate::assets::icon(&crate::assets::icon_path("flip"), 16., cx))
                                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                                if let Some(d) = &mut this.section_dlg {
                                                    d.opposite = !d.opposite;
                                                }
                                                this.update_preview(cx);
                                                cx.notify();
                                            })),
                                    ),
                            )
                        })
                        .child(div().h(px(26.)).flex().items_center().px(px(4.)).child(Checkbox::new("ds-cut-only").label("Show cut geometry only").small().checked(d.cut_only).on_click(cx.listener(|this, v: &bool, _, cx| {
                            if let Some(d) = &mut this.section_dlg {
                                d.cut_only = *v;
                            }
                            this.update_preview(cx);
                            cx.notify();
                        }))))
                        .when(of_section, |e| {
                            e.child(div().h(px(26.)).flex().items_center().px(px(4.)).child(Checkbox::new("ds-cut-from").label("Cut from section").small().checked(d.cut_from).on_click(cx.listener(|this, v: &bool, _, cx| {
                                if let Some(d) = &mut this.section_dlg {
                                    d.cut_from = *v;
                                }
                                this.update_preview(cx);
                                cx.notify();
                            }))))
                        })
                        .child(div().h(px(26.)).flex().items_center().px(px(4.)).child(Checkbox::new("ds-depth").label("Section depth").small().checked(d.depth.is_some()).on_click(cx.listener(|this, v: &bool, w, cx| this.toggle_section_depth(*v, w, cx)))))
                        .when(d.depth.is_some(), |e| {
                            let tab = |id: &'static str, label: &'static str, on: bool, v: bool, cx: &mut Context<Self>| {
                                div()
                                    .id(id)
                                    .h(px(24.))
                                    .px(px(7.))
                                    .flex()
                                    .items_center()
                                    .text_size(px(12.))
                                    .cursor_pointer()
                                    .border_b_1()
                                    .when(on, |d| d.border_b_2().border_color(pal.accent).text_color(pal.emphasis))
                                    .when(!on, |d| d.border_color(gpui_kit::rgb(0xdddddd)))
                                    .child(label)
                                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                        if let Some(d) = &mut this.section_dlg {
                                            d.up_to = v;
                                            d.picking = if v && d.depth_ref.is_none() { 1 } else { 0 };
                                        }
                                        this.update_preview(cx);
                                        cx.notify();
                                    }))
                            };
                            e.child(h_flex().px(px(4.)).child(tab("ds-blind", "Blind", !d.up_to, false, cx)).child(tab("ds-upto", "Up to entity", d.up_to, true, cx)))
                        })
                        .when_some(d.depth.as_ref().map(|x| x.0.clone()).filter(|_| !d.up_to), |e, st| e.child(h_flex().h(px(30.)).px(px(6.)).gap(px(6.)).child(div().text_size(px(12.)).text_color(pal.text_muted).child("Depth")).child(div().flex_1().child(Input::new(&st).text_size(px(13.33)).text_color(gpui_kit::rgb(0x555555)).small()))))
                        .when(d.depth.is_some() && d.up_to, |e| {
                            let label = if d.depth_ref.is_some() { "1 entity" } else { "Select a vertex" };
                            let on = d.picking == 1;
                            e.child(
                                div()
                                    .id("ds-upto-field")
                                    .mx(px(6.))
                                    .my(px(3.))
                                    .h(px(26.))
                                    .px(px(6.))
                                    .flex()
                                    .items_center()
                                    .text_size(px(12.))
                                    .border_1()
                                    .border_color(if on { pal.accent } else { gpui_kit::rgb(0xd3d7d9) })
                                    .when(on, |d| d.bg(gpui_kit::rgb(0xe8f1fb)))
                                    .cursor_pointer()
                                    .child(label)
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                        if let Some(d) = &mut this.section_dlg {
                                            d.picking = if d.picking == 1 { 0 } else { 1 };
                                        }
                                        cx.notify();
                                    })),
                            )
                        })
                        .when(assembly, |e| {
                            let n = d.exclude.len();
                            let on = d.picking == 2;
                            e.child(div().px(px(6.)).pt(px(4.)).text_size(px(12.)).text_color(pal.text_muted).child("Exclude from cut")).child(
                                div()
                                    .id("ds-exclude")
                                    .mx(px(6.))
                                    .my(px(3.))
                                    .h(px(26.))
                                    .px(px(6.))
                                    .flex()
                                    .items_center()
                                    .text_size(px(12.))
                                    .border_1()
                                    .border_color(if on { pal.accent } else { gpui_kit::rgb(0xd3d7d9) })
                                    .when(on, |d| d.bg(gpui_kit::rgb(0xe8f1fb)))
                                    .cursor_pointer()
                                    .child(if n == 0 { "Select parts".to_string() } else { format!("{n} part{}", if n == 1 { "" } else { "s" }) })
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                        if let Some(d) = &mut this.section_dlg {
                                            d.picking = if d.picking == 2 { 0 } else { 2 };
                                        }
                                        cx.notify();
                                    })),
                            )
                        }),
                )
                .into_any_element(),
        )
    }
}
