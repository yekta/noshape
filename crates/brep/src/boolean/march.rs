//! Our own sections of an elementary surface given by its signed distance (a plane, a sphere) with a free-form
//! surface (c2-rings-loft's slot planes across its lofted ring, cf-surface-pattern's balls on a B-spline: OCCT's
//! walking lines a quarter of those cuts' time): the distance over the free-form surface's parameter box on a grid of
//! its spans, cells its control hull keeps off the other surface skipped, the curves found where the grid's lines
//! cross it and chained through the cells, then filled in where cubic pieces between those points stray. Anything the
//! grid can't tell (a cell the surface may touch without crossing its lines, a cell crossed four times) is None:
//! OCCT's.

use crate::freeform::FreeSurface;
use crate::nurbs::NurbsCurve;
use glam::{DVec2, DVec3};

/// A section curve: on the surface (its parameters, z 0) and in space, cubic pieces with the same parameters (the
/// length along it), and how far its points stray from the plane and the surface.
pub(crate) struct Marched {
    pub(crate) uv: NurbsCurve,
    pub(crate) curve: NurbsCurve,
    pub(crate) t1: f64,
    pub(crate) error: f64,
}

/// An elementary surface by its signed distance.
#[derive(Clone, Copy)]
pub(crate) enum Implicit {
    /// `n · p = d` (n unit).
    Plane { n: DVec3, d: f64 },
    /// Radius `r` about the line through `o` along `a` (unit).
    Cylinder { o: DVec3, a: DVec3, r: f64 },
}

impl Implicit {
    fn at(&self, p: DVec3) -> f64 {
        match *self {
            Implicit::Plane { n, d } => n.dot(p) - d,
            Implicit::Cylinder { o, a, r } => {
                let w = p - o;
                (w - a * w.dot(a)).length() - r
            }
        }
    }

    fn grad(&self, p: DVec3) -> DVec3 {
        match *self {
            Implicit::Plane { n, .. } => n,
            Implicit::Cylinder { o, a, .. } => {
                let w = p - o;
                (w - a * w.dot(a)).normalize_or_zero()
            }
        }
    }

    /// Its distance's range over box `l`..`h`.
    fn range(&self, l: DVec3, h: DVec3) -> (f64, f64) {
        match *self {
            Implicit::Plane { n, d } => {
                let hi = DVec3::new(if n.x >= 0.0 { h.x } else { l.x }, if n.y >= 0.0 { h.y } else { l.y }, if n.z >= 0.0 { h.z } else { l.z });
                let lo = DVec3::new(if n.x >= 0.0 { l.x } else { h.x }, if n.y >= 0.0 { l.y } else { h.y }, if n.z >= 0.0 { l.z } else { h.z });
                (n.dot(lo) - d, n.dot(hi) - d)
            }
            // (Farthest from the axis at a corner; nearest no nearer than the centre's distance less the half
            // diagonal.)
            Implicit::Cylinder { o, a, r } => {
                let off = |p: DVec3| {
                    let w = p - o;
                    (w - a * w.dot(a)).length()
                };
                let far = (0..8).map(|k| off(DVec3::new(if k & 1 == 0 { l.x } else { h.x }, if k & 2 == 0 { l.y } else { h.y }, if k & 4 == 0 { l.z } else { h.z }))).fold(0.0, f64::max);
                let near = (off((l + h) * 0.5) - 0.5 * (h - l).length()).max(0.0);
                (near - r, far - r)
            }
        }
    }
}

/// The distance at (u, v) and its gradient there.
fn value(s: &FreeSurface, f: &Implicit, q: DVec2) -> (f64, DVec2, [DVec3; 6]) {
    let dv = s.derivs(q.x, q.y);
    let g = f.grad(dv[0]);
    (f.at(dv[0]), DVec2::new(g.dot(dv[1]), g.dot(dv[2])), dv)
}

/// The root of the plane's function on the line from `a` to `b` (opposite signs at its ends): bisection with secant
/// steps.
fn root_on(s: &FreeSurface, f: &Implicit, a: DVec2, b: DVec2, fa: f64, fb: f64, eps: f64) -> DVec2 {
    let (mut lo, mut hi, mut flo, mut fhi) = (0.0f64, 1.0f64, fa, fb);
    for _ in 0..100 {
        let secant = lo + (hi - lo) * flo / (flo - fhi);
        let t = if secant > lo + 0.01 * (hi - lo) && secant < hi - 0.01 * (hi - lo) { secant } else { 0.5 * (lo + hi) };
        let ft = f.at(s.at(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t));
        if ft.abs() <= eps || hi - lo <= 1e-15 {
            return a + (b - a) * t;
        }
        if (ft < 0.0) == (flo < 0.0) {
            (lo, flo) = (t, ft);
        } else {
            (hi, fhi) = (t, ft);
        }
    }
    a + (b - a) * (0.5 * (lo + hi))
}

/// `q` moved onto the other surface along the distance's gradient (Newton), within the box.
fn onto(s: &FreeSurface, f: &Implicit, q: DVec2, uv: [f64; 4], eps: f64) -> Option<DVec2> {
    let mut q = q;
    for _ in 0..30 {
        let (v, g, _) = value(s, f, q);
        if v.abs() <= eps {
            return Some(q);
        }
        let gg = g.length_squared();
        if gg <= 1e-300 {
            return None;
        }
        q -= g * (v / gg);
        q = DVec2::new(q.x.clamp(uv[0], uv[1]), q.y.clamp(uv[2], uv[3]));
    }
    None
}

/// A line of the grid between two of its points: (i, j) and the way it goes (0: along u, 1: along v).
type Line = (usize, usize, u8);

/// Parameters along one direction: the surface's breaks within [lo, hi] and the ends, each span in `per` parts,
/// `min` parts at least over the range.
fn steps(breaks: &[f64], lo: f64, hi: f64, per: usize, min: usize) -> Vec<f64> {
    let mut knots: Vec<f64> = std::iter::once(lo).chain(breaks.iter().copied().filter(|&k| k > lo && k < hi)).chain(std::iter::once(hi)).collect();
    knots.dedup_by(|a, b| (*a - *b).abs() <= 1e-12 * (1.0 + a.abs()));
    let spans = knots.len() - 1;
    let per = per.max(min.div_ceil(spans.max(1)));
    let mut out = Vec::with_capacity(spans * per + 1);
    for w in knots.windows(2) {
        for k in 0..per {
            out.push(w[0] + (w[1] - w[0]) * k as f64 / per as f64);
        }
    }
    out.push(hi);
    out
}

/// Where surface `f` meets surface `s` over parameter box `uv` (u0, u1, v0, v1), only in cells whose control hull's
/// box `near` takes, the curves' runs whose points `within` takes: its curves, each within `tol` of both. None where
/// the grid can't tell.
pub(crate) fn sections(s: &FreeSurface, uv: [f64; 4], f: &Implicit, tol: f64, near: &dyn Fn((DVec3, DVec3)) -> bool, within: &dyn Fn(DVec2, DVec3) -> bool) -> Option<Vec<Marched>> {
    let why = |w: &str| { if std::env::var_os("X_MARCH").is_some() { eprintln!("XMARCH none: {w}"); } };
    if s.periodic().iter().any(|&p| p) {
        why("periodic");
        return None;
    }
    let us = steps(&s.breaks(true), uv[0], uv[1], 4, 32);
    let vs = steps(&s.breaks(false), uv[2], uv[3], 4, 32);
    let (nu, nv) = (us.len(), vs.len());
    if nu * nv > 1 << 18 {
        why("grid");
        return None;
    }
    // (The distance at the grid's points, each found when a cell near the other surface first asks.)
    let fs: Vec<std::cell::Cell<f64>> = vec![std::cell::Cell::new(f64::NAN); nu * nv];
    let at = |i: usize, j: usize| {
        let c = &fs[i * nv + j];
        if c.get().is_nan() {
            c.set(f.at(s.at(us[i], vs[j])));
        }
        c.get()
    };
    let eps = 1e-3 * tol;
    let range = |(l, h): (DVec3, DVec3)| f.range(l, h);
    // Crossings on the grid's lines: along u at (i, j)-(i+1, j) (index 0) and along v at (i, j)-(i, j+1) (index 1).
    let mut crossing: std::collections::HashMap<Line, DVec2> = std::collections::HashMap::new();
    let mut cells: Vec<((usize, usize), Vec<Line>)> = Vec::new();
    // (Cells whose hull reaches the other surface with no line of theirs crossed: probed after, unless next to a
    // crossed one.)
    let mut doubt: Vec<(usize, usize)> = Vec::new();
    for i in 0..nu - 1 {
        for j in 0..nv - 1 {
            let (u, v) = ((us[i], us[i + 1]), (vs[j], vs[j + 1]));
            let Some(b) = s.hull_box(u, v) else { why("hull"); return None };
            if !near(b) {
                continue;
            }
            let (a, z) = range(b);
            if a > eps || z < -eps {
                continue;
            }
            let sign = |x: f64| x >= 0.0;
            let lines = [(i, j, 0u8, at(i, j), at(i + 1, j)), (i, j + 1, 0, at(i, j + 1), at(i + 1, j + 1)), (i, j, 1, at(i, j), at(i, j + 1)), (i + 1, j, 1, at(i + 1, j), at(i + 1, j + 1))];
            let crossed: Vec<Line> = lines.iter().filter(|x| sign(x.3) != sign(x.4)).map(|x| (x.0, x.1, x.2)).collect();
            match crossed.len() {
                0 => doubt.push((i, j)),
                2 => {
                    for &(a0, b0, k, fa, fb) in lines.iter().filter(|x| sign(x.3) != sign(x.4)) {
                        crossing.entry((a0, b0, k)).or_insert_with(|| {
                            let p = DVec2::new(us[a0], vs[b0]);
                            let q = if k == 0 { DVec2::new(us[a0 + 1], vs[b0]) } else { DVec2::new(us[a0], vs[b0 + 1]) };
                            root_on(s, f, p, q, fa, fb, eps)
                        });
                    }
                    cells.push(((i, j), crossed));
                }
                n => { why(&format!("crossed {n}")); return None }
            }
        }
    }
    let crossed: std::collections::HashSet<(usize, usize)> = cells.iter().map(|c| c.0).collect();
    // (Not beside one: it touches where it comes within twice the tolerance at its corners or middle.)
    for (i, j) in doubt {
        let beside = (i.saturating_sub(1)..=i + 1).any(|a| (j.saturating_sub(1)..=j + 1).any(|b| crossed.contains(&(a, b))));
        if beside {
            continue;
        }
        let mid = f.at(s.at(0.5 * (us[i] + us[i + 1]), 0.5 * (vs[j] + vs[j + 1])));
        if [at(i, j), at(i + 1, j), at(i, j + 1), at(i + 1, j + 1), mid].iter().any(|x| x.abs() <= 2.0 * tol) || (mid >= 0.0) != (at(i, j) >= 0.0) {
            why("touches");
            return None;
        }
    }
    // Chains: each cell joins its two crossings; a crossing in one cell only is an end.
    let mut links: std::collections::HashMap<Line, Vec<Line>> = std::collections::HashMap::new();
    for (_, c) in &cells {
        links.entry(c[0]).or_default().push(c[1]);
        links.entry(c[1]).or_default().push(c[0]);
    }
    let mut keys: Vec<Line> = links.keys().copied().collect();
    keys.sort_unstable();
    let mut done: std::collections::HashSet<Line> = std::collections::HashSet::new();
    let mut chains: Vec<Vec<DVec2>> = Vec::new();
    let walk = |start: Line, done: &mut std::collections::HashSet<Line>| {
        let mut chain = vec![crossing[&start]];
        done.insert(start);
        let (mut prev, mut cur) = (start, start);
        while let Some(&next) = links[&cur].iter().find(|&&x| x != prev && !done.contains(&x)).or_else(|| links[&cur].iter().find(|&&x| x != prev && x == start && chain.len() > 2)) {
            if next == start {
                chain.push(crossing[&start]);
                break;
            }
            done.insert(next);
            chain.push(crossing[&next]);
            (prev, cur) = (cur, next);
        }
        chain
    };
    for &k in &keys {
        if links[&k].len() == 1 && !done.contains(&k) {
            chains.push(walk(k, &mut done));
        }
    }
    for &k in &keys {
        if !done.contains(&k) {
            chains.push(walk(k, &mut done));
        }
    }
    if std::env::var_os("X_CHAIN").is_some() {
        for c in &chains {
            let (a, b) = (c[0], c[c.len() - 1]);
            eprintln!("XCHAIN {} pts uv {a:?} -> {b:?} p {:?} -> {:?} box {uv:?} grid {nu}x{nv}", c.len(), s.at(a.x, a.y) * 1e3, s.at(b.x, b.y) * 1e3);
        }
    }
    // (Only their runs over the other face (`within`), each with the point before and after it.)
    let mut runs: Vec<Vec<DVec2>> = Vec::new();
    for mut c in chains {
        let mut inside: Vec<bool> = c.iter().map(|&q| within(q, s.at(q.x, q.y))).collect();
        // (A closed one from a point off the faces, so no run is cut where the loop happened to start.)
        let closed = c.len() > 2 && c[0] == c[c.len() - 1];
        if closed && let Some(k) = inside.iter().position(|&x| !x) {
            c.pop();
            inside.pop();
            c.rotate_left(k);
            inside.rotate_left(k);
            c.push(c[0]);
            inside.push(inside[0]);
        }
        let mut k = 0;
        while k < c.len() {
            if !inside[k] {
                k += 1;
                continue;
            }
            let start = k.saturating_sub(1);
            while k < c.len() && inside[k] {
                k += 1;
            }
            runs.push(c[start..(k + 1).min(c.len())].to_vec());
        }
    }
    let r: Option<Vec<Marched>> = runs.into_iter().filter(|c| c.len() >= 2).map(|c| fill(s, f, uv, c, tol, eps)).collect();
    if r.is_none() { why("fill"); } else if std::env::var_os("X_MARCH").is_some() { eprintln!("XMARCH ok {} curves grid {nu}x{nv}", r.as_ref().map_or(0, |x| x.len())); }
    r
}

/// A chain of points on the plane's section (parameters) as cubic pieces in parameters and in space, points added
/// where a piece's middle strays from the section by more than a tenth of `tol`.
fn fill(s: &FreeSurface, f: &Implicit, uv: [f64; 4], chain: Vec<DVec2>, tol: f64, eps: f64) -> Option<Marched> {
    // (A point's tangent along the section: across the function's gradient, in parameters and in space.)
    let tangent = |q: DVec2| -> Option<(DVec2, DVec3, DVec3)> {
        let (_, g, dv) = value(s, f, q);
        let t = DVec2::new(-g.y, g.x);
        let t3 = dv[1] * t.x + dv[2] * t.y;
        let l = t3.length();
        (l > 1e-300).then(|| (t / l, t3 / l, dv[0]))
    };
    let mut pts: Vec<(DVec2, DVec2, DVec3, DVec3)> = Vec::with_capacity(chain.len() * 2);
    for q in chain {
        let (t, t3, p) = tangent(q)?;
        pts.push((q, t, p, t3));
    }
    // (All tangents the chain's way.)
    for k in 0..pts.len() {
        let dir = if k + 1 < pts.len() { pts[k + 1].2 - pts[k].2 } else { pts[k].2 - pts[k - 1].2 };
        if pts[k].3.dot(dir) < 0.0 {
            pts[k].1 = -pts[k].1;
            pts[k].3 = -pts[k].3;
        }
    }
    let mut error = 0.0f64;
    let mut k = 0;
    let mut guard = 0;
    while k + 1 < pts.len() {
        guard += 1;
        if guard > 100_000 {
            return None;
        }
        let (a, b) = (pts[k], pts[k + 1]);
        let h = (b.2 - a.2).length();
        if h <= 1e-12 {
            pts.remove(k + 1);
            continue;
        }
        // (The pieces' middles: in parameters (tangents scaled to the parameters' rate along the length) and in
        // space.)
        let rate = |q: DVec2, t: DVec2| -> DVec2 {
            let dv = s.derivs(q.x, q.y);
            let l = (dv[1] * t.x + dv[2] * t.y).length();
            if l > 1e-300 { t / l } else { t }
        };
        let (ra, rb) = (rate(a.0, a.1), rate(b.0, b.1));
        let mid2 = (a.0 + b.0) * 0.5 + (ra - rb) * (h / 8.0);
        let mid3 = (a.2 + b.2) * 0.5 + (a.3 - b.3) * (h / 8.0);
        let on = s.at(mid2.x, mid2.y);
        let stray = f.at(on).abs().max((on - mid3).length()).max(f.at(mid3).abs());
        if stray > 0.1 * tol && h > 1e-9 {
            let q = onto(s, f, mid2, uv, eps)?;
            let (t, t3, p) = tangent(q)?;
            let (t, t3) = if t3.dot(b.2 - a.2) < 0.0 { (-t, -t3) } else { (t, t3) };
            pts.insert(k + 1, (q, t, p, t3));
            continue;
        }
        error = error.max(stray);
        k += 1;
    }
    if pts.len() < 2 {
        return None;
    }
    // (Thinned: from each point kept, the farthest one whose cubic piece passes every point between within a tenth
    // of the tolerance, in space and on the surface; the pieces' lengths the chords' sums.)
    let rate = |q: DVec2, tq: DVec2| -> DVec2 {
        let dv = s.derivs(q.x, q.y);
        let l = (dv[1] * tq.x + dv[2] * tq.y).length();
        if l > 1e-300 { tq / l } else { tq }
    };
    let rates: Vec<DVec2> = pts.iter().map(|p| rate(p.0, p.1)).collect();
    let mut along = vec![0.0f64; pts.len()];
    for k in 1..pts.len() {
        along[k] = along[k - 1] + (pts[k].2 - pts[k - 1].2).length();
    }
    let hermite = |p0: DVec3, m0: DVec3, p1: DVec3, m1: DVec3, h: f64, x: f64| {
        let (x2, x3) = (x * x, x * x * x);
        p0 * (2.0 * x3 - 3.0 * x2 + 1.0) + m0 * (h * (x3 - 2.0 * x2 + x)) + p1 * (3.0 * x2 - 2.0 * x3) + m1 * (h * (x3 - x2))
    };
    // (Against the dense pieces at their points and middles: those are within a tenth of the tolerance.)
    let fits = |i: usize, j: usize| {
        let h = along[j] - along[i];
        (i..j).all(|m| {
            let hd = along[m + 1] - along[m];
            [(along[m], pts[m].2, pts[m].0), (along[m] + 0.5 * hd, hermite(pts[m].2, pts[m].3, pts[m + 1].2, pts[m + 1].3, hd, 0.5), hermite(pts[m].0.extend(0.0), rates[m].extend(0.0), pts[m + 1].0.extend(0.0), rates[m + 1].extend(0.0), hd, 0.5).truncate())]
                .iter()
                .all(|&(l, want, want_uv)| {
                    let x = (l - along[i]) / h;
                    let p = hermite(pts[i].2, pts[i].3, pts[j].2, pts[j].3, h, x);
                    let q = hermite(pts[i].0.extend(0.0), rates[i].extend(0.0), pts[j].0.extend(0.0), rates[j].extend(0.0), h, x);
                    (p - want).length() <= 0.1 * tol && (s.at(q.x, q.y) - s.at(want_uv.x, want_uv.y)).length() <= 0.1 * tol
                })
        })
    };
    let mut keep = vec![0usize];
    let mut i = 0;
    while i + 1 < pts.len() {
        let mut j = i + 1;
        while j + 1 < pts.len() && std::env::var_os("X_NOTHIN").is_none() && fits(i, j + 1) {
            j += 1;
        }
        keep.push(j);
        i = j;
    }
    let dense = (pts.clone(), along.clone());
    let pts: Vec<(DVec2, DVec2, DVec3, DVec3)> = keep.iter().map(|&k| pts[k]).collect();
    let lens: Vec<f64> = keep.windows(2).map(|w| along[w[1]] - along[w[0]]).collect();
    if std::env::var_os("X_MARCH").is_some() {
        eprintln!("XMARCH pieces {} error {error:e}", pts.len() - 1);
    }
    // (One smooth spline (C2) through the kept points, else through all of them, in space and on the free-form face
    // with the same parameters: OCCT's 2D intersections in the result's check take cubic pieces one span at a time.)
    let check = |m: &Marched, n: usize| {
        let mut stray = 0.0f64;
        for k in 0..=n {
            let x = m.t1 * k as f64 / n as f64;
            let (p, q) = (m.curve.at(x), m.uv.at(x));
            stray = stray.max(f.at(p).abs()).max((s.at(q.x, q.y) - p).length());
        }
        stray
    };
    for (pp, al) in [(pts.clone(), keep.iter().map(|&k| along[k]).collect::<Vec<f64>>()), dense] {
        if std::env::var_os("X_NOSPLINE").is_some() {
            break;
        }
        let n = pp.len() - 1;
        let ts: Vec<f64> = al.iter().map(|x| x - al[0]).collect();
        let (Some((knots, poles3)), Some((_, poles2))) = (
            spline(&ts, &pp.iter().map(|p| p.2).collect::<Vec<_>>(), pp[0].3, pp[n].3),
            spline(&ts, &pp.iter().map(|p| p.0.extend(0.0)).collect::<Vec<_>>(), rate(pp[0].0, pp[0].1).extend(0.0), rate(pp[n].0, pp[n].1).extend(0.0)),
        ) else {
            continue;
        };
        let m = Marched {
            uv: NurbsCurve { degree: 3, knots: knots.clone(), poles: poles2, weights: Vec::new(), spans: Default::default() },
            curve: NurbsCurve { degree: 3, knots, poles: poles3, weights: Vec::new(), spans: Default::default() },
            t1: ts[n],
            error,
        };
        let stray = check(&m, 16 * n.max(1));
        if stray <= 0.1 * tol {
            return Some(Marched { error: error.max(stray), ..m });
        }
    }
    // Else Bézier pieces with triple knots, the parameter the length along the chords.
    let mut knots = vec![0.0; 4];
    let mut poles2 = vec![pts[0].0.extend(0.0)];
    let mut poles3 = vec![pts[0].2];
    let mut t = 0.0;
    for (w, &h) in pts.windows(2).zip(&lens) {
        let (a, b) = (w[0], w[1]);
        let (ra, rb) = (rate(a.0, a.1), rate(b.0, b.1));
        poles2.push((a.0 + ra * (h / 3.0)).extend(0.0));
        poles2.push((b.0 - rb * (h / 3.0)).extend(0.0));
        poles2.push(b.0.extend(0.0));
        poles3.push(a.2 + a.3 * (h / 3.0));
        poles3.push(b.2 - b.3 * (h / 3.0));
        poles3.push(b.2);
        t += h;
        knots.extend([t; 3]);
    }
    let last = knots.len() - 1;
    knots.push(knots[last]);
    let m = Marched {
        uv: NurbsCurve { degree: 3, knots: knots.clone(), poles: poles2, weights: Vec::new(), spans: Default::default() },
        curve: NurbsCurve { degree: 3, knots, poles: poles3, weights: Vec::new(), spans: Default::default() },
        t1: t,
        error,
    };
    // (Checked over every piece: its points on both surfaces and on its curve on the free-form one.)
    let mut stray = 0.0f64;
    let mut a = 0.0;
    for &h in &lens {
        for k in 1..8 {
            let x = a + h * k as f64 / 8.0;
            let (p, q) = (m.curve.at(x), m.uv.at(x));
            let (e1, e2) = (f.at(p).abs(), (s.at(q.x, q.y) - p).length());
            if std::env::var_os("X_MARCH2").is_some() && e1.max(e2) > tol {
                eprintln!("XSTRAY piece len {h:e} at {k}/8: off other {e1:e} off surface {e2:e} p {p} q {q} S(q) {}", s.at(q.x, q.y));
            }
            stray = stray.max(e1).max(e2);
        }
        a += h;
    }
    if std::env::var_os("X_MARCH").is_some() {
        eprintln!("XMARCH checked stray {stray:e} pieces {}", lens.len());
    }
    (stray <= tol).then_some(Marched { error: m.error.max(stray), ..m })
}

/// The cubic B-spline through `pts` at parameters `ts` (increasing, from 0) with derivatives `d0`, `dn` at its ends
/// (C2 inside: one knot a point): its knots and poles. None for fewer than two points or a parameter not increasing.
fn spline(ts: &[f64], pts: &[DVec3], d0: DVec3, dn: DVec3) -> Option<(Vec<f64>, Vec<DVec3>)> {
    let n = pts.len().checked_sub(1)?;
    if n == 0 || ts.windows(2).any(|w| w[1] <= w[0]) {
        return None;
    }
    let mut knots = vec![ts[0]; 4];
    knots.extend(&ts[1..n]);
    knots.extend([ts[n]; 4]);
    let mut poles = vec![DVec3::ZERO; n + 3];
    poles[0] = pts[0];
    poles[1] = pts[0] + d0 * ((ts[1] - ts[0]) / 3.0);
    poles[n + 2] = pts[n];
    poles[n + 1] = pts[n] - dn * ((ts[n] - ts[n - 1]) / 3.0);
    if n == 1 {
        return Some((knots, poles));
    }
    // (At an inner knot t_i the basis of poles i, i+1, i+2; the unknown poles 2..=n by Thomas's algorithm.)
    let basis = |k: usize, u: f64| -> [f64; 4] {
        let mut nb = [0.0; 4];
        nb[0] = 1.0;
        let (mut left, mut right) = ([0.0; 4], [0.0; 4]);
        for j in 1..=3 {
            left[j] = u - knots[k + 1 - j];
            right[j] = knots[k + j] - u;
            let mut saved = 0.0;
            for r in 0..j {
                let tmp = nb[r] / (right[r + 1] + left[j - r]);
                nb[r] = saved + right[r + 1] * tmp;
                saved = left[j - r] * tmp;
            }
            nb[j] = saved;
        }
        nb
    };
    let m = n - 1;
    let (mut a, mut b, mut c, mut r) = (vec![0.0; m], vec![0.0; m], vec![0.0; m], vec![DVec3::ZERO; m]);
    for i in 1..n {
        // (Knot t_i is knots[i + 3]; its span starts there: poles i..=i+3, the last one's basis 0.)
        let nb = basis(i + 3, ts[i]);
        let row = i - 1;
        a[row] = nb[0];
        b[row] = nb[1];
        c[row] = nb[2];
        r[row] = pts[i];
    }
    r[0] -= poles[1] * a[0];
    r[m - 1] -= poles[n + 1] * c[m - 1];
    for i in 1..m {
        let w = a[i] / b[i - 1];
        b[i] -= w * c[i - 1];
        let rp = r[i - 1];
        r[i] -= rp * w;
    }
    if b.iter().any(|x| x.abs() <= 1e-300) {
        return None;
    }
    let mut x = vec![DVec3::ZERO; m];
    x[m - 1] = r[m - 1] / b[m - 1];
    for i in (0..m - 1).rev() {
        x[i] = (r[i] - x[i + 1] * c[i]) / b[i];
    }
    poles[2..(m + 2)].copy_from_slice(&x);
    Some((knots, poles))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A quarter circle of radius 1 (rational) swept 1 along z, cut by the plane x = 0.6: one straight section at
    /// y = 0.8 from z 0 to 1.
    #[test]
    fn a_plane_across_a_swept_arc() {
        let w = std::f64::consts::FRAC_1_SQRT_2;
        let c = NurbsCurve { degree: 2, knots: vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0], poles: vec![DVec3::X, DVec3::new(1.0, 1.0, 0.0), DVec3::Y], weights: vec![1.0, w, 1.0], spans: Default::default() };
        let s = FreeSurface::Extrusion { c, d: DVec3::Z };
        let secs = sections(&s, [0.0, 1.0, 0.0, 1.0], &Implicit::Plane { n: DVec3::X, d: 0.6 }, 1e-7, &|_| true, &|_, _| true).expect("marched");
        assert_eq!(secs.len(), 1);
        let m = &secs[0];
        let (a, b) = (m.curve.at(0.0), m.curve.at(m.t1));
        let (lo, hi) = if a.z < b.z { (a, b) } else { (b, a) };
        assert!((lo - DVec3::new(0.6, 0.8, 0.0)).length() < 1e-9 && (hi - DVec3::new(0.6, 0.8, 1.0)).length() < 1e-9, "{lo} {hi}");
        for k in 0..=20 {
            let t = m.t1 * k as f64 / 20.0;
            let p = m.curve.at(t);
            let q = m.uv.at(t);
            assert!((p.x - 0.6).abs() < 1e-7 && (p.y - 0.8).abs() < 1e-7, "{p}");
            assert!((s.at(q.x, q.y) - p).length() < 1e-7);
        }
    }

    /// The spline through points of a circle's arc, its ends' tangents given: on the circle within 1e-4 of its
    /// radius over 32 points, through each point exactly.
    #[test]
    fn a_spline_through_an_arc() {
        let n = 8;
        let ts: Vec<f64> = (0..=n).map(|k| k as f64 / n as f64 * std::f64::consts::FRAC_PI_2).collect();
        let pts: Vec<DVec3> = ts.iter().map(|&t| DVec3::new(t.cos(), t.sin(), 0.0)).collect();
        let (knots, poles) = spline(&ts, &pts, DVec3::Y, -DVec3::X).expect("a spline");
        let c = NurbsCurve { degree: 3, knots, poles, weights: Vec::new(), spans: Default::default() };
        for (t, p) in ts.iter().zip(&pts) {
            assert!((c.at(*t) - *p).length() < 1e-12);
        }
        for k in 0..=32 {
            let t = std::f64::consts::FRAC_PI_2 * k as f64 / 32.0;
            assert!((c.at(t).length() - 1.0).abs() < 1e-4);
        }
    }
}
