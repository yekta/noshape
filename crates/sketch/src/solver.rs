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
    /// A drag: the components holding the dragged geometry solved and it
    /// moved, whatever the rest of the sketch does (which `status` reports).
    #[serde(default)]
    pub drag_solved: bool,
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
    /// Points of `light` entities held in one coordinate (0 x, 1 y) only:
    /// a drag's Horizontal / Vertical line slides along itself.
    pub still_across: Vec<(EntityId, u8)>,
    /// A line may collapse to a point (a dimension value edit: live solves a
    /// fillet radius that shrinks the next line to nothing, or a length set
    /// to 0). Off, a solve collapsing a line is refused (CON-HV-COLLAPSE:
    /// Vertical added to a Horizontal line).
    pub allow_collapse: bool,
    /// Geometry the constraints don't hold yet is first placed the way live
    /// does (`Sketch::place_constructively`, fuzz probes 1–2): a
    /// regeneration's solve and a dimension edit. Off for the tools, which
    /// rely on the solve to pull new geometry onto its inferences (live
    /// places it there exactly), and for that pass's own solves.
    pub constructive: bool,
}

impl Default for SolveOptions {
    fn default() -> Self {
        SolveOptions { tolerance: 1e-10, max_iterations: 100, analyze: true, light: Vec::new(), still: Vec::new(), still_across: Vec::new(), allow_collapse: false, constructive: false }
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
    /// A held-direction line's direction (see [`Sketch::held_line`]).
    Held(EntityId, u8),
    /// The fixed 0 a held direction's length is measured from.
    Zero,
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
    held_base: HashMap<EntityId, u32>,
    zero: u32,
}

impl Params {
    fn build(sk: &Sketch) -> Params {
        let mut p = Params {
            values: Vec::new(),
            fixed: Vec::new(),
            owner: Vec::new(),
            ent_base: vec![NONE; sk.max_entity_id() as usize + 1],
            aux_base: HashMap::new(),
            held_base: HashMap::new(),
            zero: NONE,
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
        // A line a sketch fillet consumed: the direction it keeps.
        for e in sk.entities() {
            let Some((_, dir)) = sk.held_line(e.id) else { continue };
            if p.zero == NONE {
                p.zero = p.values.len() as u32;
                p.values.push(0.0);
                p.fixed.push(true);
                p.owner.push(Owner::Zero);
            }
            p.held_base.insert(e.id, p.values.len() as u32);
            for (k, v) in [dir.x, dir.y].into_iter().enumerate() {
                p.values.push(v);
                p.fixed.push(e.external);
                p.owner.push(Owner::Held(e.id, k as u8));
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
    fn held_dir(&self, id: EntityId) -> Option<([u32; 2], u32)> {
        self.p.held_base.get(&id).map(|b| ([*b, b + 1], self.p.zero))
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
/// Weight of a held-direction line's direction ([`Owner::Held`]).
const HELD_WEIGHT: f64 = 1e-10;

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
        // (Sets: a big drag's `still` lists thousands of points, scanned once per parameter.)
        let still: HashSet<EntityId> = opts.still.iter().copied().collect();
        let across: HashSet<(EntityId, u8)> = opts.still_across.iter().copied().collect();
        for i in 0..self.p.owner.len() {
            if let Owner::Ent(id, k) = self.p.owner[i]
                && (still.contains(&id) || across.contains(&(id, k)))
            {
                light_reps.remove(&find(&mut self.uf, i as u32));
            }
        }
        (0..self.weight.len())
            .filter(|&i| find(&mut self.uf, i as u32) as usize == i && !light_reps.contains(&(i as u32)) && !self.p.fixed[i] && self.weight[i] < DRAG_WEIGHT && !matches!(self.p.owner[i], Owner::Held(..)))
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
        // Turning a zero-length line's kept direction moves nothing drawn: it costs (almost) nothing.
        for (w, o) in sys.weight.iter_mut().zip(&sys.p.owner) {
            if matches!(o, Owner::Held(..)) {
                *w = HELD_WEIGHT;
            }
        }
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
        // Already within tolerance (a drag's untouched components): no factorisation set up.
        if r.iter().all(|v| v.abs() <= opts.tolerance) {
            return CompSolve { converged: true, iterations: 0, residual: r, rows };
        }

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
            // A plateau (conflicting constraints at their least-squares
            // minimum) ends the solve after two steps that gain nothing: a
            // converging solve gains orders of magnitude a step (a
            // conflicting dimension on a 300-entity sketch took ~40 ms of
            // such steps).
            if iterations >= opts.max_iterations || stall >= 2 {
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
                        if cost - c_new <= 1e-6 * cost {
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
        let mut held: HashMap<EntityId, [f64; 2]> = HashMap::new();
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
                Owner::Held(id, slot) => held.entry(id).or_default()[slot as usize & 1] = v,
                Owner::Zero => {}
            }
        }
        for (id, [x, y]) in held {
            let dir = DVec2::new(x, y);
            if let Some(e) = sk.entity_mut_silent(id)
                && !e.external
                && dir.length() > 1e-12
            {
                e.held_direction = Some(dir.normalize());
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
        // A drag solves only the components holding dragged geometry; the
        // others are evaluated where they are (CONFLICT-COLLAPSE-ON-DRAG: a
        // line with conflicting Horizontal + Vertical collapsed to a point,
        // "solved", when another entity was dragged; live leaves it as is).
        let held = SolveOptions { max_iterations: 0, ..opts.clone() };
        for comp in &comps {
            let x0: Vec<f64> = comp.vars.iter().map(|&pv| self.p.values[pv as usize]).collect();
            let solve = self.drag_target.is_empty() || comp.vars.iter().any(|v| self.drag_target.contains_key(v));
            let mut res = self.solve_comp(comp, &mut var_local, &out.var_of, if solve { opts } else { &held });
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

/// Moves only `opts.light` (with its points, less `opts.still`), every
/// other parameter held; true (and written back) when that solves.
pub(crate) fn solve_held(sk: &mut Sketch, opts: &SolveOptions, drag: &[DragTarget]) -> bool {
    let mut sys = System::build(sk, &HashSet::new(), drag);
    for i in sys.still_reps(sk, opts) {
        sys.p.fixed[i] = true;
    }
    let out = sys.solve(&SolveOptions { analyze: false, ..opts.clone() });
    let ok = out.all_converged && out.comp_conflicts.is_empty() && out.const_conflicts.is_empty();
    if ok {
        sys.write_back(sk);
    }
    ok
}

/// Runs the solver on a sketch.
pub(crate) fn run(sk: &mut Sketch, opts: &SolveOptions, drag: &[DragTarget]) -> SolveReport {
    run_ext(sk, opts, drag, true)
}

pub(crate) fn run_ext(sk: &mut Sketch, opts: &SolveOptions, drag: &[DragTarget], allow_exclusion: bool) -> SolveReport {
    if !opts.light.is_empty() {
        // First try to move `light` alone (everything else held; a drag's
        // dimensioned geometry, `Sketch::drag_weights`); the weighted solve
        // then only analyses the result.
        solve_held(sk, opts, drag);
    }
    run_weighted(sk, opts, drag, allow_exclusion)
}

/// `run_ext` after its held step.
fn run_weighted(sk: &mut Sketch, opts: &SolveOptions, drag: &[DragTarget], allow_exclusion: bool) -> SolveReport {
    let mut excluded: HashSet<ConstraintId> = HashSet::new();
    let mut conflicting: Vec<ConstraintId> = Vec::new();
    let mut attempt = 0;
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
        // A drag moves what it drags when the components holding the dragged
        // geometry solve, whatever the rest of the sketch does: components
        // that fail keep their state (`System::solve` restores them), and
        // conflicts elsewhere don't stop it (a sketch with one conflicting
        // constraint let nothing be dragged, not even a free point).
        // The status is the whole sketch's, for a drag too: Solved only when
        // every equation is within the tolerance (SKETCH-SOLVED-WITH-RESIDUAL:
        // a drag beside a failing component reported Solved with its residual).
        let drag_ok = is_drag && out.comps.iter().zip(&out.comp_ok).all(|(c, ok)| *ok || !c.vars.iter().any(|v| sys.drag_target.contains_key(v)));
        let failed = || SolveStatus::Failed { reason: format!("solver did not converge (max residual {:.3e})", out.max_residual) };
        // A driving dimension the rest already fixes over-defines the sketch
        // (live fuzz s7: a radius dimensioned twice, both 13.096 mm: "Sketch
        // could not be solved."), where a redundant relation (a second
        // Perpendicular, an Equal twice) doesn't.
        let analysis = (opts.analyze && !is_drag).then(|| analyze(sk, &mut sys, &out, &conflicting));
        let mut conflicting = conflicting;
        // A conflict's driving dimensions include the ones the sketch solves without (live errors-1118 / recheck 11-19:
        // a rectangle's 50 typed on the side opposite its 40, both red, not the 30).
        if analysis.is_some()
            && !conflicting.is_empty()
            && let Some(dims) = conflict_dims(sk, &conflicting)
        {
            conflicting = dedup_sorted(conflicting.into_iter().chain(dims).collect());
        }
        if let Some(a) = &analysis {
            let dims: Vec<ConstraintId> = a.redundant.iter().copied().filter(|c| sk.constraint(*c).is_some_and(|c| c.driving && c.kind.is_dimension())).collect();
            let dims = if dims.is_empty() { dims } else { redundant_nearby(sk, dims) };
            if !dims.is_empty() {
                conflicting = dedup_sorted(conflicting.into_iter().chain(dims).collect());
            }
        }
        let status = if !out.all_converged || out.max_residual > opts.tolerance {
            failed()
        } else if conflicting.is_empty() {
            SolveStatus::Solved
        } else {
            SolveStatus::Overconstrained { conflicting: conflicting.clone() }
        };
        if !is_drag || drag_ok {
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
            drag_solved: drag_ok,
        };
        if opts.analyze {
            let a = match analysis {
                Some(a) if conflicting.is_empty() => a,
                _ => analyze(sk, &mut sys, &out, &conflicting),
            };
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

/// Of the driving dimensions `dims` the rest already fixes here, those it still fixes on a nearby configuration: the
/// rest settled from a nudged start with the dimension driven, then the dimension driving at what it measures there.
/// One that only meets the rest at a singular point is kept, as live keeps it (asm-welding-config 9c7704's Sketch 13:
/// an arc's ends held at two heights, its centre midway, its Ø their gap: only their dx = 0 is left to it); one the
/// rest fixes everywhere stays redundant (live 1862: a Ø10 circle tangent to two lines 10 mm apart, 10 dimensioned).
fn redundant_nearby(sk: &Sketch, dims: Vec<ConstraintId>) -> Vec<ConstraintId> {
    thread_local! { static INSIDE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }
    if INSIDE.with(|c| c.get()) {
        return dims;
    }
    INSIDE.with(|c| c.set(true));
    let size = sk.bbox().size().max_element().max(1e-3);
    let kept = dims
        .into_iter()
        .filter(|&c| {
            let mut probe = sk.clone();
            if probe.set_driving(c, false).is_err() {
                return true;
            }
            let points: Vec<(EntityId, DVec2)> = probe.entities().filter(|e| !e.external && e.kind() == EntityKind::Point).filter_map(|e| Some((e.id, probe.point(e.id)?))).collect();
            for (k, (id, p)) in points.into_iter().enumerate() {
                let t = (k as f64 + 1.0) * 0.618_033_988_75;
                let _ = probe.set_point(id, p + DVec2::new(t.fract() - 0.5, (t * 1.7).fract() - 0.5) * size * 1e-3);
            }
            if !probe.solve_with(&SolveOptions { analyze: false, ..Default::default() }).is_solved() || probe.set_driving(c, true).is_err() {
                return true;
            }
            probe.solve_with(&SolveOptions::default()).redundant.contains(&c)
        })
        .collect();
    INSIDE.with(|c| c.set(false));
    kept
}

/// More driving dimensions of a conflict that has one in `conflicting`: those in its piece of the sketch it solves
/// without (at most 12 tried: a broken sketch solves on every edit; solves without analysis, so no recursion). None
/// when none does (a conflict among relations): the least squares' set stands.
fn conflict_dims(sk: &Sketch, conflicting: &[ConstraintId]) -> Option<Vec<ConstraintId>> {
    let dim = |c: &ConstraintId| sk.constraint(*c).is_some_and(|k| k.driving && k.kind.is_dimension());
    let first = conflicting.iter().find(|c| dim(c)).and_then(|c| sk.constraint(*c)?.kind.entities().first().copied())?;
    let piece = sk.free_piece(first);
    let dims: Vec<ConstraintId> = sk
        .constraints()
        .filter(|c| c.driving && c.kind.is_dimension() && c.kind.entities().iter().all(|e| piece.contains(e) || sk.entity(*e).is_some_and(|x| x.external)))
        .map(|c| c.id)
        .take(12)
        .collect();
    let freeing: Vec<ConstraintId> = dims
        .into_iter()
        .filter(|&c| {
            let mut probe = sk.clone();
            probe.set_driving(c, false).is_ok() && probe.solve_with(&SolveOptions { analyze: false, ..Default::default() }).is_solved()
        })
        .collect();
    (!freeing.is_empty()).then_some(freeing)
}

/// The driving dimension equation `eq` comes from, if any.
fn driving_dimension(sk: &Sketch, sys: &System, eq: u32) -> Option<ConstraintId> {
    match sys.src[eq as usize] {
        Src::Constraint(c) => sk.constraint(c).filter(|k| k.driving && k.kind.is_dimension()).map(|_| c),
        _ => None,
    }
}

/// A line of no length or a circle / arc of no radius.
fn has_collapsed_curve(sk: &Sketch) -> bool {
    sk.entities().filter_map(|e| sk.curve(e.id)).any(|c| match c {
        crate::geom::Curve2::Line { a, b } => (b - a).length() < 1e-9,
        crate::geom::Curve2::Circle { radius, .. } | crate::geom::Curve2::Arc { radius, .. } => radius < 1e-9,
        _ => false,
    })
}

fn analyze(sk: &Sketch, sys: &mut System, out: &Outcome, conflicting: &[ConstraintId]) -> Analysis {
    let np = sys.p.values.len();
    // free flag per variable index
    let nv = out.var_of.iter().filter(|v| **v != NONE).count();
    let mut var_free = vec![true; nv];
    let mut dof = 0usize;
    let mut redundant: Vec<ConstraintId> = sys.alias_redundant.clone();
    redundant.extend(out.const_redundant.iter().copied());
    let mut collapsed: Option<bool> = None;
    // Live (behaviours "sketch snapping and hover" and live2's colours, 11-13): a line or an arc is coloured by its own
    // geometry (the infinite line; the arc's circle), its points apart: one whose free ends only slide along it is fully
    // defined. Per component, the motions that move it ([`own_motions`]) are asked of the rank analysis.
    let (line_queries, mut line_moves) = line_motion_queries(sk, sys, out);
    for (ci, comp) in out.comps.iter().enumerate() {
        let rows = &out.comp_rows[ci];
        let queries: Vec<SparseRow> = line_queries[ci].iter().map(|(_, q)| q.clone()).collect();
        let a = crate::dof::analyze_component_with(rows, comp.vars.len(), &queries);
        for ((line, _), moves) in line_queries[ci].iter().zip(&a.query_free) {
            if *moves {
                line_moves.insert(*line, true);
            }
        }
        dof += comp.vars.len() - a.rank;
        for (k, &pv) in comp.vars.iter().enumerate() {
            var_free[out.var_of[pv as usize] as usize] = a.free[k];
        }
        // A dependency that takes in a dimension makes that dimension
        // redundant, whichever row the rank analysis gave up (SKETCH-REDUNDANT-SOLVES,
        // live 1862: a Ø10 circle Tangent to two lines 10 mm apart, the
        // 10 mm dimensioned too: "Sketch could not be solved."). Not where a
        // curve has collapsed: the rank drops there without a redundancy
        // (live solves a rectangle's height set to 0).
        if !a.redundant_rows.is_empty() && !a.redundant_rows.iter().any(|ri| driving_dimension(sk, sys, comp.eqs[*ri]).is_some()) {
            let dims: Vec<usize> = (0..rows.len()).filter(|ri| driving_dimension(sk, sys, comp.eqs[*ri]).is_some()).collect();
            let without = |skip: &dyn Fn(usize) -> bool| -> Vec<SparseRow> { rows.iter().enumerate().filter(|(k, _)| !skip(*k)).map(|(_, r)| r.clone()).collect() };
            // (One analysis without any dimension first: the same
            // deficiency there means none takes part.)
            let relations = without(&|k| dims.contains(&k));
            let involved = !dims.is_empty() && rows.len() - a.rank != relations.len() - analyze_component(&relations, comp.vars.len()).rank;
            let col = involved
                && *collapsed.get_or_insert_with(|| {
                    let mut solved = sk.clone();
                    sys.write_back(&mut solved);
                    has_collapsed_curve(&solved)
                });
            if involved && !col {
                for &ri in &dims {
                    if analyze_component(&without(&|k| k == ri), comp.vars.len()).rank == a.rank
                        && let Some(c) = driving_dimension(sk, sys, comp.eqs[ri])
                    {
                        redundant.push(c);
                    }
                }
            }
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
        } else if let Some(moves) = line_moves.get(&e.id) {
            if *moves { ConstraintState::UnderConstrained } else { ConstraintState::FullyConstrained }
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

/// Per component: (curve, a query of a motion that moves it).
type LineQueries = Vec<Vec<(EntityId, SparseRow)>>;

/// The motions that move a curve's own geometry, each as point motions dotted with a direction: a line's two ends
/// across it (sliding along it doesn't move the line); an arc's centre, and its start's radial motion against the
/// centre (its circle; the ends sliding along it don't move it). Other curves: None (their points' rule).
fn own_motions(sk: &Sketch, e: &crate::entities::Entity) -> Option<Vec<Vec<(EntityId, DVec2)>>> {
    match e.geometry {
        Geometry::Line { start, end } => {
            let (pa, pb) = (sk.point(start)?, sk.point(end)?);
            let n = (pb - pa).try_normalize()?.perp();
            Some(vec![vec![(start, n)], vec![(end, n)]])
        }
        Geometry::Arc { center, start, .. } => {
            let (c, a) = (sk.point(center)?, sk.point(start)?);
            let u = (a - c).try_normalize()?;
            Some(vec![vec![(center, DVec2::X)], vec![(center, DVec2::Y)], vec![(start, u), (center, -u)]])
        }
        _ => None,
    }
}

/// Per component, each line's and arc's own motions ([`own_motions`]) as queries of its unknowns
/// ([`crate::dof::analyze_component_with`]), and every curve asked about (true already for one with an unknown no
/// equation holds in such a motion).
fn line_motion_queries(sk: &Sketch, sys: &mut System, out: &Outcome) -> (LineQueries, std::collections::HashMap<EntityId, bool>) {
    let mut local: std::collections::HashMap<u32, (usize, u32)> = std::collections::HashMap::new();
    for (ci, comp) in out.comps.iter().enumerate() {
        for (k, &pv) in comp.vars.iter().enumerate() {
            local.insert(pv, (ci, k as u32));
        }
    }
    let mut queries: LineQueries = vec![Vec::new(); out.comps.len()];
    let mut moves = std::collections::HashMap::new();
    for e in sk.entities() {
        if e.external {
            continue;
        }
        let Some(motions) = own_motions(sk, e) else { continue };
        let mut free = false;
        for motion in motions {
            // This motion, split by the components its unknowns fall in.
            let mut parts: std::collections::BTreeMap<usize, SparseRow> = std::collections::BTreeMap::new();
            for (p, dir) in motion {
                let base = sys.p.ent_base.get(p.0 as usize).copied().unwrap_or(NONE);
                if base == NONE {
                    continue;
                }
                for (i, c) in [dir.x, dir.y].into_iter().enumerate() {
                    let r = find(&mut sys.uf, base + i as u32) as usize;
                    if sys.p.fixed[r] || c.abs() < 1e-12 {
                        continue;
                    }
                    match local.get(&(r as u32)) {
                        Some(&(ci, k)) => {
                            let row = parts.entry(ci).or_default();
                            match row.iter_mut().find(|(col, _)| *col == k) {
                                Some(x) => x.1 += c,
                                None => row.push((k, c)),
                            }
                        }
                        None if out.var_of[r] == NONE => free = true,
                        None => {}
                    }
                }
            }
            for (ci, row) in parts {
                queries[ci].push((e.id, row));
            }
        }
        moves.insert(e.id, free);
    }
    (queries, moves)
}

/// Held points a drag past its dimension hold keeps in place one coordinate at a time (each a solve).
const MAX_KEPT: usize = 8;

impl Sketch {
    /// Driving constraints that don't hold as the geometry stands (within
    /// `tol`): the equations' residuals, and coincident points / concentric
    /// centers apart (one unknown in the solve, so no equation shows them).
    pub(crate) fn unheld_constraints(&self, tol: f64) -> HashSet<ConstraintId> {
        let sys = System::build(self, &HashSet::new(), &[]);
        let mut out = HashSet::new();
        let mut buf: SmallVec<[f64; 16]> = SmallVec::new();
        for (i, eq) in sys.eqs.iter().enumerate() {
            sys.gather(eq, &mut buf);
            if let Src::Constraint(c) = sys.src[i]
                && eq.eval(&buf).abs().partial_cmp(&tol) != Some(std::cmp::Ordering::Less)
            {
                out.insert(c);
            }
        }
        for c in self.constraints().filter(|c| c.driving) {
            if let Some((a, b)) = alias_pair(self, &c.kind)
                && let (Some(pa), Some(pb)) = (self.point(a), self.point(b))
                && (pa - pb).length() > tol
            {
                out.insert(c.id);
            }
        }
        out
    }

    /// Options for solving after dimension `cid` got a new value in the
    /// editor: live's least-change placement (`constructive`: with other
    /// dimensions the far side takes all of it, live probes 8 e1–e6), after
    /// [`Sketch::scale_for_edit`] scaled a free profile (live 1775 / 1785);
    /// or the geometry that takes the change (`SolveOptions::light`).
    ///
    /// A fillet arc's radius (Onshape, qa5 SK5: 30.089 × 22.065 rectangle,
    /// top-right R5 → R3 gives 28.089 × 22.065): the arc keeps its end point
    /// (counter-clockwise) on the line there, so that line keeps its
    /// length; the line at the arc's start moves with it, along with points
    /// on it (the virtual sharp).
    pub fn dimension_edit_options(&self, cid: ConstraintId) -> SolveOptions {
        let mut opts = SolveOptions { allow_collapse: true, constructive: true, ..Default::default() };
        let Some(c) = self.constraint(cid) else { return opts };
        // An offset's distance: its sources take the change, the copies stay (live psb 10-24 j (c): 5 -> 3 on a free
        // r5 circle's copy, the copy keeps r10, the source grows to r7).
        if let ConstraintKind::Distance { a, b, .. } = c.kind {
            let sources: Vec<EntityId> = self
                .constraints()
                .filter_map(|o| match o.kind {
                    ConstraintKind::OffsetPair { source, master_source, master_copy, .. } if (master_source, master_copy) == (a, b) || (master_source, master_copy) == (b, a) => Some(source),
                    _ => None,
                })
                .collect();
            if !sources.is_empty() {
                opts.light = sources;
                return opts;
            }
        }
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

    /// A free line's length set anew (its typed box, or its dimension edited when the sketch isn't scaled): its start,
    /// the first click, moves along it and its end stays (live 2022, all four directions). Free: nothing else holds
    /// the line or its end points.
    pub fn free_line_edit_options(&self, cid: ConstraintId) -> Option<SolveOptions> {
        let c = self.constraint(cid)?;
        let ConstraintKind::Length { curve, .. } = c.kind else { return None };
        let (start, end) = self.endpoints(curve).filter(|_| self.kind(curve) == Some(EntityKind::Line))?;
        let mine = [curve, start, end];
        if self.constraints().any(|k| k.id != cid && k.kind.entities().iter().any(|e| mine.contains(e))) {
            return None;
        }
        if self.entities().any(|x| x.id != curve && x.point_refs().iter().any(|p| *p == start || *p == end)) {
            return None;
        }
        Some(SolveOptions { light: vec![curve, start], still: vec![end], ..Default::default() })
    }

    /// A sketch's first dimension, a length or distance, scales the whole sketch uniformly onto its value, about the
    /// centre of its geometry's box, or about the one Origin point it's fixed to (live 2022: two free lines, the first
    /// dimensioned 18.543 -> 15 mm, both ×0.8089 about their box's centre; 1785: a corner rectangle's width 40 -> 50
    /// scales it ×1.25, height too; 1775: a lone line grows about its midpoint). Not a radius (live SK5: a fillet's
    /// new radius moves the edges beside it, the rectangle keeps its size). Only when the sketch has no other
    /// dimension and nothing fixed; returns whether it scaled (the solve after then finds everything held).
    pub fn scale_for_edit(&mut self, cid: ConstraintId) -> bool {
        let Some(c) = self.constraint(cid).filter(|c| c.driving).cloned() else { return false };
        let (ConstraintKind::Length { value, .. } | ConstraintKind::Distance { value, .. }) = c.kind else { return false };
        // An offset's distance is no first dimension (live psb 10-24 j (c): the free source moves, nothing scales).
        if let ConstraintKind::Distance { a, b, .. } = c.kind
            && self.constraints().any(|o| matches!(o.kind, ConstraintKind::OffsetPair { source, copy, .. } if (source, copy) == (a, b) || (source, copy) == (b, a)))
        {
            return false;
        }
        let Some(now) = self.measure_new(&c.kind).filter(|m| *m > 1e-12) else { return false };
        let k = value / now;
        if !k.is_finite() || k <= 0.0 || (k - 1.0).abs() < 1e-12 {
            return false;
        }
        // The sketch's first dimension: no other, nothing fixed (live 2022: once one is there, the next only moves its
        // own geometry).
        if self.constraints().any(|k| k.driving && k.id != cid && (k.kind.is_dimension() || matches!(k.kind, ConstraintKind::Fix { .. }))) {
            return false;
        }
        // The whole sketch, and the outside points its constraints hold it to.
        let mut set: HashSet<EntityId> = self.entities().filter(|x| !x.external).map(|x| x.id).collect();
        for k in self.constraints().filter(|k| k.driving) {
            let es = k.kind.entities();
            if es.iter().any(|e| set.contains(e)) {
                set.extend(es.iter().copied().filter(|e| self.entity(*e).is_some_and(|x| x.external)));
            }
        }
        let external: Vec<EntityId> = set.iter().copied().filter(|e| self.entity(*e).is_some_and(|x| x.external)).collect();
        if external.iter().any(|e| self.kind(*e) != Some(EntityKind::Point)) || external.len() > 1 {
            return false;
        }
        let pts: Vec<EntityId> = set.iter().copied().filter(|e| self.kind(*e) == Some(EntityKind::Point) && !external.contains(e)).collect();
        let pivot = match external.first().and_then(|e| self.point(*e)) {
            Some(p) => p,
            None => {
                let (mut lo, mut hi) = (DVec2::splat(f64::INFINITY), DVec2::splat(f64::NEG_INFINITY));
                for p in pts.iter().filter_map(|p| self.point(*p)) {
                    lo = lo.min(p);
                    hi = hi.max(p);
                }
                if !lo.is_finite() {
                    return false;
                }
                (lo + hi) * 0.5
            }
        };
        for p in &pts {
            if let Some(x) = self.point(*p) {
                let _ = self.set_point(*p, pivot + (x - pivot) * k);
            }
        }
        for e in &set {
            if let Some(x) = self.entity_mut_silent(*e) {
                match &mut x.geometry {
                    Geometry::Circle { radius, .. } => *radius *= k,
                    Geometry::Ellipse { major, minor, .. } | Geometry::EllipticalArc { major, minor, .. } => {
                        *major *= k;
                        *minor *= k;
                    }
                    _ => {}
                }
            }
        }
        self.touch();
        true
    }

    /// Free arcs keep their radius on curves that changed (live, `Sketch::refit_free_arcs`): refitted from where they
    /// were before this solve, kept when the sketch then solves without moving a point. Not what a drag holds.
    fn keep_free_arcs(&mut self, before: &Sketch, opts: &SolveOptions) -> Option<SolveReport> {
        let held = |e: EntityId| opts.light.contains(&e) || opts.still.contains(&e);
        let was = |p: EntityId| before.point(p);
        let arcs: Vec<EntityId> = self
            .entities()
            .filter(|e| e.kind() == EntityKind::Arc && !held(e.id))
            .filter(|e| match e.geometry {
                Geometry::Arc { center, start, end } => ![center, start, end].into_iter().any(held) && [start, end].into_iter().any(|p| was(p).zip(self.point(p)).is_some_and(|(a, b)| (a - b).length() > 1e-12)),
                _ => false,
            })
            .map(|e| e.id)
            .collect();
        if arcs.is_empty() {
            return None;
        }
        let mut trial = self.clone();
        if !trial.refit_free_arcs(&arcs, &was) {
            return None;
        }
        let at = trial.clone();
        let rep = trial.solve_inner(opts);
        let moved = trial.entities().any(|e| match (at.entity(e.id).map(|a| &a.geometry), &e.geometry) {
            (Some(Geometry::Point { pos: p }), Geometry::Point { pos: q }) => (*p - *q).length() > 1e-9,
            _ => false,
        });
        (rep.is_solved() && !moved).then(|| {
            *self = trial;
            rep
        })
    }

    /// Solves all driving constraints, warm-started from the current geometry.
    pub fn solve(&mut self) -> SolveReport {
        self.solve_with(&SolveOptions::default())
    }

    pub fn solve_with(&mut self, opts: &SolveOptions) -> SolveReport {
        let snapshot = self.clone();
        // The DOF analysis is the constraints', not where a least-change placement moves things: a solve without
        // `analyze` keeps reporting the one current when it started (a regeneration's placement touched the sketch,
        // and the open sketch's row lost its under-defined badge, user 11-12).
        let analysis = if !opts.analyze && self.analysis_current() { self.cache.analysis.clone() } else { None };
        // Live's least change first (constructive.rs); kept only when the
        // solve then finds every constraint held where it put things.
        let unheld = if opts.constructive && opts.light.is_empty() && opts.still.is_empty() { self.unheld_constraints(1e-9) } else { HashSet::new() };
        let rep = if !unheld.is_empty() {
            let left: HashSet<EntityId> = self.place_constructively(&unheld).into_iter().collect();
            let at = self.clone();
            let rep = self.solve_inner(opts);
            let moved = || self.entities().filter(|e| !left.contains(&e.id)).any(|e| at.entity(e.id).is_none_or(|a| match (&a.geometry, &e.geometry) {
                (Geometry::Point { pos: p }, Geometry::Point { pos: q }) => (*p - *q).length() > 1e-9,
                _ => false,
            }));
            if rep.is_solved() && !moved() && self.collapsed_lines(&snapshot).is_empty() {
                rep
            } else {
                *self = snapshot.clone();
                self.solve_inner(opts)
            }
        } else {
            self.solve_inner(opts)
        };
        // A shape its constraints hold, pulled only by Coincidents onto fixed points, slides there whole when the solve
        // fails (live: c2-loft-guide-curves-wave-washer's Sketch 4 rectangles, a far corner 63.5 mm off a model
        // vertex, move over; ours diverged: the Minimum distance between two sides broke once the solve sheared them).
        let rep = if rep.is_solved() { rep } else { self.carried_onto_anchors(&snapshot, opts).unwrap_or(rep) };
        let rep = if rep.is_solved() { self.keep_free_arcs(&snapshot, opts).unwrap_or(rep) } else { rep };
        let mut rep = rep;
        if let Some((_, a)) = analysis
            && rep.dof.is_none()
        {
            rep.dof = Some(a.dof);
            rep.entity_states = a.states.clone();
            rep.redundant = a.redundant.clone();
            self.cache.analysis = Some((self.cache.revision, a));
        }
        // Live (qa4 round 5 v3 s6, CON-HV-COLLAPSE): constraints that
        // collapse a line to a point (Vertical on a Horizontal line) don't
        // solve: the line stays, its constraints are the conflicting ones
        // ("Sketch could not be solved.", red).
        let collapsed = if opts.allow_collapse { Vec::new() } else { self.collapsed_lines(&snapshot) };
        if collapsed.is_empty() {
            return rep;
        }
        *self = snapshot;
        self.collapse_report(rep, &collapsed)
    }

    /// [`Self::solve_with`]'s retry: the free geometry of each point a Coincident ties to a fixed (external) point
    /// elsewhere moved rigidly onto it, kept when every constraint then holds and the solve passes. None (and self as it
    /// was) otherwise, or when the pulls on one piece differ.
    fn carried_onto_anchors(&mut self, before: &Sketch, opts: &SolveOptions) -> Option<SolveReport> {
        let fixed = |e: EntityId| before.entity(e).is_some_and(|x| x.external);
        let mut pulls = Vec::new();
        for c in before.constraints().filter(|c| c.driving) {
            let ConstraintKind::Coincident { a, b } = c.kind else { continue };
            let (mine, anchor) = match (fixed(a), fixed(b)) {
                (false, true) => (a, b),
                (true, false) => (b, a),
                _ => continue,
            };
            let (Some(p), Some(q)) = (before.point(mine), before.point(anchor)) else { continue };
            if (q - p).length() > 1e-9 {
                pulls.push((mine, q - p));
            }
        }
        if pulls.is_empty() {
            return None;
        }
        let mut carried = before.clone();
        let mut shift_of: HashMap<EntityId, DVec2> = HashMap::new();
        for (p, d) in pulls {
            if let Some(s) = shift_of.get(&p) {
                if (*s - d).length() > 1e-9 {
                    return None;
                }
                continue;
            }
            let piece = before.free_piece(p);
            if piece.iter().any(|e| before.entity(*e).is_some_and(|x| x.kind() != EntityKind::Point && x.point_refs().is_empty())) {
                return None;
            }
            for e in piece {
                if let Some(q) = before.point(e) {
                    carried.set_point(e, q + d).ok()?;
                }
                shift_of.insert(e, d);
            }
        }
        if !carried.unheld_constraints(1e-9).is_empty() {
            return None;
        }
        let failed = std::mem::replace(self, carried);
        let rep = self.solve_inner(opts);
        if rep.is_solved() && self.collapsed_lines(before).is_empty() {
            return Some(rep);
        }
        *self = failed;
        None
    }

    /// The free (not external) entities joined to `start` by shared points and driving constraints.
    fn free_piece(&self, start: EntityId) -> HashSet<EntityId> {
        let free = |e: EntityId| self.entity(e).is_some_and(|x| !x.external);
        let mut next: HashMap<EntityId, Vec<EntityId>> = HashMap::new();
        let mut link = |a: EntityId, b: EntityId| {
            next.entry(a).or_default().push(b);
            next.entry(b).or_default().push(a);
        };
        for e in self.entities().filter(|e| !e.external) {
            for p in e.point_refs().into_iter().filter(|p| free(*p)) {
                link(e.id, p);
            }
        }
        for c in self.constraints().filter(|c| c.driving) {
            let es: Vec<EntityId> = c.kind.entities().into_iter().filter(|e| free(*e)).collect();
            for w in es.windows(2) {
                link(w[0], w[1]);
            }
        }
        let mut seen = HashSet::from([start]);
        let mut todo = vec![start];
        while let Some(e) = todo.pop() {
            for n in next.get(&e).into_iter().flatten() {
                if seen.insert(*n) {
                    todo.push(*n);
                }
            }
        }
        seen
    }

    /// `rep` for a solve reverted because it collapsed `collapsed`: their
    /// constraints are the conflicting ones.
    fn collapse_report(&self, mut rep: SolveReport, collapsed: &[EntityId]) -> SolveReport {
        let conflicting: Vec<ConstraintId> = self.constraints().filter(|c| c.driving && c.kind.entities().iter().any(|x| collapsed.contains(x))).map(|c| c.id).collect();
        for id in collapsed {
            rep.entity_states.insert(*id, ConstraintState::OverConstrained);
        }
        rep.status = SolveStatus::Overconstrained { conflicting };
        rep.drag_solved = false;
        rep
    }

    /// Lines of `before` that have collapsed to a point here.
    fn collapsed_lines(&self, before: &Sketch) -> Vec<EntityId> {
        let len = |sk: &Sketch, id| match sk.curve(id) {
            Some(crate::Curve2::Line { a, b }) => Some((b - a).length()),
            _ => None,
        };
        self.entities()
            .filter(|e| e.kind() == EntityKind::Line)
            .filter(|e| matches!((len(before, e.id), len(self, e.id)), (Some(b), Some(a)) if b > 1e-9 && a <= 1e-9))
            .map(|e| e.id)
            .collect()
    }

    /// The driving dimensions: (id, the value measured now, the target).
    fn walk_dims(&self) -> Vec<(ConstraintId, f64, f64)> {
        self.constraints()
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
            .collect()
    }

    /// Whether this sketch solves with `opts` (and then solved): a constructive placement's trial, its sketch used
    /// only when it does. One solve: no constructive pass, no conflicting constraints set aside (a solve that needs
    /// that isn't Solved), no homotopy walk (over the live fuzz cases and real sketches it rescued 1 trial of 40,000,
    /// without changing any placement; the walks were most of a regeneration's solve: 5-49 ms a sketch, user docs).
    pub(crate) fn solve_trial(&mut self, opts: &SolveOptions) -> bool {
        let snapshot = self.clone();
        if !run_ext(self, opts, &[], false).is_solved() {
            return false;
        }
        if !opts.allow_collapse && !self.collapsed_lines(&snapshot).is_empty() {
            *self = snapshot;
            return false;
        }
        true
    }

    fn solve_inner(&mut self, opts: &SolveOptions) -> SolveReport {
        let dims = self.walk_dims();
        // Nothing to walk (every dimension already measures its value): a
        // failed solve would only be repeated, so solve with conflict
        // exclusion at once (a broken sketch paid a full solve and a DOF
        // analysis more on every edit).
        if dims.iter().all(|(_, a, b)| (b - a).abs() <= 1e-9 * (1.0 + b.abs())) {
            return run(self, opts, &[]);
        }
        let snapshot = self.clone();
        let rep = run_ext(self, opts, &[], false);
        if !matches!(rep.status, SolveStatus::Failed { .. }) {
            return rep;
        }
        // Homotopy: large dimension changes can leave Newton's basin of
        // convergence. Walk the driving dimensions from their currently
        // measured values to the targets in sub-steps.
        let quick = SolveOptions { analyze: false, ..opts.clone() };
        // Not when the walk's start doesn't solve either (a sketch broken
        // whatever the change): every walk would fail its first step.
        *self = snapshot.clone();
        for (id, a, _) in &dims {
            if let Some(c) = self.constraint_mut_silent(*id) {
                c.kind.set_value(*a);
            }
        }
        if matches!(run_ext(self, &quick, &[], false).status, SolveStatus::Failed { .. }) {
            *self = snapshot;
            return run(self, opts, &[]);
        }
        for steps in [4usize, 16, 64] {
            *self = snapshot.clone();
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

    /// Makes the DOF analysis and the drag's relation graph current, so a
    /// copy taken now (a drag's start) keeps them. True when each step
    /// should solve from that start (live: a drag's result depends only on
    /// where it ends); false for a dragged component too big for that each
    /// frame, whose steps go on from the last.
    pub fn begin_drag(&mut self, handles: &[(DragHandle, DVec2)]) -> bool {
        self.refresh_analysis();
        self.drag_graph().dragged(handles).is_some()
    }

    /// Interactive drag: pulls the handles toward their targets while keeping
    /// every constraint satisfied. Fully constrained geometry does not move.
    /// A target the constraints can't reach (or a jump that leaves the
    /// convergence basin) is approached in steps: the geometry goes as far
    /// toward it as solves and stops there, as Onshape's does; when not even
    /// the first step solves it is left unchanged. Each frame's work stays
    /// bounded (a ~300-entity sketch: ~2 ms a step, a few ms at the limit).
    /// Constraint states come from the last analysis if the sketch structure
    /// has not changed since.
    pub fn drag(&mut self, handles: &[(DragHandle, DVec2)]) -> SolveReport {
        let lives = self.lives_drag(handles);
        let handles = lives.as_deref().unwrap_or(handles);
        // Well-posed drag steps converge in a few iterations; a step that
        // doesn't converge soon sits at a limit of the constraints.
        let mut opts = SolveOptions { analyze: false, max_iterations: 8, ..SolveOptions::default() };
        let (light, still) = self.drag_weights(handles);
        opts.light = light;
        opts.still = still;
        if opts.light.is_empty() {
            opts.still_across = self.drag_levels(handles);
            if !opts.still_across.is_empty() {
                opts.light = self.entities().filter(|e| !e.external).map(|e| e.id).collect();
            }
        }
        let targets = self.drag_targets(handles);
        if !opts.light.is_empty() {
            self.drag_graph();
        }
        let start = self.clone();
        let report = if opts.light.is_empty() {
            run(self, &opts, &targets)
        } else {
            if !(solve_held(self, &opts, &targets) && self.targets_met(&targets)) && !opts.still.is_empty() {
                self.keep_far(&start, handles, &opts, &targets);
            }
            run_weighted(self, &opts, &targets, true)
        };
        // A drag never satisfies its constraints by collapsing a line (one
        // with Horizontal + Vertical, dragged itself): it doesn't move.
        let collapsed = self.collapsed_lines(&start);
        if !collapsed.is_empty() {
            *self = start;
            return self.collapse_report(report, &collapsed);
        }
        // Live (live2's fromOrigin-body, 3 takes): a curve's body drag translates it whole; with a point held (on the
        // origin, an external or a fixed point) nothing moves, it doesn't swing round the held end.
        if handles.iter().any(|(h, _)| matches!(*h, DragHandle::Entity { id, .. } if self.body_drag_held(id))) {
            *self = start;
            return SolveReport { drag_solved: false, ..report };
        }
        if report.drag_solved {
            return report;
        }
        // Live fc-1120 (2): where the dimension hold can't reach the pointer, dimensioned curves keep their sizes and
        // move by least change, each held coordinate kept while it still solves (a rectangle with a 30 mm bottom
        // slides sideways with its dragged corner, its bottom at its height).
        // Only for a small dragged component (`MAX_UNITS`): each kept coordinate is another solve.
        if !opts.still.is_empty() && self.drag_graph().dragged(handles).is_some() {
            let plain = SolveOptions { analyze: false, max_iterations: 8, ..SolveOptions::default() };
            let solve = |t: &[DragTarget]| {
                let mut s = start.clone();
                let r = run(&mut s, &plain, t);
                (r.drag_solved && s.targets_met(t) && s.collapsed_lines(&start).is_empty()).then_some((s, r))
            };
            if let Some(mut best) = solve(&targets) {
                let mut held = targets.clone();
                let mut kept: Vec<EntityId> = Vec::new();
                for &p in &opts.still {
                    if kept.contains(&p) || kept.len() >= MAX_KEPT {
                        continue;
                    }
                    kept.push(p);
                    let Some(q) = start.point(p) else { continue };
                    for (comp, value) in [(0, q.x), (1, q.y)] {
                        held.push(DragTarget::Coord { point: p, comp, value });
                        match solve(&held) {
                            Some(b) => best = b,
                            None => {
                                held.pop();
                            }
                        }
                    }
                }
                *self = best.0;
                return best.1;
            }
        }
        const SUBSTEPS: usize = 4;
        let mut best: Option<SolveReport> = None;
        for k in 1..=SUBSTEPS {
            let f = k as f64 / SUBSTEPS as f64;
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
                        DragHandle::Entity { id, grab } => DragHandle::Entity { id, grab: grab + (*t - grab) * ((k - 1) as f64 / SUBSTEPS as f64) },
                        p => p,
                    };
                    (h2, target)
                })
                .collect();
            let targets = self.drag_targets(&partial);
            let prev = self.clone();
            let r = run(self, &opts, &targets);
            if !r.drag_solved || !self.collapsed_lines(&start).is_empty() {
                *self = prev;
                break;
            }
            best = Some(r);
        }
        match best {
            Some(r) => r,
            None => {
                *self = start;
                report
            }
        }
    }

    /// A point drag as live places the curve it belongs to (live2 sketch-drags 11-17), as more handles: an arc's end
    /// keeps its radius and other end, the centre moving, until the chord passes the diameter (then a semicircle on
    /// the chord); an arc's centre keeps the end, the start sliding onto the new circle; a LENGTH line's end
    /// translates the line. Only where nothing else holds the curve; `None`: the solver's own drag.
    fn lives_drag(&self, handles: &[(DragHandle, DVec2)]) -> Option<Vec<(DragHandle, DVec2)>> {
        if let [(DragHandle::Entity { id, grab }, t)] = handles {
            return self.lives_line_body_drag(*id, *grab, *t);
        }
        let [(DragHandle::Point(p), t)] = handles else { return None };
        let (p, t) = (*p, *t);
        // Nothing but `ok` constraints on `ids`, and their points used by `curve` alone.
        let loose = |curve: EntityId, ids: &[EntityId], ok: &dyn Fn(&ConstraintKind) -> bool| {
            !self.constraints().any(|c| !ok(&c.kind) && c.kind.entities().iter().any(|e| ids.contains(e)))
                && !self.entities().any(|e| e.id != curve && e.point_refs().iter().any(|q| ids.contains(q)))
        };
        let none = |_: &ConstraintKind| false;
        for e in self.entities().filter(|e| !e.external) {
            match e.geometry {
                Geometry::Arc { center, start, end } if p == start || p == end => {
                    let other = if p == start { end } else { start };
                    if !loose(e.id, &[e.id, center, other], &none) {
                        continue;
                    }
                    let (c, q, r) = (self.point(center)?, self.point(other)?, (self.point(p)? - self.point(center)?).length());
                    let (mid, half) = ((t + q) * 0.5, (t - q) * 0.5);
                    // The centre on the side of the chord it was on.
                    let side = (self.point(p)? - q).perp_dot(c - q).signum();
                    let n = (t - q).perp().normalize_or_zero() * side;
                    let h = (r * r - half.length_squared()).max(0.0).sqrt();
                    return Some(vec![(DragHandle::Point(p), t), (DragHandle::Point(other), q), (DragHandle::Point(center), mid + n * h)]);
                }
                Geometry::Arc { center, start, end } if p == center => {
                    if !loose(e.id, &[e.id, start, end], &none) {
                        continue;
                    }
                    let (s0, q) = (self.point(start)?, self.point(end)?);
                    let r = (q - t).length();
                    return Some(vec![(DragHandle::Point(center), t), (DragHandle::Point(end), q), (DragHandle::Point(start), t + (s0 - t).normalize_or(DVec2::X) * r)]);
                }
                Geometry::Line { start, end } if p == start || p == end => {
                    let other = if p == start { end } else { start };
                    let own = |k: &ConstraintKind| match *k {
                        ConstraintKind::Length { curve, .. } => curve == e.id,
                        ConstraintKind::Horizontal { a, b: None } | ConstraintKind::Vertical { a, b: None } => a == e.id,
                        _ => false,
                    };
                    let sized = self.constraints().any(|c| c.driving && matches!(c.kind, ConstraintKind::Length { curve, .. } if curve == e.id));
                    if !sized || !loose(e.id, &[e.id, other], &own) {
                        continue;
                    }
                    let d = t - self.point(p)?;
                    return Some(vec![(DragHandle::Point(p), t), (DragHandle::Point(other), self.point(other)? + d)]);
                }
                _ => {}
            }
        }
        None
    }

    /// A line dragged by its body with an end shared with another line (live2 sketch-drags run4 c2): it keeps its
    /// direction and moves across itself through the pointer; a free end moves straight across, a shared end slides
    /// along the other line, which keeps its direction and far end. Only where nothing else holds them; `None` (a line
    /// free at both ends translates whole) otherwise.
    fn lives_line_body_drag(&self, id: EntityId, grab: DVec2, t: DVec2) -> Option<Vec<(DragHandle, DVec2)>> {
        let Geometry::Line { start, end } = self.entity(id)?.geometry else { return None };
        let (a, b) = (self.point(start)?, self.point(end)?);
        let n = (b - a).try_normalize()?.perp();
        let shift = n * (t - grab).dot(n);
        let constrained = |ids: &[EntityId]| self.constraints().any(|c| c.kind.entities().iter().any(|e| ids.contains(e)));
        let mut out = Vec::new();
        let mut shared = false;
        for (p, q) in [(start, a), (end, b)] {
            let others: Vec<&crate::entities::Entity> = self.entities().filter(|e| e.id != id && e.point_refs().contains(&p)).collect();
            match others.as_slice() {
                [] => out.push((DragHandle::Point(p), q + shift)),
                [m] => {
                    let Geometry::Line { start: ms, end: me } = m.geometry else { return None };
                    let far = if ms == p { me } else { ms };
                    if constrained(&[m.id, far]) || self.entities().any(|e| e.id != m.id && e.point_refs().contains(&far)) {
                        return None;
                    }
                    let f = self.point(far)?;
                    let u = (q - f).try_normalize()?;
                    // Where the other line (kept) meets the moved one.
                    let den = u.dot(n);
                    if den.abs() < 1e-9 {
                        return None;
                    }
                    let k = (q + shift - f).dot(n) / den;
                    out.push((DragHandle::Point(p), f + u * k));
                    out.push((DragHandle::Point(far), f));
                    shared = true;
                }
                _ => return None,
            }
        }
        (shared && !constrained(&[id, start, end])).then_some(out)
    }

    /// Whether a body drag of curve `id` (not a circle or an arc: those resize) can't translate it: one of its points is
    /// fixed, external, or coincident with a fixed or external point.
    fn body_drag_held(&self, id: EntityId) -> bool {
        let Some(e) = self.entity(id) else { return false };
        if matches!(e.geometry, Geometry::Point { .. } | Geometry::Circle { .. } | Geometry::Arc { .. } | Geometry::Ellipse { .. }) {
            return false;
        }
        let fixed = |p: EntityId| self.entity(p).is_some_and(|x| x.external) || self.constraints().any(|c| matches!(c.kind, ConstraintKind::Fix { entity } if entity == p || entity == id));
        e.point_refs().iter().any(|&p| {
            fixed(p)
                || self.constraints().any(|c| match c.kind {
                    ConstraintKind::Coincident { a, b } if a == p => fixed(b),
                    ConstraintKind::Coincident { a, b } if b == p => fixed(a),
                    _ => false,
                })
        })
    }

    /// A drag's `light` / `still`: dimensioned curves (a Length, Radius or
    /// Diameter of their own) hold their points, the rest takes the move
    /// (live fuzz s3: a dimensioned line stays put while the arc and the
    /// free line beside it change size). Empty when nothing is dimensioned.
    fn drag_weights(&self, handles: &[(DragHandle, DVec2)]) -> (Vec<EntityId>, Vec<EntityId>) {
        let dragged: Vec<EntityId> = handles.iter().map(|(h, _)| match *h {
            DragHandle::Point(id) => id,
            DragHandle::Entity { id, .. } => id,
        }).collect();
        let sized: HashSet<EntityId> = self.constraints().filter(|c| c.driving).filter_map(|c| match c.kind {
            ConstraintKind::Length { curve, .. } | ConstraintKind::Radius { curve, .. } | ConstraintKind::Diameter { curve, .. } => Some(curve),
            _ => None,
        }).collect();
        if sized.is_empty() {
            return (Vec::new(), Vec::new());
        }
        let dimensioned = |e: EntityId| sized.contains(&e);
        let (mut light, mut still) = (Vec::new(), Vec::new());
        for e in self.entities().filter(|e| e.kind() != EntityKind::Point && !e.external) {
            let pts = e.point_refs();
            if dimensioned(e.id) && !dragged.contains(&e.id) && !pts.iter().any(|p| dragged.contains(p)) {
                still.extend(pts.iter().copied());
            } else {
                light.push(e.id);
            }
        }
        if still.is_empty() {
            return (Vec::new(), Vec::new());
        }
        let held: HashSet<EntityId> = still.iter().copied().collect();
        light.extend(self.entities().filter(|e| e.kind() == EntityKind::Point).map(|e| e.id).filter(|p| !held.contains(p)));
        (light, still)
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
                        // Live2 sketch-drags (f): the rim scales the ellipse about its centre, its ratio kept, through the pointer.
                        Geometry::Ellipse { center, major, minor } => {
                            if let (Some(c), Some(u)) = (self.point(*center), major.try_normalize()) {
                                let v = *target - c;
                                let k = ((v.dot(u) / major.length()).powi(2) + (v.dot(u.perp()) / minor).powi(2)).sqrt();
                                push_pt(&mut out, *center, c);
                                for (slot, value) in [major.x * k, major.y * k, minor * k].into_iter().enumerate() {
                                    out.push(DragTarget::Own { entity: id, slot: slot as u8, value });
                                }
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

#[cfg(test)]
mod analysis_tests {
    use crate::{ConstraintKind, Geometry, SolveOptions, Sketch};
    use glam::DVec2;

    /// User 11-12 (hat15b): a regeneration solves without the DOF analysis while the sketch's is current, and its
    /// least-change placement moved geometry the regeneration had updated (a constraint no longer held): the report had
    /// no DOF, so the open sketch's row lost its under-defined badge. The analysis is the constraints', kept.
    #[test]
    fn a_placement_without_analysis_keeps_reporting_the_current_one() {
        let mut sk = Sketch::new();
        let line = sk.add_line(DVec2::ZERO, DVec2::new(0.02, 0.0));
        sk.add_constraint(ConstraintKind::Length { curve: line, value: 0.02 }).unwrap();
        let dof = sk.solve().dof;
        assert!(dof.is_some_and(|d| d > 0), "a free line with a length: {dof:?}");
        // Its end moved as a regeneration moves used geometry: without a new revision.
        let (_, end) = sk.entity(line).and_then(|e| e.endpoints()).unwrap();
        if let Some(Geometry::Point { pos }) = sk.entity_mut_silent(end).map(|e| &mut e.geometry) {
            *pos = DVec2::new(0.03, 0.0);
        }
        assert!(sk.analysis_current());
        let rep = sk.solve_with(&SolveOptions { analyze: false, constructive: true, ..Default::default() });
        assert!(rep.is_solved());
        assert_eq!(rep.dof, dof, "the analysis after the placement");
        assert!(sk.analysis_current());
    }
}

#[cfg(test)]
mod state_tests {
    use crate::{ConstraintKind, ConstraintState, Sketch};
    use glam::DVec2;

    /// Live (behaviours "sketch snapping and hover", 11-12, geometry-full-vs-under): colour goes entity by entity. A
    /// horizontal line from a fixed point with its length free: the line is black (fully defined: its direction and
    /// place are held), its free end point blue; with the length dimensioned the end turns black.
    #[test]
    fn a_line_with_a_free_length_is_defined_and_its_free_end_is_not() {
        let mut sk = Sketch::new();
        let line = sk.add_line(DVec2::ZERO, DVec2::new(0.04, 0.0));
        let (start, end) = sk.entity(line).and_then(|e| e.endpoints()).unwrap();
        sk.add_constraint(ConstraintKind::Fix { entity: start }).unwrap();
        sk.add_constraint(ConstraintKind::Horizontal { a: line, b: None }).unwrap();
        let r = sk.solve();
        assert_eq!((r.state(line), r.state(start), r.state(end)), (Some(ConstraintState::FullyConstrained), Some(ConstraintState::FullyConstrained), Some(ConstraintState::UnderConstrained)), "length free");
        sk.add_constraint(ConstraintKind::Length { curve: line, value: 0.04 }).unwrap();
        let r = sk.solve();
        assert_eq!(r.state(end), Some(ConstraintState::FullyConstrained), "length dimensioned");
    }

    /// Live2 (11-13, colours (a)): an arc with its centre held and its radius dimensioned is black, its ends blue
    /// (free to slide along its circle); with the radius free the arc is blue too.
    #[test]
    fn an_arc_held_on_its_circle_is_defined_and_its_sliding_ends_are_not() {
        let mut sk = Sketch::new();
        let arc = sk.add_arc(DVec2::ZERO, DVec2::new(0.02, 0.0), DVec2::new(0.0, 0.02));
        let crate::Geometry::Arc { center, start, end } = sk.entity(arc).unwrap().geometry else { panic!("an arc") };
        sk.add_constraint(ConstraintKind::Fix { entity: center }).unwrap();
        let r = sk.solve();
        assert_eq!((r.state(arc), r.state(center)), (Some(ConstraintState::UnderConstrained), Some(ConstraintState::FullyConstrained)), "radius free");
        sk.add_constraint(ConstraintKind::Radius { curve: arc, value: 0.02 }).unwrap();
        let r = sk.solve();
        assert_eq!((r.state(arc), r.state(start), r.state(end)), (Some(ConstraintState::FullyConstrained), Some(ConstraintState::UnderConstrained), Some(ConstraintState::UnderConstrained)), "radius dimensioned");
    }

    /// Live2 (11-13, colours; interaction-feel "…a circle, a spline"): a circle with its centre held is blue while its
    /// radius is free, its centre black, and black once the radius is dimensioned; a spline is blue while any of its
    /// points is free.
    #[test]
    fn a_circle_is_defined_by_its_radius_and_a_spline_by_all_its_points() {
        let mut sk = Sketch::new();
        let circle = sk.add_circle(DVec2::ZERO, 0.02);
        let crate::Geometry::Circle { center, .. } = sk.entity(circle).unwrap().geometry else { panic!("a circle") };
        sk.add_constraint(ConstraintKind::Fix { entity: center }).unwrap();
        let r = sk.solve();
        assert_eq!((r.state(circle), r.state(center)), (Some(ConstraintState::UnderConstrained), Some(ConstraintState::FullyConstrained)), "radius free");
        sk.add_constraint(ConstraintKind::Radius { curve: circle, value: 0.02 }).unwrap();
        assert_eq!(sk.solve().state(circle), Some(ConstraintState::FullyConstrained), "radius dimensioned");
        let spline = sk.add_fit_spline(&[DVec2::new(0.05, 0.0), DVec2::new(0.06, 0.01), DVec2::new(0.07, 0.0)]).unwrap();
        let crate::Geometry::FitSpline { points, .. } = sk.entity(spline).unwrap().geometry.clone() else { panic!("a fit spline") };
        for p in &points[..2] {
            sk.add_constraint(ConstraintKind::Fix { entity: *p }).unwrap();
        }
        assert_eq!(sk.solve().state(spline), Some(ConstraintState::UnderConstrained), "its last point free");
        sk.add_constraint(ConstraintKind::Fix { entity: points[2] }).unwrap();
        assert_eq!(sk.solve().state(spline), Some(ConstraintState::FullyConstrained), "every point held");
    }

    /// An end free on a guide that isn't along the line (a fixed vertical line): moving it turns the line, so the
    /// line is under defined; an end free only along a guide that lies on the line keeps it defined.
    #[test]
    fn a_line_whose_end_moves_off_it_is_under_defined() {
        let mut sk = Sketch::new();
        let line = sk.add_line(DVec2::ZERO, DVec2::new(0.04, 0.0));
        let (start, end) = sk.entity(line).and_then(|e| e.endpoints()).unwrap();
        let guide = sk.add_line(DVec2::new(0.04, -0.01), DVec2::new(0.04, 0.01));
        let (g0, g1) = sk.entity(guide).and_then(|e| e.endpoints()).unwrap();
        for c in [ConstraintKind::Fix { entity: start }, ConstraintKind::Fix { entity: g0 }, ConstraintKind::Fix { entity: g1 }, ConstraintKind::Coincident { a: end, b: guide }] {
            sk.add_constraint(c).unwrap();
        }
        let r = sk.solve();
        assert_eq!((r.state(line), r.state(end), r.state(guide)), (Some(ConstraintState::UnderConstrained), Some(ConstraintState::UnderConstrained), Some(ConstraintState::FullyConstrained)));
    }
}

#[cfg(test)]
mod carried_tests {
    use crate::constraints::{ConstraintKind as K, DistanceMode};
    use crate::entities::Geometry;
    use crate::sketch::Sketch;
    use glam::DVec2;

    /// Live (c2-loft-guide-curves-wave-washer's Sketch 4): a 0.45 x 0.06 in rectangle its constraints hold, one corner
    /// made coincident with a model vertex 63.5 mm away, slides over whole (x 13.97...25.4 mm); the plain solve
    /// diverged (the Minimum distance between its long sides broke once they sheared), with the inferred Horizontal on
    /// its bottom too.
    #[test]
    fn a_held_rectangle_slides_onto_a_far_anchor() {
        for bottom_h in [false, true] {
            let mut sk = Sketch::new();
            let m = |x: f64, y: f64| DVec2::new(x * 1e-3, y * 1e-3);
            let [a, b, c, d] = [m(88.9, 3.81), m(77.47, 3.81), m(88.9, 2.286), m(77.47, 2.286)].map(|p| sk.add_point(p));
            let bottom = sk.add_line_between(a, b).unwrap();
            let top = sk.add_line_between(c, d).unwrap();
            let left = sk.add_line_between(a, c).unwrap();
            let right = sk.add_line_between(b, d).unwrap();
            sk.add_constraint(K::Horizontal { a: top, b: None }).unwrap();
            if bottom_h {
                sk.add_constraint(K::Horizontal { a: bottom, b: None }).unwrap();
            }
            sk.add_constraint(K::Perpendicular { a: top, b: left }).unwrap();
            sk.add_constraint(K::Parallel { a: bottom, b: top }).unwrap();
            sk.add_constraint(K::Parallel { a: left, b: right }).unwrap();
            sk.add_constraint(K::Distance { a: top, b: bottom, value: 1.524e-3, mode: DistanceMode::Minimum }).unwrap();
            sk.add_constraint(K::Length { curve: top, value: 11.43e-3 }).unwrap();
            assert!(sk.solve().is_solved());
            let vertex = sk.add_entity(Geometry::Point { pos: m(25.4, 3.81) }, false, true).unwrap();
            sk.add_constraint(K::Coincident { a, b: vertex }).unwrap();
            let r = sk.solve();
            let got = [a, b, c, d].map(|p| sk.point(p).unwrap());
            let want = [m(25.4, 3.81), m(13.97, 3.81), m(25.4, 2.286), m(13.97, 2.286)];
            assert!(r.is_solved() && r.dof == Some(0), "bottom H {bottom_h}: {:?}", r.status);
            assert!(got.iter().zip(want).all(|(g, w)| (*g - w).length() < 1e-9), "bottom H {bottom_h}: {got:?}");
        }
    }
}
