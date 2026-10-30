//! Fillet, chamfer and shell.

use crate::naming::Name;
use crate::reference::Role;
use crate::regen::eval::{EvalCtx, FResult, err};
use noshape_kernel as k;
use glam::DVec3;
use noshape_kernel::{Shape, ShapeType};
use std::collections::{BTreeMap, BTreeSet};

/// Group resolved (body, edge) pairs by body.
fn by_body(items: Vec<(usize, usize)>) -> BTreeMap<usize, Vec<usize>> {
    let mut m: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for (b, e) in items {
        m.entry(b).or_default().push(e);
    }
    m
}

/// Expand an edge set across tangent-continuous neighbors (tangent
/// propagation): edges meeting at a shared vertex with parallel tangents
/// (within ~1 degree) are added transitively. Shared by fillet, chamfer and
/// face blend.
pub fn propagate_tangent(shape: &k::Shape, edges: &[usize]) -> Vec<usize> {
    let Some(links) = TangentLinks::new(shape) else { return edges.to_vec() };
    let mut set: BTreeSet<usize> = edges.iter().copied().collect();
    let mut stack = edges.to_vec();
    while let Some(ei) = stack.pop() {
        for o in links.neighbours(ei) {
            if set.insert(o) {
                stack.push(o);
            }
        }
    }
    set.into_iter().collect()
}

/// Edges meeting at a shared vertex with parallel tangents (within ~1 degree).
struct TangentLinks {
    edges: Vec<k::Shape>,
    vertices: Vec<k::Shape>,
    vertex_edges: Vec<Vec<u32>>,
    shape: k::Shape,
}

impl TangentLinks {
    fn new(shape: &k::Shape) -> Option<Self> {
        Some(TangentLinks { edges: shape.edges(), vertices: shape.vertices(), vertex_edges: shape.vertex_edges().ok()?, shape: shape.clone() })
    }

    fn neighbours(&self, ei: usize) -> Vec<usize> {
        let tangent_at = |e: &k::Shape, p: glam::DVec3| -> Option<glam::DVec3> {
            // The parameter end at `p` (a reversed edge starts at its last
            // parameter: extrudes since 339de774 reverse their top edges).
            let info = e.edge_info().ok()?;
            let at = |t: f64| e.point_at(t).map(|q| (q - p).length()).unwrap_or(f64::MAX);
            let t = if at(info.first_param) < at(info.last_param) { info.first_param } else { info.last_param };
            e.tangent_at(t).ok().map(|v| v.normalize())
        };
        let mut out = Vec::new();
        let Some(e) = self.edges.get(ei) else { return out };
        for v in e.vertices() {
            let Some(vi) = self.shape.index_of(&v, ShapeType::Vertex) else { continue };
            let Ok(p) = self.vertices[vi].vertex_point() else { continue };
            for &o in self.vertex_edges.get(vi).map(|x| x.as_slice()).unwrap_or(&[]) {
                let o = o as usize;
                if o != ei
                    && !out.contains(&o)
                    && let (Some(t1), Some(t2)) = (tangent_at(e, p), self.edges.get(o).and_then(|x| tangent_at(x, p)))
                    && t1.dot(t2).abs() > 0.9998
                {
                    out.push(o);
                }
            }
        }
        out
    }
}

/// Without tangent propagation Parasolid runs a straight edge's blend out
/// onto a tangent neighbour, but refuses a curved edge that continues
/// tangentially into an edge left out (Onshape G7: one top arc of a
/// filleted corner, alone or with its straight neighbour).
fn curved_edge_left_open(shape: &k::Shape, edges: &[usize]) -> bool {
    let Some(links) = TangentLinks::new(shape) else { return false };
    edges.iter().any(|&e| {
        let straight = links.edges.get(e).and_then(|x| x.edge_info().ok()).is_some_and(|i| matches!(i.curve, k::Curve::Line { .. }));
        !straight && links.neighbours(e).iter().any(|o| !edges.contains(o))
    })
}

fn blend_role(n: &Name) -> Role {
    Role::Blend(n.clone())
}

/// Onshape's message for a failed fillet/chamfer; the kernel reason goes to the log.
fn blend_failed(message: &str, e: impl std::fmt::Display) -> crate::regen::eval::FeatureError {
    log::debug!("{message} ({e})");
    crate::regen::eval::FeatureError::new(message)
}

/// Onshape's message for a failed fillet: one that would roll past the faces
/// it touches has its own (live "kern blend sweep" 2a).
fn fillet_failed(e: k::KernelError) -> crate::regen::eval::FeatureError {
    if e.to_string().contains("unfilleted edge overlapped") {
        return blend_failed("Unfilleted edge overlapped by fillet.", e);
    }
    blend_failed("Failed to fillet selections.", e)
}

/// Onshape's message for a failed chamfer: a chamfer wider than a face it
/// runs over has its own.
fn chamfer_failed(e: k::KernelError) -> crate::regen::eval::FeatureError {
    if e.to_string().contains("chamfer distance too large") {
        return blend_failed("Distance of chamfer on face too large.", e);
    }
    blend_failed("Failed to chamfer selections.", e)
}

/// Whether a fillet of `r` on a straight edge between two planar faces
/// reaches past either face (its setback wider than the face).
fn overflows(shape: &Shape, edges: &[usize], r: f64) -> bool {
    let (Ok(ef), faces, all) = (shape.edge_faces(), shape.faces(), shape.edges()) else { return false };
    edges.iter().any(|&e| {
        let (Some(edge), Some(fs)) = (all.get(e), ef.get(e)) else { return false };
        let Ok(info) = edge.edge_info() else { return false };
        if fs.len() != 2 || !matches!(info.curve, k::Curve::Line { .. }) {
            return false;
        }
        let ff = [&faces[fs[0] as usize], &faces[fs[1] as usize]];
        if !ff.iter().all(|f| matches!(f.face_surface(), Ok(k::Surface::Plane(_)))) {
            return false;
        }
        let d = (info.end - info.start).normalize_or_zero();
        let mid = (info.start + info.end) * 0.5;
        let (Ok(n0), Ok(n1)) = (ff[0].normal_at_point(mid), ff[1].normal_at_point(mid)) else { return false };
        let half = n0.normalize().dot(n1.normalize()).clamp(-1.0, 1.0).acos() / 2.0;
        let setback = r * half.tan();
        let line_dist = |q: DVec3| (q - info.start - d * (q - info.start).dot(d)).length();
        ff.iter().any(|f| f.vertices().iter().filter_map(|v| v.vertex_point().ok()).map(line_dist).fold(0.0, f64::max) < setback * (1.0 - 1e-9))
    })
}

/// A selected edge's setback on each of its two planar faces: (face, the
/// direction into it across the edge, the setback).
type Setbacks = [(u32, DVec3, f64); 2];

/// Where a round of a multi-edge selection runs past a face (see
/// [`round_overrun`]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Overrun {
    /// Across the face to another selected edge: "End boundary intersects
    /// unfilleted edge." / "... unchamfered edge.".
    PastSelected,
    /// Past an edge that isn't selected: "Failed to fillet selections." /
    /// "Failed to chamfer selections.".
    PastUnselected,
}

/// Live's rule for rounds and chamfers of two or more edges (behaviours
/// "spec: kernel FILLET-OVERSIZE-OK chamfer cases and fillet thresholds",
/// request 1516 / 1587): a selected straight edge between planar faces that
/// shares a face with another selected edge may set back on each of its faces
/// at most that face's width (to the parallel edge across it); one step
/// beyond, the feature fails and the part stays as it was. Rounds that only
/// overlap each other are fine; a single edge rolls over any face. `amount(i,
/// t)`: the setback of the selection's edge `i` on a face, `t` the tangent of
/// half the angle between the faces' normals (a fillet's r·t, a chamfer's d).
fn round_overrun(shape: &Shape, edges: &[usize], amount: &dyn Fn(usize, f64) -> f64) -> Option<Overrun> {
    if edges.len() < 2 {
        return None;
    }
    let (Ok(ef), faces, all) = (shape.edge_faces(), shape.faces(), shape.edges()) else { return None };
    // Edges that meet (the 4 top edges R12, live F2) fail otherwise: "Failed
    // to fillet selections.", from the kernel.
    let ends: Vec<Vec<Shape>> = edges.iter().map(|&e| all.get(e).map(|x| x.vertices()).unwrap_or_default()).collect();
    if ends.iter().enumerate().any(|(i, a)| ends[i + 1..].iter().any(|b| a.iter().any(|v| b.iter().any(|w| v.is_same(w))))) {
        return None;
    }
    let mut face_edges = vec![Vec::new(); faces.len()];
    for (e, fs) in ef.iter().enumerate() {
        for &f in fs {
            if let Some(l) = face_edges.get_mut(f as usize) {
                l.push(e);
            }
        }
    }
    let line = |e: usize| match all.get(e).map(|s| s.edge_info()) {
        Some(Ok(i)) if matches!(i.curve, k::Curve::Line { .. }) => Some((i.start, i.end)),
        _ => None,
    };
    let plane = |f: u32| matches!(faces.get(f as usize).map(|f| f.face_surface()), Some(Ok(k::Surface::Plane(_))));
    // Convex edges only.
    let setbacks = |i: usize, e: usize| -> Option<Setbacks> {
        let (a, b) = line(e)?;
        let fs = ef.get(e)?;
        if fs.len() != 2 || !plane(fs[0]) || !plane(fs[1]) {
            return None;
        }
        let d = (b - a).normalize_or_zero();
        let mid = (a + b) * 0.5;
        let n = [faces[fs[0] as usize].normal_at_point(mid).ok()?.normalize(), faces[fs[1] as usize].normal_at_point(mid).ok()?.normalize()];
        let setback = amount(i, (n[0].dot(n[1]).clamp(-1.0, 1.0).acos() / 2.0).tan());
        let mut out = [(0, DVec3::ZERO, 0.0); 2];
        for k in 0..2 {
            // Into the face: the side its vertices are on.
            let w = n[k].cross(d).normalize_or_zero();
            let reach = |w: DVec3| faces[fs[k] as usize].vertices().iter().filter_map(|v| v.vertex_point().ok()).map(|p| (p - a).dot(w)).fold(0.0, f64::max);
            let w = if reach(w) >= reach(-w) { w } else { -w };
            // Convex: the face runs away from the other face's outside.
            if w.dot(n[1 - k]) > -1e-9 {
                return None;
            }
            out[k] = (fs[k], w, setback);
        }
        Some(out)
    };
    let selected: Vec<(usize, Setbacks)> = edges.iter().enumerate().filter_map(|(i, &e)| Some((e, setbacks(i, e)?))).collect();
    let largest = selected.iter().flat_map(|(_, s)| s.iter().map(|x| x.2)).fold(0.0, f64::max);
    let tol = 1e-9 * (1.0 + largest);
    let (mut past_selected, mut past_unselected) = (false, false);
    for (e, sides) in &selected {
        // With another selected edge on one of its faces.
        if !sides.iter().any(|&(f, _, _)| face_edges.get(f as usize).is_some_and(|l| l.iter().any(|o| o != e && edges.contains(o)))) {
            continue;
        }
        let (a, b) = line(*e).unwrap_or_default();
        let d = (b - a).normalize_or_zero();
        for &(f, w, setback) in sides {
            for &o in face_edges.get(f as usize).into_iter().flatten() {
                let Some((p, q)) = line(o) else { continue };
                // Parallel, across the face, beside this edge for some length.
                let width = (p - a).dot(w);
                let (s0, s1) = ((p - a).dot(d), (q - a).dot(d));
                if o == *e || (q - p).normalize_or_zero().dot(d).abs() < 1.0 - 1e-9 || width <= tol || s0.max(s1) <= tol || s0.min(s1) >= (b - a).length() - tol {
                    continue;
                }
                if setback > width + tol {
                    if edges.contains(&o) {
                        past_selected = true;
                    } else {
                        past_unselected = true;
                    }
                }
            }
        }
    }
    if past_selected {
        Some(Overrun::PastSelected)
    } else if past_unselected {
        Some(Overrun::PastUnselected)
    } else {
        None
    }
}

pub fn fillet(ev: &mut EvalCtx) -> FResult<()> {
    if ev.is("filletType", "FULL_ROUND") {
        return full_round(ev);
    }
    let refs = ev.query("entities");
    if refs.is_empty() {
        return crate::regen::eval::err_at(&["entities"], "Select edges or faces to fillet.");
    }
    let circular = !ev.is("crossSection", "CONIC") && !ev.is("crossSection", "CURVATURE");
    let width = ev.is("blendControlType", "WIDTH");
    let radius = if width {
        ev.length("width")? / std::f64::consts::SQRT_2
    } else if circular {
        ev.length("radius")?
    } else {
        ev.length("nonCircularRadius")?
    };
    if radius <= 0.0 {
        return err("Fillet radius must be positive");
    }
    if ev.bool("isVariable") && !width {
        return variable_fillet(ev, radius);
    }
    // Sections OCCT can't build (conic, curvature, asymmetric, partial) go to booleans.
    let special = {
        use super::blend_section::{BlendSpec, Bound, Partial, Section};
        let asym = !width && ev.bool("isAsymmetric");
        let partial = if !width && ev.bool("isPartial") {
            // std fillet.fs: the end offset is measured from the chain's end.
            let bound = |ev: &EvalCtx, ty: &str, frac: &str, off: &str, ent: &str, second: bool| -> FResult<Bound> {
                Ok(match ev.enum_(ty) {
                    "OFFSET" if second => Bound::OffsetFromEnd(ev.length(off)?),
                    "OFFSET" => Bound::Offset(ev.length(off)?),
                    "ENTITY" => {
                        let r = ev.query(ent).first().ok_or_else(|| crate::regen::eval::FeatureError::new("Select an entity for the partial fillet end type."))?;
                        let p = ev.resolve_vertex_point(r).or_else(|| ev.resolve_faces(std::slice::from_ref(r)).first().and_then(|&(b, f)| ev.ctx.bodies[b].shape.sub_shape(ShapeType::Face, f).ok()?.centroid().ok()));
                        Bound::Point(p.ok_or_else(|| crate::regen::eval::FeatureError::new("The bounding entity no longer exists"))?)
                    }
                    _ => Bound::Fraction(ev.real(frac)?),
                })
            };
            let first = bound(ev, "startPartialType", "partialFirstEdgeTotalParameter", "startPartialOffset", "startPartialEntity", false)?;
            let second = if ev.bool("secondBound") { Some(bound(ev, "endPartialType", "partialSecondEdgeTotalParameter", "endPartialOffset", "endPartialEntity", true)?) } else { None };
            Some(Partial { first, second, opposite: ev.bool("partialOppositeParameter") })
        } else {
            None
        };
        if !circular || asym || partial.is_some() {
            let (mut r1, mut r2) = (radius, if asym { ev.length("otherRadius")? } else { radius });
            if asym && ev.bool("flipAsymmetric") {
                std::mem::swap(&mut r1, &mut r2);
            }
            let section = if circular {
                Section::Circular
            } else if ev.is("crossSection", "CURVATURE") {
                Section::Curvature(ev.real("magnitude")?)
            } else {
                Section::Conic(ev.real("rho")?)
            };
            Some(BlendSpec { section, r1, r2, width: width.then_some(radius * std::f64::consts::SQRT_2), partial, run_on: !ev.bool("tangentPropagation"), cliff: false, sides: None })
        } else {
            None
        }
    };
    let groups = by_body(ev.resolve_edges(refs));
    // Live (qa5 DER13): edges that are all gone read like an empty field.
    if groups.is_empty() {
        return crate::regen::eval::err_at(&["entities"], "Select edges or faces to fillet.");
    }
    for (bi, edges) in groups.into_iter().rev() {
        let body = ev.ctx.bodies[bi].clone();
        let propagate = ev.bool("tangentPropagation");
        let unpropagated = |r: f64| super::blend_section::BlendSpec { section: super::blend_section::Section::Circular, r1: r, r2: r, width: None, partial: None, run_on: true, cliff: false, sides: None };
        if !propagate && curved_edge_left_open(&body.shape, &edges) {
            return err("Adjoining edge not filleted.");
        }
        if let Some(spec) = &special {
            let edges = if propagate { propagate_tangent(&body.shape, &edges) } else { edges };
            if let Some(p) = &spec.partial {
                super::blend_section::check_partial(&body.shape, &edges, p)?;
            }
            super::blend_section::section_blend(ev, bi, &edges, spec)?;
            continue;
        }
        // Allow edge overflow off and a ball wider than a straight edge's
        // planar face: it rests on the face's far edge (live oracle_p1 F7).
        if special.is_none() && !width && !ev.bool("allowEdgeOverflow") && overflows(&body.shape, &edges, radius) {
            let spec = super::blend_section::BlendSpec { cliff: true, ..unpropagated(radius) };
            super::blend_section::section_blend(ev, bi, &edges, &spec)?;
            continue;
        }
        if !propagate && propagate_tangent(&body.shape, &edges).len() > edges.len() {
            // OCCT always runs a fillet on along tangent edges; Onshape stops at the
            // selection (the blend runs straight on into the tangent neighbour).
            super::blend_section::section_blend(ev, bi, &edges, &unpropagated(radius))?;
            continue;
        }
        let edges = if propagate { propagate_tangent(&body.shape, &edges) } else { edges };
        let shapes: Vec<k::Shape> = edges.iter().filter_map(|e| body.shape.sub_shape(ShapeType::Edge, *e).ok()).collect();
        let t0 = std::time::Instant::now();
        // Checked: a valid solid or an error, also where OCCT needs help (blends consuming a face).
        // Width = chord between the tangent points: 2 R sin(φ/2), φ the angle
        // between the faces' normals (R = W / √2 on a square edge).
        let radii: Vec<f64> = if ev.is("blendControlType", "WIDTH") {
            let w = radius * std::f64::consts::SQRT_2;
            let ef = body.shape.edge_faces()?;
            let faces = body.shape.faces();
            edges
                .iter()
                .map(|&e| {
                    let phi = (|| {
                        let edge = body.shape.sub_shape(ShapeType::Edge, e).ok()?;
                        let info = edge.edge_info().ok()?;
                        let m = edge.point_at((info.first_param + info.last_param) / 2.0).ok()?;
                        let fs = ef.get(e)?;
                        let (a, b) = (faces.get(*fs.first()? as usize)?, faces.get(*fs.get(1)? as usize)?);
                        let (na, nb) = (a.normal_at_point(m).ok()?, b.normal_at_point(m).ok()?);
                        Some(na.normalize().dot(nb.normalize()).clamp(-1.0, 1.0).acos())
                    })()
                    .unwrap_or(std::f64::consts::FRAC_PI_2);
                    if phi > 1e-6 { w / (2.0 * (phi / 2.0).sin()) } else { radius }
                })
                .collect()
        } else {
            vec![radius]
        };
        let radius_of = |i: usize| radii.get(i).or(radii.first()).copied().unwrap_or(0.0);
        match round_overrun(&body.shape, &edges, &|i, t| radius_of(i) * t) {
            Some(Overrun::PastSelected) => return err("End boundary intersects unfilleted edge."),
            Some(Overrun::PastUnselected) => return err("Failed to fillet selections."),
            None => {}
        }
        let res = match k::fillet_checked(&body.shape, &shapes, &radii) {
            Ok(r) => r,
            // Without propagation a single straight edge whose ball fits its faces
            // can still be filleted by booleans (F3: R4 after R2/R3 stopped at their edges).
            Err(_) if !propagate && edges.len() == 1 && super::blend_section::section_blend(ev, bi, &edges, &unpropagated(radius)).is_ok() => continue,
            Err(e) => return Err(fillet_failed(e)),
        };
        let t1 = std::time::Instant::now();
        let names = body.shape_names();
        let face_names = ev.name_result(&[&names], &res.history, &res.shape, &BTreeMap::new(), Some(&blend_role));
        let t2 = std::time::Instant::now();
        ev.replace_body(bi, res.shape, face_names);
        log::debug!("fillet: kernel {:?}, naming {:?}, body {:?} (history records {})", t1 - t0, t2 - t1, t2.elapsed(), res.history.records.len());
    }
    Ok(())
}

/// Radius law of a variable fillet along one edge: (relative parameter, radius)
/// pairs from the ends and the points between. Smooth transition on: each
/// piece eases between its radii with zero slope at both points (smoothstep;
/// Onshape G3/G6 match). Off: a C1 cubic through the points (Onshape G6): at a
/// point between, the slope is zero where the radius peaks or dips, else the
/// gentler of the two neighbouring chords (that span is then straight); at the
/// ends, the chord's slope, or 1.5 times it next to a peak or dip (Onshape G6,
/// G11: exact Bézier control points of the blend's boundary).
/// OCCT interpolates its law smoothly through the given points (even two), so
/// the pieces are sampled densely.
fn variable_law(ends: (f64, f64), points: &mut Vec<(f64, f64)>, smooth: bool) -> Vec<(f64, f64)> {
    points.retain(|p| p.0 > 1e-9 && p.0 < 1.0 - 1e-9);
    points.sort_by(|a, b| a.0.total_cmp(&b.0));
    points.dedup_by(|a, b| (a.0 - b.0).abs() < 1e-9);
    let mut knots = vec![(0.0, ends.0)];
    knots.extend(points.iter().copied());
    knots.push((1.0, ends.1));
    let n = knots.len();
    let chord: Vec<f64> = knots.windows(2).map(|w| (w[1].1 - w[0].1) / (w[1].0 - w[0].0)).collect();
    let mut slope = vec![0.0; n];
    if !smooth && n == 2 {
        // Linear from end to end (Onshape F4 without points: 7939.8876 mm³).
        slope = vec![chord[0]; 2];
    } else if !smooth {
        for i in 1..n - 1 {
            let (a, b) = (chord[i - 1], chord[i]);
            slope[i] = if a * b <= 0.0 { 0.0 } else if a.abs() < b.abs() { a } else { b };
        }
        // At an end: the chord's slope, or 1.5 times it next to a peak or dip
        // (Onshape G6/G11: the boundary is a cubic Bézier per span).
        let end = |c: f64, inner: f64| if inner == 0.0 { 1.5 * c } else { c };
        slope[0] = end(chord[0], slope[1]);
        slope[n - 1] = end(chord[n - 2], slope[n - 2]);
    }
    let mut law = Vec::new();
    for (i, w) in knots.windows(2).enumerate() {
        let h = w[1].0 - w[0].0;
        let steps = 16;
        for j in 0..steps {
            let f = j as f64 / steps as f64;
            // Cubic Hermite; with zero end slopes this is smoothstep.
            let (h00, h10, h01, h11) = (2.0 * f * f * f - 3.0 * f * f + 1.0, f * f * f - 2.0 * f * f + f, -2.0 * f * f * f + 3.0 * f * f, f * f * f - f * f);
            law.push((w[0].0 + h * f, h00 * w[0].1 + h10 * h * slope[i] + h01 * w[1].1 + h11 * h * slope[i + 1]));
        }
    }
    law.push(*knots.last().expect("two knots"));
    law
}



/// Variable fillet: Onshape's `vertexSettings` (vertex + radius) and
/// `pointOnEdgeSettings` (edge, position, radius); a plain `vertices` query
/// takes the main radius. Edges are filleted one at a time, found again by name.
fn variable_fillet(ev: &mut EvalCtx, radius: f64) -> FResult<()> {
    use crate::params::NumKind;
    let mut at_vertex: Vec<(glam::DVec3, f64)> = Vec::new();
    for item in ev.params.array("vertexSettings") {
        let Some(r) = item.query("vertex").first() else { continue };
        let Some(p) = ev.resolve_vertex_point(r) else { return err("A selected vertex no longer exists") };
        at_vertex.push((p, ev.eval_num_in(item, "vertexRadius", NumKind::Length)?));
    }
    for r in ev.query("vertices") {
        if let Some(p) = ev.resolve_vertex_point(r) {
            at_vertex.push((p, radius));
        }
    }
    let mut on_edge: Vec<(Name, f64, f64)> = Vec::new();
    for item in ev.params.array("pointOnEdgeSettings") {
        let Some(&(bi, ei)) = ev.resolve_edges(item.query("edge")).first() else { continue };
        let name = ev.ctx.bodies[bi].edges[ei].clone();
        on_edge.push((name, ev.eval_num_in(item, "edgeParameter", NumKind::Real)?, ev.eval_num_in(item, "pointOnEdgeRadius", NumKind::Length)?));
    }
    let smooth = ev.bool("smoothTransition");
    let refs = ev.query("entities");
    if refs.is_empty() {
        return crate::regen::eval::err_at(&["entities"], "Select edges or faces to fillet.");
    }
    let groups = by_body(ev.resolve_edges(refs));
    // Live (qa5 DER13): edges that are all gone read like an empty field.
    if groups.is_empty() {
        return crate::regen::eval::err_at(&["entities"], "Select edges or faces to fillet.");
    }
    let radius_at = |p: glam::DVec3| at_vertex.iter().find(|(q, _)| (*q - p).length() < 1e-9).map(|v| v.1).unwrap_or(radius);
    for (bi, edges) in groups.into_iter().rev() {
        let body = ev.ctx.bodies[bi].clone();
        let edges = if ev.bool("tangentPropagation") { propagate_tangent(&body.shape, &edges) } else { edges };
        let names: Vec<Name> = edges.iter().map(|&e| body.edges[e].clone()).collect();
        for name in names {
            let body = ev.ctx.bodies[bi].clone();
            let Some(&e) = body.find_edges(&name).first() else { continue };
            let edge = body.shape.sub_shape(ShapeType::Edge, e)?;
            let info = edge.edge_info()?;
            // Onshape's position 0 is the edge's end nearest the origin corner (lowest x, y, z).
            let key = |p: glam::DVec3| (p.x, p.y, p.z);
            let flip = key(info.end).partial_cmp(&key(info.start)) == Some(std::cmp::Ordering::Less);
            let mut points: Vec<(f64, f64)> = on_edge.iter().filter(|(n, _, _)| *n == name).map(|(_, t, r)| (if flip { 1.0 - *t } else { *t }, *r)).collect();
            let law = variable_law((radius_at(info.start), radius_at(info.end)), &mut points, smooth);
            let res = k::fillet_variable_checked(&body.shape, &edge, &law).map_err(|e| blend_failed("Failed to fillet selections.", e))?;
            let names = body.shape_names();
            let face_names = ev.name_result(&[&names], &res.history, &res.shape, &BTreeMap::new(), Some(&blend_role));
            ev.replace_body(bi, res.shape, face_names);
        }
    }
    Ok(())
}

pub fn chamfer(ev: &mut EvalCtx) -> FResult<()> {
    let refs = ev.query("entities");
    if refs.is_empty() {
        return crate::regen::eval::err_at(&["entities"], "Select edges or faces to chamfer.");
    }
    let spec = match ev.enum_("chamferType") {
        "TWO_OFFSETS" => k::ChamferSpec::TwoDistances(ev.length("width1")?, ev.length("width2")?),
        "OFFSET_ANGLE" => k::ChamferSpec::DistanceAngle(ev.length("width")?, ev.angle("angle")?),
        _ => k::ChamferSpec::Equal(ev.length("width")?),
    };
    let overrides: BTreeSet<(usize, usize)> = ev.resolve_edges(ev.query("directionOverrides")).into_iter().collect();
    let groups = by_body(ev.resolve_edges(refs));
    // Live (qa5 DER13): edges that are all gone read like an empty field.
    if groups.is_empty() {
        return crate::regen::eval::err_at(&["entities"], "Select edges or faces to chamfer.");
    }
    for (bi, edges) in groups.into_iter().rev() {
        let body = ev.ctx.bodies[bi].clone();
        let propagate = ev.bool("tangentPropagation");
        if let k::ChamferSpec::Equal(d) = spec
            && !propagate
            && propagate_tangent(&body.shape, &edges).len() > edges.len()
        {
            // OCCT runs a chamfer on along tangent edges; Onshape stops at the
            // selection and runs the section out onto the neighbour (G7).
            let spec = super::blend_section::BlendSpec { section: super::blend_section::Section::Chamfer, r1: d, r2: d, width: None, partial: None, run_on: true, cliff: false, sides: None };
            super::blend_section::section_blend(ev, bi, &edges, &spec).map_err(|e| blend_failed("Failed to chamfer selections.", e.message))?;
            continue;
        }
        let edges = if propagate { propagate_tangent(&body.shape, &edges) } else { edges };
        let shapes: Vec<k::Shape> = edges.iter().filter_map(|e| body.shape.sub_shape(ShapeType::Edge, *e).ok()).collect();
        let reference = if matches!(spec, k::ChamferSpec::Equal(_)) {
            Vec::new()
        } else {
            let flipped: BTreeSet<usize> = overrides.iter().filter(|(b, _)| *b == bi).map(|&(_, e)| e).collect();
            distance_one_faces(ev, &body, &edges, &flipped, ev.bool("oppositeDirection"))?
        };
        if let k::ChamferSpec::Equal(d) = spec {
            match round_overrun(&body.shape, &edges, &|_, _| d) {
                Some(Overrun::PastSelected) => return err("End boundary intersects unchamfered edge."),
                Some(Overrun::PastUnselected) => return err("Failed to chamfer selections."),
                None => {}
            }
        }
        let res = k::chamfer_checked(&body.shape, &shapes, spec, &reference).map_err(chamfer_failed)?;
        let names = body.shape_names();
        let face_names = ev.name_result(&[&names], &res.history, &res.shape, &BTreeMap::new(), Some(&blend_role));
        ev.replace_body(bi, res.shape, face_names);
    }
    Ok(())
}

/// Onshape's entity order of faces (its deterministic ids, live G37): faces of
/// earlier features first; within a feature that sweeps profiles, the side of
/// the first profile segment, the start cap, the end cap, then the other sides
/// in profile order (G30's block drawn (0,0)→(40,0)→(40,20)→(0,20): front,
/// bottom, top, right, back, left). Other faces follow in name order.
pub(crate) fn face_order(ev: &EvalCtx, body: &crate::regen::context::Body, face: &Name) -> (usize, u8, Name) {
    let crate::reference::TopoName::Face { feature, role } = face.root() else { return (usize::MAX, 4, face.clone()) };
    let position = ev.features.iter().position(|f| f.id == *feature).unwrap_or(usize::MAX);
    let first_side = || {
        body.faces
            .iter()
            .filter_map(|n| match n.root() {
                crate::reference::TopoName::Face { feature: f, role: Role::Side(src) } if f == feature => Some(src),
                _ => None,
            })
            .min()
    };
    let class = match role {
        Role::Side(src) if first_side() == Some(src) => 0,
        Role::CapStart(_) => 1,
        Role::CapEnd(_) => 2,
        Role::Side(_) => 3,
        _ => 4,
    };
    (position, class, face.clone())
}

/// The face each edge's Distance 1 (or the distance of Distance and angle) is
/// measured on, as Onshape picks it (live G30/G37): the edge's first face in
/// Onshape's entity order; along a chain of connected edges the chain's first
/// edge decides and the others keep to the same side of it; a direction
/// override flips its whole chain, Opposite direction every edge.
fn distance_one_faces(ev: &EvalCtx, body: &crate::regen::context::Body, edges: &[usize], overrides: &BTreeSet<usize>, opposite: bool) -> FResult<Vec<k::Shape>> {
    let adjacency = body.shape.edge_faces()?;
    let faces: Vec<[usize; 2]> = edges
        .iter()
        .filter_map(|&e| match adjacency.get(e).map(|f| f.as_slice()) {
            Some(&[a, b]) if a != b => Some([a as usize, b as usize]),
            _ => None,
        })
        .collect();
    if faces.len() != edges.len() {
        // A sheet's boundary edges: the kernel's default.
        return Ok(Vec::new());
    }
    let key = |f: usize| face_order(ev, body, &body.faces[f]);
    let ends: Vec<[glam::DVec3; 2]> = edges
        .iter()
        .map(|&e| body.shape.sub_shape(ShapeType::Edge, e).and_then(|s| s.edge_info()).map(|i| [i.start, i.end]))
        .collect::<Result<_, _>>()?;
    let touch = |i: usize, j: usize| ends[i].iter().any(|p| ends[j].iter().any(|q| p.distance(*q) < 1e-7));
    let first = |i: usize| if key(faces[i][0]) <= key(faces[i][1]) { 0 } else { 1 };
    let mut side: Vec<Option<usize>> = vec![None; edges.len()];
    let mut order: Vec<usize> = (0..edges.len()).collect();
    // Edges in Onshape's order, approximated by their faces' order.
    order.sort_by_cached_key(|&i| {
        let (a, b) = (key(faces[i][0]), key(faces[i][1]));
        if a <= b { (a, b) } else { (b, a) }
    });
    for &start in &order {
        if side[start].is_some() {
            continue;
        }
        side[start] = Some(first(start));
        let mut chain = vec![start];
        let mut stack = vec![start];
        while let Some(i) = stack.pop() {
            let mine = faces[i][side[i].unwrap_or(0)];
            for j in 0..edges.len() {
                if side[j].is_some() || !touch(i, j) {
                    continue;
                }
                // Through the face the two edges share: the same side.
                let shared = faces[i].iter().find(|f| faces[j].contains(f));
                side[j] = Some(match shared {
                    Some(&s) => {
                        let on_shared = mine == s;
                        let at = faces[j].iter().position(|&f| f == s).unwrap_or(0);
                        if on_shared { at } else { 1 - at }
                    }
                    None => first(j),
                });
                chain.push(j);
                stack.push(j);
            }
        }
        let flips = chain.iter().filter(|&&i| overrides.contains(&edges[i])).count();
        if flips % 2 == 1 {
            for &i in &chain {
                side[i] = side[i].map(|s| 1 - s);
            }
        }
    }
    (0..edges.len())
        .map(|i| {
            let s = side[i].unwrap_or(0);
            let s = if opposite { 1 - s } else { s };
            Ok(body.shape.sub_shape(ShapeType::Face, faces[i][s])?)
        })
        .collect()
}

pub fn shell(ev: &mut EvalCtx) -> FResult<()> {
    let t = ev.length("thickness")?;
    if t <= 0.0 {
        return err("Shell thickness must be positive");
    }
    let side = if ev.bool("oppositeDirection") { k::ShellSide::Outside } else { k::ShellSide::Inside };
    let mut groups: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    if ev.bool("isHollow") {
        for bi in ev.resolve_bodies(ev.query("parts")) {
            groups.insert(bi, vec![]);
        }
    } else {
        for (bi, fi) in ev.resolve_faces(ev.query("entities")) {
            groups.entry(bi).or_default().push(fi);
        }
    }
    if groups.is_empty() {
        // std shell.fs: SHELL_SELECT_PARTS / SHELL_SELECT_FACES, the field marked.
        return if ev.bool("isHollow") { crate::regen::eval::err_at(&["parts"], "Select parts to hollow.") } else { crate::regen::eval::err_at(&["entities"], "Select faces to remove.") };
    }
    for (bi, faces) in groups.into_iter().rev() {
        let body = ev.ctx.bodies[bi].clone();
        let shapes: Vec<k::Shape> = faces.iter().filter_map(|f| body.shape.sub_shape(ShapeType::Face, *f).ok()).collect();
        // Like Parasolid: offsets extend until they meet, vanishing faces
        // disappear and overlapping walls merge (crates/kernel/cpp/shell.cpp).
        let res = k::shell_solid(&body.shape, &shapes, t, side, k::ShellMethod::Auto).map_err(|e| blend_failed("Could not shell part with selections.", e))?;
        // Live (qa5 P1-5): a shell that breaks the part into pieces fails
        // (and leaves the part as it was), it never returns the slivers.
        if res.shape.count(ShapeType::Solid) > body.shape.count(ShapeType::Solid).max(1) || !shell_is_whole(&body.shape, &shapes, &res.shape, t) {
            return err("Could not shell part with selections.");
        }
        let names = body.shape_names();
        let offset_role = |n: &Name| Role::Offset(n.clone());
        let face_names = ev.name_result(&[&names], &res.history, &res.shape, &BTreeMap::new(), Some(&offset_role));
        ev.replace_body(bi, res.shape, face_names);
    }
    Ok(())
}

/// Whether a shell result is the whole shelled part, not what is left of a
/// broken one (macOS arm64 builds a single 101.7 mm³ sliver for the part of
/// qa_regressions::shell_that_breaks_the_part_fails where Linux fails): no
/// piece is thinner than the wall (its box is at least `t` every way, as a
/// wall of thickness `t` is), and every face the shell keeps still lies on
/// the result (a point inside it, within a thousandth of the wall).
fn shell_is_whole(part: &k::Shape, removed: &[k::Shape], result: &k::Shape, t: f64) -> bool {
    let thin = result.solids().iter().any(|s| {
        s.bounding_box(false).is_ok_and(|b| {
            let d = b.max - b.min;
            d.x.min(d.y).min(d.z) < 0.99 * t
        })
    });
    if thin {
        return false;
    }
    part.faces().iter().filter(|f| !removed.iter().any(|r| r.is_same(f))).all(|f| {
        let Some(p) = super::helpers_a::point_on_face(f) else { return true };
        k::make_vertex(p).and_then(|v| k::distance(&v, result)).is_ok_and(|d| d.distance <= t * 1e-3)
    })
}

/// Full round fillet: the center faces between two parallel planar side
/// faces become a half cylinder tangent to both sides (radius = half the
/// distance between them).
fn full_round(ev: &mut EvalCtx) -> FResult<()> {
    use super::sheetmetal_util::{bool_named, body_named, oriented_box, plane_of, tool_named};
    use crate::reference::TopoName;
    use std::sync::Arc;
    let s1 = ev.resolve_faces(ev.query("side1Face"));
    let s2 = ev.resolve_faces(ev.query("side2Face"));
    let center = ev.resolve_faces(ev.query("centerFaces"));
    // Onshape's opFullRoundFillet messages (FULL_ROUND_SELECT_FACES,
    // FULL_ROUND_COMMON_BODY); shapes it handles but we don't fail as
    // FILLET_FAILED.
    const UNSUPPORTED: &str = "Failed to fillet selections.";
    let (Some(&(b1, f1)), Some(&(b2, f2))) = (s1.first(), s2.first()) else { return err("Select a first side face, a second side face, and one or more faces to round.") };
    if center.is_empty() {
        return err("Select a first side face, a second side face, and one or more faces to round.");
    }
    if b1 != b2 || center.iter().any(|c| c.0 != b1) {
        return err("Cannot use faces from more than one part or surface.");
    }
    let bi = b1;
    let body = ev.ctx.bodies[bi].clone();
    let face = |f: usize| body.shape.sub_shape(ShapeType::Face, f);
    let (Some((p1, n1)), Some((p2, n2))) = (plane_of(&face(f1)?), plane_of(&face(f2)?)) else {
        return err(UNSUPPORTED);
    };
    if n1.dot(n2) > -1.0 + 1e-7 {
        return err(UNSUPPORTED);
    }
    let w = (p2 - p1).dot(n1).abs();
    if w < 1e-9 {
        return err(UNSUPPORTED);
    }
    let r = w * 0.5;
    let cf = face(center[0].1)?;
    let Some((cc, nc)) = plane_of(&cf) else { return err(UNSUPPORTED) };
    if nc.dot(n1).abs() > 1e-6 {
        return err(UNSUPPORTED);
    }
    let e = nc.cross(n1).normalize();
    // Axis: mid-plane between the sides, R below the center face.
    let mid = p1 - n1 * (w * 0.5);
    let c_mid = cc - n1 * (cc - mid).dot(n1);
    let axis = c_mid - nc * r;
    // Extent along the axis: the center faces.
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    for &(_, f) in &center {
        for v in face(f)?.vertices() {
            let d = (v.vertex_point()? - axis).dot(e);
            lo = lo.min(d);
            hi = hi.max(d);
        }
    }
    let margin = w * 1e-3;
    let ez = e.cross(n1);
    let zr = if ez.dot(nc) > 0.0 { [0.0, 2.0 * r] } else { [-2.0 * r, 0.0] };
    let block = oriented_box(axis, e, n1, [lo - margin, hi + margin], [-r - margin, r + margin], zr)?;
    let cyl = k::make_cylinder(&k::Plane::new(axis + e * (lo - 2.0 * margin), e, n1), r, hi - lo + 4.0 * margin, std::f64::consts::TAU)?;
    let tool = k::cut(&block, &cyl)?.shape;
    let feature = ev.feature;
    let key = body.faces[center[0].1].clone();
    let tool = tool_named(tool, |i| Arc::new(TopoName::face(feature, Role::Blend(Arc::new(TopoName::Split { parent: key.clone(), by: feature, ordinal: i as u16 })))));
    let (shape, names) = bool_named(ev, k::BooleanOp::Cut, &body_named(&body), &[tool])?;
    let un = k::unify_same_domain(&shape)?;
    let rn = crate::regen::eval::names_for(&shape, names);
    let names = ev.name_result(&[&rn], &un.history, &un.shape, &BTreeMap::new(), None);
    ev.replace_body(bi, un.shape, names);
    Ok(())
}

#[cfg(test)]
mod tests {
    /// Onshape G6, F4: x=0 → R1, 0.5 → R6, x=40 → R4, smooth off, sampled at x mm.
    #[test]
    fn variable_law_like_onshape() {
        let law = super::variable_law((1.0, 4.0), &mut vec![(0.5, 6.0)], false);
        let at = |law: &[(f64, f64)], t: f64| {
            let i = law.windows(2).position(|w| t <= w[1].0).unwrap_or(law.len() - 2);
            let (a, b) = (law[i], law[i + 1]);
            a.1 + (b.1 - a.1) * (t - a.0) / (b.0 - a.0)
        };
        for (x, r) in [(2.0, 1.7474), (5.0, 2.8353), (10.0, 4.4370), (15.0, 5.5697), (20.0, 5.9996), (25.0, 5.8276), (30.0, 5.3743), (35.0, 4.7338), (38.0, 4.2989)] {
            assert!((at(&law, x / 40.0) - r).abs() < 0.003, "x={x}: {} vs {r}", at(&law, x / 40.0));
        }
        // G6/G3(d): 0.25 → R3, the second span straight.
        let law = super::variable_law((1.0, 4.0), &mut vec![(0.25, 3.0)], false);
        for (x, r) in [(2.0, 1.4537), (5.0, 2.2079), (8.0, 2.8129), (10.0, 2.9997), (12.0, 3.0667), (20.0, 3.3333), (38.0, 3.9333)] {
            assert!((at(&law, x / 40.0) - r).abs() < 0.003, "x={x}: {} vs {r}", at(&law, x / 40.0));
        }
    }
}
