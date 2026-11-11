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
/// tangentially into an edge left out on one of its faces (Onshape G7: one
/// top arc of a filleted corner, alone or with its straight neighbour). A
/// curved edge whose neighbour lies between other faces runs on (live 2227:
/// cf-surface-pattern's ellipse between its top and its nose).
fn curved_edge_left_open(shape: &k::Shape, edges: &[usize]) -> bool {
    let Some(links) = TangentLinks::new(shape) else { return false };
    let Ok(ef) = shape.edge_faces() else { return false };
    let shares_face = |a: usize, b: usize| ef.get(a).zip(ef.get(b)).is_some_and(|(fa, fb)| fa.iter().any(|f| fb.contains(f)));
    edges.iter().any(|&e| {
        let straight = links.edges.get(e).and_then(|x| x.edge_info().ok()).is_some_and(|i| matches!(i.curve, k::Curve::Line { .. }));
        !straight && links.neighbours(e).iter().any(|&o| !edges.contains(&o) && shares_face(e, o))
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
    // (By the vertices' edges: comparing every two edges' vertices was 80 k kernel calls for big-plate's 400 edges.)
    let Ok(ve) = shape.vertex_edges() else { return None };
    let mut picks = vec![0usize; all.len()];
    for &e in edges {
        if let Some(n) = picks.get_mut(e) {
            *n += 1;
        }
    }
    let shared = |es: &Vec<u32>| {
        let mut es = es.clone();
        es.sort_unstable();
        es.dedup();
        es.iter().map(|&e| picks.get(e as usize).copied().unwrap_or(0)).sum::<usize>() >= 2
    };
    if ve.iter().any(shared) {
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
        return crate::regen::eval::err_at(&[if width { "width" } else if circular { "radius" } else { "nonCircularRadius" }], "Error regenerating: a parameter is out of range.");
    }
    if ev.bool("isVariable") && !width && variable_fillet(ev, radius)? {
        return Ok(());
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
                        // A vertex or mate connector's point; a plane (datum or planar face) where the chain crosses it; another
                        // face's centroid.
                        if let Some(p) = ev.resolve_vertex_point(r) {
                            Bound::Point(p)
                        } else if let Some(pl) = ev.resolve_plane(r) {
                            Bound::Plane(pl.origin, pl.normal)
                        } else {
                            let p = ev.resolve_faces(std::slice::from_ref(r)).first().and_then(|&(b, f)| ev.ctx.bodies[b].shape.sub_shape(ShapeType::Face, f).ok()?.centroid().ok());
                            Bound::Point(p.ok_or_else(|| crate::regen::eval::FeatureError::new("Select an entity for the partial fillet end type."))?)
                        }
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
            Some(BlendSpec { section, r1, r2, width: width.then_some(radius * std::f64::consts::SQRT_2), partial, run_on: !ev.bool("tangentPropagation"), cliff: false, sides: None, overflow: false, reference: None })
        } else {
            None
        }
    };
    let faces_only = refs.iter().all(|r| matches!(r, crate::reference::Reference::Face(_)));
    let groups = by_body(sharp_of_picked_faces(ev, refs));
    // Live (qa5 DER13): edges that are all gone read like an empty field.
    if groups.is_empty() {
        return crate::regen::eval::err_at(&["entities"], "Select edges or faces to fillet.");
    }
    for (bi, edges) in groups.into_iter().rev() {
        let body = ev.ctx.bodies[bi].clone();
        let propagate = ev.bool("tangentPropagation");
        // Live 2202 / 2227: picked faces alone, their edges aren't propagated (the result doesn't depend on propagation).
        // With edges picked too, all are (learning-center-michael's Fillet 2: seven edges and a cap face, as live).
        let mut chain = if propagate && !faces_only { propagate_tangent(&body.shape, &edges) } else { edges.clone() };
        chain.extend(edges.iter().filter(|&&e| !chain.contains(&e)).copied().collect::<Vec<_>>());
        chain.sort_unstable();
        // The selection stops where its edges run on tangentially into edges left out.
        let stops = propagate_tangent(&body.shape, &chain).len() > chain.len();
        // Live 1796: Width control takes one unbranched chain of edges per part (open or closed); a corner of three
        // edges, two separate edges or a box's 12 fail.
        if width && !single_chain(&body.shape, &chain) {
            return err("Edges must form single chain per part.");
        }
        let unpropagated = |r: f64| super::blend_section::BlendSpec { section: super::blend_section::Section::Circular, r1: r, r2: r, width: None, partial: None, run_on: true, cliff: false, sides: None, overflow: false, reference: None };
        if stops && curved_edge_left_open(&body.shape, &chain) {
            return err("Adjoining edge not filleted.");
        }
        if let Some(spec) = &special {
            let edges = chain;
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
        if stops {
            // OCCT always runs a fillet on along tangent edges; Onshape stops at the
            // selection (the blend runs straight on into the tangent neighbour).
            if !width && super::blend_section::run_on_rim_blend(ev, bi, &chain, radius)? {
                continue;
            }
            super::blend_section::section_blend(ev, bi, &chain, &unpropagated(radius))?;
            continue;
        }
        let edges = chain;
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
        // Boss rims whose wall is shorter than the round, overflow off: the ball rolls over the wall's far edge.
        if !ev.bool("allowEdgeOverflow") && super::blend_section::cliff_rim_blend(ev, bi, &edges, &radii)? {
            continue;
        }
        // A tube's round whose ball would touch it past its end: over the end's rim, as live's.
        if !ev.bool("allowEdgeOverflow") && !width && super::blend_section::cliff_end_round(ev, bi, &edges, radius)? {
            continue;
        }
        // Whole circles between faces of revolution about them, a torus or sphere among them: exact toroidal rounds.
        if !width && super::blend_section::coaxial_round(ev, bi, &edges, radius)? {
            continue;
        }
        // Rims whose rounds run off their plane or into each other: each round trimmed by what it meets, as live's.
        use super::blend_section::{RimBlend, ring_blend, rims, rims_overflow};
        let rims = rims(&body.shape, &edges);
        let overflow = rims.as_ref().is_some_and(|r| rims_overflow(r, &body.shape, &radii, RimBlend::Round));
        if overflow && rims.as_ref().is_some_and(|r| ring_blend(ev, bi, r, &edges, &radii, RimBlend::Round).is_ok()) {
            continue;
        }
        // noshape's blend takes some of the edge groups but not all (knob-jason's Fillet 1: arcs it rounds exactly,
        // hole rims on cones it doesn't): those first, OCCT's for the rest on that result, each named in turn.
        if let Some((first, rest)) = rust_groups(&body.shape, &shapes, &radii)
            && let Ok(r1) = k::fillet_checked(&body.shape, &first.0, &first.1)
            && let Some(rest_edges) = rest.0.iter().map(|e| r1.shape.edges().into_iter().find(|x| x.is_same(e))).collect::<Option<Vec<_>>>()
            && let Ok(r2) = k::fillet_checked(&r1.shape, &rest_edges, &rest.1)
        {
            let names1 = ev.name_result(&[&body.shape_names()], &r1.history, &r1.shape, &BTreeMap::new(), Some(&blend_role));
            let names2 = ev.name_result(&[&crate::regen::eval::names_for(&r1.shape, names1)], &r2.history, &r2.shape, &BTreeMap::new(), Some(&blend_role));
            ev.replace_body(bi, r2.shape, names2);
            continue;
        }
        let res = match k::fillet_checked(&body.shape, &shapes, &radii) {
            Ok(r) => r,
            // Without propagation a single straight edge whose ball fits its faces
            // can still be filleted by booleans (F3: R4 after R2/R3 stopped at their edges).
            Err(_) if !propagate && edges.len() == 1 && super::blend_section::section_blend(ev, bi, &edges, &unpropagated(radius)).is_ok() => continue,
            Err(_) if !overflow && rims.as_ref().is_some_and(|r| ring_blend(ev, bi, r, &edges, &radii, RimBlend::Round).is_ok()) => continue,
            // Concave lines whose round runs off their ends over other faces, overflow on: run on, trimmed by those
            // faces extended (c2-hunting-knife's Fillet 2, where OCCT's patch over the grind face leaves a bad solid).
            Err(_) if !width && ev.bool("allowEdgeOverflow") && all_lines(&shapes) && super::blend_section::section_blend(ev, bi, &edges, &super::blend_section::BlendSpec { overflow: true, ..unpropagated(radius) }).is_ok() => continue,
            Err(e) => return Err(fillet_failed(e)),
        };
        let t1 = std::time::Instant::now();
        let names = body.shape_names();
        let face_names = ev.name_result(&[&names], &res.history, &res.shape, &BTreeMap::new(), Some(&blend_role));
        let (shape, face_names) = match merged_blends(&res) {
            Some(un) => {
                let merged = ev.name_result(&[&crate::regen::eval::names_for(&res.shape, face_names)], &un.history, &un.shape, &BTreeMap::new(), None);
                (un.shape, merged)
            }
            None => match joined_edges(&res) {
                Some(j) => {
                    let named = ev.name_result(&[&crate::regen::eval::names_for(&res.shape, face_names)], &j.history, &j.shape, &BTreeMap::new(), None);
                    (j.shape, named)
                }
                None => (res.shape, face_names),
            },
        };
        let t2 = std::time::Instant::now();
        ev.replace_body(bi, shape, face_names);
        log::debug!("fillet: kernel {:?}, naming {:?}, body {:?} (history records {})", t1 - t0, t2 - t1, t2.elapsed(), res.history.records.len());
    }
    Ok(())
}

/// Whether every edge is a straight line.
fn all_lines(edges: &[k::Shape]) -> bool {
    edges.iter().all(|e| e.edge_info().is_ok_and(|i| matches!(i.curve, k::Curve::Line { .. })))
}

/// Edges and their radii.
type EdgeSet = (Vec<k::Shape>, Vec<f64>);

/// When noshape's blend declines the whole fillet but takes some of its edge groups (edges joined through shared
/// vertices): those groups, and the rest. None when it takes all or none, or there's one group.
fn rust_groups(shape: &k::Shape, edges: &[k::Shape], radii: &[f64]) -> Option<(EdgeSet, EdgeSet)> {
    if edges.len() < 2 || k::fillet_rust_accepts(shape, edges, radii) {
        return None;
    }
    let radius = |i: usize| radii.get(i).or(radii.first()).copied().unwrap_or(0.0);
    let verts: Vec<Vec<k::Shape>> = edges.iter().map(|e| e.vertices()).collect();
    let mut group: Vec<usize> = (0..edges.len()).collect();
    fn root(g: &mut [usize], mut i: usize) -> usize {
        while g[i] != i {
            g[i] = g[g[i]];
            i = g[i];
        }
        i
    }
    for i in 0..edges.len() {
        for j in i + 1..edges.len() {
            if verts[i].iter().any(|a| verts[j].iter().any(|b| a.is_same(b))) {
                let (a, b) = (root(&mut group, i), root(&mut group, j));
                group[a] = b;
            }
        }
    }
    let roots: Vec<usize> = (0..edges.len()).map(|i| root(&mut group, i)).collect();
    let mut ids: Vec<usize> = roots.clone();
    ids.sort_unstable();
    ids.dedup();
    if ids.len() < 2 {
        return None;
    }
    let set = |pick: &dyn Fn(usize) -> bool| -> EdgeSet {
        let idx: Vec<usize> = (0..edges.len()).filter(|&i| pick(i)).collect();
        (idx.iter().map(|&i| edges[i].clone()).collect(), idx.iter().map(|&i| radius(i)).collect())
    };
    let taken: Vec<usize> = ids
        .into_iter()
        .filter(|&g| {
            let (es, rs) = set(&|i| roots[i] == g);
            k::fillet_rust_accepts(shape, &es, &rs)
        })
        .collect();
    let first = set(&|i| taken.contains(&roots[i]));
    let rest = set(&|i| !taken.contains(&roots[i]));
    (!first.0.is_empty() && !rest.0.is_empty() && k::fillet_rust_accepts(shape, &first.0, &first.1)).then_some((first, rest))
}

/// Blends that meet on one surface are one face, as Parasolid's (pulley02: a groove's two rounds, each half its
/// height, meet on one torus): the result unified when only blend faces merge, every other face as it was.
/// A blend's result with the pieces of its loops joined (stef-radijator's chamfered hole: its bottom arcs kept the
/// flats' corners as vertices; live's are whole), where any were.
fn joined_edges(res: &k::OpResult) -> Option<k::OpResult> {
    let j = k::join_edges(&res.shape).ok()?;
    (j.shape.count(k::ShapeType::Edge) < res.shape.count(k::ShapeType::Edge)).then_some(j)
}

fn merged_blends(res: &k::OpResult) -> Option<k::OpResult> {
    let faces = res.shape.faces();
    let blend: Vec<bool> = (0..faces.len())
        .map(|i| res.history.records.iter().any(|r| r.relation == k::Relation::Generated && r.output_kind == k::ShapeType::Face && r.output_index as usize == i))
        .collect();
    if blend.iter().filter(|b| **b).count() < 2 {
        return None;
    }
    let un = k::unify_same_domain(&res.shape).ok()?;
    if un.shape.faces().len() >= faces.len() {
        return None;
    }
    // Each result face's faces before (through the unify's history): only blends may share one.
    let mut from: BTreeMap<u32, Vec<usize>> = BTreeMap::new();
    for r in &un.history.records {
        if r.input_kind == k::ShapeType::Face && r.output_kind == k::ShapeType::Face && matches!(r.relation, k::Relation::Modified | k::Relation::Kept) {
            from.entry(r.output_index).or_default().push(r.input_index as usize);
        }
    }
    let merged_only_blends = from.values().filter(|ins| ins.len() > 1).all(|ins| ins.iter().all(|&i| blend.get(i).copied().unwrap_or(false)));
    merged_only_blends.then_some(un)
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
    let (knots, slope) = law_hermite(ends, points, smooth);
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

/// The variable law as cubic Hermite spans: its points (fraction, radius), from 0 to 1, and the slope at each.
pub(super) fn law_hermite(ends: (f64, f64), points: &mut Vec<(f64, f64)>, smooth: bool) -> super::blend_section::RadiusLaw {
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
    (knots, slope)
}



/// The edge's normalized parameter (0 at its start) at `p` when `p` lies on it (within 1e-6 of its length).
fn param_on_edge(edge: &k::Shape, info: &k::EdgeInfo, p: glam::DVec3) -> Option<f64> {
    let (t0, t1) = (info.first_param, info.last_param);
    let at = |f: f64| edge.point_at(t0 + (t1 - t0) * f).ok().map_or(f64::INFINITY, |q| (q - p).length());
    let n = 256;
    let k = (0..=n).min_by(|a, b| at(*a as f64 / n as f64).total_cmp(&at(*b as f64 / n as f64)))?;
    let (mut lo, mut hi) = (((k as f64) - 1.0).max(0.0) / n as f64, ((k as f64) + 1.0).min(n as f64) / n as f64);
    for _ in 0..80 {
        let (a, b) = (lo + (hi - lo) / 3.0, hi - (hi - lo) / 3.0);
        if at(a) < at(b) { hi = b } else { lo = a }
    }
    let f = 0.5 * (lo + hi);
    (at(f) <= 1e-6 * info.length.max(1e-9)).then_some(f)
}

/// Variable fillet: Onshape's `vertexSettings` (vertex + radius) and
/// `pointOnEdgeSettings` (edge, position, radius); a plain `vertices` query
/// takes the main radius. Edges are filleted one at a time, found again by name.
/// The edges a fillet's picks stand for: the edges picked, and a picked face's edges but those where it meets its
/// neighbour tangentially (live 2202: cf-surface-pattern's Fillet 1 leaves the picked top's 0° edge to Extrude 6's cut
/// face as it is and blends its sharp ones).
fn sharp_of_picked_faces(ev: &EvalCtx, refs: &[crate::reference::Reference]) -> Vec<(usize, usize)> {
    let edges: Vec<crate::reference::Reference> = refs.iter().filter(|r| !matches!(r, crate::reference::Reference::Face(_))).cloned().collect();
    let picked = ev.resolve_edges(&edges);
    let mut smooth: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    let mut is_smooth = |bi: usize, ei: usize| -> bool {
        let body = &ev.ctx.bodies[bi];
        if let std::collections::btree_map::Entry::Vacant(v) = smooth.entry(bi) {
            let faces = body.shape.faces();
            let ef = body.shape.edge_faces().unwrap_or_default();
            v.insert((0..ef.len()).filter(|&e| super::detail_a::edge_normals(&body.shape, &faces, &ef, e).is_some_and(|(a, b)| a.dot(b) > 1.0 - 1e-9)).collect());
        }
        smooth[&bi].contains(&ei)
    };
    ev.resolve_edges(refs).into_iter().filter(|&(bi, ei)| picked.contains(&(bi, ei)) || !is_smooth(bi, ei)).collect()
}

/// A variable fillet; false where it comes to the constant fillet (every radius the default: live 2325 A's plain
/// cylinder), left to the caller.
fn variable_fillet(ev: &mut EvalCtx, radius: f64) -> FResult<bool> {
    use crate::params::NumKind;
    let mut at_vertex: Vec<(glam::DVec3, f64)> = Vec::new();
    for item in ev.params.array("vertexSettings") {
        let Some(r) = item.query("vertex").first() else { continue };
        let Some(p) = ev.resolve_vertex_point(r) else { return crate::regen::eval::err_at(&["vertexSettings"], "Select vertices on fillet chain.") };
        at_vertex.push((p, ev.eval_num_in(item, "vertexRadius", NumKind::Length)?));
    }
    let settings = at_vertex.len();
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
    // Live 2325 A: vertex settings off the chain are dropped (a warning); a closed chain needs two of them, else the
    // whole fillet is the constant one (a warning).
    {
        let mut ends: Vec<(k::Shape, k::EdgeInfo)> = Vec::new();
        let mut closed = false;
        for (bi, edges) in &groups {
            let body = &ev.ctx.bodies[*bi];
            let edges = if ev.bool("tangentPropagation") { propagate_tangent(&body.shape, edges) } else { edges.clone() };
            let mut infos = Vec::new();
            for &e in &edges {
                let edge = body.shape.sub_shape(ShapeType::Edge, e)?;
                let info = edge.edge_info()?;
                infos.push(info.clone());
                ends.push((edge, info));
            }
            let count = |p: glam::DVec3| infos.iter().map(|i| [i.start, i.end].iter().filter(|q| (**q - p).length() < 1e-9).count()).sum::<usize>();
            closed |= !infos.is_empty() && infos.iter().all(|i| count(i.start) == 2 && count(i.end) == 2);
        }
        let on_chain = |p: glam::DVec3| ends.iter().any(|(e, i)| (p - i.start).length() < 1e-9 || (p - i.end).length() < 1e-9 || param_on_edge(e, i, p).is_some());
        let before = at_vertex.len();
        at_vertex.retain(|(p, _)| on_chain(*p));
        if at_vertex.len() < before {
            ev.warnings.push("Some selected vertices are not on an edge chain.".into());
        }
        if closed && settings == 1 {
            ev.warnings.push("At least two vertices need to be selected for variable fillet on closed edge chains.".into());
            at_vertex.clear();
            on_edge.clear();
        }
        let default = |r: f64| (r - radius).abs() <= 1e-12 * radius;
        if at_vertex.iter().all(|v| default(v.1)) && on_edge.iter().all(|p| default(p.2)) {
            return Ok(false);
        }
    }
    let radius_at = |p: glam::DVec3| at_vertex.iter().find(|(q, _)| (*q - p).length() < 1e-9).map(|v| v.1).unwrap_or(radius);
    for (bi, edges) in groups.into_iter().rev() {
        let body = ev.ctx.bodies[bi].clone();
        let edges = if ev.bool("tangentPropagation") { propagate_tangent(&body.shape, &edges) } else { edges };
        // Each edge's law on the part as it is (for the section route, where OCCT's fails).
        let mut laws: Vec<(usize, super::blend_section::RadiusLaw)> = Vec::new();
        for &e in &edges {
            let edge = body.shape.sub_shape(ShapeType::Edge, e)?;
            let info = edge.edge_info()?;
            let name = body.edges[e].clone();
            let key = |p: glam::DVec3| (p.x, p.y, p.z);
            let flip = key(info.end).partial_cmp(&key(info.start)) == Some(std::cmp::Ordering::Less);
            let mut points: Vec<(f64, f64)> = on_edge.iter().filter(|(n, _, _)| *n == name).map(|(_, t, r)| (if flip { 1.0 - *t } else { *t }, *r)).collect();
            for (p, r) in &at_vertex {
                if (*p - info.start).length() > 1e-9 && (*p - info.end).length() > 1e-9
                    && let Some(t) = param_on_edge(&edge, &info, *p)
                {
                    points.push((t, *r));
                }
            }
            laws.push((e, law_hermite((radius_at(info.start), radius_at(info.end)), &mut points, smooth)));
        }
        // A round wider than a face beside it: OCCT's fails there (after ~70 ms), the exact one first.
        if laws.iter().any(|(e, law)| super::blend_section::variable_round_overflows(&body.shape, *e, law)) {
            if super::blend_section::variable_round(ev, bi, &laws).is_ok() {
                continue;
            }
            ev.ctx.bodies[bi] = body.clone();
        }
        let names: Vec<Name> = edges.iter().map(|&e| body.edges[e].clone()).collect();
        let occt = (|| -> FResult<()> {
        for name in names {
            let body = ev.ctx.bodies[bi].clone();
            let Some(&e) = body.find_edges(&name).first() else { continue };
            let edge = body.shape.sub_shape(ShapeType::Edge, e)?;
            let info = edge.edge_info()?;
            // Onshape's position 0 is the edge's end nearest the origin corner (lowest x, y, z).
            let key = |p: glam::DVec3| (p.x, p.y, p.z);
            let flip = key(info.end).partial_cmp(&key(info.start)) == Some(std::cmp::Ordering::Less);
            let mut points: Vec<(f64, f64)> = on_edge.iter().filter(|(n, _, _)| *n == name).map(|(_, t, r)| (if flip { 1.0 - *t } else { *t }, *r)).collect();
            // A vertex setting on a point inside the edge (a sketch point on it: c2-tpd-fillet-variable's ten) is a
            // point on the edge there, as live takes it.
            for (p, r) in &at_vertex {
                if (*p - info.start).length() > 1e-9 && (*p - info.end).length() > 1e-9
                    && let Some(t) = param_on_edge(&edge, &info, *p)
                {
                    points.push((t, *r));
                }
            }
            let law = variable_law((radius_at(info.start), radius_at(info.end)), &mut points, smooth);
            let res = k::fillet_variable_checked(&body.shape, &edge, &law).map_err(|e| blend_failed("Failed to fillet selections.", e))?;
            let names = body.shape_names();
            let face_names = ev.name_result(&[&names], &res.history, &res.shape, &BTreeMap::new(), Some(&blend_role));
            ev.replace_body(bi, res.shape, face_names);
        }
        Ok(())
        })();
        // OCCT's failed (a round wider than its walls: c2-tpd-fillet-variable): the part as it was, the rounds as cuts.
        if let Err(e) = occt {
            ev.ctx.bodies[bi] = body.clone();
            super::blend_section::variable_round(ev, bi, &laws).map_err(|_| e)?;
        }
    }
    Ok(true)
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
        let stops = !propagate && propagate_tangent(&body.shape, &edges).len() > edges.len();
        let edges = if propagate { propagate_tangent(&body.shape, &edges) } else { edges };
        let shapes: Vec<k::Shape> = edges.iter().filter_map(|e| body.shape.sub_shape(ShapeType::Edge, *e).ok()).collect();
        let flipped: BTreeSet<usize> = overrides.iter().filter(|(b, _)| *b == bi).map(|&(_, e)| e).collect();
        use super::blend_section::ChamferSides;
        let sides = match (ev.enum_("chamferMethod") == "APEX_RANGE", spec) {
            (true, k::ChamferSpec::Equal(d)) => Some(ChamferSides::Tangent(d, d)),
            (true, k::ChamferSpec::TwoDistances(a, b)) => Some(ChamferSides::Tangent(a, b)),
            (true, k::ChamferSpec::DistanceAngle(d, angle)) => Some(ChamferSides::TangentAngle(d, angle)),
            (false, k::ChamferSpec::DistanceAngle(d, angle)) => Some(ChamferSides::OffsetAngle(d, angle)),
            (false, _) => None,
        };
        let as_offsets = match sides {
            Some(sides) => two_offsets(ev, &body, &edges, sides, &flipped)?,
            None => None,
        };
        let (spec, reference) = match as_offsets {
            Some(offsets) => offsets,
            None if matches!(spec, k::ChamferSpec::Equal(_)) => (spec, Vec::new()),
            None => (spec, distance_one_faces(ev, &body, &edges, &flipped, ev.bool("oppositeDirection"))?),
        };
        // OCCT runs a chamfer on along tangent edges; Onshape stops at the selection and runs the section out onto the
        // neighbour (G7). (Two distances: one edge, its distance 1 on the reference face.)
        let distances = match spec {
            _ if !stops => None,
            k::ChamferSpec::Equal(d) => Some((d, d)),
            k::ChamferSpec::TwoDistances(a, b) if edges.len() == 1 => {
                let first = body.shape.edge_faces()?.get(edges[0]).and_then(|f| f.first().copied()).map(|f| body.shape.sub_shape(ShapeType::Face, f as usize)).transpose()?;
                let on_first = first.is_some_and(|f| reference.first().is_some_and(|r| r.is_same(&f)));
                Some(if on_first { (a, b) } else { (b, a) })
            }
            _ => None,
        };
        if let Some((r1, r2)) = distances {
            let spec = super::blend_section::BlendSpec { section: super::blend_section::Section::Chamfer, r1, r2, width: None, partial: None, run_on: true, cliff: false, sides: None, overflow: false, reference: None };
            super::blend_section::section_blend(ev, bi, &edges, &spec).map_err(|e| blend_failed("Failed to chamfer selections.", e.message))?;
            continue;
        }
        if let k::ChamferSpec::Equal(d) = spec {
            match round_overrun(&body.shape, &edges, &|_, _| d) {
                Some(Overrun::PastSelected) => return err("End boundary intersects unchamfered edge."),
                Some(Overrun::PastUnselected) => return err("Failed to chamfer selections."),
                None => {}
            }
        }
        // Arcs of slanted rims whose chamfers run off their plane: each chamfer as if alone, trimmed by what it meets; tilted
        // rims' chamfers, the chord of one rolling ball (live 2441).
        use super::blend_section::{RimBlend, ring_blend, rims, rims_overflow, rims_tilted};
        if let k::ChamferSpec::Equal(d) = spec
            && let Some(rims) = rims(&body.shape, &edges)
            && (rims_tilted(&rims) || rims_overflow(&rims, &body.shape, &[d], RimBlend::Chamfer))
            && ring_blend(ev, bi, &rims, &edges, &[d], RimBlend::Chamfer).is_ok()
        {
            continue;
        }
        // Where OCCT can't build it (c2-hunting-knife's Chamfer 1: an offset and angle along a blade's B-spline grind
        // face): as sections along the edges, each from its reference face. (A distance-and-angle or two-distance
        // chamfer, sections to fall back on: OCCT's invalid result not retried at slightly smaller sizes, 0.2 s there.)
        use super::blend_section::{BlendSpec, ChamferRef, Section};
        let faces = body.shape.faces();
        let refs: Vec<usize> = reference.iter().filter_map(|r| faces.iter().position(|f| f.is_same(r))).collect();
        let (r1, r2, angle) = match spec {
            k::ChamferSpec::Equal(d) => (d, d, None),
            k::ChamferSpec::TwoDistances(a, b) => (a, b, None),
            k::ChamferSpec::DistanceAngle(d, a) => (d, d, Some(a)),
        };
        let sections = BlendSpec { section: Section::Chamfer, r1, r2, width: None, partial: None, run_on: true, cliff: false, sides: None, overflow: false, reference: Some(ChamferRef { faces: refs, angle, offsets: None }) };
        let quick = !matches!(spec, k::ChamferSpec::Equal(_));
        let first = if quick { k::without_blend_retries(|| k::chamfer_checked(&body.shape, &shapes, spec, &reference)) } else { k::chamfer_checked(&body.shape, &shapes, spec, &reference) };
        let res = match first {
            Ok(r) => r,
            Err(e) => {
                if quick && std::env::var_os("NOSHAPE_BLEND_VERIFY").is_some() && k::chamfer_checked(&body.shape, &shapes, spec, &reference).is_ok() {
                    eprintln!("blend verify: {}: OCCT's retries build the chamfer", ev.name);
                }
                if super::blend_section::section_blend(ev, bi, &edges, &sections).is_ok() {
                    continue;
                }
                match quick.then(|| k::chamfer_checked(&body.shape, &shapes, spec, &reference)) {
                    Some(Ok(r)) => r,
                    _ => return Err(chamfer_failed(e)),
                }
            }
        };
        let names = body.shape_names();
        let face_names = ev.name_result(&[&names], &res.history, &res.shape, &BTreeMap::new(), Some(&blend_role));
        let (shape, face_names) = match joined_edges(&res) {
            Some(j) => {
                let named = ev.name_result(&[&crate::regen::eval::names_for(&res.shape, face_names)], &j.history, &j.shape, &BTreeMap::new(), None);
                (j.shape, named)
            }
            None => (res.shape, face_names),
        };
        ev.replace_body(bi, shape, face_names);
    }
    Ok(())
}

/// A chamfer OCCT reads otherwise than Onshape (see `blend_section::ChamferSides`) as two distances, where its edges' sections are
/// the same along them and all come to the same distances. None where OCCT reads it as Onshape does, or can't be told.
fn two_offsets(ev: &EvalCtx, body: &crate::regen::context::Body, edges: &[usize], sides: super::blend_section::ChamferSides, flipped: &BTreeSet<usize>) -> FResult<Option<(k::ChamferSpec, Vec<k::Shape>)>> {
    let reference = distance_one_faces(ev, body, edges, flipped, ev.bool("oppositeDirection"))?;
    Ok(sides_as_offsets(&body.shape, edges, &reference, sides)?.map(|spec| (spec, reference)))
}

/// A chamfer's sides as Onshape measures them (see `blend_section::ChamferSides`) read as two distances on `reference`
/// (each edge's Distance 1 face) and the other face, where its edges' sections are the same along them and all come to
/// the same distances. None where OCCT reads it as Onshape does, or can't be told.
pub fn sides_as_offsets(shape: &k::Shape, edges: &[usize], reference: &[k::Shape], sides: super::blend_section::ChamferSides) -> FResult<Option<k::ChamferSpec>> {
    if reference.len() != edges.len() {
        return Ok(None);
    }
    let adjacency = shape.edge_faces()?;
    let mut offsets: Option<[f64; 2]> = None;
    for (&e, r) in edges.iter().zip(reference) {
        let Some(&[a, b]) = adjacency.get(e).map(|f| f.as_slice()) else { return Ok(None) };
        let (fa, fb) = (shape.sub_shape(ShapeType::Face, a as usize)?, shape.sub_shape(ShapeType::Face, b as usize)?);
        let other = if fa.is_same(r) { fb } else { fa };
        let d = match super::blend_section::chamfer_offsets(shape, e, [r, &other], sides)? {
            Some(Some(d)) => d,
            // OCCT reads it as Onshape does (a mix of edges: OCCT's reading for now).
            _ => return Ok(None),
        };
        if offsets.is_some_and(|o| (o[0] - d[0]).abs().max((o[1] - d[1]).abs()) > 1e-9 * d[0].max(d[1])) {
            return Ok(None);
        }
        offsets = Some(d);
    }
    Ok(offsets.map(|[d0, d1]| k::ChamferSpec::TwoDistances(d0, d1)))
}

/// Onshape's entity order of faces (its deterministic ids, live G37): faces of
/// earlier features first; within a feature that sweeps profiles, the side of
/// the first profile segment, the start cap, the end cap, then the other sides
/// in profile order (G30's block drawn (0,0)→(40,0)→(40,20)→(0,20): front,
/// bottom, top, right, back, left). Other faces follow in name order.
pub(crate) fn face_order(ev: &EvalCtx, body: &crate::regen::context::Body, face: &Name) -> (usize, u8, Name) {
    face_order_in(ev, body, face, None)
}

/// [`face_order`], each profile loop's first side its own (`adjacency`: the body's edge faces; the sides of one loop
/// are those beside the caps this side borders): a feature's several pins each measure from their side (live
/// c2-fan-blade-sweep's connector: four 0.7 mm pins, Chamfer 1's 0.4 mm down each pin's sides).
fn face_order_in(ev: &EvalCtx, body: &crate::regen::context::Body, face: &Name, adjacency: Option<&[Vec<u32>]>) -> (usize, u8, Name) {
    use crate::reference::TopoName;
    let TopoName::Face { feature, role } = face.root() else { return (usize::MAX, 4, face.clone()) };
    let position = ev.features.iter().position(|f| f.id == *feature).unwrap_or(usize::MAX);
    let side_of = |n: &Name| match n.root() {
        TopoName::Face { feature: f, role: Role::Side(src) } if f == feature => Some(src.clone()),
        _ => None,
    };
    let is_cap = |n: &Name| matches!(n.root(), TopoName::Face { feature: f, role: Role::CapStart(_) | Role::CapEnd(_) } if f == feature);
    let first_side = || {
        let loop_sides = adjacency.and_then(|adj| {
            let me = body.faces.iter().position(|n| n == face)?;
            let beside = |f: usize| -> Vec<usize> {
                adj.iter().filter(|fs| fs.contains(&(f as u32))).flat_map(|fs| fs.iter().map(|&g| g as usize)).filter(|&g| g != f).collect()
            };
            let caps: Vec<usize> = beside(me).into_iter().filter(|&g| is_cap(&body.faces[g])).collect();
            caps.iter().flat_map(|&c| beside(c)).filter_map(|g| side_of(&body.faces[g])).min()
        });
        loop_sides.or_else(|| body.faces.iter().filter_map(side_of).min())
    };
    let class = match role {
        Role::Side(src) if first_side().as_ref() == Some(src) => 0,
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
    let key = |f: usize| face_order_in(ev, body, &body.faces[f], Some(&adjacency));
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
        return crate::regen::eval::err_at(&["thickness"], "Error regenerating: a parameter is out of range.");
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
        let res = shell_part(&body.shape, &shapes, t, side)?;
        let names = body.shape_names();
        let offset_role = |n: &Name| Role::Offset(n.clone());
        let face_names = ev.name_result(&[&names], &res.history, &res.shape, &BTreeMap::new(), Some(&offset_role));
        ev.replace_body(bi, res.shape, face_names);
    }
    Ok(())
}

/// Whether edges `es` of `shape` are one chain: connected through their vertices, no vertex on more than two of them.
fn single_chain(shape: &k::Shape, es: &[usize]) -> bool {
    fn root(p: &mut [usize], i: usize) -> usize {
        if p[i] == i {
            i
        } else {
            let r = root(p, p[i]);
            p[i] = r;
            r
        }
    }
    let mut degree: BTreeMap<usize, usize> = BTreeMap::new();
    let mut parent: Vec<usize> = (0..es.len()).collect();
    let mut first: BTreeMap<usize, usize> = BTreeMap::new();
    for (i, &e) in es.iter().enumerate() {
        let Ok(edge) = shape.sub_shape(ShapeType::Edge, e) else { return true };
        let mut vs: Vec<usize> = edge.vertices().iter().filter_map(|v| shape.index_of(v, ShapeType::Vertex)).collect();
        vs.sort_unstable();
        vs.dedup();
        for v in vs {
            *degree.entry(v).or_default() += 1;
            match first.get(&v) {
                Some(&j) => {
                    let (a, b) = (root(&mut parent, i), root(&mut parent, j));
                    parent[a] = b;
                }
                None => {
                    first.insert(v, i);
                }
            }
        }
    }
    degree.values().all(|&d| d <= 2) && (0..es.len()).all(|i| root(&mut parent, i) == root(&mut parent, 0))
}

/// Shell `part` (faces `removed`; none: hollow), as Shell and std's opShell do: offsets extend until they meet,
/// vanishing faces disappear and overlapping walls merge (crates/kernel/cpp/shell.cpp); "Could not shell part with
/// selections." when it fails, breaks the part (live qa5 P1-5: never the slivers) or, inward with faces removed, its
/// walls reach half the part or more and nothing is removed (live 1765: a 40 mm cube at 20 / 20.1 / 25 mm; 19.9 mm
/// leaves a 0.2 mm pocket).
pub fn shell_part(part: &k::Shape, removed: &[k::Shape], t: f64, side: k::ShellSide) -> Result<k::OpResult, crate::regen::eval::FeatureError> {
    let failed = || crate::regen::eval::FeatureError::new("Could not shell part with selections.");
    let res = k::shell_solid(part, removed, t, side, k::ShellMethod::Auto).map_err(|e| blend_failed("Could not shell part with selections.", e))?;
    if res.shape.count(ShapeType::Solid) > part.count(ShapeType::Solid).max(1) || !shell_is_whole(part, removed, &res.shape, t) {
        return Err(failed());
    }
    // (Opened where a removed face's own point is off the result: no volumes integrated, 80-170 ms each on a swept
    // hose, dx 11-04; they decide only where every removed face still lies on it, as a strip narrower than the wall.)
    let opened = || {
        removed.iter().any(|f| {
            super::helpers_a::inner_point(f).is_some_and(|p| !on_faces(&res.shape, p, t * 1e-3))
        })
    };
    let volume = |s: &k::Shape| k::volume_properties(s).map_or(0.0, |p| p.mass);
    if side == k::ShellSide::Inside && !removed.is_empty() && !opened() && volume(&res.shape) >= volume(part) * (1.0 - 1e-9) {
        return Err(failed());
    }
    Ok(res)
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
        let Some(p) = super::helpers_a::inner_point(f) else { return true };
        on_faces(result, p, t * 1e-3)
    })
}

/// Whether `p` lies within `tol` of a face of `shape`: the faces whose box holds it, each classified (BRepExtrema's
/// distance to the whole solid took ~25 ms a point on a swept hose: cf-hose-routing's Shell, 128 of its 160 ms).
fn on_faces(shape: &k::Shape, p: glam::DVec3, tol: f64) -> bool {
    shape.faces().iter().any(|f| {
        f.bounding_box(false).is_ok_and(|b| (b.min - glam::DVec3::splat(tol)).cmple(p).all() && p.cmple(b.max + glam::DVec3::splat(tol)).all()) && f.contains_point(p, tol).unwrap_or(false)
    })
}

/// Full round fillet: the center faces between two parallel planar side
/// faces become a half cylinder tangent to both sides (radius = half the
/// distance between them).
fn full_round(ev: &mut EvalCtx) -> FResult<()> {
    use super::sheetmetal_util::plane_of;
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
    let cf = face(center[0].1)?;
    let tool = match cf.face_surface() {
        // A bore or boss between the sides: a half torus (live motor-mount-jack's Fillet 2).
        Ok(k::Surface::Cylinder { origin, axis, x_dir, radius }) if axis.normalize().cross(n1).length() < 1e-9 => {
            if n1.dot(n2) > -1.0 + 1e-7 {
                return err(UNSUPPORTED);
            }
            let w = (p2 - p1).dot(n1).abs();
            if w < 1e-9 {
                return err(UNSUPPORTED);
            }
            let r = w * 0.5;
            let faces: Vec<Shape> = center.iter().map(|c| face(c.1)).collect::<Result<_, _>>()?;
            let Some(tool) = coaxial_round_tool(&faces, k::Plane::new(origin, axis.normalize(), x_dir), radius, [p1, p2], r) else { return err(UNSUPPORTED) };
            // (Its sides and core on the ring's own planes and cylinder: OCCT's cut glued, 64 -> 32 ms.)
            return finish_full_round(ev, bi, &body, center[0].1, tool, true);
        }
        _ => {
            // Planar sides and center along one direction (a rib, drafted or not): every section the circle tangent to
            // the three lines, a cylinder along the rib; the tool the section between the faces and the arc, extruded.
            let Some((cc, nc)) = plane_of(&cf) else { return err(UNSUPPORTED) };
            let Some(e) = n1.cross(nc).try_normalize() else { return err(UNSUPPORTED) };
            if n2.dot(e).abs() > 1e-9 {
                // A rib tapering along its length (live 2306 E): a cone.
                let faces: Vec<Shape> = center.iter().map(|c| face(c.1)).collect::<Result<_, _>>()?;
                let Some(tool) = tapered_round_tool(&faces, [(p1, n1), (p2, n2), (cc, nc)]) else { return err(UNSUPPORTED) };
                return finish_full_round(ev, bi, &body, center[0].1, tool, false);
            }
            // The section's plane: x along nc, y along e x nc, origin on the center face.
            let (ax, ay) = (nc, e.cross(nc));
            let to2 = |p: DVec3| glam::DVec2::new((p - cc).dot(ax), (p - cc).dot(ay));
            let n2d = |n: DVec3| glam::DVec2::new(n.dot(ax), n.dot(ay));
            // n_i . c + r = n_i . p_i for the three outward normals (the centre r inside each).
            let rows = [(n2d(n1), n2d(n1).dot(to2(p1))), (n2d(n2), n2d(n2).dot(to2(p2))), (n2d(nc), 0.0)];
            let m = glam::DMat3::from_cols(
                DVec3::new(rows[0].0.x, rows[1].0.x, rows[2].0.x),
                DVec3::new(rows[0].0.y, rows[1].0.y, rows[2].0.y),
                DVec3::ONE,
            );
            if m.determinant().abs() < 1e-12 {
                return err(UNSUPPORTED);
            }
            let sol = m.inverse() * DVec3::new(rows[0].1, rows[1].1, rows[2].1);
            let (c2, r) = (glam::DVec2::new(sol.x, sol.y), sol.z);
            if r.is_nan() || r <= 1e-9 {
                return err(UNSUPPORTED);
            }
            let to3 = |q: glam::DVec2| cc + ax * q.x + ay * q.y;
            let c3 = to3(c2);
            // Tangent points, and each side's line r past the center face (into the air: the section's top, so the
            // region doesn't pinch where the arc touches the center face).
            let (t1, t2, tc) = (c3 + n1 * r, c3 + n2 * r, c3 + nc * r);
            let meet = |pi: DVec3, ni: DVec3| -> Option<DVec3> {
                let n = n2d(ni);
                (n.y.abs() > 1e-12).then(|| to3(glam::DVec2::new(r, (n.dot(to2(pi)) - n.x * r) / n.y)))
            };
            let (Some(q1), Some(q2)) = (meet(p1, n1), meet(p2, n2)) else { return err(UNSUPPORTED) };
            // Extent along the rib: the center faces' vertices.
            let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
            for &(_, f) in &center {
                for v in face(f)?.vertices() {
                    let d = (v.vertex_point()? - c3).dot(e);
                    lo = lo.min(d);
                    hi = hi.max(d);
                }
            }
            let margin = r * 1e-3;
            let start = |p: DVec3| p + e * (lo - margin);
            let wire = k::make_wire(&[
                k::make_line(start(t1), start(q1))?,
                k::make_line(start(q1), start(q2))?,
                k::make_line(start(q2), start(t2))?,
                k::make_arc_3pts(start(t2), start(tc), start(t1))?,
            ])?;
            let profile = k::make_face_from_wire(&wire, true)?;
            k::extrude(&profile, e * (hi - lo + 2.0 * margin))?.shape
        }
    };
    finish_full_round(ev, bi, &body, center[0].1, tool, false)
}

/// Cuts a full round's tool from the body, its faces named after the (first) center face; `glued` where the tool's
/// faces lie on the body's or don't cross them (OCCT's glue option).
fn finish_full_round(ev: &mut EvalCtx, bi: usize, body: &crate::regen::context::Body, center: usize, tool: Shape, glued: bool) -> FResult<()> {
    use super::sheetmetal_util::{body_named, tool_named};
    use crate::reference::TopoName;
    use std::sync::Arc;
    let feature = ev.feature;
    let key = body.faces[center].clone();
    let tool = tool_named(tool, |i| Arc::new(TopoName::face(feature, Role::Blend(Arc::new(TopoName::Split { parent: key.clone(), by: feature, ordinal: i as u16 })))));
    let part = body_named(body);
    let glue = if glued { k::Glue::Shift } else { k::Glue::Off };
    let res = k::boolean(k::BooleanOp::Cut, std::slice::from_ref(&part.shape), std::slice::from_ref(&tool.shape), &k::BooleanOptions { glue, ..Default::default() })?;
    let names = ev.name_result(&[&part.names, &tool.names], &res.history, &res.shape, &BTreeMap::new(), None);
    let shape = res.shape;
    let un = k::unify_same_domain(&shape)?;
    let rn = crate::regen::eval::names_for(&shape, names);
    let names = ev.name_result(&[&rn], &un.history, &un.shape, &BTreeMap::new(), None);
    ev.replace_body(bi, un.shape, names);
    Ok(())
}

/// The tool a full round takes off a rib whose planar sides and center don't run along one direction: the spheres
/// touching all three planes have their centres on a line from the planes' common point, so the round is the cone
/// they sweep (live 2306 E: a rib 10 → 6 wide, a CONE). The tool is the region above the plane of the cone's two
/// touch lines, between the sides and up past the center, less the cone; its ends past the center faces' vertices.
fn tapered_round_tool(center: &[Shape], planes: [(DVec3, DVec3); 3]) -> Option<Shape> {
    let m = glam::DMat3::from_cols(planes[0].1, planes[1].1, planes[2].1).transpose();
    if m.determinant().abs() < 1e-12 {
        return None;
    }
    let minv = m.inverse();
    let apex = minv * DVec3::new(planes[0].1.dot(planes[0].0), planes[1].1.dot(planes[1].0), planes[2].1.dot(planes[2].0));
    // n_i . c + r = n_i . p_i: c = apex + r w.
    let w = minv * DVec3::splat(-1.0);
    let [(_, n1), (_, n2), (pc, nc)] = planes;
    let along = (w - nc * w.dot(nc)).try_normalize()?;
    // The r at which each touch line passes each center vertex's station along the rib.
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    for f in center {
        for v in f.vertices() {
            let at = (v.vertex_point().ok()? - apex).dot(along);
            for n in [n1, n2] {
                let per_r = (w + n).dot(along);
                if per_r.abs() < 1e-12 {
                    return None;
                }
                lo = lo.min(at / per_r);
                hi = hi.max(at / per_r);
            }
        }
    }
    if lo <= 0.0 {
        return None;
    }
    let margin = (hi - lo) * 1e-3;
    let (lo, hi) = (lo - margin, hi + margin);
    let touch = |n: DVec3, r: f64| apex + w * r + n * r;
    let up = hi;
    // A point on side i, on the plane `up` above the center, in the end plane through both touch points and nc.
    let corner = |n: DVec3, r: f64| -> Option<DVec3> {
        let (a, b) = (touch(n1, r), touch(n2, r));
        let ne = (b - a).cross(nc).try_normalize()?;
        let side_p = touch(n, r);
        let top_p = pc + nc * up;
        let mm = glam::DMat3::from_cols(n, nc, ne).transpose();
        (mm.determinant().abs() > 1e-12).then(|| mm.inverse() * DVec3::new(n.dot(side_p), nc.dot(top_p), ne.dot(a)))
    };
    let (t1l, t2l, t1h, t2h) = (touch(n1, lo), touch(n2, lo), touch(n1, hi), touch(n2, hi));
    let (q1l, q2l, q1h, q2h) = (corner(n1, lo)?, corner(n2, lo)?, corner(n1, hi)?, corner(n2, hi)?);
    let quad = |p: [DVec3; 4]| -> Option<Shape> { k::make_face_from_wire(&k::make_polyline(&p, true).ok()?, true).ok() };
    let faces = vec![
        quad([t1l, q1l, q2l, t2l])?,
        quad([t1h, t2h, q2h, q1h])?,
        quad([t1l, t1h, q1h, q1l])?,
        quad([t2l, q2l, q2h, t2h])?,
        quad([q1l, q1h, q2h, q2l])?,
        quad([t1l, t2l, t2h, t1h])?,
    ];
    let region = k::sew(&faces, 1e-9, true).ok()?.shape;
    // The cone: half angle from sin = 1 / |w|, apex to past both ends.
    let axis = w.normalize();
    let tan = (1.0 / (w.length_squared() - 1.0).max(1e-300)).sqrt();
    let (s0, s1) = (lo * w.length() * 0.5, hi * w.length() * 1.5 + hi);
    // Its seam toward the center (the other way the round came out in two faces).
    let x = (nc - axis * nc.dot(axis)).try_normalize()?;
    let cone = k::make_cone(&k::Plane::new(apex + axis * s0, axis, x), s0 * tan, s1 * tan, s1 - s0, std::f64::consts::TAU).ok()?;
    k::cut(&region, &cone).ok().map(|o| o.shape)
}

/// The tool a full round of a whole cylinder (`faces`, on `cyl`'s axis, radius `rc`) between two planes through
/// `sides` takes off: the ring from the cylinder to the torus' core circle, less the torus of radius `r`; outside the
/// cylinder for a bore, inside for a boss. `None` where the faces don't go all the way round or the boss is too thin.
fn coaxial_round_tool(faces: &[Shape], cyl: k::Plane, rc: f64, sides: [DVec3; 2], r: f64) -> Option<Shape> {
    let turn: f64 = faces.iter().map(|f| f.uv_bounds().map_or(0.0, |b| b[1] - b[0])).sum();
    if turn < std::f64::consts::TAU - 1e-6 {
        return None;
    }
    let p = super::helpers_a::point_on_face(&faces[0])?;
    let (q, n) = super::helpers_a::face_normal_near(&faces[0], p)?;
    let radial = {
        let d = q - cyl.origin;
        d - cyl.normal * d.dot(cyl.normal)
    };
    let bore = n.dot(radial) < 0.0;
    let core = if bore { rc + r } else { rc - r };
    if core <= r * 1e-6 {
        return None;
    }
    let [z1, z2] = sides.map(|s| (s - cyl.origin).dot(cyl.normal));
    let (lo, hi) = (z1.min(z2), z1.max(z2));
    let margin = r * 1e-3;
    // One revolve of its meridian section (the ring's rectangle less the torus' half disc), no Booleans (motor-mount-
    // jack's Fillet 2: two of its three Booleans).
    let at = |rho: f64, z: f64| cyl.origin + cyl.x_dir * rho + cyl.normal * z;
    let mid = 0.5 * (lo + hi);
    let (rect, bulge) = if bore { (rc - margin, core - r) } else { (rc + margin, core + r) };
    let wire = k::make_wire(&[
        k::make_line(at(rect, lo - margin), at(core, lo - margin)).ok()?,
        k::make_line(at(core, lo - margin), at(core, lo)).ok()?,
        k::make_arc_3pts(at(core, lo), at(bulge, mid), at(core, hi)).ok()?,
        k::make_line(at(core, hi), at(core, hi + margin)).ok()?,
        k::make_line(at(core, hi + margin), at(rect, hi + margin)).ok()?,
        k::make_line(at(rect, hi + margin), at(rect, lo - margin)).ok()?,
    ])
    .ok()?;
    let profile = k::make_face_from_wire(&wire, true).ok()?;
    k::revolve(&profile, cyl.origin, cyl.normal, std::f64::consts::TAU).ok().map(|o| o.shape)
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
