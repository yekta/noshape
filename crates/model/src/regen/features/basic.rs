//! Boolean, Plane, Variable and Delete part.

use crate::display::PlaneFrame;
use crate::reference::Reference;
use crate::regen::context::PlaneEntity;
use crate::regen::eval::{EvalCtx, FResult, NamedShape, err};
use glam::DVec3;
use noshape_kernel as k;
use noshape_units::Dim;

pub fn boolean(ev: &mut EvalCtx) -> FResult<()> {
    let tools_idx = ev.resolve_bodies(ev.query("tools"));
    if tools_idx.is_empty() {
        return err("Select tool parts");
    }
    let op = ev.enum_("operationType").to_string();
    match op.as_str() {
        "UNION" => {
            if tools_idx.len() < 2 {
                return err("Select at least two parts to union");
            }
            let first = tools_idx[0];
            let rest: Vec<NamedShape> = tools_idx[1..].iter().map(|&i| {
                let b = &ev.ctx.bodies[i];
                NamedShape { shape: b.shape.clone(), names: b.shape_names() }
            }).collect();
            ev.boolean_into(&[first], rest, k::BooleanOp::Fuse)?;
            // Remove consumed tools (indices shift; remove by id).
            let ids: Vec<_> = tools_idx[1..].iter().map(|&i| ev.ctx.bodies.get(i).map(|b| b.id.clone())).collect();
            let keep_first = ev.ctx.bodies.get(first).map(|b| b.id.clone());
            ev.ctx.bodies.retain(|b| Some(&b.id) == keep_first.as_ref() || !ids.iter().flatten().any(|id| *id == b.id));
            Ok(())
        }
        "SUBTRACTION" | "INTERSECTION" => {
            let targets = ev.resolve_bodies(ev.query("targets"));
            if targets.is_empty() && op == "SUBTRACTION" {
                return err("Select target parts");
            }
            let tool_ids: Vec<_> = tools_idx.iter().map(|&i| ev.ctx.bodies[i].id.clone()).collect();
            let tools: Vec<NamedShape> = tools_idx.iter().map(|&i| {
                let b = &ev.ctx.bodies[i];
                NamedShape { shape: b.shape.clone(), names: b.shape_names() }
            }).collect();
            if op == "INTERSECTION" && targets.is_empty() {
                // Intersect all tools with each other: keep the common volume in the first.
                let first = tools_idx[0];
                let others = tools[1..].to_vec();
                ev.boolean_into(&[first], others, k::BooleanOp::Common)?;
                let keep = ev.ctx.bodies[first].id.clone();
                ev.ctx.bodies.retain(|b| b.id == keep || !tool_ids.contains(&b.id));
                return Ok(());
            }
            let bop = if op == "SUBTRACTION" { k::BooleanOp::Cut } else { k::BooleanOp::Common };
            ev.boolean_into(&targets, tools, bop)?;
            if !ev.bool("keepTools") {
                ev.ctx.bodies.retain(|b| !tool_ids.contains(&b.id));
            }
            Ok(())
        }
        other => err(format!("Unknown boolean operation {other}")),
    }
}

pub fn plane(ev: &mut EvalCtx) -> FResult<()> {
    let refs = ev.query("entities");
    let ty = ev.enum_("cplaneType").to_string();
    let frame = match ty.as_str() {
        "OFFSET" => {
            let Some(r) = refs.first() else { return err("Select a plane or planar face") };
            let Some(f) = ev.resolve_plane(r) else { return err("Select a plane or planar face") };
            let d = ev.length("offset")?;
            let d = if ev.bool("oppositeDirection") { -d } else { d };
            PlaneFrame { origin: f.origin + f.normal * d, ..f }
        }
        "PLANE_POINT" => {
            let plane = refs.iter().find_map(|r| ev.resolve_plane(r));
            let point = refs.iter().find_map(|r| ev.resolve_vertex_point(r));
            match (plane, point) {
                (Some(f), Some(p)) => PlaneFrame { origin: f.origin + f.normal * (p - f.origin).dot(f.normal), ..f },
                _ => return err("Select a plane and a point"),
            }
        }
        "POINT_NORMAL" | "LINE_POINT" => {
            let point = refs.iter().find_map(|r| ev.resolve_vertex_point(r));
            let axis = refs.iter().find_map(|r| ev.resolve_axis(r));
            match (point, axis) {
                (Some(p), Some((_, d))) => frame_from(p, d),
                _ => return err("Select a point and a line"),
            }
        }
        "THREE_POINT" => {
            let pts: Vec<DVec3> = refs.iter().filter_map(|r| ev.resolve_vertex_point(r)).collect();
            if pts.len() < 3 {
                return err("Select three points");
            }
            let n = (pts[1] - pts[0]).cross(pts[2] - pts[0]);
            if n.length() < 1e-12 {
                return err("Points are collinear");
            }
            let x = (pts[1] - pts[0]).normalize();
            let n = n.normalize();
            PlaneFrame { origin: pts[0], x, y: n.cross(x), normal: n }
        }
        "MID_PLANE" => {
            let frames: Vec<PlaneFrame> = refs.iter().filter_map(|r| ev.resolve_plane(r)).collect();
            if frames.len() == 2 && frames[0].normal.cross(frames[1].normal).length() < 1e-9 {
                let o = (frames[0].origin + frames[1].origin) * 0.5;
                PlaneFrame { origin: o, ..frames[0] }
            } else if frames.len() == 2 {
                // Bisector of two intersecting planes (through their common line).
                let (a, b) = (frames[0], frames[1]);
                let nb = if a.normal.dot(b.normal) < 0.0 { -b.normal } else { b.normal };
                let n = (a.normal + nb).normalize();
                let d = a.normal.cross(b.normal).normalize();
                // A point on both planes: solve in the plane spanned by the normals.
                let (n1, n2) = (a.normal, b.normal);
                let (h1, h2) = (a.origin.dot(n1), b.origin.dot(n2));
                let c = n1.dot(n2);
                let det = 1.0 - c * c;
                let p = n1 * ((h1 - h2 * c) / det) + n2 * ((h2 - h1 * c) / det);
                PlaneFrame { origin: p, x: d, y: n.cross(d), normal: n }
            } else {
                let pts: Vec<DVec3> = refs.iter().filter_map(|r| ev.resolve_vertex_point(r)).collect();
                if pts.len() == 2 {
                    frame_from((pts[0] + pts[1]) * 0.5, (pts[1] - pts[0]).normalize())
                } else {
                    return err("Select two planes or two points");
                }
            }
        }
        "LINE_ANGLE" => {
            let axis = refs.iter().find_map(|r| ev.resolve_axis(r));
            let Some((o, d)) = axis else { return err("Select a line") };
            let a = ev.angle("angle")?;
            let base = frame_from(o, d.any_orthonormal_vector());
            // Rotate a plane containing the line about the line.
            let n0 = base.normal - d * base.normal.dot(d);
            let n = glam::DQuat::from_axis_angle(d, a) * n0.normalize();
            PlaneFrame { origin: o, x: d, y: n.cross(d), normal: n }
        }
        "CURVE_POINT" => {
            let point = refs.iter().find_map(|r| ev.resolve_vertex_point(r));
            let axis = refs.iter().find_map(|r| ev.resolve_axis(r));
            match (point, axis) {
                (Some(p), Some((_, d))) => frame_from(p, d),
                _ => return err("Select a curve and a point"),
            }
        }
        "TANGENT_CYLINDER" => super::refgeom::tangent_cylinder_frame(ev, refs, ev.bool("oppositeDirection"))?,
        "TANGENT_PLANE" => {
            // Tangent to a face at a point (or at the face center).
            let face = ev.resolve_faces(refs).first().copied();
            let Some((bi, fi)) = face else { return err("Select a face and optionally a point") };
            let f = ev.face_shape(bi, fi)?;
            let p = refs.iter().filter(|r| !matches!(r, Reference::Face(_))).find_map(|r| ev.resolve_vertex_point(r)).unwrap_or(f.centroid()?);
            let q = f.project_point(p)?;
            let n = f.normal_at_point(q.point)?;
            frame_from(q.point, n)
        }
        other => return err(format!("Plane type {other} is not supported yet")),
    };
    let frame = if ev.bool("flipAlignment") { PlaneFrame { normal: -frame.normal, y: -frame.y, ..frame } } else { frame };
    let half = ev.length("width").unwrap_or(0.15) * 0.5;
    let half_h = ev.length("height").unwrap_or(0.15) * 0.5;
    let name = ev.name.to_string();
    ev.ctx.planes.push(PlaneEntity { reference: Reference::FeaturePlane(ev.feature), name, frame, half_width: half.max(0.01), half_height: half_h.max(0.01) });
    Ok(())
}

fn frame_from(origin: DVec3, normal: DVec3) -> PlaneFrame {
    let n = normal.normalize();
    let x = n.any_orthonormal_vector();
    PlaneFrame { origin, x, y: n.cross(x), normal: n }
}

pub fn variable(ev: &mut EvalCtx) -> FResult<()> {
    let name = ev.params.text("name").trim().trim_start_matches('#').to_string();
    if name.is_empty() || !name.chars().next().is_some_and(|c| c.is_alphabetic() || c == '_') || !name.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return err("Enter a valid variable name");
    }
    let q = match ev.enum_("variableType") {
        "LENGTH" => noshape_units::Quantity::new(ev.length("lengthValue")?, Dim::LENGTH),
        "ANGLE" => noshape_units::Quantity::new(ev.angle("angleValue")?, Dim::ANGLE),
        "NUMBER" => noshape_units::Quantity::new(ev.real("numberValue")?, Dim::UNITLESS),
        _ => {
            let text = ev.params.text("anyValue").to_string();
            let scope_vars = ev.ctx.variables.clone();
            let scope = noshape_units::LayeredScope { inner: &scope_vars, outer: ev.outer_vars };
            noshape_units::evaluate(&text, &scope).map_err(|e| crate::regen::eval::FeatureError::new(format!("{text}: {e}")))?
        }
    };
    ev.ctx.variables.insert(name, q);
    Ok(())
}

pub fn delete_bodies(ev: &mut EvalCtx) -> FResult<()> {
    let ids: Vec<_> = ev.resolve_bodies(ev.query("entities")).into_iter().map(|i| ev.ctx.bodies[i].id.clone()).collect();
    if ids.is_empty() {
        return err("Select parts to delete");
    }
    ev.ctx.bodies.retain(|b| !ids.contains(&b.id));
    Ok(())
}
