//! Free-form (NURBS) faces in the Boolean: their parameters scaled to lengths, curves on them, points' parameters,
//! rays across them.

use super::*;
use crate::curve2::Map;
use crate::freeform::FreeSurface;
use glam::DVec4;

/// A free-form face's parameter box: what its edges' curves on it span (within the surface's own range), grown by
/// a hundredth.
/// (None where its curves lie outside the surface's own range (OCCT's curves on a periodic surface a period on): the
/// box would be inside out.)
/// How far [`uv_box`] reaches past a face's curves, of their span each way.
const UV_PAD: f64 = 0.01;

/// A face's parameter range along u (`i` 0) or v (1) as its curves span it: [`uv_box`]'s without its margin (where no
/// natural range cut it).
fn uv_span(g: &GFace, i: usize) -> (f64, f64) {
    let (lo, hi) = (g.uvbox[2 * i], g.uvbox[2 * i + 1]);
    let pad = (hi - lo - 2e-9) / (1.0 + 2.0 * UV_PAD) * UV_PAD + 1e-9;
    (lo + pad, hi - pad)
}

pub(super) fn uv_box(ns: Option<&FreeSurface>, pcurves: &PCurves) -> Option<[f64; 4]> {
    let Some(ns) = ns else { return Some([0.0; 4]) };
    let (mut lo, mut hi) = (DVec2::splat(f64::INFINITY), DVec2::splat(f64::NEG_INFINITY));
    for (c, t0, t1) in pcurves.values() {
        for k in 0..=32 {
            let q = c.at(t0 + (t1 - t0) * k as f64 / 32.0).truncate();
            lo = lo.min(q);
            hi = hi.max(q);
        }
    }
    let grow = (hi - lo) * UV_PAD + DVec2::splat(1e-9);
    let (mut lo, mut hi) = (lo - grow, hi + grow);
    for (k, r) in ns.range().into_iter().enumerate() {
        if let Some((a, b)) = r {
            lo[k] = lo[k].max(a);
            hi[k] = hi[k].min(b);
        }
    }
    (lo.x <= hi.x && lo.y <= hi.y && lo.is_finite() && hi.is_finite()).then_some([lo.x, hi.x, lo.y, hi.y])
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

/// A NURBS curve's length from `t0` to `t1` (Gauss–Legendre, 4 points on each of 4 pieces a knot span it is on).
pub(super) fn length(c: &NurbsCurve, t0: f64, t1: f64) -> f64 {
    const G: [(f64, f64); 4] = [
        (0.069_431_844_202_973_71, 0.173_927_422_568_726_9),
        (0.330_009_478_207_571_9, 0.326_072_577_431_273_1),
        (0.669_990_521_792_428_1, 0.326_072_577_431_273_1),
        (0.930_568_155_797_026_3, 0.173_927_422_568_726_9),
    ];
    let n = 4 * spans_over(c, t0, t1);
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
    /// A Bézier piece's control points (holding it); none for another ([`patch_points`]).
    pts: Vec<DVec3>,
}

/// A free-form face's patches and, for each run of [`GROUP`] of them in order, a box holding theirs.
pub(super) struct Patches {
    all: Vec<Patch>,
    groups: Vec<(DVec3, DVec3)>,
}

const GROUP: usize = 16;

/// [`patches`] with their runs' boxes.
pub(super) fn grouped(g: &GFace) -> Patches {
    let all = patches(g);
    let groups = all.chunks(GROUP).map(|c| c.iter().fold((DVec3::INFINITY, DVec3::NEG_INFINITY), |(l, h), x| (l.min(x.lo), h.max(x.hi)))).collect();
    Patches { all, groups }
}

/// A free-form face's patches over its box (each span between the surface's breaks in 4, at least 8 a side),
/// boxes from a 5 × 5 grid of points grown by their chords' sag.
pub(super) fn patches(g: &GFace) -> Vec<Patch> {
    let (ns, uv) = (g.ns.as_deref().expect("a free-form face"), g.uvbox);
    // (A Bézier surface's pieces: their control points' boxes hold them.)
    if let Some(hs) = g.hulls.get_or_init(|| hulls(ns)).as_deref() {
        let grow = DVec3::splat(1e-9);
        return hs.iter().filter(|h| h.uv[1] >= uv[0] && h.uv[0] <= uv[1] && h.uv[3] >= uv[2] && h.uv[2] <= uv[3]).map(|h| Patch { lo: h.lo - grow, hi: h.hi + grow, uv: h.uv, piece: true, pts: h.pts.clone() }).collect();
    }
    let (mut us, mut vs) = (splits(ns, true, uv[0], uv[1], 4), splits(ns, false, uv[2], uv[3], 4));
    for s in [&mut us, &mut vs] {
        if s.len() < 9 {
            *s = (0..=8).map(|k| s[0] + (s[s.len() - 1] - s[0]) * k as f64 / 8.0).collect();
        }
    }
    // (Each patch's 5 × 5 points, at its own parameters (its last row needn't be the next one's first), for its
    // box; [`patch_points`] makes them again where a ray needs them.)
    let fifths = |s: &[f64]| s.windows(2).flat_map(|w| (0..=4).map(move |i| w[0] + (w[1] - w[0]) * i as f64 / 4.0)).collect::<Vec<f64>>();
    let vl = fifths(&vs);
    let mut out = Vec::with_capacity((us.len() - 1) * (vs.len() - 1));
    for w in us.windows(2) {
        let ul: Vec<f64> = (0..=4).map(|i| w[0] + (w[1] - w[0]) * i as f64 / 4.0).collect();
        let grid = ns.grid(&ul, &vl);
        for (zi, z) in vs.windows(2).enumerate() {
            let (mut lo, mut hi) = (DVec3::splat(f64::INFINITY), DVec3::splat(f64::NEG_INFINITY));
            for row in grid.chunks(vl.len()) {
                for &q in &row[zi * 5..zi * 5 + 5] {
                    lo = lo.min(q);
                    hi = hi.max(q);
                }
            }
            let grow = DVec3::splat(0.25 * (hi - lo).length() + 1e-9);
            out.push(Patch { lo: lo - grow, hi: hi + grow, uv: [w[0], w[1], z[0], z[1]], piece: false, pts: Vec::new() });
        }
    }
    out
}

/// A patch's (not a Bézier piece's) 5 × 5 grid of points, row by row in u.
fn patch_points(ns: &FreeSurface, pa: &Patch) -> Vec<DVec3> {
    let [ua, ub, va, vb] = pa.uv;
    let (ul, vl): (Vec<f64>, Vec<f64>) = (0..=4).map(|i| (ua + (ub - ua) * i as f64 / 4.0, va + (vb - va) * i as f64 / 4.0)).unzip();
    let grid = ns.grid(&ul, &vl);
    (0..25).map(|k| grid[(k % 5) * 5 + k / 5]).collect()
}

/// Where the ray `p + t r` crosses a NURBS surface: from the middle of each of its patches whose box the ray
/// passes, Newton's method on (u, v, t); each hit's (u, v) and t (distinct ones).
pub(super) fn ray_hits(ns: &FreeSurface, patches: &Patches, uv: [f64; 4], p: DVec3, r: DVec3) -> Vec<(DVec2, f64)> {
    let mut out: Vec<(DVec2, f64)> = Vec::new();
    let [u0, u1, v0, v1] = uv;
    // (A run whose box the ray misses: so does each of its patches'.)
    let met = patches.groups.iter().enumerate().filter(|x| ray_meets_box(p, r, x.1.0, x.1.1)).flat_map(|(k, _)| &patches.all[k * GROUP..((k + 1) * GROUP).min(patches.all.len())]);
    for pa in met {
        if !ray_meets_box(p, r, pa.lo, pa.hi) {
            continue;
        }
        // (A piece the ray passes clear of, across it, or that lies behind the point: not met.)
        if pa.piece {
            let (a, b) = r.any_orthonormal_pair();
            let ext = |ax: DVec3| pa.pts.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), q| (lo.min(q.dot(ax)), hi.max(q.dot(ax))));
            let ((alo, ahi), (blo, bhi), (_, thi)) = (ext(a), ext(b), ext(r));
            let m = 1e-9;
            if p.dot(a) < alo - m || p.dot(a) > ahi + m || p.dot(b) < blo - m || p.dot(b) > bhi + m || thi < p.dot(r) - m {
                continue;
            }
        }
        let [ua, ub, va, vb] = pa.uv;
        // (Newton's from a start: a crossing within the face, on the ray.)
        let newton = |mut uv: DVec2, leash: bool| -> Option<(DVec2, f64)> {
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
                // (A piece's start gone a piece away: the crossing, if any, is another piece's.)
                if leash && (uv.x < 2.0 * ua - ub || uv.x > 2.0 * ub - ua || uv.y < 2.0 * va - vb || uv.y > 2.0 * vb - va) {
                    break;
                }
            }
            // (Where the steps stall at rounding, there if it is on the ray: a 3 mm letter's wall over u 0 .. 1 moves u
            // by its rounding, 3e-14 a step; the ray crossed it unseen, the letter's counter taken as inside it.)
            if !ok {
                ok = (ns.at(uv.x, uv.y) - p - r * t).length() <= 1e-12;
            }
            if !ok || uv.x < u0 - 1e-12 || uv.x > u1 + 1e-12 || uv.y < v0 - 1e-12 || uv.y > v1 + 1e-12 {
                return None;
            }
            ((ns.at(uv.x, uv.y) - p - r * t).length() <= 1e-9).then_some((uv, t))
        };
        let mut add = |h: (DVec2, f64)| {
            if !out.iter().any(|x| (x.1 - h.1).abs() < 1e-9) {
                out.push(h);
            }
        };
        let at = |fu: f64, fv: f64| DVec2::new(ua + (ub - ua) * fu, va + (vb - va) * fv);
        // (On a piece the ray may cross twice (a twisted helical knurl flank's): every start.)
        if pa.piece {
            for (fu, fv) in [(0.5, 0.5), (0.1, 0.1), (0.5, 0.1), (0.9, 0.1), (0.1, 0.5), (0.9, 0.5), (0.1, 0.9), (0.5, 0.9), (0.9, 0.9)] {
                if let Some(h) = newton(at(fu, fv), true) {
                    add(h);
                }
            }
            continue;
        }
        let m = 1e-9 * (1.0 + (ub - ua).abs() + (vb - va).abs());
        let inside = |uv: DVec2| uv.x >= ua - m && uv.x <= ub + m && uv.y >= va - m && uv.y <= vb + m;
        if let Some(h) = newton(at(0.5, 0.5), false) {
            add(h);
            continue;
        }
        // (The middle's steps gone off the face (a long helical flank's eighth, the crossing at its corner): again
        // from the middle of each cell of its grid the ray passes ahead, seen along the ray (the cell's box grown by a
        // tenth of its size), the steps kept near the patch; a cell whose middle finds nothing, the ray within its
        // corners' box, again from the corner nearest the ray (a crossing in a flank's cell's corner, at the knurl's
        // end).)
        let (ax, bx) = r.any_orthonormal_pair();
        let seen = |q: DVec3| DVec3::new((q - p).dot(ax), (q - p).dot(bx), (q - p).dot(r));
        let pts = patch_points(ns, pa);
        for k in 0..16 {
            let (i, j) = (k % 4, k / 4);
            let c = [pts[j * 5 + i], pts[j * 5 + i + 1], pts[j * 5 + i + 5], pts[j * 5 + i + 6]].map(seen);
            let (lo, hi) = c.iter().fold((DVec3::INFINITY, DVec3::NEG_INFINITY), |(lo, hi), &x| (lo.min(x), hi.max(x)));
            let grow = 0.1 * (hi - lo).max_element();
            if lo.x - grow > 0.0 || lo.y - grow > 0.0 || hi.x + grow < 0.0 || hi.y + grow < 0.0 || hi.z + grow < 0.0 {
                continue;
            }
            let (f0, g0) = (i as f64 / 4.0, j as f64 / 4.0);
            let mut h = newton(at(f0 + 0.125, g0 + 0.125), true);
            let through = lo.x <= 0.0 && lo.y <= 0.0 && hi.x >= 0.0 && hi.y >= 0.0 && hi.z >= 0.0;
            if h.is_none() && through {
                let (n, _) = c.iter().enumerate().map(|(n, x)| (n, x.truncate().length())).fold((0, f64::INFINITY), |a, b| if b.1 < a.1 { b } else { a });
                h = newton(at(f0 + 0.25 * (n % 2) as f64, g0 + 0.25 * (n / 2) as f64), true);
            }
            if let Some(h) = h {
                add(h);
                if inside(h.0) {
                    break;
                }
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
            Map::Cyl { o, a, x, y, r, flip, th0, mid, eps: g.seam_eps() }
        }
        Surf::Nurbs { .. } => return Err(Decline("a curve on a free-form face")),
    };
    Ok(C2::Mapped { c: c.clone(), t0, t1, m })
}

/// How many of a NURBS curve's knot spans its part from `t0` to `t1` is on (all where that part reaches past its
/// knots).
fn spans_over(c: &NurbsCurve, t0: f64, t1: f64) -> usize {
    let all = (c.poles.len() - c.degree).max(1);
    let (lo, hi) = (t0.min(t1), t0.max(t1));
    let (Some(&k0), Some(&k1)) = (c.knots.first(), c.knots.last()) else { return all };
    if lo < k0 || hi > k1 || c.spans().is_empty() {
        return all;
    }
    (c.span_of(hi) - c.span_of(lo) + 1).min(all)
}

/// A box around a NURBS curve's part from `t0` to `t1`: 16 samples a span it is on, grown by their chords' sag.
pub(super) fn piece_box(c: &NurbsCurve, t0: f64, t1: f64) -> (DVec3, DVec3) {
    let n = 16 * spans_over(c, t0, t1);
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
    // (Within its control points' hull: a 124 mm ring 0.1 mm high grew 12 mm up, past another solid it never meets.)
    let Some((hl, hh)) = ns.hull_box((u0, u1), (v0, v1)) else { return Some((lo - grow, hi + grow)) };
    Some(((lo - grow).max(hl), (hi + grow).min(hh)))
}

/// Where a 3D curve crosses the angle `th0` of the cylinders and cones among `faces` (a full one's seam, a part's
/// gap): its parameters, in order (its parameter plane image jumps there).
pub(super) fn seam_cuts(cx: &Ctx, faces: [usize; 2], c: &NurbsCurve, t0: f64, t1: f64) -> Vec<f64> {
    let mut out = Vec::new();
    for f in faces {
        let Some(&Surf::Cyl { o, x, y, th0, .. }) = cx.faces.get(f).map(|g| &g.s) else { continue };
        if off_seam(c, t0, t1, o, y * th0.cos() - x * th0.sin(), x * th0.cos() + y * th0.sin()) {
            continue;
        }
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

/// Whether a NURBS curve's part from `t0` to `t1` keeps off the half-plane from an axis through `o` along `dir`
/// (`n`: its plane's normal): its Bézier pieces' control points, whose hull holds it, all on one side of that plane
/// or all behind the axis.
fn off_seam(c: &NurbsCurve, t0: f64, t1: f64, o: DVec3, n: DVec3, dir: DVec3) -> bool {
    let spans = c.spans();
    let (lo, hi) = (t0.min(t1), t0.max(t1));
    let (Some(&k0), Some(&k1)) = (c.knots.first(), c.knots.last()) else { return false };
    if spans.is_empty() || lo < k0 || hi > k1 {
        return false;
    }
    let (mut n_lo, mut n_hi, mut d_hi, mut size) = (f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY, 0.0_f64);
    for q in spans[c.span_of(lo)..=c.span_of(hi)].iter().flat_map(|s| s.2.iter()) {
        if q.w <= 0.0 {
            return false;
        }
        let w = q.truncate() / q.w - o;
        let h = w.dot(n);
        (n_lo, n_hi, d_hi, size) = (n_lo.min(h), n_hi.max(h), d_hi.max(w.dot(dir)), size.max(w.abs().max_element()));
    }
    let m = 1e-9 * size;
    n_lo > m || n_hi < -m || d_hi < -m
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
        let duv = DVec2::new((c * e - b * f) / det, (a * f - b * e) / det);
        // (Starting on the seam of a face going a whole turn round and heading out of it there: from the seam's other
        // side, a boss's round edge on a revolved face.)
        let mut uv = uv;
        if k == 0 {
            for (i, round) in ns.periodic().into_iter().enumerate() {
                let (lo, hi) = uv_span(g, i);
                if !round || (hi - lo - TAU).abs() > 1e-6 {
                    continue;
                }
                if duv[i] < 0.0 && (uv[i] - lo).abs() < 1e-6 {
                    uv[i] = hi;
                } else if duv[i] > 0.0 && (uv[i] - hi).abs() < 1e-6 {
                    uv[i] = lo;
                }
            }
        }
        qs.push((uv, duv));
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

/// A Bézier patch of a NURBS surface: its homogeneous control points by row (along u) then column, its degrees, its
/// parameter range (u0, u1, v0, v1).
pub(super) struct Bez {
    h: Vec<DVec4>,
    p: usize,
    q: usize,
    uv: [f64; 4],
}

impl Bez {
    fn pt(&self, i: usize, j: usize) -> DVec3 {
        let h = self.h[i * (self.q + 1) + j];
        h.truncate() / h.w
    }

    fn hull(&self) -> Hull {
        let pts: Vec<DVec3> = self.h.iter().map(|h| h.truncate() / h.w).collect();
        let (lo, hi) = pts.iter().fold((DVec3::INFINITY, DVec3::NEG_INFINITY), |(lo, hi), &x| (lo.min(x), hi.max(x)));
        let (du, dv) = (self.pt(self.p, self.q / 2) - self.pt(0, self.q / 2), self.pt(self.p / 2, self.q) - self.pt(self.p / 2, 0));
        Hull { pts, lo, hi, du, dv, uv: [0.0; 4] }
    }

    /// Whether its control net is longer along u than along v (its longest row against its longest column: a patch
    /// closing on itself has ends that meet).
    fn longer_along_u(&self) -> bool {
        let row = |j: usize| (0..self.p).map(|i| (self.pt(i + 1, j) - self.pt(i, j)).length()).sum::<f64>();
        let col = |i: usize| (0..self.q).map(|j| (self.pt(i, j + 1) - self.pt(i, j)).length()).sum::<f64>();
        (0..=self.q).map(row).fold(0.0, f64::max) >= (0..=self.p).map(col).fold(0.0, f64::max)
    }

    /// Its halves along u or v (de Casteljau at the middle).
    fn halves(&self, along_u: bool) -> (Bez, Bez) {
        let (n, m) = if along_u { (self.p + 1, self.q + 1) } else { (self.q + 1, self.p + 1) };
        let (mut l, mut r) = (self.h.clone(), self.h.clone());
        for k in 0..m {
            let idx = |t: usize| if along_u { t * (self.q + 1) + k } else { k * (self.q + 1) + t };
            let mut c: Vec<DVec4> = (0..n).map(|t| self.h[idx(t)]).collect();
            for s in 0..n {
                l[idx(s)] = c[0];
                r[idx(n - 1 - s)] = c[n - 1 - s];
                for t in 0..n - 1 - s {
                    c[t] = (c[t] + c[t + 1]) * 0.5;
                }
            }
        }
        let [u0, u1, v0, v1] = self.uv;
        let (lu, ru) = if along_u { ([u0, 0.5 * (u0 + u1), v0, v1], [0.5 * (u0 + u1), u1, v0, v1]) } else { ([u0, u1, v0, 0.5 * (v0 + v1)], [u0, u1, 0.5 * (v0 + v1), v1]) };
        (Bez { h: l, uv: lu, ..*self }, Bez { h: r, uv: ru, ..*self })
    }
}

/// A NURBS surface's Bézier patches (its spans each way, by knot insertion), an extrusion's over `uv`'s heights; None
/// where a weight isn't positive or its knots aren't clamped.
fn bezier_patches(ns: &FreeSurface, uv: [f64; 4]) -> Option<Vec<Bez>> {
    let ext;
    let s = match ns {
        FreeSurface::Nurbs(s) => s,
        FreeSurface::Extrusion { c, d } => {
            let poles = c.poles.iter().flat_map(|&x| [x + *d * uv[2], x + *d * uv[3]]).collect();
            let weights = c.weights.iter().flat_map(|&w| [w, w]).collect();
            ext = crate::nurbs::NurbsSurface { udegree: c.degree, vdegree: 1, uknots: c.knots.clone(), vknots: vec![uv[2], uv[2], uv[3], uv[3]], nu: c.poles.len(), nv: 2, poles, weights };
            &ext
        }
        _ => return None,
    };
    let (p, q) = (s.udegree, s.vdegree);
    if s.weights.iter().any(|&w| w <= 0.0) || s.poles.len() != s.nu * s.nv {
        return None;
    }
    let w = |k: usize| if s.weights.is_empty() { 1.0 } else { s.weights[k] };
    let curve = |degree: usize, knots: &[f64], h: &[DVec4]| NurbsCurve { degree, knots: knots.to_vec(), poles: h.iter().map(|x| x.truncate() / x.w).collect(), weights: h.iter().map(|x| x.w).collect(), spans: Default::default() };
    // Along u: each column (v index fixed) cut into its spans.
    let cols: Vec<NurbsCurve> = (0..s.nv).map(|j| curve(p, &s.uknots, &(0..s.nu).map(|i| (s.poles[i * s.nv + j] * w(i * s.nv + j)).extend(w(i * s.nv + j))).collect::<Vec<_>>())).collect();
    let nu = cols[0].spans().len();
    if nu == 0 || cols.iter().any(|c| c.spans().len() != nu) {
        return None;
    }
    let mut out = Vec::new();
    for ku in 0..nu {
        // Then each of that span's rows (u index fixed) along v.
        let rows: Vec<NurbsCurve> = (0..=p).map(|i| curve(q, &s.vknots, &cols.iter().map(|c| c.spans()[ku].2[i]).collect::<Vec<_>>())).collect();
        let nv = rows[0].spans().len();
        if nv == 0 || rows.iter().any(|r| r.spans().len() != nv) {
            return None;
        }
        let (u0, u1) = (cols[0].spans()[ku].0, cols[0].spans()[ku].1);
        for kv in 0..nv {
            let (v0, v1) = (rows[0].spans()[kv].0, rows[0].spans()[kv].1);
            out.push(Bez { h: rows.iter().flat_map(|r| r.spans()[kv].2.iter().copied()).collect(), p, q, uv: [u0, u1, v0, v1] });
        }
    }
    Some(out)
}

/// Whether two Bézier patches are farther apart than `gap` by their control points' hulls, the larger one halved
/// where they aren't, while `budget` (hull pairs) lasts and 64 halvings deep (a patch closing on itself, its ends'
/// chord zero, can be halved the way that never shrinks it: CRASH-6297's stack overflow).
fn bez_apart(a: &Bez, b: &Bez, gap: f64, budget: &mut usize, depth: u32) -> bool {
    if *budget == 0 || depth >= 64 {
        return false;
    }
    *budget -= 1;
    let (ha, hb) = (a.hull(), b.hull());
    if hulls_apart(&ha, &hb, gap) {
        return true;
    }
    let size = |h: &Hull| (h.hi - h.lo).length();
    let (big, hbig, small) = if size(&ha) >= size(&hb) { (a, &ha, b) } else { (b, &hb, a) };
    if size(hbig) <= gap {
        return false;
    }
    let (l, r) = big.halves(hbig.du.length() >= hbig.dv.length());
    bez_apart(&l, small, gap, budget, depth + 1) && bez_apart(&r, small, gap, budget, depth + 1)
}

/// Whether two faces, one or both free-form, are farther apart than `gap`, as their surfaces' control points tell
/// (a Bézier surface against another or a plane; NURBS surfaces of several spans by their patches, halved while
/// they meet (neighbouring helical gear flanks); else false). `flat`: each face's box in its parameter plane where
/// it is a plane or cylinder face and has one ([`Ctx::face_box`]).
pub(super) fn apart(f: &GFace, g: &GFace, gap: f64, flat: [Option<(DVec2, DVec2)>; 2]) -> bool {
    fn pieces(x: &GFace) -> Option<&[Hull]> {
        x.hulls.get_or_init(|| x.ns.as_deref().and_then(hulls)).as_deref()
    }
    fn patches(x: &GFace) -> Option<&[Bez]> {
        x.bez.get_or_init(|| bezier_patches(x.ns.as_deref()?, pole_box(x)?)).as_deref()
    }
    // (A free-form face and a plane or cylinder face: the patches the face's curves on it reach, each clear of the
    // other face's box or of its surface, halved while neither (c2-rings-loft's 51 cutters against the ring's faces:
    // 900 empty sections, 15 s of OCCT's).)
    for (x, y, yb) in [(f, g, flat[1]), (g, f, flat[0])] {
        if matches!(y.s, Surf::Plane { .. } | Surf::Cyl { k: 0.0, .. })
            && let Some(a) = patches(x)
            && off_face(a, x, y, yb, gap)
        {
            return true;
        }
    }
    if (pieces(f).is_none() || pieces(g).is_none())
        && let (Some(a), Some(b)) = (patches(f), patches(g))
    {
        let mut budget = 20_000;
        return a.iter().all(|x| b.iter().all(|y| bez_apart(x, y, gap, &mut budget, 0)));
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

/// The box (u0, u1, v0, v1) of a free-form face's curves on it by their control points (holding them), None where a
/// weight isn't positive.
fn pole_box(f: &GFace) -> Option<[f64; 4]> {
    let (mut lo, mut hi) = (DVec2::splat(f64::INFINITY), DVec2::splat(f64::NEG_INFINITY));
    for (c, _, _) in f.pcurves.values() {
        if c.weights.iter().any(|&w| w <= 0.0) {
            return None;
        }
        for x in &c.poles {
            lo = lo.min(x.truncate());
            hi = hi.max(x.truncate());
        }
    }
    (lo.is_finite() && hi.is_finite()).then_some([lo.x, hi.x, lo.y, hi.y])
}

/// Whether free-form face `f` (its Bézier patches `a`) is farther than `gap` from plane or cylinder face `g`: each
/// patch within `f`'s curves' box on it outside `g`'s box grown by `gap` or clear of its surface ([`off_surface`]),
/// halved while neither (64 halvings in all).
fn off_face(a: &[Bez], f: &GFace, g: &GFace, flat: Option<(DVec2, DVec2)>, gap: f64) -> bool {
    let Some(uv) = pole_box(f) else { return false };
    let reaches = |b: &Bez| b.uv[1] >= uv[0] && b.uv[0] <= uv[1] && b.uv[3] >= uv[2] && b.uv[2] <= uv[3];
    let (glo, ghi) = (g.lo - DVec3::splat(gap), g.hi + DVec3::splat(gap));
    let mut todo: Vec<Bez> = a.iter().filter(|b| reaches(b)).map(|b| Bez { h: b.h.clone(), ..*b }).collect();
    let mut budget = 64usize;
    while let Some(b) = todo.pop() {
        let h = b.hull();
        if !reaches(&b) || (h.lo - ghi).max_element() > 0.0 || (glo - h.hi).max_element() > 0.0 || off_surface(&h.pts, g, flat, gap) {
            continue;
        }
        if budget == 0 {
            return false;
        }
        budget -= 1;
        let (l, r) = b.halves(b.longer_along_u());
        todo.extend([l, r]);
    }
    true
}

/// Whether the convex hull of `pts` is farther than `gap` from plane or cylinder face `g` (`flat` its box in its
/// parameter plane): all on one side of its plane or outside that box; inside its cylinder, beyond a line along it,
/// outside its heights or outside the wedge of its angles (less than half a turn).
fn off_surface(pts: &[DVec3], g: &GFace, flat: Option<(DVec2, DVec2)>, gap: f64) -> bool {
    let extent = |f: &dyn Fn(DVec3) -> f64| pts.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(l, h), &x| (l.min(f(x)), h.max(f(x))));
    let outside = |(lo, hi): (f64, f64), (a, b): (f64, f64)| lo > b + gap || hi < a - gap;
    match g.s {
        Surf::Plane { n, d, origin, u, v, .. } => {
            let (lo, hi) = extent(&|x| n.dot(x) - d);
            if lo > gap || hi < -gap {
                return true;
            }
            let Some((flo, fhi)) = flat else { return false };
            outside(extent(&|x| (x - origin).dot(u)), (flo.x, fhi.x)) || outside(extent(&|x| (x - origin).dot(v)), (flo.y, fhi.y))
        }
        Surf::Cyl { o, a, x, y, r, r0, k: 0.0, flip, full, .. } => {
            let across = |p: DVec3| {
                let w = p - o;
                w - a * w.dot(a)
            };
            if pts.iter().all(|&p| across(p).length() < r0 - gap) {
                return true;
            }
            let c = pts.iter().map(|&p| across(p)).sum::<DVec3>() / pts.len().max(1) as f64;
            if c.length() > 0.0 && extent(&|p| across(p).dot(c.normalize())).0 > r0 + gap {
                return true;
            }
            let Some((flo, fhi)) = flat else { return false };
            let s = if flip { -1.0 } else { 1.0 };
            if outside(extent(&|p| s * (p - o).dot(a)), (flo.y, fhi.y)) {
                return true;
            }
            let (t0, t1) = (flo.x / r, fhi.x / r);
            if full || t1 - t0 >= 3.0 {
                return false;
            }
            let (n0, n1) = (x * t0.sin() - y * t0.cos(), y * t1.cos() - x * t1.sin());
            extent(&|p| across(p).dot(n0)).0 > gap || extent(&|p| across(p).dot(n1)).0 > gap
        }
        _ => false,
    }
}

/// Whether `p` is farther than `gap` from face `g`'s Bézier surface (outside all its pieces' boxes grown by it).
pub(super) fn clearly_off(g: &GFace, p: DVec3, gap: f64) -> bool {
    let Some(ns) = g.ns.as_deref() else { return false };
    g.hulls.get_or_init(|| hulls(ns)).as_deref().is_some_and(|hs| hs.iter().all(|h| (h.lo - p).max_element() > gap || (p - h.hi).max_element() > gap))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two wavy strips of three spans each, one the other's mirror (their control points ±0.1 off a plane): apart by
    /// their Bézier patches, halved while they meet, when 0.3 apart; not where they cross.
    #[test]
    fn b_spline_strips_apart_by_their_patches() {
        let strip = |wave: f64, dz: f64| {
            let poles = (0..5).flat_map(|i| (0..2).map(move |j| DVec3::new(i as f64, j as f64, if i % 2 == 0 { wave } else { -wave } + dz))).collect();
            FreeSurface::Nurbs(crate::nurbs::NurbsSurface { udegree: 2, vdegree: 1, uknots: vec![0.0, 0.0, 0.0, 1.0, 2.0, 3.0, 3.0, 3.0], vknots: vec![0.0, 0.0, 1.0, 1.0], nu: 5, nv: 2, poles, weights: Vec::new() })
        };
        let a = bezier_patches(&strip(0.1, 0.0), [0.0; 4]).expect("its patches");
        assert_eq!(a.len(), 3);
        let apart = |dz: f64| {
            let b = bezier_patches(&strip(-0.1, dz), [0.0; 4]).expect("its patches");
            let mut budget = 20_000;
            a.iter().all(|x| b.iter().all(|y| bez_apart(x, y, 1e-3, &mut budget, 0)))
        };
        assert!(apart(0.3));
        assert!(!apart(0.0));
    }

    /// A patch closing on itself along v (a ring's loft: its ends' chord along v is zero) is halved along v, its
    /// longer control net, not along its short u forever (c2-rings-loft's 51-tool cut).
    #[test]
    fn a_closed_patch_halves_along_its_longer_net() {
        let ring = [DVec3::X, DVec3::Y, -DVec3::X, -DVec3::Y, DVec3::X];
        let h = (0..2).flat_map(|i| ring.iter().map(move |&x| (x + DVec3::Z * (0.1 * i as f64)).extend(1.0))).collect();
        let b = Bez { h, p: 1, q: 4, uv: [0.0, 1.0, 0.0, 1.0] };
        let hull = b.hull();
        assert!(hull.du.length() > hull.dv.length());
        assert!(!b.longer_along_u());
    }

    /// Points clear of a cylinder face (axis z, radius 1, heights 0 to 1, angles 0 to 1 rad) by being inside it,
    /// beyond a line along it, above it or outside its angles' wedge; not where they lie on it.
    #[test]
    fn hulls_off_a_cylinder_face() {
        let s = Surf::Cyl { o: DVec3::ZERO, a: DVec3::Z, x: DVec3::X, y: DVec3::Y, r: 1.0, r0: 1.0, k: 0.0, flip: false, th0: 0.0, full: false };
        let g = GFace { arg: 0, index: 0, t: 0.0, solid: 0, s, edges: Vec::new(), loop_lens: Vec::new(), bound: Default::default(), bound_spans: Default::default(), bound_pts: Default::default(), right: Vec::new(), wrap_eps: 0.0, lo: DVec3::new(0.0, 0.0, 0.0), hi: DVec3::new(1.0, 1.0, 1.0), ns: None, uvbox: [0.0; 4], pcurves: HashMap::default(), grid: Default::default(), patches: Default::default(), hulls: Default::default(), bez: Default::default(), inner: Vec::new() };
        let flat = Some((DVec2::new(0.0, 0.0), DVec2::new(1.0, 1.0)));
        let near = |r: f64, th: f64, z: f64| [-0.01, 0.01].into_iter().flat_map(|d| [DVec3::new(r * (th + d).cos(), r * (th + d).sin(), z + d), DVec3::new((r + d) * th.cos(), (r + d) * th.sin(), z)]).collect::<Vec<_>>();
        let off = |pts: Vec<DVec3>| off_surface(&pts, &g, flat, 0.005);
        assert!(!off(near(1.0, 0.5, 0.5)));
        assert!(off(near(0.5, 0.5, 0.5)));
        assert!(off(near(2.0, 0.5, 0.5)));
        assert!(off(near(1.0, 0.5, 2.0)));
        assert!(off(near(1.0, 2.0, 0.5)));
        assert!(off(near(1.0, -0.5, 0.5)));
    }

    /// A patch closing on itself (a ring, its ends' chord along v zero) against a patch touching it: halved along u
    /// it never shrinks, so the answer (not apart) comes from the depth bound, on a small stack (CRASH-6297: 2,770
    /// frames deep on a rayon worker).
    #[test]
    fn a_ring_patch_against_a_touching_one_stays_shallow() {
        let ring = [DVec3::X, DVec3::Y, -DVec3::X, -DVec3::Y, DVec3::X];
        let h = (0..2).flat_map(|i| ring.iter().map(move |&x| (x + DVec3::Z * (0.1 * i as f64)).extend(1.0))).collect();
        let a = Bez { h, p: 1, q: 4, uv: [0.0, 1.0, 0.0, 1.0] };
        let b = Bez { h: [DVec3::new(0.5, 0.0, 0.0), DVec3::new(0.5, 0.0, 0.1), DVec3::new(0.6, 0.0, 0.0), DVec3::new(0.6, 0.0, 0.1)].iter().map(|x| x.extend(1.0)).collect(), p: 1, q: 1, uv: [0.0, 1.0, 0.0, 1.0] };
        let apart = std::thread::Builder::new().stack_size(256 << 10).spawn(move || bez_apart(&a, &b, 1e-3, &mut 20_000, 0)).expect("a thread").join().expect("no overflow");
        assert!(!apart);
    }

    /// A face's curves a period past its B-spline surface's own range (OCCT's on a periodic surface; fuzz seeds 979,
    /// 1036: RUST-BOOL-CLAMP-ABORT, f64::clamp with min 8.3 > max 2π): no box, so the face declines, not a panic.
    #[test]
    fn curves_past_the_surface_range_give_no_box() {
        let tau = std::f64::consts::TAU;
        let s = crate::nurbs::NurbsSurface { udegree: 1, vdegree: 1, uknots: vec![0.0, 0.0, tau, tau], vknots: vec![0.0, 0.0, 1.0, 1.0], nu: 2, nv: 2, poles: vec![DVec3::ZERO, DVec3::Y, DVec3::X, DVec3::X + DVec3::Y], weights: Vec::new() };
        let ns = FreeSurface::Nurbs(s);
        let line = |a: DVec2, b: DVec2| Arc::new(NurbsCurve { degree: 1, knots: vec![0.0, 0.0, 1.0, 1.0], poles: vec![a.extend(0.0), b.extend(0.0)], weights: Vec::new(), spans: Default::default() });
        let mut pc: PCurves = HashMap::default();
        pc.insert((0, false), (line(DVec2::new(8.3, 0.2), DVec2::new(9.0, 0.5)), 0.0, 1.0));
        assert!(uv_box(Some(&ns), &pc).is_none());
        pc.insert((0, false), (line(DVec2::new(1.0, 0.2), DVec2::new(2.0, 0.5)), 0.0, 1.0));
        let b = uv_box(Some(&ns), &pc).expect("a box inside the range");
        assert!(b[0] <= b[1] && b[2] <= b[3]);
        // (The Newton steps clamp to a box either way round.)
        let _ = ns.param_of(DVec3::new(0.5, 0.5, 0.0), DVec2::new(1.0, 0.5), [tau, 0.0, 1.0, 0.0]);
    }
}
