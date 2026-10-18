//! Grow a sketch selection into related curves (Onshape's right-click
//! "Select ▸" submenu in a sketch: Tangent connected edges, Equal
//! length/radius edges, Parallel edges).

use crate::constraints::ConstraintKind;
use crate::entities::EntityKind;
use crate::geom::Curve2;
use crate::ids::EntityId;
use crate::sketch::Sketch;
use glam::DVec2;
use std::collections::{HashSet, VecDeque};

/// How [`Sketch::select_related`] grows a selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RelatedSelection {
    /// Curves joined end to end with a smooth (tangent) joint, transitively.
    TangentConnected,
    /// Lines of equal length and circles / arcs of equal radius.
    EqualSize,
    /// Lines parallel to a selected line.
    Parallel,
}

impl RelatedSelection {
    pub const ALL: [RelatedSelection; 3] = [RelatedSelection::TangentConnected, RelatedSelection::EqualSize, RelatedSelection::Parallel];

    /// Menu label (Onshape wording).
    pub fn label(self) -> &'static str {
        match self {
            RelatedSelection::TangentConnected => "Tangent connected edges",
            RelatedSelection::EqualSize => "Equal length/radius edges",
            RelatedSelection::Parallel => "Parallel edges",
        }
    }
}

const ANGLE_TOL: f64 = 1e-6;
const SIZE_TOL: f64 = 1e-9;

impl Sketch {
    /// The selected curves plus every curve related to them by `how`. Points
    /// in `seeds` are kept as they are; the result has no duplicates and
    /// keeps the seeds first.
    pub fn select_related(&self, seeds: &[EntityId], how: RelatedSelection) -> Vec<EntityId> {
        let mut out: Vec<EntityId> = Vec::new();
        let mut seen = HashSet::new();
        for s in seeds {
            if self.entity(*s).is_some() && seen.insert(*s) {
                out.push(*s);
            }
        }
        let curves: Vec<EntityId> = out.iter().copied().filter(|e| self.curve(*e).is_some()).collect();
        let mut add = |e: EntityId, out: &mut Vec<EntityId>| {
            if seen.insert(e) {
                out.push(e);
            }
        };
        match how {
            RelatedSelection::TangentConnected => {
                let mut queue: VecDeque<EntityId> = curves.into_iter().collect();
                while let Some(c) = queue.pop_front() {
                    for n in self.tangent_neighbours(c) {
                        if !out.contains(&n) {
                            add(n, &mut out);
                            queue.push_back(n);
                        }
                    }
                }
            }
            RelatedSelection::EqualSize => {
                let lengths: Vec<f64> = curves.iter().filter_map(|c| self.line_length(*c)).collect();
                let radii: Vec<f64> = curves.iter().filter_map(|c| self.round_radius(*c)).collect();
                let near = |v: f64, set: &[f64]| set.iter().any(|x| (x - v).abs() <= SIZE_TOL.max(x.abs() * 1e-9));
                let others: Vec<EntityId> = self.entities().map(|e| e.id).collect();
                for e in others {
                    let hit = self.line_length(e).is_some_and(|l| near(l, &lengths)) || self.round_radius(e).is_some_and(|r| near(r, &radii));
                    if hit {
                        add(e, &mut out);
                    }
                }
            }
            RelatedSelection::Parallel => {
                let dirs: Vec<DVec2> = curves.iter().filter_map(|c| self.line_dir(*c)).collect();
                let others: Vec<EntityId> = self.entities().map(|e| e.id).collect();
                for e in others {
                    if self.line_dir(e).is_some_and(|d| dirs.iter().any(|x| x.perp_dot(d).abs() <= ANGLE_TOL)) {
                        add(e, &mut out);
                    }
                }
            }
        }
        out
    }

    fn line_dir(&self, id: EntityId) -> Option<DVec2> {
        match self.curve(id)? {
            Curve2::Line { a, b } => (b - a).try_normalize(),
            _ => None,
        }
    }

    fn line_length(&self, id: EntityId) -> Option<f64> {
        match self.curve(id)? {
            Curve2::Line { a, b } => Some((b - a).length()),
            _ => None,
        }
    }

    fn round_radius(&self, id: EntityId) -> Option<f64> {
        match self.curve(id)? {
            Curve2::Circle { radius, .. } | Curve2::Arc { radius, .. } => Some(radius),
            _ => None,
        }
    }

    /// Points that coincide with `p`: itself plus points joined to it by
    /// coincident constraints (transitively).
    fn joint_points(&self, p: EntityId) -> Vec<EntityId> {
        let mut pts = vec![p];
        let mut k = 0;
        while k < pts.len() {
            let q = pts[k];
            for c in self.constraints() {
                if let ConstraintKind::Coincident { a, b } = c.kind {
                    let other = if a == q { b } else if b == q { a } else { continue };
                    if self.kind(other) == Some(EntityKind::Point) && !pts.contains(&other) {
                        pts.push(other);
                    }
                }
            }
            k += 1;
        }
        pts
    }

    /// Unit tangent of `id` at its end nearest to `at`, pointing along the curve.
    fn end_tangent(&self, id: EntityId, at: DVec2) -> Option<DVec2> {
        let c = self.curve(id)?;
        let (t0, t1) = c.domain();
        let t = if (c.start_point() - at).length() <= (c.end_point() - at).length() { t0 } else { t1 };
        c.tangent(t).try_normalize()
    }

    /// Curves sharing an end with `id` at a smooth joint.
    fn tangent_neighbours(&self, id: EntityId) -> Vec<EntityId> {
        let Some((s, e)) = self.endpoints(id) else { return Vec::new() };
        let mut out = Vec::new();
        for end in [s, e] {
            let Some(at) = self.point(end) else { continue };
            let Some(t) = self.end_tangent(id, at) else { continue };
            for p in self.joint_points(end) {
                for other in self.users_of_point(p) {
                    if other == id || out.contains(&other) {
                        continue;
                    }
                    let Some((os, oe)) = self.endpoints(other) else { continue };
                    if p != os && p != oe {
                        continue;
                    }
                    let Some(u) = self.end_tangent(other, at) else { continue };
                    if t.perp_dot(u).abs() <= 1e-4 {
                        out.push(other);
                    }
                }
            }
        }
        out
    }
}
