//! The translate triad of Transform ▸ Translate by XYZ (and Copy here…):
//! live part-studio/102-copy-here.png. Three screen-sized white arrows along
//! the world axes with hollow heads, and three small planar handles between
//! them, at the centre of the entities' bounding box moved by the current
//! translation. Dragging an arrow sets its X / Y / Z translation, dragging a
//! planar handle the two in its plane.

use super::PartStudioView;
use super::manipulators::Manip;
use crate::viewport::ViewportEvent;
use glam::{DVec2, DVec3};
use gpui_kit::*;
use noshape_model::reference::Reference;
use noshape_render::{Drawable, Instance, LineData, LineSegment};
use noshape_units::{Dim, Quantity};

/// Screen sizes (px) measured on the live capture.
const SHAFT_PX: f64 = 45.0;
const HEAD_PX: f64 = 20.0;
/// Planar handles span this part of the shafts.
const PLANE_FROM: f64 = 0.32;
const PLANE_TO: f64 = 0.58;
const PARAMS: [&str; 3] = ["dx", "dy", "dz"];

#[derive(Clone, Copy, Debug)]
pub(super) struct Triad {
    /// Where the triad is drawn (the entities' centre moved by the translation).
    pub origin: DVec3,
    /// The current (dx, dy, dz) in meters.
    pub offset: DVec3,
}

/// A handle: 0–2 = the X / Y / Z arrows, 3–5 = the YZ / XZ / XY planes.
#[derive(Clone, Debug)]
pub(super) struct TriadDrag {
    pub handle: usize,
    /// The offset and the grabbed point when the drag started.
    pub start: DVec3,
    pub grab: DVec3,
}

fn axis(i: usize) -> DVec3 {
    [DVec3::X, DVec3::Y, DVec3::Z][i]
}

/// The two axes of planar handle `h` (3 = YZ, 4 = XZ, 5 = XY).
fn plane_axes(h: usize) -> (usize, usize) {
    [(1, 2), (0, 2), (0, 1)][h - 3]
}

impl PartStudioView {
    pub(super) fn current_triad(&self, cx: &App) -> Option<Triad> {
        let d = self.edit.as_ref()?.dialog.read(cx);
        let p = &d.feature.params;
        if d.feature.type_id != "transform" || !p.is("transformType", "TRANSLATION_3D") {
            return None;
        }
        let ctx = self.ui.before_context.as_ref().or(self.context.as_ref())?;
        let mut lo = DVec3::splat(f64::MAX);
        let mut hi = DVec3::splat(f64::MIN);
        for r in p.query("entities") {
            let id = match r {
                Reference::Body(b) => Some(b.clone()),
                r => self.body_of(r),
            };
            let Some(b) = id.and_then(|id| ctx.bodies.iter().find(|b| b.id == id)) else { continue };
            if let Ok(bb) = b.shape.bounding_box(false) {
                lo = lo.min(bb.min);
                hi = hi.max(bb.max);
            }
        }
        if lo.x > hi.x {
            return None;
        }
        let units = self.doc.read(cx).session.doc.settings.units;
        let value = |id: &str| p.expr(id).and_then(|x| x.cached.map(|q| q.value).or_else(|| noshape_units::evaluate_as(&x.text, Dim::LENGTH, &units, &self.display.variables).ok().map(|q| q.value))).unwrap_or(0.0);
        let offset = DVec3::new(value("dx"), value("dy"), value("dz"));
        // Live (part-studio/440): at the centre of the part's top face.
        let c = (lo + hi) * 0.5;
        Some(Triad { origin: DVec3::new(c.x, c.y, hi.z) + offset, offset })
    }

    /// World length of a screen pixel at the triad.
    fn triad_wpp(&self, t: &Triad, cx: &App) -> Option<f64> {
        let vp = self.viewport.read(cx);
        let right = vp.camera.right();
        let (p0, p1) = (vp.project(t.origin)?, vp.project(t.origin + right * 1e-3)?);
        Some(1e-3 / (p1 - p0).length().max(1e-9))
    }

    /// The handle under a viewport position.
    pub(super) fn triad_handle_at(&self, t: &Triad, p: DVec2, cx: &App) -> Option<usize> {
        let wpp = self.triad_wpp(t, cx)?;
        let vp = self.viewport.read(cx);
        let s = |w: DVec3| vp.project(w);
        let o = t.origin;
        // Planar handles first: they sit between the shafts.
        for h in 3..6 {
            let (i, j) = plane_axes(h);
            let (a, b) = (axis(i) * SHAFT_PX * wpp, axis(j) * SHAFT_PX * wpp);
            let quad = [o + a * PLANE_FROM + b * PLANE_FROM, o + a * PLANE_TO + b * PLANE_FROM, o + a * PLANE_TO + b * PLANE_TO, o + a * PLANE_FROM + b * PLANE_TO];
            let q: Option<Vec<DVec2>> = quad.iter().map(|w| s(*w)).collect();
            if let Some(q) = q
                && inside(&q, p)
            {
                return Some(h);
            }
        }
        let so = s(o)?;
        (0..3)
            .filter_map(|i| {
                let tip = s(o + axis(i) * (SHAFT_PX + HEAD_PX) * wpp)?;
                let d = tip - so;
                let u = ((p - so).dot(d) / d.length_squared().max(1e-9)).clamp(0.0, 1.0);
                let dist = (so + d * u - p).length();
                (dist < 8.0 && u > 0.15).then_some((i, dist))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i)
    }

    /// Hit segments for the viewport (it starts a tool drag on them).
    pub(super) fn triad_hit_segments(&self, t: &Triad, cx: &App) -> Vec<(DVec3, DVec3, f64)> {
        let Some(wpp) = self.triad_wpp(t, cx) else { return Vec::new() };
        let o = t.origin;
        let mut out: Vec<(DVec3, DVec3, f64)> = (0..3).map(|i| (o + axis(i) * 0.15 * SHAFT_PX * wpp, o + axis(i) * (SHAFT_PX + HEAD_PX) * wpp, 0.0)).collect();
        for h in 3..6 {
            let (i, j) = plane_axes(h);
            let (a, b) = (axis(i) * SHAFT_PX * wpp, axis(j) * SHAFT_PX * wpp);
            out.push((o + (a + b) * PLANE_FROM, o + (a + b) * PLANE_TO, 0.0));
        }
        out
    }

    /// The point a drag of `handle` follows: on the arrow's axis, or in
    /// the planar handle's plane, under the cursor.
    fn triad_point(&self, origin: DVec3, handle: usize, p: DVec2, cx: &App) -> Option<DVec3> {
        let (ro, rd) = self.viewport.read(cx).ray(p);
        if handle < 3 {
            let a = axis(handle);
            let w0 = origin - ro;
            let (b, d, e) = (a.dot(rd), a.dot(w0), rd.dot(w0));
            let denom = 1.0 - b * b;
            (denom.abs() > 1e-9).then(|| origin + a * ((b * e - d) / denom))
        } else {
            let (i, j) = plane_axes(handle);
            let n = axis(i).cross(axis(j));
            let den = rd.dot(n);
            (den.abs() > 1e-9).then(|| ro + rd * ((origin - ro).dot(n) / den))
        }
    }

    pub(super) fn triad_event(&mut self, t: Triad, ev: &ViewportEvent, window: &mut Window, cx: &mut Context<Self>) -> bool {
        match ev {
            ViewportEvent::ToolDown { position, .. } => {
                let Some(handle) = self.triad_handle_at(&t, *position, cx) else { return false };
                let Some(grab) = self.triad_point(t.origin, handle, *position, cx) else { return false };
                self.ui.triad_drag = Some(TriadDrag { handle, start: t.offset, grab });
                true
            }
            ViewportEvent::ToolDrag { position, .. } => {
                let Some(d) = self.ui.triad_drag.clone() else { return false };
                // The plane / axis through where the drag started.
                let origin = d.grab;
                let Some(q) = self.triad_point(origin, d.handle, *position, cx) else { return true };
                let delta = q - d.grab;
                let axes: Vec<usize> = if d.handle < 3 { vec![d.handle] } else { let (i, j) = plane_axes(d.handle); vec![i, j] };
                let units = self.doc.read(cx).session.doc.settings.units;
                if let Some(e) = &self.edit {
                    e.dialog.update(cx, |dlg, cx| {
                        for i in axes {
                            // Live (dom/transform-xyz-after-x-drag.txt): one decimal
                            // of the workspace unit ("13.9 mm").
                            let unit = units.si_factor(Dim::LENGTH);
                            let v = ((d.start[i] + delta[i]) / unit * 10.0).round() / 10.0 * unit;
                            let text = noshape_units::format_quantity_with(Quantity::new(v, Dim::LENGTH), &units, 1);
                            dlg.set_expr_text(PARAMS[i], &text, window, cx);
                        }
                    });
                }
                true
            }
            ViewportEvent::ToolUp { .. } => {
                let Some(d) = self.ui.triad_drag.take() else { return false };
                let field = if d.handle < 3 { PARAMS[d.handle] } else { PARAMS[plane_axes(d.handle).0] };
                if let Some(e) = &self.edit {
                    e.dialog.update(cx, |dlg, cx| dlg.focus_field_selected(field, window, cx));
                }
                // The triad turns white again.
                self.rebuild_scene(cx);
                true
            }
            _ => false,
        }
    }
}

/// Whether `p` is inside the convex quad `q`.
fn inside(q: &[DVec2], p: DVec2) -> bool {
    let n = q.len();
    let mut sign = 0.0;
    for k in 0..n {
        let (a, b) = (q[k], q[(k + 1) % n]);
        let c = (b - a).perp_dot(p - a);
        if c.abs() < 1e-12 {
            continue;
        }
        if sign == 0.0 {
            sign = c.signum();
        } else if c.signum() != sign {
            return false;
        }
    }
    true
}

impl Manip {
    pub(super) fn triad(self) -> Option<Triad> {
        match self {
            Manip::Triad(t) => Some(t),
            _ => None,
        }
    }
}

/// The triad's glyph for a frame's view (built by the renderer, see
/// `manipulators::ManipGeometry`): `hover` the hovered or dragged handle.
pub(super) fn triad_glyph(t: &Triad, hover: Option<usize>, dragging: bool, view: &noshape_render::camera_geometry::View, up: &noshape_render::renderer::Uploader) -> Vec<Instance> {
    let Some(wpp) = view.world_per_px(t.origin) else { return Vec::new() };
    let f = |v: DVec3| -> [f32; 3] { [v.x as f32, v.y as f32, v.z as f32] };
    let view_dir = view.direction();
    let o = t.origin;
    let mut back = LineData::default();
    let mut front = LineData::default();
    // Like the extrude arrow: white strokes over a light gray outline
    // (blue on the hovered handle).
    // Live (part-studio/444): a hovered handle turns blue, outline
    // rgb(63,111,172) and inner rgb(151,177,217).
    let gray = crate::theme::rgb8(0xa8adb2);
    let blue = crate::theme::rgb8(0x3f6fac);
    let blue_inner = crate::theme::rgb8(0x97b1d9);
    let white = crate::theme::rgb8(0xffffff);
    // While dragging, the other handles go translucent (part-studio/445).
    let fade = |c: [u8; 4], on: bool| if dragging && !on { [c[0], c[1], c[2], 70] } else { c };
    let mut stroke = |pts: &[DVec3], on: bool| {
        let p: Vec<[f32; 3]> = pts.iter().map(|w| f(*w)).collect();
        back.push_polyline(&p, fade(if on { blue } else { gray }, on), 0, 3.0, LineSegment::ON_TOP);
        front.push_polyline(&p, fade(if on { blue_inner } else { white }, on), 0, 1.5, LineSegment::ON_TOP);
    };
    for i in 0..3 {
        let dir = axis(i);
        let base = o + dir * SHAFT_PX * wpp;
        let apex = base + dir * HEAD_PX * wpp;
        let side = dir.cross(view_dir).try_normalize().unwrap_or_else(|| dir.any_orthonormal_vector());
        let (l, r) = (base + side * HEAD_PX * 0.4 * wpp, base - side * HEAD_PX * 0.4 * wpp);
        let on = hover == Some(i);
        stroke(&[o, base], on);
        stroke(&[l, apex, r, l], on);
    }
    for h in 3..6 {
        let (i, j) = plane_axes(h);
        let (a, b) = (axis(i) * SHAFT_PX * wpp, axis(j) * SHAFT_PX * wpp);
        let quad = [o + a * PLANE_FROM + b * PLANE_FROM, o + a * PLANE_TO + b * PLANE_FROM, o + a * PLANE_TO + b * PLANE_TO, o + a * PLANE_FROM + b * PLANE_TO];
        stroke(&[quad[0], quad[1], quad[2], quad[3], quad[0]], hover == Some(h));
    }
    vec![
        Instance::new(Drawable::Lines(up.upload_lines(&back))).per_primitive_color().unpickable().ignore_clip().layer(40),
        Instance::new(Drawable::Lines(up.upload_lines(&front))).per_primitive_color().unpickable().ignore_clip().layer(41),
    ]
}
