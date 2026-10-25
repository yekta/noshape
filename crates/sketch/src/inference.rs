//! Snapping / inference for interactive sketching (pure functions).
//!
//! Given the cursor position, the active tool and the existing geometry,
//! [`infer`] returns ranked candidates (endpoint, midpoint, center,
//! quadrant, on-curve, horizontal/vertical alignment, and anchor-relative
//! horizontal / vertical / perpendicular / parallel / tangent hints), each
//! with the constraints that should be auto-created if the user clicks.

use crate::constraints::ConstraintKind;
use crate::entities::{EntityKind, Geometry};
use crate::geom::Curve2;
use crate::ids::EntityId;
use crate::sketch::Sketch;
use glam::DVec2;
use serde::{Deserialize, Serialize};
use std::f64::consts::PI;

/// The active sketch tool (affects which anchor-relative hints are offered).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SketchTool {
    #[default]
    Select,
    Point,
    Line,
    Rectangle,
    Circle,
    Arc,
    TangentArc,
    Spline,
    Other,
}

/// Inference query parameters.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InferenceContext {
    pub tool: SketchTool,
    /// Previous click of the current tool (e.g. line start), if any.
    pub anchor: Option<DVec2>,
    /// Existing point entity at the anchor (e.g. the end of the previous
    /// line when chaining), used for perpendicular/tangent hints.
    pub anchor_point: Option<EntityId>,
    /// Snap radius in sketch units (pixels / zoom).
    pub snap_radius: f64,
    /// Angular tolerance for horizontal/vertical/parallel/perpendicular hints.
    pub angle_tolerance: f64,
    /// Entities to ignore (the geometry being created or dragged).
    pub exclude: Vec<EntityId>,
    /// Offer the sketch origin as a snap point and for horizontal /
    /// vertical alignment (Onshape's always-on origin inferences). Its
    /// constraints reference [`ORIGIN`], which the caller replaces with a
    /// real (fixed) point when the click is placed.
    #[serde(default)]
    pub origin: bool,
    /// "Woken" entities: curves and points the cursor recently passed over.
    /// Lines give parallel / perpendicular hints for the line being drawn
    /// even when they don't touch it; points give horizontal / vertical
    /// alignment, which other points don't (Onshape's wake-up inferences,
    /// behaviours "skb (10-25) sketch inference").
    #[serde(default)]
    pub woken: Vec<EntityId>,
}

/// Placeholder id for the sketch origin in inference constraints.
pub const ORIGIN: EntityId = EntityId(u32::MAX);

impl Default for InferenceContext {
    fn default() -> Self {
        InferenceContext {
            tool: SketchTool::Select,
            anchor: None,
            anchor_point: None,
            snap_radius: 1e-3,
            angle_tolerance: 3f64.to_radians(),
            exclude: Vec::new(),
            origin: false,
            woken: Vec::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum InferenceKind {
    /// Existing point (curve endpoint or standalone point).
    Endpoint,
    Midpoint,
    Center,
    Quadrant,
    OnCurve,
    /// Horizontally aligned with an existing point (dotted guide).
    AlignHorizontal,
    /// Vertically aligned with an existing point.
    AlignVertical,
    /// Aligned both horizontally and vertically with existing points.
    AlignBoth,
    /// New line from the anchor is horizontal.
    Horizontal,
    Vertical,
    Perpendicular,
    Parallel,
    Tangent,
}

/// A constraint to create when the candidate is accepted. "New point" is
/// the point being placed; "new curve" is the curve being drawn from the
/// anchor (if any).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum AutoConstraint {
    /// New point coincident with (merged into) an existing point.
    CoincidentPoint(EntityId),
    /// New point on a curve.
    OnCurve(EntityId),
    /// New point at the midpoint of a line/arc.
    MidpointOf(EntityId),
    /// New point horizontally aligned with a point.
    HorizontalWith(EntityId),
    VerticalWith(EntityId),
    /// The new curve (line) is horizontal / vertical.
    HorizontalLine,
    VerticalLine,
    PerpendicularTo(EntityId),
    ParallelTo(EntityId),
    TangentTo(EntityId),
    /// New point at an end of an ellipse's major (`true`) or minor axis.
    QuadrantOf(EntityId, bool),
}

impl AutoConstraint {
    /// Concrete constraint for the given new point / new curve.
    pub fn to_constraint(&self, new_point: EntityId, new_curve: Option<EntityId>) -> Option<ConstraintKind> {
        use AutoConstraint::*;
        Some(match *self {
            CoincidentPoint(p) => ConstraintKind::Coincident { a: new_point, b: p },
            OnCurve(c) => ConstraintKind::Coincident { a: new_point, b: c },
            MidpointOf(c) => ConstraintKind::Midpoint { point: new_point, curve: c },
            HorizontalWith(p) => ConstraintKind::Horizontal { a: new_point, b: Some(p) },
            VerticalWith(p) => ConstraintKind::Vertical { a: new_point, b: Some(p) },
            HorizontalLine => ConstraintKind::Horizontal { a: new_curve?, b: None },
            VerticalLine => ConstraintKind::Vertical { a: new_curve?, b: None },
            PerpendicularTo(l) => ConstraintKind::Perpendicular { a: new_curve?, b: l },
            ParallelTo(l) => ConstraintKind::Parallel { a: new_curve?, b: l },
            TangentTo(c) => ConstraintKind::Tangent { a: new_curve?, b: c },
            QuadrantOf(e, major) => ConstraintKind::Quadrant { point: new_point, ellipse: e, major },
        })
    }
}

/// A snapping candidate.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Inference {
    pub kind: InferenceKind,
    /// Snapped position for the new point.
    pub position: DVec2,
    /// Entity the inference refers to.
    pub entity: Option<EntityId>,
    /// Reference location for drawing guides (aligned point, line start, ...).
    pub reference: Option<DVec2>,
    pub constraints: Vec<AutoConstraint>,
    /// Distance from the cursor to the snapped position.
    pub distance: f64,
    /// Lower is better.
    pub priority: u8,
}

fn excluded(sk: &Sketch, ctx: &InferenceContext, id: EntityId) -> bool {
    if ctx.exclude.contains(&id) {
        return true;
    }
    // points of excluded curves
    ctx.exclude.iter().any(|e| sk.entity(*e).is_some_and(|x| x.point_refs().contains(&id)))
}

/// Computes inference candidates, best first.
pub fn infer(sk: &Sketch, cursor: DVec2, ctx: &InferenceContext) -> Vec<Inference> {
    let r = ctx.snap_radius.max(1e-15);
    let mut out: Vec<Inference> = Vec::new();
    let push = |out: &mut Vec<Inference>, kind, position: DVec2, entity, reference, constraints: Vec<AutoConstraint>, priority| {
        out.push(Inference { kind, position, entity, reference, constraints, distance: (position - cursor).length(), priority });
    };
    // role of points (center vs endpoint)
    let mut centers: Vec<EntityId> = Vec::new();
    for e in sk.entities() {
        if let Some(c) = e.center() {
            centers.push(c);
        }
    }
    let mut best_h: Option<(f64, EntityId, DVec2)> = None;
    let mut best_v: Option<(f64, EntityId, DVec2)> = None;
    for e in sk.entities() {
        if excluded(sk, ctx, e.id) {
            continue;
        }
        match &e.geometry {
            Geometry::Point { pos } => {
                let d = (*pos - cursor).length();
                if d <= r {
                    let kind = if centers.contains(&e.id) { InferenceKind::Center } else { InferenceKind::Endpoint };
                    push(&mut out, kind, *pos, Some(e.id), None, vec![AutoConstraint::CoincidentPoint(e.id)], 0);
                }
                // Alignment: woken points and a point on the (always-on) origin.
                if !(ctx.woken.contains(&e.id) || (ctx.origin && pos.length() < 1e-12)) {
                    continue;
                }
                let dx = (pos.x - cursor.x).abs();
                let dy = (pos.y - cursor.y).abs();
                if dx <= r && d > r && best_v.is_none_or(|b| dx < b.0) {
                    best_v = Some((dx, e.id, *pos));
                }
                if dy <= r && d > r && best_h.is_none_or(|b| dy < b.0) {
                    best_h = Some((dy, e.id, *pos));
                }
            }
            _ => {
                let Some(c) = sk.curve(e.id) else { continue };
                if !c.bbox().expanded(r).contains(cursor) {
                    continue;
                }
                // midpoints
                match &c {
                    Curve2::Line { a, b } => {
                        let m = (*a + *b) * 0.5;
                        if (m - cursor).length() <= r {
                            push(&mut out, InferenceKind::Midpoint, m, Some(e.id), None, vec![AutoConstraint::MidpointOf(e.id)], 1);
                        }
                    }
                    Curve2::Arc { start_angle, end_angle, .. } => {
                        let m = c.point(0.5 * (start_angle + end_angle));
                        if (m - cursor).length() <= r {
                            push(&mut out, InferenceKind::Midpoint, m, Some(e.id), None, vec![AutoConstraint::MidpointOf(e.id)], 1);
                        }
                    }
                    _ => {}
                }
                // Circles and arcs have no quadrant snaps (live "skb (10-25)
                // sketch inference": 1.9 px off the top or left quadrant the
                // end stays where it is, on the curve). Ellipse quadrants
                // (axis end points) are Onshape's QUADRANT constraint.
                if let (Some(a), SketchTool::Line, Curve2::Circle { center, .. } | Curve2::Arc { center, .. }) = (ctx.anchor, ctx.tool, &c) {
                    // A line ending on a circle or arc nearly along its
                    // tangent there: Coincident + Tangent (live, no Horizontal).
                    let (_, q, d) = c.closest(cursor);
                    let (u, t) = ((q - a).normalize_or_zero(), (q - *center).perp().normalize_or_zero());
                    if d <= r && u != DVec2::ZERO && t != DVec2::ZERO && u.perp_dot(t).abs() <= ctx.angle_tolerance.sin() {
                        push(&mut out, InferenceKind::Tangent, q, Some(e.id), Some(a), vec![AutoConstraint::OnCurve(e.id), AutoConstraint::TangentTo(e.id)], 1);
                    }
                }
                // ellipse quadrants (axis end points)
                if let Curve2::Ellipse { center, major, minor } | Curve2::EllipticalArc { center, major, minor, .. } = &c {
                    let n = major.normalize_or_zero().perp() * *minor;
                    for (q, is_major) in [(*center + *major, true), (*center - *major, true), (*center + n, false), (*center - n, false)] {
                        if (q - cursor).length() <= r && c.closest(q).2 < 1e-9 * (1.0 + major.length()) {
                            push(&mut out, InferenceKind::Quadrant, q, Some(e.id), Some(*center), vec![AutoConstraint::QuadrantOf(e.id, is_major)], 1);
                        }
                    }
                }
                // on curve
                let (_, q, d) = c.closest(cursor);
                if d <= r {
                    push(&mut out, InferenceKind::OnCurve, q, Some(e.id), None, vec![AutoConstraint::OnCurve(e.id)], 3);
                }
            }
        }
    }
    // the sketch origin (unless a point already sits on it)
    if ctx.origin && !sk.entities().any(|e| matches!(e.geometry, Geometry::Point { pos } if pos.length() < 1e-12)) {
        let d = cursor.length();
        if d <= r {
            push(&mut out, InferenceKind::Endpoint, DVec2::ZERO, Some(ORIGIN), None, vec![AutoConstraint::CoincidentPoint(ORIGIN)], 0);
        } else {
            if cursor.x.abs() <= r && best_v.is_none_or(|b| cursor.x.abs() < b.0) {
                best_v = Some((cursor.x.abs(), ORIGIN, DVec2::ZERO));
            }
            if cursor.y.abs() <= r && best_h.is_none_or(|b| cursor.y.abs() < b.0) {
                best_h = Some((cursor.y.abs(), ORIGIN, DVec2::ZERO));
            }
        }
    }
    // alignment with existing points
    match (best_h, best_v) {
        (Some((_, hp, hpos)), Some((_, vp, vpos))) if hp != vp => {
            let p = DVec2::new(vpos.x, hpos.y);
            push(
                &mut out,
                InferenceKind::AlignBoth,
                p,
                None,
                Some(hpos),
                vec![AutoConstraint::HorizontalWith(hp), AutoConstraint::VerticalWith(vp)],
                4,
            );
            push(&mut out, InferenceKind::AlignHorizontal, DVec2::new(cursor.x, hpos.y), Some(hp), Some(hpos), vec![AutoConstraint::HorizontalWith(hp)], 5);
            push(&mut out, InferenceKind::AlignVertical, DVec2::new(vpos.x, cursor.y), Some(vp), Some(vpos), vec![AutoConstraint::VerticalWith(vp)], 5);
        }
        (Some((_, hp, hpos)), _) => {
            push(&mut out, InferenceKind::AlignHorizontal, DVec2::new(cursor.x, hpos.y), Some(hp), Some(hpos), vec![AutoConstraint::HorizontalWith(hp)], 5);
        }
        (_, Some((_, vp, vpos))) => {
            push(&mut out, InferenceKind::AlignVertical, DVec2::new(vpos.x, cursor.y), Some(vp), Some(vpos), vec![AutoConstraint::VerticalWith(vp)], 5);
        }
        _ => {}
    }
    // anchor-relative hints for line-like tools
    if let (Some(a), true) = (ctx.anchor, matches!(ctx.tool, SketchTool::Line | SketchTool::Spline | SketchTool::Other)) {
        let d = cursor - a;
        let len = d.length();
        if len > 1e-15 {
            let ang = d.y.atan2(d.x);
            let tol = ctx.angle_tolerance;
            let near = |x: f64, target: f64| {
                let diff = (x - target).rem_euclid(PI);
                diff.min(PI - diff) <= tol
            };
            if near(ang, 0.0) {
                push(&mut out, InferenceKind::Horizontal, DVec2::new(cursor.x, a.y), None, Some(a), vec![AutoConstraint::HorizontalLine], 2);
                // A horizontal line ending in vertical alignment with a woken
                // point takes both (live: Horizontal + Vertical with it).
                if let Some((_, vp, vpos)) = best_v {
                    push(&mut out, InferenceKind::AlignVertical, DVec2::new(vpos.x, a.y), Some(vp), Some(vpos), vec![AutoConstraint::HorizontalLine, AutoConstraint::VerticalWith(vp)], 1);
                }
            }
            if near(ang, PI / 2.0) {
                push(&mut out, InferenceKind::Vertical, DVec2::new(a.x, cursor.y), None, Some(a), vec![AutoConstraint::VerticalLine], 2);
                if let Some((_, hp, hpos)) = best_h {
                    push(&mut out, InferenceKind::AlignHorizontal, DVec2::new(a.x, hpos.y), Some(hp), Some(hpos), vec![AutoConstraint::VerticalLine, AutoConstraint::HorizontalWith(hp)], 1);
                }
            }
            // woken lines (not attached to the anchor): parallel / perpendicular
            for &w in &ctx.woken {
                if excluded(sk, ctx, w) || sk.kind(w) != Some(EntityKind::Line) {
                    continue;
                }
                if ctx.anchor_point.is_some_and(|ap| sk.entity(w).and_then(|e| e.endpoints()).is_some_and(|(s, e)| s == ap || e == ap)) {
                    continue;
                }
                let Some(Curve2::Line { a: la, b: lb }) = sk.curve(w) else { continue };
                let dir = (lb - la).normalize_or_zero();
                if dir == DVec2::ZERO {
                    continue;
                }
                // horizontal / vertical woken lines add nothing over the H/V hints
                if dir.x.abs() < 1e-9 || dir.y.abs() < 1e-9 {
                    continue;
                }
                let proj_on = |u: DVec2| a + u * d.dot(u);
                let perp = dir.perp();
                if near(ang, dir.y.atan2(dir.x)) {
                    let u = if d.dot(dir) < 0.0 { -dir } else { dir };
                    push(&mut out, InferenceKind::Parallel, proj_on(u), Some(w), Some(a), vec![AutoConstraint::ParallelTo(w)], 2);
                } else if near(ang, perp.y.atan2(perp.x)) {
                    let u = if d.dot(perp) < 0.0 { -perp } else { perp };
                    push(&mut out, InferenceKind::Perpendicular, proj_on(u), Some(w), Some(a), vec![AutoConstraint::PerpendicularTo(w)], 2);
                }
            }
            if let Some(ap) = ctx.anchor_point {
                for e in sk.entities() {
                    if excluded(sk, ctx, e.id) {
                        continue;
                    }
                    let Some((s, en)) = e.endpoints() else { continue };
                    if s != ap && en != ap {
                        continue;
                    }
                    let Some(c) = sk.curve(e.id) else { continue };
                    let (t0, t1) = c.domain();
                    // direction of the existing curve at the anchor, pointing away from it
                    let dir = if s == ap { c.tangent(t0) } else { -c.tangent(t1) };
                    let dir = dir.normalize_or_zero();
                    if dir == DVec2::ZERO {
                        continue;
                    }
                    let proj_on = |u: DVec2| a + u * d.dot(u);
                    match e.kind() {
                        EntityKind::Line => {
                            let perp = dir.perp();
                            if near(ang, perp.y.atan2(perp.x)) {
                                push(&mut out, InferenceKind::Perpendicular, proj_on(perp), Some(e.id), Some(a), vec![AutoConstraint::PerpendicularTo(e.id)], 2);
                            } else if near(ang, dir.y.atan2(dir.x)) {
                                push(&mut out, InferenceKind::Parallel, proj_on(dir), Some(e.id), Some(a), vec![AutoConstraint::ParallelTo(e.id)], 2);
                            }
                        }
                        k if k.is_circular() || k.is_spline() || k.is_elliptical() => {
                            if near(ang, dir.y.atan2(dir.x)) {
                                push(&mut out, InferenceKind::Tangent, proj_on(dir), Some(e.id), Some(a), vec![AutoConstraint::TangentTo(e.id)], 2);
                            } else if k.is_circular() {
                                let perp = dir.perp();
                                if near(ang, perp.y.atan2(perp.x)) {
                                    push(&mut out, InferenceKind::Perpendicular, proj_on(perp), Some(e.id), Some(a), vec![], 6);
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    out.sort_by(|x, y| x.priority.cmp(&y.priority).then(x.distance.total_cmp(&y.distance)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(x: f64, y: f64) -> DVec2 {
        DVec2::new(x, y)
    }

    #[test]
    fn snaps() {
        let mut sk = Sketch::new();
        let l = sk.add_line(v(0.0, 0.0), v(2.0, 0.0));
        let c = sk.add_circle(v(5.0, 0.0), 1.0);
        let ctx = InferenceContext { snap_radius: 0.1, ..Default::default() };
        let (s, _) = sk.endpoints(l).unwrap();
        let r = infer(&sk, v(0.05, 0.02), &ctx);
        assert_eq!(r[0].kind, InferenceKind::Endpoint);
        assert_eq!(r[0].constraints, vec![AutoConstraint::CoincidentPoint(s)]);
        let r = infer(&sk, v(1.02, 0.03), &ctx);
        assert_eq!(r[0].kind, InferenceKind::Midpoint);
        // no circle quadrants: on the curve where the cursor is
        let r = infer(&sk, v(5.03, 0.98), &ctx);
        assert_eq!(r[0].kind, InferenceKind::OnCurve);
        assert!((r[0].position - v(5.0, 1.0)).length() > 1e-3, "{:?}", r[0].position);
        let r = infer(&sk, v(5.02, -0.05), &ctx);
        assert_eq!(r[0].kind, InferenceKind::Center);
        let r = infer(&sk, v(1.5, 0.05), &ctx);
        assert_eq!(r[0].kind, InferenceKind::OnCurve);
        assert!((r[0].position - v(1.5, 0.0)).length() < 1e-12);
        let r = infer(&sk, v(5.6, 0.85), &ctx);
        assert!(r.iter().any(|x| x.kind == InferenceKind::OnCurve && x.entity == Some(c)));
        // alignment: only with a woken point (live: one the cursor passed over)
        let (_, le) = sk.endpoints(l).unwrap();
        assert!(infer(&sk, v(2.05, 3.0), &ctx).is_empty());
        let r = infer(&sk, v(2.05, 3.0), &InferenceContext { woken: vec![le], ..ctx.clone() });
        assert_eq!(r[0].kind, InferenceKind::AlignVertical);
        assert!((r[0].position.x - 2.0).abs() < 1e-12);
        // line tool: horizontal and perpendicular from an endpoint
        let (_, e) = sk.endpoints(l).unwrap();
        let ctx2 = InferenceContext { tool: SketchTool::Line, anchor: Some(v(2.0, 0.0)), anchor_point: Some(e), snap_radius: 0.1, ..Default::default() };
        let r = infer(&sk, v(4.0, 0.03 + 3.0), &ctx2);
        let _ = r;
        let r = infer(&sk, v(2.02, 3.0), &ctx2);
        assert!(r.iter().any(|x| x.kind == InferenceKind::Perpendicular));
        assert!(r.iter().any(|x| x.kind == InferenceKind::Vertical));
        let r = infer(&sk, v(3.0, 3.0 * 0.01), &ctx2);
        assert!(r.iter().any(|x| x.kind == InferenceKind::Horizontal));
        let k = AutoConstraint::HorizontalLine.to_constraint(EntityId(99), Some(l)).unwrap();
        assert_eq!(k, ConstraintKind::Horizontal { a: l, b: None });
    }

    /// Live ("skb (10-25) sketch inference"): a horizontal line ending under
    /// a woken point gets Horizontal + Vertical with that point.
    #[test]
    fn horizontal_line_and_woken_point_alignment_combine() {
        let mut sk = Sketch::new();
        let p = sk.add_point(v(2.0, 3.0));
        let ctx = InferenceContext { tool: SketchTool::Line, anchor: Some(v(4.0, 6.0)), snap_radius: 0.1, woken: vec![p], ..Default::default() };
        let r = infer(&sk, v(2.03, 6.03), &ctx);
        assert_eq!(r[0].constraints, vec![AutoConstraint::HorizontalLine, AutoConstraint::VerticalWith(p)]);
        assert!((r[0].position - v(2.0, 6.0)).length() < 1e-12);
        let r = infer(&sk, v(2.03, 6.03), &InferenceContext { woken: vec![], ..ctx });
        assert_eq!(r[0].constraints, vec![AutoConstraint::HorizontalLine]);
    }

    #[test]
    fn tangent_hint_from_arc_end() {
        let mut sk = Sketch::new();
        let a = sk.add_arc(v(0.0, 0.0), v(1.0, 0.0), v(0.0, 1.0));
        let (_, e) = sk.endpoints(a).unwrap();
        let ctx = InferenceContext { tool: SketchTool::Line, anchor: Some(v(0.0, 1.0)), anchor_point: Some(e), snap_radius: 0.05, ..Default::default() };
        // tangent direction at the arc end (CCW) continues toward -x
        let r = infer(&sk, v(-2.0, 1.02), &ctx);
        let t = r.iter().find(|x| x.kind == InferenceKind::Tangent).expect("tangent hint");
        assert_eq!(t.constraints, vec![AutoConstraint::TangentTo(a)]);
        assert!((t.position - v(-2.0, 1.0)).length() < 1e-9);
    }

    #[test]
    fn origin_snap_and_alignment() {
        let sk = Sketch::new();
        let ctx = InferenceContext { origin: true, snap_radius: 0.1, ..Default::default() };
        let r = infer(&sk, v(0.03, -0.04), &ctx);
        assert_eq!(r[0].kind, InferenceKind::Endpoint);
        assert_eq!(r[0].constraints, vec![AutoConstraint::CoincidentPoint(ORIGIN)]);
        assert_eq!(r[0].position, DVec2::ZERO);
        // directly above the origin: vertical alignment with it
        let r = infer(&sk, v(0.05, 2.0), &ctx);
        assert_eq!(r[0].kind, InferenceKind::AlignVertical);
        assert_eq!(r[0].constraints, vec![AutoConstraint::VerticalWith(ORIGIN)]);
        assert!((r[0].position - v(0.0, 2.0)).length() < 1e-12);
        // without the flag nothing is offered
        assert!(infer(&sk, v(0.03, -0.04), &InferenceContext { snap_radius: 0.1, ..Default::default() }).is_empty());
        // an existing point on the origin replaces the virtual one
        let mut sk2 = Sketch::new();
        let p = sk2.add_point(DVec2::ZERO);
        let r = infer(&sk2, v(0.03, -0.04), &ctx);
        assert_eq!(r[0].constraints, vec![AutoConstraint::CoincidentPoint(p)]);
        assert_eq!(r.iter().filter(|x| x.kind == InferenceKind::Endpoint).count(), 1);
    }

    #[test]
    fn woken_line_gives_parallel_and_perpendicular() {
        let mut sk = Sketch::new();
        let l = sk.add_line(v(0.0, 0.0), v(1.0, 1.0));
        let base = InferenceContext { tool: SketchTool::Line, anchor: Some(v(5.0, 0.0)), snap_radius: 0.05, ..Default::default() };
        // not woken: no parallel hint
        let r = infer(&sk, v(7.0, 2.01), &base);
        assert!(!r.iter().any(|x| x.kind == InferenceKind::Parallel));
        let ctx = InferenceContext { woken: vec![l], ..base };
        let r = infer(&sk, v(7.0, 2.01), &ctx);
        let par = r.iter().find(|x| x.kind == InferenceKind::Parallel).expect("parallel");
        assert_eq!(par.constraints, vec![AutoConstraint::ParallelTo(l)]);
        let q = par.position - v(5.0, 0.0);
        assert!((q.x - q.y).abs() < 1e-9);
        let r = infer(&sk, v(3.0, 2.02), &ctx);
        let perp = r.iter().find(|x| x.kind == InferenceKind::Perpendicular).expect("perpendicular");
        assert_eq!(perp.constraints, vec![AutoConstraint::PerpendicularTo(l)]);
    }
}
