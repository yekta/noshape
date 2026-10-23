//! Drawing editor: Onshape's Auxiliary view dialog (while the tool is
//! active): the view label, Normal (perpendicular to a linear edge), In line
//! (about a sheet sketch line or a centerline), Axis (down a hole's axis),
//! the reverse button and "Show view reference indicator". The view then
//! follows the cursor along the fold direction; a click places it.

use crate::drawing_view::*;
use crate::theme::p;
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::drawing::ViewId;

/// Pushes one highlight segment (from, to, colour, width in px).
type SegFn = dyn FnMut(&mut noshape_render::LineData, [f64; 2], [f64; 2], [u8; 4], f32);

/// Auxiliary view dialog state.
pub struct AuxDlg {
    /// 0 Normal, 1 In line, 2 Axis.
    pub mode: u8,
    pub label: Entity<InputState>,
    pub reverse: bool,
    pub show_ref: bool,
    /// Where the picked edge / line is (parent view coordinates).
    pub anchor: Option<[f64; 2]>,
}

fn sub2(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    [a[0] - b[0], a[1] - b[1]]
}
fn unit(a: [f64; 2]) -> [f64; 2] {
    let l = (a[0] * a[0] + a[1] * a[1]).sqrt().max(1e-12);
    [a[0] / l, a[1] / l]
}

impl DrawingEditor {
    /// Create / drop the dialog with the Auxiliary view tool (called from render).
    pub fn sync_aux_dlg(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let active = matches!(self.tool, Tool::Auxiliary { .. });
        match (self.aux_dlg.is_some(), active) {
            (false, true) => {
                let next = self.current_sheet().next_label();
                let label = cx.new(|cx| InputState::new(window, cx).select_on_focus(false).revert_on_escape(false).placeholder("Auxiliary id").placeholder_style(gpui_kit::base::input::PlaceholderStyle { italic: false, weight: Some(gpui_kit::FontWeight::NORMAL), color: Some(gpui_kit::rgb(0x999999).into()) }).default_value(next));
                self.aux_dlg = Some(AuxDlg { mode: 0, label, reverse: false, show_ref: true, anchor: None });
            }
            (true, false) => self.aux_dlg = None,
            _ => {}
        }
    }

    /// The fold edge for the dialog's mode under the cursor: (view, edge
    /// direction on the sheet, anchor in view coordinates).
    pub fn aux_pick(&mut self) -> Option<(noshape_model::drawing::ViewId, [f64; 2], [f64; 2])> {
        let mode = self.aux_dlg.as_ref().map(|d| d.mode).unwrap_or(0);
        self.pk().aux_pick(mode)
    }

    /// Reference indicator › Edit auxiliary view: the dialog for that view
    /// (label, Reverse, Show view reference indicator; the checkmark applies).
    pub fn edit_aux_view(&mut self, id: noshape_model::drawing::ViewId, window: &mut Window, cx: &mut Context<Self>) {
        let Some(v) = self.current_sheet().view(id).cloned() else { return };
        let r = v.aux_ref.clone();
        let label = cx.new(|cx| InputState::new(window, cx).select_on_focus(false).revert_on_escape(false).placeholder("Auxiliary id").placeholder_style(gpui_kit::base::input::PlaceholderStyle { italic: false, weight: Some(gpui_kit::FontWeight::NORMAL), color: Some(gpui_kit::rgb(0x999999).into()) }).default_value(r.as_ref().map(|r| r.letter.clone()).unwrap_or_default()));
        self.aux_edit = Some((id, AuxDlg { mode: 0, label, reverse: v.flip, show_ref: r.as_ref().is_none_or(|r| r.show), anchor: r.map(|r| r.anchor) }));
        cx.notify();
    }

    fn apply_aux_edit(&mut self, cx: &mut Context<Self>) {
        let Some((id, d)) = self.aux_edit.take() else { return };
        let letter = d.label.read(cx).value().trim().to_string();
        self.edit_view("Edit auxiliary view", id, cx, |v| {
            v.flip = d.reverse;
            if let Some(r) = &mut v.aux_ref {
                r.letter = letter;
                r.show = d.show_ref;
            }
        });
    }

    /// The editing dialog (an existing auxiliary view).
    pub fn render_aux_edit(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (_, d) = self.aux_edit.as_ref()?;
        let pal = p();
        let (reverse, show) = (d.reverse, d.show_ref);
        Some(
            v_flex()
                .id("dr-aux-edit")
                .absolute()
                .top(px(DIALOG_TOP))
                .left(px(self.dialog_left()))
                .w(px(240.))
                .bg(pal.dialog_bg)
                .shadow(crate::drawing_chrome::dialog_shadow())
                .text_color(pal.text)
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    h_flex()
                        .h(px(28.))
                        .pl(px(11.))
                        .border_b_1()
                        .border_color(gpui_kit::rgb(0xd3d7d9))
                        .child(div().flex_1().text_size(px(13.)).font_weight(FontWeight::MEDIUM).child("Auxiliary view"))
                        .child(div().id("auxe-ok").w(px(28.)).h(px(27.)).flex().items_center().justify_center().bg(gpui_kit::rgb(0x009500)).cursor_pointer().child(crate::ui::glyph("check", 18., gpui_kit::white())).on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.apply_aux_edit(cx))))
                        .child(div().id("auxe-cancel").w(px(28.)).h(px(27.)).flex().items_center().justify_center().cursor_pointer().child(crate::drawing_chrome::cancel_x(cx)).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.aux_edit = None;
                            cx.notify();
                        }))),
                )
                .child(
                    v_flex()
                        .p(px(3.))
                        .child(gpui_kit::base::input::input_frame(div().mt(px(1.)).border_b_1().border_color(pal.accent).child(Input::new(&d.label).appearance(false).small().text_size(px(13.33)).text_color(gpui_kit::rgb(0x555555)).pl(px(3.)))))
                        .child(div().h(px(26.)).flex().items_center().px(px(4.)).child(Checkbox::new("auxe-rev").label("Reverse").small().checked(reverse).on_click(cx.listener(|this, v: &bool, _, cx| {
                            if let Some((_, d)) = &mut this.aux_edit {
                                d.reverse = *v;
                            }
                            cx.notify();
                        }))))
                        .child(div().h(px(26.)).flex().items_center().px(px(4.)).child(Checkbox::new("auxe-show").label("Show view reference indicator").small().checked(show).on_click(cx.listener(|this, v: &bool, _, cx| {
                            if let Some((_, d)) = &mut this.aux_edit {
                                d.show_ref = *v;
                            }
                            cx.notify();
                        })))),
                )
                .into_any_element(),
        )
    }

    pub fn render_aux_dlg(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let d = self.aux_dlg.as_ref()?;
        let pal = p();
        let (mode, reverse, show) = (d.mode, d.reverse, d.show_ref);
        let tab = |id: &'static str, label: &'static str, m: u8, cx: &mut Context<Self>| {
            let on = mode == m;
            div()
                .id(id)
                .h(px(26.))
                .px(px(8.))
                .flex()
                .items_center()
                .text_size(px(13.33))
                .cursor_pointer()
                .border_b_1()
                .when(on, |d| d.border_b_2().border_color(pal.accent).text_color(pal.accent).font_weight(FontWeight::MEDIUM))
                .when(!on, |d| d.border_color(gpui_kit::rgb(0xdddddd)).text_color(pal.text))
                .child(label)
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    if let Some(d) = &mut this.aux_dlg {
                        d.mode = m;
                        d.anchor = None;
                    }
                    this.tool = Tool::Auxiliary { parent: None };
                    this.update_preview(cx);
                    cx.notify();
                }))
        };
        let picked = matches!(self.tool, Tool::Auxiliary { parent: Some(_) });
        let field_text = match (picked, mode) {
            (true, _) => "1 entity",
            (false, 1) => "Select a sketch line or centerline",
            (false, 2) => "Select a hole axis",
            _ => "Select a linear edge",
        };
        Some(
            v_flex()
                .id("dr-aux-dlg")
                .absolute()
                .top(px(DIALOG_TOP))
                .left(px(self.dialog_left()))
                .w(px(240.))
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
                        .child(div().flex_1().text_size(px(13.)).font_weight(FontWeight::MEDIUM).child("Auxiliary view"))
                        .child(
                            div()
                                .id("aux-cancel")
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
                        .child(gpui_kit::base::input::input_frame(div().mt(px(1.)).border_b_1().border_color(pal.accent).child(Input::new(&d.label).appearance(false).small().text_size(px(13.33)).text_color(gpui_kit::rgb(0x555555)).pl(px(3.)))))
                        .child(h_flex().mt(px(3.)).child(tab("aux-normal", "Normal", 0, cx)).child(tab("aux-inline", "In line", 1, cx)).child(tab("aux-axis", "Axis", 2, cx)))
                        .child(
                            h_flex()
                                .mt(px(3.))
                                .gap(px(4.))
                                .child(
                                    div()
                                        .flex_1()
                                        .h(px(26.))
                                        .px(px(6.))
                                        .flex()
                                        .items_center()
                                        .text_size(px(12.))
                                        .border_1()
                                        .border_color(if picked { gpui_kit::rgb(0xd3d7d9) } else { pal.accent })
                                        .when(!picked, |d| d.bg(gpui_kit::rgb(0xe8f1fb)))
                                        .child(field_text),
                                )
                                .child(
                                    div()
                                        .id("aux-reverse")
                                        .size(px(26.))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .border_1()
                                        .border_color(gpui_kit::rgb(0xd3d7d9))
                                        .when(reverse, |d| d.bg(gpui_kit::rgb(0xcfdbe5)))
                                        .cursor_pointer()
                                        .tooltip(crate::ui::tooltip("Reverse"))
                                        .child(crate::assets::icon(&crate::assets::icon_path("flip"), 18., cx))
                                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                            if let Some(d) = &mut this.aux_dlg {
                                                d.reverse = !d.reverse;
                                            }
                                            this.update_preview(cx);
                                            cx.notify();
                                        })),
                                ),
                        )
                        .child(div().h(px(26.)).flex().items_center().px(px(4.)).child(Checkbox::new("aux-show-ref").label("Show view reference indicator").small().checked(show).on_click(cx.listener(|this, v: &bool, _, cx| {
                            if let Some(d) = &mut this.aux_dlg {
                                d.show_ref = *v;
                            }
                            this.update_preview(cx);
                            cx.notify();
                        })))),
                )
                .into_any_element(),
        )
    }
}

/// The auxiliary view reference indicator once placed (live behaviours
/// "Auxiliary view reference indicator", asb 10-20 u): five snap points (the
/// line's two ends, its middle and the two letters); an end lengthens or
/// shortens the line symmetrically, the middle moves it parallel (no
/// limit), a letter moves alone. The parent edge is highlighted while the
/// view is selected.
impl DrawingEditor {
    /// Auxiliary view `v`'s reference indicator on the sheet, when shown.
    pub fn aux_indicator_of(&self, v: ViewId) -> Option<noshape_model::drawing::build::AuxIndicator> {
        let sheet = self.current_sheet();
        sheet.view(v)?.aux_ref.as_ref().filter(|a| a.show && self.drawing.props.view_tab.aux_indicator)?;
        let parent = sheet.parent_of(v)?;
        let pl = self.placement(parent)?;
        noshape_model::drawing::build::aux_indicator(sheet, v, &pl, self.geoms.get(&parent).map(|g| g.as_ref()))
    }

    /// The views whose indicator shows its five dots: the view or the
    /// indicator selected, or the indicator hovered (live u-905…907).
    pub fn aux_dot_views(&self) -> Vec<ViewId> {
        let mut out: Vec<ViewId> = self.selection.iter().filter_map(|s| if let Sel::View(v) | Sel::AuxIndicator(v) = s { Some(*v) } else { None }).collect();
        if let Some(Sel::AuxIndicator(v)) = self.hover.and_then(|h| self.sel_of_pick(h)) {
            out.push(v);
        }
        out.sort_unstable_by_key(|v| v.0);
        out.dedup();
        out
    }

    /// The indicator snap point of view `v` under sheet point `s`: 0 / 1 an
    /// end, 2 the middle, 3 / 4 a letter.
    pub fn aux_grip_at(&self, v: ViewId, s: [f64; 2]) -> Option<usize> {
        let tol = self.tol_mm(5.0);
        self.aux_dots(v).into_iter().find(|(_, p)| (p[0] - s[0]).hypot(p[1] - s[1]) < tol).map(|(k, _)| k)
    }

    /// View `v`'s indicator dots (grip number, sheet point): Line style the
    /// ends, middle and letters; Arrow style the arrow's tail and letter.
    pub fn aux_dots(&self, v: ViewId) -> Vec<(usize, [f64; 2])> {
        let Some(i) = self.aux_indicator_of(v) else { return vec![] };
        if self.drawing.props.view_tab.aux_arrow_format {
            let Some(a) = self.current_sheet().view(v).and_then(|x| x.aux_ref.as_ref()) else { return vec![] };
            return vec![(2, i.mid), (3, noshape_model::drawing::build::aux_arrow_letter(&i, a))];
        }
        vec![(2, i.mid), (0, i.elbows[0]), (1, i.elbows[1]), (3, i.letters[0]), (4, i.letters[1])]
    }

    /// Drag snap point `grip` of view `view`'s indicator to sheet point `c`.
    pub fn drag_aux_grip(sheet: &mut noshape_model::drawing::Sheet, view: ViewId, grip: usize, orig: &noshape_model::drawing::AuxRef, ind: &noshape_model::drawing::build::AuxIndicator, c: [f64; 2], arrow: bool) {
        let Some(a) = sheet.view_mut(view).and_then(|v| v.aux_ref.as_mut()) else { return };
        // `ind` is the indicator as last drawn (with the drag so far).
        let drawn = a.letter_at.unwrap_or([[0.0; 2]; 2]);
        *a = orig.clone();
        match grip {
            // The middle: a parallel move, across the edge or into the view too.
            2 => {
                let d = sub2(c, ind.anchor);
                a.offset = Some(d[0] * ind.ns[0] + d[1] * ind.ns[1]);
            }
            // An end: both ends move about the middle.
            0 | 1 => {
                let centre = [(ind.elbows[0][0] + ind.elbows[1][0]) / 2.0, (ind.elbows[0][1] + ind.elbows[1][1]) / 2.0];
                let d = sub2(c, centre);
                let half = (d[0] * ind.es[0] + d[1] * ind.es[1]).abs().max(1.0);
                a.ext = Some([half; 2]);
            }
            // A letter: that letter alone, anywhere.
            k => {
                let i = k - 3;
                let mut at = orig.letter_at.unwrap_or([[0.0; 2]; 2]);
                let shown = if arrow { noshape_model::drawing::build::aux_arrow_letter(ind, &noshape_model::drawing::AuxRef { letter_at: Some(drawn), ..orig.clone() }) } else { ind.letters[i] };
                let base = [shown[0] - drawn[i][0], shown[1] - drawn[i][1]];
                at[i] = sub2(c, base);
                a.letter_at = Some(at);
            }
        }
    }

    /// The parent edge of the selected auxiliary views (sheet segments).
    pub fn aux_edge_highlight(&self) -> Vec<([f64; 2], [f64; 2])> {
        self.selection.iter().filter_map(|s| if let Sel::View(v) = s { Some(*v) } else { None }).flat_map(|v| self.aux_parent_edge(v)).collect()
    }

    /// Auxiliary view `v`'s edge in its parent (sheet segments).
    fn aux_parent_edge(&self, v: ViewId) -> Vec<([f64; 2], [f64; 2])> {
        let Some(b) = &self.built else { return vec![] };
        let sheet = self.current_sheet();
        let mut out = Vec::new();
        {
            let Some(view) = sheet.view(v) else { return out };
            let (noshape_model::drawing::ViewKind::Auxiliary { parent, edge_dir }, Some(a)) = (&view.kind, view.aux_ref.as_ref()) else { return out };
            let Some(pl) = self.placement(*parent) else { return out };
            let (c, e) = (pl.to_sheet(a.anchor), unit(pl.dir_to_sheet(*edge_dir)));
            for l in b.geometry.lines.iter().filter(|l| matches!(b.picks.get(l.pick as usize), Some(noshape_model::drawing::build::PickTarget::Edge { view, .. }) if view == parent)) {
                for w in l.points.windows(2) {
                    let d = unit(sub2(w[1], w[0]));
                    if (d[0] * e[1] - d[1] * e[0]).abs() < 0.02 && noshape_model::drawing::views::dist_to_segment(c, w[0], w[1]) < 0.05 {
                        out.push((w[0], w[1]));
                    }
                }
            }
        }
        out
    }

    /// Selection looks of views and indicators (live u-905…907, behaviours
    /// 2815): a selected view gets an orange frame, its indicator and parent
    /// edge turn light blue; a selected indicator turns orange (the pick
    /// state), its parent edge orange with light-blue end dots and its view
    /// a light-blue frame. The indicator's five dots show while the view or
    /// the indicator is selected or the indicator hovered; not while dragged.
    pub fn push_aux_states(&self, scene: &mut noshape_render::scene::Scene) {
        use noshape_model::drawing::build::PickTarget;
        use noshape_model::drawing::font;
        use noshape_render::{Drawable, Instance, LineData, LineSegment, MeshData, PointData, PointSprite};
        const ORANGE: [u8; 4] = [0xcc, 0x92, 0x5c, 255];
        const BLUE: [u8; 4] = [0x90, 0xce, 0xf1, 255];
        let Some(b) = &self.built else { return };
        let sheet = self.current_sheet();
        let mut lines = LineData::default();
        let mut points = PointData::default();
        let mut blue = MeshData::default();
        let width = crate::drawing_scene::px_width(noshape_model::drawing::sheet::LineStyle::Visible, self.zoom).max(2.0);
        let mut seg = |l: &mut LineData, a: [f64; 2], c: [f64; 2], color: [u8; 4], w: f32| l.segments.push(LineSegment { a: [a[0] as f32, a[1] as f32, 0.6], b: [c[0] as f32, c[1] as f32, 0.6], color, pick_id: 0, width: w, style: LineSegment::ON_TOP });
        let dot = |pts: &mut PointData, p: [f64; 2], color: [u8; 4]| pts.points.push(PointSprite { position: [p[0] as f32, p[1] as f32, 0.7], color, pick_id: 0, size: 8.0, style: LineSegment::ON_TOP, _pad: 0 });
        let tri = |m: &mut MeshData, pts: &[[f64; 2]], idx: &[u32]| {
            let base = m.positions.len() as u32;
            for p in pts {
                m.positions.push([p[0] as f32, p[1] as f32, 0.5]);
                m.normals.push([0.0, 0.0, 1.0]);
                m.pick_ids.push(0);
            }
            m.indices.extend(idx.iter().map(|i| base + i));
        };
        let frame = |l: &mut LineData, v: ViewId, color: [u8; 4], seg: &mut SegFn| {
            let Some(pl) = self.placement(v) else { return };
            let m = 1.0;
            let (x0, y0, x1, y1) = (pl.min[0] - m, pl.min[1] - m, pl.max[0] + m, pl.max[1] + m);
            for (a, c) in [([x0, y0], [x1, y0]), ([x1, y0], [x1, y1]), ([x1, y1], [x0, y1]), ([x0, y1], [x0, y0])] {
                seg(l, a, c, color, 2.0);
            }
        };
        for s in &self.selection {
            match s {
                Sel::View(v) if !v.is_ghost() => {
                    frame(&mut lines, *v, ORANGE, &mut seg);
                    // An auxiliary view's indicator in light blue.
                    let is_ind = |p: u32| matches!(b.picks.get(p as usize), Some(PickTarget::AuxIndicator(x)) if x == v);
                    for l in b.geometry.lines.iter().filter(|l| is_ind(l.pick)) {
                        for w in l.points.windows(2) {
                            seg(&mut lines, w[0], w[1], BLUE, crate::drawing_scene::px_width(l.style, self.zoom).max(1.5));
                        }
                    }
                    // Arrowheads (fills carry no pick): those at the indicator's tips.
                    let tips = self.aux_indicator_of(*v).map(|i| i.tips).unwrap_or_default();
                    let at_tip = |p: [f64; 2]| tips.iter().any(|t| (t[0] - p[0]).abs() < 1e-6 && (t[1] - p[1]).abs() < 1e-6);
                    for f in b.geometry.fills.iter().filter(|f| f.points.len() >= 3 && at_tip(f.points[0])) {
                        let idx: Vec<u32> = (1..f.points.len() as u32 - 1).flat_map(|i| [0, i, i + 1]).collect();
                        tri(&mut blue, &f.points, &idx);
                    }
                    for (ti, t) in b.geometry.texts.iter().enumerate().filter(|(_, t)| is_ind(t.pick)) {
                        let (pts, idx) = font::text_fill_font(t, b.geometry.text_font(ti));
                        tri(&mut blue, &pts, &idx);
                    }
                    for (a, c) in self.aux_parent_edge(*v) {
                        dot(&mut points, a, BLUE);
                        dot(&mut points, c, BLUE);
                    }
                }
                Sel::AuxIndicator(v) => {
                    frame(&mut lines, *v, BLUE, &mut seg);
                    for (a, c) in self.aux_parent_edge(*v) {
                        seg(&mut lines, a, c, ORANGE, width);
                        dot(&mut points, a, BLUE);
                        dot(&mut points, c, BLUE);
                    }
                }
                _ => {}
            }
        }
        if !matches!(self.drag, Drag::AuxIndicator { .. }) {
            for v in self.aux_dot_views().into_iter().filter(|v| sheet.view(*v).is_some()) {
                for (_, p) in self.aux_dots(v) {
                    dot(&mut points, p, ORANGE);
                }
            }
        }
        let up = self.gpu.uploader();
        if !blue.indices.is_empty() {
            scene.instances.push(Instance::new(Drawable::Mesh(up.upload_mesh(&blue))).with_color(crate::theme::lin(0x90cef1)).unlit().unpickable().layer(19));
        }
        if !lines.segments.is_empty() {
            scene.instances.push(Instance::new(Drawable::Lines(up.upload_lines(&lines))).per_primitive_color().unpickable().layer(20));
        }
        if !points.points.is_empty() {
            scene.instances.push(Instance::new(Drawable::Points(up.upload_points(&points))).per_primitive_color().unpickable().layer(22));
        }
    }
}
