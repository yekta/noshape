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

pub fn draft(ev: &mut EvalCtx) -> FResult<()> {
    if ev.is("draftFeatureType", "PARTING_LINE") {
        return err("Parting line draft is not supported yet");
    }
    let Some(np) = ev.query("neutralPlane").first() else { return err("Select a neutral plane") };
    let frame = ev.resolve_plane(np).ok_or_else(|| fe("The neutral plane could not be resolved"))?;
    let refs = ev.query("draftFaces");
    if refs.is_empty() {
        return err("Select faces to draft");
    }
    let angle = ev.angle("angle")?;
    let pull = if ev.bool("pullDirection") { -frame.normal } else { frame.normal };
    let neutral = k::Plane::new(frame.origin, pull, frame.x);
    let groups = by_body(ev.resolve_faces(refs));
    if groups.is_empty() {
        return err("The faces to draft no longer exist");
    }
    for (bi, faces) in groups.into_iter().rev() {
        let body = ev.ctx.bodies[bi].clone();
        let faces = if ev.bool("tangentPropagation") { propagate_tangent_faces(&body.shape, &faces, pull, angle) } else { faces };
        let shapes: Vec<Shape> = faces.iter().filter_map(|f| body.shape.sub_shape(ShapeType::Face, *f).ok()).collect();
        let res = k::draft(&body.shape, &shapes, pull, &neutral, angle).map_err(kerr("Draft failed"))?;
        let names = body.shape_names();
        let face_names = ev.name_result(&[&names], &res.history, &res.shape, &BTreeMap::new(), None);
        ev.replace_body(bi, res.shape, face_names);
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
    if ev.is("crossSection", "SWEPT_PROFILE") {
        return err("Face blend with a swept profile cross section is not supported yet");
    }
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
            edges = propagate_tangent_edges(&body.shape, &edges);
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

/// Extend an edge set across tangent-continuous neighbors.
fn propagate_tangent_edges(shape: &Shape, edges: &[usize]) -> Vec<usize> {
    let all = shape.edges();
    let Ok(vertex_edges) = shape.vertex_edges() else { return edges.to_vec() };
    let verts = shape.vertices();
    let tangent_at = |e: &Shape, p: DVec3| -> Option<DVec3> {
        let info = e.edge_info().ok()?;
        let t = if (info.start - p).length() < (info.end - p).length() { info.first_param } else { info.last_param };
        e.tangent_at(t).ok().map(|v| v.normalize())
    };
    let mut set: BTreeSet<usize> = edges.iter().copied().collect();
    let mut stack = edges.to_vec();
    while let Some(ei) = stack.pop() {
        let Some(e) = all.get(ei) else { continue };
        for v in e.vertices() {
            let Some(vi) = shape.index_of(&v, ShapeType::Vertex) else { continue };
            let Ok(p) = verts[vi].vertex_point() else { continue };
            for &o in vertex_edges.get(vi).map(|x| x.as_slice()).unwrap_or(&[]) {
                let o = o as usize;
                if set.contains(&o) {
                    continue;
                }
                if let (Some(t1), Some(t2)) = (tangent_at(e, p), all.get(o).and_then(|x| tangent_at(x, p)))
                    && t1.dot(t2).abs() > 0.9998
                {
                    set.insert(o);
                    stack.push(o);
                }
            }
        }
    }
    set.into_iter().collect()
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
