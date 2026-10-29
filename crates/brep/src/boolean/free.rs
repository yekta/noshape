//! Free-form (NURBS) faces in the Boolean: their parameters scaled to lengths, curves on them, points' parameters,
//! rays across them.

use super::*;
use crate::curve2::Map;
use crate::freeform::FreeSurface;

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
/// lengths in its parameter plane are about lengths on it, as the tolerances want).
pub(super) fn nurbs_surf(ns: &FreeSurface, uv: [f64; 4], reversed: bool) -> Surf {
    let (mut su, mut sv) = (0.0, 0.0);
    for i in 0..=4 {
        for j in 0..=4 {
            let d = ns.derivs(uv[0] + (uv[1] - uv[0]) * i as f64 / 4.0, uv[2] + (uv[3] - uv[2]) * j as f64 / 4.0);
            su += d[1].length();
            sv += d[2].length();
        }
    }
    Surf::Nurbs { su: (su / 25.0).max(1e-12), sv: (sv / 25.0).max(1e-12), flip: reversed }
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
}

/// A free-form face's patches over its box (each span between the surface's breaks in 4, at least 8 a side),
/// boxes from a 5 × 5 grid of points grown by their chords' sag.
pub(super) fn patches(ns: &FreeSurface, uv: [f64; 4]) -> Vec<Patch> {
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
            out.push(Patch { lo: lo - grow, hi: hi + grow, uv: [ua, ub, va, vb] });
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
        let mut uv = DVec2::new(0.5 * (ua + ub), 0.5 * (va + vb));
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
