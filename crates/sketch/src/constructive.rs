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

    /// Places `comp`'s units from what changes outwards (module docs).
    fn place_component(&mut self, comp: &[EntityId], cons: &[(crate::ids::ConstraintId, Vec<EntityId>)], unit_of: &HashMap<EntityId, EntityId>) -> bool {
        if self.scale_for_chord(comp, cons) {
            return true;
        }
        let mut placed: BTreeSet<EntityId> = BTreeSet::new();
        // Sizes and directions to keep are as the pass found them (a side
        // sharing a point with one placed before has already been pulled:
        // a corner rectangle's right side after its bottom stretched).
        let orig = self.clone();
        let unheld = self.unheld_constraints(1e-9);
        let mut holds: HashMap<crate::ids::ConstraintId, bool> = cons.iter().map(|(c, _)| (*c, !unheld.contains(c))).collect();
        self.shift_loops_for_distances(cons, &placed);
        while placed.len() < comp.len() {
            // Next: a unit with a constraint to placed units (or of its own)
            // that doesn't hold, the one with most of them; with none, a
            // unit of a constraint that doesn't hold (the member it keeps
            // first); with none at all, everything left stays.
            let pending = |u: EntityId| cons.iter().filter(|(c, us)| us.contains(&u) && us.iter().all(|x| *x == u || placed.contains(x)) && !holds[c]).count();
            let open: Vec<EntityId> = comp.iter().copied().filter(|u| !placed.contains(u)).collect();
            // Then the unit most joined (shared points, its own
            // constraints; relations to others don't count: probe t4, a
            // rectangle side Perpendicular to a separate line isn't first).
            let joined = |u: EntityId| cons.iter().filter(|(c, us)| us.contains(&u) && (us.len() == 1 || self.connective(*c))).count();
            let rank = |u: EntityId| (pending(u), joined(u));
            let best = open.iter().map(|u| rank(*u)).max().unwrap();
            // First, a relation between units that doesn't hold, none of
            // them placed: the member it keeps stays as it is (if its own
            // constraints hold), so the others come to it (fuzz s8: the
            // dimensioned circle of a concentric pair moves onto the other).
            let own_hold = |u: EntityId| cons.iter().filter(|(_, us)| us.as_slice() == [u]).all(|(c, _)| holds[c]);
            let keep = cons.iter().filter(|(c, us)| us.len() >= 2 && !holds[c] && !us.iter().any(|x| placed.contains(x)) && !self.chained_to_change(*c, cons, &holds)).find_map(|(c, us)| {
                let k = self.kept_first(us, cons);
                (own_hold(k) && self.class_keeps(*c, k, cons) && self.direction_agrees(k, cons)).then_some(k)
            });
            if let Some(k) = keep {
                placed.insert(k);
                continue;
            }
            let u = if best.0 > 0 {
                // Ties: the member a relation keeps, else the lower one
                // (probe t4: a rectangle's bottom before its right side).
                let tied: Vec<EntityId> = open.iter().copied().filter(|u| rank(*u) == best).collect();
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
            let round = matches!(self.kind(u), Some(EntityKind::Arc | EntityKind::Circle));
            let lower_held = if round { self.lower_shared(u, &held) } else { None };
            // (neighbours held: 0 none, 1 all, 2 the lower one; shape)
            let ladder: &[(u8, u8)] = if lower_held.is_some() {
                &[(1, 3), (0, 3), (1, 2), (1, 1), (1, 0), (2, 1), (2, 0), (0, 2), (0, 1), (0, 0)]
            } else {
                &[(1, 3), (0, 3), (1, 2), (1, 1), (1, 0), (0, 2), (0, 1), (0, 0)]
            };
            'attempts: for &(hold, shape) in ladder {
                let hold_neighbours = hold == 1;
                let mut fixed = placed.clone();
                if hold_neighbours {
                    fixed.extend(held.iter().copied());
                }
                if hold == 2 {
                    fixed.extend(lower_held.map(|(v, _)| v));
                }
                let mut with = fixed.clone();
                with.insert(u);
                let keep = |c: crate::ids::ConstraintId, us: &[EntityId]| us.contains(&u) && us.iter().all(|x| with.contains(x)) && (us.iter().all(|x| *x == u || placed.contains(x)) || self.connective(c));
                let Some(mut sub) = self.sub_sketch_by(cons, &keep, &fixed) else { return false };
                if shape == 3 && end_carried {
                    continue;
                }
                if (shape == 3 || shape == 2) && !orig.hold_size(&mut sub, u) {
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
                let opts = SolveOptions { light: vec![u], still: still.into_iter().collect(), analyze: false, constructive: false, ..Default::default() };
                let st = sub.solve_with(&opts).status;
                // With its neighbours let go: only when each of them can
                // follow, stretching along itself or carried by a neighbour
                // that can (live fuzz s5: a rectangle's right side moves out
                // whole, the top stretches; s8: a triangle side tied to a
                // turned line turns too, its other end stays; probe t1: the
                // top doesn't move sideways, the left side couldn't follow).
                if hold == 0 && shape >= 1 && matches!(st, SolveStatus::Solved) && !held.is_empty() && !self.neighbours_follow(&sub, u, &held, &before, &placed) {
                    continue;
                }
                if matches!(st, SolveStatus::Solved) {
                    done = Some(sub);
                    break 'attempts;
                }
            }
            // A loop one unit can't close: u with its unplaced neighbours
            // (placed geometry still fixed), then with its placed ones too;
            // the change is shared within the group.
            let mut group = vec![u];
            if done.is_none() {
                let pin = self.lowest_point(cons.iter().filter(|(c, us)| us.contains(&u) && !holds[c]).filter_map(|(c, _)| self.constraint(*c)).flat_map(|c| c.kind.entities()));
                let all_neighbours: Vec<EntityId> = cons.iter().filter(|(_, us)| us.contains(&u)).flat_map(|(_, us)| us.iter().copied()).filter(|x| *x != u).collect();
                // Placed neighbours without constraints of their own go
                // before the rest (live fuzz s11: an equilateral triangle
                // keeps its dimensioned side and closes on the other two).
                let own = |x: EntityId| cons.iter().any(|(_, us)| us.as_slice() == [x]);
                for release in 0..3 {
                    let mut g: BTreeSet<EntityId> = all_neighbours.iter().copied().filter(|x| !placed.contains(x) || release == 2 || (release == 1 && !own(*x))).collect();
                    if release == 1 && g.iter().all(|x| !placed.contains(x)) {
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
                placed.insert(g);
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

    /// Whether each unit of `held` sharing a point `u` moved (from `before`
    /// to `sub`) can follow it: the move runs along its line, or it can be
    /// carried across itself because each other unit at its other points is
    /// unplaced and runs along that part of the move.
    fn neighbours_follow(&self, sub: &Sketch, u: EntityId, held: &BTreeSet<EntityId>, before: &HashMap<EntityId, DVec2>, placed: &BTreeSet<EntityId>) -> bool {
        let Some(own) = self.entity(u).map(|e| e.point_refs()) else { return false };
        let along = |v: EntityId, d: DVec2| self.placing_line_dir(v).filter(|x| x.length() > 0.0).is_some_and(|dir| dir.normalize().perp_dot(d).abs() <= 1e-9);
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
                if !held.contains(&v) || along(v, d) || !self.direction_bound(v) {
                    continue;
                }
                // Carried across itself (it stretches along): its other
                // points' units stretch along that.
                // (Only lines are judged: an arc or circle follows its own
                // way, live fuzz s17's slot.)
                let Some(dir) = self.placing_line_dir(v).filter(|x| x.length() > 0.0).map(|x| x.normalize()) else { continue };
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
    /// of its own first, then the lower point (smaller y, then x). Live:
    /// fuzz s223 keeps the end on the dimensioned line; s234 (both lines
    /// dimensioned), probes 5 u1 / u3 and s215 (neither) the lower end.
    fn lower_shared(&self, u: EntityId, held: &BTreeSet<EntityId>) -> Option<(EntityId, EntityId)> {
        let own = self.entity(u)?.point_refs();
        let dimensioned = |v: EntityId| self.constraints().any(|c| c.driving && c.kind.is_dimension() && c.kind.entities().as_slice() == [v]);
        own.iter()
            .flat_map(|p| self.coincident_with(*p).into_iter().map(move |q| (*p, q)))
            .filter_map(|(p, q)| held.iter().find(|v| self.entity(**v).is_some_and(|e| e.point_refs().contains(&q))).map(|v| (*v, p)))
            .filter_map(|(v, p)| self.point(p).map(|x| (v, p, x, dimensioned(v))))
            .min_by(|a, b| b.3.cmp(&a.3).then(if (a.2.y - b.2.y).abs() > 1e-9 { a.2.y.total_cmp(&b.2.y) } else { a.2.x.total_cmp(&b.2.x) }))
            .map(|(v, p, _, _)| (v, p))
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

    /// Of tied units, the one a relation between two of them keeps (Equal:
    /// see `equal_key`; Parallel / Perpendicular: the smaller angle in [0°, 180°);
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
                    // Along the distance: a's point against b's.
                    let (qa_own, qb_own) = if owns(a, pa) { (qa, qb) } else { (qb, qa) };
                    let d = qb_own - qa_own;
                    let along = if d.x.abs() >= d.y.abs() { d.x } else { d.y };
                    if along.abs() > 1e-12 {
                        return if along > 0.0 { a } else { b };
                    }
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
            // Nothing fixed in it, no other distance out of it.
            let fixed = self.constraints().filter(|c| c.driving).any(|c| {
                let touches = c.kind.entities().iter().any(|e| owns(self, *e) || group.contains(e));
                touches && (matches!(c.kind, ConstraintKind::Fix { .. }) || (c.id != *cid && matches!(c.kind, ConstraintKind::Distance { .. } | ConstraintKind::HorizontalDistance { .. } | ConstraintKind::VerticalDistance { .. }) && !c.kind.entities().iter().all(|e| owns(self, *e) || group.contains(e))))
            });
            if fixed || group.iter().any(|u| self.entity(*u).is_some_and(|e| e.external)) {
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
