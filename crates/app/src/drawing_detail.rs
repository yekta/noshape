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
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::{Input, InputState};
use crate::os_ui::OsDropdownExt as _;
use gpui_kit::component::{Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::drawing::{DrawingView, ViewId, views};

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
                let label = cx.new(|cx| InputState::new(window, cx).default_value(next));
                self.detail_dlg = Some(DetailDlg { shape: 0, pts: Vec::new(), view: None, profile: None, scale: None, label, style: self.drawing.props.view_tab.detail_style, show_profile: self.drawing.props.view_tab.detail_show_profile, clip_border: self.drawing.props.view_tab.detail_clip_border, pick_region: false });
            }
            (true, false) => self.detail_dlg = None,
            _ => {}
        }
        if let Some(d) = &self.detail_dlg {
            self.detail_label = d.label.read(cx).value().trim().to_string();
        }
    }

    /// The dialog's settings on the new detail view.
    pub fn apply_detail_options(&self, v: &mut DrawingView, parent_scale: f64) {
        let Some(d) = &self.detail_dlg else { return };
        v.scale = Some(d.scale.unwrap_or(parent_scale * 2.0));
        v.detail_profile = d.profile.clone();
        v.detail_style = d.style;
        v.detail_hide_profile = !d.show_profile;
        v.detail_clip_border = d.clip_border;
        if !self.detail_label.is_empty()
            && let noshape_model::drawing::ViewKind::Detail { label, .. } = &mut v.kind
        {
            *label = self.detail_label.clone();
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
        let poly: Vec<[f64; 2]> = match d.shape {
            1 if d.pts.len() >= 2 => {
                let (a, b) = (d.pts[0], d.pts[1]);
                vec![a, [b[0], a[1]], b, [a[0], b[1]]]
            }
            2 if d.pts.len() >= 3 => views::closed_spline(&d.pts, 16),
            3 if d.pts.len() >= 3 => d.pts.clone(),
            _ => return false,
        };
        let n = poly.len() as f64;
        let center = [poly.iter().map(|p| p[0]).sum::<f64>() / n, poly.iter().map(|p| p[1]).sum::<f64>() / n];
        let radius = poly.iter().map(|p| ((p[0] - center[0]).powi(2) + (p[1] - center[1]).powi(2)).sqrt()).fold(0.0, f64::max);
        d.profile = Some(poly);
        self.tool = Tool::Detail { parent: Some(view), center: Some(center), radius: Some(radius) };
        self.update_preview(cx);
        cx.notify();
        true
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
        let (shape, style, show, clip, scale) = (d.shape, d.style, d.show_profile, d.clip_border, d.scale);
        let ed = cx.entity();
        // Parent scale for the default (twice the parent's).
        let parent_scale = match &self.tool {
            Tool::Detail { parent: Some(p), .. } => views::view_scale(self.current_sheet(), *p),
            _ => self.current_sheet().scale,
        };
        let shown = scale.unwrap_or(parent_scale * 2.0);
        let fmt = |s: f64| noshape_model::drawing::sheet::format_scale(s);
        let ed2 = ed.clone();
        let scale_btn = Button::new("det-scale").label(fmt(shown)).ghost().small().w(px(110.)).dropdown_caret(true).os_dropdown_menu("det-scale-dd", true, move |mut m, _, _| {
            for s in SCALES {
                let e = ed2.clone();
                m = m.item(crate::os_menu::OsMenuItem::new(fmt(s)).checked((s - shown).abs() < 1e-9).on_click(move |_, _, cx| {
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
        let style_btn = Button::new("det-style").label(STYLES[style as usize]).ghost().small().w(px(110.)).dropdown_caret(true).os_dropdown_menu("det-style-dd", true, move |mut m, _, _| {
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
        let mut tools = h_flex().px(px(4.)).py(px(3.)).gap(px(1.));
        for (i, (tip, icon)) in SHAPES.iter().enumerate() {
            tools = tools.child(crate::ui::tool_button(SharedString::from(format!("det-shape-{i}")), icon, shape == i as u8, cx).tooltip(*tip).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
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
        let row = |label: &'static str, body: AnyElement| h_flex().h(px(30.)).px(px(6.)).child(div().w(px(90.)).text_size(px(12.)).child(label)).child(body).into_any_element();
        let check = |id: &'static str, label: &'static str, on: bool, f: fn(&mut DetailDlg, bool), cx: &mut Context<Self>| {
            div().h(px(26.)).flex().items_center().px(px(4.)).child(Checkbox::new(id).label(label).small().checked(on).on_click(cx.listener(move |this, v: &bool, _, cx| {
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
                .w(px(240.))
                .bg(pal.dialog_bg)
                .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(5.)), blur_radius: px(10.), spread_radius: px(0.), inset: false }])
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
                        .child(
                            div()
                                .id("det-cancel")
                                .w(px(28.))
                                .h(px(27.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .child(crate::ui::glyph("close", 14., pal.cancel))
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.cancel(cx))),
                        ),
                )
                .child(tools)
                .child(row("View scale", scale_btn.into_any_element()))
                .child(row("View label", div().w(px(110.)).child(Input::new(&d.label).small()).into_any_element()))
                .child(row("Profile style", style_btn.into_any_element()))
                .child(check("det-show", "Show detail view profile", show, |d, v| d.show_profile = v, cx))
                .child(check("det-clip", "Clip border to geometry", clip, |d, v| d.clip_border = v, cx))
                .when(shape >= 2, |e| e.child(div().px(px(6.)).py(px(4.)).text_size(px(12.)).text_color(pal.text_muted).child("Click points; double-click to close the profile")))
                .into_any_element(),
        )
    }
}
