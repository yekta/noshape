//! Sheet metal loft (03c SM-16, FS `sheetMetalLoft`), ruled case: two
//! profiles with the same number of segments (curves are faceted by the
//! chordal tolerance) are joined by planar facets. Each facet becomes a
//! wall of the model thickness (mitered at the joints), every joint between
//! neighbouring facets becomes a bend of the model radius, and a closed loft
//! gets one rip (the first joint, or a connection marked Rip) so it can be
//! flattened.

use super::bshared::{curve_edges, order_chain, sample_chain};
use super::sheetmetal_ops::{bend_corner, slot_rip};
use super::sheetmetal_util::*;
use crate::display::BodyKind;
use crate::naming::Name;
use crate::reference::{Reference, Role, Source, TopoName};
use crate::regen::eval::{EvalCtx, FResult, NamedShape, err};
use crate::sheetmetal::Wall;
use glam::DVec3;
use noshape_kernel as k;
use noshape_kernel::Shape;
use std::sync::Arc;

/// Ordered polyline of a profile selection: (points, closed).
fn profile_polyline(ev: &EvalCtx, id: &str, chordal: f64) -> FResult<(Vec<DVec3>, bool)> {
    let refs = ev.query(id);
    if refs.is_empty() {
        return err(format!("Select {}", if id == "profile1" { "the first profile" } else { "the second profile" }));
    }
    let mut edges = curve_edges(ev, refs)?;
    for (bi, fi) in ev.resolve_faces(refs) {
        edges.extend(ev.face_shape(bi, fi)?.wires().first().map(|w| w.edges()).unwrap_or_default());
    }
    for p in ev.profiles(&refs.iter().filter(|r| matches!(r, Reference::SketchRegion { .. })).cloned().collect::<Vec<_>>())? {
        edges.extend(p.shape.wires().first().map(|w| w.edges()).unwrap_or_default());
    }
    if edges.is_empty() {
        return err("Profiles must be edges, sketch curves, regions or faces");
    }
    let scale = edges.iter().filter_map(|e| e.bounding_box(false).ok()).map(|b| b.diagonal()).fold(0.0f64, f64::max).max(1e-6);
    let chain = order_chain(&edges, scale * 1e-7)?;
    let mut pts: Vec<DVec3> = Vec::new();
    for (e, rev) in &chain {
        let info = e.edge_info()?;
        let n = match info.curve {
            k::Curve::Line { .. } => 1,
            k::Curve::Circle { radius, .. } => {
                let step = 2.0 * (1.0 - (chordal / radius).min(1.0)).acos();
                let sweep = info.length / radius;
                ((sweep / step.max(1e-3)).ceil() as usize).clamp(2, 256)
            }
            _ => ((info.length / (chordal * 50.0).max(1e-6)).ceil() as usize).clamp(4, 256),
        };
        let s = sample_chain(&[(e.clone(), *rev)], n + 1)?;
        let s: Vec<DVec3> = if n == 1 { vec![s[0], s[s.len() - 1]] } else { s };
        let skip = usize::from(!pts.is_empty());
        pts.extend(s.into_iter().skip(skip));
    }
    let closed = pts.len() > 2 && (pts[0] - pts[pts.len() - 1]).length() < scale * 1e-7;
    if closed {
        pts.pop();
    }
    // Drop collinear interior points (straight runs split into several edges).
    let mut clean: Vec<DVec3> = Vec::new();
    let n = pts.len();
    for i in 0..n {
        if !closed && (i == 0 || i == n - 1) {
            clean.push(pts[i]);
            continue;
        }
        let a = pts[(i + n - 1) % n];
        let b = pts[(i + 1) % n];
        let p = pts[i];
        if (p - a).cross(b - p).length() > 1e-12 * (p - a).length() * (b - p).length() + 1e-18 {
            clean.push(p);
        }
    }
    if clean.len() < 2 {
        return err("A profile has too few points");
    }
    Ok((clean, closed))
}

/// Best-fit plane normal of a closed or open polyline (Newell's method).
fn polyline_normal(p: &[DVec3]) -> DVec3 {
    let mut n = DVec3::ZERO;
    for i in 0..p.len() {
        let (a, b) = (p[i], p[(i + 1) % p.len()]);
        n += DVec3::new((a.y - b.y) * (a.z + b.z), (a.z - b.z) * (a.x + b.x), (a.x - b.x) * (a.y + b.y));
    }
    n.normalize_or_zero()
}

/// Intersection point of three planes (point, normal); `None` if degenerate.
fn three_planes(p: [(DVec3, DVec3); 3]) -> Option<DVec3> {
    let [(p1, n1), (p2, n2), (p3, n3)] = p;
    let den = n1.dot(n2.cross(n3));
    if den.abs() < 1e-12 {
        return None;
    }
    Some((n2.cross(n3) * p1.dot(n1) + n3.cross(n1) * p2.dot(n2) + n1.cross(n2) * p3.dot(n3)) / den)
}

fn quad_face(pts: &[DVec3]) -> FResult<Shape> {
    let w = k::make_polyline(pts, true)?;
    Ok(k::make_face_from_wire(&w, true).or_else(|_| k::make_face_from_wire(&w, false))?)
}

pub fn loft(ev: &mut EvalCtx) -> FResult<()> {
    if !ev.is("surfaceOperationType", "NEW") {
        return err("Adding a sheet metal loft to an existing model is not supported yet; use New");
    }
    let t = ev.length("thickness")?;
    if t <= 0.0 {
        return err("Thickness must be positive");
    }
    let chordal = ev.length("chordalTolerance").unwrap_or(1e-3).max(1e-6);
    let (p1, c1) = profile_polyline(ev, "profile1", chordal)?;
    let (mut p2, c2) = profile_polyline(ev, "profile2", chordal)?;
    if c1 != c2 {
        return err("Both profiles must be open, or both closed");
    }
    if p1.len() != p2.len() {
        return err(format!(
            "The profiles have {} and {} corners; the ruled sheet metal loft needs the same number of segments in both",
            p1.len(),
            p2.len()
        ));
    }
    let n = p1.len();
    // Match corners: same direction, best rotation (closed).
    let cost = |q: &[DVec3]| p1.iter().zip(q).map(|(a, b)| (*a - *b).length_squared()).sum::<f64>();
    let mut best = (cost(&p2), p2.clone());
    let mut cands = vec![p2.clone(), p2.iter().rev().copied().collect::<Vec<_>>()];
    if c1 {
        let base = cands.clone();
        cands.clear();
        for b in base {
            for r in 0..n {
                let mut v = b.clone();
                v.rotate_left(r);
                cands.push(v);
            }
        }
    }
    for cnd in cands {
        let c = cost(&cnd);
        if c < best.0 {
            best = (c, cnd);
        }
    }
    p2 = best.1;
    let nf = if c1 { n } else { n - 1 };
    if nf < 1 {
        return err("The profiles need at least one segment");
    }
    // Facet planes, normals toward the material side.
    let center: DVec3 = (p1.iter().chain(p2.iter()).copied().sum::<DVec3>()) / (2 * n) as f64;
    let flip = ev.bool("oppositeDirection");
    let mut normals = Vec::with_capacity(nf);
    for i in 0..nf {
        let j = (i + 1) % n;
        let q = [p1[i], p1[j], p2[j], p2[i]];
        let nn = polyline_normal(&q);
        if nn.length() < 0.5 {
            return err(format!("Facet {} is degenerate", i + 1));
        }
        let off = q.iter().map(|p| (*p - q[0]).dot(nn).abs()).fold(0.0, f64::max);
        if off > 1e-7 * (q[2] - q[0]).length().max(1e-9) + 1e-9 {
            return err(format!("Facet {} is not planar; the ruled sheet metal loft needs matching segments that span planar facets", i + 1));
        }
        normals.push(nn);
    }
    // Material inside a closed loft (toward the center), else along the facet normals.
    let inward = if c1 { normals.iter().enumerate().map(|(i, nn)| (center - p1[i]).dot(*nn)).sum::<f64>() > 0.0 } else { true };
    let sign = if inward != flip { 1.0 } else { -1.0 };
    for nn in &mut normals {
        *nn *= sign;
    }
    let pn1 = polyline_normal(&p1);
    let pn2 = polyline_normal(&p2);
    let end_normal = |p: &[DVec3], pn: DVec3, other: &[DVec3]| -> FResult<DVec3> {
        let pn = if pn.length() > 0.5 {
            pn
        } else if p.len() >= 2 {
            // Open straight profile: plane containing it, perpendicular to the loft direction.
            let d = (p[p.len() - 1] - p[0]).normalize_or_zero();
            let along = (other[0] - p[0]).normalize_or_zero();
            let x = d.cross(along).cross(d).normalize_or_zero();
            if x.length() < 0.5 {
                return err("A profile is degenerate");
            }
            x
        } else {
            return err("A profile is degenerate");
        };
        let off = p.iter().map(|q| (*q - p[0]).dot(pn).abs()).fold(0.0, f64::max);
        if off > 1e-7 {
            return err("Each profile must be planar");
        }
        Ok(pn)
    };
    let e1 = end_normal(&p1, pn1, &p2)?;
    let e2 = end_normal(&p2, pn2, &p1)?;
    // Offset (back side) corners at each profile.
    let offset_corners = |p: &[DVec3], en: DVec3| -> FResult<Vec<DVec3>> {
        let mut out = Vec::with_capacity(n);
        for j in 0..n {
            let fa = if c1 { Some((j + nf - 1) % nf) } else if j > 0 { Some(j - 1) } else { None };
            let fb = if c1 || j < nf { Some(j % nf) } else { None };
            let q = match (fa, fb) {
                (Some(a), Some(b)) if a != b => {
                    let pa = (p[a] + normals[a] * t, normals[a]);
                    let pb = (p[b % n] + normals[b] * t, normals[b]);
                    three_planes([pa, pb, (p[j], en)])
                }
                (Some(a), _) | (_, Some(a)) => {
                    // Free end: move across the facet's offset plane within the profile plane.
                    let nn = normals[a];
                    let dir = nn - en * nn.dot(en);
                    let l = dir.length();
                    (l > 1e-9).then(|| p[j] + dir / l * (t / (dir.dot(nn) / l)))
                }
                _ => None,
            };
            out.push(q.ok_or_else(|| fe("The loft facets meet at a degenerate joint"))?);
        }
        Ok(out)
    };
    let o1 = offset_corners(&p1, e1)?;
    let o2 = offset_corners(&p2, e2)?;
    let feature = ev.feature;
    let name = |k: u32| -> Name { Arc::new(TopoName::face(feature, Role::Side(Source::Index(k)))) };
    let mut faces: Vec<(Shape, Name)> = Vec::new();
    for i in 0..nf {
        let j = (i + 1) % n;
        faces.push((quad_face(&[p1[i], p1[j], p2[j], p2[i]])?, name(i as u32)));
        faces.push((quad_face(&[o1[i], o1[j], o2[j], o2[i]])?, name(1000 + i as u32)));
        faces.push((quad_face(&[p1[i], p1[j], o1[j], o1[i]])?, name(2000 + i as u32)));
        faces.push((quad_face(&[p2[i], p2[j], o2[j], o2[i]])?, name(3000 + i as u32)));
    }
    if !c1 {
        faces.push((quad_face(&[p1[0], p2[0], o2[0], o1[0]])?, name(4000)));
        faces.push((quad_face(&[p1[n - 1], p2[n - 1], o2[n - 1], o1[n - 1]])?, name(4001)));
    }
    let shapes: Vec<Shape> = faces.iter().map(|f| f.0.clone()).collect();
    let sewn = k::sew(&shapes, (t * 1e-4).min(1e-6), true).map_err(|e| fe(format!("The loft walls could not be joined into a solid: {e}")))?;
    let solid = sewn.shape;
    // Name output faces by their source face (centroid match).
    let src_c: Vec<DVec3> = faces.iter().map(|f| f.0.centroid().unwrap_or_default()).collect();
    let mut names = Vec::new();
    for f in solid.faces() {
        let c = f.centroid()?;
        let k = (0..src_c.len()).min_by(|&a, &b| (src_c[a] - c).length().total_cmp(&(src_c[b] - c).length())).unwrap_or(0);
        names.push(faces[k].1.clone());
    }
    let before = ev.ctx.bodies.len();
    ev.apply_operation(vec![NamedShape::new(solid, names)], "NEW", None, BodyKind::Solid)?;
    if ev.ctx.bodies.len() <= before {
        return err("The sheet metal loft produced no part");
    }
    let bi = ev.ctx.bodies.len() - 1;
    let mut data = model_from_params(ev, t)?;
    for i in 0..nf {
        data.walls.push(Wall { feature, faces: vec![name(i as u32)], back: vec![name(1000 + i as u32)] });
    }
    // Rip joint of a closed loft: a connection marked Rip near a corner, else corner 0.
    let mut rip_at = 0usize;
    if c1 && ev.bool("matchConnections") {
        for c in ev.params.array("connections") {
            if !c.bool("isRip") {
                continue;
            }
            for r in c.query("connectionEntities") {
                let pt = ev.resolve_vertex_point(r).or_else(|| {
                    let (b, e) = *ev.resolve_edges(std::slice::from_ref(r)).first()?;
                    ev.edge_shape(b, e).ok()?.endpoints().ok().map(|(a, b)| (a + b) * 0.5)
                });
                if let Some(pt) = pt
                    && let Some(j) = (0..n).min_by(|&a, &b| (p1[a] - pt).length().min((p2[a] - pt).length()).total_cmp(&(p1[b] - pt).length().min((p2[b] - pt).length())))
                {
                    rip_at = j;
                }
            }
        }
    }
    let (r, kf) = (data.radius, data.k_factor);
    let joints: Vec<usize> = if c1 { (0..n).collect() } else { (1..n - 1).collect() };
    let mut sharp = 0;
    for j in joints {
        let (a, b) = ((j + nf - 1) % nf, j % nf);
        if c1 && j == rip_at {
            slot_rip(ev, bi, &mut data, a, b, "EDGE").map_err(|e| fe(format!("The loft rip failed: {}", e.message)))?;
            continue;
        }
        if normals[a].cross(normals[b]).length() < 1e-9 {
            continue; // tangent facets
        }
        if bend_corner(ev, bi, &mut data, a.min(b), a.max(b), r, kf).is_err() {
            sharp += 1;
        }
    }
    if sharp > 0 {
        ev.warnings.push(format!("{sharp} joint(s) could not be bent (radius too large for the facets) and were left sharp"));
    }
    put_data(ev, bi, data);
    Ok(())
}
