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
    /// (see the module docs). Returns the entities of the components that
    /// can't be placed that way (an entity alone can't meet its constraints
    /// to the placed ones): those are left as they were, for the least
    /// change (live fuzz s464: a hexagon the pass can't close beside a
    /// rounded rectangle it places as live).
    pub(crate) fn place_constructively(&mut self, unheld: &HashSet<crate::ids::ConstraintId>) -> Vec<EntityId> {
        // Curves sharing a point entity (the tools' rectangles, polylines)
        // are placed as if joined by a Coincident: on a copy whose shared
        // points are split, the positions copied back (a rectangle drawn
        // with the tool keeps its lower-left corner as one posted does).
        // (Placed directly: no second split, so no recursion, whatever the
        // copy still shares; qa SKETCH-CONSTRUCTIVE-RECURSION.)
        if let Some(mut split) = self.split_shared_points() {
            let unheld = split.unheld_constraints(1e-9);
            let left = split.place_split(&unheld);
            for e in self.entities().map(|e| e.id).collect::<Vec<_>>() {
                if let Some(g) = split.entity(e).map(|x| x.geometry.clone())
                    && let Some(mine) = self.entity_mut_silent(e)
                {
                    match (&mut mine.geometry, g) {
                        (Geometry::Point { pos }, Geometry::Point { pos: q }) => *pos = q,
                        (Geometry::Circle { radius, .. }, Geometry::Circle { radius: r, .. }) => *radius = r,
                        _ => {}
                    }
                }
            }
            self.touch();
            return left.into_iter().filter(|e| self.entity(*e).is_some()).collect();
        }
        self.place_split(unheld)
    }

    /// This sketch with every point entity shared by several curves given
    /// one copy per further curve, joined to it by a Coincident; None when
    /// no point is shared.
    fn split_shared_points(&self) -> Option<Sketch> {
        let mut users: HashMap<EntityId, Vec<EntityId>> = HashMap::new();
        for e in self.entities().filter(|e| e.kind() != EntityKind::Point) {
            for p in e.point_refs() {
                // Each curve once (a closed spline lists its first point
                // again: not shared, and its swap would move both).
                let us = users.entry(p).or_default();
                if !us.contains(&e.id) {
                    us.push(e.id);
                }
            }
        }
        let shared: Vec<(EntityId, Vec<EntityId>)> = users.into_iter().filter(|(_, us)| us.len() > 1).collect();
        if shared.is_empty() {
            return None;
        }
        let mut out = self.clone();
        for (p, us) in shared {
            let pos = self.point(p)?;
            for &curve in &us[1..] {
                let q = out.add_point(pos);
                let e = out.entity_mut_silent(curve)?;
                let swap = |x: &mut EntityId| {
                    if *x == p {
                        *x = q;
                    }
                };
                match &mut e.geometry {
                    Geometry::Line { start, end } => {
                        swap(start);
                        swap(end);
                    }
                    Geometry::Arc { center, start, end } | Geometry::EllipticalArc { center, start, end, .. } => {
                        swap(center);
                        swap(start);
                        swap(end);
                    }
                    Geometry::Circle { center, .. } | Geometry::Ellipse { center, .. } => swap(center),
                    Geometry::Conic { start, control, end, .. } => {
                        swap(start);
                        swap(control);
                        swap(end);
                    }
                    Geometry::BSpline { control, .. } => control.iter_mut().for_each(swap),
                    Geometry::FitSpline { points, handles } => {
                        points.iter_mut().for_each(swap);
                        if let Some((a, b)) = handles {
                            swap(a);
                            swap(b);
                        }
                    }
                    Geometry::Point { .. } => {}
                }
                out.add_constraint(ConstraintKind::Coincident { a: p, b: q }).ok()?;
            }
        }
        out.touch();
        Some(out)
    }

    /// [`Sketch::place_constructively`] on a sketch whose curves share no
    /// point entity.
    fn place_split(&mut self, unheld: &HashSet<crate::ids::ConstraintId>) -> Vec<EntityId> {
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
        // A point's units: its own, and those of the points it coincides
        // with.
        let mut point_units: HashMap<EntityId, BTreeSet<EntityId>> = HashMap::new();
        for c in self.constraints().filter(|c| c.driving) {
            if let ConstraintKind::Coincident { a, b } = c.kind
                && self.kind(a) == Some(EntityKind::Point)
                && self.kind(b) == Some(EntityKind::Point)
            {
                for (p, q) in [(a, b), (b, a)] {
                    if let Some(u) = unit_of.get(&q) {
                        point_units.entry(p).or_default().insert(*u);
                    }
                }
            }
        }
        // Each driving constraint's units (external geometry is fixed). One
        // between points of a single unit (through shared points) is that
        // unit's own (live fuzz s17: a slot arc's chord dimensioned between
        // its start and the next line's start shrinks the arc about its
        // center).
        let cons: Vec<(crate::ids::ConstraintId, Vec<EntityId>)> = self
            .constraints()
            .filter(|c| c.driving)
            .map(|c| {
                let ents = c.kind.entities();
                if ents.len() >= 2 && !matches!(c.kind, ConstraintKind::Coincident { .. }) && ents.iter().all(|e| self.kind(*e) == Some(EntityKind::Point)) {
                    let sets: Vec<BTreeSet<EntityId>> = ents.iter().map(|e| {
                        let mut set = point_units.get(e).cloned().unwrap_or_default();
                        set.extend(unit_of.get(e).copied());
                        set
                    }).collect();
                    if let Some(u) = sets[0].iter().find(|u| sets[1..].iter().all(|s| s.contains(u))) {
                        return (c.id, vec![*u]);
                    }
                }
                let mut us: Vec<EntityId> = ents.iter().filter_map(|e| unit_of.get(e).copied()).collect();
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
        let mut left = Vec::new();
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
            let own: Vec<EntityId> = comp.iter().flat_map(|u| std::iter::once(*u).chain(self.entity(*u).map(|e| e.point_refs()).unwrap_or_default())).collect();
            let was: Vec<(EntityId, Geometry)> = own.iter().filter_map(|e| self.entity(*e).map(|x| (*e, x.geometry.clone()))).collect();
            if !self.place_component(&comp, &comp_cons, &unit_of) {
                for (e, g) in was {
                    if let Some(x) = self.entity_mut_silent(e) {
                        x.geometry = g;
                    }
                }
                if !self.hold_the_base(&comp, &comp_cons) {
                    left.extend(own);
                }
            }
        }
        left
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
        let mut wanted: HashSet<crate::ids::ConstraintId> = cons.iter().filter(|(_, us)| keep(us)).map(|(c, _)| *c).collect();
        // A kept constraint between points brings the points they coincide
        // with (one a unit owns through a shared point).
        let between_points: HashSet<EntityId> = self
            .constraints()
            .filter(|c| wanted.contains(&c.id))
            .map(|c| c.kind.entities())
            .filter(|es| es.len() >= 2 && es.iter().all(|e| self.kind(*e) == Some(EntityKind::Point)))
            .flatten()
            .collect();
        for c in self.constraints().filter(|c| c.driving) {
            if let ConstraintKind::Coincident { a, b } = c.kind
                && (between_points.contains(&a) || between_points.contains(&b))
                && self.kind(a) == Some(EntityKind::Point)
                && self.kind(b) == Some(EntityKind::Point)
            {
                wanted.insert(c.id);
            }
        }
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

    /// Two lines a point-to-line Distance spans, one with a new Length: the
    /// Distance (holding) and the side it keeps ([`Sketch::kept_first_in`]).
    fn kept_across(&self, cons: &[(crate::ids::ConstraintId, Vec<EntityId>)]) -> Option<(crate::ids::ConstraintId, EntityId)> {
        let unheld = self.unheld_constraints(1e-9);
        let resizes = |u: EntityId| cons.iter().any(|(c, us)| us.as_slice() == [u] && unheld.contains(c) && matches!(self.constraint(*c).map(|k| &k.kind), Some(ConstraintKind::Length { .. })));
        cons.iter().find_map(|(c, us)| {
            let spans = us.len() == 2
                && !unheld.contains(c)
                && us.iter().all(|u| self.kind(*u) == Some(EntityKind::Line))
                && us.iter().any(|u| resizes(*u))
                && matches!(self.constraint(*c).map(|k| &k.kind), Some(ConstraintKind::Distance { a, b, .. }) if self.kind(*a) == Some(EntityKind::Point) && self.kind(*b) == Some(EntityKind::Line));
            spans.then(|| (*c, self.kept_first_in(us, cons, Some(*c))))
        })
    }

    /// A component the pass couldn't place: its unchanged units held,
    /// lowest first, each while the rest still solves, then the rest at
    /// least change (live fuzz s653 / s725: a T's wider top and thinner
    /// flange keep the whole stem; the Equal overhangs centre the top on it).
    fn hold_the_base(&mut self, comp: &[EntityId], cons: &[(crate::ids::ConstraintId, Vec<EntityId>)]) -> bool {
        let unheld = self.unheld_constraints(1e-9);
        let steady = |u: EntityId| !cons.iter().any(|(c, us)| us.contains(&u) && unheld.contains(c));
        let low = |u: EntityId| self.anchor(u).and_then(|p| self.point(p)).unwrap_or(DVec2::splat(f64::MAX));
        let mut base: Vec<EntityId> = comp.iter().copied().filter(|u| steady(*u)).collect();
        base.sort_by(|a, b| (low(*a).y, low(*a).x).partial_cmp(&(low(*b).y, low(*b).x)).unwrap_or(std::cmp::Ordering::Equal));
        let opts = SolveOptions { analyze: false, constructive: false, ..Default::default() };
        let mut held: BTreeSet<EntityId> = BTreeSet::new();
        let mut done = None;
        // The side a Distance keeps, held first; resized, from its lowest
        // point (live probes 14 q3: a trapezoid's growing shorter top keeps
        // its left end), or from the Distance's point when that is its own
        // and the other side resizes too (fuzz s713: the top grows, the base
        // shrinks, the top keeps the corner its height is dimensioned from).
        if let Some((c, k)) = self.kept_across(cons) {
            let other_resizes = cons.iter().find(|(x, _)| *x == c).is_some_and(|(_, us)| {
                us.iter().any(|u| *u != k && cons.iter().any(|(x, xs)| xs.as_slice() == [*u] && unheld.contains(x) && matches!(self.constraint(*x).map(|o| &o.kind), Some(ConstraintKind::Length { .. }))))
            });
            let was: Vec<(EntityId, Geometry)> = self.entity(k).map(|e| e.point_refs()).unwrap_or_default().into_iter().filter_map(|p| self.entity(p).map(|x| (p, x.geometry.clone()))).collect();
            let length = cons.iter().filter(|(x, us)| us.as_slice() == [k] && unheld.contains(x)).find_map(|(x, _)| match self.constraint(*x).map(|o| &o.kind) {
                Some(ConstraintKind::Length { value, .. }) => Some(*value),
                _ => None,
            });
            if let (Some(len), Some((s0, e0))) = (length, self.endpoints(k)) {
                let own = |p: EntityId| p == s0 || p == e0 || self.coincident_with(p).iter().any(|q| *q == s0 || *q == e0);
                let point = match self.constraint(c).map(|o| &o.kind) {
                    Some(ConstraintKind::Distance { a, .. }) if other_resizes && own(*a) => [s0, e0].into_iter().find(|q| *q == *a || self.coincident_with(*a).contains(q)),
                    _ => None,
                }
                .or_else(|| self.anchor(k));
                if let (Some(stay), Some(a), Some(b)) = (point, self.point(s0), self.point(e0)) {
                    let (from, other) = if stay == s0 { (a, e0) } else { (b, s0) };
                    let dir = if stay == s0 { b - a } else { a - b };
                    let _ = self.set_point(other, from + dir.normalize_or_zero() * len);
                }
            }
            held.insert(k);
            match self.sub_sketch(comp, cons, &|_| true, &held).map(|mut sub| (matches!(sub.solve_with(&opts).status, SolveStatus::Solved), sub)) {
                Some((true, sub)) => done = Some(sub),
                _ => {
                    for (p, g) in was {
                        if let Some(x) = self.entity_mut_silent(p) {
                            x.geometry = g;
                        }
                    }
                    return false;
                }
            }
        }
        // A T's resized stem: its corner at the side whose height is
        // dimensioned stays first (live probes 17 u1-u5).
        if held.is_empty() {
            let unit_of: HashMap<EntityId, EntityId> = comp.iter().flat_map(|u| self.entity(*u).map(|e| e.point_refs()).unwrap_or_default().into_iter().map(move |p| (p, *u))).collect();
            let holds: HashMap<crate::ids::ConstraintId, bool> = cons.iter().map(|(c, _)| (*c, !unheld.contains(c))).collect();
            if let Some(p) = comp.iter().find_map(|u| self.stem_anchor(*u, cons, &unit_of, &holds))
                && let Some(mut sub) = self.sub_sketch(comp, cons, &|_| true, &BTreeSet::from([p]))
                && matches!(sub.solve_with(&opts).status, SolveStatus::Solved)
            {
                held.insert(p);
                done = Some(sub);
            }
        }
        let base: Vec<EntityId> = base.into_iter().filter(|u| !held.contains(u)).collect();
        for u in base {
            let mut with = held.clone();
            with.insert(u);
            let Some(mut sub) = self.sub_sketch(comp, cons, &|_| true, &with) else { continue };
            if matches!(sub.solve_with(&opts).status, SolveStatus::Solved) {
                held = with;
                done = Some(sub);
            }
        }
        let Some(sub) = done else { return false };
        for &u in comp {
            let Some(e) = self.entity(u) else { continue };
            for p in e.point_refs().into_iter().chain(std::iter::once(u)) {
                if let Some(x) = sub.entity(p).map(|x| x.geometry.clone())
                    && let Some(mine) = self.entity_mut_silent(p)
                {
                    mine.geometry = x;
                }
            }
        }
        self.touch();
        true
    }

    /// Places `comp`'s units from what changes outwards (module docs).
    fn place_component(&mut self, comp: &[EntityId], cons: &[(crate::ids::ConstraintId, Vec<EntityId>)], unit_of: &HashMap<EntityId, EntityId>) -> bool {
        if self.scale_for_chord(comp, cons) {
            return true;
        }
        // Lines a point-to-line Distance spans, one resizing: the side the
        // Distance keeps fixes the rest (`hold_the_base`).
        if self.kept_across(cons).is_some() {
            return false;
        }
        let mut placed: BTreeSet<EntityId> = BTreeSet::new();
        // Sizes and directions to keep are as the pass found them (a side
        // sharing a point with one placed before has already been pulled:
        // a corner rectangle's right side after its bottom stretched).
        let orig = self.clone();
        let unheld = self.unheld_constraints(1e-9);
        let mut holds: HashMap<crate::ids::ConstraintId, bool> = cons.iter().map(|(c, _)| (*c, !unheld.contains(c))).collect();
        self.shift_loops_for_distances(cons, &placed);
        // Loops placed round from an anchor: their units next to placed
        // geometry go first.
        let mut anchored: BTreeSet<EntityId> = BTreeSet::new();
        let mut kept: BTreeSet<EntityId> = BTreeSet::new();
        for a in self.loop_anchors(comp, cons, &holds) {
            placed.insert(a);
            kept.insert(a);
            anchored.extend(self.loop_of(a, cons));
        }
        // A closed loop whose only change is a Distance between two of its
        // own points, with no size or Fix of its own, scales about that
        // Distance's first point and stays (live fuzz s240: a rectangle's
        // new diagonal keeps the corner it starts from).
        for &u in comp {
            if placed.contains(&u) {
                continue;
            }
            let ring = self.ring_of(u, cons);
            if ring.len() < 3 || !self.closed(&ring) || ring.iter().any(|x| placed.contains(x)) {
                continue;
            }
            let touching: Vec<&(crate::ids::ConstraintId, Vec<EntityId>)> = cons.iter().filter(|(_, us)| us.iter().any(|x| ring.contains(x))).collect();
            let broken: Vec<&&(crate::ids::ConstraintId, Vec<EntityId>)> = touching.iter().filter(|(c, _)| !holds[c]).collect();
            let [(cid, us)] = broken.as_slice() else { continue };
            if !us.iter().all(|x| ring.contains(x)) {
                continue;
            }
            let Some(ConstraintKind::Distance { a, b, value, .. }) = self.constraint(*cid).map(|k| k.kind.clone()) else { continue };
            let (Some(pa), Some(pb)) = (self.point(a), self.point(b)) else { continue };
            let sized = touching.iter().any(|(c, us)| *c != *cid && us.iter().all(|x| ring.contains(x)) && self.constraint(*c).is_some_and(|k| k.kind.is_dimension() || matches!(k.kind, ConstraintKind::Fix { .. })));
            // (Not the two ends of one curve: that's its length, fuzz s5.)
            let class = |p: EntityId| -> Vec<EntityId> { std::iter::once(p).chain(self.coincident_class(p)).collect() };
            let (ca, cb) = (class(a), class(b));
            let one_curve = ring.iter().filter_map(|x| self.entity(*x)).any(|e| {
                let r = e.point_refs();
                r.iter().any(|p| ca.contains(p)) && r.iter().any(|p| cb.contains(p))
            });
            let now = (pb - pa).length();
            if one_curve || sized || now < 1e-12 || value <= 0.0 || self.kind(a) != Some(EntityKind::Point) || self.kind(b) != Some(EntityKind::Point) {
                continue;
            }
            let k = value / now;
            let pts: BTreeSet<EntityId> = ring.iter().filter_map(|x| self.entity(*x)).flat_map(|e| e.point_refs()).collect();
            for p in pts {
                if let Some(x) = self.point(p) {
                    let _ = self.set_point(p, pa + (x - pa) * k);
                }
            }
            for x in &ring {
                if let Some(e) = self.entity_mut_silent(*x)
                    && let Geometry::Circle { radius, .. } = &mut e.geometry
                {
                    *radius *= k;
                }
            }
            placed.extend(ring.iter().copied());
            kept.extend(ring.iter().copied());
            let unheld = self.unheld_constraints(1e-9);
            for (c, _) in cons {
                holds.insert(*c, !unheld.contains(c));
            }
        }
        while placed.len() < comp.len() {
            // Next: a unit with a constraint to placed units (or of its own)
            // that doesn't hold, the one with most of them; with none, a
            // unit of a constraint that doesn't hold (the member it keeps
            // first); with none at all, everything left stays.
            // (An own Horizontal / Vertical reads unheld when an end's
            // Coincident partner moved, through the solver's aliasing: not a
            // reason of its own, fuzz s343; a shared point moved is one.)
            let own_relation = |c: crate::ids::ConstraintId, us: &[EntityId]| us.len() == 1 && self.constraint(c).is_some_and(|k| !k.kind.is_dimension()) && self.aliased(c);
            let pending = |u: EntityId| cons.iter().filter(|(c, us)| us.contains(&u) && us.iter().all(|x| *x == u || placed.contains(x)) && !holds[c] && !own_relation(*c, us)).count();
            let open: Vec<EntityId> = comp.iter().copied().filter(|u| !placed.contains(u)).collect();
            // Then the unit most joined (shared points, its own
            // constraints; relations to others don't count: probe t4, a
            // rectangle side Perpendicular to a separate line isn't first).
            let joined = |u: EntityId| cons.iter().filter(|(c, us)| us.contains(&u) && (us.len() == 1 || self.connective(*c))).count();
            let adjacent = |u: EntityId| usize::from(anchored.contains(&u) && cons.iter().any(|(c, us)| us.contains(&u) && self.connective(*c) && us.iter().any(|x| placed.contains(x))));
            // In a loop placed round from its anchor, a curve whose Equal
            // partner's dimension resizes it is pending too.
            let chained_change = |u: EntityId| usize::from(anchored.contains(&u) && self.chained_size(u, cons).is_some_and(|k| (k.value().unwrap_or(0.0) - self.size(u)).abs() > 1e-9));
            // A construction line with a new Length of its own goes first:
            // the profile hangs on it (live fuzz s811: a lever's centre
            // distance keeps its start, the small arc's centre comes in
            // before that arc is placed).
            let skeleton = |u: EntityId| {
                usize::from(self.entity(u).is_some_and(|e| e.construction && e.kind() == EntityKind::Line) && cons.iter().any(|(c, us)| us.as_slice() == [u] && !holds[c] && matches!(self.constraint(*c).map(|k| &k.kind), Some(ConstraintKind::Length { .. }))))
            };
            // Then the arcs and circles centred on a placed one's ends.
            let on_skeleton = |u: EntityId| {
                let centre = self.entity(u).filter(|e| matches!(e.kind(), EntityKind::Arc | EntityKind::Circle)).and_then(|e| e.center());
                usize::from(centre.is_some_and(|c| {
                    placed.iter().filter(|x| self.entity(**x).is_some_and(|e| e.construction && e.kind() == EntityKind::Line)).filter_map(|x| self.endpoints(*x)).any(|(a, b)| [a, b].iter().any(|p| *p == c || self.coincident_class(c).contains(p)))
                }))
            };
            // On a tie, a line with a new Length of its own before a curve
            // (live probes 17 v3: an L-bracket's thinner foot comes down
            // before its rounded inner corner grows).
            let own_change = |u: EntityId, radius: bool| cons.iter().any(|(c, us)| us.as_slice() == [u] && !holds[c] && matches!((radius, self.constraint(*c).map(|k| &k.kind)), (false, Some(ConstraintKind::Length { .. })) | (true, Some(ConstraintKind::Radius { .. } | ConstraintKind::Diameter { .. }))));
            let joined_to = |u: EntityId| -> Vec<EntityId> { cons.iter().filter(|(c, us)| us.contains(&u) && self.connective(*c)).flat_map(|(_, us)| us.iter().copied()).filter(|x| *x != u).collect() };
            let resized_line = |u: EntityId| {
                usize::from(self.kind(u) == Some(EntityKind::Line) && own_change(u, false) && joined_to(u).into_iter().filter(|n| self.kind(*n) == Some(EntityKind::Line)).any(|n| joined_to(n).into_iter().any(|a| a != u && matches!(self.kind(a), Some(EntityKind::Arc)) && own_change(a, true))))
            };
            let rank = |u: EntityId| (pending(u) + chained_change(u) + 100 * skeleton(u) + 50 * on_skeleton(u), adjacent(u), resized_line(u), joined(u));
            let best = open.iter().map(|u| rank(*u)).max().unwrap();
            // First, a relation between units that doesn't hold, none of
            // them placed: the member it keeps stays as it is (if its own
            // constraints hold), so the others come to it (fuzz s8: the
            // dimensioned circle of a concentric pair moves onto the other).
            let own_hold = |u: EntityId| cons.iter().filter(|(_, us)| us.as_slice() == [u]).all(|(c, _)| holds[c]);
            let keep = cons.iter().filter(|(c, us)| us.len() >= 2 && !holds[c] && !(self.connective(*c) && self.aliased(*c)) && !us.iter().any(|x| placed.contains(x)) && !self.chained_to_change(*c, cons, &holds)).find_map(|(c, us)| {
                let k = self.kept_first_in(us, cons, Some(*c));
                let resized = anchored.contains(&k) && self.chained_size(k, cons).is_some_and(|x| (x.value().unwrap_or(0.0) - self.size(k)).abs() > 1e-9);
                // Not one with a relation to placed geometry that doesn't
                // hold (fuzz s218: a side whose end must come onto a kept
                // line can't stay; the pass then closes, s215 / s219 nearer
                // live too).
                let tied = cons.iter().any(|(x, xs)| xs.len() >= 2 && xs.contains(&k) && !holds[x] && xs.iter().any(|y| *y != k && placed.contains(y)));
                (own_hold(k) && !resized && !tied && self.class_keeps(*c, k, cons) && self.direction_agrees(k, cons)).then_some(k)
            });
            // An arc or circle so kept whose Equal chain gives it a new size
            // keeps the end it shares with a neighbour instead: that
            // neighbour stays and the arc is placed next, from it (live
            // fuzz s303 / s354: a slot with a new radius and a new centre
            // distance keeps its free top line and its left arc's centre x).
            let mut forced = None;
            let mut stays = None;
            if let Some(k) = keep {
                let round = matches!(self.kind(k), Some(EntityKind::Arc | EntityKind::Circle));
                let grows = round && self.chained_size(k, cons).is_some_and(|x| (x.value().unwrap_or(0.0) - self.size(k)).abs() > 1e-9);
                let neighbours: BTreeSet<EntityId> = cons.iter().filter(|(c, us)| us.contains(&k) && self.connective(*c)).flat_map(|(_, us)| us.iter().copied()).filter(|x| *x != k && !placed.contains(x)).collect();
                match grows.then(|| self.lower_shared(k, &neighbours)).flatten() {
                    Some((v, _)) => {
                        stays = Some(v);
                        forced = Some(k);
                    }
                    None => {
                        placed.insert(k);
                        kept.insert(k);
                        continue;
                    }
                }
            }
            // A Concentric's kept circle with a new size of its own is placed
            // next (its centre stays), the other comes onto it (live fuzz
            // s386 / s400: washers, the circle at larger coordinates moves).
            // (A pair tied to nothing else: s213's circles, with a distance
            // to a slot, go the loops' way.)
            let alone = |us: &[EntityId]| cons.iter().all(|(_, xs)| xs.iter().all(|x| us.contains(x)) || !xs.iter().any(|x| us.contains(x)));
            let concentric_first = cons.iter().filter(|(c, us)| us.len() == 2 && !holds[c] && !us.iter().any(|x| placed.contains(x)) && alone(us) && matches!(self.constraint(*c).map(|k| &k.kind), Some(ConstraintKind::Concentric { .. }))).find_map(|(c, us)| {
                let k = self.kept_first_in(us, cons, Some(*c));
                (!own_hold(k)).then_some(k)
            });
            let u = if let Some(k) = forced {
                k
            } else if let Some(k) = concentric_first {
                k
            } else if best.0 > 0 {
                // Ties: the member a relation keeps, else the lower one
                // (probe t4: a rectangle's bottom before its right side).
                let tied: Vec<EntityId> = open.iter().copied().filter(|u| rank(*u) == best).collect();
                // One joined to placed geometry before one that isn't (live
                // fuzz s11: the quad's side at the kept side's corner grows
                // to its Equal before the side Equal to the triangle).
                let touching: Vec<EntityId> = tied.iter().copied().filter(|u| cons.iter().any(|(c, us)| us.contains(u) && self.connective(*c) && us.iter().any(|x| placed.contains(x)))).collect();
                let tied = if !touching.is_empty() && touching.len() < tied.len() { touching } else { tied };
                // One whose own size changes before one only carried (live
                // fuzz s556: a rounded-corner rectangle's new height goes up
                // from the bottom before the right side is carried out).
                let resizing: Vec<EntityId> = tied.iter().copied().filter(|u| cons.iter().any(|(c, us)| us.as_slice() == [*u] && !holds[c] && matches!(self.constraint(*c).map(|k| &k.kind), Some(ConstraintKind::Length { .. } | ConstraintKind::Radius { .. } | ConstraintKind::Diameter { .. })))).collect();
                let tied = if !resizing.is_empty() && resizing.len() < tied.len() { resizing } else { tied };
                // One that can be placed keeping its whole shape goes first
                // (fuzz s336: an L-bracket's riser shifts whole before the
                // inner side, which then shortens).
                let whole: Vec<EntityId> = if tied.len() > 1 { tied.iter().copied().filter(|u| self.can_keep_shape(*u, &placed, cons, unit_of)).collect() } else { Vec::new() };
                let pick: Vec<EntityId> = if !whole.is_empty() && whole.len() < tied.len() { whole } else { tied };
                // Then the one placed geometry pulls furthest (fuzz s343: an
                // L-bracket's riser, its top carried 23 mm out, before the
                // inner side its short leg lifts 0.08 mm).
                let pull = |u: EntityId| self.pulled_by(u, &placed, unit_of);
                let far = pick.iter().map(|u| pull(*u)).fold(0.0, f64::max);
                let pick: Vec<EntityId> = if far > 1e-9 && pick.len() > 1 { pick.iter().copied().filter(|u| (pull(*u) - far).abs() <= 1e-9).collect() } else { pick };
                self.kept_first(&pick, cons)
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
            if let Some(v) = stays {
                placed.insert(v);
                kept.insert(v);
            }
            let before: HashMap<EntityId, DVec2> = self.entity(u).map(|e| e.point_refs()).into_iter().flatten().filter_map(|p| self.point(p).map(|q| (p, q))).collect();
            let carried_at = self.carry_onto_placed(u, &placed, unit_of);
            // A line moved at its end keeps its start and turns (live as a
            // line's (start, direction, length): probes 5 u8, fuzz s8); one
            // moved at its start is carried whole first.
            let end_carried = self.kind(u) == Some(EntityKind::Line) && carried_at.is_some() && self.endpoints(u).map(|(_, e)| e) == carried_at;
            // Turned onto a Perpendicular to a placed line: about the two
            // lines' intersection (live probes 5 u4–u6, u8: even off the
            // segment, its length kept); a line's least change then starts
            // there.
            let mut start = before.clone();
            if let Some(turned) = self.turn_perpendicular(u, &placed, cons, &holds) {
                start = turned;
            }
            // Neighbours joined to u (a shared point, a tangency) hanging on
            // u alone follow it; the others are held. Relations to unplaced
            // units (Equal, Parallel, …) wait for those units (fuzz s10: an
            // Equal to the next line didn't hold the line being dimensioned;
            // that line follows the Equal when its turn comes).
            let neighbours: BTreeSet<EntityId> = cons.iter().filter(|(c, us)| us.contains(&u) && self.connective(*c)).flat_map(|(_, us)| us.iter().copied()).filter(|x| *x != u && !placed.contains(x)).collect();
            let held: BTreeSet<EntityId> = neighbours.iter().copied().filter(|v| cons.iter().any(|(_, us)| us.contains(v) && !us.contains(&u))).collect();
            let anchor = if end_carried { self.endpoints(u).map(|(s0, _)| s0) } else { self.anchor(u) };
            let mut done = None;
            // Least change in its own terms: its length and direction kept,
            // then its length only (Parallel turns it, probe p11), then
            // neither; tied neighbours held first, released after (fuzz s5:
            // a side carried by a distance translates, it doesn't tilt).
            // An arc or circle may keep the end it shares with one held
            // neighbour (`lower_shared`; live probes 5 u1 / u3: a slot arc's
            // new radius keeps the bottom line, the top comes down).
            let chained = self.chained_size(u, cons);
            let round = matches!(self.kind(u), Some(EntityKind::Arc | EntityKind::Circle));
            let lower_held = if round { self.lower_shared(u, &held) } else { None };
            // (neighbours held: 0 none, 1 all, 2 the lower one, 3 lines
            // sliding: each keeps its far end and direction, the end it
            // shares with u slides along it; shape)
            // A line carried at its end that its Equal chain resizes keeps
            // its direction, its free start taking the change (live fuzz
            // s11: the quad's side follows its moved corner whole and grows
            // to the triangle's side).
            let resized = chained.as_ref().is_some_and(|k| (k.value().unwrap_or(0.0) - self.size(u)).abs() > 1e-9);
            // An arc between two H / V lines (a rectangle's rounded corner)
            // slides them before it's moved whole (live fuzz s547: a new
            // radius keeps both straight sides).
            let hv = |x: EntityId| self.kind(x) == Some(EntityKind::Line) && cons.iter().any(|(c, us)| us.as_slice() == [x] && matches!(self.constraint(*c).map(|k| &k.kind), Some(ConstraintKind::Horizontal { b: None, .. } | ConstraintKind::Vertical { b: None, .. })));
            let lines_beside: BTreeSet<EntityId> = cons.iter().filter(|(c, us)| us.contains(&u) && self.connective(*c)).flat_map(|(_, us)| us.iter().copied()).filter(|x| *x != u).collect();
            let corner = round && lines_beside.len() == 2 && lines_beside.iter().all(|x| hv(*x));
            let joined_both_ends = self.endpoints(u).is_some_and(|(a, b)| !self.coincident_class(a).is_empty() && !self.coincident_class(b).is_empty());
            let ladder: &[(u8, u8)] = if corner {
                &[(1, 3), (3, 1), (0, 3), (1, 2), (1, 1), (1, 0), (2, 1), (2, 0), (0, 2), (0, 1), (0, 0)]
            } else if end_carried && resized {
                &[(1, 1), (0, 1), (1, 2), (1, 0), (0, 2), (0, 0)]
            } else if lower_held.is_some() {
                &[(1, 3), (0, 3), (1, 2), (1, 1), (1, 0), (2, 1), (2, 0), (0, 2), (0, 1), (3, 1), (0, 0)]
            } else if hv(u) && chained.is_none() && joined_both_ends && !cons.iter().any(|(c, us)| us.as_slice() == [u] && matches!(self.constraint(*c).map(|k| &k.kind), Some(ConstraintKind::Length { .. }))) {
                // An H / V line joined at both ends with no size of its own
                // takes the change, its neighbours staying (live fuzz s622:
                // a T's thinner flange keeps the other underside's level).
                &[(1, 3), (1, 1), (0, 3), (1, 2), (1, 0), (4, 0), (0, 2), (0, 1), (3, 1), (0, 0)]
            } else {
                &[(1, 3), (0, 3), (1, 2), (1, 1), (1, 0), (4, 0), (0, 2), (0, 1), (3, 1), (0, 0)]
            };
            // The lines that slide: the held neighbours, and placed ones
            // joined to u that no Length fixes (live fuzz s548: a rounded
            // corner's new radius shortens the side placed before it).
            let placed_lines: Vec<EntityId> = cons.iter().filter(|(c, us)| us.contains(&u) && self.connective(*c)).flat_map(|(_, us)| us.iter().copied()).filter(|x| *x != u && placed.contains(x) && self.kind(*x) == Some(EntityKind::Line) && !cons.iter().any(|(c, xs)| xs.as_slice() == [*x] && matches!(self.constraint(*c).map(|k| &k.kind), Some(ConstraintKind::Length { .. })))).collect::<BTreeSet<_>>().into_iter().collect();
            let sliding: Vec<EntityId> = held.iter().copied().chain(placed_lines.iter().copied()).collect();
            // Each sliding line's end away from u (none: no slide).
            let far_ends: Option<Vec<(EntityId, EntityId)>> = sliding.iter().map(|n| {
                let (s0, e0) = self.endpoints(*n).filter(|_| self.kind(*n) == Some(EntityKind::Line))?;
                let mine = self.entity(u).map(|e| e.point_refs()).unwrap_or_default();
                let near = |p: EntityId| mine.contains(&p) || self.coincident_class(p).iter().any(|q| mine.contains(q));
                match (near(s0), near(e0)) {
                    (true, false) => Some((*n, e0)),
                    (false, true) => Some((*n, s0)),
                    _ => None,
                }
            }).collect();
            let mut slid: Vec<EntityId> = Vec::new();
            // Slid on a rail (their far end on a resizing partner): moved,
            // not placed: the partner places them.
            let mut rails: Vec<EntityId> = Vec::new();
            // (4: an Equal partner joined to u comes along, the other
            // neighbours held: live fuzz s555, an isosceles triangle's new leg
            // length keeps the base, the apex goes straight up.)
            let partners: Vec<EntityId> = held.iter().copied().filter(|x| cons.iter().any(|(c, us)| us.len() == 2 && us.contains(&u) && us.contains(x) && matches!(self.constraint(*c).map(|k| &k.kind), Some(ConstraintKind::Equal { .. })))).collect();
            // Only a change of its own size slides (live fuzz s465: a 45°
            // chamfer's new length; s466 / s489: a parallelogram side with
            // a new angle turns about its start instead).
            let size_dim = |c: crate::ids::ConstraintId| matches!(self.constraint(c).map(|k| &k.kind), Some(ConstraintKind::Length { .. } | ConstraintKind::Radius { .. } | ConstraintKind::Diameter { .. }));
            let unheld_mine: Vec<&(crate::ids::ConstraintId, Vec<EntityId>)> = cons.iter().filter(|(c, us)| us.contains(&u) && !holds[c]).collect();
            // (A joint to a sliding line may read broken: the slide closes it.)
            let own_size_only = unheld_mine.iter().any(|(c, us)| us.as_slice() == [u] && size_dim(*c))
                && unheld_mine.iter().all(|(c, us)| (us.as_slice() == [u] && size_dim(*c)) || (self.connective(*c) && us.iter().any(|x| placed_lines.contains(x))));
            // (A Concentric's kept circle doesn't hold the other: it comes.)
            // Two such lines meeting, each pulled by placed geometry, are
            // placed together (live fuzz s633: a stepped shaft's step and
            // shoulder both follow their resized neighbours, their corner
            // where those put it).
            let absorbs = |x: EntityId| hv(x) && self.chained_size(x, cons).is_none() && self.endpoints(x).is_some_and(|(a, b)| !self.coincident_class(a).is_empty() && !self.coincident_class(b).is_empty()) && !cons.iter().any(|(c, us)| us.as_slice() == [x] && matches!(self.constraint(*c).map(|k| &k.kind), Some(ConstraintKind::Length { .. })));
            let pulled = |x: EntityId| cons.iter().any(|(c, us)| us.contains(&x) && us.len() > 1 && self.connective(*c) && us.iter().all(|y| *y == x || placed.contains(y)) && !holds[c]);
            let paired = absorbs(u) && pulled(u) && lines_beside.iter().any(|x| !placed.contains(x) && absorbs(*x) && pulled(*x));
            'attempts: for &(hold, shape) in ladder.iter().filter(|(h, _)| (concentric_first.is_none() || *h == 0) && !paired) {
                let hold_neighbours = hold == 1;
                let slide = hold == 3;
                let pair = hold == 4;
                if pair && (partners.is_empty() || !held.iter().any(|x| !partners.contains(x) && cons.iter().any(|(_, us)| us.as_slice() == [*x])) || !cons.iter().any(|(c, us)| us.as_slice() == [u] && !holds[c] && size_dim(*c))) {
                    continue;
                }
                if slide && (sliding.is_empty() || far_ends.is_none() || !own_size_only) {
                    continue;
                }
                let mut fixed = placed.clone();
                if slide {
                    fixed.retain(|x| !placed_lines.contains(x));
                }
                if hold_neighbours {
                    fixed.extend(held.iter().copied());
                }
                if pair {
                    fixed.extend(held.iter().copied().filter(|x| !partners.contains(x)));
                }
                if hold == 2 {
                    fixed.extend(lower_held.map(|(v, _)| v));
                }
                let mut with = fixed.clone();
                with.insert(u);
                if slide {
                    with.extend(sliding.iter().copied());
                }
                if pair {
                    with.extend(partners.iter().copied());
                }
                let keep = |c: crate::ids::ConstraintId, us: &[EntityId]| {
                    us.iter().all(|x| with.contains(x))
                        && if pair { us.iter().any(|x| *x == u || partners.contains(x)) } else { us.contains(&u) && (us.iter().all(|x| *x == u || placed.contains(x)) || self.connective(c)) }
                };
                let Some(mut sub) = self.sub_sketch_by(cons, &keep, &fixed) else { return false };
                if shape == 3 && end_carried {
                    continue;
                }
                // An Equal partner of a dimension that changes takes its new
                // value as the size it keeps (fuzz s321: a filleted
                // rectangle's other corners grow with the dimensioned one).
                let keeps_size = shape == 3 || shape == 2;
                if keeps_size
                    && !match &chained {
                        Some(kind) => sub.add_constraint(kind.clone()).is_ok(),
                        None => orig.hold_size(&mut sub, u),
                    }
                {
                    continue;
                }
                if (shape == 3 || shape == 1) && !orig.hold_direction(&mut sub, u) {
                    continue;
                }
                // Carried whole when it keeps its shape; else a line's least
                // change from where it was (probe t1: a rectangle's top,
                // raised and stretched, keeps its far end's x).
                if shape < 3 && self.kind(u) == Some(EntityKind::Line) {
                    for (p, q) in &start {
                        let _ = sub.set_point(*p, *q);
                    }
                }
                let still = if hold == 2 { lower_held.map(|(_, q)| q) } else { anchor };
                let mut light = vec![u];
                if pair {
                    light.extend(partners.iter().copied());
                }
                let mut railed = Vec::new();
                if slide {
                    for &(n, far) in far_ends.iter().flatten() {
                        // (A far end joined to placed geometry is where that is.)
                        if let Some(q) = self.coincident_class(far).into_iter().find(|q| unit_of.get(q).is_some_and(|x| placed.contains(x))).and_then(|q| self.point(q)) {
                            let _ = sub.set_point(far, q);
                        }
                        // A far end on an unplaced Equal partner of u, which
                        // resizes too, keeps only its level: it slides along a
                        // fixed line (live fuzz s769: a rectangle's two rounded
                        // top corners grow, the top shrinks from both ends).
                        let partner_end = self.coincident_class(far).into_iter().any(|q| {
                            unit_of.get(&q).is_some_and(|x| *x != u && !placed.contains(x) && cons.iter().any(|(c, us)| us.len() == 2 && us.contains(x) && us.contains(&u) && matches!(self.constraint(*c).map(|k| &k.kind), Some(ConstraintKind::Equal { .. }))))
                        });
                        if partner_end {
                            railed.push(n);
                        }
                        let held_far = match (partner_end, sub.curve(n), sub.point(far)) {
                            (true, Some(crate::geom::Curve2::Line { a, b }), Some(q)) => {
                                let d = (b - a).normalize_or_zero();
                                let rail = sub.add_line(q - d, q + d);
                                let _ = sub.set_construction(rail, true);
                                sub.add_constraint(ConstraintKind::Fix { entity: rail }).is_ok() && sub.add_constraint(ConstraintKind::Coincident { a: far, b: rail }).is_ok()
                            }
                            _ => sub.add_constraint(ConstraintKind::Fix { entity: far }).is_ok(),
                        };
                        if !held_far || !orig.hold_direction(&mut sub, n) {
                            continue 'attempts;
                        }
                        light.push(n);
                    }
                }
                let opts = SolveOptions { light, still: still.into_iter().collect(), analyze: false, constructive: false, ..Default::default() };
                let st = sub.solve_with(&opts).status;
                // With its neighbours let go: only when each of them can
                // follow, stretching along itself or carried by a neighbour
                // that can (live fuzz s5: a rectangle's right side moves out
                // whole, the top stretches; s8: a triangle side tied to a
                // turned line turns too, its other end stays; probe t1: the
                // top doesn't move sideways, the left side couldn't follow).
                if hold == 0 && shape >= 1 && matches!(st, SolveStatus::Solved) && !held.is_empty() && !self.neighbours_follow(&sub, u, &held, &before, &placed, cons) {
                    continue;
                }
                if matches!(st, SolveStatus::Solved) {
                    done = Some(sub);
                    if slide {
                        slid = sliding.clone();
                        rails = railed;
                    }
                    if pair {
                        slid = partners.clone();
                    }
                    break 'attempts;
                }
            }
            // A loop one unit can't close: u with its unplaced neighbours
            // (placed geometry still fixed), then with its placed ones too;
            // the change is shared within the group.
            let mut group: Vec<EntityId> = std::iter::once(u).chain(slid).collect();
            if done.is_none() {
                let pin = self.lowest_point(cons.iter().filter(|(c, us)| us.contains(&u) && !holds[c]).filter_map(|(c, _)| self.constraint(*c)).flat_map(|c| c.kind.entities()));
                // An unplaced Equal partner stays out: it comes later, its
                // size from the group's (live fuzz s11: the triangle side
                // Equal to the quad's l7 closes the triangle; l7 is carried
                // by the quad's corner afterwards).
                let size_only = |c: crate::ids::ConstraintId, us: &[EntityId]| matches!(self.constraint(c).map(|k| &k.kind), Some(ConstraintKind::Equal { .. })) && us.iter().any(|x| *x != u && !placed.contains(x));
                let all_neighbours: Vec<EntityId> = cons.iter().filter(|(c, us)| us.contains(&u) && !size_only(*c, us)).flat_map(|(_, us)| us.iter().copied()).filter(|x| *x != u).collect();
                // Placed neighbours without constraints of their own go
                // before the rest (live fuzz s11: an equilateral triangle
                // keeps its dimensioned side and closes on the other two).
                let own = |x: EntityId| cons.iter().any(|(_, us)| us.as_slice() == [x]);
                // A closed loop tied to another loop's placed curve gives way
                // whole first, the other loop staying (live probes 10
                // r6-k26 / r6-k29: a rectangle comes onto the kept one by a
                // point on its top line, an Equal height and a distance),
                // unless the pass kept one of its curves as it was.
                let ring = self.ring_of(u, cons);
                let ring_first = all_neighbours.iter().any(|x| placed.contains(x) && !ring.contains(x)) && self.closed(&ring) && ring.iter().all(|x| !kept.contains(x));
                for release in 0..4 {
                    if release == 1 && !ring_first {
                        continue;
                    }
                    let mut g: BTreeSet<EntityId> = if release == 1 {
                        ring.iter().copied().filter(|x| comp.contains(x)).collect()
                    } else {
                        all_neighbours.iter().copied().filter(|x| !placed.contains(x) || release == 3 || (release == 2 && !own(*x))).collect()
                    };
                    if (release == 1 || release == 2) && g.iter().all(|x| !placed.contains(x)) {
                        continue;
                    }
                    g.insert(u);
                    let fixed: BTreeSet<EntityId> = placed.iter().copied().filter(|x| !g.contains(x)).collect();
                    let mut with = fixed.clone();
                    with.extend(g.iter().copied());
                    let keep = |us: &[EntityId]| us.iter().any(|x| g.contains(x)) && us.iter().all(|x| with.contains(x));
                    let Some(mut sub) = self.sub_sketch(comp, cons, &keep, &fixed) else { return false };
                    // The change's lower point pinned, least change for the
                    // rest (live fuzz s104: a rectangle's diagonal scales it
                    // about its lower-left corner).
                    let still: Vec<EntityId> = match pin {
                        Some(p) => vec![p],
                        None => g.iter().filter_map(|x| self.anchor(*x)).collect(),
                    };
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
                if !rails.contains(&g) {
                    placed.insert(g);
                }
            }
            self.shift_loops_for_distances(cons, &placed);
            let unheld = self.unheld_constraints(1e-9);
            for (c, _) in cons {
                holds.insert(*c, !unheld.contains(c));
            }
        }
        self.touch();
        true
    }

    /// Whether constraint `c` only reads as not holding because a point of
    /// its curves has moved off a point it coincides with (the evaluation
    /// merges coincident points: fuzz s368, a Tangent next to a moved
    /// corner, isn't a reason to keep the arc as it is; a Perpendicular
    /// read that way still is, fuzz s238).
    fn aliased(&self, c: crate::ids::ConstraintId) -> bool {
        let Some(k) = self.constraint(c) else { return false };
        if matches!(k.kind, ConstraintKind::Coincident { .. }) {
            return false;
        }
        k.kind.entities().iter().flat_map(|e| {
            let mut v: Vec<EntityId> = self.entity(*e).map(|x| x.point_refs().into_iter().collect()).unwrap_or_default();
            v.push(*e);
            v
        }).any(|p| self.coincident_class(p).iter().any(|q| match (self.point(p), self.point(*q)) {
            (Some(a), Some(b)) => (a - b).length() > 1e-9,
            _ => false,
        }))
    }

    /// The points `p` coincides with, through chains of Coincidents (the
    /// solver merges them all: live fuzz s485, a diagonal's end on the top
    /// line's start on the moved side's end).
    fn coincident_class(&self, p: EntityId) -> Vec<EntityId> {
        let mut out = vec![p];
        let mut i = 0;
        while i < out.len() {
            for q in self.coincident_with(out[i]) {
                if !out.contains(&q) {
                    out.push(q);
                }
            }
            i += 1;
        }
        out.remove(0);
        out
    }

    /// How far a placed unit's shared point is from `u`'s (the move
    /// carrying `u` onto placed geometry would take).
    fn pulled_by(&self, u: EntityId, placed: &BTreeSet<EntityId>, unit_of: &HashMap<EntityId, EntityId>) -> f64 {
        let Some(own) = self.entity(u).map(|e| e.point_refs()) else { return 0.0 };
        let mut far: f64 = 0.0;
        for &p in own.iter() {
            for q in self.coincident_with(p) {
                if unit_of.get(&q).is_some_and(|x| placed.contains(x))
                    && let (Some(a), Some(b)) = (self.point(p), self.point(q))
                {
                    far = far.max((a - b).length());
                }
            }
        }
        far
    }

    /// Whether `u` could be placed now carried whole (its size and
    /// direction kept) with its neighbours able to follow.
    fn can_keep_shape(&self, u: EntityId, placed: &BTreeSet<EntityId>, cons: &[(crate::ids::ConstraintId, Vec<EntityId>)], unit_of: &HashMap<EntityId, EntityId>) -> bool {
        let mut probe = self.clone();
        let before: HashMap<EntityId, DVec2> = self.entity(u).map(|e| e.point_refs()).into_iter().flatten().filter_map(|p| self.point(p).map(|q| (p, q))).collect();
        probe.carry_onto_placed(u, placed, unit_of);
        let mut with = placed.clone();
        with.insert(u);
        let keep = |c: crate::ids::ConstraintId, us: &[EntityId]| us.contains(&u) && us.iter().all(|x| with.contains(x)) && (us.iter().all(|x| *x == u || placed.contains(x)) || probe.connective(c));
        let Some(mut sub) = probe.sub_sketch_by(cons, &keep, placed) else { return false };
        if !self.hold_size(&mut sub, u) || !self.hold_direction(&mut sub, u) {
            return false;
        }
        let opts = SolveOptions { light: vec![u], still: probe.anchor(u).into_iter().collect(), analyze: false, constructive: false, ..Default::default() };
        if !matches!(sub.solve_with(&opts).status, SolveStatus::Solved) {
            return false;
        }
        let neighbours: BTreeSet<EntityId> = cons.iter().filter(|(c, us)| us.contains(&u) && self.connective(*c)).flat_map(|(_, us)| us.iter().copied()).filter(|x| *x != u && !placed.contains(x)).collect();
        let held: BTreeSet<EntityId> = neighbours.iter().copied().filter(|v| cons.iter().any(|(_, us)| us.contains(v) && !us.contains(&u))).collect();
        probe.neighbours_follow(&sub, u, &held, &before, placed, cons)
    }

    /// Whether each unit of `held` sharing a point `u` moved (from `before`
    /// to `sub`) can follow it: the move runs along its line, or it can be
    /// carried across itself because each other unit at its other points is
    /// unplaced and runs along that part of the move.
    fn neighbours_follow(&self, sub: &Sketch, u: EntityId, held: &BTreeSet<EntityId>, before: &HashMap<EntityId, DVec2>, placed: &BTreeSet<EntityId>, cons: &[(crate::ids::ConstraintId, Vec<EntityId>)]) -> bool {
        let Some(own) = self.entity(u).map(|e| e.point_refs()) else { return false };
        let unit_of_placed = |r: EntityId| placed.iter().any(|x| self.entity(*x).is_some_and(|e| e.point_refs().contains(&r) || e.id == r));
        // A line with a length of its own can't stretch (fuzz s330: an
        // L-bracket's dimensioned short leg doesn't take the riser's move).
        let sized = |v: EntityId| self.constraints().any(|c| c.driving && matches!(c.kind, ConstraintKind::Length { curve, .. } if curve == v));
        let along = |v: EntityId, d: DVec2| d.length() <= 1e-9 || (!sized(v) && self.placing_line_dir(v).filter(|x| x.length() > 0.0).is_some_and(|dir| dir.normalize().perp_dot(d).abs() <= 1e-9));
        // The units sharing point p (through a Coincident), but `not`.
        let sharing = |p: EntityId, not: EntityId| -> Vec<(EntityId, EntityId)> {
            self.constraints()
                .filter(|c| c.driving)
                .filter_map(|c| match c.kind {
                    ConstraintKind::Coincident { a, b } if a == p => Some(b),
                    ConstraintKind::Coincident { a, b } if b == p => Some(a),
                    _ => None,
                })
                .filter_map(|q| self.entities().find(|e| e.id != not && e.point_refs().contains(&q)).map(|e| (e.id, q)))
                .collect()
        };
        for &mine in own.iter() {
            let (Some(p0), Some(p1)) = (before.get(&mine).copied(), sub.point(mine)) else { return false };
            let d = p1 - p0;
            if d.length() <= 1e-9 {
                continue;
            }
            for (v, theirs) in sharing(mine, u) {
                // Free in direction, it follows by turning (probes 5 u8).
                if !held.contains(&v) || along(v, d) || (self.kind(v) == Some(EntityKind::Line) && !self.direction_bound(v)) {
                    continue;
                }
                // Carried across itself (it stretches along): its other
                // points' units stretch along that.
                // An arc or circle follows its own way (live fuzz s17's
                // slot), unless its other end is on placed geometry (fuzz
                // s368: a filleted rectangle's top-left arc on the kept left
                // side: the top stretches, it isn't carried whole).
                let Some(dir) = self.placing_line_dir(v).filter(|x| x.length() > 0.0).map(|x| x.normalize()) else {
                    let pinned = self.entity(v).is_some_and(|e| e.point_refs().iter().filter(|q| **q != theirs).any(|q| self.coincident_with(*q).iter().any(|r| unit_of_placed(*r))));
                    // (One whose size gives way still follows: probes 5 u1,
                    // the left arc shrinks with its Equal partner.)
                    let sized_still = self.constraints().any(|c| c.driving && matches!(c.kind, ConstraintKind::Radius { curve, .. } | ConstraintKind::Diameter { curve, .. } if curve == v))
                        || cons.iter().any(|(c, us)| us.len() == 2 && us.contains(&v) && matches!(self.constraint(*c).map(|k| &k.kind), Some(ConstraintKind::Equal { .. })) && us.iter().any(|w| *w != v && placed.contains(w) && (self.size(*w) - self.size(v)).abs() <= 1e-9));
                    if pinned && sized_still {
                        return false;
                    }
                    continue;
                };
                let t = d - dir * dir.dot(d);
                let Some(vp) = self.entity(v).map(|e| e.point_refs()) else { return false };
                let n = dir.perp();
                for q in vp.iter().copied().filter(|q| *q != theirs) {
                    for (w, wq) in sharing(q, v) {
                        let follows = if w == u {
                            false
                        } else if placed.contains(&w) {
                            // Placed: it already took its end the same way
                            // across (live fuzz s14: a slot's side between
                            // two shrunk arcs).
                            let (Some(a), Some(b)) = (self.point(wq), self.point(q)) else { return false };
                            ((a - b) - t).dot(n).abs() <= 1e-9
                        } else {
                            self.kind(w) != Some(EntityKind::Line) || along(w, t)
                        };
                        if !follows {
                            return false;
                        }
                    }
                }
            }
        }
        true
    }

    /// Whether `k`, the member relation `c` keeps, is the one its whole
    /// class keeps (units joined by Equal, or by Parallel / Perpendicular):
    /// the shortest, the smallest angle in [0°, 180°) (live fuzz s14:
    /// l2 = l4 = l3 all take l3's length, the shortest).
    fn class_keeps(&self, c: crate::ids::ConstraintId, k: EntityId, cons: &[(crate::ids::ConstraintId, Vec<EntityId>)]) -> bool {
        let group = |id: crate::ids::ConstraintId| match self.constraint(id).map(|x| &x.kind) {
            Some(ConstraintKind::Equal { .. }) => 1,
            Some(ConstraintKind::Parallel { .. } | ConstraintKind::Perpendicular { .. }) => 2,
            _ => 0,
        };
        let g = group(c);
        if g == 0 {
            return true;
        }
        let mut class: BTreeSet<EntityId> = BTreeSet::from([k]);
        let mut todo = vec![k];
        while let Some(u) = todo.pop() {
            for (x, us) in cons {
                if us.len() == 2 && us.contains(&u) && group(*x) == g {
                    for v in us {
                        if class.insert(*v) {
                            todo.push(*v);
                        }
                    }
                }
            }
        }
        let key = |u: EntityId| if g == 1 { self.equal_key(u) } else { self.axis_off(u) };
        let kk = key(k);
        !kk.is_finite() || class.iter().all(|u| *u == k || key(*u).partial_cmp(&kk) != Some(std::cmp::Ordering::Less))
    }

    /// Whether line `k`'s direction already agrees with the member its
    /// Parallel / Perpendicular class keeps (the smallest angle in
    /// [0°, 180°), offset 90° per Perpendicular on the way); true off any
    /// such class (live fuzz s14: l3, the shortest of its Equal class, still
    /// turns Parallel to the vertical l8, so it isn't kept as it is).
    fn direction_agrees(&self, k: EntityId, cons: &[(crate::ids::ConstraintId, Vec<EntityId>)]) -> bool {
        use std::f64::consts::{FRAC_PI_2, PI};
        let angle = |u: EntityId| self.placing_line_dir(u).filter(|d| d.length() > 0.0).map(|d| d.y.atan2(d.x).rem_euclid(PI));
        let Some(ak) = angle(k) else { return true };
        let mut class: HashMap<EntityId, f64> = HashMap::from([(k, 0.0)]);
        let mut todo = vec![k];
        while let Some(u) = todo.pop() {
            for (x, us) in cons {
                let off = match self.constraint(*x).map(|c| &c.kind) {
                    Some(ConstraintKind::Parallel { .. }) => 0.0,
                    Some(ConstraintKind::Perpendicular { .. }) => FRAC_PI_2,
                    _ => continue,
                };
                if us.len() != 2 || !us.contains(&u) {
                    continue;
                }
                let v = if us[0] == u { us[1] } else { us[0] };
                let at = class[&u] + off;
                if let std::collections::hash_map::Entry::Vacant(e) = class.entry(v) {
                    e.insert(at);
                    todo.push(v);
                }
            }
        }
        let Some((best, off, _)) = class.iter().filter_map(|(u, off)| angle(*u).map(|a| (a, *off, self.axis_off(*u)))).min_by(|a, b| a.2.total_cmp(&b.2)) else { return true };
        let d = (ak - (best - off)).rem_euclid(PI);
        d.min(PI - d) <= 1e-9
    }

    /// Whether relation `c` reaches, through relations of its kind (Equal
    /// to Equal…), a unit with a constraint of its own that doesn't hold:
    /// its members' values come from there, so neither is kept as it is
    /// (live fuzz s11: l7 = l2 = l1, l1 dimensioned).
    fn chained_to_change(&self, c: crate::ids::ConstraintId, cons: &[(crate::ids::ConstraintId, Vec<EntityId>)], holds: &HashMap<crate::ids::ConstraintId, bool>) -> bool {
        let Some(kind) = self.constraint(c).map(|k| std::mem::discriminant(&k.kind)) else { return false };
        let same = |x: crate::ids::ConstraintId| self.constraint(x).is_some_and(|k| std::mem::discriminant(&k.kind) == kind);
        let Some(start) = cons.iter().find(|(x, _)| *x == c).map(|(_, us)| us.clone()) else { return false };
        let mut seen: BTreeSet<EntityId> = start.iter().copied().collect();
        let members = seen.clone();
        let mut todo = start;
        while let Some(u) = todo.pop() {
            if !members.contains(&u) && cons.iter().any(|(x, us)| us.as_slice() == [u] && !holds[x]) {
                return true;
            }
            for (x, us) in cons {
                if *x != c && us.contains(&u) && us.len() == 2 && same(*x) {
                    for v in us {
                        if seen.insert(*v) {
                            todo.push(*v);
                        }
                    }
                }
            }
        }
        false
    }

    /// Of `held`, the unit an arc or circle `u` keeps its shared end with
    /// (through a Coincident), and that point of u's: one with a dimension
    /// of its own first, then one without a Horizontal / Vertical of its
    /// own, then the lower point (smaller y, then x). Live: fuzz s223 keeps
    /// the end on the dimensioned line; s303 / s307 / s377 a slot's free
    /// top over its Horizontal bottom; s234 (both lines dimensioned), probes
    /// 5 u1 / u3 and s215 (neither) the lower end.
    fn lower_shared(&self, u: EntityId, held: &BTreeSet<EntityId>) -> Option<(EntityId, EntityId)> {
        let own = self.entity(u)?.point_refs();
        let dimensioned = |v: EntityId| self.constraints().any(|c| c.driving && c.kind.is_dimension() && c.kind.entities().as_slice() == [v]);
        let bound = |v: EntityId| self.constraints().any(|c| c.driving && matches!(c.kind, ConstraintKind::Horizontal { a, b: None } | ConstraintKind::Vertical { a, b: None } if a == v));
        let mut ends: Vec<(EntityId, EntityId, DVec2, bool, bool)> = own
            .iter()
            .flat_map(|p| self.coincident_with(*p).into_iter().map(move |q| (*p, q)))
            .filter_map(|(p, q)| held.iter().find(|v| self.entity(**v).is_some_and(|e| e.point_refs().contains(&q))).map(|v| (*v, p)))
            .filter_map(|(v, p)| self.point(p).map(|x| (v, p, x, dimensioned(v), bound(v))))
            .collect();
        ends.sort_by(|a, b| b.3.cmp(&a.3).then(a.4.cmp(&b.4)).then(if (a.2.y - b.2.y).abs() > 1e-9 { a.2.y.total_cmp(&b.2.y) } else { a.2.x.total_cmp(&b.2.x) }));
        // Two alike at one level: neither (live fuzz s710: a U bend's arc
        // grows about its centre, both legs move out).
        if let [a, b, ..] = ends.as_slice()
            && a.0 != b.0
            && (a.3, a.4) == (b.3, b.4)
            && (a.2.y - b.2.y).abs() <= 1e-9
        {
            return None;
        }
        ends.first().map(|(v, p, _, _, _)| (*v, *p))
    }

    /// A free component whose one change is the distance between an arc's
    /// own ends (its chord): scaled whole about the arc's center (live
    /// probes 5 u2, fuzz s17: a slot by 0.8 about the right arc's center),
    /// where a radius would only resize it. Only when nothing else in it is
    /// a dimension or fixed (everything else holds at any scale).
    fn scale_for_chord(&mut self, comp: &[EntityId], cons: &[(crate::ids::ConstraintId, Vec<EntityId>)]) -> bool {
        let unheld = self.unheld_constraints(1e-9);
        let changes: Vec<&(crate::ids::ConstraintId, Vec<EntityId>)> = cons.iter().filter(|(c, _)| unheld.contains(c)).collect();
        let [(cid, us)] = changes.as_slice() else { return false };
        let ([u], Some(c)) = (us.as_slice(), self.constraint(*cid)) else { return false };
        let ConstraintKind::Distance { a, b, value, .. } = c.kind else { return false };
        if self.kind(*u) != Some(EntityKind::Arc) {
            return false;
        }
        let (Some(center), Some(pa), Some(pb)) = (self.entity(*u).and_then(|e| e.center()), self.point(a), self.point(b)) else { return false };
        let Some(o) = self.point(center) else { return false };
        let now = (pb - pa).length();
        if now < 1e-12 || value <= 0.0 {
            return false;
        }
        let set: BTreeSet<EntityId> = comp.iter().copied().collect();
        let fixed_or_sized = self.constraints().filter(|k| k.driving && k.id != *cid).any(|k| {
            let touches = cons.iter().any(|(x, us)| *x == k.id && us.iter().any(|v| set.contains(v)));
            touches && (k.kind.is_dimension() || matches!(k.kind, ConstraintKind::Fix { .. }))
        });
        if fixed_or_sized || comp.iter().any(|v| self.entity(*v).is_some_and(|e| e.external)) {
            return false;
        }
        let k = value / now;
        let mut pts: BTreeSet<EntityId> = BTreeSet::new();
        for v in comp {
            let Some(e) = self.entity(*v) else { return false };
            let refs: Vec<EntityId> = e.point_refs().into_iter().collect();
            if refs.is_empty() && e.kind() == EntityKind::Point {
                pts.insert(*v);
            }
            pts.extend(refs);
        }
        for p in pts {
            if let Some(x) = self.point(p) {
                let _ = self.set_point(p, o + (x - o) * k);
            }
        }
        for v in comp {
            if let Some(e) = self.entity_mut_silent(*v)
                && let Geometry::Circle { radius, .. } = &mut e.geometry
            {
                *radius *= k;
            }
        }
        self.touch();
        true
    }

    /// Whether a driving constraint fixes `v`'s direction (Horizontal,
    /// Vertical, Fix, Angle, Parallel, Perpendicular).
    fn direction_bound(&self, v: EntityId) -> bool {
        self.constraints().filter(|c| c.driving).any(|c| match c.kind {
            ConstraintKind::Horizontal { a, b: None } | ConstraintKind::Vertical { a, b: None } | ConstraintKind::Fix { entity: a } => a == v,
            ConstraintKind::Angle { a, b, .. } | ConstraintKind::Parallel { a, b } | ConstraintKind::Perpendicular { a, b } => a == v || b == v,
            _ => false,
        })
    }

    /// Turns line `u` about the intersection of its line with a placed
    /// line it must be Perpendicular to (a pending one that doesn't hold),
    /// the shortest way; its points' new positions, or None.
    fn turn_perpendicular(&mut self, u: EntityId, placed: &BTreeSet<EntityId>, cons: &[(crate::ids::ConstraintId, Vec<EntityId>)], holds: &HashMap<crate::ids::ConstraintId, bool>) -> Option<HashMap<EntityId, DVec2>> {
        let (s0, e0) = self.endpoints(u)?;
        if self.kind(u) != Some(EntityKind::Line) {
            return None;
        }
        let w = cons.iter().find_map(|(c, us)| {
            if holds[c] || us.len() != 2 || !us.contains(&u) {
                return None;
            }
            let ConstraintKind::Perpendicular { a, b } = self.constraint(*c)?.kind else { return None };
            let w = if a == u { b } else { a };
            (placed.contains(&w) && self.kind(w) == Some(EntityKind::Line)).then_some(w)
        })?;
        let (p, q) = (self.point(s0)?, self.point(e0)?);
        let (ws, we) = self.endpoints(w)?;
        let (wp, wq) = (self.point(ws)?, self.point(we)?);
        let (d, wd) = (q - p, wq - wp);
        let den = d.perp_dot(wd);
        if den.abs() < 1e-12 * d.length() * wd.length() {
            return None;
        }
        let pivot = p + d * ((wp - p).perp_dot(wd) / den);
        // The shortest turn onto either normal of w.
        let n = wd.perp();
        let a = d.y.atan2(d.x);
        let turn = [n, -n].iter().map(|m| (m.y.atan2(m.x) - a + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI).min_by(|x, y| x.abs().total_cmp(&y.abs()))?;
        let rot = DVec2::from_angle(turn);
        let mut out = HashMap::new();
        for pt in [s0, e0] {
            let x = pivot + rot.rotate(self.point(pt)? - pivot);
            self.set_point(pt, x).ok()?;
            out.insert(pt, x);
        }
        Some(out)
    }

    /// Holds a line's direction in `sub` (parallel to a fixed construction
    /// line along it); false for other curves.
    fn hold_direction(&self, sub: &mut Sketch, u: EntityId) -> bool {
        // An arc: the directions from its center to its ends (live fuzz
        // s104: a dimensioned quarter arc stays a quarter arc, scaled about
        // the end it shares).
        if let Some(Geometry::Arc { center, start, end, .. }) = self.entity(u).map(|e| e.geometry.clone()) {
            let Some(c) = self.point(center) else { return false };
            for p in [start, end] {
                let Some(q) = self.point(p).filter(|q| (*q - c).length() > 0.0) else { return false };
                let Ok(spoke) = sub.add_line_between(center, p) else { return false };
                let r = sub.add_line(c, q);
                let _ = sub.set_construction(spoke, true);
                let _ = sub.set_construction(r, true);
                if sub.add_constraint(ConstraintKind::Fix { entity: r }).is_err() || sub.add_constraint(ConstraintKind::Parallel { a: spoke, b: r }).is_err() {
                    return false;
                }
            }
            return true;
        }
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

    /// For each loop of `comp` that changes only within itself (its sizes,
    /// relations between its own curves): its leftmost unit that doesn't
    /// change, kept as it is (live fuzz s321: a filleted rectangle keeps
    /// its left side and grows round from it).
    fn loop_anchors(&self, comp: &[EntityId], cons: &[(crate::ids::ConstraintId, Vec<EntityId>)], holds: &HashMap<crate::ids::ConstraintId, bool>) -> Vec<EntityId> {
        let mut seen: BTreeSet<EntityId> = BTreeSet::new();
        let mut out = Vec::new();
        for &u in comp {
            if seen.contains(&u) {
                continue;
            }
            let lp = self.loop_of(u, cons);
            seen.extend(lp.iter().copied());
            // Closed (an open chain keeps its usual order: fuzz s223).
            let closed = self.closed(&lp);
            // With arcs (a plain rectangle keeps its lower-left corner as
            // before: fuzz s319's corner rectangle, only its top Horizontal).
            let rounded = lp.iter().any(|v| self.kind(*v) == Some(EntityKind::Arc));
            if lp.len() < 3 || !closed || !rounded {
                continue;
            }
            let touching: Vec<&(crate::ids::ConstraintId, Vec<EntityId>)> = cons.iter().filter(|(_, us)| us.iter().any(|x| lp.contains(x))).collect();
            if !touching.iter().any(|(c, _)| !holds[c]) || touching.iter().any(|(c, us)| !holds[c] && us.iter().any(|x| !lp.contains(x))) {
                continue;
            }
            // Not a line running like one whose length changes (probe r5:
            // a rectangle's bottom must lengthen with its dimensioned top).
            let hv = |v: EntityId| self.constraints().find_map(|c| match c.kind {
                ConstraintKind::Horizontal { a, b: None } if a == v => Some(0),
                ConstraintKind::Vertical { a, b: None } if a == v => Some(1),
                _ => None,
            });
            let resizing: Vec<EntityId> = lp.iter().copied().filter(|v| cons.iter().any(|(c, us)| us.as_slice() == [*v] && !holds[c] && matches!(self.constraint(*c).map(|k| &k.kind), Some(ConstraintKind::Length { .. })))).collect();
            let own_length = |v: EntityId| cons.iter().any(|(c, us)| us.as_slice() == [v] && matches!(self.constraint(*c).map(|k| &k.kind), Some(ConstraintKind::Length { .. })));
            let alongside = |v: EntityId| !own_length(v) && hv(v).is_some_and(|k| resizing.iter().any(|w| *w != v && hv(*w) == Some(k)))
                || cons.iter().any(|(c, us)| us.len() == 2 && us.contains(&v) && matches!(self.constraint(*c).map(|k| &k.kind), Some(ConstraintKind::Parallel { .. })) && us.iter().any(|w| *w != v && resizing.contains(w)));
            // A Vertical line (fuzz s222: a slot's free top line isn't
            // kept; s377: nor a slot's Horizontal bottom, its free top is).
            let steady = |v: EntityId| hv(v) == Some(1) && cons.iter().filter(|(_, us)| us.contains(&v)).all(|(c, _)| holds[c]) && self.chained_size(v, cons).is_none() && !alongside(v);
            let lower = |v: EntityId| self.anchor(v).and_then(|p| self.point(p)).unwrap_or(DVec2::splat(f64::MAX));
            // Only the leftmost Vertical line: when it resizes, the loop keeps
            // its lower-left corner as a plain one does (live fuzz s662 /
            // s680: an L-bracket with a filleted inner corner, its height
            // and width changed, didn't keep the short right side).
            let leftmost = lp.iter().copied().filter(|v| hv(*v) == Some(1)).min_by(|a, b| {
                let (p, q) = (lower(*a), lower(*b));
                (p.x, p.y).partial_cmp(&(q.x, q.y)).unwrap_or(std::cmp::Ordering::Equal)
            });
            if leftmost.is_some_and(|v| !steady(v)) {
                continue;
            }
            if let Some(a) = lp.iter().copied().filter(|v| steady(*v)).min_by(|a, b| {
                let (p, q) = (lower(*a), lower(*b));
                (p.x, p.y).partial_cmp(&(q.x, q.y)).unwrap_or(std::cmp::Ordering::Equal)
            }) {
                out.push(a);
            }
        }
        out
    }

    /// The size `u` takes from a dimension on a curve it is Equal to
    /// (through Equal relations), as a constraint on `u`: a line's Length,
    /// an arc's or circle's Radius.
    fn chained_size(&self, u: EntityId, cons: &[(crate::ids::ConstraintId, Vec<EntityId>)]) -> Option<ConstraintKind> {
        let mut seen: BTreeSet<EntityId> = BTreeSet::from([u]);
        let mut todo = vec![u];
        while let Some(x) = todo.pop() {
            if x != u {
                for (c, us) in cons {
                    if us.as_slice() != [x] {
                        continue;
                    }
                    let value = match self.constraint(*c).map(|k| &k.kind) {
                        Some(ConstraintKind::Length { value, .. }) => *value,
                        Some(ConstraintKind::Radius { value, .. }) => *value,
                        Some(ConstraintKind::Diameter { value, .. }) => *value / 2.0,
                        _ => continue,
                    };
                    return match self.kind(u) {
                        Some(EntityKind::Line) if self.kind(x) == Some(EntityKind::Line) => Some(ConstraintKind::Length { curve: u, value }),
                        Some(EntityKind::Arc | EntityKind::Circle) if matches!(self.kind(x), Some(EntityKind::Arc | EntityKind::Circle)) => Some(ConstraintKind::Radius { curve: u, value }),
                        _ => None,
                    };
                }
            }
            for (c, us) in cons {
                if us.len() == 2 && us.contains(&x) && matches!(self.constraint(*c).map(|k| &k.kind), Some(ConstraintKind::Equal { .. })) {
                    for v in us {
                        if seen.insert(*v) {
                            todo.push(*v);
                        }
                    }
                }
            }
        }
        None
    }

    /// Of tied units, the one a relation between two of them keeps (Equal:
    /// see `equal_key`; Parallel / Perpendicular: the smaller angle in [0°, 180°);
    /// Concentric: the lower center); else the one with the lower point.
    fn kept_first(&self, tied: &[EntityId], cons: &[(crate::ids::ConstraintId, Vec<EntityId>)]) -> EntityId {
        self.kept_first_in(tied, cons, None)
    }

    /// [`Sketch::kept_first`] decided by relation `only` alone (then the
    /// lower point), when given (fuzz s309: a slot's centre distance keeps
    /// the left arc, not the one the arcs' Equal would keep).
    fn kept_first_in(&self, tied: &[EntityId], cons: &[(crate::ids::ConstraintId, Vec<EntityId>)], only: Option<crate::ids::ConstraintId>) -> EntityId {
        let lower = |u: EntityId| self.anchor(u).and_then(|p| self.point(p)).unwrap_or(DVec2::splat(f64::MAX));
        for (cid, us) in cons {
            if us.len() != 2 || !us.iter().all(|u| tied.contains(u)) || only.is_some_and(|o| o != *cid) {
                continue;
            }
            let Some(c) = self.constraint(*cid) else { continue };
            let key = |u: EntityId| -> f64 {
                match c.kind {
                    ConstraintKind::Equal { .. } => self.equal_key(u),
                    // The angle in [0°, 180°) (live fuzz s14: a vertical line
                    // stays, the one at 97° turns).
                    ConstraintKind::Parallel { .. } | ConstraintKind::Perpendicular { .. } => self.axis_off(u),
                    _ => f64::NAN,
                }
            };
            let (a, b) = (us[0], us[1]);
            // A member whose own constraints fix what the relation relates
            // stays (live fuzz s8: a line Parallel to a Vertical one turns).
            let fixes = |u: EntityId| {
                self.constraints().filter(|o| o.driving && o.id != *cid).any(|o| match (&c.kind, &o.kind) {
                    (ConstraintKind::Parallel { .. } | ConstraintKind::Perpendicular { .. }, ConstraintKind::Horizontal { a, b: None } | ConstraintKind::Vertical { a, b: None }) => *a == u,
                    (ConstraintKind::Equal { .. }, ConstraintKind::Length { curve, .. } | ConstraintKind::Radius { curve, .. } | ConstraintKind::Diameter { curve, .. }) => *curve == u,
                    _ => false,
                })
            };
            let (fa, fb) = (fixes(a), fixes(b));
            if fa != fb {
                return if fa { a } else { b };
            }
            // A distance between points of two separate loops: the loop
            // with more points stays; between equal ones, the one at the
            // smaller coordinate along the distance (live probes 6 w1–w10:
            // a lone line moves before a square on either side, the right /
            // upper of two squares moves; fuzz s226, s236, s215).
            if let ConstraintKind::Distance { a: pa, b: pb, .. } = c.kind
                && self.kind(pa) == Some(EntityKind::Point)
                && self.kind(pb) == Some(EntityKind::Point)
                && !self.joined_by_points(a, b, cons)
                && let (Some(qa), Some(qb)) = (self.point(pa), self.point(pb))
            {
                let owns = |u: EntityId, p: EntityId| self.entity(u).is_some_and(|e| e.point_refs().contains(&p)) || self.coincident_with(p).iter().any(|q| self.entity(u).is_some_and(|e| e.point_refs().contains(q)));
                if owns(a, pa) != owns(a, pb) {
                    let (na, nb) = (self.loop_points(a, cons), self.loop_points(b, cons));
                    if na != nb {
                        return if na > nb { a } else { b };
                    }
                    // Then the one more tied to other loops (fuzz s328: a
                    // bolt circle's big circle, three centre distances,
                    // stays; its holes move out).
                    let ties = |u: EntityId| {
                        let lp = self.loop_of(u, cons);
                        cons.iter().filter(|(_, us)| us.len() >= 2 && us.iter().any(|x| lp.contains(x)) && us.iter().any(|x| !lp.contains(x))).count()
                    };
                    let (ra, rb) = (ties(a), ties(b));
                    if ra != rb {
                        return if ra > rb { a } else { b };
                    }
                    // Along the distance: a's point against b's.
                    let (qa_own, qb_own) = if owns(a, pa) { (qa, qb) } else { (qb, qa) };
                    let d = qb_own - qa_own;
                    let along = if d.x.abs() >= d.y.abs() { d.x } else { d.y };
                    if along.abs() > 1e-12 {
                        return if along > 0.0 { a } else { b };
                    }
                }
            }
            // A distance from a point of one to the other line within one
            // loop: the point's member stays (live fuzz s711 / s713: an isosceles
            // trapezoid's new height, dimensioned from its top, keeps the
            // top; s240 scales about the Distance's first point too).
            if let ConstraintKind::Distance { a: pa, b: pb, .. } = c.kind
                && self.kind(pa) == Some(EntityKind::Point)
                && self.kind(pb) == Some(EntityKind::Line)
                && self.joined_by_points(a, b, cons)
            {
                // The member with a Length of its own stays first (live
                // probes 14 q5 / q6 / q8, fuzz s747: a C channel's new web
                // keeps the dimensioned outer side, whichever way the
                // Distance runs; a rectangle's height its dimensioned bottom).
                let sized = |u: EntityId| self.constraints().any(|o| o.driving && matches!(o.kind, ConstraintKind::Length { curve, .. } if curve == u));
                if sized(a) != sized(b) {
                    return if sized(a) { a } else { b };
                }
                // Then the shorter line (live probes 15: a trapezoid keeps
                // its shorter side, upright or inverted, drawn either way
                // round, whichever way the Distance runs).
                let (la, lb) = (self.size(a), self.size(b));
                if self.kind(a) == Some(EntityKind::Line) && self.kind(b) == Some(EntityKind::Line) && (la - lb).abs() > 1e-9 {
                    return if la < lb { a } else { b };
                }
                let owns = |u: EntityId| self.entity(u).is_some_and(|e| e.point_refs().contains(&pa)) || self.coincident_with(pa).iter().any(|q| self.entity(u).is_some_and(|e| e.point_refs().contains(q)));
                if owns(a) != owns(b) {
                    return if owns(a) { a } else { b };
                }
            }
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

    /// A T's stem bottom with a new Length (its sides lead on to Equal
    /// overhangs): it keeps the end at the side whose height is dimensioned
    /// (live probes 17 u1-u5, fuzz s838 / s841 / s857); None otherwise.
    fn stem_anchor(&self, u: EntityId, cons: &[(crate::ids::ConstraintId, Vec<EntityId>)], unit_of: &HashMap<EntityId, EntityId>, holds: &HashMap<crate::ids::ConstraintId, bool>) -> Option<EntityId> {
        let length_of = |x: EntityId| cons.iter().find(|(c, us)| us.as_slice() == [x] && matches!(self.constraint(*c).map(|k| &k.kind), Some(ConstraintKind::Length { .. }))).map(|(c, _)| *c);
        if self.kind(u) != Some(EntityKind::Line) || length_of(u).is_none_or(|c| holds[&c]) {
            return None;
        }
        let (s0, e0) = self.endpoints(u)?;
        let next = |p: EntityId, not: EntityId| -> Option<(EntityId, EntityId)> {
            let n = self.coincident_class(p).into_iter().find_map(|q| unit_of.get(&q).copied().filter(|x| *x != not && self.kind(*x) == Some(EntityKind::Line)))?;
            let (a, b) = self.endpoints(n)?;
            let far = if self.coincident_class(p).contains(&a) || a == p { b } else { a };
            Some((n, far))
        };
        let (ns, fs) = next(s0, u)?;
        let (ne, fe) = next(e0, u)?;
        let (os, os_far) = next(fs, ns)?;
        let (oe, oe_far) = next(fe, ne)?;
        let equal = cons.iter().any(|(c, us)| us.len() == 2 && us.contains(&os) && us.contains(&oe) && matches!(self.constraint(*c).map(|k| &k.kind), Some(ConstraintKind::Equal { .. })));
        // The overhangs point away from the stem (a flange's top has them
        // pointing back under it).
        let (a, b) = (self.point(s0)?, self.point(e0)?);
        let (mid, dir) = ((a + b) / 2.0, (b - a).normalize_or_zero());
        let along = |p: EntityId| self.point(p).map(|q| (q - mid).dot(dir).abs());
        let outward = |joint: EntityId, tip: EntityId| along(tip).zip(along(joint)).is_some_and(|(t, j)| t > j + 1e-9);
        if !equal || !outward(fs, os_far) || !outward(fe, oe_far) {
            return None;
        }
        match (length_of(ns).is_some(), length_of(ne).is_some()) {
            (true, false) => Some(s0),
            (false, true) => Some(e0),
            _ => None,
        }
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

    /// Of these entities' points (a curve: its anchor), the lowest
    /// (smaller x, then y).
    fn lowest_point(&self, ids: impl Iterator<Item = EntityId>) -> Option<EntityId> {
        ids.filter_map(|id| if self.kind(id) == Some(EntityKind::Point) { Some(id) } else { self.anchor(id) })
            .filter_map(|p| self.point(p).map(|q| (p, q)))
            .min_by(|a, b| (a.1.x, a.1.y).partial_cmp(&(b.1.x, b.1.y)).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(p, _)| p)
    }

    /// How far line `u` is turned from the nearest axis (0 to 45°): of
    /// lines related by Parallel / Perpendicular the one nearer an axis
    /// stays (live: fuzz s102 turns the side 10.8° off horizontal, not the
    /// one 7.2° off vertical; s14 keeps a vertical line; probes 2 q7 / q8,
    /// 5 u6).
    /// Ties (both on an axis: fuzz s238) go to the smaller angle in
    /// [0°, 180°).
    fn axis_off(&self, u: EntityId) -> f64 {
        use std::f64::consts::{FRAC_PI_2, PI};
        self.placing_line_dir(u).filter(|d| d.length() > 0.0).map_or(f64::MAX, |d| {
            let raw = d.y.atan2(d.x).rem_euclid(PI);
            let a = raw.rem_euclid(FRAC_PI_2);
            (a.min(FRAC_PI_2 - a) * 1e9).round() * 1e-6 + raw
        })
    }

    /// Of Equal members, the one with the smaller key stays: a line's
    /// largest x (its end that would move: live probes 1–2 shrink the 20 mm
    /// line ending further right, probes 5 u9 / u10 grow the shorter side of
    /// a quad whose free end is further right, fuzz s14's chain keeps its
    /// leftmost side); other curves their size.
    fn equal_key(&self, u: EntityId) -> f64 {
        match self.kind(u) {
            Some(EntityKind::Line) => self.entity(u).map_or(f64::NAN, |e| e.point_refs().iter().filter_map(|p| self.point(*p)).map(|q| q.x).fold(f64::NEG_INFINITY, f64::max)),
            _ => self.size(u),
        }
    }

    /// A distance between points of two separate loops that doesn't hold
    /// moves the loop `kept_first` doesn't keep, whole, along the distance
    /// (live probes 6: the upper square shifts up, it doesn't stretch; w6:
    /// shifted first, its own new size after). Not a loop with fixed or
    /// external geometry, nor one other distances tie elsewhere. Called
    /// before and after each placement (`placed`).
    fn shift_loops_for_distances(&mut self, cons: &[(crate::ids::ConstraintId, Vec<EntityId>)], placed: &BTreeSet<EntityId>) {
        let unheld = self.unheld_constraints(1e-9);
        for (cid, us) in cons {
            let [a, b] = us.as_slice() else { continue };
            if !unheld.contains(cid) {
                continue;
            }
            let Some(ConstraintKind::Distance { a: pa, b: pb, value, .. }) = self.constraint(*cid).map(|c| c.kind.clone()) else { continue };
            if self.kind(pa) != Some(EntityKind::Point) || self.kind(pb) != Some(EntityKind::Point) || self.joined_by_points(*a, *b, cons) {
                continue;
            }
            let kept = self.kept_first(&[*a, *b], cons);
            let moving = if kept == *a { *b } else { *a };
            let group = self.loop_of(moving, cons);
            // Once the kept loop is settled (all placed, or nothing in it
            // to change) and before any of the moving one is placed (live
            // probes 6 w7: the left square grows first, the right one then
            // shifts to the distance from its new corner).
            let kept_loop = self.loop_of(kept, cons);
            let settled = kept_loop.iter().all(|u| placed.contains(u)) || !cons.iter().any(|(x, xs)| x != cid && unheld.contains(x) && xs.iter().any(|u| kept_loop.contains(u)));
            if !(settled || placed.is_empty()) || group.iter().any(|u| placed.contains(u)) {
                continue;
            }
            let owns = |s: &Sketch, p: EntityId| group.iter().any(|u| s.entity(*u).is_some_and(|e| e.point_refs().contains(&p)) || s.coincident_with(p).iter().any(|q| s.entity(*u).is_some_and(|e| e.point_refs().contains(q))));
            let (mine, theirs) = if owns(self, pa) && !owns(self, pb) { (pa, pb) } else if owns(self, pb) && !owns(self, pa) { (pb, pa) } else { continue };
            // Nothing fixed in it; other relations out of it only to the
            // kept loop: then it is placed rigidly onto all of them (live
            // fuzz s215: a triangle with two distances and an angle to a
            // slot turns and shifts whole), else shifted.
            let fixed = self.constraints().filter(|c| c.driving).any(|c| matches!(c.kind, ConstraintKind::Fix { .. }) && c.kind.entities().iter().any(|e| owns(self, *e) || group.contains(e)));
            if fixed || group.iter().any(|u| self.entity(*u).is_some_and(|e| e.external)) {
                continue;
            }
            let outward: Vec<(crate::ids::ConstraintId, Vec<EntityId>)> = cons.iter().filter(|(x, xs)| x != cid && xs.iter().any(|u| group.contains(u)) && xs.iter().any(|u| !group.contains(u))).cloned().collect();
            let elsewhere = |o: &(crate::ids::ConstraintId, Vec<EntityId>)| o.1.iter().any(|u| !group.contains(u) && !kept_loop.contains(u));
            let is_distance = |o: &(crate::ids::ConstraintId, Vec<EntityId>)| matches!(self.constraint(o.0).map(|k| &k.kind), Some(ConstraintKind::Distance { .. } | ConstraintKind::HorizontalDistance { .. } | ConstraintKind::VerticalDistance { .. }));
            if outward.iter().any(|o| elsewhere(o) && is_distance(o)) {
                continue;
            }
            if !outward.is_empty() && !outward.iter().any(elsewhere) {
                self.place_loop_rigidly(&group, &kept_loop, cons);
                continue;
            }
            if outward.iter().any(|o| !elsewhere(o) && is_distance(o)) {
                continue;
            }
            let (Some(m), Some(t)) = (self.point(mine), self.point(theirs)) else { continue };
            let d = m - t;
            if d.length() < 1e-12 || value <= 0.0 {
                continue;
            }
            let shift = d.normalize() * (value - d.length());
            let pts: BTreeSet<EntityId> = group.iter().flat_map(|u| {
                let mut ps: Vec<EntityId> = self.entity(*u).map(|e| e.point_refs().into_iter().collect()).unwrap_or_default();
                if ps.is_empty() {
                    ps.push(*u);
                }
                ps
            }).collect();
            for p in pts {
                if let Some(x) = self.point(p) {
                    let _ = self.set_point(p, x + shift);
                }
            }
        }
    }

    /// Moves loop `group` as one rigid piece (its shape held by distances
    /// from two of its vertices) onto its relations to the fixed loop
    /// `kept`; left as it was when that doesn't solve.
    fn place_loop_rigidly(&mut self, group: &BTreeSet<EntityId>, kept: &BTreeSet<EntityId>, cons: &[(crate::ids::ConstraintId, Vec<EntityId>)]) {
        let mut sub = self.clone();
        let points_of = |s: &Sketch, us: &BTreeSet<EntityId>| -> Vec<EntityId> {
            let mut v: Vec<EntityId> = us.iter().flat_map(|u| {
                let mut ps: Vec<EntityId> = s.entity(*u).map(|e| e.point_refs().into_iter().collect()).unwrap_or_default();
                if ps.is_empty() {
                    ps.push(*u);
                }
                ps
            }).collect();
            v.sort_unstable();
            v.dedup();
            v
        };
        // Relations between the two loops, the loop's shared points.
        let keep: HashSet<crate::ids::ConstraintId> = cons
            .iter()
            .filter(|(c, xs)| {
                let inter = xs.iter().any(|u| group.contains(u)) && xs.iter().all(|u| group.contains(u) || kept.contains(u)) && !xs.iter().all(|u| group.contains(u));
                let joint = xs.iter().all(|u| group.contains(u)) && matches!(self.constraint(*c).map(|k| &k.kind), Some(ConstraintKind::Coincident { .. }));
                inter || joint
            })
            .map(|(c, _)| *c)
            .collect();
        let drop: Vec<crate::ids::ConstraintId> = sub.constraints().map(|c| c.id).filter(|c| !keep.contains(c)).collect();
        for c in drop {
            let _ = sub.remove_constraint(c);
        }
        for p in points_of(self, kept) {
            let _ = sub.add_constraint(ConstraintKind::Fix { entity: p });
        }
        // The loop held rigid: its distinct vertices at their distances from
        // two of them (and circles their radii).
        let mut reps: Vec<(EntityId, DVec2)> = Vec::new();
        for p in points_of(self, group) {
            let Some(x) = self.point(p) else { continue };
            if self.kind(p) == Some(EntityKind::Point) && !reps.iter().any(|(_, y)| (*y - x).length() < 1e-9) {
                reps.push((p, x));
            }
        }
        for u in group {
            if self.kind(*u) == Some(EntityKind::Circle) && !self.hold_size(&mut sub, *u) {
                return;
            }
        }
        if reps.len() >= 2 {
            let (a0, x0) = reps[0];
            let Some(&(a1, x1)) = reps.iter().max_by(|a, b| (a.1 - x0).length().total_cmp(&(b.1 - x0).length())) else { return };
            let dist = |s: &mut Sketch, p: EntityId, q: EntityId, d: f64| s.add_constraint(ConstraintKind::Distance { a: p, b: q, value: d, mode: crate::constraints::DistanceMode::Center }).is_ok();
            if !dist(&mut sub, a0, a1, (x1 - x0).length()) {
                return;
            }
            for &(v, x) in &reps {
                if v != a0 && v != a1 && !(dist(&mut sub, a0, v, (x - x0).length()) && dist(&mut sub, a1, v, (x - x1).length())) {
                    return;
                }
            }
        }
        let opts = SolveOptions { light: group.iter().copied().collect(), analyze: false, constructive: false, ..Default::default() };
        if !matches!(sub.solve_with(&opts).status, SolveStatus::Solved) {
            return;
        }
        for p in points_of(self, group) {
            if let Some(x) = sub.point(p) {
                let _ = self.set_point(p, x);
            }
        }
    }

    /// Whether units `a` and `b` are in one loop / chain of shared points
    /// (joined through joining constraints within `cons`).
    fn joined_by_points(&self, a: EntityId, b: EntityId, cons: &[(crate::ids::ConstraintId, Vec<EntityId>)]) -> bool {
        self.loop_of(a, cons).contains(&b)
    }

    /// The units joined to `u` by shared points (joining constraints).
    fn loop_of(&self, u: EntityId, cons: &[(crate::ids::ConstraintId, Vec<EntityId>)]) -> BTreeSet<EntityId> {
        let mut seen: BTreeSet<EntityId> = BTreeSet::from([u]);
        let mut todo = vec![u];
        while let Some(x) = todo.pop() {
            for (c, us) in cons {
                if us.contains(&x) && self.connective(*c) {
                    for v in us {
                        if seen.insert(*v) {
                            todo.push(*v);
                        }
                    }
                }
            }
        }
        seen
    }

    /// Whether every curve end of `lp` is shared with another of its curves.
    fn closed(&self, lp: &BTreeSet<EntityId>) -> bool {
        lp.iter().all(|v| match self.endpoints(*v) {
            Some((a, b)) => [a, b].iter().all(|p| self.coincident_with(*p).iter().any(|q| lp.iter().any(|w| *w != *v && self.entity(*w).is_some_and(|e| e.point_refs().contains(q))))),
            None => false,
        })
    }

    /// The curves joined to `u` end to end (point-to-point Coincidents
    /// only: a point on another loop's line doesn't join the loops).
    fn ring_of(&self, u: EntityId, cons: &[(crate::ids::ConstraintId, Vec<EntityId>)]) -> BTreeSet<EntityId> {
        let ends = |c: crate::ids::ConstraintId| matches!(self.constraint(c).map(|k| &k.kind), Some(ConstraintKind::Coincident { a, b }) if self.kind(*a) == Some(EntityKind::Point) && self.kind(*b) == Some(EntityKind::Point));
        let mut seen: BTreeSet<EntityId> = BTreeSet::from([u]);
        let mut todo = vec![u];
        while let Some(x) = todo.pop() {
            for (c, us) in cons {
                if us.contains(&x) && ends(*c) {
                    for v in us {
                        if seen.insert(*v) {
                            todo.push(*v);
                        }
                    }
                }
            }
        }
        seen
    }

    /// How many vertices the loop of `u` has (each curve's ends, a circle
    /// or point one; a spline's fit points don't count: fuzz s210's spline
    /// loop moves before a quad).
    fn loop_points(&self, u: EntityId, cons: &[(crate::ids::ConstraintId, Vec<EntityId>)]) -> usize {
        self.loop_of(u, cons).iter().map(|v| if self.endpoints(*v).is_some() { 2 } else { 1 }).sum()
    }

    /// The points a driving Coincident joins to point `p`.
    fn coincident_with(&self, p: EntityId) -> Vec<EntityId> {
        self.constraints()
            .filter(|c| c.driving)
            .filter_map(|c| match c.kind {
                ConstraintKind::Coincident { a, b } if a == p => Some(b),
                ConstraintKind::Coincident { a, b } if b == p => Some(a),
                _ => None,
            })
            .collect()
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
    /// a placed unit is where that one is; that point of u's, if it moved.
    fn carry_onto_placed(&mut self, u: EntityId, placed: &BTreeSet<EntityId>, unit_of: &HashMap<EntityId, EntityId>) -> Option<EntityId> {
        let own = self.entity(u).map(|e| {
            let mut v: Vec<EntityId> = e.point_refs().into_iter().collect();
            v.push(u);
            v
        })?;
        let center = |s: &Sketch, id: EntityId| -> Option<EntityId> {
            let e = s.entity(id)?;
            if e.kind() == EntityKind::Point { Some(id) } else { e.center() }
        };
        let mut shift = None;
        let mut at = None;
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
                at = Some(mine);
                break;
            }
        }
        let d = shift?;
        if d.length() <= 1e-12 {
            return None;
        }
        let points = self.entity(u).map(|e| e.point_refs())?;
        let points: Vec<EntityId> = if points.is_empty() { vec![u] } else { points.into_iter().collect() };
        for p in points {
            if let Some(x) = self.point(p) {
                let _ = self.set_point(p, x + d);
            }
        }
        at
    }
}
