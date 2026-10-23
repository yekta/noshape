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
fn draft_robust(shape: &Shape, faces: &[Shape], pull: DVec3, neutral: &k::Plane, angle: f64) -> k::Result<k::OpResult> {
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
fn blends_to_reapply(shape: &Shape, names: &[Name], drafted: &[usize], refillet: bool) -> FResult<Vec<usize>> {
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

/// Draft with fillets rolled again: remove `blends`, draft the remaining
/// drafted faces, fillet the edges between each removed fillet's two support
/// faces again (one operation, original radii), the new fillet faces keep
/// the old names.
fn draft_reblend(ev: &mut EvalCtx, bi: usize, drafted: &[usize], blends: &[usize], pull: DVec3, neutral: &k::Plane, angle: f64) -> FResult<()> {
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
    let res = k::delete_faces(&body.shape, &del).map_err(|_| fe(DRAFT_FAILED))?;
    let n1 = crate::regen::eval::names_for(&res.shape, ev.name_result(&[&body.shape_names()], &res.history, &res.shape, &BTreeMap::new(), None));
    let img1 = |f: usize| -> Vec<u32> { res.history.modified(0, ShapeType::Face, f as u32) };
    let to_draft: Vec<u32> = drafted.iter().filter(|f| !removed.contains(f)).flat_map(|&f| img1(f)).collect();
    let shapes: Vec<Shape> = to_draft.iter().filter_map(|&f| res.shape.sub_shape(ShapeType::Face, f as usize).ok()).collect();
    let res2 = draft_robust(&res.shape, &shapes, pull, neutral, angle).map_err(|_| fe(DRAFT_FAILED))?;
    let n2 = crate::regen::eval::names_for(&res2.shape, ev.name_result(&[&n1], &res2.history, &res2.shape, &BTreeMap::new(), None));
    let img = |f: usize| -> Vec<u32> { img1(f).into_iter().flat_map(|g| res2.history.modified(0, ShapeType::Face, g)).collect() };
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
    let res3 = k::fillet_checked(&res2.shape, &shapes, &radii).map_err(|_| fe(DRAFT_FAILED))?;
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

/// Parting line draft with planar parting lines (perpendicular to the pull
/// direction): the faces next to the parting edges on the drafted side(s)
/// are tapered about the parting plane.
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
    let bi = edges[0].0;
    if edges.iter().any(|e| e.0 != bi) {
        return err("The parting edges must belong to one part");
    }
    let body = ev.ctx.bodies[bi].clone();
    let mut pts = Vec::new();
    for &(_, ei) in &edges {
        let e = body.shape.sub_shape(ShapeType::Edge, ei)?;
        let info = e.edge_info()?;
        // draft.fs getOrderedFaceData: DRAFT_PARALLEL_PARTING_EDGE.
        let t = e.tangent_at(0.5 * (info.first_param + info.last_param))?;
        if t.normalize().cross(pull).length() < 1e-9 {
            return crate::regen::eval::err_at(&["partingEdges"], "Parting edge cannot be parallel to pull direction.");
        }
        for t in 0..=4 {
            pts.push(e.point_at(info.first_param + (info.last_param - info.first_param) * t as f64 / 4.0)?);
        }
    }
    let h0 = pts[0].dot(pull);
    if pts.iter().any(|p| (p.dot(pull) - h0).abs() > 1e-7) {
        return err("Parting lines must lie in a plane perpendicular to the pull direction");
    }
    let origin = pts[0];
    let ef = body.shape.edge_faces()?;
    let faces = body.shape.faces();
    let (mut above, mut below): (Vec<usize>, Vec<usize>) = (Vec::new(), Vec::new());
    for &(_, ei) in &edges {
        for &f in ef.get(ei).map(|v| v.as_slice()).unwrap_or(&[]) {
            let f = f as usize;
            let Some(p) = point_on_face(&faces[f]) else { continue };
            let Some((_, n)) = face_normal_near(&faces[f], p) else { continue };
            if n.normalize().dot(pull).abs() > 1.0 - 1e-9 {
                continue; // faces perpendicular to the pull are not drafted
            }
            let list = if (p - origin).dot(pull) >= 0.0 { &mut above } else { &mut below };
            if !list.contains(&f) {
                list.push(f);
            }
        }
    }
    // A positive angle narrows the part along the pull. One sided: the pull
    // side narrows away from the parting line. Two sided (live L1): the
    // first angle is the side opposite the pull, the second the pull side,
    // and by default both narrow toward the parting line.
    let s1 = if ev.bool("pullDirection") { -1.0 } else { 1.0 };
    let angle = ev.angle("angle")? * s1;
    let sets: Vec<(Vec<usize>, DVec3, f64)> = match ev.enum_("partingLineSides") {
        "SYMMETRIC" => vec![(below, pull, angle), (above, pull, -angle)],
        "TWO_SIDED" => {
            let a2 = ev.angle("secondAngle")? * if ev.bool("secondPullDirection") { -1.0 } else { 1.0 };
            vec![(below, pull, angle), (above, pull, a2)]
        }
        _ => {
            if ev.bool("alongPull") { vec![(above, pull, angle)] } else { vec![(below, pull, angle)] }
        }
    };
    let orig = body.faces.clone();
    let mut drafted = false;
    for (set, dir, ang) in sets {
        if set.is_empty() || ang.abs() < 1e-12 {
            continue;
        }
        let body = ev.ctx.bodies[bi].clone();
        // Faces by name (indices change after the first draft).
        let names: Vec<Name> = set.iter().filter_map(|&f| orig.get(f).cloned()).collect();
        let idx: Vec<usize> = names.iter().flat_map(|n| body.find_faces(n)).collect();
        let shapes: Vec<Shape> = idx.iter().filter_map(|f| body.shape.sub_shape(ShapeType::Face, *f).ok()).collect();
        let neutral = k::Plane::new(origin, dir, if dir.cross(DVec3::X).length() > 0.1 { DVec3::X } else { DVec3::Y });
        let res = k::draft(&body.shape, &shapes, dir, &neutral, ang).map_err(|_| fe(DRAFT_FAILED))?;
        let face_names = ev.name_result(&[&body.shape_names()], &res.history, &res.shape, &BTreeMap::new(), None);
        drafted = true;
        ev.replace_body(bi, res.shape, face_names);
    }
    if !drafted {
        return err(DRAFT_FAILED);
    }
    Ok(())
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

pub fn body_draft(_ev: &mut EvalCtx) -> FResult<()> {
    err("Body draft is not supported yet")
}
