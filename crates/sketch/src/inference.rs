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
    /// How near a point's x or y aligns with another's (None: the snap radius).
    #[serde(default)]
    pub align_radius: Option<f64>,
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
    /// The view's projection of the plane: reach measured on screen (live's), not in the sketch. None: in the sketch
    /// (the agent API's headless editor, tests).
    #[serde(default)]
    pub screen: Option<crate::geom::ScreenMetric>,
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
            align_radius: None,
            exclude: Vec::new(),
            origin: false,
            woken: Vec::new(),
            screen: None,
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

/// Whether a constraint holds line `l`'s direction: its own Horizontal /
/// Vertical or Fix, or a Parallel / Perpendicular to a line so held (a
/// rectangle's sides); an external line's is held too.
fn direction_held(sk: &Sketch, l: EntityId) -> bool {
    let mut seen = vec![l];
    let mut i = 0;
    while let Some(&x) = seen.get(i) {
        i += 1;
        if sk.entity(x).is_some_and(|e| e.external) {
            return true;
        }
        for c in sk.constraints() {
            match c.kind {
                ConstraintKind::Horizontal { a, b: None } | ConstraintKind::Vertical { a, b: None } | ConstraintKind::Fix { entity: a } if a == x => return true,
                ConstraintKind::Parallel { a, b } | ConstraintKind::Perpendicular { a, b } if a == x || b == x => {
                    let o = if a == x { b } else { a };
                    if !seen.contains(&o) {
                        seen.push(o);
                    }
                }
                _ => {}
            }
        }
    }
    false
}

fn excluded(sk: &Sketch, ctx: &InferenceContext, id: EntityId) -> bool {
    // (A spline's handle arm: no inference to or along it.)
    if ctx.exclude.contains(&id) || sk.entity(id).is_some_and(|e| e.handle_of.is_some()) {
        return true;
    }
    // points of excluded curves
    ctx.exclude.iter().any(|e| sk.entity(*e).is_some_and(|x| x.point_refs().contains(&id)))
}

/// Without a view (the headless editor): how far a point / a midpoint reaches next to a curve's
/// [`InferenceContext::snap_radius`]. On screen a point is live's square instead (see [`infer`]).
pub const POINT_REACH: f64 = 8.5 / 7.5;
pub const MIDPOINT_REACH: f64 = 6.0 / 7.5;
/// Horizontal and vertical (with the line's start or a point): live's inference lines are 15 px wide, ±7.5 px at the
/// pointer, as a curve's (live/feel/sketch-snaps.json "pickShapes"; measured 7–8 px each side).
pub const HORIZONTAL_REACH: f64 = 1.0;
pub const VERTICAL_REACH: f64 = 1.0;

/// Perpendicular and tangent take a line within this many px² over its length in px of the exact direction, at the
/// pointer: 6 px at 60 px long, 2.4 at 150, 1.2 at 300 (live2 "feel (sketch)" (1): from an arc's end 6 / 2–4 / 1–2 px
/// at 60 / 100 / 200 px, square to a line 4 / 2–3 / 1–2 at 80 / 150 / 300), at most the snap radius.
pub const DIRECTION_REACH_PX2: f64 = 360.0;
/// The snap radius `InferenceContext::snap_radius` stands for, in px (the editor's).
const SNAP_RADIUS_PX: f64 = 7.5;

/// Computes inference candidates, best first.
pub fn infer(sk: &Sketch, cursor: DVec2, ctx: &InferenceContext) -> Vec<Inference> {
    let r = ctx.snap_radius.max(1e-15);
    let (r_point, r_mid) = (r * POINT_REACH, r * MIDPOINT_REACH);
    let (ra_h, ra_v) = match ctx.align_radius {
        Some(a) => (a, a),
        None => (r * HORIZONTAL_REACH, r * VERTICAL_REACH),
    };
    // Lengths on screen when the view is known: live draws each inference (a point, the horizontal or vertical
    // through one) into a pick pass and picks the pointer's pixel, so its reach is the pointer's distance on screen to
    // what is drawn (ui-parity sketch-horizontal-key / -vertical-key on Top: Horizontal 5.7 px off level, no Vertical
    // 7.9 px off plumb). `off_x` / `off_y`: from the vertical / horizontal through a point.
    let seen = |v: DVec2| ctx.screen.map_or(v.length(), |m| m.len(v));
    let off = |v: DVec2, along: DVec2| match ctx.screen {
        Some(m) => m.off_line(v, along),
        None => v.perp_dot(along).abs(),
    };
    let off_x = |v: DVec2| off(v, DVec2::Y);
    let off_y = |v: DVec2| off(v, DVec2::X);
    // A point (an end, a centre, a midpoint, the origin) is a 15 px square on screen round it, the pointer's pixel in
    // it (live/feel/sketch-snaps.json: INFERENCE_POINT_SIZE_PX 15; 7 px straight out, 9–10 diagonally, not 8 / 11).
    let in_point = |v: DVec2, headless: f64| match ctx.screen {
        Some(m) => m.box_len(v) < r,
        None => v.length() <= headless,
    };
    let mut out: Vec<Inference> = Vec::new();
    let push = |out: &mut Vec<Inference>, kind, position: DVec2, entity, reference, constraints: Vec<AutoConstraint>, priority| {
        out.push(Inference { kind, position, entity, reference, constraints, distance: seen(position - cursor), priority });
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
                let on_point = in_point(*pos - cursor, r_point);
                if on_point {
                    let kind = if centers.contains(&e.id) { InferenceKind::Center } else { InferenceKind::Endpoint };
                    push(&mut out, kind, *pos, Some(e.id), None, vec![AutoConstraint::CoincidentPoint(e.id)], 0);
                }
                // Alignment: woken points and a point on the (always-on) origin.
                if !(ctx.woken.contains(&e.id) || (ctx.origin && pos.length() < 1e-12)) {
                    continue;
                }
                let dx = off_x(cursor - *pos);
                let dy = off_y(cursor - *pos);
                if dx <= ra_v && !on_point && best_v.is_none_or(|b| dx < b.0) {
                    best_v = Some((dx, e.id, *pos));
                }
                if dy <= ra_h && !on_point && best_h.is_none_or(|b| dy < b.0) {
                    best_h = Some((dy, e.id, *pos));
                }
            }
            _ => {
                let Some(c) = sk.curve(e.id) else { continue };
                if !c.bbox().expanded(ctx.screen.map_or(r, |m| m.reach(r))).contains(cursor) {
                    continue;
                }
                // midpoints
                match &c {
                    Curve2::Line { a, b } => {
                        let m = (*a + *b) * 0.5;
                        if in_point(m - cursor, r_mid) {
                            push(&mut out, InferenceKind::Midpoint, m, Some(e.id), None, vec![AutoConstraint::MidpointOf(e.id)], 1);
                        }
                    }
                    Curve2::Arc { start_angle, end_angle, .. } => {
                        let m = c.point(0.5 * (start_angle + end_angle));
                        if in_point(m - cursor, r_mid) {
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
                    let (_, q, _) = c.closest(cursor);
                    let d = seen(q - cursor);
                    let (u, t) = ((q - a).normalize_or_zero(), (q - *center).perp().normalize_or_zero());
                    if d <= r && u != DVec2::ZERO && t != DVec2::ZERO && u.perp_dot(t).abs() <= ctx.angle_tolerance.sin() {
                        push(&mut out, InferenceKind::Tangent, q, Some(e.id), Some(a), vec![AutoConstraint::OnCurve(e.id), AutoConstraint::TangentTo(e.id)], 1);
                    }
                }
                // ellipse quadrants (axis end points)
                if let Curve2::Ellipse { center, major, minor } | Curve2::EllipticalArc { center, major, minor, .. } = &c {
                    let n = major.normalize_or_zero().perp() * *minor;
                    for (q, is_major) in [(*center + *major, true), (*center - *major, true), (*center + n, false), (*center - n, false)] {
                        if seen(q - cursor) <= r && c.closest(q).2 < 1e-9 * (1.0 + major.length()) {
                            push(&mut out, InferenceKind::Quadrant, q, Some(e.id), Some(*center), vec![AutoConstraint::QuadrantOf(e.id, is_major)], 1);
                        }
                    }
                }
                // on curve (a line's point nearest on screen)
                let q = match (&c, ctx.screen) {
                    (Curve2::Line { a, b }, Some(m)) => m.closest_on_segment(*a, *b, cursor),
                    _ => c.closest(cursor).1,
                };
                if seen(q - cursor) <= r {
                    push(&mut out, InferenceKind::OnCurve, q, Some(e.id), None, vec![AutoConstraint::OnCurve(e.id)], 3);
                }
            }
        }
    }
    // the sketch origin (unless a point already sits on it)
    if ctx.origin && !sk.entities().any(|e| matches!(e.geometry, Geometry::Point { pos } if pos.length() < 1e-12)) {
        if in_point(cursor, r_point) {
            push(&mut out, InferenceKind::Endpoint, DVec2::ZERO, Some(ORIGIN), None, vec![AutoConstraint::CoincidentPoint(ORIGIN)], 0);
        } else {
            let (dx, dy) = (off_x(cursor), off_y(cursor));
            if dx <= ra_v && best_v.is_none_or(|b| dx < b.0) {
                best_v = Some((dx, ORIGIN, DVec2::ZERO));
            }
            if dy <= ra_h && best_h.is_none_or(|b| dy < b.0) {
                best_h = Some((dy, ORIGIN, DVec2::ZERO));
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
            // On screen, live's reach for a direction shrinks with the line's length (`DIRECTION_REACH_PX2`); the
            // headless editor keeps its angle.
            let tol = match ctx.screen {
                Some(_) => {
                    let len_px = seen(d) / (r / SNAP_RADIUS_PX);
                    (DIRECTION_REACH_PX2 / len_px).min(SNAP_RADIUS_PX) / len_px.max(1.0)
                }
                None => ctx.angle_tolerance,
            };
            let near = |x: f64, target: f64| {
                let diff = (x - target).rem_euclid(PI);
                diff.min(PI - diff) <= tol
            };
            // A woken line the new line starts on (its midpoint or a point inside it): live2 "feel (sketch)" (1)
            // infers perpendicular only from a line the start is on, never to one elsewhere, and parallel never.
            let starts_on = |w: EntityId| sk.curve(w).is_some_and(|c| c.closest(a).2 <= 1e-9 * (1.0 + a.length()));
            // Square to a woken line (the pointer passed over it: the line
            // it starts on or crosses; not one joined at the anchor) whose
            // direction a constraint holds: Perpendicular to it instead of
            // Horizontal / Vertical (live 1866 (e): also one it crosses, both ends clear; on a free line, Vertical).
            let square_to = |horizontal: bool| {
                ctx.woken.iter().copied().find(|&w| {
                    !excluded(sk, ctx, w)
                        && !ctx.anchor_point.is_some_and(|ap| sk.entity(w).and_then(|e| e.endpoints()).is_some_and(|(s, e)| s == ap || e == ap))
                        && matches!(sk.curve(w), Some(Curve2::Line { a: la, b: lb }) if { let t = (lb - la).normalize_or_zero(); if horizontal { t.y.abs() > 1.0 - 1e-12 } else { t.x.abs() > 1.0 - 1e-12 } })
                        && direction_held(sk, w)
                })
            };
            let line_kind = |horizontal: bool| match square_to(horizontal) {
                Some(w) => (InferenceKind::Perpendicular, Some(w), AutoConstraint::PerpendicularTo(w)),
                None if horizontal => (InferenceKind::Horizontal, None, AutoConstraint::HorizontalLine),
                None => (InferenceKind::Vertical, None, AutoConstraint::VerticalLine),
            };
            // Live (ui-parity sketch-horizontal-key: (0, 0) to (30, 3) snaps level): a line turns horizontal or vertical
            // while the pointer is within the inference reach of the horizontal / vertical through its start, a
            // distance on screen, not an angle.
            let (reach_h, reach_v) = (ra_h, ra_v);
            let (to_h, to_v) = (off_y(d), off_x(d));
            let near_axis = |horizontal: bool| if horizontal { to_h <= reach_h && to_h < to_v } else { to_v <= reach_v && to_v <= to_h };
            if near_axis(true) {
                let (kind, on, c) = line_kind(true);
                push(&mut out, kind, DVec2::new(cursor.x, a.y), on, Some(a), vec![c], 2);
                // A horizontal line ending in vertical alignment with a woken
                // point takes both (live: Horizontal + Vertical with it).
                if let Some((_, vp, vpos)) = best_v {
                    push(&mut out, InferenceKind::AlignVertical, DVec2::new(vpos.x, a.y), Some(vp), Some(vpos), vec![c, AutoConstraint::VerticalWith(vp)], 1);
                }
            }
            if near_axis(false) {
                let (kind, on, c) = line_kind(false);
                push(&mut out, kind, DVec2::new(a.x, cursor.y), on, Some(a), vec![c], 2);
                if let Some((_, hp, hpos)) = best_h {
                    push(&mut out, InferenceKind::AlignHorizontal, DVec2::new(a.x, hpos.y), Some(hp), Some(hpos), vec![c, AutoConstraint::HorizontalWith(hp)], 1);
                }
            }
            // A woken line (snapped onto in this tool session; not one joined at the anchor): parallel and
            // perpendicular through the start (live/feel/sketch-snaps.json "wakeSummary": a sketch line → PARALLEL +
            // PERPENDICULAR through the start; not offered until it is woken).
            for &w in &ctx.woken {
                if excluded(sk, ctx, w) {
                    continue;
                }
                // A woken circle or arc: the two lines from the start tangent to it ("wakeSummary": circle / arc rim →
                // 2 TANGENT lines from the start); the end lands on the line, the line tangent to the circle.
                if let Some(Curve2::Circle { center, radius } | Curve2::Arc { center, radius, .. }) = sk.curve(w)
                    && ctx.anchor_point.is_none_or(|ap| sk.entity(w).and_then(|e| e.endpoints()).is_none_or(|(s, e)| s != ap && e != ap))
                {
                    let to_c = center - a;
                    let dist = to_c.length();
                    if dist > radius * (1.0 + 1e-9) {
                        let (base, off) = (to_c.y.atan2(to_c.x), (radius / dist).asin());
                        for t in [base + off, base - off] {
                            let u = DVec2::new(t.cos(), t.sin());
                            if d.dot(u) > 0.0 && near(ang, t) {
                                push(&mut out, InferenceKind::Tangent, a + u * d.dot(u), Some(w), Some(a), vec![AutoConstraint::TangentTo(w)], 2);
                            }
                        }
                    }
                    continue;
                }
                if sk.kind(w) != Some(EntityKind::Line) {
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
                if near(ang, perp.y.atan2(perp.x)) {
                    let u = if d.dot(perp) < 0.0 { -perp } else { perp };
                    push(&mut out, InferenceKind::Perpendicular, proj_on(u), Some(w), Some(a), vec![AutoConstraint::PerpendicularTo(w)], 2);
                } else if near(ang, dir.y.atan2(dir.x)) && !starts_on(w) {
                    // (Along the line it starts on, the end is on that line: no parallel to itself.)
                    let u = if d.dot(dir) < 0.0 { -dir } else { dir };
                    push(&mut out, InferenceKind::Parallel, proj_on(u), Some(w), Some(a), vec![AutoConstraint::ParallelTo(w)], 2);
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
                        // (Live2: a chain's fold-back or a collinear continuation gives nothing: no parallel.)
                        EntityKind::Line => {
                            let perp = dir.perp();
                            if near(ang, perp.y.atan2(perp.x)) {
                                push(&mut out, InferenceKind::Perpendicular, proj_on(perp), Some(e.id), Some(a), vec![AutoConstraint::PerpendicularTo(e.id)], 2);
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

    /// Live 1866: a line started on (or drawn across) a woken line whose
    /// direction a constraint holds (the Line tool's Horizontal, a
    /// rectangle's sides) and square to it is Perpendicular to it; on a free
    /// line, Vertical; a chain's corner (the lines joined at the anchor)
    /// stays Vertical; a line the pointer never passed over doesn't count.
    #[test]
    fn square_to_a_held_line_is_perpendicular() {
        let mut sk = Sketch::new();
        let held = sk.add_line(v(0.0, 0.0), v(4.0, 0.0));
        sk.add_constraint(ConstraintKind::Horizontal { a: held, b: None }).unwrap();
        let free = sk.add_line(v(0.0, 2.0), v(4.0, 2.0));
        let up = |sk: &Sketch, base: EntityId, y: f64| {
            let ctx = InferenceContext { tool: SketchTool::Line, anchor: Some(v(2.0, y)), snap_radius: 0.05, woken: vec![base], ..Default::default() };
            infer(sk, v(2.01, y + 1.5), &ctx).into_iter().next().unwrap()
        };
        let r = up(&sk, held, 0.0);
        assert_eq!((r.kind, r.constraints.clone()), (InferenceKind::Perpendicular, vec![AutoConstraint::PerpendicularTo(held)]));
        assert!((r.position.x - 2.0).abs() < 1e-12);
        assert_eq!(up(&sk, free, 2.0).kind, InferenceKind::Vertical);
        // From the held line's end, as a chain: Vertical.
        let (_, end) = sk.endpoints(held).unwrap();
        let ctx = InferenceContext { tool: SketchTool::Line, anchor: Some(v(4.0, 0.0)), anchor_point: Some(end), snap_radius: 0.05, woken: vec![held], ..Default::default() };
        assert_eq!(infer(&sk, v(4.01, 1.5), &ctx)[0].kind, InferenceKind::Vertical);
        // Across it, woken on the way: Perpendicular; not woken: Vertical.
        let across = InferenceContext { tool: SketchTool::Line, anchor: Some(v(1.0, -1.0)), snap_radius: 0.05, woken: vec![held], ..Default::default() };
        assert_eq!(infer(&sk, v(1.01, 1.0), &across)[0].constraints, vec![AutoConstraint::PerpendicularTo(held)]);
        let unwoken = InferenceContext { woken: Vec::new(), ..across };
        assert_eq!(infer(&sk, v(1.01, 1.0), &unwoken)[0].kind, InferenceKind::Vertical);
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

    /// Live's inference zones (live/feel/sketch-snaps.json, measured 7–8 px each way): the horizontal / vertical
    /// through the start 15 px wide; a point a 15 px square on screen, so 7 px straight out and 10 diagonally take it,
    /// 8 and 11 don't.
    #[test]
    fn lines_are_15_px_bands_and_points_15_px_squares() {
        let mut sk = Sketch::new();
        // One unit a pixel: the snap radius is the 7.5 px inference reach.
        let ctx = InferenceContext { tool: SketchTool::Line, anchor: Some(v(0.0, 0.0)), snap_radius: 7.5, origin: false, screen: Some(crate::geom::ScreenMetric(glam::DMat2::IDENTITY)), ..Default::default() };
        let kind = |sk: &Sketch, p: DVec2| infer(sk, p, &ctx).first().map(|i| i.kind);
        for len in [150.0, 300.0] {
            assert_eq!(kind(&sk, v(len, 7.0)), Some(InferenceKind::Horizontal), "{len}: 7 px off level");
            assert_eq!(kind(&sk, v(len, 8.0)), None, "{len}: 8 px off level");
            assert_eq!(kind(&sk, v(7.0, len)), Some(InferenceKind::Vertical), "{len}: 7 px off plumb");
            assert_eq!(kind(&sk, v(8.0, len)), None, "{len}: 8 px off plumb");
        }
        let p = sk.add_point(v(100.0, 60.0));
        let on = |sk: &Sketch, d: DVec2| infer(sk, v(100.0, 60.0) + d, &ctx).first().is_some_and(|i| i.entity == Some(p));
        assert!(on(&sk, v(7.0, 0.0)) && !on(&sk, v(8.0, 0.0)), "straight out: 7 px, not 8");
        let diag = |d: f64| v(d, d) * std::f64::consts::FRAC_1_SQRT_2;
        assert!(on(&sk, diag(10.0)) && !on(&sk, diag(11.0)), "diagonally: 10 px, not 11");
    }

    /// Live2's catalogue ("wakeSummary"): a woken sketch line gives PARALLEL and PERPENDICULAR through the start,
    /// wherever the start is; along the line the start is on, no parallel to itself.
    #[test]
    fn a_woken_line_gives_parallel_and_perpendicular_through_the_start() {
        let mut sk = Sketch::new();
        let l = sk.add_line(v(0.0, 0.0), v(1.0, 1.0));
        let elsewhere = InferenceContext { tool: SketchTool::Line, anchor: Some(v(5.0, 0.0)), snap_radius: 0.05, woken: vec![l], ..Default::default() };
        let r = infer(&sk, v(7.0, 2.01), &elsewhere);
        assert_eq!(r.iter().find(|x| x.kind == InferenceKind::Parallel).map(|x| x.constraints.clone()), Some(vec![AutoConstraint::ParallelTo(l)]), "{r:?}");
        let r = infer(&sk, v(3.0, 2.02), &elsewhere);
        assert_eq!(r.iter().find(|x| x.kind == InferenceKind::Perpendicular).map(|x| x.constraints.clone()), Some(vec![AutoConstraint::PerpendicularTo(l)]), "{r:?}");
        // Not woken: neither.
        let asleep = InferenceContext { woken: vec![], ..elsewhere.clone() };
        assert!(!infer(&sk, v(7.0, 2.01), &asleep).iter().any(|x| matches!(x.kind, InferenceKind::Parallel | InferenceKind::Perpendicular)));
        // Started on it (at (0.5, 0.5)): perpendicular, and no parallel along it.
        let on = InferenceContext { anchor: Some(v(0.5, 0.5)), ..elsewhere };
        let r = infer(&sk, v(-0.5, 1.51), &on);
        assert!(r.iter().any(|x| x.kind == InferenceKind::Perpendicular), "{r:?}");
        assert!(!infer(&sk, v(1.5, 1.51), &on).iter().any(|x| x.kind == InferenceKind::Parallel));
    }
}
