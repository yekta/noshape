//! Free-form (NURBS) faces in the Boolean: their parameters scaled to lengths, curves on them, points' parameters,
//! rays across them.

use super::*;
use crate::curve2::Map;
use crate::freeform::FreeSurface;
use glam::DVec4;

/// A free-form face's parameter box: what its edges' curves on it span (within the surface's own range), grown by
/// a hundredth.
pub(super) fn uv_box(ns: Option<&FreeSurface>, pcurves: &PCurves) -> [f64; 4] {
    let Some(ns) = ns else { return [0.0; 4] };
    let (mut lo, mut hi) = (DVec2::splat(f64::INFINITY), DVec2::splat(f64::NEG_INFINITY));
    for (c, t0, t1) in pcurves.values() {
        for k in 0..=32 {
            let q = c.at(t0 + (t1 - t0) * k as f64 / 32.0).truncate();
            lo = lo.min(q);
            hi = hi.max(q);
        }
    }
    let grow = (hi - lo) * 0.01 + DVec2::splat(1e-9);
    let (mut lo, mut hi) = (lo - grow, hi + grow);
    for (k, r) in ns.range().into_iter().enumerate() {
        if let Some((a, b)) = r {
            lo[k] = lo[k].max(a);
            hi[k] = hi[k].min(b);
        }
    }
    [lo.x, hi.x, lo.y, hi.y]
}

/// A free-form face's `Surf`: its parameters scaled by the surface's mean speeds along u and v over its box (so
/// lengths in its parameter plane are about lengths on it, as the tolerances want), v the other way on a reversed
/// face.
pub(super) fn nurbs_surf(ns: &FreeSurface, uv: [f64; 4], reversed: bool) -> Surf {
    let (mut su, mut sv) = (0.0, 0.0);
    for i in 0..=4 {
        for j in 0..=4 {
            let d = ns.derivs(uv[0] + (uv[1] - uv[0]) * i as f64 / 4.0, uv[2] + (uv[3] - uv[2]) * j as f64 / 4.0);
            su += d[1].length();
            sv += d[2].length();
        }
    }
    let sv = (sv / 25.0).max(1e-12);
    Surf::Nurbs { su: (su / 25.0).max(1e-12), sv: if reversed { -sv } else { sv }, flip: reversed }
}

/// The values splitting `lo..hi` into pieces: the surface's own breaks there, each gap in `n` parts.
fn splits(ns: &FreeSurface, along_u: bool, lo: f64, hi: f64, n: usize) -> Vec<f64> {
    let mut v: Vec<f64> = vec![lo];
    v.extend(ns.breaks(along_u).into_iter().filter(|&k| k > lo && k < hi));
    v.push(hi);
    v.dedup();
    let mut out = Vec::new();
    for w in v.windows(2) {
        for k in 0..n {
            out.push(w[0] + (w[1] - w[0]) * k as f64 / n as f64);
        }
    }
    out.push(hi);
    out
}

/// Points of a free-form face's surface on a grid over its box (4 a span each way, at least 16), with their
/// parameters: seeds for finding a point's parameters.
pub(super) fn samples(ns: &FreeSurface, uv: [f64; 4]) -> Vec<(DVec2, DVec3)> {
    let (us, vs) = (splits(ns, true, uv[0], uv[1], 4), splits(ns, false, uv[2], uv[3], 4));
    let fill = |s: Vec<f64>| if s.len() < 17 { (0..=16).map(|k| s[0] + (s[s.len() - 1] - s[0]) * k as f64 / 16.0).collect() } else { s };
    let (us, vs) = (fill(us), fill(vs));
    let mut out = Vec::with_capacity(us.len() * vs.len());
    for &u in &us {
        for &v in &vs {
            out.push((DVec2::new(u, v), ns.at(u, v)));
        }
    }
    out
}

/// A NURBS curve's length from `t0` to `t1` (Gauss–Legendre, 4 points on each of 4 pieces a knot span).
pub(super) fn length(c: &NurbsCurve, t0: f64, t1: f64) -> f64 {
    const G: [(f64, f64); 4] = [
        (0.069_431_844_202_973_71, 0.173_927_422_568_726_9),
        (0.330_009_478_207_571_9, 0.326_072_577_431_273_1),
        (0.669_990_521_792_428_1, 0.326_072_577_431_273_1),
        (0.930_568_155_797_026_3, 0.173_927_422_568_726_9),
    ];
    let n = 4 * (c.poles.len() - c.degree).max(1);
    let h = (t1 - t0) / n as f64;
    (0..n).map(|k| G.iter().map(|&(x, w)| w * c.derivs(t0 + h * (k as f64 + x))[1].length()).sum::<f64>() * h.abs()).sum()
}

/// `c` with its x and y scaled by `su` and `sv` (a curve on a NURBS face, into the face's scaled parameters).
pub(super) fn scaled(c: &NurbsCurve, su: f64, sv: f64) -> Arc<NurbsCurve> {
    let mut c = c.clone();
    for p in &mut c.poles {
        *p = DVec3::new(p.x * su, p.y * sv, 0.0);
    }
    Arc::new(c)
}

/// A part of a NURBS surface (a sixteenth of a knot span patch) and a box around it.
pub(super) struct Patch {
    lo: DVec3,
    hi: DVec3,
    uv: [f64; 4],
    /// A Bézier piece's (its box its control points'): Newton from a 3 × 3 grid of starts in it.
    piece: bool,
}

/// A free-form face's patches over its box (each span between the surface's breaks in 4, at least 8 a side),
/// boxes from a 5 × 5 grid of points grown by their chords' sag.
pub(super) fn patches(g: &GFace) -> Vec<Patch> {
    let (ns, uv) = (g.ns.as_deref().expect("a free-form face"), g.uvbox);
    // (A Bézier surface's pieces: their control points' boxes hold them.)
    if let Some(hs) = g.hulls.get_or_init(|| hulls(ns)).as_deref() {
        let grow = DVec3::splat(1e-9);
        return hs.iter().filter(|h| h.uv[1] >= uv[0] && h.uv[0] <= uv[1] && h.uv[3] >= uv[2] && h.uv[2] <= uv[3]).map(|h| Patch { lo: h.lo - grow, hi: h.hi + grow, uv: h.uv, piece: true }).collect();
    }
    let (mut us, mut vs) = (splits(ns, true, uv[0], uv[1], 4), splits(ns, false, uv[2], uv[3], 4));
    for s in [&mut us, &mut vs] {
        if s.len() < 9 {
            *s = (0..=8).map(|k| s[0] + (s[s.len() - 1] - s[0]) * k as f64 / 8.0).collect();
        }
    }
    let mut out = Vec::new();
    for w in us.windows(2) {
        for z in vs.windows(2) {
            let (ua, ub, va, vb) = (w[0], w[1], z[0], z[1]);
            let (mut lo, mut hi) = (DVec3::splat(f64::INFINITY), DVec3::splat(f64::NEG_INFINITY));
            for i in 0..=4 {
                for j in 0..=4 {
                    let q = ns.at(ua + (ub - ua) * i as f64 / 4.0, va + (vb - va) * j as f64 / 4.0);
                    lo = lo.min(q);
                    hi = hi.max(q);
                }
            }
            let grow = DVec3::splat(0.25 * (hi - lo).length() + 1e-9);
            out.push(Patch { lo: lo - grow, hi: hi + grow, uv: [ua, ub, va, vb], piece: false });
        }
    }
    out
}

/// Where the ray `p + t r` crosses a NURBS surface: from the middle of each of its patches whose box the ray
/// passes, Newton's method on (u, v, t); each hit's (u, v) and t (distinct ones).
pub(super) fn ray_hits(ns: &FreeSurface, patches: &[Patch], uv: [f64; 4], p: DVec3, r: DVec3) -> Vec<(DVec2, f64)> {
    let mut out: Vec<(DVec2, f64)> = Vec::new();
    let [u0, u1, v0, v1] = uv;
    for pa in patches {
        if !ray_meets_box(p, r, pa.lo, pa.hi) {
            continue;
        }
        let [ua, ub, va, vb] = pa.uv;
        let starts: &[(f64, f64)] = if pa.piece { &[(0.5, 0.5), (0.1, 0.1), (0.5, 0.1), (0.9, 0.1), (0.1, 0.5), (0.9, 0.5), (0.1, 0.9), (0.5, 0.9), (0.9, 0.9)] } else { &[(0.5, 0.5)] };
        for &(fu, fv) in starts {
            let mut uv = DVec2::new(ua + (ub - ua) * fu, va + (vb - va) * fv);
            let mut t = (ns.at(uv.x, uv.y) - p).dot(r);
            let mut ok = false;
            for _ in 0..40 {
                let d = ns.derivs(uv.x, uv.y);
                // F = S(u, v) - p - t r = 0: [Su Sv -r] (du dv dt) = -F.
                let f = d[0] - p - r * t;
                let m = glam::DMat3::from_cols(d[1], d[2], -r);
                if m.determinant().abs() < 1e-300 {
                    break;
                }
                let x = m.inverse() * -f;
                uv += DVec2::new(x.x, x.y);
                t += x.z;
                if x.length() < 1e-15 * (1.0 + uv.length() + t.abs()) {
                    ok = true;
                    break;
                }
            }
            // (A piece's start: where the steps stall at rounding, there if it is on the ray.)
            if pa.piece && !ok {
                ok = (ns.at(uv.x, uv.y) - p - r * t).length() <= 1e-12;
            }
            if !ok || uv.x < u0 - 1e-12 || uv.x > u1 + 1e-12 || uv.y < v0 - 1e-12 || uv.y > v1 + 1e-12 {
                continue;
            }
            if (ns.at(uv.x, uv.y) - p - r * t).length() > 1e-9 {
                continue;
            }
            if !out.iter().any(|h| (h.1 - t).abs() < 1e-9) {
                out.push((uv, t));
            }
        }
    }
    out
}

/// Whether the ray `p + t r` (t >= 0) passes the box.
pub(super) fn ray_meets(p: DVec3, r: DVec3, lo: DVec3, hi: DVec3) -> bool {
    ray_meets_box(p, r, lo, hi)
}

fn ray_meets_box(p: DVec3, r: DVec3, lo: DVec3, hi: DVec3) -> bool {
    let (mut t0, mut t1) = (f64::NEG_INFINITY, f64::INFINITY);
    for i in 0..3 {
        if r[i].abs() < 1e-300 {
            if p[i] < lo[i] || p[i] > hi[i] {
                return false;
            }
            continue;
        }
        let (a, b) = ((lo[i] - p[i]) / r[i], (hi[i] - p[i]) / r[i]);
        t0 = t0.max(a.min(b));
        t1 = t1.min(a.max(b));
    }
    t0 <= t1 && t1 >= 0.0
}

/// A 3D NURBS curve's part from `t0` to `t1` in a plane's, cylinder's or cone's parameter plane (`copy` 1: a
/// seam's right side).
pub(super) fn mapped(g: &GFace, c: &Arc<NurbsCurve>, t0: f64, t1: f64) -> R<C2> {
    let m = match g.s {
        Surf::Plane { origin, u, v, .. } => Map::Plane { origin, u, v },
        Surf::Cyl { o, a, x, y, r, flip, th0, .. } => {
            let mid = g.to2(c.at(0.5 * (t0 + t1))).x;
            Map::Cyl { o, a, x, y, r, flip, th0, mid }
        }
        Surf::Nurbs { .. } => return Err(Decline("a curve on a free-form face")),
    };
    Ok(C2::Mapped { c: c.clone(), t0, t1, m })
}

/// A box around a NURBS curve's part from `t0` to `t1`: 16 samples a span, grown by their chords' sag.
pub(super) fn piece_box(c: &NurbsCurve, t0: f64, t1: f64) -> (DVec3, DVec3) {
    let n = 16 * (c.poles.len() - c.degree).max(1);
    let (mut lo, mut hi) = (DVec3::splat(f64::INFINITY), DVec3::splat(f64::NEG_INFINITY));
    let mut step = 0.0_f64;
    let mut prev = c.at(t0);
    for k in 0..=n {
        let p = c.at(t0 + (t1 - t0) * k as f64 / n as f64);
        lo = lo.min(p);
        hi = hi.max(p);
        step = step.max((p - prev).length());
        prev = p;
    }
    (lo - DVec3::splat(0.25 * step), hi + DVec3::splat(0.25 * step))
}

/// A box around a free-form face: its surface over its parameter box (a grid of points there, grown by a tenth of
/// its size).
pub(super) fn face_box(g: &GFace) -> Option<(DVec3, DVec3)> {
    let ns = g.ns.as_ref()?;
    let [u0, u1, v0, v1] = g.uvbox;
    let (mut lo, mut hi) = (DVec3::splat(f64::INFINITY), DVec3::splat(f64::NEG_INFINITY));
    for i in 0..=8 {
        for j in 0..=8 {
            let p = ns.at(u0 + (u1 - u0) * i as f64 / 8.0, v0 + (v1 - v0) * j as f64 / 8.0);
            lo = lo.min(p);
            hi = hi.max(p);
        }
    }
    let grow = DVec3::splat(0.1 * (hi - lo).length());
    Some((lo - grow, hi + grow))
}

/// Where a 3D curve crosses the angle `th0` of the cylinders and cones among `faces` (a full one's seam, a part's
/// gap): its parameters, in order (its parameter plane image jumps there).
pub(super) fn seam_cuts(cx: &Ctx, faces: [usize; 2], c: &NurbsCurve, t0: f64, t1: f64) -> Vec<f64> {
    let mut out = Vec::new();
    for f in faces {
        let Some(&Surf::Cyl { o, x, y, th0, .. }) = cx.faces.get(f).map(|g| &g.s) else { continue };
        // The angle from th0 in (-π, π]: a crossing where it changes sign without passing π.
        let ang = |t: f64| {
            let w = c.at(t) - o;
            let d = w.dot(y).atan2(w.dot(x)) - th0;
            (d + PI).rem_euclid(TAU) - PI
        };
        let n = 32 * (c.poles.len() - c.degree).max(1);
        let mut prev = (t0, ang(t0));
        for k in 1..=n {
            let t = t0 + (t1 - t0) * k as f64 / n as f64;
            let cur = (t, ang(t));
            if (prev.1 < 0.0) != (cur.1 < 0.0) && (prev.1 - cur.1).abs() < PI {
                let (mut lo, mut hi) = (prev.0, cur.0);
                for _ in 0..80 {
                    let mid = 0.5 * (lo + hi);
                    if (ang(mid) < 0.0) == (prev.1 < 0.0) {
                        lo = mid;
                    } else {
                        hi = mid;
                    }
                }
                out.push(0.5 * (lo + hi));
            }
            prev = cur;
        }
    }
    out.sort_by(f64::total_cmp);
    out
}

/// The distance from `p` to face `f`'s surface.
pub(super) fn surface_distance(g: &GFace, p: DVec3) -> f64 {
    match g.s {
        Surf::Plane { n, d, .. } => (n.dot(p) - d).abs(),
        Surf::Cyl { o, a, r0, k, .. } => {
            let w = p - o;
            let h = w.dot(a);
            ((w - a * h).length() - (r0 + k * h)).abs()
        }
        Surf::Nurbs { .. } => g.ns.as_ref().map_or(f64::INFINITY, |ns| {
            let uv = g.uv_of(p);
            (ns.at(uv.x, uv.y) - p).length()
        }),
    }
}

/// A straight or round edge lying on free-form face `g` (its point and derivative at `t` from `at`): its curve on the
/// face, in the face's own parameters and the edge's parameter, `t0` to `t1`: its points' parameters there (Newton
/// from the one before), cubic between them with the surface's own tangents. None where a point is off the face's
/// surface by more than `tol` or the curve jumps (across the face's seam).
pub(super) fn projected(g: &GFace, at: &dyn Fn(f64) -> (DVec3, DVec3), t0: f64, t1: f64, tol: f64) -> Option<Arc<NurbsCurve>> {
    let ns = g.ns.as_ref()?;
    const N: usize = 64;
    let [u0, u1, v0, v1] = g.uvbox;
    let mut qs: Vec<(DVec2, DVec2)> = Vec::with_capacity(N + 1);
    for k in 0..=N {
        let t = t0 + (t1 - t0) * k as f64 / N as f64;
        let (p, dp) = at(t);
        let seed = qs.last().map_or_else(|| g.uv_of(p), |q| q.0);
        let (uv, dist) = ns.param_of(p, seed, g.uvbox);
        if dist > tol {
            return None;
        }
        if let Some(q) = qs.last()
            && ((uv.x - q.0.x).abs() > 0.25 * (u1 - u0) || (uv.y - q.0.y).abs() > 0.25 * (v1 - v0))
        {
            return None;
        }
        // Its tangent there: [Su Sv] (u', v') = P' (least squares).
        let d = ns.derivs(uv.x, uv.y);
        let (a, b, c) = (d[1].dot(d[1]), d[1].dot(d[2]), d[2].dot(d[2]));
        let (e, f) = (d[1].dot(dp), d[2].dot(dp));
        let det = a * c - b * b;
        if det.abs() < 1e-300 {
            return None;
        }
        qs.push((uv, DVec2::new((c * e - b * f) / det, (a * f - b * e) / det)));
    }
    // Bézier pieces between the points (a B-spline with triple knots).
    let h = (t1 - t0) / N as f64;
    let mut knots = vec![t0; 4];
    let mut poles = vec![qs[0].0.extend(0.0)];
    for k in 0..N {
        let (q, dq) = qs[k];
        let (r, dr) = qs[k + 1];
        poles.push((q + dq * (h / 3.0)).extend(0.0));
        poles.push((r - dr * (h / 3.0)).extend(0.0));
        poles.push(r.extend(0.0));
        let t = t0 + h * (k + 1) as f64;
        knots.extend(if k + 1 == N { [t1; 4].to_vec() } else { vec![t; 3] });
    }
    Some(Arc::new(NurbsCurve { degree: 3, knots, poles, weights: Vec::new(), spans: Default::default() }))
}

/// A piece of a Bézier surface: its control points (the piece lies in their hull), their box, and its directions
/// across and along (corner to corner).
pub(super) struct Hull {
    pts: Vec<DVec3>,
    lo: DVec3,
    hi: DVec3,
    du: DVec3,
    dv: DVec3,
    /// Its parameter range (u0, u1, v0, v1).
    uv: [f64; 4],
}

/// The pieces of a face's NURBS surface of one span each way (a Bézier patch): cut by de Casteljau (homogeneous)
/// into 32 along v and 4 along u (one along a straight u); None for other surfaces.
fn hulls(ns: &FreeSurface) -> Option<Vec<Hull>> {
    let FreeSurface::Nurbs(s) = ns else { return None };
    let (p, q) = (s.udegree, s.vdegree);
    if s.nu != p + 1 || s.nv != q + 1 || s.uknots.len() != 2 * (p + 1) || s.vknots.len() != 2 * (q + 1) || s.weights.iter().any(|&w| w <= 0.0) {
        return None;
    }
    let w = |k: usize| if s.weights.is_empty() { 1.0 } else { s.weights[k] };
    // Homogeneous rows (each a curve along v), then each cut along v.
    let rows: Vec<Vec<DVec4>> = (0..=p).map(|i| (0..=q).map(|j| (s.poles[i * s.nv + j] * w(i * s.nv + j)).extend(w(i * s.nv + j))).collect()).collect();
    // A Bézier curve's piece from parameter a to b (of [0, 1]): cut at b, then the left part at a / b.
    let piece = |c: &[DVec4], a: f64, b: f64| -> Vec<DVec4> {
        let cut = |c: &[DVec4], t: f64, left: bool| -> Vec<DVec4> {
            let mut c = c.to_vec();
            let n = c.len();
            let mut out = vec![DVec4::ZERO; n];
            for r in 0..n {
                out[if left { r } else { n - 1 - r }] = if left { c[0] } else { c[n - 1 - r] };
                for k in 0..n - 1 - r {
                    c[k] = c[k] * (1.0 - t) + c[k + 1] * t;
                }
            }
            out
        };
        let l = cut(c, b, true);
        if a <= 0.0 { l } else { cut(&l, a / b, false) }
    };
    let (nu, nv) = (if p == 1 { 1 } else { 4 }, 32);
    let ((ra, rb), (sa, sb)) = s.range();
    let mut out = Vec::with_capacity(nu * nv);
    for kv in 0..nv {
        let (va, vb) = (kv as f64 / nv as f64, (kv + 1) as f64 / nv as f64);
        let cols: Vec<Vec<DVec4>> = rows.iter().map(|r| piece(r, va, vb)).collect();
        for ku in 0..nu {
            let (ua, ub) = (ku as f64 / nu as f64, (ku + 1) as f64 / nu as f64);
            // Along u: each column (j fixed) cut too.
            let mut grid: Vec<Vec<DVec3>> = Vec::with_capacity(q + 1);
            for j in 0..=q {
                let col: Vec<DVec4> = cols.iter().map(|c| c[j]).collect();
                grid.push(piece(&col, ua, ub).iter().map(|h| h.truncate() / h.w).collect());
            }
            let pts: Vec<DVec3> = grid.iter().flatten().copied().collect();
            let (lo, hi) = pts.iter().fold((DVec3::INFINITY, DVec3::NEG_INFINITY), |(lo, hi), &x| (lo.min(x), hi.max(x)));
            let du = grid[q / 2][p] - grid[q / 2][0];
            let dv = grid[q][p / 2] - grid[0][p / 2];
            let uv = [ra + (rb - ra) * ua, ra + (rb - ra) * ub, sa + (sb - sa) * va, sa + (sb - sa) * vb];
            out.push(Hull { pts, lo, hi, du, dv, uv });
        }
    }
    Some(out)
}

/// Whether two pieces are farther apart than `gap`: on one of a few axes (the coordinate axes, either piece's
/// normal, the cross products of their directions) their control points' extents are.
fn hulls_apart(a: &Hull, b: &Hull, gap: f64) -> bool {
    if (a.lo - b.hi).max_element() > gap || (b.lo - a.hi).max_element() > gap {
        return true;
    }
    for ax in [a.du.cross(a.dv), b.du.cross(b.dv), a.dv.cross(b.dv), a.du.cross(b.dv), a.dv.cross(b.du), a.du.cross(b.du)] {
        let l = ax.length();
        if l < 1e-300 {
            continue;
        }
        let ax = ax / l;
        let ext = |h: &Hull| h.pts.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), x| (lo.min(x.dot(ax)), hi.max(x.dot(ax))));
        let ((alo, ahi), (blo, bhi)) = (ext(a), ext(b));
        if alo - bhi > gap || blo - ahi > gap {
            return true;
        }
    }
    false
}

/// Whether two faces, one or both free-form, are farther apart than `gap`, as their surfaces' control points tell
/// (a Bézier surface against another or a plane; else false).
pub(super) fn apart(f: &GFace, g: &GFace, gap: f64) -> bool {
    fn pieces(x: &GFace) -> Option<&[Hull]> {
        x.hulls.get_or_init(|| x.ns.as_deref().and_then(hulls)).as_deref()
    }
    match (pieces(f), pieces(g)) {
        (Some(a), Some(b)) => a.iter().all(|x| b.iter().all(|y| hulls_apart(x, y, gap))),
        (Some(a), None) | (None, Some(a)) => {
            let other = if pieces(f).is_some() { g } else { f };
            match other.s {
                Surf::Plane { n, d, .. } => {
                    let side = |x: &DVec3| n.dot(*x) - d;
                    a.iter().all(|h| h.pts.iter().all(|x| side(x) > gap)) || a.iter().all(|h| h.pts.iter().all(|x| side(x) < -gap))
                }
                _ => false,
            }
        }
        (None, None) => false,
    }
}

/// Whether `p` is farther than `gap` from face `g`'s Bézier surface (outside all its pieces' boxes grown by it).
pub(super) fn clearly_off(g: &GFace, p: DVec3, gap: f64) -> bool {
    let Some(ns) = g.ns.as_deref() else { return false };
    g.hulls.get_or_init(|| hulls(ns)).as_deref().is_some_and(|hs| hs.iter().all(|h| (h.lo - p).max_element() > gap || (p - h.hi).max_element() > gap))
}
