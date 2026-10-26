//! Wrap (03a PS-16, FS `wrap`), Isocline (03b CV, FS `isocline`) and
//! Constrained surface (03b SF-10, FS `constrainedSurface`).
//!
//! **Wrap** maps planar faces / sketch regions onto a cylinder or cone
//! through the surfaces' isometric developments (lengths and areas are
//! kept). The default anchor follows Onshape: the source anchor lies on
//! the target axis projected into the source plane (closest to the source
//! center) and maps to its projection on the target; the axis direction is
//! kept. Results: surfaces, thickened solids merged per the operation type,
//! or the wrapped edges imprinted on the target (Split).

use super::bshared::{chain_wires, face_name, names_from_sources};
use crate::display::BodyKind;
use crate::naming::{Name, ShapeNames};
use crate::reference::{Reference, Role, Source, TopoName};
use crate::regen::eval::{EvalCtx, FResult, NamedShape, err};
use glam::{DVec2, DVec3};
use noshape_kernel as k;
use noshape_kernel::{Developable, Placement2d, Shape, ShapeType};
use std::collections::BTreeMap;
use std::sync::Arc;

fn fe(m: impl Into<String>) -> crate::regen::eval::FeatureError {
    crate::regen::eval::FeatureError::new(m)
}

/// Source faces of a wrap with the naming source of each.
fn wrap_sources(ev: &EvalCtx) -> FResult<Vec<(Shape, Source, DVec3)>> {
    let mut out = Vec::new();
    for r in ev.query("source") {
        match r {
            Reference::Face(n) => {
                for (bi, fi) in ev.resolve_faces(std::slice::from_ref(r)) {
                    let f = ev.face_shape(bi, fi)?;
                    let c = f.centroid()?;
                    let nrm = f.normal_at_point(c)?;
                    out.push((f, Source::Topo(Arc::new(n.clone())), nrm));
                }
            }
            _ => {
                for p in ev.profiles(std::slice::from_ref(r))? {
                    out.push((p.shape.clone(), p.key.clone(), p.frame.normal));
                }
            }
        }
    }
    if out.is_empty() {
        return crate::regen::eval::err_at(&["source"], "Select planar faces or sketch regions to wrap.");
    }
    Ok(out)
}

/// Developable surface of a destination selection (face or plane).
fn destination(ev: &EvalCtx) -> FResult<(Developable, Option<(usize, usize)>)> {
    let no_target = || crate::regen::eval::err_at(&["destination"], "Select a cylindrical or conical face to wrap around.");
    let Some(r) = ev.query("destination").first() else { return no_target() };
    match r {
        Reference::Face(_) => {
            let Some(&(bi, fi)) = ev.resolve_faces(std::slice::from_ref(r)).first() else { return no_target() };
            let f = ev.face_shape(bi, fi)?;
            let Ok(d) = Developable::of_face(&f) else { return no_target() };
            Ok((d, Some((bi, fi))))
        }
        // A plane can't be a target (live L20).
        _ => no_target(),
    }
}

/// Axis (origin, direction) of a curved developable surface.
fn axis_of(d: &Developable) -> Option<(DVec3, DVec3)> {
    match *d {
        Developable::Cylinder { origin, axis, .. } | Developable::Cone { origin, axis, .. } => Some((origin, axis.normalize())),
        Developable::Plane { .. } => None,
    }
}

/// Outward normal of a developable surface at a point.
fn normal_at(d: &Developable, p: DVec3, ref_angle: f64) -> DVec3 {
    match *d {
        Developable::Plane { normal, .. } => normal.normalize(),
        _ => {
            let f = d.flat(p, ref_angle);
            let (ax, ay, n) = d.flat_axes(f, ref_angle);
            // Point the normal away from the axis.
            let (o, z) = axis_of(d).unwrap_or((DVec3::ZERO, DVec3::Z));
            let radial = (p - o) - z * (p - o).dot(z);
            let _ = (ax, ay);
            if n.dot(radial) < 0.0 { -n } else { n }
        }
    }
}

/// Unit flat-space direction of a 3D tangent direction at flat point `f`.
fn flat_dir(d: &Developable, f: DVec2, ref_angle: f64, v: DVec3) -> DVec2 {
    let (ax, ay, _) = d.flat_axes(f, ref_angle);
    DVec2::new(v.dot(ax), v.dot(ay)).normalize_or_zero()
}

pub fn wrap(ev: &mut EvalCtx) -> FResult<()> {
    let sources = wrap_sources(ev)?;
    let (dst, dst_face) = destination(ev)?;
    let src_faces: Vec<Shape> = sources.iter().map(|s| s.0.clone()).collect();
    // Live L20: only planar tools.
    let src = match Developable::of_face(&src_faces[0]) {
        Ok(d @ Developable::Plane { .. }) => d,
        _ => return crate::regen::eval::err_at(&["source"], "Select planar faces or sketch regions to wrap."),
    };
    if matches!(dst, Developable::Plane { .. }) {
        return crate::regen::eval::err_at(&["destination"], "Select a cylindrical or conical face to wrap around.");
    }
    // All tools on one surface.
    for f in &src_faces[1..] {
        let Ok(d) = Developable::of_face(f) else { return crate::regen::eval::err_at(&["source"], "Source entities must all lie on the same plane.") };
        let same = match (src, d) {
            (Developable::Plane { origin, normal, .. }, Developable::Plane { origin: o2, normal: n2, .. }) => normal.cross(n2).length() < 1e-9 && (o2 - origin).dot(normal).abs() < 1e-7,
            (Developable::Cylinder { origin, axis, radius, .. }, Developable::Cylinder { origin: o2, axis: a2, radius: r2, .. }) => {
                axis.cross(a2).length() < 1e-9 && ((o2 - origin) - axis * (o2 - origin).dot(axis)).length() < 1e-7 && (radius - r2).abs() < 1e-7
            }
            (a, b) => a == b,
        };
        if !same {
            return crate::regen::eval::err_at(&["source"], "Source entities must all lie on the same plane.");
        }
    }
    // Anchors.
    let src_center = {
        let mut lo = DVec3::splat(f64::INFINITY);
        let mut hi = DVec3::splat(f64::NEG_INFINITY);
        for f in &src_faces {
            let b = f.bounding_box(false)?;
            lo = lo.min(b.min);
            hi = hi.max(b.max);
        }
        (lo + hi) * 0.5
    };
    let (curved, flat_side) = (&dst, &src);
    let (ax_o, ax_z) = axis_of(curved).ok_or_else(|| fe("The wrap surfaces are invalid"))?;
    let plane_n = match *flat_side {
        Developable::Plane { normal, .. } => normal.normalize(),
        _ => DVec3::Z,
    };
    let plane_o = match *flat_side {
        Developable::Plane { origin, .. } => origin,
        _ => DVec3::ZERO,
    };
    let to_plane = |p: DVec3| p - plane_n * (p - plane_o).dot(plane_n);
    // Axis projected into the plane (the line that maps onto the axis direction).
    let zp = ax_z - plane_n * ax_z.dot(plane_n);
    let (a_src, a_dst): (DVec3, DVec3) = if ev.bool("customAnchors") {
        let get = |id: &str| ev.query(id).first().and_then(|r| ev.resolve_vertex_point(r));
        // wrap.fs marks each missing anchor.
        match (get("sourceAnchor"), get("destinationAnchor")) {
            (Some(s), Some(d)) => (s, d),
            (s, d) => {
                let missing: Vec<&str> = [("sourceAnchor", s.is_none()), ("destinationAnchor", d.is_none())].into_iter().filter(|m| m.1).map(|m| m.0).collect();
                return crate::regen::eval::err_at(&missing, "Select anchor points.");
            }
        }
    } else {
        let c = to_plane(src_center);
        let a = if zp.length() > 1e-9 {
            let o = to_plane(ax_o);
            let d = zp.normalize();
            o + d * (c - o).dot(d)
        } else {
            c
        };
        (a, a)
    };
    let ref_curved = curved.angle_of(a_dst, 0.0);
    let (src_ref, dst_ref) = (0.0, ref_curved);
    let f_src = src.flat(a_src, src_ref);
    let f_dst = dst.project(a_dst, dst_ref);
    // Directions: the axis direction is kept.
    let axis_dir = if zp.length() > 1e-9 { zp.normalize() } else { k::Plane::new(plane_o, plane_n, DVec3::X).unit_x() };
    let (d_src, d_dst) = (flat_dir(&src, f_src, src_ref, axis_dir), flat_dir(&dst, f_dst, dst_ref, ax_z));
    if d_src.length() < 0.5 || d_dst.length() < 0.5 {
        return crate::regen::eval::err_at(&["customAnchors"], "Anchor points could not be automatically chosen. Select \"Specify anchor points\" option to choose them manually.");
    }
    // Orientation: keep the tool's side facing the same way as the target normal.
    let p_dst3 = dst.point(f_dst, dst_ref);
    let n_src = sources[0].2;
    let n_dst_surf = normal_at(&dst, p_dst3, dst_ref);
    let aligned = n_src.dot(n_dst_surf) >= 0.0;
    let n_dst = if aligned { n_dst_surf } else { -n_dst_surf };
    let (_, _, ns_flat) = src.flat_axes(f_src, src_ref);
    let (_, _, nd_flat) = dst.flat_axes(f_dst, dst_ref);
    let mut mirror = (ns_flat.dot(n_src) > 0.0) != (nd_flat.dot(n_dst) > 0.0);
    if ev.bool("flipAlignment") {
        mirror = !mirror;
    }
    let angle = ev.angle("angle")? * if ev.bool("angleOppositeDirection") { -1.0 } else { 1.0 };
    let u = ev.length("uShift")? * if ev.bool("uShiftOppositeDirection") { -1.0 } else { 1.0 };
    let v = ev.length("vShift")? * if ev.bool("vShiftOppositeDirection") { -1.0 } else { 1.0 };
    let shift = d_dst.perp() * u + d_dst * v;
    let placement = Placement2d::align(f_src, d_src, f_dst, d_dst, mirror).then_rotate_shift(f_dst, angle, shift);
    let res = k::wrap_faces(&src_faces, &src, src_ref, &dst, dst_ref, &placement, 1e-7).map_err(|e| fe(format!("Wrap failed: {e}")))?;
    let out_faces = res.shape.faces();
    if out_faces.len() != src_faces.len() {
        return err("Wrap produced an unexpected result");
    }
    let names: Vec<Name> = sources.iter().map(|s| face_name(ev, Role::Profile(s.1.clone()))).collect();
    let mut wrapped = NamedShape::new(res.shape.clone(), names);
    let result_type = ev.enum_("resultType").to_string();
    match result_type.as_str() {
        "IMPRINT" => {
            let Some((bi, _)) = dst_face else { return err("Split needs a target face of a part") };
            let body = ev.ctx.bodies[bi].clone();
            let edges: Vec<Shape> = out_faces.iter().flat_map(|f| f.edges()).collect();
            let imp = k::imprint(&body.shape, &edges).map_err(|_| fe("Failed to imprint edges onto destination face."))?;
            let names = ev.name_result(&[&body.shape_names(), &ShapeNames::default()], &imp.history, &imp.shape, &BTreeMap::new(), None);
            if imp.shape.count(ShapeType::Face) <= body.shape.count(ShapeType::Face) {
                ev.warnings.push("The wrapped edges did not split the target face".into());
            }
            ev.replace_body(bi, imp.shape, names);
            Ok(())
        }
        rt => {
            if ev.bool("trim")
                && let Some((bi, fi)) = dst_face
            {
                let target = ev.face_shape(bi, fi)?;
                match k::common(&wrapped.shape, &target) {
                    Ok(c) if c.shape.count(ShapeType::Face) > 0 => {
                        let names = ev.name_result(&[&wrapped.names, &ShapeNames::default()], &c.history, &c.shape, &BTreeMap::new(), None);
                        wrapped = NamedShape::new(c.shape, names);
                    }
                    _ => ev.warnings.push("The wrapped faces could not be trimmed to the target".into()),
                }
            }
            if rt == "SOLID" {
                let t = ev.length("thickness")?;
                if t <= 0.0 {
                    return err("Thickness must be positive");
                }
                let mut tools = Vec::new();
                for (fi, f) in wrapped.shape.faces().into_iter().enumerate() {
                    // Thicken along the outward normal of the target (flip: inward).
                    let c = f.centroid()?;
                    let fnrm = f.normal_at_point(c)?;
                    let outward = normal_at(&dst, c, dst_ref);
                    let along = fnrm.dot(outward) >= 0.0;
                    let positive = along != ev.bool("oppositeDirection");
                    let th = k::thicken(&f, if positive { t } else { 0.0 }, if positive { 0.0 } else { t })?;
                    let src_name = wrapped.names.faces.get(fi).cloned().unwrap_or_else(|| face_name(ev, Role::New(fi as u32)));
                    let names = names_from_sources(ev, &th.history, &th.shape, 0, std::slice::from_ref(&src_name), |n| Role::Offset(n.clone()));
                    tools.push(NamedShape::new(th.shape, names));
                }
                let op = ev.enum_("operationType").to_string();
                let scope = super::extrude::scope(ev);
                ev.apply_operation(tools, &op, scope, BodyKind::Solid)
            } else {
                ev.apply_operation(vec![wrapped], "NEW", None, BodyKind::Surface)
            }
        }
    }
}

// ------------------------------------------------------------------ isocline

pub fn isocline(ev: &mut EvalCtx) -> FResult<()> {
    let faces = ev.resolve_faces(ev.query("faces"));
    if faces.is_empty() {
        return err("Select faces for the isocline");
    }
    let Some(dr) = ev.query("direction").first() else { return err("Select a direction") };
    let Some(mut dir) = ev.resolve_direction(dr) else { return err("The selected direction no longer exists") };
    if ev.bool("oppositeDirection") {
        dir = -dir;
    }
    let angle = ev.angle("angle")?;
    let split = ev.bool("split");
    let mut by_body: BTreeMap<usize, Vec<Shape>> = BTreeMap::new();
    for &(bi, fi) in &faces {
        let f = ev.face_shape(bi, fi)?;
        let curves = k::isocline_curves(&f, dir, angle, split).map_err(|e| fe(format!("Isocline failed: {e}")))?;
        by_body.entry(bi).or_default().extend(curves);
    }
    if by_body.values().all(|v| v.is_empty()) {
        if split {
            ev.warnings.push("No isocline at this angle on the selected faces".into());
            return Ok(());
        }
        return err("No isocline at this angle on the selected faces");
    }
    if split {
        for (bi, edges) in by_body.into_iter().rev() {
            if edges.is_empty() {
                continue;
            }
            let body = ev.ctx.bodies[bi].clone();
            let res = k::imprint(&body.shape, &edges).map_err(|e| fe(format!("The faces could not be split: {e}")))?;
            let names = ev.name_result(&[&body.shape_names(), &ShapeNames::default()], &res.history, &res.shape, &BTreeMap::new(), None);
            ev.replace_body(bi, res.shape, names);
        }
        Ok(())
    } else {
        let edges: Vec<Shape> = by_body.into_values().flatten().collect();
        let wires = chain_wires(&edges)?;
        super::bshared::add_wire_bodies(ev, wires)
    }
}

// ------------------------------------------------------------------ constrained surface

pub fn constrained_surface(ev: &mut EvalCtx) -> FResult<()> {
    let mesh = ev.is("csType", "MESH");
    let mut points: Vec<DVec3> = Vec::new();
    let mut normals: Vec<(usize, DVec3)> = Vec::new();
    let push = |points: &mut Vec<DVec3>, p: DVec3| -> usize {
        if let Some(i) = points.iter().position(|q| (*q - p).length() < 1e-9) {
            return i;
        }
        points.push(p);
        points.len() - 1
    };
    if mesh {
        let refs = ev.query("meshes");
        if refs.is_empty() {
            return err("Select mesh faces or bodies");
        }
        let mut faces: Vec<Shape> = ev.resolve_faces(refs).into_iter().map(|(b, f)| ev.face_shape(b, f)).collect::<FResult<_>>()?;
        let bodies: Vec<Reference> = refs.iter().filter(|r| matches!(r, Reference::Body(_))).cloned().collect();
        for bi in ev.resolve_bodies(&bodies) {
            faces.extend(ev.ctx.bodies[bi].shape.faces());
        }
        let mut grid: std::collections::HashSet<[i64; 3]> = std::collections::HashSet::new();
        for f in &faces {
            for v in f.vertices() {
                let p = v.vertex_point()?;
                if grid.insert([(p.x * 1e9).round() as i64, (p.y * 1e9).round() as i64, (p.z * 1e9).round() as i64]) {
                    points.push(p);
                }
            }
        }
    } else {
        for item in ev.params.array("vertices") {
            let Some(r) = item.query("vertex").first() else { continue };
            let Some(p) = ev.resolve_vertex_point(r) else { return err("A selected vertex no longer exists") };
            let i = push(&mut points, p);
            if let Some(nr) = item.query("normal").first() {
                let Some(mut nv) = ev.resolve_direction(nr) else { return err("A selected normal no longer exists") };
                if item.bool("flipNormal") {
                    nv = -nv;
                }
                normals.push((i, nv));
            }
        }
    }
    if points.len() < 3 {
        return err("Select at least three points (vertices) to fit a surface");
    }
    let tol = ev.length("tolerance").unwrap_or(1e-5).max(1e-9);
    let opts = k::SurfaceFitOptions { tolerance: tol, interpolate: !mesh, smooth: ev.is("optimize", "SMOOTH") };
    let fit = k::fit_surface(&points, &normals, &opts).map_err(|e| fe(format!("The surface could not be fitted: {e}")))?;
    if ev.bool("computeDeviation") {
        ev.warnings.push(format!("Maximum deviation: {}", noshape_units::format_quantity(noshape_units::Quantity::meters(fit.max_deviation), ev.units)));
    }
    let name: Name = Arc::new(TopoName::face(ev.feature, Role::New(0)));
    ev.apply_operation(vec![NamedShape::new(fit.face, vec![name])], "NEW", None, BodyKind::Surface)
}
