//! Sketch editing tools: trim, extend, split, offset, fillet, mirror, linear
//! and circular patterns, transform and use/convert (external geometry).
//!
//! Tools keep constraints consistent where reasonable: new endpoints created
//! at intersections are constrained onto the cutting curve (or share the
//! cutting curve's endpoint), dimensions that no longer make sense on a
//! trimmed curve (length, arc angle, midpoint, symmetric, fix, equal length)
//! are removed, and split pieces stay related (collinear / equal radius).

use crate::constraints::{reflect, ConstraintKind};
use crate::entities::{EntityKind, Geometry};
use crate::geom::{ccw_sweep, norm_angle, BBox, Curve2};
use crate::ids::{ConstraintId, EntityId};
use crate::intersect::intersect;
use crate::nurbs::Nurbs;
use crate::sketch::{Result, Sketch, SketchError};
use glam::DVec2;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::f64::consts::{PI, TAU};

/// What an editing operation changed.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct EditResult {
    pub created: Vec<EntityId>,
    pub removed: Vec<EntityId>,
    pub modified: Vec<EntityId>,
    pub constraints_added: Vec<ConstraintId>,
    pub constraints_removed: Vec<ConstraintId>,
}

/// Rigid / similarity transforms for [`Sketch::transform`].
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Transform {
    Translate(DVec2),
    Rotate { center: DVec2, angle: f64 },
    Scale { center: DVec2, factor: f64 },
}

impl Transform {
    pub fn apply(&self, p: DVec2) -> DVec2 {
        match *self {
            Transform::Translate(d) => p + d,
            Transform::Rotate { center, angle } => center + DVec2::from_angle(angle).rotate(p - center),
            Transform::Scale { center, factor } => center + (p - center) * factor,
        }
    }
    /// Applies the linear part to a vector.
    pub fn apply_vec(&self, v: DVec2) -> DVec2 {
        match *self {
            Transform::Translate(_) => v,
            Transform::Rotate { angle, .. } => DVec2::from_angle(angle).rotate(v),
            Transform::Scale { factor, .. } => v * factor,
        }
    }
    fn scale(&self) -> f64 {
        match *self {
            Transform::Scale { factor, .. } => factor.abs(),
            _ => 1.0,
        }
    }
}

/// (point map, entity map) produced when copying entities.
type CopyMaps = (BTreeMap<EntityId, EntityId>, Vec<(EntityId, EntityId)>);

#[derive(Clone, Copy, Debug)]
struct Cut {
    t: f64,
    p: DVec2,
    other: EntityId,
    at_point: Option<EntityId>,
}

impl EditResult {
    fn merge_removed(&mut self, ids: Vec<EntityId>) {
        for id in ids {
            if !self.removed.contains(&id) {
                self.removed.push(id);
            }
        }
        self.created.retain(|c| !self.removed.contains(c));
    }
}

impl Sketch {
    fn tool_tol(&self) -> f64 {
        1e-9 * self.bbox().size().max_element().max(1.0)
    }

    fn curve_ids(&self) -> Vec<EntityId> {
        self.entities().filter(|e| e.kind() != EntityKind::Point).map(|e| e.id).collect()
    }

    /// Intersections of entity `id` with every other curve (sorted by parameter).
    fn cuts(&self, id: EntityId) -> Vec<Cut> {
        let tol = self.tool_tol();
        let Some(c) = self.curve(id) else { return Vec::new() };
        let bb = c.bbox().expanded(tol);
        let mut out: Vec<Cut> = Vec::new();
        for other in self.curve_ids() {
            if other == id {
                continue;
            }
            let Some(oc) = self.curve(other) else { continue };
            if !oc.bbox().expanded(tol).intersects(&bb) {
                continue;
            }
            for x in intersect(&c, &oc, tol) {
                let at_point = self.endpoints(other).and_then(|(s, e)| {
                    [s, e].into_iter().find(|p| self.point(*p).is_some_and(|q| (q - x.point).length() <= 4.0 * tol))
                });
                out.push(Cut { t: x.t1, p: x.point, other, at_point });
            }
        }
        out.sort_by(|a, b| a.t.total_cmp(&b.t));
        let mut dedup: Vec<Cut> = Vec::new();
        for x in out {
            if let Some(last) = dedup.last()
                && (last.p - x.p).length() <= 4.0 * tol {
                    continue;
                }
            dedup.push(x);
        }
        dedup
    }

    fn add_c(&mut self, res: &mut EditResult, k: ConstraintKind) {
        if let Ok(id) = self.add_constraint(k) {
            res.constraints_added.push(id);
        }
    }

    /// Point entity at a cut: the cutting curve's endpoint if it is there,
    /// otherwise a new point constrained onto the cutting curve.
    fn point_at_cut(&mut self, cut: &Cut, res: &mut EditResult) -> EntityId {
        if let Some(p) = cut.at_point {
            return p;
        }
        let p = self.add_point(cut.p);
        res.created.push(p);
        self.add_c(res, ConstraintKind::Coincident { a: p, b: cut.other });
        p
    }

    /// Removes constraints that no longer make sense after an entity was
    /// shortened/extended.
    fn drop_shape_constraints(&mut self, id: EntityId, res: &mut EditResult) {
        let is_line = self.kind(id) == Some(EntityKind::Line);
        let ids: Vec<ConstraintId> = self
            .constraints()
            .filter(|c| c.kind.entities().contains(&id))
            .filter(|c| match c.kind {
                ConstraintKind::Length { .. }
                | ConstraintKind::ArcAngle { .. }
                | ConstraintKind::Midpoint { .. }
                | ConstraintKind::Symmetric { .. }
                | ConstraintKind::Fix { .. } => true,
                ConstraintKind::Equal { .. } => is_line,
                _ => false,
            })
            .map(|c| c.id)
            .collect();
        for cid in ids {
            if self.remove_constraint(cid).is_ok() {
                res.constraints_removed.push(cid);
            }
        }
    }

    /// Removes the given points if no entity references them anymore.
    fn remove_orphans(&mut self, pts: &[EntityId], res: &mut EditResult) {
        for p in pts {
            if self.kind(*p) == Some(EntityKind::Point) && self.users_of_point(*p).is_empty() {
                // keep standalone points that the user created on purpose only
                // if they carry no relation to the removed geometry: a removed
                // curve endpoint is always dropped here.
                let before: Vec<ConstraintId> = self.constraints_on(*p);
                let removed = self.remove_entities(&[*p]);
                res.constraints_removed.extend(before);
                res.merge_removed(removed);
            }
        }
    }

    /// Geometry of the part `[t0, t1]` of entity `id` with endpoints `p0`, `p1`.
    fn piece_geometry(&mut self, id: EntityId, t0: f64, t1: f64, p0: EntityId, p1: EntityId, res: &mut EditResult) -> Result<Geometry> {
        let e = self.entity(id).ok_or(SketchError::UnknownEntity(id))?.clone();
        let curve = self.curve(id).ok_or(SketchError::UnknownEntity(id))?;
        Ok(match &e.geometry {
            Geometry::Line { .. } => Geometry::Line { start: p0, end: p1 },
            Geometry::Arc { center, .. } | Geometry::Circle { center, .. } => Geometry::Arc { center: *center, start: p0, end: p1 },
            Geometry::Ellipse { center, major, minor } | Geometry::EllipticalArc { center, major, minor, .. } => {
                Geometry::EllipticalArc { center: *center, major: *major, minor: *minor, start: p0, end: p1 }
            }
            Geometry::Conic { .. } => {
                let Curve2::Nurbs(n) = &curve else { unreachable!() };
                let sub = n.sub_curve(t0, t1);
                let w = sub.weights.clone().unwrap_or(vec![1.0; 3]);
                let wn = w[1] / (w[0] * w[2]).sqrt();
                let ctrl = self.add_point(sub.ctrl[1]);
                res.created.push(ctrl);
                Geometry::Conic { start: p0, control: ctrl, end: p1, rho: wn / (1.0 + wn) }
            }
            Geometry::BSpline { .. } | Geometry::FitSpline { .. } => {
                let Curve2::Nurbs(n) = &curve else { unreachable!() };
                let sub = n.sub_curve(t0, t1);
                let m = sub.ctrl.len();
                let mut control = vec![p0];
                for q in &sub.ctrl[1..m - 1] {
                    let pid = self.add_point(*q);
                    res.created.push(pid);
                    control.push(pid);
                }
                control.push(p1);
                Geometry::BSpline { control, degree: sub.degree, knots: sub.knots, weights: sub.weights }
            }
            Geometry::Point { .. } => return Err(SketchError::InvalidGeometry("points cannot be trimmed".into())),
        })
    }

    /// Replaces entity `id` by its part `[t0, t1]`.
    fn apply_interval(&mut self, id: EntityId, t0: f64, t1: f64, p0: EntityId, p1: EntityId, res: &mut EditResult) -> Result<()> {
        let old_refs = self.entity(id).ok_or(SketchError::UnknownEntity(id))?.point_refs();
        let g = self.piece_geometry(id, t0, t1, p0, p1, res)?;
        self.replace_geometry(id, g);
        res.modified.push(id);
        self.drop_shape_constraints(id, res);
        self.remove_orphans(&old_refs, res);
        Ok(())
    }

    /// Creates a new entity for the part `[t0, t1]` of `id`.
    fn new_piece(&mut self, id: EntityId, t0: f64, t1: f64, p0: EntityId, p1: EntityId, res: &mut EditResult) -> Result<EntityId> {
        let construction = self.entity(id).ok_or(SketchError::UnknownEntity(id))?.construction;
        let g = self.piece_geometry(id, t0, t1, p0, p1, res)?;
        let nid = self.add_entity(g, construction, false)?;
        res.created.push(nid);
        Ok(nid)
    }

    fn relate_pieces(&mut self, a: EntityId, b: EntityId, res: &mut EditResult) {
        match self.kind(a) {
            Some(EntityKind::Line) => self.add_c(res, ConstraintKind::Collinear { a, b }),
            Some(EntityKind::Arc) | Some(EntityKind::EllipticalArc) => self.add_c(res, ConstraintKind::Equal { a, b }),
            _ => {}
        }
    }

    /// The parameter interval [`Sketch::trim`] would remove at `pick` (for
    /// periodic curves `t1` may pass the period end): the Trim tool's hover
    /// highlights only that piece (live, qa5 sketch tools).
    pub fn trim_span(&self, id: EntityId, pick: DVec2) -> Option<(f64, f64)> {
        let c = self.curve(id)?;
        let tp = c.closest(pick).0;
        let (a, b) = c.domain();
        let cuts = self.cuts(id);
        if c.is_periodic() {
            let period = b - a;
            let mut ts: Vec<f64> = cuts.iter().map(|x| a + (x.t - a).rem_euclid(period)).collect();
            ts.sort_by(f64::total_cmp);
            if ts.len() < 2 {
                return Some((a, b));
            }
            let lo = ts.iter().rev().find(|t| **t < tp).copied().unwrap_or(*ts.last()?);
            let hi = ts.iter().find(|t| **t > tp).copied().unwrap_or(ts[0]);
            return Some(if lo < hi { (lo, hi) } else { (lo, hi + period) });
        }
        let eps = 1e-9 * (b - a);
        let inner: Vec<f64> = cuts.iter().map(|x| x.t).filter(|t| *t > a + eps && *t < b - eps).collect();
        let lo = inner.iter().rev().find(|t| **t < tp).copied().unwrap_or(a);
        let hi = inner.iter().find(|t| **t > tp).copied().unwrap_or(b);
        Some((lo, hi))
    }

    /// Trim: removes the piece of `id` between the intersections around the
    /// picked point (deletes the whole entity if it has no intersections).
    pub fn trim(&mut self, id: EntityId, pick: DVec2) -> Result<EditResult> {
        let c = self.curve(id).ok_or(SketchError::UnknownEntity(id))?;
        let mut res = EditResult::default();
        let tp = c.closest(pick).0;
        let cuts = self.cuts(id);
        let (a, b) = c.domain();
        if c.is_periodic() {
            let period = b - a;
            let mut cs: Vec<Cut> = cuts.into_iter().map(|mut x| {
                x.t = a + (x.t - a).rem_euclid(period);
                x
            }).collect();
            cs.sort_by(|x, y| x.t.total_cmp(&y.t));
            if cs.len() < 2 {
                let removed = self.remove_entities(&[id]);
                res.merge_removed(removed);
                return Ok(res);
            }
            let lo = cs.iter().rev().find(|x| x.t < tp).copied().unwrap_or(*cs.last().unwrap());
            let hi = cs.iter().find(|x| x.t > tp).copied().unwrap_or(cs[0]);
            let p_hi = self.point_at_cut(&hi, &mut res);
            let p_lo = self.point_at_cut(&lo, &mut res);
            let t1 = if lo.t > hi.t { lo.t } else { lo.t + period };
            self.apply_interval(id, hi.t, t1, p_hi, p_lo, &mut res)?;
            return Ok(res);
        }
        let eps = 1e-9 * (b - a);
        let (s, e) = self.endpoints(id).ok_or_else(|| SketchError::InvalidGeometry("curve has no endpoints".into()))?;
        let inner: Vec<Cut> = cuts.into_iter().filter(|x| x.t > a + eps && x.t < b - eps).collect();
        let lo = inner.iter().rev().find(|x| x.t < tp).copied();
        let hi = inner.iter().find(|x| x.t > tp).copied();
        match (lo, hi) {
            (None, None) => {
                let removed = self.remove_entities(&[id]);
                res.merge_removed(removed);
            }
            (None, Some(h)) => {
                let p = self.point_at_cut(&h, &mut res);
                self.apply_interval(id, h.t, b, p, e, &mut res)?;
            }
            (Some(l), None) => {
                let p = self.point_at_cut(&l, &mut res);
                self.apply_interval(id, a, l.t, s, p, &mut res)?;
            }
            (Some(l), Some(h)) => {
                let pl = self.point_at_cut(&l, &mut res);
                let ph = self.point_at_cut(&h, &mut res);
                let nid = self.new_piece(id, h.t, b, ph, e, &mut res)?;
                self.apply_interval(id, a, l.t, s, pl, &mut res)?;
                self.relate_pieces(id, nid, &mut res);
            }
        }
        Ok(res)
    }

    /// Splits an open curve at the point closest to `at`. Both pieces share
    /// the new point.
    pub fn split(&mut self, id: EntityId, at: DVec2) -> Result<EditResult> {
        let c = self.curve(id).ok_or(SketchError::UnknownEntity(id))?;
        if c.is_periodic() {
            return Err(SketchError::OperationFailed("closed curves must be trimmed or split at two points".into()));
        }
        let (a, b) = c.domain();
        let (t, q, _) = c.closest(at);
        if t <= a + 1e-9 * (b - a) || t >= b - 1e-9 * (b - a) {
            return Err(SketchError::OperationFailed("split point is at an end of the curve".into()));
        }
        let (s, e) = self.endpoints(id).unwrap();
        let mut res = EditResult::default();
        let p = self.add_point(q);
        res.created.push(p);
        let nid = self.new_piece(id, t, b, p, e, &mut res)?;
        let old_refs = self.entity(id).unwrap().point_refs();
        let g = self.piece_geometry(id, a, t, s, p, &mut res)?;
        self.replace_geometry(id, g);
        res.modified.push(id);
        // keep only length-type constraints dropped
        self.drop_shape_constraints(id, &mut res);
        self.remove_orphans(&old_refs, &mut res);
        self.relate_pieces(id, nid, &mut res);
        Ok(res)
    }

    /// Extends a line or arc at the end nearest to `pick` up to the next
    /// intersection with another curve.
    pub fn extend(&mut self, id: EntityId, pick: DVec2) -> Result<EditResult> {
        let tol = self.tool_tol();
        let e = self.entity(id).ok_or(SketchError::UnknownEntity(id))?.clone();
        let (s, en) = e.endpoints().ok_or_else(|| SketchError::OperationFailed("cannot extend a closed curve".into()))?;
        let (ps, pe) = (self.point(s).unwrap(), self.point(en).unwrap());
        let at_end = (pick - pe).length() <= (pick - ps).length();
        let moving = if at_end { en } else { s };
        let extent = self.bbox().size().max_element().max(1.0) * 100.0;
        let mut best: Option<(f64, DVec2, EntityId, Option<EntityId>)> = None;
        let mut consider = |sk: &Sketch, probe: &Curve2, score: &dyn Fn(f64, DVec2) -> Option<f64>| {
            for other in sk.curve_ids() {
                if other == id {
                    continue;
                }
                let Some(oc) = sk.curve(other) else { continue };
                for x in intersect(probe, &oc, tol) {
                    if let Some(sc) = score(x.t1, x.point)
                        && best.is_none_or(|b| sc < b.0) {
                            let at_point = sk.endpoints(other).and_then(|(a, b)| {
                                [a, b].into_iter().find(|p| sk.point(*p).is_some_and(|q| (q - x.point).length() <= 4.0 * tol))
                            });
                            best = Some((sc, x.point, other, at_point));
                        }
                }
            }
        };
        match &e.geometry {
            Geometry::Line { .. } => {
                let (from, dir) = if at_end { (pe, (pe - ps).normalize()) } else { (ps, (ps - pe).normalize()) };
                let probe = Curve2::Line { a: from, b: from + dir * extent };
                consider(self, &probe, &|t, _| (t * extent > tol * 10.0).then_some(t));
            }
            Geometry::Arc { .. } => {
                let Some(Curve2::Arc { center, radius, start_angle, end_angle }) = self.curve(id) else { unreachable!() };
                let probe = Curve2::Circle { center, radius };
                let eps = 10.0 * tol / radius;
                consider(self, &probe, &|t, _| {
                    let d = if at_end { norm_angle(t - end_angle) } else { norm_angle(start_angle - t) };
                    let max_ext = TAU - (end_angle - start_angle);
                    (d > eps && d < max_ext - eps).then_some(d)
                });
            }
            _ => return Err(SketchError::OperationFailed("extend supports lines and arcs".into())),
        }
        let (_, p, other, at_point) = best.ok_or_else(|| SketchError::OperationFailed("nothing to extend to".into()))?;
        let mut res = EditResult::default();
        let cut = Cut { t: 0.0, p, other, at_point };
        let np = self.point_at_cut(&cut, &mut res);
        if let Some(ent) = self.entity_mut(id) {
            ent.replace_point(moving, np);
        }
        res.modified.push(id);
        self.drop_shape_constraints(id, &mut res);
        self.remove_orphans(&[moving], &mut res);
        Ok(res)
    }

    /// Sketch fillet between two lines/arcs that share an endpoint. Both
    /// curves are trimmed back to the tangent points and a tangent arc with
    /// a radius dimension is inserted.
    pub fn fillet(&mut self, a: EntityId, b: EntityId, radius: f64) -> Result<EditResult> {
        if radius <= 0.0 {
            return Err(SketchError::OperationFailed("fillet radius must be positive".into()));
        }
        let tol = self.tool_tol();
        let (ea, eb) = (self.endpoints(a), self.endpoints(b));
        let ((sa, fa), (sb, fb)) = match (ea, eb) {
            (Some(x), Some(y)) => (x, y),
            _ => return Err(SketchError::OperationFailed("fillet needs two open curves".into())),
        };
        // find the shared corner (same point or coincident positions)
        let mut corner: Option<(EntityId, EntityId)> = None;
        for pa in [sa, fa] {
            for pb in [sb, fb] {
                let same = pa == pb || (self.point(pa).unwrap() - self.point(pb).unwrap()).length() <= 4.0 * tol;
                if same && corner.is_none() {
                    corner = Some((pa, pb));
                }
            }
        }
        let (ca, cb) = corner.ok_or_else(|| SketchError::OperationFailed("curves do not share an endpoint".into()))?;
        let v = self.point(ca).unwrap();
        let (cva, cvb) = (self.curve(a).unwrap(), self.curve(b).unwrap());
        let offsets = |c: &Curve2| -> Vec<Curve2> {
            match c {
                Curve2::Line { a, b } => {
                    let d = (*b - *a).normalize();
                    let n = d.perp() * radius;
                    let far = 1e4 * ((*b - *a).length() + radius);
                    [n, -n].iter().map(|o| Curve2::Line { a: *a + *o - d * far, b: *b + *o + d * far }).collect()
                }
                Curve2::Arc { center, radius: r, .. } => {
                    let mut v = vec![Curve2::Circle { center: *center, radius: r + radius }];
                    if *r - radius > 1e-12 {
                        v.push(Curve2::Circle { center: *center, radius: r - radius });
                    }
                    v
                }
                _ => Vec::new(),
            }
        };
        let (oa, ob) = (offsets(&cva), offsets(&cvb));
        if oa.is_empty() || ob.is_empty() {
            return Err(SketchError::OperationFailed("fillet supports lines and arcs".into()));
        }
        let foot = |c: &Curve2, p: DVec2| -> (f64, DVec2, bool) {
            match c {
                Curve2::Line { a, b } => {
                    let d = *b - *a;
                    let t = (p - *a).dot(d) / d.length_squared();
                    (t, *a + d * t, (-1e-9..=1.0 + 1e-9).contains(&t))
                }
                Curve2::Arc { center, radius: r, start_angle, end_angle } => {
                    let ang = (p - *center).to_angle();
                    let q = *center + (p - *center).normalize_or(DVec2::X) * *r;
                    let inr = crate::geom::angle_in_range(ang, *start_angle, end_angle - start_angle, 1e-9);
                    (inr.unwrap_or(ang), q, inr.is_some())
                }
                _ => (0.0, p, false),
            }
        };
        let mut best: Option<(f64, DVec2, DVec2, DVec2)> = None;
        for x in &oa {
            for y in &ob {
                for ip in intersect(x, y, tol) {
                    let center = ip.point;
                    let (_, ta, oka) = foot(&cva, center);
                    let (_, tb, okb) = foot(&cvb, center);
                    if !oka || !okb {
                        continue;
                    }
                    let score = (center - v).length();
                    if best.is_none_or(|bb| score < bb.0) {
                        best = Some((score, center, ta, tb));
                    }
                }
            }
        }
        let (_, center, ta, tb) = best.ok_or_else(|| SketchError::OperationFailed("fillet radius too large".into()))?;
        let mut res = EditResult::default();
        // Length dimensions of trimmed lines are kept by re-targeting them to
        // the "virtual sharp" (the old corner point, kept as a construction
        // point on both lines), so the design intent survives the fillet.
        let mut retargeted = false;
        for (id, corner) in [(a, ca), (b, cb)] {
            if self.kind(id) != Some(EntityKind::Line) {
                continue;
            }
            let Some((s, e)) = self.endpoints(id) else { continue };
            let far = if s == corner { e } else { s };
            let lens: Vec<(ConstraintId, f64)> = self
                .constraints()
                .filter_map(|c| match c.kind {
                    ConstraintKind::Length { curve, value } if curve == id => Some((c.id, value)),
                    _ => None,
                })
                .collect();
            for (cid, value) in lens {
                if self.retarget_constraint(cid, ConstraintKind::Distance { a: far, b: ca, value, mode: crate::constraints::DistanceMode::Center }).is_ok() {
                    retargeted = true;
                }
            }
        }
        let pa = self.add_point(ta);
        let pb = self.add_point(tb);
        let pc = self.add_point(center);
        res.created.extend([pa, pb, pc]);
        for (id, old, new) in [(a, ca, pa), (b, cb, pb)] {
            if let Some(ent) = self.entity_mut(id) {
                ent.replace_point(old, new);
            }
            res.modified.push(id);
            self.drop_shape_constraints(id, &mut res);
        }
        // fillet arc on the short side
        let (aa, ab) = ((ta - center).to_angle(), (tb - center).to_angle());
        let arc = if ccw_sweep(aa, ab) <= PI { self.add_arc_between(pc, pa, pb)? } else { self.add_arc_between(pc, pb, pa)? };
        res.created.push(arc);
        self.add_c(&mut res, ConstraintKind::Tangent { a, b: arc });
        self.add_c(&mut res, ConstraintKind::Tangent { a: b, b: arc });
        self.add_c(&mut res, ConstraintKind::Radius { curve: arc, value: radius });
        let mut olds: Vec<EntityId> = if ca == cb { vec![ca] } else { vec![ca, cb] };
        // Keep the corner as a virtual sharp when dimensions or constraints
        // still refer to it.
        let keep_sharp = retargeted || self.constraints().any(|c| c.kind.entities().contains(&ca));
        if keep_sharp && self.users_of_point(ca).is_empty() {
            olds.retain(|p| *p != ca);
            let _ = self.set_construction(ca, true);
            for id in [a, b] {
                if matches!(self.kind(id), Some(EntityKind::Line)) {
                    self.add_c(&mut res, ConstraintKind::Coincident { a: ca, b: id });
                }
            }
            res.modified.push(ca);
        }
        self.remove_orphans(&olds, &mut res);
        Ok(res)
    }

    /// Fillet at a corner point shared by exactly two curves.
    pub fn fillet_corner(&mut self, point: EntityId, radius: f64) -> Result<EditResult> {
        let users: Vec<EntityId> = self
            .entities()
            .filter(|e| e.endpoints().is_some_and(|(s, en)| s == point || en == point))
            .map(|e| e.id)
            .collect();
        if users.len() != 2 {
            return Err(SketchError::OperationFailed("corner must join exactly two curves".into()));
        }
        self.fillet(users[0], users[1], radius)
    }

    /// Sketch chamfer between two lines sharing a corner: both lines are cut
    /// back by `d1` / `d2` from the corner and joined by a new line. The
    /// corner stays as a construction "virtual sharp" carrying the two
    /// distance dimensions.
    pub fn chamfer(&mut self, a: EntityId, b: EntityId, d1: f64, d2: f64) -> Result<EditResult> {
        if d1 <= 0.0 || d2 <= 0.0 {
            return Err(SketchError::OperationFailed("chamfer distances must be positive".into()));
        }
        if self.kind(a) != Some(EntityKind::Line) || self.kind(b) != Some(EntityKind::Line) {
            return Err(SketchError::OperationFailed("sketch chamfer needs two lines".into()));
        }
        let ((sa, fa), (sb, fb)) = (self.endpoints(a).unwrap(), self.endpoints(b).unwrap());
        let corner = [sa, fa].into_iter().find(|p| *p == sb || *p == fb).ok_or_else(|| SketchError::OperationFailed("lines do not share a corner".into()))?;
        let far_a = if sa == corner { fa } else { sa };
        let far_b = if sb == corner { fb } else { sb };
        let v = self.point(corner).unwrap();
        let (qa, qb) = (self.point(far_a).unwrap(), self.point(far_b).unwrap());
        let (ua, ub) = ((qa - v).normalize_or_zero(), (qb - v).normalize_or_zero());
        if ua.perp_dot(ub).abs() < 1e-9 {
            return Err(SketchError::OperationFailed("cannot chamfer parallel lines".into()));
        }
        if d1 >= (qa - v).length() - 1e-12 || d2 >= (qb - v).length() - 1e-12 {
            return Err(SketchError::OperationFailed("chamfer is too large for the corner".into()));
        }
        let mut res = EditResult::default();
        let pa = self.add_point(v + ua * d1);
        let pb = self.add_point(v + ub * d2);
        res.created.extend([pa, pb]);
        for (id, new) in [(a, pa), (b, pb)] {
            if let Some(ent) = self.entity_mut(id) {
                ent.replace_point(corner, new);
            }
            res.modified.push(id);
            self.drop_shape_constraints(id, &mut res);
        }
        let line = self.add_line_between(pa, pb)?;
        res.created.push(line);
        // The old corner becomes the virtual sharp.
        if self.users_of_point(corner).is_empty() {
            let _ = self.set_construction(corner, true);
            self.add_c(&mut res, ConstraintKind::Coincident { a: corner, b: a });
            self.add_c(&mut res, ConstraintKind::Coincident { a: corner, b });
            res.modified.push(corner);
        }
        self.add_c(&mut res, ConstraintKind::Distance { a: corner, b: pa, value: d1, mode: crate::constraints::DistanceMode::Center });
        self.add_c(&mut res, ConstraintKind::Distance { a: corner, b: pb, value: d2, mode: crate::constraints::DistanceMode::Center });
        Ok(res)
    }

    /// Chamfer at a corner point shared by exactly two lines.
    pub fn chamfer_corner(&mut self, point: EntityId, d1: f64, d2: f64) -> Result<EditResult> {
        let users: Vec<EntityId> = self
            .entities()
            .filter(|e| e.endpoints().is_some_and(|(s, en)| s == point || en == point))
            .map(|e| e.id)
            .collect();
        if users.len() != 2 {
            return Err(SketchError::OperationFailed("corner must join exactly two lines".into()));
        }
        self.chamfer(users[0], users[1], d1, d2)
    }

    /// Copies entities, mapping every defining point through `f` (shared
    /// points stay shared). Returns the point map and created entities.
    fn copy_entities(
        &mut self,
        ids: &[EntityId],
        f: &dyn Fn(DVec2) -> DVec2,
        vecf: &dyn Fn(DVec2) -> DVec2,
        reverse_arcs: bool,
        keep_point: &dyn Fn(DVec2) -> bool,
        res: &mut EditResult,
    ) -> Result<CopyMaps> {
        let mut pmap: BTreeMap<EntityId, EntityId> = BTreeMap::new();
        let mut emap: Vec<(EntityId, EntityId)> = Vec::new();
        let mut map_pt = |sk: &mut Sketch, p: EntityId, res: &mut EditResult| -> EntityId {
            if let Some(q) = pmap.get(&p) {
                return *q;
            }
            let pos = sk.point(p).unwrap();
            let q = if keep_point(pos) {
                p
            } else {
                let q = sk.add_point(f(pos));
                res.created.push(q);
                q
            };
            pmap.insert(p, q);
            q
        };
        for &id in ids {
            let e = self.entity(id).ok_or(SketchError::UnknownEntity(id))?.clone();
            let g = match &e.geometry {
                Geometry::Point { .. } => {
                    let q = map_pt(self, id, res);
                    emap.push((id, q));
                    continue;
                }
                Geometry::Line { start, end } => Geometry::Line { start: map_pt(self, *start, res), end: map_pt(self, *end, res) },
                Geometry::Circle { center, radius } => {
                    let s = vecf(DVec2::new(*radius, 0.0)).length();
                    Geometry::Circle { center: map_pt(self, *center, res), radius: s }
                }
                Geometry::Arc { center, start, end } => {
                    let c = map_pt(self, *center, res);
                    let (s, en) = (map_pt(self, *start, res), map_pt(self, *end, res));
                    if reverse_arcs { Geometry::Arc { center: c, start: en, end: s } } else { Geometry::Arc { center: c, start: s, end: en } }
                }
                Geometry::Ellipse { center, major, minor } => {
                    let m = vecf(*major);
                    Geometry::Ellipse { center: map_pt(self, *center, res), major: m, minor: minor * m.length() / major.length().max(1e-300) }
                }
                Geometry::EllipticalArc { center, major, minor, start, end } => {
                    let m = vecf(*major);
                    let c = map_pt(self, *center, res);
                    let (s, en) = (map_pt(self, *start, res), map_pt(self, *end, res));
                    let minor = minor * m.length() / major.length().max(1e-300);
                    if reverse_arcs {
                        Geometry::EllipticalArc { center: c, major: m, minor, start: en, end: s }
                    } else {
                        Geometry::EllipticalArc { center: c, major: m, minor, start: s, end: en }
                    }
                }
                Geometry::BSpline { control, degree, knots, weights } => Geometry::BSpline {
                    control: control.iter().map(|p| map_pt(self, *p, res)).collect(),
                    degree: *degree,
                    knots: knots.clone(),
                    weights: weights.clone(),
                },
                Geometry::FitSpline { points, handles } => Geometry::FitSpline {
                    points: points.iter().map(|p| map_pt(self, *p, res)).collect(),
                    handles: handles.map(|(a, b)| (map_pt(self, a, res), map_pt(self, b, res))),
                },
                Geometry::Conic { start, control, end, rho } => Geometry::Conic {
                    start: map_pt(self, *start, res),
                    control: map_pt(self, *control, res),
                    end: map_pt(self, *end, res),
                    rho: *rho,
                },
            };
            let nid = self.add_entity(g, e.construction, false)?;
            res.created.push(nid);
            emap.push((id, nid));
        }
        Ok((pmap, emap))
    }

    /// Mirrors entities about line `axis`, adding symmetric constraints
    /// between corresponding points (and equal constraints for circles and
    /// ellipses). Points on the axis are reused.
    pub fn mirror(&mut self, ids: &[EntityId], axis: EntityId) -> Result<EditResult> {
        let (a, b) = self.line_pts(axis).ok_or_else(|| SketchError::OperationFailed("mirror axis must be a line".into()))?;
        let tol = self.tool_tol();
        let d = (b - a).normalize_or_zero();
        if d == DVec2::ZERO {
            return Err(SketchError::OperationFailed("degenerate mirror axis".into()));
        }
        let ids: Vec<EntityId> = ids.iter().copied().filter(|i| *i != axis).collect();
        let mut res = EditResult::default();
        let f = move |p: DVec2| reflect(p, a, b);
        let vf = move |v: DVec2| d * (2.0 * v.dot(d)) - v;
        let on_axis = move |p: DVec2| (p - a).perp_dot(d).abs() <= 4.0 * tol;
        let (pmap, emap) = self.copy_entities(&ids, &f, &vf, true, &on_axis, &mut res)?;
        for (p, q) in &pmap {
            if p != q {
                self.add_c(&mut res, ConstraintKind::Symmetric { a: *p, b: *q, axis });
            }
        }
        for (src, dst) in emap {
            if matches!(self.kind(src), Some(EntityKind::Circle | EntityKind::Ellipse | EntityKind::EllipticalArc)) {
                self.add_c(&mut res, ConstraintKind::Equal { a: src, b: dst });
            }
        }
        Ok(res)
    }

    /// Linear pattern: `count - 1` copies, each offset by `step`.
    pub fn linear_pattern(&mut self, ids: &[EntityId], step: DVec2, count: usize) -> Result<EditResult> {
        let mut res = EditResult::default();
        for g in self.linear_pattern_groups(ids, step, count)? {
            res.created.extend(g);
        }
        Ok(res)
    }

    /// Linear pattern returning the created entities of each instance.
    pub fn linear_pattern_groups(&mut self, ids: &[EntityId], step: DVec2, count: usize) -> Result<Vec<Vec<EntityId>>> {
        let mut out = Vec::new();
        for k in 1..count {
            let off = step * k as f64;
            let mut res = EditResult::default();
            self.copy_entities(ids, &move |p| p + off, &|v| v, false, &|_| false, &mut res)?;
            out.push(res.created);
        }
        Ok(out)
    }

    /// Circular pattern: `count` instances spread over `total_angle` (use 2pi
    /// for a full circle) around `center`.
    pub fn circular_pattern(&mut self, ids: &[EntityId], center: DVec2, count: usize, total_angle: f64) -> Result<EditResult> {
        let mut res = EditResult::default();
        for g in self.circular_pattern_groups(ids, center, count, total_angle)? {
            res.created.extend(g);
        }
        Ok(res)
    }

    /// Circular pattern returning the created entities of each instance.
    pub fn circular_pattern_groups(&mut self, ids: &[EntityId], center: DVec2, count: usize, total_angle: f64) -> Result<Vec<Vec<EntityId>>> {
        self.circular_pattern_groups_impl(ids, center, count, total_angle, false)
    }

    /// Circular pattern whose instances remain linked to their seeds. The
    /// shared construction center belongs to the first group for deletion
    /// and pattern editing. It can be constrained or dragged like any point.
    pub fn circular_pattern_linked_groups(&mut self, ids: &[EntityId], center: DVec2, count: usize, total_angle: f64) -> Result<Vec<Vec<EntityId>>> {
        self.circular_pattern_groups_impl(ids, center, count, total_angle, true)
    }

    fn circular_pattern_groups_impl(&mut self, ids: &[EntityId], center: DVec2, count: usize, total_angle: f64, linked: bool) -> Result<Vec<Vec<EntityId>>> {
        let mut out = Vec::new();
        if count < 2 || ids.is_empty() {
            return Ok(out);
        }
        // Validate before adding the center or making any copies.
        for id in ids {
            self.entity(*id).ok_or(SketchError::UnknownEntity(*id))?;
        }
        let center_id = if linked {
            let id = self.add_point(center);
            self.set_construction(id, true)?;
            Some(id)
        } else { None };
        let full = (total_angle.abs() - TAU).abs() < 1e-9;
        let step = if full { total_angle / count as f64 } else { total_angle / (count - 1) as f64 };
        for k in 1..count {
            let t = Transform::Rotate { center, angle: step * k as f64 };
            let mut res = EditResult::default();
            let (points, entities) = self.copy_entities(ids, &move |p| t.apply(p), &move |v| t.apply_vec(v), false, &|_| false, &mut res)?;
            if let Some(center) = center_id {
                for (seed, inst) in points {
                    self.add_c(&mut res, ConstraintKind::CircularPatternLink { seed, inst, center, angle: step * k as f64 });
                }
                for (seed, inst) in entities {
                    match self.kind(seed) {
                        Some(EntityKind::Circle) => self.add_c(&mut res, ConstraintKind::Equal { a: seed, b: inst }),
                        Some(EntityKind::Ellipse | EntityKind::EllipticalArc) => {
                            self.add_c(&mut res, ConstraintKind::CircularPatternLink { seed, inst, center, angle: step * k as f64 });
                        }
                        _ => {}
                    }
                }
                if k == 1 {
                    res.created.push(center);
                }
            }
            out.push(res.created);
        }
        Ok(out)
    }

    /// Transforms entities in place (external geometry is not moved). Fix
    /// anchors of moved entities are updated.
    pub fn transform(&mut self, ids: &[EntityId], t: Transform) -> Result<EditResult> {
        let mut res = EditResult::default();
        let mut pts: Vec<EntityId> = Vec::new();
        for &id in ids {
            let e = self.entity(id).ok_or(SketchError::UnknownEntity(id))?;
            if e.kind() == EntityKind::Point {
                pts.push(id);
            } else {
                pts.extend(e.point_refs());
            }
        }
        pts.sort();
        pts.dedup();
        for p in &pts {
            let ext = self.entity(*p).is_some_and(|e| e.external);
            if !ext {
                let q = t.apply(self.point(*p).unwrap());
                self.set_point(*p, q)?;
                res.modified.push(*p);
            }
        }
        for &id in ids {
            let Some(e) = self.entity_mut_silent(id) else { continue };
            if e.external {
                continue;
            }
            match &mut e.geometry {
                Geometry::Circle { radius, .. } => *radius *= t.scale(),
                Geometry::Ellipse { major, minor, .. } | Geometry::EllipticalArc { major, minor, .. } => {
                    *major = t.apply_vec(*major);
                    *minor *= t.scale();
                }
                _ => {}
            }
            res.modified.push(id);
        }
        let touched: Vec<EntityId> = res.modified.clone();
        let fixes: Vec<ConstraintId> = self
            .constraints()
            .filter(|c| matches!(c.kind, ConstraintKind::Fix { entity } if touched.contains(&entity) || self.entity(entity).is_some_and(|e| e.point_refs().iter().any(|p| touched.contains(p)))))
            .map(|c| c.id)
            .collect();
        for f in fixes {
            self.recompute_hints(f)?;
        }
        self.touch();
        Ok(res)
    }

    /// "Use" / "Convert": adds fixed external geometry (e.g. projected model
    /// edges). The new entities are regular (non-construction) curves so they
    /// can bound regions.
    pub fn use_external(&mut self, curve: &Curve2) -> Result<EditResult> {
        let mut res = EditResult::default();
        let pt = |sk: &mut Sketch, p: DVec2, res: &mut EditResult| -> EntityId {
            let id = sk.add_entity(Geometry::Point { pos: p }, false, true).unwrap();
            res.created.push(id);
            id
        };
        let g = match curve {
            Curve2::Line { a, b } => Geometry::Line { start: pt(self, *a, &mut res), end: pt(self, *b, &mut res) },
            Curve2::Circle { center, radius } => Geometry::Circle { center: pt(self, *center, &mut res), radius: *radius },
            Curve2::Arc { center, radius, start_angle, end_angle } => {
                let c = pt(self, *center, &mut res);
                let s = pt(self, *center + DVec2::from_angle(*start_angle) * *radius, &mut res);
                let e = pt(self, *center + DVec2::from_angle(*end_angle) * *radius, &mut res);
                Geometry::Arc { center: c, start: s, end: e }
            }
            Curve2::Ellipse { center, major, minor } => Geometry::Ellipse { center: pt(self, *center, &mut res), major: *major, minor: *minor },
            Curve2::EllipticalArc { center, major, minor, start_param, end_param } => {
                let c = pt(self, *center, &mut res);
                let s = pt(self, curve.point(*start_param), &mut res);
                let e = pt(self, curve.point(*end_param), &mut res);
                Geometry::EllipticalArc { center: c, major: *major, minor: *minor, start: s, end: e }
            }
            Curve2::Nurbs(n) => {
                let control = n.ctrl.iter().map(|p| pt(self, *p, &mut res)).collect();
                Geometry::BSpline { control, degree: n.degree, knots: n.knots.clone(), weights: n.weights.clone() }
            }
        };
        let id = self.add_entity(g, false, true)?;
        res.created.push(id);
        Ok(res)
    }

    /// Adds a fixed external point (projected vertex).
    pub fn use_external_point(&mut self, p: DVec2) -> EntityId {
        self.add_entity(Geometry::Point { pos: p }, false, true).unwrap()
    }

    /// Offsets curves (chains are detected through shared endpoints) by
    /// `distance` to the left of each chain's direction (a lone circle or
    /// ellipse: positive = inward). Outer corners get arcs centered on the
    /// original vertex; inner corners are trimmed to the intersection.
    pub fn offset(&mut self, ids: &[EntityId], distance: f64) -> Result<EditResult> {
        if distance == 0.0 {
            return Err(SketchError::OperationFailed("offset distance must be non-zero".into()));
        }
        let tol = self.tool_tol();
        let mut res = EditResult::default();
        for chain in self.chains(ids) {
            self.offset_chain(&chain.0, chain.1, distance, tol, &mut res, false, &mut Vec::new())?;
        }
        Ok(res)
    }

    /// The Offset tool (live "psb (10-24 j)"): [`Sketch::offset`], except
    /// that the copies get their own points joined by Coincidents, outer
    /// corners are sharp, arcs and circles get their own centres, and the
    /// copies follow their sources: Onshape's OFFSET constraints (one per
    /// further pair, all referring to the first picked curve's pair) and one
    /// DISTANCE dimension on that pair, per chain. The copies' free ends stay
    /// unconstrained (live).
    pub fn offset_tool(&mut self, ids: &[EntityId], distance: f64) -> Result<EditResult> {
        if distance == 0.0 {
            return Err(SketchError::OperationFailed("offset distance must be non-zero".into()));
        }
        let tol = self.tool_tol();
        let mut res = EditResult::default();
        for chain in self.chains(ids) {
            let mut pairs = Vec::new();
            self.offset_chain(&chain.0, chain.1, distance, tol, &mut res, true, &mut pairs)?;
            self.offset_constraints(ids, &pairs, distance.abs(), &mut res);
        }
        Ok(res)
    }

    /// The Offset tool's constraints over its (source, copy) pairs: lines
    /// and circular curves only (other copies are approximations).
    fn offset_constraints(&mut self, picked: &[EntityId], pairs: &[(EntityId, EntityId)], d: f64, res: &mut EditResult) {
        let kinds = |sk: &Self, (a, b): (EntityId, EntityId)| (sk.kind(a), sk.kind(b));
        let pairs: Vec<(EntityId, EntityId)> = pairs
            .iter()
            .copied()
            .filter(|p| match kinds(self, *p) {
                (Some(EntityKind::Line), Some(EntityKind::Line)) => true,
                (Some(a), Some(b)) => a.is_circular() && b.is_circular(),
                _ => false,
            })
            .collect();
        let Some(first) = picked.iter().find_map(|id| pairs.iter().find(|p| p.0 == *id)).copied() else { return };
        // +1 / −1: a line copy left of its source's direction, a circular copy outside it.
        let side = |sk: &Self, (src, copy): (EntityId, EntityId)| -> f64 {
            let s = match (sk.curve(src), sk.curve(copy)) {
                (Some(Curve2::Line { a, b }), Some(c)) => {
                    let (t0, t1) = c.domain();
                    (b - a).perp_dot(c.point((t0 + t1) * 0.5) - a)
                }
                _ => match (sk.radius_of(src), sk.radius_of(copy)) {
                    (Some(rs), Some(rc)) => rc - rs,
                    _ => 1.0,
                },
            };
            if s < 0.0 { -1.0 } else { 1.0 }
        };
        let s1 = side(self, first);
        let others: Vec<(EntityId, EntityId)> = pairs.iter().copied().filter(|p| *p != first).collect();
        if others.is_empty() {
            self.add_c(res, ConstraintKind::OffsetCurve { master: first.0, curve: first.1, second: None, sides: (s1, s1), first_shape: true });
        }
        for (k, p) in others.iter().enumerate() {
            let s2 = side(self, *p);
            self.add_c(res, ConstraintKind::OffsetCurve { master: first.0, curve: first.1, second: Some(*p), sides: (s1, s2), first_shape: k == 0 });
        }
        self.add_c(res, ConstraintKind::Distance { a: first.0, b: first.1, value: d, mode: crate::constraints::DistanceMode::Minimum });
    }

    /// Slots around connected chains of centerline curves (Onshape Slot on a
    /// chain selection): both sides are offsets of the chain by half the
    /// width, joined at open ends by semicircular caps centered on the chain
    /// ends. Curves not connected to others get their own slot. Returns the
    /// created curves and the caps (for the shared width dimension).
    pub fn slot_chains(&mut self, ids: &[EntityId], width: f64) -> Result<(Vec<EntityId>, Vec<EntityId>)> {
        if width <= 0.0 {
            return Err(SketchError::InvalidGeometry("slot width must be positive".into()));
        }
        let h = width * 0.5;
        let mut curves = Vec::new();
        let mut caps = Vec::new();
        for (items, closed) in self.chains(ids) {
            if items.len() == 1 && !closed {
                let line = self.kind(items[0].0) == Some(EntityKind::Line);
                let s = self.slot_around(items[0].0, width)?;
                if line {
                    // Live: only the end cap is dimensioned (the offsets tie the rest).
                    caps.push(s.curves[1]);
                    curves.extend(s.curves);
                    continue;
                }
                caps.extend(s.curves.iter().copied().filter(|c| self.kind(*c) == Some(EntityKind::Arc) && self.entity(*c).and_then(|e| e.center()).is_some_and(|ctr| self.entity(items[0].0).is_some_and(|x| x.point_refs().contains(&ctr)))));
                curves.extend(s.curves);
                continue;
            }
            let chain: Vec<EntityId> = items.iter().map(|x| x.0).collect();
            let left = self.offset(&chain, h)?;
            let right = self.offset(&chain, -h)?;
            let mut made: Vec<EntityId> = left.created.iter().chain(right.created.iter()).copied().filter(|e| self.kind(*e).is_some_and(|k| k != EntityKind::Point)).collect();
            if !closed {
                // Chain end points in traversal order.
                let (first, ffwd) = items[0];
                let (last, lfwd) = items[items.len() - 1];
                let (fs, fe) = self.endpoints(first).unwrap();
                let (ls, le) = self.endpoints(last).unwrap();
                let start = if ffwd { fs } else { fe };
                let end = if lfwd { le } else { ls };
                for (centre, at_start) in [(start, true), (end, false)] {
                    let c = self.point(centre).unwrap();
                    // the two offset curve ends nearest to this chain end
                    let mut ends: Vec<(f64, EntityId)> = made
                        .iter()
                        .filter_map(|e| self.endpoints(*e))
                        .flat_map(|(a, b)| [a, b])
                        .filter_map(|p| self.point(p).map(|q| (((q - c).length() - h).abs(), p)))
                        .filter(|(d, _)| *d <= h * 1e-6 + 1e-9)
                        .collect();
                    ends.sort_by(|a, b| a.0.total_cmp(&b.0));
                    ends.dedup_by_key(|e| e.1);
                    if ends.len() < 2 {
                        continue;
                    }
                    let (p, q) = (ends[0].1, ends[1].1);
                    // Cap bulges away from the chain: CCW from the right side to the left side at the end.
                    let tangent = {
                        let (id, fwd) = if at_start { items[0] } else { items[items.len() - 1] };
                        let cv = self.curve(id).unwrap();
                        let (a, b) = cv.domain();
                        let t = if at_start == fwd { cv.tangent(a) } else { -cv.tangent(b) };
                        if at_start { t } else { -t }
                    };
                    let out_dir = -tangent.normalize_or_zero();
                    let (pp, qq) = (self.point(p).unwrap(), self.point(q).unwrap());
                    let mid_ccw = {
                        let a0 = (pp - c).to_angle();
                        let sw = crate::geom::ccw_sweep(a0, (qq - c).to_angle());
                        DVec2::from_angle(a0 + sw * 0.5)
                    };
                    let cap = if mid_ccw.dot(out_dir) >= 0.0 { self.add_arc_between(centre, p, q)? } else { self.add_arc_between(centre, q, p)? };
                    made.push(cap);
                    caps.push(cap);
                }
                if caps.len() >= 2 {
                    let n = caps.len();
                    let _ = self.add_constraint(ConstraintKind::Equal { a: caps[n - 2], b: caps[n - 1] });
                }
            }
            // Live Onshape: the picked centerline stays a normal (solid)
            // line, like the single-curve slot (sketch/307–308).
            curves.extend(made);
        }
        Ok((curves, caps))
    }

    /// Groups curves into chains: (items (id, forward), closed).
    fn chains(&self, ids: &[EntityId]) -> Vec<(Vec<(EntityId, bool)>, bool)> {
        let tol = self.tool_tol() * 4.0;
        let mut out = Vec::new();
        let mut open: Vec<EntityId> = Vec::new();
        for &id in ids {
            match self.curve(id) {
                Some(c) if c.is_periodic() => out.push((vec![(id, true)], true)),
                Some(_) => open.push(id),
                None => {}
            }
        }
        let ends: Vec<(DVec2, DVec2)> = open.iter().map(|id| {
            let c = self.curve(*id).unwrap();
            (c.start_point(), c.end_point())
        }).collect();
        let mut used = vec![false; open.len()];
        let find_next = |p: DVec2, used: &[bool]| -> Option<(usize, bool)> {
            for (k, (s, e)) in ends.iter().enumerate() {
                if used[k] {
                    continue;
                }
                if (*s - p).length() <= tol {
                    return Some((k, true));
                }
                if (*e - p).length() <= tol {
                    return Some((k, false));
                }
            }
            None
        };
        for start in 0..open.len() {
            if used[start] {
                continue;
            }
            used[start] = true;
            let mut items: std::collections::VecDeque<(usize, bool)> = std::collections::VecDeque::new();
            items.push_back((start, true));
            // extend forward
            let mut tail = ends[start].1;
            while let Some((k, fwd)) = find_next(tail, &used) {
                used[k] = true;
                items.push_back((k, fwd));
                tail = if fwd { ends[k].1 } else { ends[k].0 };
            }
            // extend backward
            let mut head = ends[start].0;
            while let Some((k, fwd_at_start)) = find_next(head, &used) {
                used[k] = true;
                // k's start touches head -> traverse k reversed
                let fwd = !fwd_at_start;
                items.push_front((k, fwd));
                head = if fwd { ends[k].0 } else { ends[k].1 };
            }
            let closed = items.len() > 1 && (head - tail).length() <= tol;
            out.push((items.into_iter().map(|(k, f)| (open[k], f)).collect(), closed));
        }
        out
    }

    /// `tool`: the Offset tool's copies (see [`Sketch::offset_tool`]); the
    /// (source, copy) pairs go to `pairs`.
    #[allow(clippy::too_many_arguments)]
    fn offset_chain(&mut self, items: &[(EntityId, bool)], closed: bool, d: f64, tol: f64, res: &mut EditResult, tool: bool, pairs: &mut Vec<(EntityId, EntityId)>) -> Result<()> {
        #[derive(Clone)]
        enum Off {
            Line { a: DVec2, b: DVec2 },
            /// Arc traversed from `s` to `e` around `center` (`ccw` traversal).
            Arc { center_pt: EntityId, center: DVec2, r: f64, s: DVec2, e: DVec2, ccw: bool },
            Fit { pts: Vec<DVec2> },
            /// Conic approximation of a conic's offset: start, control, end, rho.
            Conic { s: DVec2, c: DVec2, e: DVec2, rho: f64 },
            /// Least-squares cubic B-spline fit of the offset (few control points).
            Spline { n: Nurbs },
        }
        // lone closed curves
        if items.len() == 1 {
            let id = items[0].0;
            let c = self.curve(id).unwrap();
            if c.is_periodic() {
                match (&c, &self.entity(id).unwrap().geometry) {
                    (Curve2::Circle { radius, .. }, Geometry::Circle { center, .. }) => {
                        let r = radius - d;
                        if r <= tol {
                            return Err(SketchError::OperationFailed("offset collapses a circle".into()));
                        }
                        let mut center = *center;
                        if tool {
                            // Its own centre: the OFFSET keeps it concentric (live).
                            let c = self.point(center).unwrap();
                            center = self.add_point(c);
                            res.created.push(center);
                        }
                        let nid = self.add_circle_at(center, r)?;
                        res.created.push(nid);
                        pairs.push((id, nid));
                    }
                    _ => {
                        // ellipse: closed interpolating spline through offset samples
                        let pts = self.offset_samples(&c, true, d);
                        let mut ids: Vec<EntityId> = pts.iter().map(|q| self.add_point(*q)).collect();
                        res.created.extend(ids.iter().copied());
                        ids.push(ids[0]);
                        let nid = self.add_entity(Geometry::FitSpline { points: ids, handles: None }, false, false)?;
                        res.created.push(nid);
                    }
                }
                return Ok(());
            }
        }
        let mut offs: Vec<(EntityId, Off, DVec2, DVec2)> = Vec::new(); // (src, geometry, start, end)
        let mut verts: Vec<Option<EntityId>> = Vec::new(); // original vertex point between item k and k+1
        for (k, &(id, fwd)) in items.iter().enumerate() {
            let c = self.curve(id).unwrap();
            let e = self.entity(id).unwrap().clone();
            let off = match (&c, &e.geometry) {
                (Curve2::Line { a, b }, _) => {
                    let (p, q) = if fwd { (*a, *b) } else { (*b, *a) };
                    let n = (q - p).normalize().perp() * d;
                    Off::Line { a: p + n, b: q + n }
                }
                (Curve2::Arc { center, radius, start_angle, end_angle }, Geometry::Arc { center: cp, .. }) => {
                    // left of CCW traversal = toward the center
                    let r = if fwd { radius - d } else { radius + d };
                    if r <= tol {
                        return Err(SketchError::OperationFailed("offset collapses an arc".into()));
                    }
                    let (a0, a1) = if fwd { (*start_angle, *end_angle) } else { (*end_angle, *start_angle) };
                    Off::Arc { center_pt: *cp, center: *center, r, s: *center + DVec2::from_angle(a0) * r, e: *center + DVec2::from_angle(a1) * r, ccw: fwd }
                }
                (_, Geometry::Conic { .. }) => match conic_offset(&c, fwd, d) {
                    Some((s, cc, e, rho)) => Off::Conic { s, c: cc, e, rho },
                    None => match fit_offset(&c, fwd, d) {
                        Some(n) => Off::Spline { n },
                        None => Off::Fit { pts: self.offset_samples(&c, fwd, d) },
                    },
                },
                _ => match fit_offset(&c, fwd, d) {
                    Some(n) => Off::Spline { n },
                    None => Off::Fit { pts: self.offset_samples(&c, fwd, d) },
                },
            };
            let (s, en) = match &off {
                Off::Line { a, b } => (*a, *b),
                Off::Arc { s, e, .. } => (*s, *e),
                Off::Fit { pts } => (pts[0], *pts.last().unwrap()),
                Off::Conic { s, e, .. } => (*s, *e),
                Off::Spline { n } => (n.ctrl[0], *n.ctrl.last().unwrap()),
            };
            offs.push((id, off, s, en));
            // shared original vertex with the next item
            let next = if k + 1 < items.len() { Some(k + 1) } else if closed { Some(0) } else { None };
            verts.push(next.and_then(|nk| {
                let (a_s, a_e) = self.endpoints(id)?;
                let my_end = if fwd { a_e } else { a_s };
                let (nid, nfwd) = items[nk];
                let (b_s, b_e) = self.endpoints(nid)?;
                let their_start = if nfwd { b_s } else { b_e };
                let _ = their_start;
                Some(my_end)
            }));
        }
        let n = offs.len();
        // junction points between consecutive offset curves
        let mut start_pt: Vec<Option<EntityId>> = vec![None; n];
        let mut end_pt: Vec<Option<EntityId>> = vec![None; n];
        let mut joint_arcs: Vec<(usize, EntityId, EntityId, EntityId)> = Vec::new(); // (after k, center, from, to)
        let joints = if closed { n } else { n.saturating_sub(1) };
        for k in 0..joints {
            let k2 = (k + 1) % n;
            let (e_k, s_k2) = (offs[k].3, offs[k2].2);
            if (e_k - s_k2).length() <= 4.0 * tol {
                let p = self.add_point(e_k);
                res.created.push(p);
                end_pt[k] = Some(p);
                start_pt[k2] = Some(if tool { self.joint_twin(p, e_k, res) } else { p });
                continue;
            }
            let vpt = verts[k].unwrap();
            let v = self.point(vpt).unwrap();
            // inner corner: offsets cross near the vertex
            let ext = |o: &Off| -> Option<Curve2> {
                match o {
                    Off::Line { a, b } => {
                        let dd = (*b - *a).normalize();
                        let far = 1e3 * ((*b - *a).length() + d.abs());
                        Some(Curve2::Line { a: *a - dd * far, b: *b + dd * far })
                    }
                    Off::Arc { center, r, .. } => Some(Curve2::Circle { center: *center, radius: *r }),
                    _ => None,
                }
            };
            let mut inner: Option<DVec2> = None;
            if let (Some(c1), Some(c2)) = (ext(&offs[k].1), ext(&offs[k2].1)) {
                let mut best: Option<(f64, DVec2)> = None;
                for x in intersect(&c1, &c2, tol) {
                    let dist = (x.point - v).length();
                    if dist <= 2.0 * d.abs() * (1.0 + 1e-6) + 10.0 * d.abs() && best.is_none_or(|b| dist < b.0) {
                        best = Some((dist, x.point));
                    }
                }
                // inner corner: the chain turns toward the offset side
                if let Some((_, x)) = best {
                    let tan = |sk: &Sketch, (id, fwd): (EntityId, bool), at_end: bool| -> DVec2 {
                        let c = sk.curve(id).unwrap();
                        let (a, b) = c.domain();
                        let t = if at_end == fwd { b } else { a };
                        let d1 = c.tangent(t);
                        (if fwd { d1 } else { -d1 }).normalize_or_zero()
                    };
                    let t1 = tan(self, items[k], true);
                    let t2 = tan(self, items[k2], false);
                    let turn = t1.perp_dot(t2);
                    // The Offset tool's outer corners are sharp too (live).
                    if turn * d > 1e-12 || tool {
                        inner = Some(x);
                    }
                }
            }
            if let Some(x) = inner {
                let p = self.add_point(x);
                res.created.push(p);
                end_pt[k] = Some(p);
                start_pt[k2] = Some(if tool { self.joint_twin(p, x, res) } else { p });
                // update geometry ends
                offs[k].3 = x;
                offs[k2].2 = x;
                if let Off::Line { b, .. } = &mut offs[k].1 {
                    *b = x;
                }
                if let Off::Line { a, .. } = &mut offs[k2].1 {
                    *a = x;
                }
            } else {
                let p1 = self.add_point(e_k);
                let p2 = self.add_point(s_k2);
                res.created.extend([p1, p2]);
                end_pt[k] = Some(p1);
                start_pt[k2] = Some(p2);
                joint_arcs.push((k, vpt, p1, p2));
            }
        }
        // create offset curves
        let mut new_ids: Vec<EntityId> = Vec::new();
        for k in 0..n {
            let sp = match start_pt[k] {
                Some(p) => p,
                None => {
                    let p = self.add_point(offs[k].2);
                    res.created.push(p);
                    p
                }
            };
            let ep = match end_pt[k] {
                Some(p) => p,
                None => {
                    let p = self.add_point(offs[k].3);
                    res.created.push(p);
                    p
                }
            };
            let (src, off) = (offs[k].0, offs[k].1.clone());
            let id = match off {
                Off::Line { .. } => {
                    let id = self.add_line_between(sp, ep)?;
                    res.created.push(id);
                    if !tool {
                        self.add_c(res, ConstraintKind::Parallel { a: src, b: id });
                    }
                    id
                }
                Off::Arc { center_pt, center, ccw, .. } => {
                    // The tool's copy has its own centre (the OFFSET keeps it concentric).
                    let center_pt = if tool {
                        let q = self.add_point(center);
                        res.created.push(q);
                        q
                    } else {
                        center_pt
                    };
                    let id = if ccw { self.add_arc_between(center_pt, sp, ep)? } else { self.add_arc_between(center_pt, ep, sp)? };
                    res.created.push(id);
                    id
                }
                Off::Spline { n } => {
                    let mut control = vec![sp];
                    for q in &n.ctrl[1..n.ctrl.len() - 1] {
                        let p = self.add_point(*q);
                        res.created.push(p);
                        control.push(p);
                    }
                    control.push(ep);
                    let id = self.add_entity(Geometry::BSpline { control, degree: n.degree, knots: n.knots, weights: None }, false, false)?;
                    res.created.push(id);
                    id
                }
                Off::Conic { c, rho, .. } => {
                    let cp = self.add_point(c);
                    res.created.push(cp);
                    let id = self.add_entity(Geometry::Conic { start: sp, control: cp, end: ep, rho }, false, false)?;
                    res.created.push(id);
                    id
                }
                Off::Fit { pts } => {
                    let mut ids = vec![sp];
                    for q in &pts[1..pts.len() - 1] {
                        let p = self.add_point(*q);
                        res.created.push(p);
                        ids.push(p);
                    }
                    ids.push(ep);
                    let id = self.add_entity(Geometry::FitSpline { points: ids, handles: None }, false, false)?;
                    res.created.push(id);
                    id
                }
            };
            new_ids.push(id);
            pairs.push((src, id));
        }
        for (k, vpt, p1, p2) in joint_arcs {
            let v = self.point(vpt).unwrap();
            let (a1, a2) = ((self.point(p1).unwrap() - v).to_angle(), (self.point(p2).unwrap() - v).to_angle());
            let arc = if ccw_sweep(a1, a2) <= PI { self.add_arc_between(vpt, p1, p2)? } else { self.add_arc_between(vpt, p2, p1)? };
            res.created.push(arc);
            if let Some(&prev) = new_ids.get(k) {
                self.add_c(res, ConstraintKind::Tangent { a: prev, b: arc });
            }
            if let Some(&next) = new_ids.get((k + 1) % n) {
                self.add_c(res, ConstraintKind::Tangent { a: next, b: arc });
            }
        }
        Ok(())
    }

    /// A second point where an Offset tool copy meets the next one,
    /// Coincident with `p` (live: the copies' ends are joined that way).
    fn joint_twin(&mut self, p: EntityId, at: DVec2, res: &mut EditResult) -> EntityId {
        let q = self.add_point(at);
        res.created.push(q);
        self.add_c(res, ConstraintKind::Coincident { a: q, b: p });
        q
    }

    /// Points of the offset of `c` for an interpolating spline: as few as
    /// needed for the spline through them to stay within a small tolerance
    /// of the true offset (avoids dense splines when offsetting conics,
    /// ellipses and splines).
    fn offset_samples(&self, c: &Curve2, fwd: bool, d: f64) -> Vec<DVec2> {
        let (a, b) = c.domain();
        let periodic = c.is_periodic();
        let offset_at = |t: f64| -> DVec2 {
            let der = c.ders(t);
            let dir = if fwd { der[1] } else { -der[1] };
            der[0] + dir.normalize_or_zero().perp() * d
        };
        // Arc-length parametrization of the offset curve (samples spread
        // evenly along it, not along the source parameter).
        const DENSE: usize = 256;
        let ts: Vec<f64> = (0..=DENSE).map(|i| a + (b - a) * i as f64 / DENSE as f64).collect();
        let mut acc = vec![0.0];
        let mut prev = offset_at(a);
        for t in &ts[1..] {
            let q = offset_at(*t);
            acc.push(acc.last().unwrap() + (q - prev).length());
            prev = q;
        }
        let total = *acc.last().unwrap();
        let param_at = |s: f64| -> f64 {
            if total <= 1e-300 {
                return a + (b - a) * s;
            }
            let target = s * total;
            let k = acc.partition_point(|x| *x < target).clamp(1, DENSE);
            let (l0, l1) = (acc[k - 1], acc[k]);
            let f = if l1 > l0 { (target - l0) / (l1 - l0) } else { 0.0 };
            ts[k - 1] + (ts[k] - ts[k - 1]) * f
        };
        let sample = |n: usize| -> Vec<DVec2> {
            let mut pts: Vec<DVec2> = (0..=n).map(|i| offset_at(param_at(i as f64 / n as f64))).collect();
            if !fwd {
                pts.reverse();
            }
            if periodic {
                pts.pop();
            }
            pts
        };
        let tol = ((c.size() + d.abs()) * 2e-4).max(1e-9);
        let mut n = if periodic { 6 } else { 3 };
        loop {
            let pts = sample(n);
            if n >= 64 {
                return pts;
            }
            let mut fit = pts.clone();
            if periodic {
                fit.push(fit[0]);
            }
            if let Some(nb) = Nurbs::interpolate(&fit) {
                let cv = Curve2::Nurbs(nb);
                let ok = (0..2 * n).all(|i| cv.distance(offset_at(param_at((i as f64 + 0.5) / (2 * n) as f64))) <= tol);
                if ok {
                    return pts;
                }
            }
            n += (n / 2).max(1);
        }
    }

    /// Bounding box of a set of entities.
    pub fn entities_bbox(&self, ids: &[EntityId]) -> BBox {
        ids.iter().filter_map(|i| self.entity_bbox(*i)).fold(BBox::EMPTY, |a, b| a.union(&b))
    }

}

/// Offset of a conic approximated by a conic: the ends move along their
/// normals, the control point is where the offset end tangents meet and rho
/// puts the shoulder on the true offset. Exact for circular conics; `None`
/// when the approximation deviates from the true offset (the caller then
/// fits a spline). Returns (start, control, end, rho) in traversal order.
fn conic_offset(c: &Curve2, fwd: bool, d: f64) -> Option<(DVec2, DVec2, DVec2, f64)> {
    let (a, b) = c.domain();
    let off = |t: f64| -> (DVec2, DVec2) {
        let der = c.ders(t);
        let tan = der[1].normalize_or_zero();
        let dir = if fwd { tan } else { -tan };
        (der[0] + dir.perp() * d, tan)
    };
    let ((s, ts), (e, te)) = (off(a), off(b));
    let den = ts.perp_dot(te);
    if den.abs() < 1e-9 {
        return None;
    }
    let k = (e - s).perp_dot(te) / den;
    let ctrl = s + ts * k;
    let m = (s + e) * 0.5;
    let (shoulder, _) = off((a + b) * 0.5);
    let mc = ctrl - m;
    let rho = (shoulder - m).dot(mc) / mc.length_squared().max(1e-300);
    if !(0.01..=0.99).contains(&rho) {
        return None;
    }
    let approx = Curve2::Nurbs(Nurbs::conic(s, ctrl, e, rho));
    let tol = ((c.size() + d.abs()) * 2e-4).max(1e-9);
    let ok = (1..32).all(|i| approx.distance(off(a + (b - a) * i as f64 / 32.0).0) <= tol);
    if !ok {
        return None;
    }
    Some(if fwd { (s, ctrl, e, rho) } else { (e, ctrl, s, rho) })
}

/// Offset of an open curve as a least-squares cubic B-spline with as few
/// control points as keep it within tolerance of the true offset. Returned
/// in traversal order.
fn fit_offset(c: &Curve2, fwd: bool, d: f64) -> Option<Nurbs> {
    if c.is_periodic() {
        return None;
    }
    let (a, b) = c.domain();
    let off = |t: f64| -> DVec2 {
        let der = c.ders(t);
        let dir = if fwd { der[1] } else { -der[1] };
        der[0] + dir.normalize_or_zero().perp() * d
    };
    const M: usize = 160;
    let mut pts: Vec<DVec2> = (0..=M).map(|i| off(a + (b - a) * i as f64 / M as f64)).collect();
    if !fwd {
        pts.reverse();
    }
    let tol = ((c.size() + d.abs()) * 2e-4).max(1e-9);
    for n in 4..=24 {
        let Some(fit) = Nurbs::fit_least_squares(&pts, n) else { continue };
        let cv = Curve2::Nurbs(fit.clone());
        if pts.iter().all(|q| cv.distance(*q) <= tol) {
            return Some(fit);
        }
    }
    None
}
