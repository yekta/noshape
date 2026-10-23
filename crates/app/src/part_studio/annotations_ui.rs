//! Inspection panel › Datum (live psb 10-20 r, psb/r-910–922): a small
//! "Datum ✓ ✕" dialog with the next free letter; clicking a planar face
//! places the symbol at once as a preview, ✓ keeps it. The symbol is a boxed
//! letter over a filled triangle lying in the face's plane, upright for the
//! view it was placed in, at a constant on-screen size. Annotations aren't
//! features: they appear only in Model definitions. Right-click on one:
//! Show all | Select | Zoom to fit | Delete annotation.

use super::PartStudioView;
use crate::theme::p;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use glam::{DVec2, DVec3};
use noshape_model::annotations::{self as ann, Annotation, AnnotationKind};
use noshape_model::reference::Reference;

/// A datum box being dragged: from where, its offset then and now (pixels).
#[derive(Clone)]
pub struct AnnotationDrag {
    id: u32,
    from: DVec2,
    start: [f64; 2],
    offset: [f64; 2],
}

pub struct DatumEdit {
    letter: Entity<InputState>,
    /// The face picked so far (shown as a preview).
    pub placed: Option<Annotation>,
}

/// A datum symbol in world space.
struct DatumShape {
    /// The letter box's corners.
    boxp: [DVec3; 4],
    triangle: [DVec3; 3],
    /// Where the letter is laid out from, and the symbol's axes.
    origin: DVec3,
    right: DVec3,
    up: DVec3,
    /// World length of a screen pixel there.
    pixel: f64,
}

fn v3(a: [f64; 3]) -> DVec3 {
    DVec3::from_array(a)
}

impl PartStudioView {
    /// Inspection › Datum: the dialog with the next free letter.
    pub(super) fn open_datum_tool(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let next = self.studio(cx).map(|s| ann::next_datum_letter(&s.annotations)).unwrap_or_else(|| "A".into());
        let letter = cx.new(|cx| InputState::new(window, cx).default_value(next));
        self.ui.datum_edit = Some(DatumEdit { letter, placed: None });
        self.ui.inspection_tool = Some("insp-datum");
        cx.notify();
    }

    pub(super) fn close_datum_tool(&mut self, cx: &mut Context<Self>) {
        self.ui.datum_edit = None;
        if self.ui.inspection_tool == Some("insp-datum") {
            self.ui.inspection_tool = None;
        }
        self.redraw_annotations(cx);
    }

    /// The symbols are drawn by the cached underlay slice: re-render it.
    pub(super) fn redraw_annotations(&mut self, cx: &mut Context<Self>) {
        if let Some(s) = &self.slices {
            s.underlay.update(cx, |_, cx| cx.notify());
        }
        cx.notify();
    }

    /// World length of one screen pixel at `at`, and the world directions
    /// of screen right and up there.
    fn pixel_frame(&self, at: DVec3, cx: &App) -> Option<(f64, DVec3, DVec3)> {
        let vp = self.viewport.read(cx);
        let p = vp.project(at)?;
        let (o0, d0) = vp.ray(p);
        let (o1, d1) = vp.ray(p + DVec2::new(1.0, 0.0));
        let (o2, d2) = vp.ray(p + DVec2::new(0.0, -1.0));
        let (d0, d1, d2) = (d0.normalize(), d1.normalize(), d2.normalize());
        let l = (at - o0).dot(d0);
        let q0 = o0 + d0 * l;
        let right = (o1 + d1 * l) - q0;
        let up = (o2 + d2 * l) - q0;
        Some((right.length().max(1e-12), right.normalize_or_zero(), up.normalize_or_zero()))
    }

    /// A click while the Datum dialog is open: the first planar face under
    /// the pointer (through the default planes) places the symbol there.
    pub(super) fn datum_pick(&mut self, position: DVec2, cx: &mut Context<Self>) {
        let (o, dir) = self.viewport.read(cx).ray(position);
        let dir = dir.normalize();
        let hit = noshape_model::interaction::entities_along_ray(&self.display, o, dir, 0.0, &|_| false, 8).into_iter().find(|h| matches!(h.reference, Reference::Face(_)));
        let Some(hit) = hit else { return };
        let r = hit.reference.clone();
        let Some(frame) = self.frame_of_ref(&r) else { return };
        let at = o + dir * hit.depth;
        let mut normal = frame.normal;
        if normal.dot(dir) > 0.0 {
            normal = -normal;
        }
        let Some((_, _, screen_up)) = self.pixel_frame(at, cx) else { return };
        let up = (screen_up - normal * screen_up.dot(normal)).try_normalize().unwrap_or(frame.y);
        let Some(e) = &mut self.ui.datum_edit else { return };
        let letter = e.letter.read(cx).value().trim().to_string();
        e.placed = Some(Annotation { id: 0, kind: AnnotationKind::Datum { letter }, face: r, at: at.to_array(), normal: normal.to_array(), up: up.to_array(), offset: None });
        self.redraw_annotations(cx);
    }

    /// ✓: keep the placed datum.
    fn commit_datum(&mut self, cx: &mut Context<Self>) {
        let Some(e) = &self.ui.datum_edit else { return };
        let Some(mut a) = e.placed.clone() else { return };
        let letter = e.letter.read(cx).value().trim().to_string();
        if letter.is_empty() {
            return;
        }
        a.kind = AnnotationKind::Datum { letter };
        self.edit_config("Datum", cx, |ps| {
            a.id = ps.annotations.iter().map(|x| x.id).max().unwrap_or(0) + 1;
            ps.annotations.push(a);
        });
        self.close_datum_tool(cx);
    }

    pub(super) fn delete_annotation(&mut self, id: u32, cx: &mut Context<Self>) {
        self.edit_config("Delete annotation", cx, |ps| ps.annotations.retain(|a| a.id != id));
        if self.ui.annotation_selected == Some(id) {
            self.ui.annotation_selected = None;
        }
        cx.notify();
    }

    /// World length per screen pixel along `dir` at `at`, as projected (so
    /// an oblique face's symbol keeps its on-screen size, live psb/r-914).
    fn projected_pixel(&self, at: DVec3, dir: DVec3, flat: f64, cx: &App) -> f64 {
        let vp = self.viewport.read(cx);
        let (Some(a), Some(b)) = (vp.project(at), vp.project(at + dir * (flat * 10.0))) else { return flat };
        let px = (b - a).length() / 10.0;
        // Nearly edge-on: no more than 4× the facing size.
        if px > 0.25 { flat / px } else { flat * 4.0 }
    }

    /// The symbol in world space.
    fn datum_geometry(&self, a: &Annotation, cx: &App) -> Option<DatumShape> {
        let at = v3(a.at);
        let (flat, _, _) = self.pixel_frame(at, cx)?;
        let n = v3(a.normal).normalize_or_zero();
        let up = v3(a.up).normalize_or_zero();
        let right = up.cross(n).normalize_or_zero();
        let (sr, su) = (self.projected_pixel(at, right, flat, cx), self.projected_pixel(at, up, flat, cx));
        // Lift off the face a little so it isn't hidden by it.
        let base = at + n * (0.5 * flat);
        let off = a.offset.map(|o| right * (o[0] * sr) + up * (o[1] * su)).unwrap_or(DVec3::ZERO);
        let pt = |x: f64, y: f64| base + right * (x * sr) + up * (y * su);
        let [[x0, y0], [x1, y1]] = ann::DATUM_BOX;
        let boxp = [pt(x0, y0) + off, pt(x1, y0) + off, pt(x1, y1) + off, pt(x0, y1) + off];
        let triangle = ann::DATUM_TRIANGLE.map(|[x, y]| pt(x, y));
        // `right` / `up` carry the per-axis pixel sizes (pixel = 1 along up).
        Some(DatumShape { boxp, triangle, origin: base + off, right: right * (sr / su), up, pixel: su })
    }

    /// Hit areas of the datum boxes for the viewport (a press there drags
    /// the box, live psb/r-921): each box's vertical middle line.
    pub(super) fn annotation_handles(&self, cx: &App) -> Vec<(DVec3, DVec3, f64)> {
        let Some(studio) = self.studio(cx) else { return Vec::new() };
        studio
            .annotations
            .iter()
            .filter_map(|a| {
                let sh = self.datum_geometry(&self.dragged(a), cx)?;
                Some(((sh.boxp[0] + sh.boxp[1]) * 0.5, (sh.boxp[2] + sh.boxp[3]) * 0.5, 0.0))
            })
            .collect()
    }

    /// `a` with the box offset of a drag in progress.
    fn dragged(&self, a: &Annotation) -> Annotation {
        let mut a = a.clone();
        if let Some(d) = self.ui.annotation_drag.as_ref().filter(|d| d.id == a.id) {
            a.offset = Some(d.offset);
        }
        a
    }

    /// Press on a datum box: start moving it.
    pub(super) fn annotation_drag_start(&mut self, position: DVec2, cx: &mut Context<Self>) -> bool {
        let Some(id) = self.annotation_at(position, cx) else { return false };
        let start = self.studio(cx).and_then(|s| s.annotations.iter().find(|a| a.id == id)).and_then(|a| a.offset).unwrap_or([0.0, 0.0]);
        self.ui.annotation_drag = Some(AnnotationDrag { id, from: position, start, offset: start });
        true
    }

    /// The pointer moved during a box drag: the box follows it in its own
    /// plane (screen delta split along the symbol's projected axes).
    pub(super) fn annotation_drag_move(&mut self, position: DVec2, cx: &mut Context<Self>) -> bool {
        let Some(d) = self.ui.annotation_drag.clone() else { return false };
        let Some(a) = self.studio(cx).and_then(|s| s.annotations.iter().find(|a| a.id == d.id).cloned()) else { return false };
        let mut probe = a.clone();
        probe.offset = None;
        let (Some(sh), vp) = (self.datum_geometry(&probe, cx), self.viewport.read(cx)) else { return false };
        let ru = sh.right.normalize_or_zero();
        let sr = sh.pixel * sh.right.length();
        let (Some(o), Some(r), Some(u)) = (vp.project(sh.origin), vp.project(sh.origin + ru * sr), vp.project(sh.origin + sh.up * sh.pixel)) else { return false };
        let (vr, vu) = (r - o, u - o);
        let delta = position - d.from;
        let det = vr.perp_dot(vu);
        if det.abs() < 1e-9 {
            return true;
        }
        let a_px = delta.perp_dot(vu) / det;
        let b_px = vr.perp_dot(delta) / det;
        if let Some(dd) = &mut self.ui.annotation_drag {
            dd.offset = [d.start[0] + a_px, d.start[1] + b_px];
        }
        self.redraw_annotations(cx);
        true
    }

    /// Release: keep the new place (one undo step).
    pub(super) fn annotation_drag_end(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(d) = self.ui.annotation_drag.take() else { return false };
        if d.offset != d.start {
            self.edit_config("Move annotation", cx, |ps| {
                if let Some(a) = ps.annotations.iter_mut().find(|a| a.id == d.id) {
                    a.offset = Some(d.offset);
                }
            });
        }
        self.redraw_annotations(cx);
        true
    }

    /// Every datum (and the one being placed) as filled world contours:
    /// the box outline as thin bands, the triangle, the letter.
    pub(super) fn annotation_contours(&self, cx: &App) -> Vec<(Vec<Vec<DVec3>>, bool)> {
        let Some(studio) = self.studio(cx) else { return Vec::new() };
        let mut all: Vec<Annotation> = studio.annotations.iter().map(|a| self.dragged(a)).collect();
        if let Some(a) = self.ui.datum_edit.as_ref().and_then(|e| e.placed.as_ref()) {
            all.push(a.clone());
        }
        let mut out = Vec::new();
        for a in &all {
            let Some(DatumShape { boxp, triangle: tri, origin, right, up, pixel: s }) = self.datum_geometry(a, cx) else { continue };
            let mut contours: Vec<Vec<DVec3>> = Vec::new();
            // One screen pixel along an in-plane direction (the axes are
            // scaled separately), and a 1 px band across a line.
            let (ru, sr) = (right.normalize_or_zero(), s * right.length());
            let px_along = |d: DVec3| d.dot(ru).abs() * sr + d.dot(up).abs() * s;
            let band = |along: DVec3| {
                let across = along.cross(ru.cross(up)).normalize_or_zero();
                across * (0.5 * px_along(across))
            };
            // 1 px box edges.
            for (i, &p0) in boxp.iter().enumerate() {
                let p1 = boxp[(i + 1) % 4];
                let along = (p1 - p0).normalize_or_zero();
                let side = band(along);
                let ext = along * (0.5 * px_along(along));
                contours.push(vec![p0 - side - ext, p1 - side + ext, p1 + side + ext, p0 + side - ext]);
            }
            contours.push(tri.to_vec());
            // A dragged box: a dashed leader back to the triangle.
            if a.offset.is_some() {
                let from = tri[2];
                let to = (boxp[0] + boxp[1]) * 0.5;
                let len = (to - from).length();
                let along = (to - from).normalize_or_zero();
                let side = band(along);
                let mut t = 0.0;
                while t < len {
                    let (a0, a1) = (from + along * t, from + along * (t + 4.0 * s).min(len));
                    contours.push(vec![a0 - side, a1 - side, a1 + side, a0 + side]);
                    t += 7.0 * s;
                }
            }
            let AnnotationKind::Datum { letter } = &a.kind;
            let (cap, baseline) = ann::DATUM_LETTER;
            let asc = cap * s;
            let width = 0.62 * asc * letter.chars().count() as f64;
            let top_left = origin + right * (-0.5 * width - 0.25 * asc) + up * (baseline * s + 1.3 * asc);
            super::in_plane_contours(letter, top_left, right, up, asc, &mut contours);
            out.push((contours, self.ui.annotation_selected == Some(a.id)));
        }
        out
    }

    /// The annotation whose box is under `position` (viewport pixels).
    pub(super) fn annotation_at(&self, position: DVec2, cx: &App) -> Option<u32> {
        let studio = self.studio(cx)?;
        let vp = self.viewport.read(cx);
        studio.annotations.iter().rev().find_map(|a| {
            let shape = self.datum_geometry(a, cx)?;
            let q: Vec<DVec2> = shape.boxp.iter().filter_map(|p| vp.project(*p)).collect();
            (q.len() == 4 && point_in_quad(position, &q)).then_some(a.id)
        })
    }

    /// Right-click on an annotation (live psb/r-918): Show all, Select,
    /// Select other… | Zoom to fit | Delete annotation.
    pub(super) fn open_annotation_menu(&mut self, id: u32, at: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        let view = cx.entity();
        let menu = crate::os_menu::OsMenu::build(window, cx, move |m, _, _| {
            let (v1, v2, v3, v4, v5) = (view.clone(), view.clone(), view.clone(), view.clone(), view.clone());
            m.item(crate::os_menu::OsMenuItem::new("Show all").on_click(move |_, _, cx| v1.update(cx, |this, cx| this.show_everything(cx))))
                .item(crate::os_menu::OsMenuItem::new("Select").on_click(move |_, _, cx| {
                    v2.update(cx, |this, cx| {
                        this.ui.annotation_selected = Some(id);
                        this.redraw_annotations(cx);
                    })
                }))
                // Live 918 also has "Add comment" (cloud, not built).
                .item(crate::os_menu::OsMenuItem::new("Select other…").on_click(move |_, _, cx| v5.update(cx, |this, cx| this.select_other(false, cx))))
                .separator()
                .item(crate::os_menu::OsMenuItem::new("Zoom to fit").on_click(move |_, _, cx| v3.update(cx, |this, cx| this.zoom_fit(cx))))
                .separator()
                .item(crate::os_menu::OsMenuItem::new("Delete annotation").on_click(move |_, _, cx| v4.update(cx, |this, cx| this.delete_annotation(id, cx))))
        });
        crate::os_menu::open(at, menu, window, cx);
    }

    /// The Datum dialog (live psb/r-910): "Datum ✓ ✕" and the letter, at
    /// the feature dialog's place.
    pub(super) fn render_datum_dialog(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let e = self.ui.datum_edit.as_ref()?;
        let pal = p();
        let ok = e.placed.is_some() && !e.letter.read(cx).value().trim().is_empty();
        Some(
            v_flex()
                .id("datum-dialog")
                .occlude()
                .absolute()
                .top(px(3.))
                .left(px(0.))
                .w(px(120.))
                .bg(gpui_kit::rgb(0xffffff))
                .shadow_md()
                .text_color(pal.text)
                .on_key_down(cx.listener(|this, e: &KeyDownEvent, _, cx| match e.keystroke.key.as_str() {
                    "enter" => {
                        cx.stop_propagation();
                        this.commit_datum(cx);
                    }
                    "escape" => {
                        cx.stop_propagation();
                        this.close_datum_tool(cx);
                    }
                    _ => {}
                }))
                .child(
                    h_flex()
                        .h(px(29.))
                        .border_b_1()
                        .border_color(gpui_kit::rgb(0xd7d7d7))
                        .child(div().flex_1().pl(px(6.)).text_size(px(13.)).font_weight(FontWeight::MEDIUM).child("Datum"))
                        .child(div().id("datum-ok").size(px(28.)).flex().items_center().justify_center().bg(pal.accept).when(!ok, crate::ui::disabled_accept).cursor_pointer().child(crate::ui::glyph("check", 16., gpui_kit::rgb_static(0xffffff))).on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.commit_datum(cx))))
                        .child(div().id("datum-cancel").size(px(28.)).flex().items_center().justify_center().cursor_pointer().child(crate::ui::glyph("close", 14., gpui_kit::rgb(0xd52b1e))).on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.close_datum_tool(cx)))),
                )
                .child(div().p(px(3.)).child(gpui_kit::base::input::input_frame(div().border_b_1().border_color(gpui_kit::rgb(0xd7d7d7)).child(Input::new(&e.letter).small().appearance(false).text_size(px(13.33))))))
                .into_any_element(),
        )
    }
}

/// Is `p` inside the convex quad `q`?
fn point_in_quad(p: DVec2, q: &[DVec2]) -> bool {
    let sign = |a: DVec2, b: DVec2| (b - a).perp_dot(p - a);
    let s: Vec<f64> = (0..q.len()).map(|i| sign(q[i], q[(i + 1) % q.len()])).collect();
    s.iter().all(|v| *v >= 0.0) || s.iter().all(|v| *v <= 0.0)
}
