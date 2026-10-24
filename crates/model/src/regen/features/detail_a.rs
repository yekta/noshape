//! Draft (neutral plane), Face blend (rolling-ball blend of adjacent face
//! sets), Modify fillet (change radius / remove) and Body draft
//! (unsupported).

use super::helpers_a::*;
use crate::naming::Name;
use crate::reference::{Role, TopoName};
use crate::regen::eval::{EvalCtx, FResult, err};
use glam::DVec3;
use noshape_kernel as k;
use noshape_kernel::{Shape, ShapeType};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

fn blend_role(n: &Name) -> Role {
    Role::Blend(n.clone())
}

/// Unit normals of the two faces of an edge at its midpoint.
fn edge_normals(shape: &Shape, faces: &[Shape], edge_faces: &[Vec<u32>], ei: usize) -> Option<(DVec3, DVec3)> {
    let e = shape.sub_shape(ShapeType::Edge, ei).ok()?;
    let m = edge_mid(&e)?;
    let fs = edge_faces.get(ei)?;
    if fs.len() != 2 {
        return None;
    }
    let a = face_normal_near(faces.get(fs[0] as usize)?, m)?.1;
    let b = face_normal_near(faces.get(fs[1] as usize)?, m)?.1;
    Some((a.normalize(), b.normalize()))
}

// ------------------------------------------------------------------ draft

/// Onshape's DRAFT_FAILED (draft.fs).
const DRAFT_FAILED: &str = "Selected faces could not be drafted.";

pub fn draft(ev: &mut EvalCtx) -> FResult<()> {
    if ev.is("draftFeatureType", "PARTING_LINE") {
        return parting_line_draft(ev);
    }
    let Some(np) = ev.query("neutralPlane").first() else { return crate::regen::eval::err_at(&["neutralPlane"], "Select a neutral plane.") };
    let Some(frame) = ev.resolve_plane(np) else { return crate::regen::eval::err_at(&["neutralPlane"], "Select a neutral plane.") };
    let refs = ev.query("draftFaces");
    if refs.is_empty() {
        return crate::regen::eval::err_at(&["draftFaces"], "Select faces to draft.");
    }
    let angle = ev.angle("angle")?;
    // Live L3: a 0° draft succeeds and changes nothing.
    if angle.abs() < 1e-12 {
        return Ok(());
    }
    let pull = if ev.bool("pullDirection") { -frame.normal } else { frame.normal };
    let neutral = k::Plane::new(frame.origin, pull, frame.x);
    let groups = by_body(ev.resolve_faces(refs));
    if groups.is_empty() {
        return crate::regen::eval::err_at(&["draftFaces"], "Select faces to draft.");
    }
    let refillet = ev.bool("reFillet");
    for (bi, faces) in groups.into_iter().rev() {
        let body = ev.ctx.bodies[bi].clone();
        let faces = if ev.bool("tangentPropagation") { propagate_tangent_faces(&body.shape, &faces, pull, angle) } else { faces };
        // Fillets touching the drafted faces are rolled again afterwards, like
        // Parasolid: with Reapply fillet, also the drafted fillets themselves
        // (removed, their neighbours drafted, then re-filleted); without, the
        // drafted fillets become cones and only the fillets beside them are
        // rebuilt (Onshape D3: 7335.59 / 7353.62 mm³).
        let blends = blends_to_reapply(&body.shape, &body.faces, &faces, refillet)?;
        if !blends.is_empty() {
            draft_reblend(ev, bi, &faces, &blends, pull, &neutral, angle)?;
            continue;
        }
        let shapes: Vec<Shape> = faces.iter().filter_map(|f| body.shape.sub_shape(ShapeType::Face, *f).ok()).collect();
        let res = draft_robust(&body.shape, &shapes, pull, &neutral, angle).map_err(|_| fe(DRAFT_FAILED))?;
        let names = body.shape_names();
        let face_names = ev.name_result(&[&names], &res.history, &res.shape, &BTreeMap::new(), None);
        ev.replace_body(bi, res.shape, face_names);
    }
    Ok(())
}

/// OCCT's draft, or the boolean construction where it fails or builds an
/// invalid solid (drafted fillets that vanish, e.g. R3 at 30° over 10 mm).
pub(super) fn draft_robust(shape: &Shape, faces: &[Shape], pull: DVec3, neutral: &k::Plane, angle: f64) -> k::Result<k::OpResult> {
    match k::draft(shape, faces, pull, neutral, angle) {
        Ok(r) if r.shape.is_valid() => Ok(r),
        first => k::draft_wedges(shape, faces, pull, neutral, angle).map_err(|e| match first {
            Ok(_) => e,
            Err(f) => f,
        }),
    }
}

/// A fillet face of `shape`: a blend (by name) on a cylinder, torus or sphere
/// that is tangent to a neighbour. Returns its radius.
fn fillet_radius(shape: &Shape, faces: &[Shape], names: &[Name], ef: &[Vec<u32>], f: usize) -> Option<f64> {
    if !matches!(&*names[f], TopoName::Face { role: Role::Blend(_), .. }) {
        return None;
    }
    let r = match faces[f].face_surface().ok()? {
        k::Surface::Cylinder { radius, .. } => radius,
        k::Surface::Torus { minor_radius, .. } => minor_radius,
        k::Surface::Sphere { radius, .. } => radius,
        _ => return None,
    };
    let tangent = ef.iter().enumerate().any(|(ei, fs)| fs.len() == 2 && fs.contains(&(f as u32)) && edge_normals(shape, faces, ef, ei).is_some_and(|(a, b)| a.dot(b) > 0.999));
    tangent.then_some(r)
}

/// Fillets to remove and roll again around a draft (see `draft`): blends
/// beside the drafted faces (and, with Reapply fillet, those among them),
/// plus blends touching those (corner patches).
pub(super) fn blends_to_reapply(shape: &Shape, names: &[Name], drafted: &[usize], refillet: bool) -> FResult<Vec<usize>> {
    let faces = shape.faces();
    let ef = shape.edge_faces()?;
    let is_blend: Vec<bool> = (0..faces.len()).map(|f| fillet_radius(shape, &faces, names, &ef, f).is_some()).collect();
    let drafted: BTreeSet<usize> = drafted.iter().copied().collect();
    let eligible = |f: usize| is_blend[f] && (refillet || !drafted.contains(&f));
    let mut set: BTreeSet<usize> = BTreeSet::new();
    for fs in &ef {
        if fs.len() != 2 {
            continue;
        }
        let (a, b) = (fs[0] as usize, fs[1] as usize);
        for (x, y) in [(a, b), (b, a)] {
            if drafted.contains(&x) && !(refillet && is_blend[x]) && eligible(y) {
                set.insert(y);
            }
            if refillet && drafted.contains(&x) && is_blend[x] {
                set.insert(x);
            }
        }
    }
    // Blends touching removed blends (corner patches between two fillets).
    loop {
        let before = set.len();
        for fs in &ef {
            if fs.len() == 2 {
                let (a, b) = (fs[0] as usize, fs[1] as usize);
                if set.contains(&a) && eligible(b) {
                    set.insert(b);
                }
                if set.contains(&b) && eligible(a) {
                    set.insert(a);
                }
            }
        }
        if set.len() == before {
            break;
        }
    }
    Ok(set.into_iter().collect())
}

/// Draft with fillets rolled again (see [`reblend`]).
fn draft_reblend(ev: &mut EvalCtx, bi: usize, drafted: &[usize], blends: &[usize], pull: DVec3, neutral: &k::Plane, angle: f64) -> FResult<()> {
    reblend(ev, bi, drafted, blends, DRAFT_FAILED, |shape, faces| draft_robust(shape, faces, pull, neutral, angle))
}

/// An operation on some faces with fillets rolled again (Parasolid's
/// re-fillet): remove `blends`, apply `op` to the remaining `modified`
/// faces, fillet the edges between each removed fillet's two support faces
/// again (one operation, original radii); the new fillet faces keep the old
/// names. Failures read `failed`.
pub(super) fn reblend(ev: &mut EvalCtx, bi: usize, modified: &[usize], blends: &[usize], failed: &str, op: impl Fn(&Shape, &[Shape]) -> k::Result<k::OpResult>) -> FResult<()> {
    let drafted = modified;
    let body = ev.ctx.bodies[bi].clone();
    let faces = body.shape.faces();
    let ef = body.shape.edge_faces()?;
    let removed: BTreeSet<usize> = blends.iter().copied().collect();
    // Support pairs (non-removed tangent neighbours) and radius of each removed fillet.
    let mut supports: Vec<(usize, usize, f64, Name)> = Vec::new();
    for &bf in blends {
        let Some(r) = fillet_radius(&body.shape, &faces, &body.faces, &ef, bf) else { continue };
        let mut tangent: Vec<usize> = Vec::new();
        for (ei, fs) in ef.iter().enumerate() {
            if fs.len() != 2 || !fs.contains(&(bf as u32)) {
                continue;
            }
            let other = if fs[0] as usize == bf { fs[1] as usize } else { fs[0] as usize };
            if removed.contains(&other) || tangent.contains(&other) {
                continue;
            }
            if edge_normals(&body.shape, &faces, &ef, ei).is_some_and(|(a, b)| a.dot(b) > 0.999) {
                tangent.push(other);
            }
        }
        if let [a, b] = tangent[..] {
            supports.push((a, b, r, body.faces[bf].clone()));
        }
    }
    let del: Vec<Shape> = blends.iter().map(|f| faces[*f].clone()).collect();
    let res = k::delete_faces(&body.shape, &del).map_err(|_| fe(failed))?;
    let n1 = crate::regen::eval::names_for(&res.shape, ev.name_result(&[&body.shape_names()], &res.history, &res.shape, &BTreeMap::new(), None));
    // A face's images through an operation: its history, else (a face the
    // operation left alone has no record) the face of the result through a
    // point of it, facing the same way.
    fn image(hist: &k::History, from: &Shape, to: &Shape, f: u32) -> Vec<u32> {
        let m = hist.modified(0, ShapeType::Face, f);
        if !m.is_empty() {
            return m;
        }
        // Some operations (a face offset) record a moved face as generated.
        let g: Vec<u32> = hist.generated(0, ShapeType::Face, f).into_iter().filter(|(k, _)| *k == ShapeType::Face).map(|(_, i)| i).collect();
        if !g.is_empty() {
            return g;
        }
        let Ok(face) = from.sub_shape(ShapeType::Face, f as usize) else { return Vec::new() };
        let Some(p) = point_on_face(&face) else { return Vec::new() };
        let Ok(n) = face.normal_at_point(p) else { return Vec::new() };
        to.faces()
            .iter()
            .enumerate()
            .filter(|(_, g)| g.project_point(p).is_ok_and(|q| q.distance < 1e-7) && g.normal_at_point(p).is_ok_and(|m| m.dot(n) > 1.0 - 1e-6))
            .map(|(i, _)| i as u32)
            .collect()
    }
    let img1 = |f: usize| -> Vec<u32> { image(&res.history, &body.shape, &res.shape, f as u32) };
    let to_draft: Vec<u32> = drafted.iter().filter(|f| !removed.contains(f)).flat_map(|&f| img1(f)).collect();
    let shapes: Vec<Shape> = to_draft.iter().filter_map(|&f| res.shape.sub_shape(ShapeType::Face, f as usize).ok()).collect();
    let res2 = op(&res.shape, &shapes).map_err(|_| fe(failed))?;
    let n2 = crate::regen::eval::names_for(&res2.shape, ev.name_result(&[&n1], &res2.history, &res2.shape, &BTreeMap::new(), None));
    let img = |f: usize| -> Vec<u32> { img1(f).into_iter().flat_map(|g| image(&res2.history, &res.shape, &res2.shape, g)).collect() };
    let ef2 = res2.shape.edge_faces()?;
    let mut edges: Vec<usize> = Vec::new();
    let mut radii: Vec<f64> = Vec::new();
    let mut keep_name: Vec<(usize, Name)> = Vec::new();
    for (a, b, r, old) in &supports {
        let (ia, ib) = (img(*a), img(*b));
        for (ei, fs) in ef2.iter().enumerate() {
            if fs.len() == 2 && ((ia.contains(&fs[0]) && ib.contains(&fs[1])) || (ia.contains(&fs[1]) && ib.contains(&fs[0]))) && !edges.contains(&ei) {
                edges.push(ei);
                radii.push(*r);
                keep_name.push((ei, old.clone()));
            }
        }
    }
    if edges.is_empty() {
        let names = ev.name_result(&[&n1], &res2.history, &res2.shape, &BTreeMap::new(), None);
        ev.replace_body(bi, res2.shape, names);
        return Ok(());
    }
    let shapes: Vec<Shape> = edges.iter().filter_map(|e| res2.shape.sub_shape(ShapeType::Edge, *e).ok()).collect();
    let res3 = k::fillet_checked(&res2.shape, &shapes, &radii).map_err(|_| fe(failed))?;
    let mut names3 = ev.name_result(&[&n2], &res3.history, &res3.shape, &BTreeMap::new(), Some(&blend_role));
    let map: BTreeMap<Name, Name> = keep_name.into_iter().filter_map(|(ei, old)| n2.edges.get(ei).map(|en| (Arc::new(TopoName::face(ev.feature, Role::Blend(en.clone()))), old))).collect();
    for n in names3.iter_mut() {
        if let Some(old) = map.get(n) {
            *n = old.clone();
        }
    }
    let names3 = dedup_names(ev, names3);
    ev.replace_body(bi, res3.shape, names3);
    Ok(())
}

/// Parting line draft: the faces next to the parting edges on the drafted
/// side(s) are tapered about the parting line (draft.fs, each part on its
/// own). Edges in one plane perpendicular to the pull hinge about that
/// plane; a straight edge elsewhere (a stepped or sloped parting line)
/// hinges its faces about the plane through it closest to perpendicular to
/// the pull.
fn parting_line_draft(ev: &mut EvalCtx) -> FResult<()> {
    let no_pull = || crate::regen::eval::err_at(&["pullDirectionEntity"], "Select pull direction.");
    let Some(pr) = ev.query("pullDirectionEntity").first() else { return no_pull() };
    let Some(pull) = ev.resolve_direction(pr) else { return no_pull() };
    // Live L1/L2: the sides are taken from the pull direction as picked;
    // each side's Opposite direction only flips that side's taper.
    let pull = pull.normalize();
    let edges = ev.resolve_edges(ev.query("partingEdges"));
    if edges.is_empty() {
        return crate::regen::eval::err_at(&["partingEdges"], "Select parting edges.");
    }
    // draft.fs: an edge running along the pull can't part the faces.
    for &(bi, ei) in &edges {
        let e = ev.ctx.bodies[bi].shape.sub_shape(ShapeType::Edge, ei)?;
        let info = e.edge_info()?;
        let t = e.tangent_at((info.first_param + info.last_param) * 0.5)?.normalize_or_zero();
        if t.cross(pull).length() < 1e-9 {
            return crate::regen::eval::err_at(&["partingEdges"], "Parting edge cannot be parallel to pull direction.");
        }
    }
    let mut bodies: Vec<usize> = edges.iter().map(|e| e.0).collect();
    bodies.sort_unstable();
    bodies.dedup();
    let mut drafted = false;
    for bi in bodies {
        let mine: Vec<usize> = edges.iter().filter(|e| e.0 == bi).map(|e| e.1).collect();
        drafted |= parting_line_draft_body(ev, bi, &mine, pull)?;
    }
    if !drafted {
        return err(DRAFT_FAILED);
    }
    Ok(())
}

/// [`parting_line_draft`] on one part. False when nothing was drafted.
fn parting_line_draft_body(ev: &mut EvalCtx, bi: usize, edges: &[usize], pull: DVec3) -> FResult<bool> {
    let body = ev.ctx.bodies[bi].clone();
    // Hinge planes (origin, normal), each with its edges.
    let mut planes: Vec<(DVec3, DVec3, Vec<usize>)> = Vec::new();
    for &ei in edges {
        let e = body.shape.sub_shape(ShapeType::Edge, ei)?;
        let info = e.edge_info()?;
        let pts: Vec<DVec3> = (0..=4).map(|t| e.point_at(info.first_param + (info.last_param - info.first_param) * t as f64 / 4.0)).collect::<k::Result<_>>()?;
        let level = pts.iter().all(|p| (p - pts[0]).dot(pull).abs() < 1e-7);
        let (origin, normal) = if level {
            (pts[0], pull)
        } else if let Ok(k::Curve::Line { direction, .. }) = e.edge_curve() {
            let d = direction.normalize();
            (pts[0], (pull - d * pull.dot(d)).normalize())
        } else {
            return err("Parting lines must lie in a plane perpendicular to the pull direction");
        };
        match planes.iter_mut().find(|(o, n, _)| n.dot(normal) > 1.0 - 1e-9 && (origin - *o).dot(*n).abs() < 1e-7) {
            Some(pl) => pl.2.push(ei),
            None => planes.push((origin, normal, vec![ei])),
        }
    }
    let ef = body.shape.edge_faces()?;
    let faces = body.shape.faces();
    // A positive angle narrows the part along the pull. One sided: the pull
    // side narrows away from the parting line. Two sided (live L1): the
    // first angle is the side opposite the pull, the second the pull side,
    // and by default both narrow toward the parting line.
    let s1 = if ev.bool("pullDirection") { -1.0 } else { 1.0 };
    let angle = ev.angle("angle")? * s1;
    let second = if ev.is("partingLineSides", "TWO_SIDED") { ev.angle("secondAngle")? * if ev.bool("secondPullDirection") { -1.0 } else { 1.0 } } else { 0.0 };
    let sides = ev.enum_("partingLineSides").to_string();
    let along = ev.bool("alongPull");
    let orig = body.faces.clone();
    let mut drafted = false;
    let mut done: Vec<usize> = Vec::new();
    for (origin, normal, group) in planes {
        let (mut above, mut below): (Vec<usize>, Vec<usize>) = (Vec::new(), Vec::new());
        for &ei in &group {
            for &f in ef.get(ei).map(|v| v.as_slice()).unwrap_or(&[]) {
                let f = f as usize;
                if done.contains(&f) {
                    continue;
                }
                let Some(p) = point_on_face(&faces[f]) else { continue };
                let Some((_, n)) = face_normal_near(&faces[f], p) else { continue };
                if n.normalize().dot(pull).abs() > 1.0 - 1e-9 {
                    continue; // faces perpendicular to the pull are not drafted
                }
                let list = if (p - origin).dot(normal) >= 0.0 { &mut above } else { &mut below };
                if !list.contains(&f) {
                    list.push(f);
                }
            }
        }
        done.extend(above.iter().chain(&below).copied());
        let sets: Vec<(Vec<usize>, f64)> = match sides.as_str() {
            "SYMMETRIC" => vec![(below, angle), (above, -angle)],
            "TWO_SIDED" => vec![(below, angle), (above, second)],
            _ => {
                if along { vec![(above, angle)] } else { vec![(below, angle)] }
            }
        };
        for (set, ang) in sets {
            if set.is_empty() || ang.abs() < 1e-12 {
                continue;
            }
            let body = ev.ctx.bodies[bi].clone();
            // Faces by name (indices change after the first draft).
            let names: Vec<Name> = set.iter().filter_map(|&f| orig.get(f).cloned()).collect();
            let idx: Vec<usize> = names.iter().flat_map(|n| body.find_faces(n)).collect();
            let shapes: Vec<Shape> = idx.iter().filter_map(|f| body.shape.sub_shape(ShapeType::Face, *f).ok()).collect();
            let neutral = k::Plane::new(origin, normal, if normal.cross(DVec3::X).length() > 0.1 { DVec3::X } else { DVec3::Y });
            let res = k::draft(&body.shape, &shapes, pull, &neutral, ang).map_err(|_| fe(DRAFT_FAILED))?;
            let face_names = ev.name_result(&[&body.shape_names()], &res.history, &res.shape, &BTreeMap::new(), None);
            drafted = true;
            ev.replace_body(bi, res.shape, face_names);
        }
    }
    Ok(drafted)
}

/// Add faces tangent-connected to the selection that are steeper than the
/// draft angle (Onshape tangent propagation).
fn propagate_tangent_faces(shape: &Shape, seed: &[usize], pull: DVec3, angle: f64) -> Vec<usize> {
    let faces = shape.faces();
    let Ok(ef) = shape.edge_faces() else { return seed.to_vec() };
    let mut set: BTreeSet<usize> = seed.iter().copied().collect();
    let mut stack: Vec<usize> = seed.to_vec();
    while let Some(f) = stack.pop() {
        for (ei, fs) in ef.iter().enumerate() {
            if fs.len() != 2 || !fs.contains(&(f as u32)) {
                continue;
            }
            let other = if fs[0] as usize == f { fs[1] as usize } else { fs[0] as usize };
            if set.contains(&other) {
                continue;
            }
            let Some((a, b)) = edge_normals(shape, &faces, &ef, ei) else { continue };
            if a.dot(b) < 0.9998 {
                continue;
            }
            // Steep: the face normal is within `angle` of perpendicular to pull.
            let steep = faces.get(other).and_then(point_on_face).and_then(|p| face_normal_near(&faces[other], p)).is_some_and(|(_, n)| n.normalize().dot(pull).abs() < angle.sin().max(1e-6) + 1e-9);
            if steep {
                set.insert(other);
                stack.push(other);
            }
        }
    }
    set.into_iter().collect()
}

// ------------------------------------------------------------------ face blend

pub fn face_blend(ev: &mut EvalCtx) -> FResult<()> {
    let swept = ev.is("crossSection", "SWEPT_PROFILE");
    let spine_dir = if swept {
        let Some(r) = ev.query("spine").first() else { return err("Select the spine of the swept profile") };
        Some(ev.resolve_axis(r).ok_or_else(|| fe("The spine must be a straight edge"))?.1)
    } else {
        None
    };
    if ev.bool("asymmetric") {
        return err("Asymmetric face blends are not supported yet");
    }
    let shape_kind = ev.enum_("crossSectionShape").to_string();
    if shape_kind == "CONIC" || shape_kind == "CURVATURE" {
        return err("Conic and curvature face blends are not supported yet");
    }
    for (flag, what) in [("tangentHoldLines", "Tangent hold lines"), ("conicHoldLines", "Conic hold lines"), ("hasCliffEdges", "Cliff edges"), ("hasCaps", "Caps"), ("hasLimits", "Limits"), ("hasHelpPoint", "Help point"), ("detach", "Detached face blends")] {
        if ev.bool(flag) {
            return err(format!("{what} are not supported yet"));
        }
    }
    let side1 = ev.resolve_faces(ev.query("side1"));
    let side2 = ev.resolve_faces(ev.query("side2"));
    if side1.is_empty() || side2.is_empty() {
        return err("Select faces for both sides of the blend");
    }
    let radius = if ev.is("blendControlType", "WIDTH") { ev.length("width")? / std::f64::consts::SQRT_2 } else { ev.length("radius")? };
    if radius <= 0.0 {
        return err("Blend radius must be positive");
    }
    let s1 = by_body(side1);
    let s2 = by_body(side2);
    let mut done = false;
    let bodies: BTreeSet<usize> = s1.keys().chain(s2.keys()).copied().collect();
    for bi in bodies.into_iter().rev() {
        let (Some(a), Some(b)) = (s1.get(&bi), s2.get(&bi)) else { continue };
        let body = ev.ctx.bodies[bi].clone();
        let ef = body.shape.edge_faces()?;
        let mut edges: Vec<usize> = ef
            .iter()
            .enumerate()
            .filter(|(_, fs)| fs.len() == 2 && ((a.contains(&(fs[0] as usize)) && b.contains(&(fs[1] as usize))) || (a.contains(&(fs[1] as usize)) && b.contains(&(fs[0] as usize)))))
            .map(|(i, _)| i)
            .collect();
        if edges.is_empty() {
            continue;
        }
        if ev.bool("propagation") && ev.is("propagationType", "TANGENT") {
            edges = super::detail::propagate_tangent(&body.shape, &edges);
        }
        // A circular profile swept along a spine parallel to the blended
        // (straight) edges is the rolling-ball blend.
        if let Some(d) = spine_dir {
            for &ei in &edges {
                let e = body.shape.sub_shape(ShapeType::Edge, ei)?;
                let parallel = match e.edge_curve()? {
                    k::Curve::Line { direction, .. } => direction.normalize().cross(d.normalize()).length() < 1e-6,
                    _ => false,
                };
                if !parallel {
                    return err("Swept profile blends are supported when the spine is parallel to straight blended edges");
                }
            }
        }
        let shapes: Vec<Shape> = edges.iter().filter_map(|e| body.shape.sub_shape(ShapeType::Edge, *e).ok()).collect();
        let res = if shape_kind == "CHAMFER" {
            k::chamfer(&body.shape, &shapes, k::ChamferSpec::Equal(radius), &[]).map_err(kerr("Face blend failed"))?
        } else {
            k::fillet(&body.shape, &shapes, radius).map_err(kerr("Face blend failed (radius may be too large)"))?
        };
        let names = body.shape_names();
        let face_names = ev.name_result(&[&names], &res.history, &res.shape, &BTreeMap::new(), Some(&blend_role));
        ev.replace_body(bi, res.shape, face_names);
        done = true;
    }
    if !done {
        return err("Face blend between faces that don't share an edge is not supported yet");
    }
    Ok(())
}

// ------------------------------------------------------------------ modify fillet

pub fn modify_fillet(ev: &mut EvalCtx) -> FResult<()> {
    let refs = ev.query("faces");
    if refs.is_empty() {
        return err("Select fillet faces to modify");
    }
    let change = ev.is("modifyFilletType", "CHANGE_RADIUS");
    let radius = if change { ev.length("radius")? } else { 0.0 };
    if change && radius <= 0.0 {
        return err("Fillet radius must be positive");
    }
    let groups = by_body(ev.resolve_faces(refs));
    if groups.is_empty() {
        return err("The selected faces no longer exist");
    }
    for (bi, fillet_faces) in groups.into_iter().rev() {
        let body = ev.ctx.bodies[bi].clone();
        let faces = body.shape.faces();
        let ef = body.shape.edge_faces()?;
        // Tangent support faces of each fillet face (pairs to re-fillet).
        let mut supports: Vec<(usize, usize, Name)> = Vec::new();
        for &ff in &fillet_faces {
            let surf = faces[ff].face_surface()?;
            if matches!(surf, k::Surface::Plane(_)) {
                return err("Select non-planar fillet faces");
            }
            let mut tangent: Vec<usize> = Vec::new();
            for (ei, fs) in ef.iter().enumerate() {
                if fs.len() != 2 || !fs.contains(&(ff as u32)) {
                    continue;
                }
                let other = if fs[0] as usize == ff { fs[1] as usize } else { fs[0] as usize };
                if fillet_faces.contains(&other) || tangent.contains(&other) {
                    continue;
                }
                if let Some((a, b)) = edge_normals(&body.shape, &faces, &ef, ei)
                    && a.dot(b) > 0.999
                {
                    tangent.push(other);
                }
            }
            if change {
                if tangent.len() != 2 {
                    return err("Only fillets between two tangent faces can be modified");
                }
                supports.push((tangent[0], tangent[1], body.faces[ff].clone()));
            }
        }
        let del: Vec<Shape> = fillet_faces.iter().map(|f| faces[*f].clone()).collect();
        let res = k::delete_faces(&body.shape, &del).map_err(kerr("Removing the fillet failed"))?;
        let names = body.shape_names();
        let face_names = ev.name_result(&[&names], &res.history, &res.shape, &BTreeMap::new(), None);
        if !change {
            ev.replace_body(bi, res.shape, face_names);
            continue;
        }
        // Re-fillet the edges between the healed support faces, keeping the
        // original fillet face names.
        let img = |f: usize| -> Vec<u32> { res.history.modified(0, ShapeType::Face, f as u32) };
        let ef2 = res.shape.edge_faces()?;
        let mut edges: Vec<usize> = Vec::new();
        let mut keep_name: Vec<(usize, Name)> = Vec::new();
        for (a, b, old) in &supports {
            let (ia, ib) = (img(*a), img(*b));
            for (ei, fs) in ef2.iter().enumerate() {
                if fs.len() == 2 && ((ia.contains(&fs[0]) && ib.contains(&fs[1])) || (ia.contains(&fs[1]) && ib.contains(&fs[0]))) && !edges.contains(&ei) {
                    edges.push(ei);
                    keep_name.push((ei, old.clone()));
                }
            }
        }
        if edges.is_empty() {
            return err("Could not find the edges to re-fillet");
        }
        let healed = crate::regen::eval::names_for(&res.shape, face_names);
        let shapes: Vec<Shape> = edges.iter().filter_map(|e| res.shape.sub_shape(ShapeType::Edge, *e).ok()).collect();
        let res2 = k::fillet(&res.shape, &shapes, radius).map_err(kerr("Fillet failed (radius may be too large)"))?;
        let mut names2 = ev.name_result(&[&healed], &res2.history, &res2.shape, &BTreeMap::new(), Some(&blend_role));
        // Blend(edge) -> original fillet face name.
        let map: BTreeMap<Name, Name> = keep_name.into_iter().filter_map(|(ei, old)| healed.edges.get(ei).map(|en| (Arc::new(TopoName::face(ev.feature, Role::Blend(en.clone()))), old))).collect();
        for n in names2.iter_mut() {
            if let Some(old) = map.get(n) {
                *n = old.clone();
            }
        }
        let names2 = dedup_names(ev, names2);
        ev.replace_body(bi, res2.shape, names2);
    }
    Ok(())
}

// ------------------------------------------------------------------ body draft

/// bodyDraft.fs (live "psb (10-24 g) body draft"): each drafted face turns
/// about its edge away from the parting entity, so the part narrows along
/// the pull and gains material toward the parting entity (a block's top stays
/// 40 × 20, its bottom grows by 10·tan 5° each side). Symmetric drafts the
/// two sides of the parting plane each toward it. Corner type Plane bevels
/// the corners between drafted faces from the held edge's end.
pub fn body_draft(ev: &mut EvalCtx) -> FResult<()> {
    let scope = ev.enum_("selectionType").to_string();
    let (field, empty) = match scope.as_str() {
        "FACES" => ("faces", "Select faces to draft."),
        "EDGES" => ("topEdges", "Select edges to draft."),
        _ => ("bodies", "Select parts to draft."),
    };
    if ev.query(field).is_empty() {
        return crate::regen::eval::err_at(&[field], empty);
    }
    let Some(pull) = ev.query("pullDirection").first().and_then(|r| ev.resolve_direction(r)) else {
        return crate::regen::eval::err_at(&["pullDirection"], "Body draft pull direction is invalid.");
    };
    let pull = if ev.bool("flipPullDirection") { -pull } else { pull }.normalize();
    // Live 2: a block drafted on itself fails on the selection.
    if ev.bool("draftOnSelf") {
        return crate::regen::eval::err_at(&[field], "Selected parts could not be drafted.");
    }
    let Some(parting) = ev.query("partingObject").first().and_then(|r| ev.resolve_plane(r)) else {
        return crate::regen::eval::err_at(&["partingObject"], "Parting entity is invalid.");
    };
    let angle = ev.angle("angle")?;
    let height = |p: DVec3| (p - parting.origin).dot(pull);
    // The faces to draft, per part.
    let mut per_body: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    let mut held: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    match scope.as_str() {
        "FACES" => {
            for (bi, fi) in ev.resolve_faces(ev.query("faces")) {
                per_body.entry(bi).or_default().push(fi);
            }
        }
        "EDGES" => {
            for (bi, ei) in ev.resolve_edges(ev.query("topEdges")) {
                held.entry(bi).or_default().push(ei);
                let ef = ev.ctx.bodies[bi].shape.edge_faces()?;
                per_body.entry(bi).or_default().extend(ef.get(ei).map(|v| v.iter().map(|f| *f as usize).collect::<Vec<_>>()).unwrap_or_default());
            }
        }
        _ => {
            let excluded = ev.resolve_faces(ev.query("excludeFaces"));
            for bi in ev.resolve_bodies(ev.query("bodies")) {
                let n = ev.ctx.bodies[bi].shape.faces().len();
                per_body.entry(bi).or_default().extend((0..n).filter(|fi| !excluded.contains(&(bi, *fi))));
            }
        }
    }
    let both = ev.bool("bothSides");
    let plane_corners = ev.is("cornerType", "PLANE");
    for (bi, mut faces) in per_body {
        faces.sort_unstable();
        faces.dedup();
        let body = ev.ctx.bodies[bi].clone();
        let all = body.shape.faces();
        // Faces square to the pull (caps) aren't drafted.
        faces.retain(|&f| all.get(f).and_then(|face| point_on_face(face).and_then(|p| face_normal_near(face, p))).is_some_and(|(_, n)| n.normalize().dot(pull).abs() < 1.0 - 1e-9));
        if faces.is_empty() {
            continue;
        }
        // Each side of the parting plane narrows away from it, turning about
        // its far end (or the held edges).
        let sides: Vec<(Shape, f64)> = if both {
            let cut = k::split_by_plane(&body.shape, &k::Plane::new(parting.origin, pull, parting.x)).map_err(|_| fe(BODY_DRAFT_FAILED))?;
            cut.shape.solids().into_iter().map(|s| { let up = s.centroid().map(|c| height(c) >= 0.0).unwrap_or(true); (s, if up { 1.0 } else { -1.0 }) }).collect()
        } else {
            vec![(body.shape.clone(), 1.0)]
        };
        let mut drafted: Vec<Shape> = Vec::new();
        let mut history = None;
        for (piece, sign) in sides {
            let dir = pull * sign;
            let pf = piece.faces();
            // This piece's faces from the selection: same surface, overlapping.
            let mine: Vec<Shape> = pf
                .iter()
                .filter(|f| {
                    let Some(p) = point_on_face(f) else { return false };
                    faces.iter().any(|&o| k::make_vertex(p).and_then(|v| k::distance(&v, &all[o])).is_ok_and(|d| d.distance < 1e-7))
                        && face_normal_near(f, p).is_some_and(|(_, n)| n.normalize().dot(pull).abs() < 1.0 - 1e-9)
                })
                .cloned()
                .collect();
            if mine.is_empty() {
                drafted.push(piece);
                continue;
            }
            // The far end: the held edges, else the piece's extreme along the
            // draft direction.
            let far = match held.get(&bi) {
                Some(edges) => edges.iter().filter_map(|&e| body.shape.sub_shape(ShapeType::Edge, e).ok()).filter_map(|e| e.endpoints().ok()).map(|(a, _)| (a - parting.origin).dot(dir)).fold(f64::MIN, f64::max),
                None => piece.vertices().iter().filter_map(|v| v.vertex_point().ok()).map(|p| (p - parting.origin).dot(dir)).fold(f64::MIN, f64::max),
            };
            let neutral = k::Plane::new(parting.origin + dir * far, dir, parting.x);
            let res = draft_robust(&piece, &mine, dir, &neutral, angle).map_err(|_| fe(BODY_DRAFT_FAILED))?;
            if !both && !plane_corners {
                history = Some(res.history.clone());
            }
            let mut shape = res.shape;
            if plane_corners {
                shape = bevel_corners(&piece, &mine, shape, dir, parting.origin + dir * far, angle)?;
            }
            drafted.push(shape);
        }
        let shape = match drafted.as_slice() {
            [one] => one.clone(),
            many => k::boolean(k::BooleanOp::Fuse, &many[..1], &many[1..], &k::BooleanOptions::default()).map_err(|_| fe(BODY_DRAFT_FAILED))?.shape,
        };
        // The faces keep their names through a plain draft.
        let names = match &history {
            Some(h) => ev.name_result(&[&body.shape_names()], h, &shape, &BTreeMap::new(), None),
            None => name_new(ev, &shape),
        };
        ev.replace_body(bi, shape, names);
    }
    Ok(())
}

/// Onshape's text when a body draft can't be built.
const BODY_DRAFT_FAILED: &str = "Selected parts could not be drafted.";

/// Corner type Plane: each straight corner edge along the pull between two
/// drafted planes gets a planar bevel from its far end to the two drafted
/// faces' feet (live 1c: 8530.0348 = the extended corners less four
/// tetrahedra of tan²α/2 · h³/3).
fn bevel_corners(original: &Shape, faces: &[Shape], drafted: Shape, dir: DVec3, far: DVec3, angle: f64) -> FResult<Shape> {
    let normal_of = |f: &Shape| point_on_face(f).and_then(|p| face_normal_near(f, p)).map(|(_, n)| n.normalize());
    let mut body = drafted;
    let tn = angle.tan();
    for e in original.edges() {
        let Ok(k::Curve::Line { direction, .. }) = e.edge_curve() else { continue };
        if direction.normalize().cross(dir).length() > 1e-9 {
            continue;
        }
        let touching: Vec<&Shape> = faces.iter().filter(|f| k::distance(&e, f).is_ok_and(|d| d.distance < 1e-9) && f.face_plane().ok().flatten().is_some()).collect();
        let [f1, f2] = touching.as_slice() else { continue };
        let (Some(n1), Some(n2)) = (normal_of(f1), normal_of(f2)) else { continue };
        let Ok((a, b)) = e.endpoints() else { continue };
        let (top, foot) = if (a - far).dot(dir).abs() < (b - far).dot(dir).abs() { (a, b) } else { (b, a) };
        let h = (top - foot).length();
        let (p1, p2) = (foot + n1 * h * tn, foot + n2 * h * tn);
        let Some(n) = (p1 - top).cross(p2 - top).try_normalize() else { continue };
        let tip = foot + (n1 + n2) * h * tn;
        let n = if (tip - top).dot(n) > 0.0 { n } else { -n };
        let x = if n.cross(DVec3::X).length() > 0.1 { DVec3::X } else { DVec3::Y };
        let Ok(cut) = k::split_by_plane(&body, &k::Plane::new(top, n, n.cross(x).normalize())) else { continue };
        let keep: Vec<Shape> = cut.shape.solids().into_iter().filter(|s| s.centroid().is_ok_and(|c| (c - top).dot(n) < 0.0)).collect();
        if let [one] = keep.as_slice() {
            body = one.clone();
        }
    }
    Ok(body)
}
