//! Drawing editor: Onshape's Detail view dialog (while the tool is active):
//! the profile tool (Circle, Rectangle, Spline, Polygon), View scale, View
//! label, Profile style (In line / Connected / Leader), "Show detail view
//! profile" and "Clip border to geometry". Circle: center, then the
//! circumference; Rectangle: two corners; Spline / Polygon: click points,
//! double-click (or click the first point) to close; then drag and click
//! to place the detail view.

use crate::drawing_view::*;
use crate::theme::p;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputState};
use crate::os_ui::OsDropdownExt as _;
use gpui_kit::component::{Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::drawing::{ViewId, views};

/// Detail view dialog state.
pub struct DetailDlg {
    /// 0 Circle, 1 Rectangle, 2 Spline, 3 Polygon.
    pub shape: u8,
    /// Profile points so far (rectangle / spline / polygon), parent view coordinates.
    pub pts: Vec<[f64; 2]>,
    pub view: Option<ViewId>,
    /// The finished profile (polygon, parent view coordinates).
    pub profile: Option<Vec<[f64; 2]>>,
    /// View scale (None: twice the parent's).
    pub scale: Option<f64>,
    pub label: Entity<InputState>,
    /// 0 In line, 1 Connected, 2 Leader.
    pub style: u8,
    pub show_profile: bool,
    pub clip_border: bool,
    /// Esc pressed before the profile: the next click picks an existing
    /// closed sketch region as the profile.
    pub pick_region: bool,
    /// Edit detail... (live asb 10-23 views (1)): the detail view being
    /// edited; ✓ applies the dialog to it, ✕ discards.
    pub edit: Option<ViewId>,
}

const SHAPES: [(&str, &str); 4] = [("Circle", "circle"), ("Rectangle", "corner-rectangle"), ("Spline", "spline"), ("Polygon", "polyline")];
const STYLES: [&str; 3] = ["In line", "Connected", "Leader"];
const SCALES: [f64; 9] = [1.0, 1.5, 2.0, 2.5, 3.0, 4.0, 5.0, 10.0, 20.0];

impl DrawingEditor {
    /// Create / drop the dialog with the Detail view tool (called from render).
    pub fn sync_detail_dlg(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let active = matches!(self.tool, Tool::Detail { .. });
        match (self.detail_dlg.is_some(), active) {
            (false, true) => {
                let next = self.current_sheet().next_label();
                let label = cx.new(|cx| InputState::new(window, cx).select_on_focus(false).revert_on_escape(false).default_value(next));
                self.detail_dlg = Some(DetailDlg { shape: 0, pts: Vec::new(), view: None, profile: None, scale: None, label, style: self.drawing.props.view_tab.detail_style, show_profile: self.drawing.props.view_tab.detail_show_profile, clip_border: self.drawing.props.view_tab.detail_clip_border, pick_region: false, edit: None });
            }
            (true, false) if self.detail_dlg.as_ref().is_some_and(|d| d.edit.is_none()) => self.detail_dlg = None,
            _ => {}
        }
        if let Some(d) = &self.detail_dlg {
            self.detail_label = d.label.read(cx).value().trim().to_string();
        }
    }

    /// Esc before a profile is started: pick an existing closed region
    /// instead (Onshape: "press Esc and click the region"). True when used.
    pub fn detail_escape(&mut self, cx: &mut Context<Self>) -> bool {
        let fresh = matches!(self.tool, Tool::Detail { center: None, .. });
        let Some(d) = &mut self.detail_dlg else { return false };
        if !fresh || d.pick_region || !d.pts.is_empty() || d.profile.is_some() {
            return false;
        }
        d.pick_region = true;
        cx.notify();
        true
    }

    /// Region-pick click: the smallest closed sheet sketch region under the
    /// cursor becomes the profile of the view it lies on.
    fn detail_region_click(&mut self, cx: &mut Context<Self>) {
        let c = self.cursor;
        let regions = self.sketch().sketch.regions();
        let p = noshape_sketch::DVec2::new(c[0], c[1]);
        let Some(r) = regions.iter().filter(|r| r.contains(p)).min_by(|a, b| a.area().abs().total_cmp(&b.area().abs())) else {
            self.status = Some("Click inside a closed sketch region".into());
            cx.notify();
            return;
        };
        let outer: Vec<[f64; 2]> = r.outer.tessellate(0.05).into_iter().map(|q| [q.x, q.y]).collect();
        let n = outer.len().max(1) as f64;
        let mid = [outer.iter().map(|q| q[0]).sum::<f64>() / n, outer.iter().map(|q| q[1]).sum::<f64>() / n];
        let near = self.built.as_ref().and_then(|b| b.placements.iter().map(|pl| (pl.dist_to(mid), pl.id)).filter(|(d, _)| *d < 20.0).min_by(|a, b| a.0.total_cmp(&b.0)).map(|(_, id)| id));
        let Some((view, pl)) = self.view_at(mid).or(near).and_then(|v| Some((v, self.placement(v)?))) else {
            self.status = Some("The region is not on a view".into());
            cx.notify();
            return;
        };
        if let Some(d) = &mut self.detail_dlg {
            d.view = Some(view);
            d.shape = 3;
            d.pts = outer.iter().map(|q| pl.to_view(*q)).collect();
            d.pick_region = false;
        }
        self.status = None;
        self.finish_detail_profile(cx);
    }

    /// A profile click (Rectangle / Spline / Polygon). True when used.
    pub fn detail_click(&mut self, cx: &mut Context<Self>) -> bool {
        if self.detail_dlg.as_ref().is_some_and(|d| d.pick_region) {
            self.detail_region_click(cx);
            return true;
        }
        let shape = match &self.detail_dlg {
            Some(d) if d.shape != 0 && d.profile.is_none() => d.shape,
            _ => return false,
        };
        let c = self.cursor;
        // The profile may start just outside the view: the nearest view within 20 mm.
        let near = || self.built.as_ref()?.placements.iter().map(|p| (p.dist_to(c), p.id)).filter(|(d, _)| *d < 20.0).min_by(|a, b| a.0.total_cmp(&b.0)).map(|(_, id)| id);
        let view = self.detail_dlg.as_ref().and_then(|d| d.view).or_else(|| self.view_at(c)).or_else(near);
        let Some(view) = view else { return true };
        if self.current_sheet().view(view).is_some_and(|v| matches!(v.kind, noshape_model::drawing::ViewKind::Detail { .. })) {
            self.status = Some("Detail views can't be created from detail views".into());
            return true;
        }
        let Some(pl) = self.placement(view) else { return true };
        let p = self.snap_at(c).filter(|s| s.view == view).map(|s| s.p).unwrap_or(pl.to_view(c));
        let tol = self.tol_mm(8.0);
        let Some(d) = &mut self.detail_dlg else { return true };
        d.view = Some(view);
        // Clicking the first point again closes a spline / polygon.
        let close = d.pts.len() >= 3 && pl.len_to_sheet(((p[0] - d.pts[0][0]).powi(2) + (p[1] - d.pts[0][1]).powi(2)).sqrt()) < tol;
        if !close {
            d.pts.push(p);
        }
        if (shape == 1 && d.pts.len() == 2) || close {
            self.finish_detail_profile(cx);
        }
        self.update_preview(cx);
        true
    }

    /// Close the profile (double-click / first point / rectangle's second
    /// corner): the detail then follows the cursor. True when closed.
    pub fn finish_detail_profile(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(d) = &mut self.detail_dlg else { return false };
        let (Some(view), true) = (d.view, d.shape != 0 && d.profile.is_none()) else { return false };
        let Some((poly, center, radius)) = noshape_scene::drawing_tools::detail_profile(d.shape, &d.pts) else { return false };
        d.profile = Some(poly);
        self.tool = Tool::Detail { parent: Some(view), center: Some(center), radius: Some(radius) };
        self.update_preview(cx);
        cx.notify();
        true
    }

    /// Edit detail... on a detail view or its circle: the Detail view panel
    /// with the view's values (live asb 10-23 views (1)). Picking a profile
    /// tool and drawing a new profile replaces it.
    pub fn open_detail_edit(&mut self, id: ViewId, window: &mut Window, cx: &mut Context<Self>) {
        let Some(v) = self.current_sheet().view(id).cloned() else { return };
        let noshape_model::drawing::ViewKind::Detail { parent, label: text, .. } = &v.kind else { return };
        let label = cx.new(|cx| InputState::new(window, cx).select_on_focus(false).revert_on_escape(false).default_value(text.clone()));
        let profile = v.detail_profile.clone().filter(|p| p.len() >= 3);
        self.cancel(cx);
        self.tool = Tool::Select;
        self.detail_dlg = Some(DetailDlg { shape: if profile.is_some() { 3 } else { 0 }, pts: Vec::new(), view: Some(*parent), profile, scale: v.scale, label, style: v.detail_style, show_profile: !v.detail_hide_profile, clip_border: v.detail_clip_border, pick_region: false, edit: Some(id) });
        self.selection = vec![Sel::View(id)];
        self.update_preview(cx);
        cx.notify();
    }

    /// The drawing with the edited detail view as the dialog has it (None
    /// when not editing).
    pub fn detail_edit_drawing(&self, cx: &App) -> Option<noshape_model::drawing::Drawing> {
        let d = self.detail_dlg.as_ref()?;
        let id = d.edit?;
        let mut dr = self.drawing.clone();
        let v = dr.sheets.get_mut(self.sheet)?.view_mut(id)?;
        let noshape_model::drawing::ViewKind::Detail { parent, center, radius, label } = &mut v.kind else { return None };
        // A newly drawn profile (or circle) on the same parent replaces the old one.
        if let Tool::Detail { parent: Some(p), center: Some(c), radius: Some(r) } = &self.tool
            && p == parent
        {
            *center = *c;
            *radius = *r;
            v.detail_profile = d.profile.clone().filter(|_| d.shape != 0);
        }
        let text = d.label.read(cx).value().trim().to_string();
        if !text.is_empty() {
            *label = text;
        }
        if let Some(s) = d.scale {
            v.scale = Some(s);
        }
        v.detail_style = d.style;
        v.detail_hide_profile = !d.show_profile;
        v.detail_clip_border = d.clip_border;
        Some(dr)
    }

    /// ✓ (Edit detail): apply the dialog to the view; ✕ discards.
    pub fn close_detail_edit(&mut self, ok: bool, cx: &mut Context<Self>) {
        let edited = if ok { self.detail_edit_drawing(cx) } else { None };
        self.detail_dlg = None;
        self.tool = Tool::Select;
        self.set_preview(None, cx);
        if let Some(dr) = edited {
            let s = dr.sheets[self.sheet].clone();
            self.edit_sheet("Edit detail view", cx, |x| *x = s);
        }
        cx.notify();
    }

    /// Detail view `v`'s circle in its parent (sheet mm: centre, radius)
    /// and its resize grip (live asbv-8: on the circle's right); None for a
    /// rectangle / spline / polygon profile.
    pub fn detail_grip(&self, v: ViewId) -> Option<([f64; 2], f64, [f64; 2])> {
        if self.current_sheet().view(v)?.detail_profile.as_ref().is_some_and(|p| p.len() >= 3) {
            return None;
        }
        self.detail_grip_any(v)
    }

    /// A selected or hovered detail view's circle (live asbv-7/8/14/15):
    /// dashed, light blue #90CEF1 while the view is selected and tan #CCB292
    /// on hover, with the resize arrows beside the grip, the grip an orange
    /// dot and the centre a blue one.
    pub fn detail_circle_overlay(&self, o: &mut crate::drawing_scene::Overlay) {
        const BLUE: [u8; 4] = [144, 206, 241, 255];
        const TAN: [u8; 4] = [204, 178, 146, 255];
        const ORANGE: [u8; 4] = [204, 146, 92, 255];
        const CENTRE: [u8; 4] = [0, 136, 204, 255];
        let hovered = self.hover.and_then(|h| self.sel_of_pick(h));
        for v in self.current_sheet().views.iter().filter(|v| matches!(v.kind, noshape_model::drawing::ViewKind::Detail { .. })) {
            let selected = self.selection.contains(&Sel::View(v.id));
            if !selected && hovered != Some(Sel::View(v.id)) {
                continue;
            }
            let Some((c, r, g)) = self.detail_grip(v.id) else { continue };
            let color = if selected { BLUE } else { TAN };
            let at = |a: f64| [c[0] + r * a.cos(), c[1] + r * a.sin()];
            let n = 64;
            // Live draws the circle itself dashed: the gaps hide the solid one.
            for i in 0..n {
                let (a0, a1) = (i as f64 / n as f64 * std::f64::consts::TAU, (i + 1) as f64 / n as f64 * std::f64::consts::TAU);
                let ink = if i % 2 == 0 { (color, 1.5) } else { ([255, 255, 255, 255], 2.5) };
                o.tinted.push((vec![at(a0), at(a1)], ink.0, ink.1));
            }
            // The resize arrows: one each side of the grip, along the circle.
            let len = self.tol_mm(9.0);
            for s in [1.0f64, -1.0] {
                let tip = at(s * 0.35);
                let back = at(s * (0.35 + len / r.max(1e-9)));
                let d = [tip[0] - back[0], tip[1] - back[1]];
                let w = [-d[1] * 0.35, d[0] * 0.35];
                o.tinted.push((vec![tip, [back[0] + w[0], back[1] + w[1]], [back[0] - w[0], back[1] - w[1]], tip], color, 1.5));
            }
            o.dots.push((g, ORANGE));
            o.dots.push((c, CENTRE));
        }
    }

    /// Right-click on a detail circle (live asb 10-23 views (2)): Paste,
    /// Paste as table, Add leader (the label onto a leader with an arrow to
    /// the circle), Edit detail..., Clear selection, Zoom to fit, Delete.
    pub fn detail_circle_entries(&self, v: ViewId, cx: &App) -> Vec<MenuEntry> {
        let mut out: Vec<MenuEntry> = crate::drawing_props::paste_entry(cx).into_iter().collect();
        out.push(MenuEntry::item("Paste as table", |t, _, cx| {
            t.start_paste_table(cx);
        }));
        let leader = self.current_sheet().view(v).is_some_and(|x| x.detail_style == 2 || x.detail_label_at.is_some());
        if !leader && let Some((_, r, _)) = self.detail_grip_any(v) {
            let d = r * std::f64::consts::FRAC_1_SQRT_2;
            out.push(MenuEntry::item("Add leader", move |t, _, cx| t.edit_view("Add leader", v, cx, |x| x.detail_label_at = Some([d + 12.0, d + 10.0]))));
        }
        out.push(MenuEntry::item("Edit detail...", move |t, w, cx| t.open_detail_edit(v, w, cx)));
        out.push(MenuEntry::Separator);
        out.push(MenuEntry::item("Clear selection", |t, _, cx| {
            t.selection.clear();
            t.redraw(cx);
        }));
        out.push(MenuEntry::item("Zoom to fit", |t, _, cx| t.zoom_to_fit(cx)));
        out.push(MenuEntry::item("Delete", move |t, _, cx| {
            t.selection = vec![Sel::View(v)];
            t.delete_selection(cx);
        }));
        out
    }

    /// A detail view's circle (centre, radius, grip) whatever its profile.
    pub fn detail_grip_any(&self, v: ViewId) -> Option<([f64; 2], f64, [f64; 2])> {
        let view = self.current_sheet().view(v)?;
        let noshape_model::drawing::ViewKind::Detail { parent, center, radius, .. } = &view.kind else { return None };
        let ppl = self.placement(*parent)?;
        let (c, r) = (ppl.to_sheet(*center), ppl.len_to_sheet(*radius));
        Some((c, r, [c[0] + r, c[1]]))
    }

    /// The profile in progress.
    pub fn detail_overlay(&self, overlay: &mut crate::drawing_scene::Overlay) {
        let Some(d) = &self.detail_dlg else { return };
        let (Some(view), true) = (d.view, d.profile.is_none() && !d.pts.is_empty()) else { return };
        let Some(pl) = self.placement(view) else { return };
        let mut pts: Vec<[f64; 2]> = d.pts.iter().map(|p| pl.to_sheet(*p)).collect();
        if d.shape == 1 {
            let (a, b) = (pts[0], self.cursor);
            overlay.rect(a, b, true);
            return;
        }
        pts.push(self.cursor);
        if d.shape == 2 && pts.len() >= 3 {
            overlay.lines.push((views::closed_spline(&pts, 12), true));
        } else {
            overlay.lines.push((pts.clone(), true));
        }
        overlay.points.extend(pts);
    }

    pub fn render_detail_dlg(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let d = self.detail_dlg.as_ref()?;
        let pal = p();
        let (shape, style, show, clip, scale, editing) = (d.shape, d.style, d.show_profile, d.clip_border, d.scale, d.edit.is_some());
        let ed = cx.entity();
        // Parent scale for the default (twice the parent's).
        let parent_scale = match &self.tool {
            Tool::Detail { parent: Some(p), .. } => views::view_scale(self.current_sheet(), *p),
            _ => self.current_sheet().scale,
        };
        let fmt = |s: f64| noshape_model::drawing::sheet::format_scale(s);
        // Live 127 / 132 / 604: the scale is blank until the profile exists
        // (a circle, or a rectangle / spline / polygon profile).
        let placed = matches!(self.tool, Tool::Detail { radius: Some(_), .. }) || d.profile.is_some() || (editing && !matches!(self.tool, Tool::Detail { .. }));
        let current = scale.or(placed.then_some(parent_scale * 2.0));
        let shown = current.map(fmt).unwrap_or_default();
        let ed2 = ed.clone();
        let scale_btn = Button::new("det-scale").label(shown).ghost().small().w(px(106.)).h(px(25.)).rounded(px(0.)).dropdown_caret(true).os_dropdown_menu("det-scale-dd", true, move |mut m, _, _| {
            for s in SCALES {
                let e = ed2.clone();
                m = m.item(crate::os_menu::OsMenuItem::new(fmt(s)).checked(current.is_some_and(|c| (s - c).abs() < 1e-9)).on_click(move |_, _, cx| {
                    e.update(cx, |t, cx| {
                        if let Some(d) = &mut t.detail_dlg {
                            d.scale = Some(s);
                        }
                        t.update_preview(cx);
                        cx.notify();
                    })
                }));
            }
            m
        });
        let ed3 = ed.clone();
        let style_btn = Button::new("det-style").label(STYLES[style as usize]).ghost().small().w(px(117.)).h(px(25.)).rounded(px(0.)).dropdown_caret(true).os_dropdown_menu("det-style-dd", true, move |mut m, _, _| {
            for (i, s) in STYLES.iter().enumerate() {
                let e = ed3.clone();
                m = m.item(crate::os_menu::OsMenuItem::new(*s).checked(i as u8 == style).on_click(move |_, _, cx| {
                    e.update(cx, |t, cx| {
                        if let Some(d) = &mut t.detail_dlg {
                            d.style = i as u8;
                        }
                        t.update_preview(cx);
                        cx.notify();
                    })
                }));
            }
            m
        });
        let mut tools = h_flex().pl(px(4.)).pt(px(3.)).pb(px(3.)).gap(px(2.));
        for (i, (tip, icon)) in SHAPES.iter().enumerate() {
            // Live dom drawing-detail-dialog: 30 px buttons, radius 2; the active
            // one `#cfdbe5` with a 1 px `#9aa5af` border.
            // Live 604: once the profile exists no shape stays pressed.
            let on = shape == i as u8 && !placed;
            let btn = div()
                .id(SharedString::from(format!("det-shape-{i}")))
                .size(px(30.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(2.))
                .border_1()
                .cursor_pointer()
                .when(on, |d| d.bg(gpui_kit::rgb(0xcfdbe5)).border_color(gpui_kit::rgb(0x9aa5af)))
                .when(!on, |d| d.border_color(gpui_kit::transparent_black()).hover(|d| d.bg(gpui_kit::rgb(0xe6ebef))))
                .child(crate::assets::icon(icon, 20., cx))
                .tooltip(crate::ui::tooltip(*tip));
            tools = tools.child(btn.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                if let Some(d) = &mut this.detail_dlg {
                    d.shape = i as u8;
                    d.pts.clear();
                    d.profile = None;
                    d.view = None;
                }
                this.tool = Tool::Detail { parent: None, center: None, radius: None };
                this.update_preview(cx);
                cx.notify();
            })));
        }
        // Live dom drawing-detail-dialog: 26 px rows, the 12 px label 5 px in,
        // white fields with a `#ddd` underline at the right edge.
        let row = |label: &'static str, body: AnyElement| h_flex().h(px(26.)).pl(px(5.)).child(div().flex_1().text_size(px(12.)).child(label)).child(body).into_any_element();
        let field = |w: f32| div().w(px(w)).h(px(26.)).flex_none().bg(crate::theme::light().panel).border_b_1().border_color(gpui_kit::rgb(0xdddddd));
        let check = |id: &'static str, label: &'static str, on: bool, f: fn(&mut DetailDlg, bool), cx: &mut Context<Self>| {
            div().h(px(26.)).flex().items_center().pl(px(7.)).child(crate::drawing_chrome::DrCheckbox::new(id).label(label).checked(on).on_click(cx.listener(move |this, v: &bool, _, cx| {
                if let Some(d) = &mut this.detail_dlg {
                    f(d, *v);
                }
                this.update_preview(cx);
                cx.notify();
            })))
        };
        Some(
            v_flex()
                .id("dr-detail-dlg")
                .absolute()
                .top(px(DIALOG_TOP))
                .left(px(self.dialog_left()))
                .w(px(200.))
                .pb(px(3.))
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
                        .child(div().flex_1().text_size(px(13.)).font_weight(FontWeight::MEDIUM).child("Detail view"))
                        // ✓ (live asbv-5: green rgb(0,149,0)): applies Edit detail;
                        // pale while creating (the click on the sheet places).
                        .child(div().id("det-ok").w(px(28.)).h(px(27.)).flex().items_center().justify_center().cursor_pointer().bg(gpui_kit::rgb(0x009500)).when(!editing, |d| d.bg(gpui_kit::rgb(crate::drawing_chrome::OK_DISABLED))).child(crate::ui::glyph("check", 18., gpui_kit::white())).when(editing, |d| d.on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.close_detail_edit(true, cx)))))
                        .child(
                            div()
                                .id("det-cancel")
                                .w(px(28.))
                                .h(px(27.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .child(crate::drawing_chrome::cancel_x(cx))
                                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| if editing { this.close_detail_edit(false, cx) } else { this.cancel(cx) })),
                        ),
                )
                .child(tools)
                // Live 127, 600–604 (behaviours.md "Detail view dialog: prompt
                // field text per shape"): the query field reads Create circle /
                // rectangle / closed profile / closed polygon until the profile
                // exists, then "Profile created" (`#dbf1fa`, 1 px `#1b5faa`,
                // radius 2, 12 px `#616b73`).
                // Live drawing/127: 5 px under the field to the View scale row.
                .child(div().mx(px(3.)).mt(px(1.)).mb(px(5.)).h(px(26.)).px(px(7.)).flex().items_center().bg(gpui_kit::rgb(0xdbf1fa)).border_1().border_color(gpui_kit::rgb(0x1b5faa)).rounded(px(2.)).text_size(px(12.)).text_color(gpui_kit::rgb(0x616b73)).child(if editing && !matches!(self.tool, Tool::Detail { .. }) {
                    // Edit detail (live asbv-5): the profile as a "Profile" chip.
                    "Profile"
                } else if placed {
                    "Profile created"
                } else {
                    match shape {
                        1 => "Create rectangle",
                        2 => "Create closed profile",
                        3 => "Create closed polygon",
                        _ => "Create circle",
                    }
                }))
                .child(div().px(px(3.)).child(row("View scale", field(106.).child(scale_btn).into_any_element())))
                .child(div().px(px(3.)).child(row("View label", gpui_kit::base::input::input_frame(field(110.).child(Input::new(&d.label).text_size(px(13.33)).text_color(gpui_kit::rgb(0x555555)).appearance(false).small()).into_any_element()).into_any_element())))
                .child(div().px(px(3.)).child(row("Profile style", field(117.).child(style_btn).into_any_element())))
                // Live drawing/127: the checkbox rows start 3 px into the last field row.
                .child(div().mt(px(-3.)).child(check("det-show", "Show detail view profile", show, |d, v| d.show_profile = v, cx)))
                .child(check("det-clip", "Clip border to geometry", clip, |d, v| d.clip_border = v, cx))
                .when(shape >= 2, |e| e.child(div().px(px(6.)).py(px(4.)).text_size(px(12.)).text_color(pal.text_muted).child("Click points; double-click to close the profile")))
                .into_any_element(),
        )
    }
}
