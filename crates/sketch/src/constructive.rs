//! Live's least change for geometry its constraints don't hold yet (a
//! regeneration with new dimension values, a sketch posted unsolved).
//!
//! Live (fuzz probes 1–2, behaviours "skasm fuzz"): nothing is split between
//! entities. A dimensioned lone line keeps its endpoint with the smaller x
//! and moves the other; a line whose end is tied to an arc moves its free
//! start instead; what hangs on a moved point is carried rigidly (an L's
//! second line translates whole, a rectangle's right side moves and its left
//! stays); a relation changes the member with the larger value (Equal
//! shrinks the longer line, Parallel turns the line at the larger angle about
//! its start, Concentric moves the circle at larger coordinates).
//!
//! So, per connected component whose constraints don't hold: entities are
//! placed one at a time, the most constrained first (ties: the member a
//! relation keeps), each solved alone against the placed ones, after being
//! carried onto a placed point it shares, keeping its lower point. The
//! caller keeps the result only when every constraint then holds.

use crate::constraints::ConstraintKind;
use crate::entities::{EntityKind, Geometry};
use crate::ids::EntityId;
use crate::sketch::Sketch;
use crate::solver::{SolveOptions, SolveStatus};
use glam::DVec2;
use std::collections::{BTreeSet, HashMap, HashSet};

/// Components larger than this keep the weighted minimum norm.
const MAX_UNITS: usize = 60;

impl Sketch {
    /// Places the entities of every component whose constraints don't hold
    /// (see the module docs). False when a component can't be placed that way
    /// (an entity alone can't meet its constraints to the placed ones); the
    /// sketch may then be partly moved, and the caller restores it.
    pub(crate) fn place_constructively(&mut self, unheld: &HashSet<crate::ids::ConstraintId>) -> bool {
        // Units: each curve with its points; points of no curve on their own.
        let mut unit_of: HashMap<EntityId, EntityId> = HashMap::new();
        let mut units: Vec<EntityId> = Vec::new();
        for e in self.entities().filter(|e| e.kind() != EntityKind::Point && !e.external) {
            units.push(e.id);
            unit_of.insert(e.id, e.id);
            for p in e.point_refs() {
                unit_of.entry(p).or_insert(e.id);
            }
        }
        for e in self.entities().filter(|e| e.kind() == EntityKind::Point && !e.external) {
            if let std::collections::hash_map::Entry::Vacant(v) = unit_of.entry(e.id) {
                v.insert(e.id);
                units.push(e.id);
            }
        }
        // Each driving constraint's units (external geometry is fixed).
        let cons: Vec<(crate::ids::ConstraintId, Vec<EntityId>)> = self
            .constraints()
            .filter(|c| c.driving)
            .map(|c| {
                let mut us: Vec<EntityId> = c.kind.entities().iter().filter_map(|e| unit_of.get(e).copied()).collect();
                us.sort_unstable();
                us.dedup();
                (c.id, us)
            })
            .collect();
        // Components of units joined by constraints.
        let mut parent: HashMap<EntityId, EntityId> = units.iter().map(|u| (*u, *u)).collect();
        fn root(p: &mut HashMap<EntityId, EntityId>, mut x: EntityId) -> EntityId {
            while p[&x] != x {
                let up = p[&p[&x]];
                p.insert(x, up);
                x = up;
            }
            x
        }
        for (_, us) in &cons {
            for w in us.windows(2) {
                let (a, b) = (root(&mut parent, w[0]), root(&mut parent, w[1]));
                if a != b {
                    parent.insert(a, b);
                }
            }
        }
        let mut comps: HashMap<EntityId, Vec<EntityId>> = HashMap::new();
        for &u in &units {
            let r = root(&mut parent, u);
            comps.entry(r).or_default().push(u);
        }
        let mut comps: Vec<Vec<EntityId>> = comps.into_values().collect();
        comps.sort_by_key(|c| c[0]);
        for comp in comps {
            // Each entity solved on its own: kept to everyday sizes.
            if comp.len() > MAX_UNITS {
                continue;
            }
            let set: BTreeSet<EntityId> = comp.iter().copied().collect();
            let comp_cons: Vec<(crate::ids::ConstraintId, Vec<EntityId>)> = cons.iter().filter(|(_, us)| us.iter().any(|u| set.contains(u))).cloned().collect();
            if !comp_cons.iter().any(|(c, _)| unheld.contains(c)) {
                continue;
            }
            if !self.place_component(&comp, &comp_cons, &unit_of) {
                return false;
            }
        }
        true
    }

    /// Whether constraint `c` joins geometry by position (a shared point, a
    /// tangency, a point on a curve…) rather than relating sizes or
    /// directions.
    fn connective(&self, c: crate::ids::ConstraintId) -> bool {
        matches!(
            self.constraint(c).map(|c| &c.kind),
            Some(
                ConstraintKind::Coincident { .. }
                    | ConstraintKind::Concentric { .. }
                    | ConstraintKind::Tangent { .. }
                    | ConstraintKind::Midpoint { .. }
                    | ConstraintKind::MidpointOf { .. }
                    | ConstraintKind::Pierce { .. }
                    | ConstraintKind::Collinear { .. }
                    | ConstraintKind::Symmetric { .. }
            )
        )
    }

    /// [`Sketch::sub_sketch`] with a filter that sees each constraint's id.
    fn sub_sketch_by(&self, cons: &[(crate::ids::ConstraintId, Vec<EntityId>)], keep: &dyn Fn(crate::ids::ConstraintId, &[EntityId]) -> bool, placed: &BTreeSet<EntityId>) -> Option<Sketch> {
        let wanted: Vec<(crate::ids::ConstraintId, Vec<EntityId>)> = cons.iter().filter(|(c, us)| keep(*c, us)).cloned().collect();
        self.sub_sketch(&[], &wanted, &|_| true, placed)
    }

    /// This sketch with only the constraints `keep` allows (of `cons`) and
    /// the placed units fixed.
    fn sub_sketch(&self, _comp: &[EntityId], cons: &[(crate::ids::ConstraintId, Vec<EntityId>)], keep: &dyn Fn(&[EntityId]) -> bool, placed: &BTreeSet<EntityId>) -> Option<Sketch> {
        let mut sub = self.clone();
        let wanted: HashSet<crate::ids::ConstraintId> = cons.iter().filter(|(_, us)| keep(us)).map(|(c, _)| *c).collect();
        let drop: Vec<crate::ids::ConstraintId> = sub.constraints().map(|c| c.id).filter(|c| !wanted.contains(c)).collect();
        for c in drop {
            sub.remove_constraint(c).ok()?;
        }
        // Placed geometry fixed: its points (a circle whole, for its radius);
        // fixing a line and its points too would be redundant.
        for &u in placed {
            let e = self.entity(u)?;
            if e.kind() == EntityKind::Circle || e.kind() == EntityKind::Point {
                sub.add_constraint(ConstraintKind::Fix { entity: u }).ok()?;
            } else {
                for p in e.point_refs() {
                    sub.add_constraint(ConstraintKind::Fix { entity: p }).ok()?;
                }
            }
        }
        Some(sub)
    }

    /// Places `comp`'s units from what changes outwards (module docs).
    fn place_component(&mut self, comp: &[EntityId], cons: &[(crate::ids::ConstraintId, Vec<EntityId>)], unit_of: &HashMap<EntityId, EntityId>) -> bool {
        let mut placed: BTreeSet<EntityId> = BTreeSet::new();
        let unheld = self.unheld_constraints(1e-9);
        let mut holds: HashMap<crate::ids::ConstraintId, bool> = cons.iter().map(|(c, _)| (*c, !unheld.contains(c))).collect();
        let total = |u: EntityId| cons.iter().filter(|(_, us)| us.contains(&u)).count();
        while placed.len() < comp.len() {
            // Next: a unit with a constraint to placed units (or of its own)
            // that doesn't hold, the one with most of them; with none, a
            // unit of a constraint that doesn't hold (the member it keeps
            // first); with none at all, everything left stays.
            let pending = |u: EntityId| cons.iter().filter(|(c, us)| us.contains(&u) && us.iter().all(|x| *x == u || placed.contains(x)) && !holds[c]).count();
            let open: Vec<EntityId> = comp.iter().copied().filter(|u| !placed.contains(u)).collect();
            let best = open.iter().map(|u| (pending(*u), total(*u))).max().unwrap();
            // First, a relation between units that doesn't hold, none of
            // them placed: the member it keeps stays as it is (if its own
            // constraints hold), so the others come to it (fuzz s8: the
            // dimensioned circle of a concentric pair moves onto the other).
            let own_hold = |u: EntityId| cons.iter().filter(|(_, us)| us.as_slice() == [u]).all(|(c, _)| holds[c]);
            let keep = cons.iter().filter(|(c, us)| us.len() >= 2 && !holds[c] && !us.iter().any(|x| placed.contains(x))).find_map(|(_, us)| {
                let k = self.kept_first(us, cons);
                own_hold(k).then_some(k)
            });
            if let Some(k) = keep {
                placed.insert(k);
                continue;
            }
            let u = if best.0 > 0 {
                let tied: Vec<EntityId> = open.iter().copied().filter(|u| (pending(*u), total(*u)) == best).collect();
                self.kept_first(&tied, cons)
            } else {
                let broken: Vec<EntityId> = open.iter().copied().filter(|u| cons.iter().any(|(c, us)| us.contains(u) && !holds[c])).collect();
                if broken.is_empty() {
                    break;
                }
                // The relation's own rule picks, not the constraint count
                // (probe p8: the dimensioned circle of a concentric pair
                // still moves onto the lower one).
                self.kept_first(&broken, cons)
            };
            self.carry_onto_placed(u, &placed, unit_of);
            // Neighbours joined to u (a shared point, a tangency) hanging on
            // u alone follow it; the others are held. Relations to unplaced
            // units (Equal, Parallel, …) wait for those units (fuzz s10: an
            // Equal to the next line didn't hold the line being dimensioned;
            // that line follows the Equal when its turn comes).
            let neighbours: BTreeSet<EntityId> = cons.iter().filter(|(c, us)| us.contains(&u) && self.connective(*c)).flat_map(|(_, us)| us.iter().copied()).filter(|x| *x != u && !placed.contains(x)).collect();
            let held: BTreeSet<EntityId> = neighbours.iter().copied().filter(|v| cons.iter().any(|(_, us)| us.contains(v) && !us.contains(&u))).collect();
            let anchor = self.anchor(u);
            let mut done = None;
            // Least change in its own terms: its length and direction kept,
            // then its length only (Parallel turns it, probe p11), then
            // neither; tied neighbours held first, released after (fuzz s5:
            // a side carried by a distance translates, it doesn't tilt).
            'attempts: for (hold_neighbours, shape) in [(true, 2), (true, 1), (true, 0), (false, 2), (false, 1), (false, 0)] {
                let mut fixed = placed.clone();
                if hold_neighbours {
                    fixed.extend(held.iter().copied());
                }
                let mut with = fixed.clone();
                with.insert(u);
                let keep = |c: crate::ids::ConstraintId, us: &[EntityId]| us.contains(&u) && us.iter().all(|x| with.contains(x)) && (us.iter().all(|x| *x == u || placed.contains(x)) || self.connective(c));
                let Some(mut sub) = self.sub_sketch_by(cons, &keep, &fixed) else { return false };
                if shape >= 1 && !self.hold_size(&mut sub, u) {
                    continue;
                }
                if shape >= 2 && !self.hold_direction(&mut sub, u) {
                    continue;
                }
                let opts = SolveOptions { light: vec![u], still: anchor.into_iter().collect(), analyze: false, constructive: false, ..Default::default() };
                if matches!(sub.solve_with(&opts).status, SolveStatus::Solved) {
                    done = Some(sub);
                    break 'attempts;
                }
            }
            // A loop one unit can't close: u with its unplaced neighbours
            // (placed geometry still fixed), then with its placed ones too;
            // the change is shared within the group.
            let mut group = vec![u];
            if done.is_none() {
                let all_neighbours: Vec<EntityId> = cons.iter().filter(|(_, us)| us.contains(&u)).flat_map(|(_, us)| us.iter().copied()).filter(|x| *x != u).collect();
                for release_placed in [false, true] {
                    let mut g: BTreeSet<EntityId> = all_neighbours.iter().copied().filter(|x| release_placed || !placed.contains(x)).collect();
                    g.insert(u);
                    let fixed: BTreeSet<EntityId> = placed.iter().copied().filter(|x| !g.contains(x)).collect();
                    let mut with = fixed.clone();
                    with.extend(g.iter().copied());
                    let keep = |us: &[EntityId]| us.iter().any(|x| g.contains(x)) && us.iter().all(|x| with.contains(x));
                    let Some(mut sub) = self.sub_sketch(comp, cons, &keep, &fixed) else { return false };
                    let still: Vec<EntityId> = g.iter().filter_map(|x| self.anchor(*x)).collect();
                    let opts = SolveOptions { light: g.iter().copied().collect(), still, analyze: false, constructive: false, ..Default::default() };
                    if matches!(sub.solve_with(&opts).status, SolveStatus::Solved) {
                        done = Some(sub);
                        group = g.into_iter().collect();
                        break;
                    }
                }
            }
            let Some(sub) = done else {
                return false;
            };
            // The group's points (and a circle's radius) back.
            for &g in &group {
                let Some(e) = self.entity(g) else { return false };
                let points = e.point_refs();
                for p in points.iter().chain(std::iter::once(&g)) {
                    if let Some(x) = sub.entity(*p).map(|x| x.geometry.clone())
                        && let Some(mine) = self.entity_mut_silent(*p)
                    {
                        mine.geometry = x;
                    }
                }
                placed.insert(g);
            }
            let unheld = self.unheld_constraints(1e-9);
            for (c, _) in cons {
                holds.insert(*c, !unheld.contains(c));
            }
        }
        self.touch();
        true
    }

    /// Holds a line's direction in `sub` (parallel to a fixed construction
    /// line along it); false for other curves.
    fn hold_direction(&self, sub: &mut Sketch, u: EntityId) -> bool {
        let Some(d) = self.placing_line_dir(u).filter(|d| d.length() > 0.0) else { return false };
        let Some((s0, _)) = self.endpoints(u) else { return false };
        let Some(a) = self.point(s0) else { return false };
        let r = sub.add_line(a, a + d);
        let _ = sub.set_construction(r, true);
        sub.add_constraint(ConstraintKind::Fix { entity: r }).is_ok() && sub.add_constraint(ConstraintKind::Parallel { a: u, b: r }).is_ok()
    }

    /// Holds `u`'s size in `sub` (a line's length, a circle's or arc's
    /// radius) as a constraint at its current value; false for a point.
    fn hold_size(&self, sub: &mut Sketch, u: EntityId) -> bool {
        let size = self.size(u);
        let kind = match self.kind(u) {
            Some(EntityKind::Line) => ConstraintKind::Length { curve: u, value: size },
            Some(EntityKind::Circle | EntityKind::Arc) => ConstraintKind::Radius { curve: u, value: size },
            _ => return false,
        };
        size.is_finite() && sub.add_constraint(kind).is_ok()
    }

    /// Of tied units, the one a relation between two of them keeps (Equal:
    /// the shorter; Parallel / Perpendicular: the one nearer the x axis;
    /// Concentric: the lower center); else the one with the lower point.
    fn kept_first(&self, tied: &[EntityId], cons: &[(crate::ids::ConstraintId, Vec<EntityId>)]) -> EntityId {
        let lower = |u: EntityId| self.anchor(u).and_then(|p| self.point(p)).unwrap_or(DVec2::splat(f64::MAX));
        for (cid, us) in cons {
            if us.len() != 2 || !us.iter().all(|u| tied.contains(u)) {
                continue;
            }
            let Some(c) = self.constraint(*cid) else { continue };
            let key = |u: EntityId| -> f64 {
                match c.kind {
                    ConstraintKind::Equal { .. } => self.size(u),
                    ConstraintKind::Parallel { .. } | ConstraintKind::Perpendicular { .. } => self.placing_line_dir(u).map_or(f64::MAX, |d| d.y.atan2(d.x).rem_euclid(std::f64::consts::PI).min(std::f64::consts::PI - d.y.atan2(d.x).rem_euclid(std::f64::consts::PI))),
                    _ => f64::NAN,
                }
            };
            let (a, b) = (us[0], us[1]);
            // Structure first: the member with more of its points tied to
            // other geometry stays (fuzz s1: a rectangle's side kept, the lone
            // line with a free end lengthened to it).
            let (ta, tb) = (self.tied_points(a, cons), self.tied_points(b, cons));
            if ta != tb {
                return if ta > tb { a } else { b };
            }
            let (ka, kb) = (key(a), key(b));
            if ka.is_finite() && kb.is_finite() && ka != kb {
                return if ka < kb { a } else { b };
            }
        }
        let mut best = tied[0];
        for &u in &tied[1..] {
            let (p, q) = (lower(u), lower(best));
            if (p.x, p.y) < (q.x, q.y) {
                best = u;
            }
        }
        best
    }

    /// How many of `u`'s points a joining constraint ties to other units
    /// (a shared point, a concentric center, a point on a curve).
    fn tied_points(&self, u: EntityId, cons: &[(crate::ids::ConstraintId, Vec<EntityId>)]) -> usize {
        let Some(e) = self.entity(u) else { return 0 };
        let own: Vec<EntityId> = e.point_refs().into_iter().collect();
        let center = e.center();
        let mut tied: BTreeSet<EntityId> = BTreeSet::new();
        for (c, us) in cons {
            if us.len() < 2 || !us.contains(&u) || !self.connective(*c) {
                continue;
            }
            let Some(k) = self.constraint(*c).map(|c| c.kind.clone()) else { continue };
            if matches!(k, ConstraintKind::Concentric { .. }) {
                tied.extend(center);
                continue;
            }
            tied.extend(k.entities().into_iter().filter(|x| own.contains(x)));
        }
        tied.len()
    }

    /// A unit's point kept when it alone takes a change: its lowest point
    /// (smaller x, then y); a circle's or arc's center.
    fn anchor(&self, u: EntityId) -> Option<EntityId> {
        match &self.entity(u)?.geometry {
            Geometry::Point { .. } => Some(u),
            Geometry::Circle { center, .. } | Geometry::Arc { center, .. } => Some(*center),
            // A line, a conic, a spline: its lowest defining point (live
            // fuzz s116: a conic closed by a dimensioned line kept its start).
            g => {
                let _ = g;
                let e = self.entity(u)?;
                e.point_refs().into_iter().filter_map(|p| self.point(p).map(|q| (p, q))).min_by(|a, b| (a.1.x, a.1.y).partial_cmp(&(b.1.x, b.1.y)).unwrap_or(std::cmp::Ordering::Equal)).map(|(p, _)| p)
            }
        }
    }

    fn size(&self, u: EntityId) -> f64 {
        match &self.entity(u).map(|e| e.geometry.clone()) {
            Some(Geometry::Line { start, end }) => (self.point(*end).unwrap_or_default() - self.point(*start).unwrap_or_default()).length(),
            Some(Geometry::Circle { radius, .. }) => *radius,
            Some(Geometry::Arc { center, start, .. }) => (self.point(*start).unwrap_or_default() - self.point(*center).unwrap_or_default()).length(),
            _ => f64::NAN,
        }
    }

    fn placing_line_dir(&self, u: EntityId) -> Option<DVec2> {
        match self.entity(u)?.geometry {
            Geometry::Line { start, end } => Some(self.point(end)? - self.point(start)?),
            _ => None,
        }
    }

    /// Moves `u` rigidly so a point it shares (Coincident, Concentric) with
    /// a placed unit is where that one is.
    fn carry_onto_placed(&mut self, u: EntityId, placed: &BTreeSet<EntityId>, unit_of: &HashMap<EntityId, EntityId>) {
        let Some(own) = self.entity(u).map(|e| {
            let mut v: Vec<EntityId> = e.point_refs().into_iter().collect();
            v.push(u);
            v
        }) else {
            return;
        };
        let center = |s: &Sketch, id: EntityId| -> Option<EntityId> {
            let e = s.entity(id)?;
            if e.kind() == EntityKind::Point { Some(id) } else { e.center() }
        };
        let mut shift = None;
        for c in self.constraints().filter(|c| c.driving) {
            let (a, b) = match c.kind {
                ConstraintKind::Coincident { a, b } | ConstraintKind::Concentric { a, b } => (a, b),
                _ => continue,
            };
            let (Some(pa), Some(pb)) = (center(self, a), center(self, b)) else { continue };
            let (mine, theirs) = if own.contains(&pa) && unit_of.get(&pb).is_some_and(|x| placed.contains(x)) {
                (pa, pb)
            } else if own.contains(&pb) && unit_of.get(&pa).is_some_and(|x| placed.contains(x)) {
                (pb, pa)
            } else {
                continue;
            };
            if let (Some(m), Some(t)) = (self.point(mine), self.point(theirs)) {
                shift = Some(t - m);
                break;
            }
        }
        let Some(d) = shift else { return };
        let Some(points) = self.entity(u).map(|e| e.point_refs()) else { return };
        let points: Vec<EntityId> = if points.is_empty() { vec![u] } else { points.into_iter().collect() };
        for p in points {
            if let Some(x) = self.point(p) {
                let _ = self.set_point(p, x + d);
            }
        }
    }
}
