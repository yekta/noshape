//! The [`Sketch`] container: entities, constraints and their management.

use crate::constraints::{Constraint, ConstraintKind};
use crate::entities::{Entity, EntityKind, Geometry};
use crate::geom::{ccw_sweep, BBox, Curve2};
use crate::ids::{ConstraintId, EntityId};
use crate::nurbs::{clamped_uniform_knots, Nurbs};
use crate::solver::Analysis;
use glam::DVec2;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// Errors returned by sketch operations.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum SketchError {
    #[error("unknown entity {0}")]
    UnknownEntity(EntityId),
    #[error("unknown constraint {0}")]
    UnknownConstraint(ConstraintId),
    #[error("invalid constraint: {0}")]
    InvalidConstraint(String),
    #[error("invalid geometry: {0}")]
    InvalidGeometry(String),
    #[error("operation failed: {0}")]
    OperationFailed(String),
}

pub type Result<T, E = SketchError> = std::result::Result<T, E>;

/// Non-persistent solver cache.
#[derive(Clone, Debug, Default)]
pub(crate) struct Cache {
    /// Incremented on every structural change (entities/constraints added,
    /// removed or modified other than by solving).
    pub revision: u64,
    pub analysis: Option<(u64, Analysis)>,
}

/// A 2D sketch: entities + constraints.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Sketch {
    entities: IndexMap<EntityId, Entity>,
    constraints: IndexMap<ConstraintId, Constraint>,
    next_entity: u32,
    next_constraint: u32,
    #[serde(skip)]
    pub(crate) cache: Cache,
}

impl Sketch {
    pub fn new() -> Self {
        Self::default()
    }

    pub(crate) fn touch(&mut self) {
        self.cache.revision = self.cache.revision.wrapping_add(1);
    }

    // ------------------------------------------------------------------
    // queries

    pub fn entity(&self, id: EntityId) -> Option<&Entity> {
        self.entities.get(&id)
    }

    /// Mutable access to an entity. Counts as a structural edit.
    pub fn entity_mut(&mut self, id: EntityId) -> Option<&mut Entity> {
        self.touch();
        self.entities.get_mut(&id)
    }

    pub(crate) fn entity_mut_silent(&mut self, id: EntityId) -> Option<&mut Entity> {
        self.entities.get_mut(&id)
    }

    pub fn entities(&self) -> impl Iterator<Item = &Entity> {
        self.entities.values()
    }

    pub fn entity_count(&self) -> usize {
        self.entities.len()
    }

    pub fn constraint(&self, id: ConstraintId) -> Option<&Constraint> {
        self.constraints.get(&id)
    }

    pub fn constraints(&self) -> impl Iterator<Item = &Constraint> {
        self.constraints.values()
    }

    pub fn constraint_count(&self) -> usize {
        self.constraints.len()
    }

    pub(crate) fn constraint_mut_silent(&mut self, id: ConstraintId) -> Option<&mut Constraint> {
        self.constraints.get_mut(&id)
    }

    pub(crate) fn max_entity_id(&self) -> u32 {
        self.next_entity
    }

    pub fn kind(&self, id: EntityId) -> Option<EntityKind> {
        self.entity(id).map(|e| e.kind())
    }

    /// Position of a point entity.
    pub fn point(&self, id: EntityId) -> Option<DVec2> {
        match self.entities.get(&id)?.geometry {
            Geometry::Point { pos } => Some(pos),
            _ => None,
        }
    }

    /// Moves a point entity (no solve).
    pub fn set_point(&mut self, id: EntityId, p: DVec2) -> Result<()> {
        match self.entities.get_mut(&id).map(|e| &mut e.geometry) {
            Some(Geometry::Point { pos }) => {
                *pos = p;
                Ok(())
            }
            _ => Err(SketchError::UnknownEntity(id)),
        }
    }

    /// Endpoints (start, end) point entities of an open curve.
    pub fn endpoints(&self, id: EntityId) -> Option<(EntityId, EntityId)> {
        self.entity(id)?.endpoints()
    }

    /// Resolves an entity to concrete curve geometry (`None` for points).
    pub fn curve(&self, id: EntityId) -> Option<Curve2> {
        let e = self.entities.get(&id)?;
        let p = |q: EntityId| self.point(q);
        Some(match &e.geometry {
            Geometry::Point { .. } => return None,
            Geometry::Line { start, end } => Curve2::Line { a: p(*start)?, b: p(*end)? },
            Geometry::Circle { center, radius } => Curve2::Circle { center: p(*center)?, radius: *radius },
            Geometry::Arc { center, start, end } => Curve2::arc_from_points(p(*center)?, p(*start)?, p(*end)?),
            Geometry::Ellipse { center, major, minor } => Curve2::Ellipse { center: p(*center)?, major: *major, minor: *minor },
            Geometry::EllipticalArc { center, major, minor, start, end } => {
                let c = p(*center)?;
                let t0 = ellipse_param(c, *major, *minor, p(*start)?);
                let t1 = ellipse_param(c, *major, *minor, p(*end)?);
                Curve2::EllipticalArc { center: c, major: *major, minor: *minor, start_param: t0, end_param: t0 + ccw_sweep(t0, t1) }
            }
            Geometry::BSpline { control, degree, knots, weights } => {
                let ctrl = control.iter().map(|q| p(*q)).collect::<Option<Vec<_>>>()?;
                Curve2::Nurbs(Nurbs { degree: *degree, knots: knots.clone(), ctrl, weights: weights.clone() })
            }
            Geometry::FitSpline { points, handles } => {
                let pts = points.iter().map(|q| p(*q)).collect::<Option<Vec<_>>>()?;
                match handles {
                    Some((h0, h1)) => {
                        Curve2::Nurbs(Nurbs::interpolate_with_handles(&pts, p(*h0)?, p(*h1)?)?)
                    }
                    None => Curve2::Nurbs(Nurbs::interpolate(&pts)?),
                }
            }
            Geometry::Conic { start, control, end, rho } => Curve2::Nurbs(Nurbs::conic(p(*start)?, p(*control)?, p(*end)?, *rho)),
        })
    }

    /// Entities (curves) that reference point `pid`.
    pub fn users_of_point(&self, pid: EntityId) -> Vec<EntityId> {
        self.entities.values().filter(|e| e.point_refs().contains(&pid)).map(|e| e.id).collect()
    }

    /// Constraints that reference entity `id`.
    pub fn constraints_on(&self, id: EntityId) -> Vec<ConstraintId> {
        self.constraints.values().filter(|c| c.kind.entities().contains(&id)).map(|c| c.id).collect()
    }

    /// Bounding box of all geometry.
    pub fn bbox(&self) -> BBox {
        let mut b = BBox::EMPTY;
        for e in self.entities.values() {
            if let Some(bb) = self.entity_bbox(e.id) {
                b = b.union(&bb);
            }
        }
        b
    }

    // ------------------------------------------------------------------
    // entity creation

    fn alloc_entity(&mut self) -> EntityId {
        let id = EntityId(self.next_entity);
        self.next_entity += 1;
        id
    }

    /// Adds an entity with validated references.
    pub fn add_entity(&mut self, geometry: Geometry, construction: bool, external: bool) -> Result<EntityId> {
        let probe = Entity { id: EntityId(u32::MAX), geometry, construction, external, reference_only: false };
        for r in probe.point_refs() {
            if self.kind(r) != Some(EntityKind::Point) {
                return Err(SketchError::InvalidGeometry(format!("{r} is not a point")));
            }
        }
        match &probe.geometry {
            Geometry::BSpline { control, degree, knots, weights } => {
                let nb = Nurbs { degree: *degree, knots: knots.clone(), ctrl: vec![DVec2::ZERO; control.len()], weights: weights.clone() };
                if !nb.is_valid() {
                    return Err(SketchError::InvalidGeometry("invalid B-spline definition".into()));
                }
            }
            Geometry::FitSpline { points, .. } if points.len() < 2 => {
                return Err(SketchError::InvalidGeometry("fit spline needs at least 2 points".into()));
            }
            Geometry::Conic { rho, .. } if !(*rho > 0.0 && *rho < 1.0) => {
                return Err(SketchError::InvalidGeometry("conic rho must be in (0, 1)".into()));
            }
            _ => {}
        }
        let id = self.alloc_entity();
        self.entities.insert(id, Entity { id, ..probe });
        self.touch();
        Ok(id)
    }

    /// Inserts an entity with a caller-chosen id at or above the allocator
    /// range (`id >= next id`). Used for derived geometry with stable ids
    /// (text glyph outlines added after solving). Such entities must not be
    /// referenced by constraints; the solver treats them as frozen.
    pub fn add_entity_with_id(&mut self, id: EntityId, geometry: Geometry, construction: bool, external: bool) -> Result<EntityId> {
        if id.0 < self.next_entity || self.entities.contains_key(&id) {
            return Err(SketchError::InvalidGeometry(format!("entity id {id} is already allocated")));
        }
        let probe = Entity { id, geometry, construction, external, reference_only: false };
        for r in probe.point_refs() {
            if self.kind(r) != Some(EntityKind::Point) {
                return Err(SketchError::InvalidGeometry(format!("{r} is not a point")));
            }
        }
        self.entities.insert(id, probe);
        self.touch();
        Ok(id)
    }

    /// A new sketch holding copies of `ids` (with their defining points) and
    /// the constraints that only involve copied entities (Copy sketch
    /// entities). External geometry is copied as regular geometry.
    pub fn extract(&self, ids: &[EntityId]) -> Sketch {
        let mut keep: Vec<EntityId> = Vec::new();
        for id in ids {
            let Some(e) = self.entity(*id) else { continue };
            for p in e.point_refs() {
                if !keep.contains(&p) {
                    keep.push(p);
                }
            }
            if !keep.contains(id) {
                keep.push(*id);
            }
        }
        let mut out = Sketch::new();
        // Points first so curves can reference them; ids are preserved.
        keep.sort_by_key(|e| (self.kind(*e) != Some(EntityKind::Point), e.0));
        for id in &keep {
            if let Some(e) = self.entity(*id) {
                out.entities.insert(*id, Entity { external: false, reference_only: false, ..e.clone() });
            }
        }
        out.next_entity = keep.iter().map(|e| e.0 + 1).max().unwrap_or(0);
        for c in self.constraints.values() {
            if c.kind.entities().iter().all(|e| keep.contains(e)) {
                out.constraints.insert(c.id, c.clone());
            }
        }
        out.next_constraint = out.constraints.keys().map(|c| c.0 + 1).max().unwrap_or(0);
        out
    }

    /// Adds every entity and constraint of `other` with fresh ids, offset by
    /// `offset` (Paste sketch entities). Returns the new non-point entities
    /// (or the points when only points were pasted).
    pub fn paste(&mut self, other: &Sketch, offset: DVec2) -> Vec<EntityId> {
        let mut map: std::collections::HashMap<EntityId, EntityId> = std::collections::HashMap::new();
        let mut created = Vec::new();
        let mut points = Vec::new();
        for e in other.entities.values().filter(|e| e.kind() == EntityKind::Point) {
            if let Geometry::Point { pos } = e.geometry {
                let id = self.add_point(pos + offset);
                let _ = self.set_construction(id, e.construction);
                map.insert(e.id, id);
                points.push(id);
            }
        }
        for e in other.entities.values().filter(|e| e.kind() != EntityKind::Point) {
            let mut g = e.geometry.clone();
            let mut probe = Entity { id: e.id, geometry: g.clone(), construction: e.construction, external: false, reference_only: false };
            for p in e.point_refs() {
                if let Some(n) = map.get(&p) {
                    probe.replace_point(p, *n);
                }
            }
            g = probe.geometry;
            if let Ok(id) = self.add_entity(g, e.construction, false) {
                map.insert(e.id, id);
                created.push(id);
            }
        }
        for c in other.constraints.values() {
            let mut k = c.kind.clone();
            if k.entities().iter().any(|e| !map.contains_key(e)) {
                continue;
            }
            for (from, to) in &map {
                k.replace_entity(*from, *to);
            }
            if let ConstraintKind::Fix { .. } = k {
                // A fix re-anchors at the pasted position.
                let _ = self.add_constraint(k);
                continue;
            }
            let _ = self.add_constraint_ext(k, c.driving);
        }
        if created.is_empty() { points } else { created }
    }

    pub fn add_point(&mut self, p: DVec2) -> EntityId {
        self.add_entity(Geometry::Point { pos: p }, false, false).unwrap()
    }

    /// Line segment with two new endpoint points. Returns the line id.
    pub fn add_line(&mut self, a: DVec2, b: DVec2) -> EntityId {
        let pa = self.add_point(a);
        let pb = self.add_point(b);
        self.add_entity(Geometry::Line { start: pa, end: pb }, false, false).unwrap()
    }

    /// Line between existing point entities.
    pub fn add_line_between(&mut self, start: EntityId, end: EntityId) -> Result<EntityId> {
        self.add_entity(Geometry::Line { start, end }, false, false)
    }

    pub fn add_circle(&mut self, center: DVec2, radius: f64) -> EntityId {
        let c = self.add_point(center);
        self.add_entity(Geometry::Circle { center: c, radius }, false, false).unwrap()
    }

    pub fn add_circle_at(&mut self, center: EntityId, radius: f64) -> Result<EntityId> {
        self.add_entity(Geometry::Circle { center, radius }, false, false)
    }

    /// CCW arc from `start` to `end` around `center`; `end` is projected onto
    /// the radius defined by `start`.
    pub fn add_arc(&mut self, center: DVec2, start: DVec2, end: DVec2) -> EntityId {
        let r = (start - center).length();
        let end = center + (end - center).normalize_or(DVec2::X) * r;
        let c = self.add_point(center);
        let s = self.add_point(start);
        let e = self.add_point(end);
        self.add_entity(Geometry::Arc { center: c, start: s, end: e }, false, false).unwrap()
    }

    /// CCW arc by center, radius and angles.
    pub fn add_arc_angles(&mut self, center: DVec2, radius: f64, start_angle: f64, end_angle: f64) -> EntityId {
        let s = center + DVec2::from_angle(start_angle) * radius;
        let e = center + DVec2::from_angle(end_angle) * radius;
        self.add_arc(center, s, e)
    }

    pub fn add_arc_between(&mut self, center: EntityId, start: EntityId, end: EntityId) -> Result<EntityId> {
        self.add_entity(Geometry::Arc { center, start, end }, false, false)
    }

    pub fn add_ellipse(&mut self, center: DVec2, major: DVec2, minor: f64) -> EntityId {
        let c = self.add_point(center);
        self.add_entity(Geometry::Ellipse { center: c, major, minor }, false, false).unwrap()
    }

    /// CCW elliptical arc between ellipse parameters `t0` and `t1`.
    pub fn add_elliptical_arc(&mut self, center: DVec2, major: DVec2, minor: f64, t0: f64, t1: f64) -> EntityId {
        let ell = Curve2::Ellipse { center, major, minor };
        let c = self.add_point(center);
        let s = self.add_point(ell.point(t0));
        let e = self.add_point(ell.point(t1));
        self.add_entity(Geometry::EllipticalArc { center: c, major, minor, start: s, end: e }, false, false).unwrap()
    }

    /// Clamped uniform non-rational B-spline with new control points.
    pub fn add_bspline(&mut self, control: &[DVec2], degree: usize) -> Result<EntityId> {
        if control.len() < 2 {
            return Err(SketchError::InvalidGeometry("B-spline needs at least 2 control points".into()));
        }
        let degree = degree.clamp(1, control.len() - 1);
        let ids: Vec<EntityId> = control.iter().map(|p| self.add_point(*p)).collect();
        let knots = clamped_uniform_knots(ids.len(), degree);
        self.add_entity(Geometry::BSpline { control: ids, degree, knots, weights: None }, false, false)
    }

    /// Interpolating spline through new points.
    pub fn add_fit_spline(&mut self, points: &[DVec2]) -> Result<EntityId> {
        if points.len() < 2 {
            return Err(SketchError::InvalidGeometry("fit spline needs at least 2 points".into()));
        }
        let ids: Vec<EntityId> = points.iter().map(|p| self.add_point(*p)).collect();
        self.add_entity(Geometry::FitSpline { points: ids, handles: None }, false, false)
    }

    /// Interpolating spline with tangent handles at both ends (the Spline
    /// tool): the handles start at a third of the natural end derivatives.
    pub fn add_fit_spline_with_handles(&mut self, points: &[DVec2]) -> Result<EntityId> {
        if points.len() < 2 {
            return Err(SketchError::InvalidGeometry("fit spline needs at least 2 points".into()));
        }
        let (hp0, hp1) = Nurbs::natural_handles(points).ok_or_else(|| SketchError::InvalidGeometry("could not interpolate the points".into()))?;
        let ids: Vec<EntityId> = points.iter().map(|p| self.add_point(*p)).collect();
        let h0 = self.add_point(hp0);
        let h1 = self.add_point(hp1);
        self.add_entity(Geometry::FitSpline { points: ids, handles: Some((h0, h1)) }, false, false)
    }

    /// Interpolating spline through `points` whose last point repeats the
    /// first, closed smoothly (periodic seam tangent, stored as handles).
    pub fn add_closed_fit_spline(&mut self, points: &[DVec2]) -> Result<EntityId> {
        let d = Nurbs::periodic_end_derivative(points).ok_or_else(|| SketchError::InvalidGeometry("closed spline needs 3+ distinct points ending at the first".into()))?;
        let n = points.len();
        let total: f64 = points.windows(2).map(|w| (w[1] - w[0]).length()).sum();
        let first = (points[1] - points[0]).length();
        let last = (points[n - 1] - points[n - 2]).length();
        let h0 = points[0] + d * (first / (3.0 * total));
        let h1 = points[n - 1] - d * (last / (3.0 * total));
        let mut ids: Vec<EntityId> = points[..n - 1].iter().map(|p| self.add_point(*p)).collect();
        ids.push(ids[0]);
        let (h0, h1) = (self.add_point(h0), self.add_point(h1));
        self.add_entity(Geometry::FitSpline { points: ids, handles: Some((h0, h1)) }, false, false)
    }

    pub fn add_conic(&mut self, start: DVec2, control: DVec2, end: DVec2, rho: f64) -> Result<EntityId> {
        let s = self.add_point(start);
        let c = self.add_point(control);
        let e = self.add_point(end);
        self.add_entity(Geometry::Conic { start: s, control: c, end: e, rho }, false, false)
    }

    /// Marks an entity (and its defining points) as construction geometry.
    pub fn set_construction(&mut self, id: EntityId, construction: bool) -> Result<()> {
        let e = self.entities.get_mut(&id).ok_or(SketchError::UnknownEntity(id))?;
        e.construction = construction;
        self.touch();
        Ok(())
    }

    /// Marks an entity and its defining points as reference-only (see
    /// [`Entity::reference_only`]).
    pub fn set_reference_only(&mut self, id: EntityId, on: bool) -> Result<()> {
        let refs = self.entity(id).ok_or(SketchError::UnknownEntity(id))?.point_refs();
        for r in refs.iter().copied().chain(std::iter::once(id)) {
            if let Some(e) = self.entities.get_mut(&r) {
                e.reference_only = on;
            }
        }
        self.touch();
        Ok(())
    }

    /// Marks an entity and its defining points as external (fixed).
    pub fn set_external(&mut self, id: EntityId, external: bool) -> Result<()> {
        let refs = self.entity(id).ok_or(SketchError::UnknownEntity(id))?.point_refs();
        for r in refs.iter().copied().chain(std::iter::once(id)) {
            if let Some(e) = self.entities.get_mut(&r) {
                e.external = external;
            }
        }
        self.touch();
        Ok(())
    }

    // ------------------------------------------------------------------
    // removal

    /// Removes entities. Curves using removed points are removed too; defining
    /// points of removed curves are removed if no other curve uses them.
    /// Constraints that reference removed entities are removed. Returns all
    /// removed entity ids.
    pub fn remove_entities(&mut self, ids: &[EntityId]) -> Vec<EntityId> {
        let mut removed: Vec<EntityId> = Vec::new();
        let mut set: std::collections::BTreeSet<EntityId> = ids.iter().copied().filter(|i| self.entities.contains_key(i)).collect();
        // cascade: curves referencing removed points
        loop {
            let mut grew = false;
            for e in self.entities.values() {
                if !set.contains(&e.id) && e.point_refs().iter().any(|p| set.contains(p)) {
                    set.insert(e.id);
                    grew = true;
                }
            }
            if !grew {
                break;
            }
        }
        // orphaned defining points of removed curves
        let mut candidates: Vec<EntityId> = Vec::new();
        for id in &set {
            if let Some(e) = self.entities.get(id) {
                candidates.extend(e.point_refs());
            }
        }
        for p in candidates {
            if set.contains(&p) {
                continue;
            }
            let used = self.entities.values().any(|e| !set.contains(&e.id) && e.point_refs().contains(&p));
            if !used {
                set.insert(p);
            }
        }
        for id in &set {
            if self.entities.shift_remove(id).is_some() {
                removed.push(*id);
            }
        }
        self.constraints.retain(|_, c| !c.kind.entities().iter().any(|e| set.contains(e)));
        self.touch();
        removed
    }

    pub fn remove_entity(&mut self, id: EntityId) -> Vec<EntityId> {
        self.remove_entities(&[id])
    }

    /// Replaces every use of point `from` by point `to` (in curves and
    /// constraints) and removes `from`. Equivalent to a coincident constraint.
    pub fn merge_points(&mut self, from: EntityId, to: EntityId) -> Result<()> {
        if self.kind(from) != Some(EntityKind::Point) {
            return Err(SketchError::UnknownEntity(from));
        }
        if self.kind(to) != Some(EntityKind::Point) {
            return Err(SketchError::UnknownEntity(to));
        }
        if from == to {
            return Ok(());
        }
        for e in self.entities.values_mut() {
            e.replace_point(from, to);
        }
        for c in self.constraints.values_mut() {
            c.kind.replace_entity(from, to);
        }
        // drop self-referential constraints
        self.constraints.retain(|_, c| {
            !matches!(c.kind, ConstraintKind::Coincident { a, b } | ConstraintKind::Concentric { a, b } if a == b)
        });
        self.entities.shift_remove(&from);
        self.touch();
        Ok(())
    }

    /// Replaces the geometry of an entity in place (keeps id and flags).
    pub(crate) fn replace_geometry(&mut self, id: EntityId, g: Geometry) {
        if let Some(e) = self.entities.get_mut(&id) {
            e.geometry = g;
        }
        self.touch();
    }

    // ------------------------------------------------------------------
    // constraints

    fn alloc_constraint(&mut self) -> ConstraintId {
        let id = ConstraintId(self.next_constraint);
        self.next_constraint += 1;
        id
    }

    /// Adds a (driving) constraint or dimension. Orientation hints (side,
    /// quadrant, internal/external tangency, spline end) are captured from
    /// the current geometry.
    pub fn add_constraint(&mut self, kind: ConstraintKind) -> Result<ConstraintId> {
        self.add_constraint_ext(kind, true)
    }

    /// Adds a driven (reference) dimension: measured, not enforced.
    pub fn add_reference(&mut self, kind: ConstraintKind) -> Result<ConstraintId> {
        self.add_constraint_ext(kind, false)
    }

    pub fn add_constraint_ext(&mut self, kind: ConstraintKind, driving: bool) -> Result<ConstraintId> {
        for e in kind.entities() {
            if !self.entities.contains_key(&e) {
                return Err(SketchError::UnknownEntity(e));
            }
        }
        if let ConstraintKind::Pierce { target, .. } = &kind
            && !self.entity(*target).is_some_and(|e| e.external) {
                return Err(SketchError::InvalidConstraint("pierce target must be external geometry".into()));
            }
        let (hints, aux) = self.compute_hints(&kind);
        let driving = driving || !kind.is_dimension();
        let mut c = Constraint { id: ConstraintId(u32::MAX), kind, driving, hints, aux };
        crate::equations::validate(self, &c).map_err(SketchError::InvalidConstraint)?;
        let id = self.alloc_constraint();
        c.id = id;
        if !c.driving
            && let Some(v) = self.measure_kind(&c.kind, &c.hints) {
                c.kind.set_value(v);
            }
        if c.driving {
            self.apply_rho(&c.kind);
        }
        self.constraints.insert(id, c);
        self.touch();
        Ok(id)
    }

    pub fn remove_constraint(&mut self, id: ConstraintId) -> Result<Constraint> {
        let c = self.constraints.shift_remove(&id).ok_or(SketchError::UnknownConstraint(id))?;
        self.touch();
        Ok(c)
    }

    /// Changes a dimension value (SI units). Call [`Sketch::solve`] afterwards.
    pub fn set_dimension_value(&mut self, id: ConstraintId, value: f64) -> Result<()> {
        let c = self.constraints.get_mut(&id).ok_or(SketchError::UnknownConstraint(id))?;
        if let ConstraintKind::Rho { .. } = c.kind
            && !(value > 0.0 && value < 1.0)
        {
            return Err(SketchError::InvalidConstraint("rho must be between 0 and 1".into()));
        }
        if !c.kind.set_value(value) {
            return Err(SketchError::InvalidConstraint(format!("{id} is not a dimension")));
        }
        let (kind, driving) = (c.kind.clone(), c.driving);
        if driving {
            self.apply_rho(&kind);
        }
        self.touch();
        Ok(())
    }

    /// A driving rho dimension sets its conic's rho.
    fn apply_rho(&mut self, kind: &ConstraintKind) {
        if let ConstraintKind::Rho { conic, value } = kind
            && let Some(e) = self.entities.get_mut(conic)
            && let Geometry::Conic { rho, .. } = &mut e.geometry
        {
            *rho = value.clamp(1e-6, 1.0 - 1e-6);
        }
    }

    /// Changes the ratio of a [`ConstraintKind::LengthRatio`] constraint.
    pub fn set_length_ratio(&mut self, id: ConstraintId, ratio: f64) -> Result<()> {
        if !(ratio.is_finite() && ratio > 0.0) {
            return Err(SketchError::InvalidConstraint("length ratio must be positive".into()));
        }
        let c = self.constraints.get_mut(&id).ok_or(SketchError::UnknownConstraint(id))?;
        match &mut c.kind {
            ConstraintKind::LengthRatio { ratio: r, .. } => *r = ratio,
            _ => return Err(SketchError::InvalidConstraint(format!("{id} is not a length ratio"))),
        }
        self.touch();
        Ok(())
    }

    /// Re-targets a constraint to new entities (same id, so labels and
    /// expressions keyed by it survive). Hints are recomputed from the
    /// current geometry; the change is rejected if the new kind is invalid.
    pub fn retarget_constraint(&mut self, id: ConstraintId, kind: ConstraintKind) -> Result<()> {
        for e in kind.entities() {
            if !self.entities.contains_key(&e) {
                return Err(SketchError::UnknownEntity(e));
            }
        }
        let old = self.constraints.get(&id).ok_or(SketchError::UnknownConstraint(id))?.clone();
        let (hints, aux) = self.compute_hints(&kind);
        let c = Constraint { id, kind, driving: old.driving, hints, aux };
        crate::equations::validate(self, &c).map_err(SketchError::InvalidConstraint)?;
        self.constraints.insert(id, c);
        self.touch();
        Ok(())
    }

    /// Switches a dimension between driving and driven (reference).
    pub fn set_driving(&mut self, id: ConstraintId, driving: bool) -> Result<()> {
        let c = self.constraints.get_mut(&id).ok_or(SketchError::UnknownConstraint(id))?;
        if !c.kind.is_dimension() {
            return Err(SketchError::InvalidConstraint("only dimensions can be driven".into()));
        }
        c.driving = driving;
        if driving {
            let k = c.kind.clone();
            self.apply_rho(&k);
        }
        let c = self.constraints.get_mut(&id).ok_or(SketchError::UnknownConstraint(id))?;
        if !driving {
            let (k, h) = (c.kind.clone(), c.hints.clone());
            if let Some(v) = self.measure_kind(&k, &h) {
                self.constraints.get_mut(&id).unwrap().kind.set_value(v);
            }
        }
        self.touch();
        Ok(())
    }

    /// Re-captures orientation hints of a constraint from the current geometry
    /// (e.g. after the user flips a dimension's side).
    pub fn recompute_hints(&mut self, id: ConstraintId) -> Result<()> {
        let k = self.constraints.get(&id).ok_or(SketchError::UnknownConstraint(id))?.kind.clone();
        let (h, aux) = self.compute_hints(&k);
        let c = self.constraints.get_mut(&id).unwrap();
        c.hints = h;
        c.aux = aux;
        self.touch();
        Ok(())
    }

    /// Updates the measured value of driven (reference) dimensions.
    pub fn update_driven_dimensions(&mut self) {
        let ids: Vec<ConstraintId> = self.constraints.values().filter(|c| !c.driving).map(|c| c.id).collect();
        for id in ids {
            if let Some(v) = self.measure(id) {
                self.constraints.get_mut(&id).unwrap().kind.set_value(v);
            }
        }
    }
}

/// Ellipse parameter of (the projection of) point `p`.
pub fn ellipse_param(center: DVec2, major: DVec2, minor: f64, p: DVec2) -> f64 {
    let a = major.length().max(1e-300);
    let u = major / a;
    let d = p - center;
    let x = d.dot(u) / a;
    let y = u.perp_dot(d) / minor.max(1e-300);
    y.atan2(x)
}
