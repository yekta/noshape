//! Fillet, chamfer and shell.

use crate::naming::Name;
use crate::reference::Role;
use crate::regen::eval::{EvalCtx, FResult, err};
use noshape_kernel as k;
use noshape_kernel::ShapeType;
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
    let all = shape.edges();
    let Ok(vertex_edges) = shape.vertex_edges() else { return edges.to_vec() };
    let verts = shape.vertices();
    let tangent_at = |e: &k::Shape, p: glam::DVec3| -> Option<glam::DVec3> {
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

fn blend_role(n: &Name) -> Role {
    Role::Blend(n.clone())
}

pub fn fillet(ev: &mut EvalCtx) -> FResult<()> {
    if !ev.is("filletType", "EDGE") {
        return err("Full round fillet is not supported yet");
    }
    let refs = ev.query("entities");
    if refs.is_empty() {
        return err("Select edges or faces to fillet");
    }
    let radius = if ev.is("blendControlType", "WIDTH") { ev.length("width")? / std::f64::consts::SQRT_2 } else { ev.length("radius")? };
    if radius <= 0.0 {
        return err("Fillet radius must be positive");
    }
    let groups = by_body(ev.resolve_edges(refs));
    if groups.is_empty() {
        return err("Selected edges no longer exist");
    }
    for (bi, edges) in groups.into_iter().rev() {
        let body = ev.ctx.bodies[bi].clone();
        let edges = if ev.bool("tangentPropagation") { propagate_tangent(&body.shape, &edges) } else { edges };
        let shapes: Vec<k::Shape> = edges.iter().filter_map(|e| body.shape.sub_shape(ShapeType::Edge, *e).ok()).collect();
        let t0 = std::time::Instant::now();
        let res = k::fillet(&body.shape, &shapes, radius).map_err(|e| crate::regen::eval::FeatureError::new(format!("Fillet failed (radius may be too large): {e}")))?;
        let t1 = std::time::Instant::now();
        let names = body.shape_names();
        let face_names = ev.name_result(&[&names], &res.history, &res.shape, &BTreeMap::new(), Some(&blend_role));
        let t2 = std::time::Instant::now();
        ev.replace_body(bi, res.shape, face_names);
        log::debug!("fillet: kernel {:?}, naming {:?}, body {:?} (history records {})", t1 - t0, t2 - t1, t2.elapsed(), res.history.records.len());
    }
    Ok(())
}

pub fn chamfer(ev: &mut EvalCtx) -> FResult<()> {
    let refs = ev.query("entities");
    if refs.is_empty() {
        return err("Select edges or faces to chamfer");
    }
    let spec = match ev.enum_("chamferType") {
        "TWO_OFFSETS" => {
            let (a, b) = (ev.length("width1")?, ev.length("width2")?);
            if ev.bool("oppositeDirection") { k::ChamferSpec::TwoDistances(b, a) } else { k::ChamferSpec::TwoDistances(a, b) }
        }
        "OFFSET_ANGLE" => k::ChamferSpec::DistanceAngle(ev.length("width")?, ev.angle("angle")?),
        _ => k::ChamferSpec::Equal(ev.length("width")?),
    };
    let groups = by_body(ev.resolve_edges(refs));
    if groups.is_empty() {
        return err("Selected edges no longer exist");
    }
    for (bi, edges) in groups.into_iter().rev() {
        let body = ev.ctx.bodies[bi].clone();
        let edges = if ev.bool("tangentPropagation") { propagate_tangent(&body.shape, &edges) } else { edges };
        let shapes: Vec<k::Shape> = edges.iter().filter_map(|e| body.shape.sub_shape(ShapeType::Edge, *e).ok()).collect();
        let res = k::chamfer(&body.shape, &shapes, spec, &[]).map_err(|e| crate::regen::eval::FeatureError::new(format!("Chamfer failed: {e}")))?;
        let names = body.shape_names();
        let face_names = ev.name_result(&[&names], &res.history, &res.shape, &BTreeMap::new(), Some(&blend_role));
        ev.replace_body(bi, res.shape, face_names);
    }
    Ok(())
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
        return err("Select faces to remove or parts to hollow");
    }
    for (bi, faces) in groups.into_iter().rev() {
        let body = ev.ctx.bodies[bi].clone();
        let shapes: Vec<k::Shape> = faces.iter().filter_map(|f| body.shape.sub_shape(ShapeType::Face, *f).ok()).collect();
        let res = k::shell(&body.shape, &shapes, t, side, k::JoinType::Arc).map_err(|e| crate::regen::eval::FeatureError::new(format!("Shell failed: {e}")))?;
        let names = body.shape_names();
        let offset_role = |n: &Name| Role::Offset(n.clone());
        let face_names = ev.name_result(&[&names], &res.history, &res.shape, &BTreeMap::new(), Some(&offset_role));
        ev.replace_body(bi, res.shape, face_names);
    }
    Ok(())
}
