//! In-viewport manipulators for feature dialogs (Onshape's drag handles):
//! the Extrude depth arrow (dragging past zero flips direction) and the
//! Revolve angle arc.

use super::PartStudioView;
use crate::viewport::ViewportEvent;
use glam::{DVec2, DVec3};
use noshape_model::params::ParamValue;
use noshape_model::reference::Reference;
use noshape_render::{Drawable, Instance, LineData, LineSegment, PointData, PointSprite};
use noshape_units::{Dim, Quantity};
use gpui_kit::*;

/// A linear arrow manipulator bound to a length parameter.
#[derive(Clone, Copy, Debug)]
pub(super) struct Arrow {
    pub base: DVec3,
    pub dir: DVec3,
    pub length: f64,
}

/// An angular manipulator: an arc around an axis bound to an angle parameter.
#[derive(Clone, Copy, Debug)]
pub(super) struct ArcHandle {
    pub origin: DVec3,
    /// Unit axis; positive angles turn counter-clockwise about it.
    pub axis: DVec3,
    /// Vector from the axis to the profile anchor (perpendicular to `axis`).
    pub radial: DVec3,
    pub angle: f64,
}

impl ArcHandle {
    fn point(&self, a: f64) -> DVec3 {
        self.origin + glam::DQuat::from_axis_angle(self.axis, a) * self.radial
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) enum Manip {
    Arrow(Arrow),
    Arc(ArcHandle),
}

#[derive(Clone, Copy, Debug)]
pub(super) struct ManipDrag {
    /// Axis parameter at the grab point minus the current length.
    pub grab_offset: f64,
}

impl PartStudioView {
    /// The manipulator of the feature being edited, if it has one.
    pub(super) fn current_manip(&self, cx: &App) -> Option<Manip> {
        match self.edit.as_ref()?.dialog.read(cx).feature.type_id.as_str() {
            "extrude" => self.current_arrow(cx).map(Manip::Arrow),
            "revolve" => self.current_arc(cx).map(Manip::Arc),
            _ => None,
        }
    }

    fn current_arc(&self, cx: &App) -> Option<ArcHandle> {
        let d = self.edit.as_ref()?.dialog.read(cx);
        let p = &d.feature.params;
        if p.bool("fullRevolve") || !p.is("endBound", "BLIND") {
            return None;
        }
        let units = self.doc.read(cx).session.doc.settings.units.clone();
        let angle = p.expr("angle").and_then(|x| x.cached.map(|q| q.value).or_else(|| noshape_units::evaluate_as(&x.text, Dim::ANGLE, &units, &self.display.variables).ok().map(|q| q.value)))?;
        let anchor = p.query("entities").iter().find_map(|r| match r {
            Reference::SketchRegion { sketch, key } => {
                let sk = self.display.sketches.iter().find(|s| s.feature == *sketch)?;
                let reg = sk.regions.iter().find(|g| &g.key == key)?;
                region_anchor(&reg.positions, &reg.indices)
            }
            _ => None,
        })?;
        let (origin, mut axis) = self.axis_of(p.query("axis").first()?)?;
        if p.bool("oppositeDirection") {
            axis = -axis;
        }
        let rel = anchor - origin;
        let radial = rel - axis * axis.dot(rel);
        if radial.length() < 1e-9 {
            return None;
        }
        let symmetric = p.bool("symmetric");
        Some(ArcHandle { origin: origin + axis * axis.dot(rel), axis, radial, angle: if symmetric { angle * 0.5 } else { angle } })
    }

    /// World axis (point, unit direction) of a sketch line or straight/circular edge.
    fn axis_of(&self, r: &Reference) -> Option<(DVec3, DVec3)> {
        let f = |p: [f32; 3]| DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64);
        match r {
            Reference::SketchEntity { sketch, entity } => {
                let c = self.display.sketches.iter().find(|s| s.feature == *sketch)?.curves.iter().find(|c| c.entity == *entity)?;
                let (a, b) = (f(*c.points.first()?), f(*c.points.last()?));
                ((b - a).length() > 1e-12).then(|| (a, (b - a).normalize()))
            }
            Reference::Edge(_) => {
                let ctx = self.context.as_ref()?;
                let e = noshape_model::regen::measure::resolve_shapes(ctx, r).into_iter().next()?;
                match e.edge_curve().ok()? {
                    noshape_kernel::Curve::Line { origin, direction } => Some((origin, direction.normalize())),
                    noshape_kernel::Curve::Circle { center, axis, .. } => Some((center, axis.normalize())),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// The extrude depth arrow.
    fn current_arrow(&self, cx: &App) -> Option<Arrow> {
        let e = self.edit.as_ref()?;
        let d = e.dialog.read(cx);
        if d.feature.type_id != "extrude" || !d.feature.params.is("endBound", "BLIND") || d.feature.params.is("bodyType", "SURFACE") {
            return None;
        }
        let depth = d.feature.params.expr("depth").and_then(|x| x.cached.map(|q| q.value)).or_else(|| {
            let units = self.doc.read(cx).session.doc.settings.units.clone();
            d.feature.params.expr("depth").and_then(|x| noshape_units::evaluate_as(&x.text, Dim::LENGTH, &units, &self.display.variables).ok()).map(|q| q.value)
        })?;
        // Profile center and normal from the first selected region.
        let (base, normal) = d.feature.params.query("entities").iter().find_map(|r| match r {
            Reference::SketchRegion { sketch, key } => {
                let sk = self.display.sketches.iter().find(|s| s.feature == *sketch)?;
                let reg = sk.regions.iter().find(|g| &g.key == key)?;
                Some((region_anchor(&reg.positions, &reg.indices)?, sk.frame.normal))
            }
            _ => None,
        })?;
        let mut dir = normal;
        if d.feature.params.bool("oppositeDirection") {
            dir = -dir;
        }
        let symmetric = d.feature.params.bool("symmetric");
        Some(Arrow { base, dir, length: if symmetric { depth * 0.5 } else { depth } })
    }

    /// Publish the current arrow's handle to the viewport hit test.
    pub(super) fn sync_manipulator(&mut self, cx: &mut Context<Self>) {
        let handles: Vec<(DVec3, DVec3)> = match self.current_manip(cx) {
            Some(Manip::Arrow(a)) => vec![(a.base, a.base + a.dir * a.length)],
            // The handle end of the arc (last few degrees) is grabbable.
            Some(Manip::Arc(h)) => vec![(h.point(h.angle * 0.85), h.point(h.angle))],
            None => Vec::new(),
        };
        self.viewport.update(cx, |v, _| v.manipulators = handles);
    }

    /// Overlay geometry for the active manipulator.
    pub(super) fn manipulator_overlay(&self, cx: &App) -> Vec<Instance> {
        let up = self.gpu.uploader();
        let f = |v: DVec3| -> [f32; 3] { [v.x as f32, v.y as f32, v.z as f32] };
        let color = [0x8b, 0x7c, 0xff, 255];
        let a = match self.current_manip(cx) {
            Some(Manip::Arrow(a)) => a,
            Some(Manip::Arc(h)) => {
                let mut lines = LineData::default();
                let n = ((h.angle.abs().to_degrees() / 4.0).ceil() as usize).clamp(2, 90);
                let pts: Vec<[f32; 3]> = (0..=n).map(|i| f(h.point(h.angle * i as f64 / n as f64))).collect();
                lines.push_polyline(&pts, color, 0, 2.5, LineSegment::ON_TOP);
                // Spokes from the axis to both ends.
                lines.push_polyline(&[f(h.origin), f(h.point(0.0))], color, 0, 1.0, LineSegment::ON_TOP | LineSegment::DASHED);
                lines.push_polyline(&[f(h.origin), f(h.point(h.angle))], color, 0, 1.0, LineSegment::ON_TOP | LineSegment::DASHED);
                let pts = PointData { points: vec![PointSprite { position: f(h.point(h.angle)), color, pick_id: 0, size: 11.0, style: LineSegment::ON_TOP, _pad: 0 }] };
                return vec![
                    Instance::new(Drawable::Lines(up.upload_lines(&lines))).per_primitive_color().unpickable().ignore_clip().layer(40),
                    Instance::new(Drawable::Points(up.upload_points(&pts))).per_primitive_color().unpickable().ignore_clip().layer(41),
                ];
            }
            None => return Vec::new(),
        };
        let tip = a.base + a.dir * a.length;
        let mut lines = LineData::default();
        lines.push_polyline(&[f(a.base), f(tip)], color, 0, 2.5, LineSegment::ON_TOP);
        // Arrowhead: two short barbs in a plane containing the axis.
        let side = a.dir.any_orthonormal_vector();
        let side2 = a.dir.cross(side);
        let head = (a.length.abs() * 0.12).clamp(0.0015, 0.008) * a.length.signum();
        for s in [side, -side, side2, -side2] {
            let barb = tip - a.dir * head + s * head.abs() * 0.45;
            lines.push_polyline(&[f(tip), f(barb)], color, 0, 2.5, LineSegment::ON_TOP);
        }
        let pts = PointData { points: vec![PointSprite { position: f(tip), color, pick_id: 0, size: 10.0, style: LineSegment::ON_TOP, _pad: 0 }] };
        vec![
            Instance::new(Drawable::Lines(up.upload_lines(&lines))).per_primitive_color().unpickable().ignore_clip().layer(40),
            Instance::new(Drawable::Points(up.upload_points(&pts))).per_primitive_color().unpickable().ignore_clip().layer(41),
        ]
    }

    /// Axis parameter of the point on the arrow axis closest to a view ray.
    fn axis_param(a: &Arrow, origin: DVec3, dir: DVec3) -> Option<f64> {
        // Closest points between the axis line and the ray.
        let w0 = a.base - origin;
        let (b, d, e) = (a.dir.dot(dir), a.dir.dot(w0), dir.dot(w0));
        let denom = 1.0 - b * b;
        if denom.abs() < 1e-9 {
            return None;
        }
        Some((b * e - d) / denom)
    }

    /// Handle tool events for a manipulator drag. Returns true if consumed.
    pub(super) fn manipulator_event(&mut self, ev: &ViewportEvent, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let a = match self.current_manip(cx) {
            Some(Manip::Arrow(a)) => a,
            Some(Manip::Arc(h)) => return self.arc_event(h, ev, window, cx),
            None => {
                self.manip_drag = None;
                return false;
            }
        };
        let ray = |this: &Self, p: DVec2, cx: &App| this.viewport.read(cx).ray(p);
        match ev {
            ViewportEvent::ToolDown { position, .. } => {
                let (o, d) = ray(self, *position, cx);
                let t = Self::axis_param(&a, o, d).unwrap_or(a.length);
                self.manip_drag = Some(ManipDrag { grab_offset: t - a.length });
                true
            }
            ViewportEvent::ToolDrag { position, .. } => {
                let Some(m) = self.manip_drag else { return false };
                let (o, d) = ray(self, *position, cx);
                let Some(t) = Self::axis_param(&a, o, d) else { return true };
                let mut len = t - m.grab_offset;
                let symmetric = self.edit.as_ref().is_some_and(|e| e.dialog.read(cx).feature.params.bool("symmetric"));
                if symmetric {
                    len *= 2.0;
                }
                let flip = len < 0.0;
                // Snap to a round step one decade below the value (Onshape-like).
                let len = len.abs().max(1e-6);
                let step = 10f64.powf((len * 1e3).log10().floor() - 1.0).max(0.01) * 1e-3;
                let len = ((len / step).round() * step).max(step);
                let units = self.doc.read(cx).session.doc.settings.units.clone();
                let text = noshape_units::format_quantity(Quantity::new(len, Dim::LENGTH), &units);
                if let Some(e) = &self.edit {
                    e.dialog.update(cx, |dlg, cx| {
                        if flip {
                            let cur = dlg.feature.params.bool("oppositeDirection");
                            dlg.feature.params.set("oppositeDirection", ParamValue::Bool(!cur));
                        }
                        dlg.set_expr_text("depth", &text, window, cx);
                    });
                }
                if flip {
                    // Keep grabbing the same point after the flip.
                    self.manip_drag = Some(ManipDrag { grab_offset: -m.grab_offset });
                }
                true
            }
            ViewportEvent::ToolUp { .. } => self.manip_drag.take().is_some(),
            _ => false,
        }
    }
}

impl PartStudioView {
    fn arc_event(&mut self, h: ArcHandle, ev: &ViewportEvent, window: &mut Window, cx: &mut Context<Self>) -> bool {
        // Angle of the cursor around the axis (ray ∩ plane through the arc).
        let angle_at = |this: &Self, p: DVec2, cx: &App| -> Option<f64> {
            let (o, d) = this.viewport.read(cx).ray(p);
            let denom = d.dot(h.axis);
            if denom.abs() < 1e-6 {
                return None;
            }
            let hit = o + d * ((h.origin - o).dot(h.axis) / denom);
            let v = hit - h.origin;
            let (x, y) = (h.radial.normalize(), h.axis.cross(h.radial).normalize());
            let a = v.dot(y).atan2(v.dot(x));
            // Continue past 180° from the current angle (0..360 range).
            Some(if a < 0.0 { a + std::f64::consts::TAU } else { a })
        };
        match ev {
            ViewportEvent::ToolDown { .. } => {
                self.manip_drag = Some(ManipDrag { grab_offset: 0.0 });
                true
            }
            ViewportEvent::ToolDrag { position, .. } => {
                if self.manip_drag.is_none() {
                    return false;
                }
                let Some(mut a) = angle_at(self, *position, cx) else { return true };
                let symmetric = self.edit.as_ref().is_some_and(|e| e.dialog.read(cx).feature.params.bool("symmetric"));
                if symmetric {
                    a = (a * 2.0).min(std::f64::consts::TAU);
                }
                // Whole degrees, never zero.
                let deg = a.to_degrees().round().clamp(1.0, 360.0);
                let units = self.doc.read(cx).session.doc.settings.units.clone();
                let text = noshape_units::format_quantity(Quantity::new(deg.to_radians(), Dim::ANGLE), &units);
                if let Some(e) = &self.edit {
                    e.dialog.update(cx, |dlg, cx| dlg.set_expr_text("angle", &text, window, cx));
                }
                true
            }
            ViewportEvent::ToolUp { .. } => self.manip_drag.take().is_some(),
            _ => false,
        }
    }
}

/// A point on a triangulated region near its area centroid (the centroid of
/// a region with holes can fall outside it).
fn region_anchor(positions: &[[f32; 3]], indices: &[u32]) -> Option<DVec3> {
    let v = |i: u32| positions.get(i as usize).map(|p| DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64));
    let tris: Vec<(DVec3, f64)> = indices
        .chunks_exact(3)
        .filter_map(|t| {
            let (a, b, c) = (v(t[0])?, v(t[1])?, v(t[2])?);
            Some(((a + b + c) / 3.0, (b - a).cross(c - a).length() * 0.5))
        })
        .collect();
    let total: f64 = tris.iter().map(|t| t.1).sum();
    if total <= 0.0 {
        return None;
    }
    let centroid = tris.iter().fold(DVec3::ZERO, |acc, t| acc + t.0 * t.1) / total;
    // Nearest sizable triangle center to the centroid.
    let min_area = total / tris.len() as f64 * 0.25;
    tris.iter().filter(|t| t.1 >= min_area).min_by(|a, b| a.0.distance_squared(centroid).total_cmp(&b.0.distance_squared(centroid))).map(|t| t.0)
}
