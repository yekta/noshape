//! Translation of constraints into scalar residual equations, and the
//! residual kernels themselves. Kernels are written generically over
//! [`Num`] so exact gradients come from forward-mode dual numbers.

use crate::constraints::{dist_rank, fix_items, tangent_rank, Constraint, ConstraintKind, DistanceMode, FixItem};
use crate::dual::{Dual, Num, V2};
use crate::entities::{EntityKind, Geometry};
use crate::ids::EntityId;
use crate::nurbs::Nurbs;
use crate::sketch::Sketch;
use smallvec::SmallVec;
#[cfg(test)]
use smallvec::smallvec;
use std::cell::Cell;
use std::f64::consts::TAU;
use std::sync::Arc;

/// A term of a linear combination residual.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Term {
    /// A single parameter (1 input).
    Param,
    /// Distance between two points (4 inputs).
    Dist,
    /// Signed distance from point to line (6 inputs: p, a, b).
    PLDist,
    /// Signed distance from midpoint of (p, q) to line (8 inputs).
    PLDistMid,
    /// Length of a vector (2 inputs).
    Norm,
}

impl Term {
    fn arity(self) -> usize {
        match self {
            Term::Param => 1,
            Term::Dist => 4,
            Term::PLDist => 6,
            Term::PLDistMid => 8,
            Term::Norm => 2,
        }
    }
}

/// Curvature of a curve end, oriented away from the joint.
#[derive(Clone, Copy, Debug)]
pub(crate) enum CurvSpec {
    /// Inputs P0, P1, P2 (6). C' = alpha (P1-P0), C'' = b1 (P2-P1) - b0 (P1-P0).
    Spline { alpha: f64, b1: f64, b0: f64 },
    /// Inputs center, endpoint (4); curvature = sign / r.
    Arc { sign: f64 },
    Line,
}

/// One side of a curvature joint: a control-point curve or a fit spline's
/// end (inputs: its points, then its two handles if it has them).
#[derive(Clone, Copy, Debug)]
pub(crate) enum CurvSide {
    Spec(CurvSpec),
    Fit { n: usize, at_end: bool, handles: bool },
}

impl CurvSide {
    fn arity(&self) -> usize {
        match self {
            CurvSide::Spec(s) => s.arity(),
            CurvSide::Fit { n, handles, .. } => 2 * n + if *handles { 4 } else { 0 },
        }
    }

    /// Curvature at the joint, oriented away from it (as [`curv`]).
    fn curvature(&self, x: &[f64]) -> f64 {
        match self {
            CurvSide::Spec(s) => curv(s, x),
            CurvSide::Fit { n, at_end, handles } => {
                let c = if *handles { fit_curve_handles(x, *n) } else { fit_curve(x, *n) };
                let Some(c) = c else { return 0.0 };
                let (a, b) = c.domain();
                let d = c.eval_ders(if *at_end { b } else { a });
                let d1 = if *at_end { -d[1] } else { d[1] };
                let l = d1.length();
                if l < 1e-300 {
                    return 0.0;
                }
                d1.perp_dot(d[2]) / (l * l * l)
            }
        }
    }
}

impl CurvSpec {
    fn arity(&self) -> usize {
        match self {
            CurvSpec::Spline { .. } => 6,
            CurvSpec::Arc { .. } => 4,
            CurvSpec::Line => 0,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) enum EqKind {
    /// `sum coef_i * term_i + c`.
    Linear { terms: SmallVec<[(f64, Term); 4]>, c: f64 },
    /// sin of angle between (p1-p0) and (p3-p2).
    Parallel,
    /// cos of angle between (p1-p0) and (p3-p2).
    Perpendicular,
    /// Wrapped difference between the CCW angle from (p1-p0) to (p3-p2) and the target.
    Angle { cos: f64, sin: f64 },
    /// Inputs c, s, e: r * ccw sweep - len.
    ArcLength { len: f64 },
    /// Inputs a, b, l0, l1: midpoint of a,b on the line.
    SymMid,
    /// Inputs a, b, l0, l1: (b-a) perpendicular to the line.
    SymPerp,
    /// Inputs p, c, mx, my, minor.
    PointOnEllipse,
    /// Inputs p, c, mx, my: p on the major axis line (or the minor axis
    /// line with `minor`) of an ellipse centered at c with major vector m.
    OnEllipseAxis { minor: bool },
    /// Inputs c, mx, my, minor, t, l0, l1: tangent at E(t) parallel to the line.
    EllipseTangent,
    /// Inputs c, mx, my, minor, t, l0, l1: E(t) on the line.
    EllipseOnLine,
    /// Sum of oriented curvatures at a joint (G2).
    Curvature { a: CurvSpec, b: CurvSpec },
    /// Inputs p, t, control points: component of S(t) - p (analytic custom gradient).
    SplinePoint { comp: u8, curve: Arc<Nurbs> },
    /// Inputs p, t, fit points: component of S(t) - p (numeric gradient).
    FitPoint { comp: u8, handles: bool },
    /// Inputs fit points (2n), o0, o1: cross (or dot if `perp`) of the fit
    /// spline's outgoing end tangent with (o1 - o0), normalized.
    FitEndDir { at_end: bool, perp: bool, n: usize },
    /// Two fit splines' end directions parallel (perpendicular): inputs the
    /// first's `na` points, then the second's.
    FitFitDir { na: usize, end_a: bool, end_b: bool, perp: bool },
    /// Sum of oriented end curvatures (G2) with a fit spline on either side:
    /// evaluated numerically.
    CurvatureNum { a: CurvSide, b: CurvSide },
}

#[derive(Clone, Debug)]
pub(crate) struct Eq {
    pub kind: EqKind,
    pub inputs: SmallVec<[u32; 8]>,
}

#[inline]
fn pld<T: Num>(p: V2<T>, a: V2<T>, b: V2<T>) -> T {
    let d = b.sub(a);
    d.cross(p.sub(a)) / d.norm()
}

fn ellipse_eval<T: Num>(x: &[T], t: T) -> (V2<T>, V2<T>) {
    let c = V2::at(x, 0);
    let m = V2::at(x, 2);
    let b = x[4];
    let a = m.norm();
    let n = m.perp().mul(b / a);
    let (s, co) = (t.sin(), t.cos());
    (c.add(m.mul(co)).add(n.mul(s)), n.mul(co).sub(m.mul(s)))
}

fn curv<T: Num>(spec: &CurvSpec, x: &[T]) -> T {
    match spec {
        CurvSpec::Line => T::cst(0.0),
        CurvSpec::Arc { sign } => {
            let r = V2::at(x, 2).sub(V2::at(x, 0)).norm();
            T::cst(*sign) / r
        }
        CurvSpec::Spline { alpha, b1, b0 } => {
            let p0 = V2::at(x, 0);
            let p1 = V2::at(x, 2);
            let p2 = V2::at(x, 4);
            let d1 = p1.sub(p0).scale(*alpha);
            let d2 = p2.sub(p1).scale(*b1).sub(p1.sub(p0).scale(*b0));
            let l = d1.norm();
            d1.cross(d2) / (l * l * l)
        }
    }
}

fn kernel<T: Num>(kind: &EqKind, x: &[T]) -> T {
    match kind {
        EqKind::Linear { terms, c } => {
            let mut acc = T::cst(*c);
            let mut i = 0;
            for (coef, t) in terms {
                let v = match t {
                    Term::Param => x[i],
                    Term::Dist => V2::at(x, i + 2).sub(V2::at(x, i)).norm(),
                    Term::PLDist => pld(V2::at(x, i), V2::at(x, i + 2), V2::at(x, i + 4)),
                    Term::PLDistMid => pld(V2::at(x, i).add(V2::at(x, i + 2)).scale(0.5), V2::at(x, i + 4), V2::at(x, i + 6)),
                    Term::Norm => V2::at(x, i).norm(),
                };
                i += t.arity();
                acc = acc + v.scale(*coef);
            }
            acc
        }
        EqKind::Parallel | EqKind::Perpendicular | EqKind::Angle { .. } => {
            let u = V2::at(x, 2).sub(V2::at(x, 0));
            let v = V2::at(x, 6).sub(V2::at(x, 4));
            match kind {
                EqKind::Parallel => u.cross(v) / (u.norm() * v.norm()),
                EqKind::Perpendicular => u.dot(v) / (u.norm() * v.norm()),
                EqKind::Angle { cos, sin } => {
                    let cr = u.cross(v);
                    let d = u.dot(v);
                    (cr.scale(*cos) - d.scale(*sin)).atan2(d.scale(*cos) + cr.scale(*sin))
                }
                _ => unreachable!(),
            }
        }
        EqKind::ArcLength { len } => {
            let c = V2::at(x, 0);
            let u = V2::at(x, 2).sub(c);
            let v = V2::at(x, 4).sub(c);
            let mut phi = u.cross(v).atan2(u.dot(v));
            if phi.val() < 0.0 {
                phi = phi + T::cst(TAU);
            }
            u.norm() * phi - T::cst(*len)
        }
        EqKind::SymMid => {
            let m = V2::at(x, 0).add(V2::at(x, 2)).scale(0.5);
            pld(m, V2::at(x, 4), V2::at(x, 6))
        }
        EqKind::SymPerp => {
            let d = V2::at(x, 2).sub(V2::at(x, 0));
            let l = V2::at(x, 6).sub(V2::at(x, 4));
            d.dot(l) / l.norm()
        }
        EqKind::OnEllipseAxis { minor } => {
            let d = V2::at(x, 0).sub(V2::at(x, 2));
            let m = V2::at(x, 4);
            if *minor { d.dot(m) / m.norm() } else { m.cross(d) / m.norm() }
        }
        EqKind::PointOnEllipse => {
            let p = V2::at(x, 0);
            let c = V2::at(x, 2);
            let m = V2::at(x, 4);
            let b = x[6];
            let a = m.norm();
            let u = m.mul(T::cst(1.0) / a);
            let d = p.sub(c);
            let xx = d.dot(u) / a;
            let yy = u.cross(d) / b;
            ((xx * xx + yy * yy).sqrt() - T::cst(1.0)) * b
        }
        EqKind::EllipseTangent | EqKind::EllipseOnLine => {
            let (e, de) = ellipse_eval(x, x[5]);
            let l0 = V2::at(x, 6);
            let l1 = V2::at(x, 8);
            if matches!(kind, EqKind::EllipseOnLine) {
                pld(e, l0, l1)
            } else {
                let d = l1.sub(l0);
                d.cross(de) / (d.norm() * de.norm())
            }
        }
        EqKind::Curvature { a, b } => {
            let na = a.arity();
            curv(a, &x[..na]) + curv(b, &x[na..])
        }
        EqKind::SplinePoint { .. } | EqKind::FitPoint { .. } | EqKind::FitEndDir { .. } | EqKind::FitFitDir { .. } | EqKind::CurvatureNum { .. } => {
            // only evaluated through `eval_f64`
            T::cst(f64::NAN)
        }
    }
}

#[allow(clippy::needless_range_loop)]
fn spline_point_eval(curve: &Nurbs, comp: usize, x: &[f64], grad: Option<&mut [f64]>) -> f64 {
    let p = [x[0], x[1]];
    let t = x[2];
    let (a, b) = curve.domain();
    let tt = t.clamp(a - (b - a), b + (b - a));
    let mut c = curve.clone();
    for (i, q) in c.ctrl.iter_mut().enumerate() {
        q.x = x[3 + 2 * i];
        q.y = x[4 + 2 * i];
    }
    let (span, r0, r1) = c.rational_basis(tt);
    let pdeg = c.degree;
    let mut s = 0.0;
    let mut ds = 0.0;
    for j in 0..=pdeg {
        let i = span - pdeg + j;
        let v = if comp == 0 { c.ctrl[i].x } else { c.ctrl[i].y };
        s += r0[j] * v;
        ds += r1[j] * v;
    }
    if let Some(g) = grad {
        g.iter_mut().for_each(|v| *v = 0.0);
        g[comp] = -1.0;
        g[2] = ds;
        for j in 0..=pdeg {
            let i = span - pdeg + j;
            g[3 + 2 * i + comp] = r0[j];
        }
    }
    s - p[comp]
}

/// Fit spline through `n` points followed by its two handle points.
fn fit_curve_handles(x: &[f64], n: usize) -> Option<Nurbs> {
    let pts: Vec<glam::DVec2> = (0..n).map(|i| glam::DVec2::new(x[2 * i], x[2 * i + 1])).collect();
    let h0 = glam::DVec2::new(x[2 * n], x[2 * n + 1]);
    let h1 = glam::DVec2::new(x[2 * n + 2], x[2 * n + 3]);
    Nurbs::interpolate_with_handles(&pts, h0, h1)
}

fn fit_curve(x: &[f64], n: usize) -> Option<Nurbs> {
    let pts: Vec<glam::DVec2> = (0..n).map(|i| glam::DVec2::new(x[2 * i], x[2 * i + 1])).collect();
    Nurbs::interpolate(&pts)
}

impl Eq {
    /// Residual value. `x` holds the input values in input order.
    pub fn eval(&self, x: &[f64]) -> f64 {
        match &self.kind {
            EqKind::SplinePoint { comp, curve } => spline_point_eval(curve, *comp as usize, x, None),
            EqKind::FitPoint { comp, handles } => {
                let n = (x.len() - 3) / 2 - if *handles { 2 } else { 0 };
                let c = if *handles { fit_curve_handles(&x[3..], n) } else { fit_curve(&x[3..], n) };
                let Some(c) = c else { return 0.0 };
                let (a, b) = c.domain();
                let p = c.point(x[2].clamp(a, b));
                if *comp == 0 { p.x - x[0] } else { p.y - x[1] }
            }
            EqKind::FitFitDir { na, end_a, end_b, perp } => {
                let nb = x.len() / 2 - na;
                let dir = |xs: &[f64], n: usize, at_end: bool| -> Option<glam::DVec2> {
                    let c = fit_curve(xs, n)?;
                    let (a, b) = c.domain();
                    Some(c.eval_ders(if at_end { b } else { a })[1])
                };
                let (Some(t), Some(o)) = (dir(&x[..2 * na], *na, *end_a), dir(&x[2 * na..], nb, *end_b)) else { return 0.0 };
                let den = t.length() * o.length();
                if den < 1e-300 {
                    return 0.0;
                }
                if *perp { t.dot(o) / den } else { t.perp_dot(o) / den }
            }
            EqKind::CurvatureNum { a, b } => {
                let na = a.arity();
                a.curvature(&x[..na]) + b.curvature(&x[na..])
            }
            EqKind::FitEndDir { at_end, perp, n } => {
                let Some(c) = fit_curve(x, *n) else { return 0.0 };
                let (a, b) = c.domain();
                let t = if *at_end { -c.eval_ders(b)[1] } else { c.eval_ders(a)[1] };
                let o = glam::DVec2::new(x[2 * n + 2] - x[2 * n], x[2 * n + 3] - x[2 * n + 1]);
                let den = t.length() * o.length();
                if den < 1e-300 {
                    return 0.0;
                }
                if *perp { t.dot(o) / den } else { t.perp_dot(o) / den }
            }
            k => kernel::<f64>(k, x),
        }
    }

    /// Residual and gradient w.r.t. the inputs (`g.len() == x.len()`).
    pub fn eval_grad(&self, x: &[f64], g: &mut [f64]) -> f64 {
        match &self.kind {
            EqKind::SplinePoint { comp, curve } => return spline_point_eval(curve, *comp as usize, x, Some(g)),
            EqKind::FitPoint { .. } | EqKind::FitEndDir { .. } | EqKind::FitFitDir { .. } | EqKind::CurvatureNum { .. } => return self.numeric_grad(x, g),
            _ => {}
        }
        let n = x.len();
        macro_rules! run {
            ($n:literal) => {{
                let mut d = [Dual::<$n>::cst(0.0); $n];
                for i in 0..n {
                    d[i] = Dual::<$n>::var(x[i], i);
                }
                let r = kernel(&self.kind, &d[..n]);
                g[..n].copy_from_slice(&r.g[..n]);
                r.v
            }};
        }
        if n <= 4 {
            run!(4)
        } else if n <= 8 {
            run!(8)
        } else if n <= 16 {
            run!(16)
        } else {
            self.numeric_grad(x, g)
        }
    }

    fn numeric_grad(&self, x: &[f64], g: &mut [f64]) -> f64 {
        let f0 = self.eval(x);
        let mut xx: SmallVec<[f64; 32]> = SmallVec::from_slice(x);
        for i in 0..x.len() {
            let h = 1e-7 * x[i].abs().max(1e-3);
            xx[i] = x[i] + h;
            let fp = self.eval(&xx);
            xx[i] = x[i] - h;
            let fm = self.eval(&xx);
            xx[i] = x[i];
            g[i] = (fp - fm) / (2.0 * h);
        }
        f0
    }
}

// ----------------------------------------------------------------------
// builder

/// Maps entities to solver parameter indices.
pub(crate) trait Lookup {
    fn point(&self, id: EntityId) -> [u32; 2];
    fn own(&self, id: EntityId, slot: usize) -> u32;
    fn aux(&self, k: usize) -> u32;
    /// Are two point entities the same point (shared or aliased)?
    fn same(&self, a: EntityId, b: EntityId) -> bool;
    /// Is point `p` constrained to lie on `curve` by a coincident constraint?
    fn on_curve(&self, p: EntityId, curve: EntityId) -> bool;
}

type R = Result<(), String>;

struct Lin {
    terms: SmallVec<[(f64, Term); 4]>,
    inputs: SmallVec<[u32; 8]>,
    c: f64,
}

impl Lin {
    fn new() -> Self {
        Lin { terms: SmallVec::new(), inputs: SmallVec::new(), c: 0.0 }
    }
    fn p(mut self, k: f64, i: u32) -> Self {
        self.terms.push((k, Term::Param));
        self.inputs.push(i);
        self
    }
    fn dist(mut self, k: f64, a: [u32; 2], b: [u32; 2]) -> Self {
        self.terms.push((k, Term::Dist));
        self.inputs.extend_from_slice(&a);
        self.inputs.extend_from_slice(&b);
        self
    }
    fn pld(mut self, k: f64, p: [u32; 2], l: [u32; 4]) -> Self {
        self.terms.push((k, Term::PLDist));
        self.inputs.extend_from_slice(&p);
        self.inputs.extend_from_slice(&l);
        self
    }
    fn pldm(mut self, k: f64, p: [u32; 2], q: [u32; 2], l: [u32; 4]) -> Self {
        self.terms.push((k, Term::PLDistMid));
        self.inputs.extend_from_slice(&p);
        self.inputs.extend_from_slice(&q);
        self.inputs.extend_from_slice(&l);
        self
    }
    fn norm(mut self, k: f64, v: [u32; 2]) -> Self {
        self.terms.push((k, Term::Norm));
        self.inputs.extend_from_slice(&v);
        self
    }
    fn rad(mut self, k: f64, r: &Rad) -> Self {
        match r {
            Rad::Param(i) => self.p(k, *i),
            Rad::Dist(c, s) => {
                self.terms.push((k, Term::Dist));
                self.inputs.extend_from_slice(c);
                self.inputs.extend_from_slice(s);
                self
            }
        }
    }
    fn cst(mut self, c: f64) -> Self {
        self.c += c;
        self
    }
    fn eq(self) -> Eq {
        Eq { kind: EqKind::Linear { terms: self.terms, c: self.c }, inputs: self.inputs }
    }
}

enum Rad {
    Param(u32),
    Dist([u32; 2], [u32; 2]),
}

/// A direction at a curve end, pointing away from the joint.
enum Dir {
    Pts([u32; 4]),
    Fit { pts: SmallVec<[u32; 8]>, n: usize, at_end: bool },
}

fn cat(parts: &[&[u32]]) -> SmallVec<[u32; 8]> {
    let mut v = SmallVec::new();
    for p in parts {
        v.extend_from_slice(p);
    }
    v
}

fn l4(a: [u32; 2], b: [u32; 2]) -> [u32; 4] {
    [a[0], a[1], b[0], b[1]]
}

pub(crate) struct Builder<'a, L: Lookup> {
    pub sk: &'a Sketch,
    pub lk: &'a L,
    pub out: &'a mut Vec<Eq>,
}

impl<'a, L: Lookup> Builder<'a, L> {
    fn kind(&self, id: EntityId) -> Result<EntityKind, String> {
        self.sk.kind(id).ok_or_else(|| format!("unknown entity {id}"))
    }
    fn geom(&self, id: EntityId) -> Result<&'a Geometry, String> {
        self.sk.entity(id).map(|e| &e.geometry).ok_or_else(|| format!("unknown entity {id}"))
    }
    fn pt(&self, id: EntityId) -> Result<[u32; 2], String> {
        match self.kind(id)? {
            EntityKind::Point => Ok(self.lk.point(id)),
            k => Err(format!("{id} is a {k:?}, expected a point")),
        }
    }
    fn line(&self, id: EntityId) -> Result<[u32; 4], String> {
        match self.geom(id)? {
            Geometry::Line { start, end } => Ok(l4(self.lk.point(*start), self.lk.point(*end))),
            _ => Err(format!("{id} is not a line")),
        }
    }
    fn center(&self, id: EntityId) -> Result<[u32; 2], String> {
        let e = self.sk.entity(id).ok_or_else(|| format!("unknown entity {id}"))?;
        if e.kind() == EntityKind::Point {
            return Ok(self.lk.point(id));
        }
        e.center().map(|c| self.lk.point(c)).ok_or_else(|| format!("{id} has no center"))
    }
    fn radius(&self, id: EntityId) -> Result<Rad, String> {
        match self.geom(id)? {
            Geometry::Circle { .. } => Ok(Rad::Param(self.lk.own(id, 0))),
            Geometry::Arc { center, start, .. } => Ok(Rad::Dist(self.lk.point(*center), self.lk.point(*start))),
            _ => Err(format!("{id} is not a circle or arc")),
        }
    }
    fn ell(&self, id: EntityId) -> Result<[u32; 5], String> {
        match self.geom(id)? {
            Geometry::Ellipse { center, .. } | Geometry::EllipticalArc { center, .. } => {
                let c = self.lk.point(*center);
                Ok([c[0], c[1], self.lk.own(id, 0), self.lk.own(id, 1), self.lk.own(id, 2)])
            }
            _ => Err(format!("{id} is not an ellipse")),
        }
    }
    fn push(&mut self, kind: EqKind, inputs: SmallVec<[u32; 8]>) {
        self.out.push(Eq { kind, inputs });
    }
    fn lin(&mut self, l: Lin) {
        self.out.push(l.eq());
    }

    fn spline_points(&self, id: EntityId) -> Result<Vec<EntityId>, String> {
        match self.geom(id)? {
            Geometry::BSpline { control, .. } => Ok(control.clone()),
            Geometry::FitSpline { points, .. } => Ok(points.clone()),
            Geometry::Conic { start, control, end, .. } => Ok(vec![*start, *control, *end]),
            _ => Err(format!("{id} is not a spline")),
        }
    }

    fn spline_template(&self, id: EntityId) -> Result<Nurbs, String> {
        match self.geom(id)? {
            Geometry::BSpline { control, degree, knots, weights } => Ok(Nurbs {
                degree: *degree,
                knots: knots.clone(),
                ctrl: vec![glam::DVec2::ZERO; control.len()],
                weights: weights.clone(),
            }),
            Geometry::Conic { rho, .. } => Ok(Nurbs::conic(glam::DVec2::ZERO, glam::DVec2::ZERO, glam::DVec2::ZERO, *rho)),
            _ => Err(format!("{id} has no control-point form")),
        }
    }

    fn spline_end_point(&self, id: EntityId, at_end: bool) -> Result<[u32; 2], String> {
        let pts = self.spline_points(id)?;
        let p = if at_end { pts[pts.len() - 1] } else { pts[0] };
        Ok(self.lk.point(p))
    }

    fn spline_dir(&self, id: EntityId, at_end: bool) -> Result<Dir, String> {
        match self.geom(id)? {
            Geometry::BSpline { control, .. } => {
                let (p0, p1) = if at_end { (control[control.len() - 1], control[control.len() - 2]) } else { (control[0], control[1]) };
                Ok(Dir::Pts(l4(self.lk.point(p0), self.lk.point(p1))))
            }
            Geometry::Conic { start, control, end, .. } => {
                let p0 = if at_end { *end } else { *start };
                Ok(Dir::Pts(l4(self.lk.point(p0), self.lk.point(*control))))
            }
            // With tangent handles the end direction is the handle's.
            Geometry::FitSpline { points, handles: Some((h0, h1)) } => {
                let (p0, h) = if at_end { (points[points.len() - 1], *h1) } else { (points[0], *h0) };
                Ok(Dir::Pts(l4(self.lk.point(p0), self.lk.point(h))))
            }
            Geometry::FitSpline { points, handles: None } => {
                let mut pts = SmallVec::new();
                for p in points {
                    pts.extend_from_slice(&self.lk.point(*p));
                }
                Ok(Dir::Fit { pts, n: points.len(), at_end })
            }
            _ => Err(format!("{id} is not a spline")),
        }
    }

    fn dir_rel(&mut self, a: Dir, b: Dir, perp: bool) -> R {
        match (a, b) {
            (Dir::Pts(u), Dir::Pts(v)) => {
                self.push(if perp { EqKind::Perpendicular } else { EqKind::Parallel }, cat(&[&u, &v]));
                Ok(())
            }
            (Dir::Fit { pts, n, at_end }, Dir::Pts(v)) | (Dir::Pts(v), Dir::Fit { pts, n, at_end }) => {
                self.push(EqKind::FitEndDir { at_end, perp, n }, cat(&[&pts, &v]));
                Ok(())
            }
            // CON-CURVATURE (qa4 round 5 v3 s6): two Spline-tool splines.
            (Dir::Fit { pts: pa, n: na, at_end: ea }, Dir::Fit { pts: pb, at_end: eb, .. }) => {
                self.push(EqKind::FitFitDir { na, end_a: ea, end_b: eb, perp }, cat(&[&pa, &pb]));
                Ok(())
            }
        }
    }

    fn collinear(&mut self, a: EntityId, b: EntityId) -> R {
        let la = self.line(a)?;
        let lb = self.line(b)?;
        let (Some((a0, a1)), Some((b0, b1))) = (self.sk.endpoints(a), self.sk.endpoints(b)) else { unreachable!() };
        // an endpoint shared with `a` lies on it trivially (zero equation)
        for (q, qq) in [(b0, [lb[0], lb[1]]), (b1, [lb[2], lb[3]])] {
            if !self.lk.same(q, a0) && !self.lk.same(q, a1) {
                self.lin(Lin::new().pld(1.0, qq, la));
            }
        }
        Ok(())
    }

    fn sym_points(&mut self, a: [u32; 2], b: [u32; 2], axis: [u32; 4]) {
        let inp = cat(&[&a, &b, &axis]);
        self.push(EqKind::SymMid, inp.clone());
        self.push(EqKind::SymPerp, inp);
    }

    fn coincident(&mut self, a: EntityId, b: EntityId, c: &Constraint) -> R {
        let (ka, kb) = (self.kind(a)?, self.kind(b)?);
        let (p, q, kq) = if ka == EntityKind::Point { (a, b, kb) } else if kb == EntityKind::Point { (b, a, ka) } else { (a, b, kb) };
        let kp = self.kind(p)?;
        if kp == EntityKind::Point {
            let pp = self.pt(p)?;
            match kq {
                EntityKind::Point => {
                    let qq = self.pt(q)?;
                    self.lin(Lin::new().p(1.0, pp[0]).p(-1.0, qq[0]));
                    self.lin(Lin::new().p(1.0, pp[1]).p(-1.0, qq[1]));
                }
                EntityKind::Line => {
                    let l = self.line(q)?;
                    self.lin(Lin::new().pld(1.0, pp, l));
                }
                EntityKind::Circle | EntityKind::Arc => {
                    let cc = self.center(q)?;
                    let r = self.radius(q)?;
                    self.lin(Lin::new().dist(1.0, cc, pp).rad(-1.0, &r));
                }
                EntityKind::Ellipse | EntityKind::EllipticalArc => {
                    let e = self.ell(q)?;
                    self.push(EqKind::PointOnEllipse, cat(&[&pp, &e]));
                }
                EntityKind::BSpline | EntityKind::Conic => {
                    let tmpl = Arc::new(self.spline_template(q)?);
                    let t = self.lk.aux(0);
                    let mut inp: SmallVec<[u32; 8]> = cat(&[&pp, &[t]]);
                    for cp in self.spline_points(q)? {
                        inp.extend_from_slice(&self.lk.point(cp));
                    }
                    for comp in 0..2u8 {
                        self.push(EqKind::SplinePoint { comp, curve: tmpl.clone() }, inp.clone());
                    }
                }
                EntityKind::FitSpline => {
                    let t = self.lk.aux(0);
                    let mut inp: SmallVec<[u32; 8]> = cat(&[&pp, &[t]]);
                    for cp in self.spline_points(q)? {
                        inp.extend_from_slice(&self.lk.point(cp));
                    }
                    let handles = self.sk.entity(q).and_then(|e| e.spline_handles());
                    if let Some((h0, h1)) = handles {
                        inp.extend_from_slice(&self.lk.point(h0));
                        inp.extend_from_slice(&self.lk.point(h1));
                    }
                    for comp in 0..2u8 {
                        self.push(EqKind::FitPoint { comp, handles: handles.is_some() }, inp.clone());
                    }
                }
            }
            let _ = c;
            return Ok(());
        }
        match (kp, kq) {
            (EntityKind::Line, EntityKind::Line) => self.collinear(p, q),
            (x, y) if x.is_circular() && y.is_circular() => {
                let (c1, c2) = (self.center(p)?, self.center(q)?);
                self.lin(Lin::new().p(1.0, c1[0]).p(-1.0, c2[0]));
                self.lin(Lin::new().p(1.0, c1[1]).p(-1.0, c2[1]));
                let (r1, r2) = (self.radius(p)?, self.radius(q)?);
                self.lin(Lin::new().rad(1.0, &r1).rad(-1.0, &r2));
                Ok(())
            }
            _ => Err(format!("coincident not supported between {kp:?} and {kq:?}")),
        }
    }

    /// A point known to be the contact point between curve `a` and circle/arc
    /// `b` (shared endpoints, or an endpoint constrained onto the other curve).
    /// Tangency through a known contact point must be expressed as a direction
    /// condition: distance-based formulations are degenerate there.
    fn contact_point(&self, a: EntityId, b: EntityId) -> Option<EntityId> {
        let ea = self.sk.entity(a)?.endpoints();
        let eb = self.sk.entity(b)?.endpoints();
        if let (Some((a0, a1)), Some((b0, b1))) = (ea, eb) {
            for p in [a0, a1] {
                for q in [b0, b1] {
                    if self.lk.same(p, q) {
                        return Some(p);
                    }
                }
            }
        }
        if let Some((a0, a1)) = ea {
            for p in [a0, a1] {
                if self.lk.on_curve(p, b) {
                    return Some(p);
                }
            }
        }
        if let Some((b0, b1)) = eb {
            for q in [b0, b1] {
                if self.lk.on_curve(q, a) {
                    return Some(q);
                }
            }
        }
        None
    }

    fn tangent(&mut self, a: EntityId, b: EntityId, c: &Constraint) -> R {
        let (ka, kb) = (self.kind(a)?, self.kind(b)?);
        let (a, b, ka, kb) = if tangent_rank(ka) <= tangent_rank(kb) { (a, b, ka, kb) } else { (b, a, kb, ka) };
        let h = &c.hints;
        if kb.is_spline() {
            let dir_s = self.spline_dir(b, h.end_b)?;
            return match ka {
                EntityKind::Line => {
                    let l = self.line(a)?;
                    self.dir_rel(dir_s, Dir::Pts(l), false)
                }
                k if k.is_circular() => {
                    let cc = self.center(a)?;
                    let p0 = self.spline_end_point(b, h.end_b)?;
                    self.dir_rel(dir_s, Dir::Pts(l4(cc, p0)), true)
                }
                k if k.is_spline() => {
                    let dir_o = self.spline_dir(a, h.end_a)?;
                    self.dir_rel(dir_s, dir_o, false)
                }
                k => Err(format!("tangent between {k:?} and spline is not supported")),
            };
        }
        match (ka, kb) {
            (EntityKind::Line, EntityKind::Line) => self.collinear(a, b),
            (EntityKind::Line, k) if k.is_circular() => {
                let l = self.line(a)?;
                let cc = self.center(b)?;
                if let Some(q) = self.contact_point(a, b) {
                    let qq = self.lk.point(q);
                    self.push(EqKind::Perpendicular, cat(&[&cc, &qq, &l]));
                    return Ok(());
                }
                let r = self.radius(b)?;
                self.lin(Lin::new().pld(h.sign, cc, l).rad(-1.0, &r));
                Ok(())
            }
            (x, y) if x.is_circular() && y.is_circular() => {
                let (c1, c2) = (self.center(a)?, self.center(b)?);
                if let Some(q) = self.contact_point(a, b) {
                    let qq = self.lk.point(q);
                    self.push(EqKind::Parallel, cat(&[&c1, &qq, &c2, &qq]));
                    return Ok(());
                }
                let (r1, r2) = (self.radius(a)?, self.radius(b)?);
                if h.internal {
                    self.lin(Lin::new().dist(1.0, c1, c2).rad(-h.sign2, &r1).rad(h.sign2, &r2));
                } else {
                    self.lin(Lin::new().dist(1.0, c1, c2).rad(-1.0, &r1).rad(-1.0, &r2));
                }
                Ok(())
            }
            (EntityKind::Line, k) if k.is_elliptical() => {
                let l = self.line(a)?;
                let e = self.ell(b)?;
                let t = self.lk.aux(0);
                let inp = cat(&[&e, &[t], &l]);
                self.push(EqKind::EllipseOnLine, inp.clone());
                self.push(EqKind::EllipseTangent, inp);
                Ok(())
            }
            _ => Err(format!("tangent not supported between {ka:?} and {kb:?}")),
        }
    }

    /// Onshape's OFFSET: `curve` at the radius of `reference` from `master`.
    fn offset(&mut self, reference: EntityId, curve: EntityId, master: EntityId, side: f64) -> R {
        let d = self.radius(reference)?;
        match (self.kind(curve)?, self.kind(master)?) {
            (EntityKind::Line, EntityKind::Line) => {
                let (lc, lm) = (self.line(curve)?, self.line(master)?);
                for p in [[lc[0], lc[1]], [lc[2], lc[3]]] {
                    self.lin(Lin::new().pld(side, p, lm).rad(-1.0, &d));
                }
                Ok(())
            }
            (k, EntityKind::Point) if k.is_circular() => {
                let r = self.radius(curve)?;
                self.lin(Lin::new().rad(1.0, &r).rad(-1.0, &d));
                let cid = self.sk.entity(curve).and_then(|e| e.center()).ok_or_else(|| format!("{curve} has no center"))?;
                if !self.lk.same(cid, master) {
                    let (cc, pm) = (self.center(curve)?, self.lk.point(master));
                    self.lin(Lin::new().p(1.0, cc[0]).p(-1.0, pm[0]));
                    self.lin(Lin::new().p(1.0, cc[1]).p(-1.0, pm[1]));
                }
                Ok(())
            }
            (kc, km) => Err(format!("offset not supported between {kc:?} and {km:?}")),
        }
    }

    /// Onshape's tool OFFSET: see [`ConstraintKind::OffsetCurve`].
    fn offset_curve(&mut self, master: EntityId, curve: EntityId, second: Option<(EntityId, EntityId)>, sides: (f64, f64), first_shape: bool) -> R {
        if first_shape {
            self.offset_shape(master, curve)?;
        }
        if let Some((s, sc)) = second {
            self.offset_shape(s, sc)?;
            let lin = self.pair_distance(Lin::new(), sides.1, s, sc)?;
            let lin = self.pair_distance(lin, -sides.0, master, curve)?;
            self.lin(lin);
        }
        Ok(())
    }

    /// A copy parallel to its source line, or concentric with its circle /
    /// arc (no equation when they share the centre point).
    fn offset_shape(&mut self, src: EntityId, copy: EntityId) -> R {
        match (self.kind(src)?, self.kind(copy)?) {
            (EntityKind::Line, EntityKind::Line) => {
                let (ls, lc) = (self.line(src)?, self.line(copy)?);
                self.push(EqKind::Parallel, cat(&[&ls, &lc]));
                Ok(())
            }
            (ks, kc) if ks.is_circular() && kc.is_circular() => {
                let (cs, cc) = (self.center(src)?, self.center(copy)?);
                if cs != cc {
                    self.lin(Lin::new().p(1.0, cs[0]).p(-1.0, cc[0]));
                    self.lin(Lin::new().p(1.0, cs[1]).p(-1.0, cc[1]));
                }
                Ok(())
            }
            (ks, kc) => Err(format!("offset not supported between {ks:?} and {kc:?}")),
        }
    }

    /// `k` × the pair's signed offset distance added to `lin`: a line copy's
    /// midpoint from the source line, or a circular copy's radius minus the
    /// source's.
    fn pair_distance(&self, lin: Lin, k: f64, src: EntityId, copy: EntityId) -> Result<Lin, String> {
        match (self.kind(src)?, self.kind(copy)?) {
            (EntityKind::Line, EntityKind::Line) => {
                let (ls, lc) = (self.line(src)?, self.line(copy)?);
                Ok(lin.pldm(k, [lc[0], lc[1]], [lc[2], lc[3]], ls))
            }
            (ks, kc) if ks.is_circular() && kc.is_circular() => {
                let (rs, rc) = (self.radius(src)?, self.radius(copy)?);
                Ok(lin.rad(k, &rc).rad(-k, &rs))
            }
            (ks, kc) => Err(format!("offset not supported between {ks:?} and {kc:?}")),
        }
    }

    /// [`Builder::curv_spec`], or a fit spline's end (the Spline tool's).
    fn curv_side(&self, id: EntityId, at_end: bool) -> Result<(CurvSide, SmallVec<[u32; 8]>), String> {
        if let Geometry::FitSpline { points, handles } = self.geom(id)? {
            let mut inp = SmallVec::new();
            for p in points.iter().chain(handles.iter().flat_map(|(a, b)| [a, b])) {
                inp.extend_from_slice(&self.lk.point(*p));
            }
            return Ok((CurvSide::Fit { n: points.len(), at_end, handles: handles.is_some() }, inp));
        }
        let (s, inp) = self.curv_spec(id, at_end)?;
        Ok((CurvSide::Spec(s), inp))
    }

    fn curv_spec(&self, id: EntityId, at_end: bool) -> Result<(CurvSpec, SmallVec<[u32; 8]>), String> {
        match self.geom(id)? {
            Geometry::Line { .. } => Ok((CurvSpec::Line, SmallVec::new())),
            Geometry::Arc { center, start, end } => {
                let (sign, p) = if at_end { (-1.0, *end) } else { (1.0, *start) };
                Ok((CurvSpec::Arc { sign }, cat(&[&self.lk.point(*center), &self.lk.point(p)])))
            }
            Geometry::BSpline { control, .. } => {
                let tmpl = self.spline_template(id)?;
                if control.len() < 3 || tmpl.degree < 2 {
                    return Err("curvature needs a spline of degree >= 2".into());
                }
                let (alpha, b1, b0) = tmpl.end_coeffs(at_end);
                let n = control.len();
                let ids = if at_end { [control[n - 1], control[n - 2], control[n - 3]] } else { [control[0], control[1], control[2]] };
                Ok((CurvSpec::Spline { alpha, b1, b0 }, cat(&[&self.lk.point(ids[0]), &self.lk.point(ids[1]), &self.lk.point(ids[2])])))
            }
            _ => Err(format!("curvature constraint not supported for {id}")),
        }
    }

    fn distance(&mut self, a: EntityId, b: EntityId, v: f64, mode: DistanceMode, c: &Constraint) -> R {
        let (ka, kb) = (self.kind(a)?, self.kind(b)?);
        let (a, b, ka, kb) = if dist_rank(ka) <= dist_rank(kb) { (a, b, ka, kb) } else { (b, a, kb, ka) };
        let h = &c.hints;
        let circ_ok = |k: EntityKind| k.is_circular();
        match (dist_rank(ka), dist_rank(kb)) {
            (0, 0) => {
                let (p, q) = (self.pt(a)?, self.pt(b)?);
                self.lin(Lin::new().dist(1.0, p, q).cst(-v));
            }
            (0, 1) => {
                let p = self.pt(a)?;
                let l = self.line(b)?;
                let k = crate::constraints::centerline_factor(mode);
                self.lin(Lin::new().pld(h.sign * k, p, l).cst(-v));
            }
            (1, 1) => {
                let la = self.line(a)?;
                let lb = self.line(b)?;
                let k = crate::constraints::centerline_factor(mode);
                self.lin(Lin::new().pldm(h.sign * k, [lb[0], lb[1]], [lb[2], lb[3]], la).cst(-v));
            }
            (0, 2) if circ_ok(kb) => {
                let p = self.pt(a)?;
                let cc = self.center(b)?;
                let r = self.radius(b)?;
                let l = match mode {
                    DistanceMode::Center | DistanceMode::Centerline => Lin::new().dist(1.0, cc, p),
                    DistanceMode::Minimum => Lin::new().dist(h.sign, cc, p).rad(-h.sign, &r),
                    DistanceMode::Maximum => Lin::new().dist(1.0, cc, p).rad(1.0, &r),
                };
                self.lin(l.cst(-v));
            }
            (1, 2) if circ_ok(kb) => {
                let l = self.line(a)?;
                let cc = self.center(b)?;
                let r = self.radius(b)?;
                let e = match mode {
                    DistanceMode::Center => Lin::new().pld(h.sign, cc, l),
                    DistanceMode::Centerline => Lin::new().pld(2.0 * h.sign, cc, l),
                    DistanceMode::Minimum => Lin::new().pld(h.sign, cc, l).rad(-1.0, &r),
                    DistanceMode::Maximum => Lin::new().pld(h.sign, cc, l).rad(1.0, &r),
                };
                self.lin(e.cst(-v));
            }
            (2, 2) if circ_ok(ka) && circ_ok(kb) => {
                let (c1, c2) = (self.center(a)?, self.center(b)?);
                let (r1, r2) = (self.radius(a)?, self.radius(b)?);
                let e = match mode {
                    DistanceMode::Center | DistanceMode::Centerline => Lin::new().dist(1.0, c1, c2),
                    DistanceMode::Minimum if h.internal => Lin::new().dist(-1.0, c1, c2).rad(h.sign2, &r1).rad(-h.sign2, &r2),
                    DistanceMode::Minimum => Lin::new().dist(1.0, c1, c2).rad(-1.0, &r1).rad(-1.0, &r2),
                    DistanceMode::Maximum => Lin::new().dist(1.0, c1, c2).rad(1.0, &r1).rad(1.0, &r2),
                };
                self.lin(e.cst(-v));
            }
            (2, 2) | (0, 2) | (1, 2) if mode == DistanceMode::Center => {
                // centers of ellipses
                let ca = if ka == EntityKind::Point { self.pt(a)? } else { self.center(a)? };
                let cb = self.center(b)?;
                if ka == EntityKind::Line {
                    let l = self.line(a)?;
                    self.lin(Lin::new().pld(h.sign, cb, l).cst(-v));
                } else {
                    self.lin(Lin::new().dist(1.0, ca, cb).cst(-v));
                }
            }
            _ => return Err(format!("distance not supported between {ka:?} and {kb:?}")),
        }
        Ok(())
    }

    fn ref_pt(&self, id: EntityId) -> Result<[u32; 2], String> {
        let p = self.sk.ref_point(id).ok_or_else(|| format!("{id} has no reference point"))?;
        Ok(self.lk.point(p))
    }

    /// Equations of a constraint.
    pub fn constraint(&mut self, c: &Constraint) -> R {
        use ConstraintKind as K;
        let ents = c.kind.entities();
        if ents.len() >= 2 && ents[0] == ents[1] {
            return Err("a constraint needs two different entities".into());
        }
        match &c.kind {
            K::Coincident { a, b } | K::Pierce { point: a, target: b } => self.coincident(*a, *b, c),
            K::Concentric { a, b } => {
                let (ca, cb) = (self.center(*a)?, self.center(*b)?);
                self.lin(Lin::new().p(1.0, ca[0]).p(-1.0, cb[0]));
                self.lin(Lin::new().p(1.0, ca[1]).p(-1.0, cb[1]));
                Ok(())
            }
            K::Parallel { a, b } | K::Perpendicular { a, b } => {
                let (la, lb) = (self.line(*a)?, self.line(*b)?);
                let k = if matches!(c.kind, K::Parallel { .. }) { EqKind::Parallel } else { EqKind::Perpendicular };
                self.push(k, cat(&[&la, &lb]));
                Ok(())
            }
            K::Horizontal { a, b } | K::Vertical { a, b } => {
                // horizontal: equal y (comp 1); vertical: equal x (comp 0)
                let comp = if matches!(c.kind, K::Horizontal { .. }) { 1 } else { 0 };
                match b {
                    None => match self.kind(*a)? {
                        EntityKind::Line => {
                            let l = self.line(*a)?;
                            self.lin(Lin::new().p(1.0, l[comp]).p(-1.0, l[2 + comp]));
                            Ok(())
                        }
                        k if k.is_elliptical() => {
                            let e = self.ell(*a)?;
                            // major axis horizontal: major.y = 0; vertical: major.x = 0
                            self.lin(Lin::new().p(1.0, e[2 + comp]));
                            Ok(())
                        }
                        k => Err(format!("horizontal/vertical needs a line, got {k:?}")),
                    },
                    Some(b) => {
                        let (pa, pb) = (self.pt(*a)?, self.pt(*b)?);
                        self.lin(Lin::new().p(1.0, pa[comp]).p(-1.0, pb[comp]));
                        Ok(())
                    }
                }
            }
            K::Tangent { a, b } => self.tangent(*a, *b, c),
            K::Offset { reference, curve, master, side, .. } => self.offset(*reference, *curve, *master, *side),
            K::OffsetCurve { master, curve, second, sides, first_shape } => self.offset_curve(*master, *curve, *second, *sides, *first_shape),
            K::Equal { a, b } => {
                let (ka, kb) = (self.kind(*a)?, self.kind(*b)?);
                match (ka, kb) {
                    (EntityKind::Line, EntityKind::Line) => {
                        let (la, lb) = (self.line(*a)?, self.line(*b)?);
                        self.lin(Lin::new().dist(1.0, [la[0], la[1]], [la[2], la[3]]).dist(-1.0, [lb[0], lb[1]], [lb[2], lb[3]]));
                        Ok(())
                    }
                    (x, y) if x.is_circular() && y.is_circular() => {
                        let (ra, rb) = (self.radius(*a)?, self.radius(*b)?);
                        self.lin(Lin::new().rad(1.0, &ra).rad(-1.0, &rb));
                        Ok(())
                    }
                    (x, y) if x.is_elliptical() && y.is_elliptical() => {
                        let (ea, eb) = (self.ell(*a)?, self.ell(*b)?);
                        self.lin(Lin::new().norm(1.0, [ea[2], ea[3]]).norm(-1.0, [eb[2], eb[3]]));
                        self.lin(Lin::new().p(1.0, ea[4]).p(-1.0, eb[4]));
                        Ok(())
                    }
                    _ => Err(format!("equal not supported between {ka:?} and {kb:?}")),
                }
            }
            K::Midpoint { point, curve } => {
                let (p, q) = if self.kind(*point)? == EntityKind::Point { (*point, *curve) } else { (*curve, *point) };
                let pp = self.pt(p)?;
                match self.geom(q)? {
                    Geometry::Line { start, end } => {
                        let (s, e) = (self.lk.point(*start), self.lk.point(*end));
                        for k in 0..2 {
                            self.lin(Lin::new().p(2.0, pp[k]).p(-1.0, s[k]).p(-1.0, e[k]));
                        }
                        Ok(())
                    }
                    Geometry::Arc { center, start, end } => {
                        let (cc, s, e) = (self.lk.point(*center), self.lk.point(*start), self.lk.point(*end));
                        self.lin(Lin::new().dist(1.0, cc, pp).dist(-1.0, cc, s));
                        self.lin(Lin::new().dist(1.0, pp, s).dist(-1.0, pp, e));
                        Ok(())
                    }
                    _ => Err("midpoint needs a line or an arc".into()),
                }
            }
            K::Symmetric { a, b, axis } => {
                let ax = self.line(*axis)?;
                let (ka, kb) = (self.kind(*a)?, self.kind(*b)?);
                match (ka, kb) {
                    (EntityKind::Point, EntityKind::Point) => {
                        let (pa, pb) = (self.pt(*a)?, self.pt(*b)?);
                        self.sym_points(pa, pb, ax);
                        Ok(())
                    }
                    (EntityKind::Line, EntityKind::Line) => {
                        let (la, lb) = (self.line(*a)?, self.line(*b)?);
                        let (a0, a1) = ([la[0], la[1]], [la[2], la[3]]);
                        let (b0, b1) = ([lb[0], lb[1]], [lb[2], lb[3]]);
                        if c.hints.reverse {
                            self.sym_points(a0, b1, ax);
                            self.sym_points(a1, b0, ax);
                        } else {
                            self.sym_points(a0, b0, ax);
                            self.sym_points(a1, b1, ax);
                        }
                        Ok(())
                    }
                    (EntityKind::Arc, EntityKind::Arc) => {
                        let (Geometry::Arc { center: ca, start: sa, end: ea }, Geometry::Arc { center: cb, start: sb, end: eb }) =
                            (self.geom(*a)?, self.geom(*b)?)
                        else {
                            unreachable!()
                        };
                        let p = |x: &EntityId| self.lk.point(*x);
                        let (ca, sa, ea, cb, sb, eb) = (p(ca), p(sa), p(ea), p(cb), p(sb), p(eb));
                        self.sym_points(ca, cb, ax);
                        self.sym_points(sa, eb, ax);
                        self.push(EqKind::SymPerp, cat(&[&ea, &sb, &ax]));
                        Ok(())
                    }
                    (x, y) if x.is_circular() && y.is_circular() => {
                        let (ca, cb) = (self.center(*a)?, self.center(*b)?);
                        self.sym_points(ca, cb, ax);
                        let (ra, rb) = (self.radius(*a)?, self.radius(*b)?);
                        self.lin(Lin::new().rad(1.0, &ra).rad(-1.0, &rb));
                        Ok(())
                    }
                    _ => Err(format!("symmetric not supported between {ka:?} and {kb:?}")),
                }
            }
            K::Fix { entity } => {
                let items = fix_items(self.sk, *entity);
                if items.len() != c.hints.anchor.len() {
                    return Err("fix constraint anchor does not match entity".into());
                }
                for (item, v) in items.iter().zip(c.hints.anchor.iter()) {
                    match *item {
                        FixItem::PointCoord(p, k) => {
                            let pp = self.lk.point(p);
                            self.lin(Lin::new().p(1.0, pp[k]).cst(-v));
                        }
                        FixItem::Own(e, k) => {
                            let i = self.lk.own(e, k);
                            self.lin(Lin::new().p(1.0, i).cst(-v));
                        }
                        FixItem::Ray { center, point } => {
                            let (cc, pp) = (self.lk.point(center), self.lk.point(point));
                            let (s, co) = v.sin_cos();
                            self.lin(Lin::new().p(-s, pp[0]).p(s, cc[0]).p(co, pp[1]).p(-co, cc[1]));
                        }
                    }
                }
                Ok(())
            }
            K::Normal { line, curve } => {
                let l = self.line(*line)?;
                match self.kind(*curve)? {
                    k if k.is_circular() => {
                        let cc = self.center(*curve)?;
                        self.lin(Lin::new().pld(1.0, cc, l));
                        Ok(())
                    }
                    EntityKind::Line => {
                        let l2 = self.line(*curve)?;
                        self.push(EqKind::Perpendicular, cat(&[&l, &l2]));
                        Ok(())
                    }
                    k if k.is_spline() => {
                        let d = self.spline_dir(*curve, c.hints.end_b)?;
                        self.dir_rel(d, Dir::Pts(l), true)
                    }
                    k => Err(format!("normal not supported for {k:?}")),
                }
            }
            K::Collinear { a, b } => self.collinear(*a, *b),
            K::LengthRatio { a, b, ratio } => {
                if !(ratio.is_finite() && *ratio > 0.0) {
                    return Err("length ratio must be positive".into());
                }
                let (la, lb) = (self.line(*a)?, self.line(*b)?);
                self.lin(Lin::new().dist(1.0, [la[0], la[1]], [la[2], la[3]]).dist(-ratio, [lb[0], lb[1]], [lb[2], lb[3]]));
                Ok(())
            }
            K::Curvature { a, b } => {
                self.tangent(*a, *b, c)?;
                let (ka, kb) = (self.kind(*a)?, self.kind(*b)?);
                let (a, b) = if tangent_rank(ka) <= tangent_rank(kb) { (*a, *b) } else { (*b, *a) };
                let (sa, ia) = self.curv_side(a, c.hints.end_a)?;
                let (sb, ib) = self.curv_side(b, c.hints.end_b)?;
                match (sa, sb) {
                    (CurvSide::Spec(sa), CurvSide::Spec(sb)) => self.push(EqKind::Curvature { a: sa, b: sb }, cat(&[&ia, &ib])),
                    (sa, sb) => self.push(EqKind::CurvatureNum { a: sa, b: sb }, cat(&[&ia, &ib])),
                }
                Ok(())
            }
            K::Distance { a, b, value, mode } => self.distance(*a, *b, *value, *mode, c),
            K::HorizontalDistance { a, b, value } | K::VerticalDistance { a, b, value } => {
                let comp = if matches!(c.kind, K::HorizontalDistance { .. }) { 0 } else { 1 };
                let (pa, pb) = (self.ref_pt(*a)?, self.ref_pt(*b)?);
                let s = c.hints.sign;
                self.lin(Lin::new().p(s, pb[comp]).p(-s, pa[comp]).cst(-value));
                Ok(())
            }
            K::Length { curve, value } => match self.geom(*curve)? {
                Geometry::Line { start, end } => {
                    let (s, e) = (self.lk.point(*start), self.lk.point(*end));
                    self.lin(Lin::new().dist(1.0, s, e).cst(-value));
                    Ok(())
                }
                Geometry::Arc { center, start, end } => {
                    let inp = cat(&[&self.lk.point(*center), &self.lk.point(*start), &self.lk.point(*end)]);
                    self.push(EqKind::ArcLength { len: *value }, inp);
                    Ok(())
                }
                _ => Err("length needs a line or an arc".into()),
            },
            K::Radius { curve, value } | K::Diameter { curve, value } => {
                let k = if matches!(c.kind, K::Radius { .. }) { 1.0 } else { 2.0 };
                let r = self.radius(*curve)?;
                self.lin(Lin::new().rad(k, &r).cst(-value));
                Ok(())
            }
            K::Angle { a, b, value } => {
                let (la, lb) = (self.line(*a)?, self.line(*b)?);
                let h = &c.hints;
                let u = if h.flip_a { [la[2], la[3], la[0], la[1]] } else { la };
                let v = if h.flip_b { [lb[2], lb[3], lb[0], lb[1]] } else { lb };
                let inp = if h.reverse { cat(&[&v, &u]) } else { cat(&[&u, &v]) };
                self.push(EqKind::Angle { cos: value.cos(), sin: value.sin() }, inp);
                Ok(())
            }
            K::ArcAngle { arc, value } => match self.geom(*arc)? {
                Geometry::Arc { center, start, end } => {
                    let (cc, s, e) = (self.lk.point(*center), self.lk.point(*start), self.lk.point(*end));
                    self.push(EqKind::Angle { cos: value.cos(), sin: value.sin() }, cat(&[&cc, &s, &cc, &e]));
                    Ok(())
                }
                _ => Err("arc angle needs an arc".into()),
            },
            K::PatternLink { seed, inst, ref_seed, ref_inst, k } => {
                let (s, i, rs, ri) = (self.pt(*seed)?, self.pt(*inst)?, self.pt(*ref_seed)?, self.pt(*ref_inst)?);
                for comp in 0..2 {
                    // inst - seed - k * ref_inst + k * ref_seed = 0, merging repeated unknowns.
                    let mut terms: Vec<(u32, f64)> = Vec::new();
                    for (var, coef) in [(i[comp], 1.0), (s[comp], -1.0), (ri[comp], -*k), (rs[comp], *k)] {
                        match terms.iter_mut().find(|t| t.0 == var) {
                            Some(t) => t.1 += coef,
                            None => terms.push((var, coef)),
                        }
                    }
                    let mut l = Lin::new();
                    for (var, coef) in terms.into_iter().filter(|t| t.1.abs() > 1e-15) {
                        l = l.p(coef, var);
                    }
                    self.lin(l);
                }
                Ok(())
            }
            K::CircularPatternLink { seed, inst, center, angle } => {
                let (sin, cos) = angle.sin_cos();
                if self.sk.kind(*seed) == Some(EntityKind::Point) {
                    let (s, i, c) = (self.pt(*seed)?, self.pt(*inst)?, self.pt(*center)?);
                    // i = c + R(s - c). Keep the center as an unknown so it
                    // can be dragged along with the constrained instances.
                    self.lin(Lin::new().p(1.0, i[0]).p(-cos, s[0]).p(sin, s[1]).p(cos - 1.0, c[0]).p(-sin, c[1]));
                    self.lin(Lin::new().p(1.0, i[1]).p(-sin, s[0]).p(-cos, s[1]).p(sin, c[0]).p(cos - 1.0, c[1]));
                } else {
                    let (s, i) = (self.ell(*seed)?, self.ell(*inst)?);
                    self.lin(Lin::new().p(1.0, i[2]).p(-cos, s[2]).p(sin, s[3]));
                    self.lin(Lin::new().p(1.0, i[3]).p(-sin, s[2]).p(-cos, s[3]));
                    self.lin(Lin::new().p(1.0, i[4]).p(-1.0, s[4]));
                }
                Ok(())
            }
            K::Quadrant { point, ellipse, major } => {
                let p = self.pt(*point)?;
                let e = self.ell(*ellipse)?;
                self.push(EqKind::PointOnEllipse, cat(&[&p, &e]));
                self.push(EqKind::OnEllipseAxis { minor: !*major }, cat(&[&p, &e[..4]]));
                Ok(())
            }
            // Applied to the conic directly (see `Sketch::set_dimension_value`).
            K::Rho { conic, .. } => match self.geom(*conic)? {
                Geometry::Conic { .. } => Ok(()),
                _ => Err("rho needs a conic".into()),
            },
            K::EllipseRadius { ellipse, major, value } => {
                let e = self.ell(*ellipse)?;
                if *major {
                    self.lin(Lin::new().norm(1.0, [e[2], e[3]]).cst(-value));
                } else {
                    self.lin(Lin::new().p(1.0, e[4]).cst(-value));
                }
                Ok(())
            }
        }
    }

    /// Internal equations of an entity (arc radius consistency, elliptical
    /// arc endpoints on the ellipse).
    pub fn internal(&mut self, id: EntityId) {
        let Some(e) = self.sk.entity(id) else { return };
        match &e.geometry {
            Geometry::Arc { center, start, end } => {
                let (c, s, en) = (self.lk.point(*center), self.lk.point(*start), self.lk.point(*end));
                self.lin(Lin::new().dist(1.0, c, en).dist(-1.0, c, s));
            }
            Geometry::EllipticalArc { start, end, .. } => {
                if let Ok(el) = self.ell(id) {
                    let (s, en) = (self.lk.point(*start), self.lk.point(*end));
                    self.push(EqKind::PointOnEllipse, cat(&[&s, &el]));
                    self.push(EqKind::PointOnEllipse, cat(&[&en, &el]));
                }
            }
            _ => {}
        }
    }
}

struct DummyLookup {
    max_aux: Cell<usize>,
}

impl Lookup for DummyLookup {
    fn point(&self, _: EntityId) -> [u32; 2] {
        [0, 1]
    }
    fn own(&self, _: EntityId, _: usize) -> u32 {
        2
    }
    fn aux(&self, k: usize) -> u32 {
        self.max_aux.set(self.max_aux.get().max(k + 1));
        3
    }
    fn same(&self, a: EntityId, b: EntityId) -> bool {
        a == b
    }
    fn on_curve(&self, _: EntityId, _: EntityId) -> bool {
        false
    }
}

/// Checks that a constraint is well formed for the current entities.
pub(crate) fn validate(sk: &Sketch, c: &Constraint) -> Result<(), String> {
    let lk = DummyLookup { max_aux: Cell::new(0) };
    let mut out = Vec::new();
    let mut b = Builder { sk, lk: &lk, out: &mut out };
    b.constraint(c)?;
    if lk.max_aux.get() > c.aux.len() {
        return Err("could not initialize auxiliary unknowns".into());
    }
    if c.kind.is_dimension() {
        let v = c.kind.value().unwrap_or(0.0);
        if !v.is_finite() {
            return Err("dimension value must be finite".into());
        }
    }
    Ok(())
}


#[cfg(test)]
mod tests {
    use super::*;

    fn check_grad(eq: &Eq, x: &[f64]) {
        let mut g = vec![0.0; x.len()];
        let r = eq.eval_grad(x, &mut g);
        assert!((r - eq.eval(x)).abs() < 1e-12);
        let mut xx = x.to_vec();
        for i in 0..x.len() {
            let h = 1e-6;
            xx[i] = x[i] + h;
            let fp = eq.eval(&xx);
            xx[i] = x[i] - h;
            let fm = eq.eval(&xx);
            xx[i] = x[i];
            let fd = (fp - fm) / (2.0 * h);
            assert!((fd - g[i]).abs() < 1e-5 * (1.0 + fd.abs()), "{:?} input {i}: fd {fd} vs {}", eq.kind, g[i]);
        }
    }

    #[test]
    fn kernel_gradients_match_finite_differences() {
        let x8 = [0.1, 0.2, 1.3, 0.4, -0.5, 0.9, 0.7, -1.1];
        let kinds = vec![
            (EqKind::Parallel, 8),
            (EqKind::Perpendicular, 8),
            (EqKind::Angle { cos: 0.3f64.cos(), sin: 0.3f64.sin() }, 8),
            (EqKind::SymMid, 8),
            (EqKind::SymPerp, 8),
            (EqKind::ArcLength { len: 1.0 }, 6),
            (EqKind::PointOnEllipse, 7),
            (EqKind::OnEllipseAxis { minor: false }, 6),
            (EqKind::OnEllipseAxis { minor: true }, 6),
            (EqKind::Linear { terms: smallvec![(1.0, Term::Dist), (-2.0, Term::Param)], c: 0.3 }, 5),
            (EqKind::Linear { terms: smallvec![(1.0, Term::PLDist), (1.0, Term::Param)], c: 0.0 }, 7),
            (EqKind::Linear { terms: smallvec![(1.0, Term::PLDistMid)], c: 0.0 }, 8),
            (EqKind::Curvature { a: CurvSpec::Spline { alpha: 3.0, b1: 6.0, b0: 2.0 }, b: CurvSpec::Arc { sign: 1.0 } }, 10),
        ];
        let x10 = [0.1, 0.2, 1.3, 0.4, -0.5, 0.9, 0.7, -1.1, 2.0, 0.3];
        for (k, n) in kinds {
            let eq = Eq { kind: k, inputs: (0..n as u32).collect() };
            let x: Vec<f64> = if n <= 8 { x8[..n].to_vec() } else { x10[..n].to_vec() };
            check_grad(&eq, &x);
        }
        let ell = [0.0, 0.0, 2.0, 0.5, 1.0, 0.4, -3.0, 1.0, 3.0, 2.0];
        check_grad(&Eq { kind: EqKind::EllipseTangent, inputs: (0..10).collect() }, &ell);
        check_grad(&Eq { kind: EqKind::EllipseOnLine, inputs: (0..10).collect() }, &ell);
        let sp = Nurbs::new(3, vec![glam::DVec2::ZERO; 5]);
        let xs = [0.5, 0.5, 0.37, 0.0, 0.0, 1.0, 2.0, 3.0, 3.0, 4.0, 0.0, 6.0, 1.0];
        for comp in 0..2 {
            check_grad(&Eq { kind: EqKind::SplinePoint { comp, curve: Arc::new(sp.clone()) }, inputs: (0..13).collect() }, &xs);
        }
    }
}
