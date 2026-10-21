//! Drawing editor: Onshape's Crop view dialog (help Drawing › Crop view,
//! images drawings-tools__crop-view-dialog-01 / -circle- / -spline- /
//! -polygon-): "Crop view ✓ ✕", the sketch shapes Circle | Spline ▾ (Add
//! spline point) | Polygon (Spline pressed at first), and the profile field
//! ("Create closed profile", then "Edit closed profile" with a "Profile ×"
//! chip). The closed profile waits for ✓; ✕ / Esc drop it. A cropped view's
//! menu › Edit crop reopens the dialog on its crop.

use crate::drawing_view::*;
use crate::theme::p;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::drawing::{Crop, ViewId};

/// The dialog's state while a Crop view tool is active.
pub struct CropDlg {
    /// 0 circle, 1 spline, 2 polygon (live order).
    pub shape: u8,
    /// The closed profile, waiting for ✓ (view, crop in view coordinates).
    pub pending: Option<(ViewId, Crop)>,
    /// Edit crop: the view whose crop ✓ replaces.
    pub edit: Option<ViewId>,
    /// Spline ▾ › Add spline point: the next click on the profile inserts one.
    pub add_point: bool,
}

const SHAPES: [(&str, &str); 3] = [("Circle", "circle"), ("Spline", "spline"), ("Polygon", "polyline")];

/// The pending profile's colour and its points' (help crop-view-spline-01:
/// tan, light blue dots).
const PROFILE: [u8; 4] = [0xcc, 0x92, 0x5c, 255];
const POINT: [u8; 4] = [0x90, 0xce, 0xf1, 255];

/// The tool drawing shape `shape` (in view `parent` when set).
fn shape_tool(shape: u8, parent: Option<ViewId>) -> Tool {
    match shape {
        0 => Tool::Crop { parent, first: None, circle: true },
        s => Tool::CropPoly { parent, points: vec![], spline: s == 1 },
    }
}

/// The dialog shape of a crop.
fn shape_of(c: &Crop) -> u8 {
    match c {
        Crop::Circle { .. } => 0,
        Crop::Spline(_) => 1,
        Crop::Polygon(_) => 2,
    }
}

impl DrawingEditor {
    /// Create / drop the dialog with the Crop view tool (called from render).
    pub fn sync_crop_dlg(&mut self) {
        let active = matches!(self.tool, Tool::Crop { .. } | Tool::CropPoly { .. });
        match (self.crop_dlg.is_some(), active) {
            (false, true) => {
                let shape = match self.tool {
                    Tool::Crop { circle: true, .. } => 0,
                    Tool::CropPoly { spline: false, .. } => 2,
                    _ => 1,
                };
                self.crop_dlg = Some(CropDlg { shape, pending: None, edit: None, add_point: false });
            }
            (true, false) => self.crop_dlg = None,
            _ => {}
        }
    }

    /// The Crop view tool (toolbar, S toolbar, shortcuts): Spline first.
    pub fn start_crop(&mut self, cx: &mut Context<Self>) {
        self.crop_dlg = None;
        self.set_tool(shape_tool(1, None), cx);
    }

    /// View menu › Edit crop: the dialog on view `id`'s crop.
    pub fn edit_crop(&mut self, id: ViewId, cx: &mut Context<Self>) {
        let Some(crop) = self.current_sheet().view(id).and_then(|v| v.crop.clone()) else { return };
        let shape = shape_of(&crop);
        self.set_tool(shape_tool(shape, None), cx);
        self.crop_dlg = Some(CropDlg { shape, pending: Some((id, crop)), edit: Some(id), add_point: false });
        self.update_preview(cx);
    }

    /// A closed profile was drawn in view `view`: it waits for ✓, and the
    /// tool is ready for another one (which replaces it).
    pub fn crop_done(&mut self, view: ViewId, crop: Crop, cx: &mut Context<Self>) {
        let shape = shape_of(&crop);
        if let Some(d) = &mut self.crop_dlg {
            if d.edit.is_some_and(|e| e != view) {
                return;
            }
            d.pending = Some((view, crop));
            d.shape = shape;
        }
        self.tool = shape_tool(shape, None);
        self.update_preview(cx);
        cx.notify();
    }

    /// ✓: crop the view with the pending profile.
    fn accept_crop(&mut self, cx: &mut Context<Self>) {
        let Some((view, crop)) = self.crop_dlg.as_mut().and_then(|d| d.pending.take()) else { return };
        let edit = self.crop_dlg.as_ref().is_some_and(|d| d.edit.is_some());
        self.crop_dlg = None;
        self.tool = Tool::Select;
        self.edit_view(if edit { "Edit crop" } else { "Crop view" }, view, cx, |v| v.crop = Some(crop));
        self.update_preview(cx);
    }

    /// Enter in the dialog: ✓ when a profile waits. True when used.
    pub fn crop_enter(&mut self, cx: &mut Context<Self>) -> bool {
        if self.crop_dlg.as_ref().is_some_and(|d| d.pending.is_some()) {
            self.accept_crop(cx);
            return true;
        }
        false
    }

    /// Add spline point: a click near the pending spline inserts a point
    /// into its nearest segment. True when used.
    pub fn crop_add_point_click(&mut self, c: [f64; 2], cx: &mut Context<Self>) -> bool {
        let Some(d) = &self.crop_dlg else { return false };
        let (true, Some((view, Crop::Spline(pts)))) = (d.add_point, d.pending.clone()) else { return false };
        let Some(pl) = self.placement(view) else { return false };
        let p = pl.to_view(c);
        let n = pts.len();
        let best = (0..n).min_by(|&a, &b| {
            let da = noshape_model::drawing::views::dist_to_segment(p, pts[a], pts[(a + 1) % n]);
            let db = noshape_model::drawing::views::dist_to_segment(p, pts[b], pts[(b + 1) % n]);
            da.total_cmp(&db)
        });
        let Some(i) = best else { return false };
        let mut pts = pts;
        pts.insert(i + 1, p);
        if let Some(d) = &mut self.crop_dlg {
            d.pending = Some((view, Crop::Spline(pts)));
            d.add_point = false;
        }
        self.update_preview(cx);
        cx.notify();
        true
    }

    /// The pending profile on the sheet (tan, its points marked).
    pub fn crop_overlay(&self, overlay: &mut crate::drawing_scene::Overlay) {
        let Some((view, crop)) = self.crop_dlg.as_ref().and_then(|d| d.pending.as_ref()) else { return };
        let Some(pl) = self.placement(*view) else { return };
        match crop {
            Crop::Circle { center, radius } => {
                let c = pl.to_sheet(*center);
                let r = radius * pl.scale * 1000.0;
                let ring: Vec<[f64; 2]> = (0..=64).map(|i| i as f64 / 64.0 * std::f64::consts::TAU).map(|t| [c[0] + r * t.cos(), c[1] + r * t.sin()]).collect();
                overlay.tinted.push((ring, PROFILE, 2.0));
                overlay.dots.push((c, POINT));
            }
            Crop::Spline(pts) | Crop::Polygon(pts) => {
                let spline = matches!(crop, Crop::Spline(_));
                let mut closed = pts.clone();
                closed.push(pts[0]);
                overlay.tinted.push((self.profile_on_sheet(*view, &closed, spline), PROFILE, 2.0));
                overlay.dots.extend(pts.iter().map(|p| (pl.to_sheet(*p), POINT)));
            }
        }
    }

    pub fn render_crop_dlg(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let d = self.crop_dlg.as_ref()?;
        let pal = p();
        let (shape, pending) = (d.shape, d.pending.is_some());
        let mut tools = h_flex().pl(px(4.)).pt(px(3.)).pb(px(3.)).gap(px(2.));
        for (i, (tip, icon)) in SHAPES.iter().enumerate() {
            // As the Detail view dialog: 30 px buttons, the pressed one
            // `#cfdbe5` with a 1 px `#9aa5af` border.
            let on = shape == i as u8;
            let btn = div()
                .id(SharedString::from(format!("crop-shape-{i}")))
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
                .tooltip(crate::ui::tooltip(*tip))
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    if let Some(d) = &mut this.crop_dlg {
                        d.shape = i as u8;
                        d.add_point = false;
                    }
                    this.tool = shape_tool(i as u8, None);
                    this.update_preview(cx);
                    cx.notify();
                }));
            tools = tools.child(btn);
            // Spline ▾: Add spline point (with a spline profile to add to).
            if i == 1 {
                let ed = cx.entity();
                let can_add = matches!(d.pending, Some((_, Crop::Spline(_))));
                tools = tools.child(
                    div()
                        .id("crop-spline-more")
                        .w(px(12.))
                        .h(px(30.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .child(crate::assets::icon("drawing-tool-caret", 12., cx))
                        .on_click(move |e: &ClickEvent, window, cx| {
                            let ed = ed.clone();
                            let menu = crate::os_menu::OsMenu::build(window, cx, move |m, _, _| {
                                m.item(crate::os_menu::OsMenuItem::new("Add spline point").disabled(!can_add).on_click(move |_, _, cx| {
                                    ed.update(cx, |t, cx| {
                                        if let Some(d) = &mut t.crop_dlg {
                                            d.add_point = true;
                                        }
                                        cx.notify();
                                    })
                                }))
                            });
                            crate::os_menu::open(e.position(), menu, window, cx);
                        }),
                );
            }
        }
        let field = div().mx(px(3.)).mt(px(1.)).mb(px(3.)).px(px(7.)).bg(gpui_kit::rgb(0xdbf1fa)).border_1().border_color(gpui_kit::rgb(0x1b5faa)).rounded(px(2.)).text_size(px(12.)).text_color(gpui_kit::rgb(0x616b73));
        let field = if pending {
            field.py(px(3.)).child(div().child("Edit closed profile")).child(
                h_flex()
                    .h(px(22.))
                    .child(div().flex_1().text_size(px(14.)).text_color(pal.text).child("Profile"))
                    .child(div().id("crop-profile-clear").px(px(4.)).cursor_pointer().text_color(gpui_kit::rgb(0x999999)).child("×").on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        if let Some(d) = &mut this.crop_dlg {
                            d.pending = None;
                            d.add_point = false;
                        }
                        this.update_preview(cx);
                        cx.notify();
                    }))),
            )
        } else {
            field.h(px(26.)).flex().items_center().child("Create closed profile")
        };
        Some(
            v_flex()
                .id("dr-crop-dlg")
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
                        .child(div().flex_1().text_size(px(13.)).font_weight(FontWeight::MEDIUM).child("Crop view"))
                        .child(
                            div()
                                .id("crop-ok")
                                .w(px(28.))
                                .h(px(27.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .bg(if pending { pal.accept } else { gpui_kit::rgb(crate::drawing_chrome::OK_DISABLED) })
                                .cursor_pointer()
                                .child(crate::ui::glyph("check", 18., gpui_kit::white()))
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.accept_crop(cx))),
                        )
                        .child(div().id("crop-cancel").w(px(28.)).h(px(27.)).flex().items_center().justify_center().cursor_pointer().child(crate::drawing_chrome::cancel_x(cx)).on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.cancel(cx)))),
                )
                .child(tools)
                .child(field)
                .into_any_element(),
        )
    }
}
