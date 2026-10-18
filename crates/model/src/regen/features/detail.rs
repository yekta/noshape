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

/// Onshape's message for a failed fillet/chamfer; the kernel reason goes to the log.
fn blend_failed(message: &str, e: impl std::fmt::Display) -> crate::regen::eval::FeatureError {
    log::debug!("{message} ({e})");
    crate::regen::eval::FeatureError::new(message)
}

/// OCCT reports success for blends that overrun neighbouring faces (e.g. a
/// chamfer wider than the corner fillets it runs over) but returns an invalid
/// solid. Fail those like Onshape, unless the input was invalid already.
fn require_valid_blend(input: &k::Shape, result: &k::Shape, message: &str) -> FResult<()> {
    if result.is_valid() || !input.is_valid() {
        return Ok(());
    }
    Err(blend_failed(message, "result is not a valid solid"))
}

pub fn fillet(ev: &mut EvalCtx) -> FResult<()> {
    if ev.is("filletType", "FULL_ROUND") {
        return full_round(ev);
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
        let res = k::fillet(&body.shape, &shapes, radius).map_err(|e| blend_failed("Failed to fillet selections.", e))?;
        require_valid_blend(&body.shape, &res.shape, "Failed to fillet selections.")?;
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
        let res = k::chamfer(&body.shape, &shapes, spec, &[]).map_err(|e| blend_failed("Failed to chamfer selections.", e))?;
        require_valid_blend(&body.shape, &res.shape, "Failed to chamfer selections.")?;
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
    let (Some(&(b1, f1)), Some(&(b2, f2))) = (s1.first(), s2.first()) else { return err("Select both side faces") };
    if center.is_empty() {
        return err("Select the center faces to replace with the full round");
    }
    if b1 != b2 || center.iter().any(|c| c.0 != b1) {
        return err("The side and center faces must belong to one part");
    }
    let bi = b1;
    let body = ev.ctx.bodies[bi].clone();
    let face = |f: usize| body.shape.sub_shape(ShapeType::Face, f);
    let (Some((p1, n1)), Some((p2, n2))) = (plane_of(&face(f1)?), plane_of(&face(f2)?)) else {
        return err("Full round fillets need planar side faces");
    };
    if n1.dot(n2) > -1.0 + 1e-7 {
        return err("Full round fillets need parallel side faces facing away from each other");
    }
    let w = (p2 - p1).dot(n1).abs();
    if w < 1e-9 {
        return err("The side faces coincide");
    }
    let r = w * 0.5;
    let cf = face(center[0].1)?;
    let Some((cc, nc)) = plane_of(&cf) else { return err("Full round fillets need a planar center face") };
    if nc.dot(n1).abs() > 1e-6 {
        return err("The center face must be perpendicular to the side faces");
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
