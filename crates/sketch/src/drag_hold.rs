//! A drag that keeps far geometry whole (live drag probes 9 g6: a bolt
//! circle's dragged hole swings the pattern centre about the next hole,
//! which stays, and the last hole follows radially; the dimension hold of
//! `Sketch::drag_weights` kept the centre instead).

use crate::constraints::ConstraintKind;
use crate::entities::EntityKind;
use crate::ids::EntityId;
use crate::sketch::Sketch;
use crate::solver::{solve_held, DragHandle, DragTarget, SolveOptions};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

/// Dragged components larger than this keep the weighted solve alone.
const MAX_UNITS: usize = 60;
/// Holds tried per drag step.
const MAX_TRIES: usize = 4;

/// Units (a curve with its points, a point of no curve) joined by the
/// relations that place them, per sketch revision.
#[derive(Debug, Default)]
pub(crate) struct DragGraph {
    units_of: HashMap<EntityId, Vec<EntityId>>,
    adj: HashMap<EntityId, Vec<EntityId>>,
    comp_size: HashMap<EntityId, usize>,
}

impl Sketch {
    /// Built once per revision (`drag` builds it before taking its start
    /// copy, so the copies keep it).
    pub(crate) fn drag_graph(&mut self) -> Arc<DragGraph> {
        if let Some((rev, g)) = &self.cache.drag_graph
            && *rev == self.cache.revision
        {
            return g.clone();
        }
        let mut g = DragGraph::default();
        for e in self.entities().filter(|e| e.kind() != EntityKind::Point && !e.external) {
            g.units_of.entry(e.id).or_default().push(e.id);
            for p in e.point_refs() {
                let us = g.units_of.entry(p).or_default();
                if !us.contains(&e.id) {
                    us.push(e.id);
                }
            }
        }
        for e in self.entities().filter(|e| e.kind() == EntityKind::Point && !e.external) {
            g.units_of.entry(e.id).or_insert_with(|| vec![e.id]);
        }
        let link = |g: &mut DragGraph, a: EntityId, b: EntityId| {
            if a != b {
                g.adj.entry(a).or_default().push(b);
                g.adj.entry(b).or_default().push(a);
            }
        };
        for us in g.units_of.values().filter(|us| us.len() > 1).cloned().collect::<Vec<_>>() {
            for w in us.windows(2) {
                link(&mut g, w[0], w[1]);
            }
        }
        // Equal relates sizes, not places.
        for c in self.constraints().filter(|c| c.driving && !matches!(c.kind, ConstraintKind::Equal { .. })) {
            let us: Vec<EntityId> = c.kind.entities().iter().filter_map(|e| g.units_of.get(e)).flatten().copied().collect();
            for w in us.windows(2) {
                link(&mut g, w[0], w[1]);
            }
        }
        let units: Vec<EntityId> = g.units_of.values().flatten().copied().collect();
        for u in units {
            if g.comp_size.contains_key(&u) {
                continue;
            }
            let comp = g.reach(&[u]);
            let n = comp.len();
            for (v, _) in comp {
                g.comp_size.insert(v, n);
            }
        }
        let g = Arc::new(g);
        self.cache.drag_graph = Some((self.cache.revision, g.clone()));
        g
    }

    /// When the dimension hold can't bring the dragged geometry to the
    /// pointer: geometry two relations or more from it is held and the
    /// dragged geometry and its neighbours take the move from `start`; while
    /// that can't reach the pointer, the nearest held ring gives way one
    /// curve at a time, the latest drawn first. True with the sketch at the
    /// first hold that reaches it; else the sketch is left as it was.
    pub(crate) fn keep_far(&mut self, start: &Sketch, handles: &[(DragHandle, glam::DVec2)], opts: &SolveOptions, targets: &[DragTarget]) -> bool {
        let g = self.drag_graph();
        let mut dragged: Vec<EntityId> = Vec::new();
        for (h, _) in handles {
            let id = match *h {
                DragHandle::Point(id) | DragHandle::Entity { id, .. } => id,
            };
            dragged.extend(g.units_of.get(&id).into_iter().flatten().copied());
        }
        if dragged.is_empty() || dragged.iter().any(|u| g.comp_size.get(u).is_none_or(|n| *n > MAX_UNITS)) {
            return false;
        }
        let dist = g.reach(&dragged);
        let near: Vec<EntityId> = dist.iter().filter(|(_, d)| **d <= 1).map(|(u, _)| *u).collect();
        let mut far: Vec<(usize, EntityId)> = dist.iter().filter(|(_, d)| **d >= 2).map(|(u, d)| (*d, *u)).collect();
        if far.is_empty() {
            return false;
        }
        far.sort_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)));
        let was = self.clone();
        for k in 0..MAX_TRIES.min(far.len()) {
            let light: Vec<EntityId> = near.iter().copied().chain(far[..k].iter().map(|(_, u)| *u)).collect();
            let still: Vec<EntityId> = far[k..].iter().filter_map(|(_, u)| start.entity(*u)).flat_map(|e| e.point_refs()).collect();
            let hold = SolveOptions { light, still, ..opts.clone() };
            *self = start.clone();
            if solve_held(self, &hold, targets) && self.targets_met(targets) {
                if k > 0 {
                    self.place_released(start, &far[..k], &hold.still);
                }
                return true;
            }
        }
        *self = was;
        false
    }

    /// The geometry a hold released, placed last by least change from where
    /// it was, everything else held (g6: the last hole goes radially onto
    /// its circle about the moved centre). No drag targets: the solve then
    /// takes its components apart from the dragged one's.
    fn place_released(&mut self, start: &Sketch, released: &[(usize, EntityId)], held: &[EntityId]) {
        let solved = self.clone();
        let units: Vec<EntityId> = released.iter().map(|(_, u)| *u).collect();
        let others: HashSet<EntityId> = self
            .entities()
            .filter(|e| e.kind() != EntityKind::Point && !units.contains(&e.id))
            .flat_map(|e| e.point_refs())
            .chain(held.iter().copied())
            .collect();
        for &u in &units {
            let pts = self.entity(u).map(|e| e.point_refs()).unwrap_or_default();
            for id in std::iter::once(u).chain(pts.into_iter().filter(|p| !others.contains(p))) {
                if let (Some(e), Some(was)) = (self.entity_mut_silent(id), start.entity(id)) {
                    e.geometry = was.geometry.clone();
                }
            }
        }
        let alone = SolveOptions { light: units, still: others.into_iter().collect(), ..SolveOptions::default() };
        if !solve_held(self, &alone, &[]) {
            *self = solved;
        }
    }

    pub(crate) fn targets_met(&self, targets: &[DragTarget]) -> bool {
        targets.iter().all(|t| match *t {
            DragTarget::Coord { point, comp, value } => {
                self.point(point).is_some_and(|p| (if comp == 0 { p.x } else { p.y } - value).abs() <= 1e-9)
            }
            DragTarget::Own { entity, value, .. } => match self.curve(entity) {
                Some(crate::geom::Curve2::Circle { radius, .. }) => (radius - value).abs() <= 1e-9,
                _ => true,
            },
        })
    }
}

impl DragGraph {
    /// Units reached from `from` with their distance in relations.
    fn reach(&self, from: &[EntityId]) -> HashMap<EntityId, usize> {
        let mut dist: HashMap<EntityId, usize> = from.iter().map(|u| (*u, 0)).collect();
        let mut queue: VecDeque<EntityId> = from.iter().copied().collect();
        while let Some(u) = queue.pop_front() {
            let d = dist[&u];
            for v in self.adj.get(&u).into_iter().flatten() {
                if !dist.contains_key(v) {
                    dist.insert(*v, d + 1);
                    queue.push_back(*v);
                }
            }
        }
        dist
    }
}
