//! Geometric constraint solver.
//!
//! * Unknowns are the free scalar parameters of non-external entities (point
//!   coordinates, circle radii, ellipse axes) plus auxiliary per-constraint
//!   unknowns (e.g. curve parameters for point-on-spline).
//! * Point-point coincidences / concentricities are eliminated up front by
//!   aliasing parameters (union-find), which shrinks the system a lot for
//!   typical sketches.
//! * The remaining equations are split into independent connected components
//!   that are solved separately with a damped Gauss-Newton / Levenberg-
//!   Marquardt iteration (with step halving). Steps are minimum-norm in a
//!   weighted metric, so geometry changes as little as possible and dragged
//!   parameters are "heavy" (SolveSpace-style dragging).
//! * The normal equations use a sparse envelope Cholesky factorization with
//!   RCM ordering, so cost stays near-linear for large sparse sketches.

use crate::constraints::ConstraintKind;
use crate::dof::{analyze_component, SparseRow};
use crate::entities::{EntityKind, Geometry};
use crate::equations::{Builder, Eq, Lookup};
use crate::ids::{ConstraintId, EntityId};
use crate::linalg::EnvelopeSolver;
use crate::sketch::Sketch;
use glam::DVec2;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use std::collections::{HashMap, HashSet};

const NONE: u32 = u32::MAX;
/// Weight of dragged parameters in the minimum-norm metric.
const DRAG_WEIGHT: f64 = 1e6;

/// Overall result of a solve.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum SolveStatus {
    /// All (driving) constraints are satisfied.
    Solved,
    /// Some constraints conflict; they are listed and were ignored so the rest
    /// of the sketch could be solved.
    Overconstrained { conflicting: Vec<ConstraintId> },
    /// The solver did not converge; geometry was left unchanged.
    Failed { reason: String },
}

/// Per-entity constraint state (Onshape colors: black / blue / red).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConstraintState {
    FullyConstrained,
    UnderConstrained,
    OverConstrained,
}

/// Report returned by [`Sketch::solve`] and [`Sketch::drag`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SolveReport {
    pub status: SolveStatus,
    /// Remaining degrees of freedom (`None` if the DOF analysis was skipped
    /// and no cached analysis was available).
    pub dof: Option<usize>,
    /// Constraint state of every entity (empty if not analyzed).
    pub entity_states: IndexMap<EntityId, ConstraintState>,
    /// Redundant but consistent constraints.
    pub redundant: Vec<ConstraintId>,
    /// Constraints that could not be turned into equations (invalid references).
    pub invalid: Vec<ConstraintId>,
    pub iterations: usize,
    pub max_residual: f64,
}

impl SolveReport {
    pub fn is_solved(&self) -> bool {
        self.status == SolveStatus::Solved
    }
    pub fn state(&self, id: EntityId) -> Option<ConstraintState> {
        self.entity_states.get(&id).copied()
    }
    pub fn conflicting(&self) -> &[ConstraintId] {
        match &self.status {
            SolveStatus::Overconstrained { conflicting } => conflicting,
            _ => &[],
        }
    }
}

/// Solver options.
#[derive(Clone, Debug)]
pub struct SolveOptions {
    /// Max absolute residual for convergence.
    pub tolerance: f64,
    pub max_iterations: usize,
    /// Run the DOF / constraint state analysis.
    pub analyze: bool,
    /// Entities (with their defining points) that should take the change:
    /// every other free parameter is held (or, if that can't solve, weighted
    /// heavier), so a new dimension value moves the dimensioned geometry and
    /// leaves the rest where it is (Onshape: a fillet radius change extends
    /// the lines, the rectangle stays; qa5 SK5). Empty: all parameters weigh
    /// the same.
    pub light: Vec<EntityId>,
    /// Points of `light` entities that are held all the same.
    pub still: Vec<EntityId>,
}

impl Default for SolveOptions {
    fn default() -> Self {
        SolveOptions { tolerance: 1e-10, max_iterations: 100, analyze: true, light: Vec::new(), still: Vec::new() }
    }
}

/// What is being dragged.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum DragHandle {
    /// A point entity, moved to the target.
    Point(EntityId),
    /// A curve grabbed at `grab` (the cursor position of the previous drag
    /// step). Lines/splines translate, circles/arcs change radius.
    Entity { id: EntityId, grab: DVec2 },
}

/// Cached result of a DOF analysis.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Analysis {
    pub dof: usize,
    pub states: IndexMap<EntityId, ConstraintState>,
    pub redundant: Vec<ConstraintId>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum DragTarget {
    Coord { point: EntityId, comp: u8, value: f64 },
    Own { entity: EntityId, slot: u8, value: f64 },
}

#[derive(Clone, Copy, Debug)]
enum Owner {
    Ent(EntityId, u8),
    Aux(ConstraintId, u8),
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Src {
    Constraint(ConstraintId),
    Internal,
}

struct Params {
    values: Vec<f64>,
    fixed: Vec<bool>,
    owner: Vec<Owner>,
    ent_base: Vec<u32>,
    aux_base: HashMap<ConstraintId, u32>,
}

impl Params {
    fn build(sk: &Sketch) -> Params {
        let mut p = Params {
            values: Vec::new(),
            fixed: Vec::new(),
            owner: Vec::new(),
            ent_base: vec![NONE; sk.max_entity_id() as usize + 1],
            aux_base: HashMap::new(),
        };
        for e in sk.entities() {
            let own = e.own_params();
            // Frozen entities inserted above the id allocator (e.g. text glyph
            // outlines added after solving) take no part in the system.
            if own.is_empty() || e.id.0 as usize >= p.ent_base.len() {
                continue;
            }
            p.ent_base[e.id.0 as usize] = p.values.len() as u32;
            for (k, v) in own.iter().enumerate() {
                p.values.push(*v);
                p.fixed.push(e.external);
                p.owner.push(Owner::Ent(e.id, k as u8));
            }
        }
        for c in sk.constraints() {
            if c.aux.is_empty() {
                continue;
            }
            p.aux_base.insert(c.id, p.values.len() as u32);
            for (k, v) in c.aux.iter().enumerate() {
                p.values.push(*v);
                p.fixed.push(false);
                p.owner.push(Owner::Aux(c.id, k as u8));
            }
        }
        p
    }
}

struct Lk<'a> {
    p: &'a Params,
    cid: Option<ConstraintId>,
    uf: &'a [u32],
    on_curve: &'a HashSet<(u32, EntityId)>,
}

fn root(uf: &[u32], mut i: u32) -> u32 {
    while uf[i as usize] != i {
        i = uf[i as usize];
    }
    i
}

impl Lookup for Lk<'_> {
    fn point(&self, id: EntityId) -> [u32; 2] {
        let b = self.p.ent_base[id.0 as usize];
        [b, b + 1]
    }
    fn own(&self, id: EntityId, slot: usize) -> u32 {
        self.p.ent_base[id.0 as usize] + slot as u32
    }
    fn aux(&self, k: usize) -> u32 {
        self.cid.and_then(|c| self.p.aux_base.get(&c)).map_or(0, |b| b + k as u32)
    }
    fn same(&self, a: EntityId, b: EntityId) -> bool {
        if a == b {
            return true;
        }
        let (pa, pb) = (self.p.ent_base[a.0 as usize], self.p.ent_base[b.0 as usize]);
        if pa == NONE || pb == NONE {
            return false;
        }
        root(self.uf, pa) == root(self.uf, pb) && root(self.uf, pa + 1) == root(self.uf, pb + 1)
    }
    fn on_curve(&self, p: EntityId, curve: EntityId) -> bool {
        let pa = self.p.ent_base[p.0 as usize];
        pa != NONE && self.on_curve.contains(&(root(self.uf, pa), curve))
    }
}

fn find(uf: &mut [u32], mut i: u32) -> u32 {
    while uf[i as usize] != i {
        let g = uf[uf[i as usize] as usize];
        uf[i as usize] = g;
        i = g;
    }
    i
}

enum AliasResult {
    Merged,
    Same,
    Keep,
}

struct Comp {
    vars: Vec<u32>,
    eqs: Vec<u32>,
}

struct CompSolve {
    converged: bool,
    iterations: usize,
    residual: Vec<f64>,
    rows: Vec<SparseRow>,
}

pub(crate) struct System {
    p: Params,
    uf: Vec<u32>,
    eqs: Vec<Eq>,
    src: Vec<Src>,
    weight: Vec<f64>,
    alias_redundant: Vec<ConstraintId>,
    invalid: Vec<ConstraintId>,
    /// Drag targets per representative parameter.
    drag_target: HashMap<u32, f64>,
}

/// Weight of the parameters outside `SolveOptions::light`.
const STILL_WEIGHT: f64 = 100.0;

impl System {
    /// Representative free parameters not owned by `opts.light` (or its
    /// points), plus those of `opts.still`.
    fn still_reps(&mut self, sk: &Sketch, opts: &SolveOptions) -> Vec<usize> {
        let mut ents: HashSet<EntityId> = opts.light.iter().filter_map(|id| sk.entity(*id)).flat_map(|e| e.point_refs().into_iter().chain(std::iter::once(e.id))).collect();
        for id in &opts.still {
            ents.remove(id);
        }
        let mut light_reps: HashSet<u32> = HashSet::new();
        for i in 0..self.p.owner.len() {
            if let Owner::Ent(id, _) = self.p.owner[i]
                && ents.contains(&id)
            {
                light_reps.insert(find(&mut self.uf, i as u32));
            }
        }
        for i in 0..self.p.owner.len() {
            if let Owner::Ent(id, _) = self.p.owner[i]
                && opts.still.contains(&id)
            {
                light_reps.remove(&find(&mut self.uf, i as u32));
            }
        }
        (0..self.weight.len())
            .filter(|&i| find(&mut self.uf, i as u32) as usize == i && !light_reps.contains(&(i as u32)) && !self.p.fixed[i] && self.weight[i] < DRAG_WEIGHT)
            .collect()
    }

    /// Weigh every free parameter except those of `light` (and their
    /// points) heavier (see `SolveOptions::light`).
    fn weigh_still(&mut self, sk: &Sketch, opts: &SolveOptions) {
        for i in self.still_reps(sk, opts) {
            self.weight[i] = STILL_WEIGHT;
        }
    }
}

fn alias_pair(sk: &Sketch, kind: &ConstraintKind) -> Option<(EntityId, EntityId)> {
    let center = |id: EntityId| -> Option<EntityId> {
        let e = sk.entity(id)?;
        if e.kind() == EntityKind::Point { Some(id) } else { e.center() }
    };
    match kind {
        ConstraintKind::Coincident { a, b } | ConstraintKind::Pierce { point: a, target: b } => {
            (sk.kind(*a)? == EntityKind::Point && sk.kind(*b)? == EntityKind::Point && a != b).then_some((*a, *b))
        }
        ConstraintKind::Concentric { a, b } => {
            let (ca, cb) = (center(*a)?, center(*b)?);
            (ca != cb).then_some((ca, cb))
        }
        _ => None,
    }
}

impl System {
    fn build(sk: &Sketch, excluded: &HashSet<ConstraintId>, drag: &[DragTarget]) -> System {
        let p = Params::build(sk);
        let n = p.values.len();
        let mut sys = System {
            p,
            uf: (0..n as u32).collect(),
            eqs: Vec::new(),
            src: Vec::new(),
            weight: vec![1.0; n],
            alias_redundant: Vec::new(),
            invalid: Vec::new(),
            drag_target: HashMap::new(),
        };
        let mut aliased: HashSet<ConstraintId> = HashSet::new();
        for c in sk.constraints() {
            if !c.driving || excluded.contains(&c.id) {
                continue;
            }
            if let Some((a, b)) = alias_pair(sk, &c.kind) {
                match sys.try_alias(a, b) {
                    AliasResult::Merged => {
                        aliased.insert(c.id);
                    }
                    AliasResult::Same => {
                        aliased.insert(c.id);
                        sys.alias_redundant.push(c.id);
                    }
                    AliasResult::Keep => {}
                }
            }
        }
        // point-on-curve coincidences (keyed by representative x parameter)
        let mut on_curve: HashSet<(u32, EntityId)> = HashSet::new();
        for c in sk.constraints() {
            if !c.driving || excluded.contains(&c.id) {
                continue;
            }
            if let ConstraintKind::Coincident { a, b } | ConstraintKind::Pierce { point: a, target: b } = c.kind {
                let (ka, kb) = (sk.kind(a), sk.kind(b));
                let (p, q) = match (ka, kb) {
                    (Some(EntityKind::Point), Some(k)) if k.is_curve() => (a, b),
                    (Some(k), Some(EntityKind::Point)) if k.is_curve() => (b, a),
                    _ => continue,
                };
                let base = sys.p.ent_base[p.0 as usize];
                if base != NONE {
                    on_curve.insert((find(&mut sys.uf, base), q));
                }
            }
        }
        for c in sk.constraints() {
            if !c.driving || excluded.contains(&c.id) || aliased.contains(&c.id) {
                continue;
            }
            let lk = Lk { p: &sys.p, cid: Some(c.id), uf: &sys.uf, on_curve: &on_curve };
            let before = sys.eqs.len();
            let res = Builder { sk, lk: &lk, out: &mut sys.eqs }.constraint(c);
            match res {
                Ok(()) => {
                    for _ in before..sys.eqs.len() {
                        sys.src.push(Src::Constraint(c.id));
                    }
                }
                Err(msg) => {
                    log::warn!("constraint {} ignored: {msg}", c.id);
                    sys.eqs.truncate(before);
                    sys.invalid.push(c.id);
                }
            }
        }
        // Internal equations last, so that redundancy is attributed to them
        // rather than to user constraints.
        for e in sk.entities() {
            let lk = Lk { p: &sys.p, cid: None, uf: &sys.uf, on_curve: &on_curve };
            let before = sys.eqs.len();
            Builder { sk, lk: &lk, out: &mut sys.eqs }.internal(e.id);
            for _ in before..sys.eqs.len() {
                sys.src.push(Src::Internal);
            }
        }
        // map inputs to representatives
        let uf = &mut sys.uf;
        for eq in sys.eqs.iter_mut() {
            for i in eq.inputs.iter_mut() {
                *i = find(uf, *i);
            }
        }
        for t in drag {
            let (id, k, v) = match *t {
                DragTarget::Coord { point, comp, value } => (point, comp as u32, value),
                DragTarget::Own { entity, slot, value } => (entity, slot as u32, value),
            };
            let base = sys.p.ent_base.get(id.0 as usize).copied().unwrap_or(NONE);
            if base == NONE {
                continue;
            }
            let r = find(&mut sys.uf, base + k) as usize;
            if !sys.p.fixed[r] {
                sys.p.values[r] = v;
                sys.weight[r] = DRAG_WEIGHT;
                sys.drag_target.insert(r as u32, v);
            }
        }
        sys
    }

    fn try_alias(&mut self, a: EntityId, b: EntityId) -> AliasResult {
        let (pa, pb) = (self.p.ent_base[a.0 as usize], self.p.ent_base[b.0 as usize]);
        let rx = (find(&mut self.uf, pa), find(&mut self.uf, pb));
        let ry = (find(&mut self.uf, pa + 1), find(&mut self.uf, pb + 1));
        if rx.0 == rx.1 && ry.0 == ry.1 {
            return AliasResult::Same;
        }
        for (r1, r2) in [rx, ry] {
            let (i, j) = (r1 as usize, r2 as usize);
            if r1 != r2 && self.p.fixed[i] && self.p.fixed[j] && (self.p.values[i] - self.p.values[j]).abs() > 1e-12 {
                return AliasResult::Keep;
            }
        }
        for (r1, r2) in [rx, ry] {
            if r1 == r2 {
                continue;
            }
            let (i, j) = (r1 as usize, r2 as usize);
            let root = if self.p.fixed[j] && !self.p.fixed[i] { r2 } else if self.p.fixed[i] { r1 } else { r1.min(r2) };
            let other = if root == r1 { r2 } else { r1 };
            self.uf[other as usize] = root;
        }
        AliasResult::Merged
    }

    fn gather(&self, eq: &Eq, buf: &mut SmallVec<[f64; 16]>) {
        buf.clear();
        buf.extend(eq.inputs.iter().map(|&i| self.p.values[i as usize]));
    }

    /// Evaluates residuals (and the sparse Jacobian rows if `rows` is given).
    fn eval(&self, comp: &Comp, cols: &[SmallVec<[u32; 8]>], r: &mut [f64], rows: Option<&mut [SparseRow]>) {
        let mut buf: SmallVec<[f64; 16]> = SmallVec::new();
        let mut g: SmallVec<[f64; 16]> = SmallVec::new();
        match rows {
            None => {
                for (ri, &ei) in comp.eqs.iter().enumerate() {
                    let eq = &self.eqs[ei as usize];
                    self.gather(eq, &mut buf);
                    r[ri] = eq.eval(&buf);
                }
            }
            Some(rows) => {
                for (ri, &ei) in comp.eqs.iter().enumerate() {
                    let eq = &self.eqs[ei as usize];
                    self.gather(eq, &mut buf);
                    g.clear();
                    g.resize(buf.len(), 0.0);
                    r[ri] = eq.eval_grad(&buf, &mut g);
                    let row = &mut rows[ri];
                    row.clear();
                    for (k, &col) in cols[ri].iter().enumerate() {
                        if col == NONE {
                            continue;
                        }
                        let v = if g[k].is_finite() { g[k] } else { 0.0 };
                        if let Some(e) = row.iter_mut().find(|e| e.0 == col) {
                            e.1 += v;
                        } else {
                            row.push((col, v));
                        }
                    }
                }
            }
        }
        for v in r.iter_mut() {
            if !v.is_finite() {
                *v = 1e6;
            }
        }
    }

    fn solve_comp(&mut self, comp: &Comp, var_local: &mut [u32], var_of: &[u32], opts: &SolveOptions) -> CompSolve {
        let n = comp.vars.len();
        let m = comp.eqs.len();
        for (k, &pv) in comp.vars.iter().enumerate() {
            var_local[var_of[pv as usize] as usize] = k as u32;
        }
        let cols: Vec<SmallVec<[u32; 8]>> = comp
            .eqs
            .iter()
            .map(|&ei| {
                self.eqs[ei as usize]
                    .inputs
                    .iter()
                    .map(|&i| {
                        let v = var_of[i as usize];
                        if v == NONE { NONE } else { var_local[v as usize] }
                    })
                    .collect()
            })
            .collect();
        let w: Vec<f64> = comp.vars.iter().map(|&pv| self.weight[pv as usize]).collect();
        let sw: Vec<f64> = w.iter().map(|x| x.sqrt()).collect();
        let mut r = vec![0.0; m];
        let mut rows: Vec<SparseRow> = vec![SmallVec::new(); m];
        self.eval(comp, &cols, &mut r, Some(&mut rows));

        let min_norm = m <= n;
        // sparsity pattern (constant across iterations)
        let size = if min_norm { m } else { n };
        let mut col_entries: Vec<SmallVec<[(u32, u32); 6]>> = vec![SmallVec::new(); n];
        for (ri, row) in rows.iter().enumerate() {
            for (pos, &(c, _)) in row.iter().enumerate() {
                col_entries[c as usize].push((ri as u32, pos as u32));
            }
        }
        let mut edges = Vec::new();
        if min_norm {
            for ce in &col_entries {
                for a in 0..ce.len() {
                    for b in 0..a {
                        edges.push((ce[a].0 as usize, ce[b].0 as usize));
                    }
                }
            }
        } else {
            for row in &rows {
                for a in 0..row.len() {
                    for b in 0..a {
                        edges.push((row[a].0 as usize, row[b].0 as usize));
                    }
                }
            }
        }
        edges.sort_unstable();
        edges.dedup();
        let mut solver = EnvelopeSolver::new(size, &edges);

        let x0: Vec<f64> = comp.vars.iter().map(|&pv| self.p.values[pv as usize]).collect();
        let mut x = x0.clone();
        let cost_of = |r: &[f64]| r.iter().map(|v| v * v).sum::<f64>();
        let mut cost = cost_of(&r);
        let mut lambda = 0.0f64;
        let mut iterations = 0;
        let mut stall = 0;
        let mut converged = false;
        let mut rhs = vec![0.0; size];
        let mut dx = vec![0.0; n];
        let mut r_new = vec![0.0; m];
        let mut base: Vec<f64> = Vec::new();
        loop {
            let maxr = r.iter().fold(0.0f64, |a, v| a.max(v.abs()));
            if maxr <= opts.tolerance {
                converged = true;
                break;
            }
            if iterations >= opts.max_iterations || stall >= 6 {
                break;
            }
            iterations += 1;
            // assemble A
            solver.clear();
            if min_norm {
                for (k, ce) in col_entries.iter().enumerate() {
                    let iw = 1.0 / w[k];
                    for a in 0..ce.len() {
                        let (ra, pa) = ce[a];
                        let va = rows[ra as usize][pa as usize].1;
                        for &(rb, pb) in ce.iter().take(a + 1) {
                            let vb = rows[rb as usize][pb as usize].1;
                            solver.add(ra as usize, rb as usize, va * vb * iw);
                        }
                    }
                }
            } else {
                for row in &rows {
                    for a in 0..row.len() {
                        let (ca, va) = row[a];
                        for &(cb, vb) in row.iter().take(a + 1) {
                            solver.add(ca as usize, cb as usize, va * vb / (sw[ca as usize] * sw[cb as usize]));
                        }
                    }
                }
            }
            solver.snapshot(&mut base);
            // Marquardt-scaled damping (relative to each diagonal entry) so
            // badly scaled rows do not over-damp the others.
            let diag_mean = (solver.trace() / size as f64).max(1e-300);
            let lam_min = 1e-12;
            lambda = lambda.max(lam_min);
            let mut accepted = false;
            for _attempt in 0..40 {
                solver.restore(&base);
                solver.damp_diag(lambda, lambda * 1e-9 * diag_mean + 1e-300);
                if !solver.factor() {
                    lambda *= 10.0;
                    continue;
                }
                // step
                dx.iter_mut().for_each(|v| *v = 0.0);
                if min_norm {
                    for (i, v) in rhs.iter_mut().enumerate() {
                        *v = -r[i];
                    }
                    solver.solve(&mut rhs);
                    for (i, row) in rows.iter().enumerate() {
                        for &(c, v) in row {
                            dx[c as usize] += v * rhs[i];
                        }
                    }
                    for (k, d) in dx.iter_mut().enumerate() {
                        *d /= w[k];
                    }
                } else {
                    rhs.iter_mut().for_each(|v| *v = 0.0);
                    for (i, row) in rows.iter().enumerate() {
                        for &(c, v) in row {
                            rhs[c as usize] -= v * r[i];
                        }
                    }
                    for (k, v) in rhs.iter_mut().enumerate() {
                        *v /= sw[k];
                    }
                    solver.solve(&mut rhs);
                    for (k, d) in dx.iter_mut().enumerate() {
                        *d = rhs[k] / sw[k];
                    }
                }
                if dx.iter().any(|v| !v.is_finite()) {
                    lambda *= 10.0;
                    continue;
                }
                let mut done = false;
                for alpha in [1.0, 0.5] {
                    for (k, &pv) in comp.vars.iter().enumerate() {
                        self.p.values[pv as usize] = x[k] + alpha * dx[k];
                    }
                    self.eval(comp, &cols, &mut r_new, None);
                    let c_new = cost_of(&r_new);
                    if c_new < cost {
                        for (k, &pv) in comp.vars.iter().enumerate() {
                            x[k] = self.p.values[pv as usize];
                        }
                        if cost - c_new <= 1e-12 * cost {
                            stall += 1;
                        } else {
                            stall = 0;
                        }
                        cost = c_new;
                        done = true;
                        break;
                    }
                }
                if done {
                    accepted = true;
                    lambda = (lambda * 0.1).max(lam_min);
                    break;
                }
                for (k, &pv) in comp.vars.iter().enumerate() {
                    self.p.values[pv as usize] = x[k];
                }
                lambda *= 10.0;
                if lambda > 1e10 {
                    break;
                }
            }
            if !accepted {
                for (k, &pv) in comp.vars.iter().enumerate() {
                    self.p.values[pv as usize] = x[k];
                }
                break;
            }
            self.eval(comp, &cols, &mut r, Some(&mut rows));
        }
        CompSolve { converged, iterations, residual: r, rows }
    }

    fn restore_comp(&mut self, comp: &Comp, x0: &[f64]) {
        for (k, &pv) in comp.vars.iter().enumerate() {
            self.p.values[pv as usize] = x0[k];
        }
    }

    fn write_back(&mut self, sk: &mut Sketch) {
        for i in 0..self.p.values.len() {
            let r = find(&mut self.uf, i as u32) as usize;
            let v = self.p.values[r];
            match self.p.owner[i] {
                Owner::Ent(id, slot) => {
                    if let Some(e) = sk.entity_mut_silent(id)
                        && !e.external {
                            e.set_own_param(slot as usize, v);
                        }
                }
                Owner::Aux(cid, slot) => {
                    if let Some(c) = sk.constraint_mut_silent(cid)
                        && let Some(a) = c.aux.get_mut(slot as usize) {
                            *a = v;
                        }
                }
            }
        }
    }
}

struct Outcome {
    all_converged: bool,
    comp_conflicts: Vec<Vec<ConstraintId>>,
    const_conflicts: Vec<ConstraintId>,
    const_redundant: Vec<ConstraintId>,
    iterations: usize,
    max_residual: f64,
    comps: Vec<Comp>,
    comp_ok: Vec<bool>,
    comp_rows: Vec<Vec<SparseRow>>,
    var_of: Vec<u32>,
}

impl System {
    fn solve(&mut self, opts: &SolveOptions) -> Outcome {
        let np = self.p.values.len();
        let mut var_of = vec![NONE; np];
        let mut vars: Vec<u32> = Vec::new();
        for eq in &self.eqs {
            for &i in &eq.inputs {
                if !self.p.fixed[i as usize] && var_of[i as usize] == NONE {
                    var_of[i as usize] = vars.len() as u32;
                    vars.push(i);
                }
            }
        }
        let nv = vars.len();
        let mut cuf: Vec<u32> = (0..nv as u32).collect();
        let mut const_eqs = Vec::new();
        let mut eq_first = vec![NONE; self.eqs.len()];
        for (ei, eq) in self.eqs.iter().enumerate() {
            let mut first = NONE;
            for &i in &eq.inputs {
                let v = var_of[i as usize];
                if v == NONE {
                    continue;
                }
                if first == NONE {
                    first = v;
                } else {
                    let (a, b) = (find(&mut cuf, first), find(&mut cuf, v));
                    if a != b {
                        cuf[a.max(b) as usize] = a.min(b);
                    }
                }
            }
            eq_first[ei] = first;
            if first == NONE {
                const_eqs.push(ei);
            }
        }
        let mut comp_of_root = vec![NONE; nv];
        let mut comps: Vec<Comp> = Vec::new();
        for v in 0..nv as u32 {
            let r = find(&mut cuf, v);
            if comp_of_root[r as usize] == NONE {
                comp_of_root[r as usize] = comps.len() as u32;
                comps.push(Comp { vars: Vec::new(), eqs: Vec::new() });
            }
            comps[comp_of_root[r as usize] as usize].vars.push(vars[v as usize]);
        }
        for (ei, &f) in eq_first.iter().enumerate() {
            if f != NONE {
                let r = find(&mut cuf, f);
                comps[comp_of_root[r as usize] as usize].eqs.push(ei as u32);
            }
        }
        let mut out = Outcome {
            all_converged: true,
            comp_conflicts: Vec::new(),
            const_conflicts: Vec::new(),
            const_redundant: Vec::new(),
            iterations: 0,
            max_residual: 0.0,
            comps: Vec::new(),
            comp_ok: Vec::new(),
            comp_rows: Vec::new(),
            var_of,
        };
        // constant equations
        let mut buf: SmallVec<[f64; 16]> = SmallVec::new();
        for &ei in &const_eqs {
            let eq = &self.eqs[ei];
            self.gather(eq, &mut buf);
            let r = eq.eval(&buf);
            if let Src::Constraint(c) = self.src[ei] {
                if r.abs() > opts.tolerance.max(1e-9) {
                    out.const_conflicts.push(c);
                } else {
                    out.const_redundant.push(c);
                }
            }
        }
        let mut var_local = vec![NONE; nv];
        for comp in &comps {
            let x0: Vec<f64> = comp.vars.iter().map(|&pv| self.p.values[pv as usize]).collect();
            let mut res = self.solve_comp(comp, &mut var_local, &out.var_of, opts);
            // Drag polishing: re-pull dragged parameters to their targets so the
            // dragged geometry lands exactly on the cursor when feasible.
            for _ in 0..2 {
                if !res.converged || self.drag_target.is_empty() {
                    break;
                }
                let snap: Vec<f64> = comp.vars.iter().map(|&pv| self.p.values[pv as usize]).collect();
                let mut moved = false;
                for &pv in &comp.vars {
                    if let Some(&t) = self.drag_target.get(&pv)
                        && (self.p.values[pv as usize] - t).abs() > 1e-13 * (1.0 + t.abs()) {
                            self.p.values[pv as usize] = t;
                            moved = true;
                        }
                }
                if !moved {
                    break;
                }
                let res2 = self.solve_comp(comp, &mut var_local, &out.var_of, opts);
                if res2.converged {
                    res = res2;
                } else {
                    self.restore_comp(comp, &snap);
                    break;
                }
            }
            out.iterations = out.iterations.max(res.iterations);
            let maxr = res.residual.iter().fold(0.0f64, |a, v| a.max(v.abs()));
            out.max_residual = out.max_residual.max(maxr);
            if !res.converged {
                out.all_converged = false;
                let thr = (10.0 * opts.tolerance).max(1e-4 * maxr);
                let mut set = Vec::new();
                for (ri, &ei) in comp.eqs.iter().enumerate() {
                    if res.residual[ri].abs() > thr
                        && let Src::Constraint(c) = self.src[ei as usize]
                            && !set.contains(&c) {
                                set.push(c);
                            }
                }
                if !set.is_empty() {
                    out.comp_conflicts.push(set);
                }
                self.restore_comp(comp, &x0);
            }
            out.comp_ok.push(res.converged);
            out.comp_rows.push(res.rows);
        }
        out.comps = comps;
        out
    }
}

fn dedup_sorted(mut v: Vec<ConstraintId>) -> Vec<ConstraintId> {
    v.sort_unstable();
    v.dedup();
    v
}

/// Runs the solver on a sketch.
pub(crate) fn run(sk: &mut Sketch, opts: &SolveOptions, drag: &[DragTarget]) -> SolveReport {
    run_ext(sk, opts, drag, true)
}

pub(crate) fn run_ext(sk: &mut Sketch, opts: &SolveOptions, drag: &[DragTarget], allow_exclusion: bool) -> SolveReport {
    let mut excluded: HashSet<ConstraintId> = HashSet::new();
    let mut conflicting: Vec<ConstraintId> = Vec::new();
    let mut attempt = 0;
    if !opts.light.is_empty() && drag.is_empty() {
        // First try to move `light` alone (everything else held); the
        // weighted solve below then only analyses the result.
        let mut sys = System::build(sk, &excluded, drag);
        for i in sys.still_reps(sk, opts) {
            sys.p.fixed[i] = true;
        }
        let out = sys.solve(&SolveOptions { analyze: false, ..opts.clone() });
        if out.all_converged && out.comp_conflicts.is_empty() && out.const_conflicts.is_empty() {
            sys.write_back(sk);
        }
    }
    loop {
        let mut sys = System::build(sk, &excluded, drag);
        if !opts.light.is_empty() {
            sys.weigh_still(sk, opts);
        }
        let out = sys.solve(opts);
        // Drags never drop constraints: a failed drag keeps the last good state.
        let retry = allow_exclusion && drag.is_empty() && !out.all_converged && !out.comp_conflicts.is_empty() && attempt < 8;
        if retry {
            // Report the whole conflicting set, but only ignore the most
            // recently added constraint of each set so the rest still solves.
            for set in &out.comp_conflicts {
                conflicting.extend(set.iter().copied());
                if let Some(newest) = set.iter().max() {
                    excluded.insert(*newest);
                }
            }
            attempt += 1;
            continue;
        }
        conflicting.extend(out.const_conflicts.iter().copied());
        let conflicting = dedup_sorted(conflicting);
        let is_drag = !drag.is_empty();
        let status = if !out.all_converged {
            SolveStatus::Failed { reason: format!("solver did not converge (max residual {:.3e})", out.max_residual) }
        } else if conflicting.is_empty() {
            SolveStatus::Solved
        } else {
            SolveStatus::Overconstrained { conflicting: conflicting.clone() }
        };
        if !(is_drag && !out.all_converged) {
            sys.write_back(sk);
            sk.update_driven_dimensions();
        }
        let mut report = SolveReport {
            status,
            dof: None,
            entity_states: IndexMap::new(),
            redundant: Vec::new(),
            invalid: sys.invalid.clone(),
            iterations: out.iterations,
            max_residual: out.max_residual,
        };
        if opts.analyze {
            let a = analyze(sk, &mut sys, &out, &conflicting);
            sk.cache.analysis = Some((sk.cache.revision, a.clone()));
            report.dof = Some(a.dof);
            report.entity_states = a.states;
            report.redundant = a.redundant;
        } else if let Some((rev, a)) = &sk.cache.analysis
            && *rev == sk.cache.revision {
                report.dof = Some(a.dof);
                report.entity_states = a.states.clone();
                report.redundant = a.redundant.clone();
            }
        return report;
    }
}

fn analyze(sk: &Sketch, sys: &mut System, out: &Outcome, conflicting: &[ConstraintId]) -> Analysis {
    let np = sys.p.values.len();
    // free flag per variable index
    let nv = out.var_of.iter().filter(|v| **v != NONE).count();
    let mut var_free = vec![true; nv];
    let mut dof = 0usize;
    let mut redundant: Vec<ConstraintId> = sys.alias_redundant.clone();
    redundant.extend(out.const_redundant.iter().copied());
    for (ci, comp) in out.comps.iter().enumerate() {
        let rows = &out.comp_rows[ci];
        let a = analyze_component(rows, comp.vars.len());
        dof += comp.vars.len() - a.rank;
        for (k, &pv) in comp.vars.iter().enumerate() {
            var_free[out.var_of[pv as usize] as usize] = a.free[k];
        }
        for ri in a.redundant_rows {
            if let Src::Constraint(c) = sys.src[comp.eqs[ri] as usize] {
                redundant.push(c);
            }
        }
    }
    // unconstrained free parameters
    let mut param_free = vec![false; np];
    for (i, pf) in param_free.iter_mut().enumerate() {
        let r = find(&mut sys.uf, i as u32) as usize;
        if sys.p.fixed[r] {
            continue;
        }
        let v = out.var_of[r];
        *pf = if v == NONE { true } else { var_free[v as usize] };
    }
    for i in 0..np {
        let r = find(&mut sys.uf, i as u32) as usize;
        if r == i && !sys.p.fixed[i] && out.var_of[i] == NONE {
            dof += 1;
        }
    }
    let mut over: HashSet<EntityId> = HashSet::new();
    for cid in conflicting {
        if let Some(c) = sk.constraint(*cid) {
            over.extend(c.kind.entities());
        }
    }
    let mut states = IndexMap::new();
    for e in sk.entities() {
        let st = if over.contains(&e.id) {
            ConstraintState::OverConstrained
        } else if e.external {
            ConstraintState::FullyConstrained
        } else {
            let mut ids: SmallVec<[EntityId; 8]> = SmallVec::new();
            ids.push(e.id);
            ids.extend(e.point_refs());
            let mut free = false;
            for id in ids {
                let base = sys.p.ent_base.get(id.0 as usize).copied().unwrap_or(NONE);
                if base == NONE {
                    continue;
                }
                let cnt = sk.entity(id).map_or(0, |x| x.own_param_count());
                for k in 0..cnt {
                    if param_free[base as usize + k] {
                        free = true;
                    }
                }
            }
            if free { ConstraintState::UnderConstrained } else { ConstraintState::FullyConstrained }
        };
        states.insert(e.id, st);
    }
    Analysis { dof, states, redundant: dedup_sorted(redundant) }
}

impl Sketch {
    /// Options for solving after dimension `cid` got a new value: the
    /// geometry that takes the change (`SolveOptions::light`).
    ///
    /// A fillet arc's radius (Onshape, qa5 SK5: 30.089 × 22.065 rectangle,
    /// top-right R5 → R3 gives 28.089 × 22.065): the arc keeps its end point
    /// (counter-clockwise) on the line there, so that line keeps its
    /// length; the line at the arc's start moves with it, along with points
    /// on it (the virtual sharp).
    pub fn dimension_edit_options(&self, cid: ConstraintId) -> SolveOptions {
        let mut opts = SolveOptions::default();
        let Some(c) = self.constraint(cid) else { return opts };
        let (ConstraintKind::Radius { curve, .. } | ConstraintKind::Diameter { curve, .. }) = c.kind else { return opts };
        let Some(Geometry::Arc { start, end, .. }) = self.entity(curve).map(|e| &e.geometry) else { return opts };
        let (start, end) = (*start, *end);
        let tangent_lines: Vec<EntityId> = self
            .constraints()
            .filter(|t| t.driving)
            .filter_map(|t| match t.kind {
                ConstraintKind::Tangent { a, b } if b == curve => Some(a),
                ConstraintKind::Tangent { a, b } if a == curve => Some(b),
                _ => None,
            })
            .filter(|l| self.kind(*l) == Some(EntityKind::Line))
            .collect();
        let at = |p: EntityId| tangent_lines.iter().copied().find(|l| self.endpoints(*l).is_some_and(|(s, e)| s == p || e == p));
        let (Some(moved), Some(_kept)) = (at(start), at(end)) else { return opts };
        let on_moved = self.constraints().filter_map(|k| match k.kind {
            ConstraintKind::Coincident { a, b } if b == moved && self.kind(a) == Some(EntityKind::Point) => Some(a),
            ConstraintKind::Coincident { a, b } if a == moved && self.kind(b) == Some(EntityKind::Point) => Some(b),
            _ => None,
        });
        opts.light = [curve, moved].into_iter().chain(on_moved).collect();
        opts.still = vec![end];
        opts
    }

    /// Solves all driving constraints, warm-started from the current geometry.
    pub fn solve(&mut self) -> SolveReport {
        self.solve_with(&SolveOptions::default())
    }

    pub fn solve_with(&mut self, opts: &SolveOptions) -> SolveReport {
        let snapshot = self.clone();
        let rep = self.solve_inner(opts);
        // Live (qa4 round 5 v3 s6, CON-HV-COLLAPSE): constraints that
        // collapse a line to a point (Vertical on a Horizontal line) don't
        // solve: the line stays, its constraints are the conflicting ones
        // ("Sketch could not be solved.", red).
        let collapsed: Vec<EntityId> = self
            .entities()
            .filter(|e| e.kind() == EntityKind::Line)
            .filter(|e| {
                let len = |sk: &Sketch| match sk.curve(e.id) {
                    Some(crate::Curve2::Line { a, b }) => Some((b - a).length()),
                    _ => None,
                };
                matches!((len(&snapshot), len(self)), (Some(before), Some(after)) if before > 1e-9 && after <= 1e-9)
            })
            .map(|e| e.id)
            .collect();
        if collapsed.is_empty() {
            return rep;
        }
        *self = snapshot;
        let conflicting: Vec<ConstraintId> = self.constraints().filter(|c| c.driving && c.kind.entities().iter().any(|x| collapsed.contains(x))).map(|c| c.id).collect();
        let mut rep = rep;
        for id in &collapsed {
            rep.entity_states.insert(*id, ConstraintState::OverConstrained);
        }
        rep.status = SolveStatus::Overconstrained { conflicting };
        rep
    }

    fn solve_inner(&mut self, opts: &SolveOptions) -> SolveReport {
        let snapshot = self.clone();
        let rep = run_ext(self, opts, &[], false);
        if !matches!(rep.status, SolveStatus::Failed { .. }) {
            return rep;
        }
        // Homotopy: large dimension changes can leave Newton's basin of
        // convergence. Walk the driving dimensions from their currently
        // measured values to the targets in sub-steps.
        let quick = SolveOptions { analyze: false, ..opts.clone() };
        for steps in [4usize, 16, 64] {
            *self = snapshot.clone();
            let dims: Vec<(ConstraintId, f64, f64)> = self
                .constraints()
                .filter(|c| c.driving && c.kind.is_dimension())
                .filter_map(|c| {
                    let target = c.value()?;
                    let mut start = self.measure_kind(&c.kind, &c.hints)?;
                    if matches!(c.kind, ConstraintKind::Angle { .. } | ConstraintKind::ArcAngle { .. }) {
                        let d = (target - start + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI;
                        start = target - d;
                    }
                    Some((c.id, start, target))
                })
                .collect();
            if dims.is_empty() {
                break;
            }
            let mut ok = true;
            for k in 1..=steps {
                let f = k as f64 / steps as f64;
                for (id, a, b) in &dims {
                    if let Some(c) = self.constraint_mut_silent(*id) {
                        c.kind.set_value(if k == steps { *b } else { a + (b - a) * f });
                    }
                }
                let r = run_ext(self, &quick, &[], false);
                if matches!(r.status, SolveStatus::Failed { .. }) {
                    ok = false;
                    break;
                }
            }
            if ok {
                return run(self, opts, &[]);
            }
        }
        *self = snapshot;
        run(self, opts, &[])
    }

    /// Interactive drag: pulls the handles toward their targets while keeping
    /// every constraint satisfied. Fully constrained geometry does not move.
    /// On failure the geometry is left unchanged. Constraint states come from
    /// the last analysis if the sketch structure has not changed since.
    pub fn drag(&mut self, handles: &[(DragHandle, DVec2)]) -> SolveReport {
        let opts = SolveOptions { analyze: false, max_iterations: 50, ..SolveOptions::default() };
        let targets = self.drag_targets(handles);
        let report = run(self, &opts, &targets);
        if report.is_solved() {
            return report;
        }
        // Large jumps can leave the convergence basin: approach the target in
        // smaller steps (each step warm-starts the next).
        for substeps in [2usize, 4, 8] {
            let start = self.clone();
            let mut ok = true;
            let mut last = report.clone();
            for k in 1..=substeps {
                let f = k as f64 / substeps as f64;
                let partial: Vec<(DragHandle, DVec2)> = handles
                    .iter()
                    .map(|(h, t)| {
                        let from = match *h {
                            DragHandle::Point(id) => start.point(id).unwrap_or(*t),
                            DragHandle::Entity { grab, .. } => grab,
                        };
                        let target = from + (*t - from) * f;
                        // entity handles: the grab point moves with the previous sub-step
                        let h2 = match *h {
                            DragHandle::Entity { id, grab } => DragHandle::Entity { id, grab: grab + (*t - grab) * ((k - 1) as f64 / substeps as f64) },
                            p => p,
                        };
                        (h2, target)
                    })
                    .collect();
                let targets = self.drag_targets(&partial);
                last = run(self, &opts, &targets);
                if !last.is_solved() {
                    ok = false;
                    break;
                }
            }
            if ok {
                return last;
            }
            *self = start;
        }
        report
    }

    fn drag_targets(&self, handles: &[(DragHandle, DVec2)]) -> Vec<DragTarget> {
        let mut out = Vec::new();
        let push_pt = |out: &mut Vec<DragTarget>, id: EntityId, p: DVec2| {
            out.push(DragTarget::Coord { point: id, comp: 0, value: p.x });
            out.push(DragTarget::Coord { point: id, comp: 1, value: p.y });
        };
        for (h, target) in handles {
            match *h {
                DragHandle::Point(id) => {
                    if self.kind(id) == Some(EntityKind::Point) {
                        push_pt(&mut out, id, *target);
                    }
                }
                DragHandle::Entity { id, grab } => {
                    let Some(e) = self.entity(id) else { continue };
                    let delta = *target - grab;
                    match &e.geometry {
                        Geometry::Point { .. } => push_pt(&mut out, id, *target),
                        Geometry::Circle { center, .. } => {
                            if let Some(c) = self.point(*center) {
                                out.push(DragTarget::Own { entity: id, slot: 0, value: (*target - c).length() });
                            }
                        }
                        Geometry::Arc { center, start, end } => {
                            if let (Some(c), Some(s), Some(en)) = (self.point(*center), self.point(*start), self.point(*end)) {
                                let r = (*target - c).length();
                                push_pt(&mut out, *start, c + (s - c).normalize_or(DVec2::X) * r);
                                push_pt(&mut out, *end, c + (en - c).normalize_or(DVec2::X) * r);
                            }
                        }
                        _ => {
                            let mut refs = e.point_refs().to_vec();
                            refs.sort();
                            refs.dedup();
                            for p in refs {
                                if let Some(q) = self.point(p) {
                                    push_pt(&mut out, p, q + delta);
                                }
                            }
                        }
                    }
                }
            }
        }
        out
    }
}
