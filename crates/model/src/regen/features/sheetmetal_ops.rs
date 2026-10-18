//! Sheet metal operations on existing walls: Make joint / Modify joint
//! (03c SM-10, SM-11), Tab (SM-04), Bend (SM-07), Bend relief (SM-14) and
//! Corner break (SM-13).
//!
//! Bends at sharp wall corners are made by filleting the corner's convex
//! edge with `R + T` and its concave edge with `R` (concentric, so the
//! region between is a true bend); modifying a bend removes its faces with
//! the kernel's defeaturing (restoring the virtual sharp) and re-bends.
//! Rips are thin slabs cut through the corner.

use super::sheetmetal_util::*;
use crate::naming::Name;
use crate::reference::{Reference, Role, TopoName};
use crate::regen::context::Body;
use crate::regen::eval::{EvalCtx, FResult, NamedShape, err, sweep_names};
use crate::sheetmetal::{Bend, BendKind, BendReliefStyle, Rip, SheetMetalData, Wall};
use glam::{DMat4, DVec3};
use noshape_kernel as k;
use noshape_kernel::{Shape, ShapeType};
use std::collections::BTreeMap;
use std::sync::Arc;

/// Walls owning the two faces adjacent to an edge (distinct walls only).
pub fn walls_at_edge(body: &Body, data: &SheetMetalData, ei: usize) -> Option<(usize, usize)> {
    let ef = body.shape.edge_faces().ok()?;
    let faces = ef.get(ei)?;
    if faces.len() != 2 {
        return None;
    }
    let (wa, _) = data.wall_of_face(&body.faces[faces[0] as usize])?;
    let (wb, _) = data.wall_of_face(&body.faces[faces[1] as usize])?;
    (wa != wb).then_some((wa, wb))
}

/// Linear edges between faces of walls `wa` and `wb`: (edge, convex).
pub fn corner_edges(body: &Body, data: &SheetMetalData, wa: usize, wb: usize) -> Vec<(usize, bool)> {
    let Ok(ef) = body.shape.edge_faces() else { return Vec::new() };
    let mut out = Vec::new();
    for (ei, faces) in ef.iter().enumerate() {
        if faces.len() != 2 {
            continue;
        }
        let w1 = data.wall_of_face(&body.faces[faces[0] as usize]).map(|x| x.0);
        let w2 = data.wall_of_face(&body.faces[faces[1] as usize]).map(|x| x.0);
        if !((w1 == Some(wa) && w2 == Some(wb)) || (w1 == Some(wb) && w2 == Some(wa))) {
            continue;
        }
        let Ok(e) = body.shape.sub_shape(ShapeType::Edge, ei) else { continue };
        let Some((a, b)) = line_of(&e) else { continue };
        let (Ok(f1), Ok(f2)) = (body.shape.sub_shape(ShapeType::Face, faces[0] as usize), body.shape.sub_shape(ShapeType::Face, faces[1] as usize)) else { continue };
        let (Some((_, n1)), Some((_, n2))) = (plane_of(&f1), plane_of(&f2)) else { continue };
        if n1.cross(n2).length() < 1e-6 {
            continue;
        }
        let convex = is_convex(&body.shape, (a + b) * 0.5, n1, n2, data.thickness * 0.25);
        out.push((ei, convex));
    }
    out
}

/// First resolved reference-side face of a wall: (centroid, outward normal).
fn wall_plane(body: &Body, w: &Wall) -> Option<(DVec3, DVec3)> {
    let fi = *faces_named(body, &w.faces).first()?;
    plane_of(&body.shape.sub_shape(ShapeType::Face, fi).ok()?)
}

/// Turn the sharp corner between walls `parent` and `child` into a bend.
pub fn bend_corner(ev: &mut EvalCtx, bi: usize, data: &mut SheetMetalData, parent: usize, child: usize, r: f64, kf: f64) -> FResult<()> {
    let t = data.thickness;
    if r <= 0.0 {
        return err("Bend radius must be positive");
    }
    let body = ev.ctx.bodies[bi].clone();
    let edges = corner_edges(&body, data, parent, child);
    let convex: Vec<usize> = edges.iter().filter(|e| e.1).map(|e| e.0).collect();
    let concave: Vec<usize> = edges.iter().filter(|e| !e.1).map(|e| e.0).collect();
    if convex.len() != 1 || concave.len() != 1 {
        return err("The walls must meet in one sharp corner to make a bend");
    }
    let (eo, ei) = (convex[0], concave[0]);
    let shapes = [body.shape.sub_shape(ShapeType::Edge, eo)?, body.shape.sub_shape(ShapeType::Edge, ei)?];
    let res = k::fillet_radii(&body.shape, &shapes, &[r + t, r]).map_err(|e| fe(format!("The bend could not be made (radius too large?): {e}")))?;
    let names = ev.name_result(&[&body.shape_names()], &res.history, &res.shape, &BTreeMap::new(), Some(&|n: &Name| Role::Blend(n.clone())));
    ev.replace_body(bi, res.shape, names);
    let inner_name: Name = Arc::new(TopoName::face(ev.feature, Role::Blend(body.edges[ei].clone())));
    let outer_name: Name = Arc::new(TopoName::face(ev.feature, Role::Blend(body.edges[eo].clone())));
    let nb = ev.ctx.bodies[bi].clone();
    let inner = faces_named(&nb, std::slice::from_ref(&inner_name));
    let Some(&fi) = inner.first() else { return err("The bend faces were not created") };
    let face = nb.shape.sub_shape(ShapeType::Face, fi)?;
    let k::Surface::Cylinder { origin, axis, .. } = face.face_surface()? else { return err("The bend is not cylindrical") };
    let (Some((pa, na)), Some((cb, nbn))) = (wall_plane(&nb, &data.walls[parent]), wall_plane(&nb, &data.walls[child])) else {
        return err("The walls of the bend could not be found");
    };
    let axis = axis.normalize();
    let ext = extent_along(&face, origin, axis);
    let m = measure_bend(origin, axis, r, t, pa, na, pa, nbn, cb, ext);
    data.bends.push(Bend {
        feature: ev.feature,
        kind: BendKind::Bend,
        parent,
        child: Some(child),
        origin,
        axis: m.axis,
        angle: m.angle,
        radius: r,
        k_factor: kf,
        dir_a: m.dir_a,
        radial_a: m.radial_a,
        s0: m.s0,
        s1: m.s1,
        ref_inner: m.ref_inner,
        inner_faces: current_names(&nb, &[inner_name]),
        outer_faces: current_names(&nb, &[outer_name]),
    });
    data.rips.retain(|x| !((x.a == parent && x.b == child) || (x.a == child && x.b == parent)));
    Ok(())
}

/// Cut a rip (gap `minimal_clearance`) through the corner between two walls.
pub fn rip_corner(ev: &mut EvalCtx, bi: usize, data: &mut SheetMetalData, wa: usize, wb: usize, style: &str) -> FResult<()> {
    let t = data.thickness;
    let gap = data.minimal_clearance.max(1e-6);
    let body = ev.ctx.bodies[bi].clone();
    let edges = corner_edges(&body, data, wa, wb);
    let (Some(eo), Some(ei)) = (edges.iter().find(|e| e.1).map(|e| e.0), edges.iter().find(|e| !e.1).map(|e| e.0)) else {
        return err("The walls must meet in a sharp corner to rip");
    };
    let (Some((po0, po1)), Some((pi0, _))) = (line_of(&body.shape.sub_shape(ShapeType::Edge, eo)?), line_of(&body.shape.sub_shape(ShapeType::Edge, ei)?)) else {
        return err("Rip edges must be linear");
    };
    let e = (po1 - po0).normalize();
    let (sa, sb) = ((po0 - pi0).dot(e), (po1 - pi0).dot(e));
    let (smin, smax) = (sa.min(sb) - t, sa.max(sb) + t);
    let cross = (po0 - pi0) - e * (po0 - pi0).dot(e);
    let tool = if style == "BUTT" || style == "BUTT2" {
        let through = if style == "BUTT" { wa } else { wb };
        let ef = body.shape.edge_faces()?;
        let faces = &ef[ei];
        let fthrough = faces.iter().map(|f| *f as usize).find(|f| data.wall_of_face(&body.faces[*f]).map(|x| x.0) == Some(through));
        let Some(ft) = fthrough else { return err("The rip walls could not be found") };
        let Some((_, ni)) = plane_of(&body.shape.sub_shape(ShapeType::Face, ft)?) else { return err("Rip walls must be planar") };
        let across = cross - ni * cross.dot(ni);
        let x = across.normalize_or_zero();
        if x.length() < 0.5 {
            return err("The rip corner is degenerate");
        }
        let dz = e.dot(x.cross(ni));
        oriented_box(pi0, x, ni, [-t * 0.5, across.length() + t * 0.5], [0.0, gap], [smin * dz, smax * dz])?
    } else {
        let dist = cross.length();
        if dist < 1e-12 {
            return err("The rip corner is degenerate");
        }
        let x = cross / dist;
        let y = e.cross(x);
        oriented_box(pi0, x, y, [-t * 0.5, dist + t * 0.5], [-gap * 0.5, gap * 0.5], [smin, smax])?
    };
    let feature = ev.feature;
    let key = body.edges[eo].clone();
    let tool = tool_named(tool, |i| side_name(feature, &key, 80 + i as u16));
    let (shape, names) = bool_named(ev, k::BooleanOp::Cut, &body_named(&body), &[tool])?;
    if solid_count(&shape) > 1 {
        ev.warnings.push("The rip separated the part into several parts".into());
    }
    ev.replace_body(bi, shape, names);
    data.rips.push(Rip { feature, a: wa, b: wb, style: if style.is_empty() { "EDGE".into() } else { style.to_string() }, gap });
    Ok(())
}

/// Remove a bend's faces, restoring the sharp corner between its walls.
fn unbend(ev: &mut EvalCtx, bi: usize, bend: &Bend) -> FResult<()> {
    let body = ev.ctx.bodies[bi].clone();
    let mut names = bend.inner_faces.clone();
    names.extend(bend.outer_faces.iter().cloned());
    let faces: Vec<Shape> = faces_named(&body, &names).into_iter().filter_map(|i| body.shape.sub_shape(ShapeType::Face, i).ok()).collect();
    if faces.is_empty() {
        return err("The bend faces no longer exist");
    }
    let res = k::delete_faces(&body.shape, &faces).map_err(|e| fe(format!("The bend could not be removed: {e}")))?;
    if solid_count(&res.shape) != 1 {
        return err("The bend could not be removed");
    }
    let names = ev.name_result(&[&body.shape_names()], &res.history, &res.shape, &BTreeMap::new(), None);
    ev.replace_body(bi, res.shape, names);
    Ok(())
}

/// First resolved (body, sub-shape) of a face or edge reference.
fn resolve_one(ev: &EvalCtx, r: &Reference) -> Option<(usize, usize, bool)> {
    match r {
        Reference::Face(_) => ev.resolve_faces(std::slice::from_ref(r)).first().map(|&(b, f)| (b, f, true)),
        Reference::Edge(_) => ev.resolve_edges(std::slice::from_ref(r)).first().map(|&(b, e)| (b, e, false)),
        _ => None,
    }
}

pub fn make_joint(ev: &mut EvalCtx) -> FResult<()> {
    let refs = ev.query("entities").to_vec();
    if refs.is_empty() {
        return err("Select the sharp edge between two walls");
    }
    let mut corner: Option<(usize, usize, usize)> = None;
    for r in &refs {
        let Some((bi, si, is_face)) = resolve_one(ev, r) else { return err("A selected entity no longer exists") };
        if is_face {
            return err("Joining walls by side faces is not supported; select the sharp edge where the walls meet");
        }
        let data = active_data(ev, bi)?;
        let Some((wa, wb)) = walls_at_edge(&ev.ctx.bodies[bi], &data, si) else {
            return err("Select a sharp edge between two walls (walls that do not touch cannot be joined yet)");
        };
        let key = (bi, wa.min(wb), wa.max(wb));
        if corner.is_some_and(|c| c != key) {
            return err("Both edges must belong to the same corner");
        }
        corner = Some(key);
    }
    let Some((bi, wa, wb)) = corner else { return err("Select the sharp edge between two walls") };
    let mut data = (*active_data(ev, bi)?).clone();
    if ev.is("joint", "BEND") {
        let r = if ev.bool("useDefaultRadius") { data.radius } else { ev.length("radius")? };
        let kf = data.k_factor;
        bend_corner(ev, bi, &mut data, wa, wb, r, kf)?;
    } else {
        let style = ev.enum_("jointType").to_string();
        rip_corner(ev, bi, &mut data, wa, wb, &style)?;
    }
    put_data(ev, bi, data);
    Ok(())
}

pub fn modify_joint(ev: &mut EvalCtx) -> FResult<()> {
    let Some(r) = ev.query("entity").first().cloned() else { return err("Select a bend or rip") };
    let Some((bi, si, is_face)) = resolve_one(ev, &r) else { return err("The selected joint no longer exists") };
    let mut data = (*active_data(ev, bi)?).clone();
    let body = ev.ctx.bodies[bi].clone();
    let bend = if is_face {
        data.bend_of_face(&body.faces[si])
    } else {
        let ef = body.shape.edge_faces()?;
        ef.get(si).and_then(|fs| fs.iter().find_map(|f| data.bend_of_face(&body.faces[*f as usize])))
    };
    let ty = ev.enum_("jointType").to_string();
    let r_new = if ev.bool("useDefaultRadius") { data.radius } else { ev.length("radius")? };
    let k_new = if ev.bool("useDefaultKFactor") { data.k_factor } else { ev.real("kFactor")? };
    match bend {
        Some(b) => {
            let old = data.bends[b].clone();
            let Some(child) = old.child else { return err("Hems and rolled walls cannot be modified") };
            if old.kind != BendKind::Bend {
                return err("Hems and rolled walls cannot be modified");
            }
            match ty.as_str() {
                "TANGENT" => return err("Tangent joints are only possible between rolled walls"),
                "RIP" => {
                    unbend(ev, bi, &old)?;
                    data.bends.remove(b);
                    let style = ev.enum_("jointStyle").to_string();
                    rip_corner(ev, bi, &mut data, old.parent, child, &style)?;
                }
                _ => {
                    if (r_new - old.radius).abs() < 1e-12 {
                        data.bends[b].k_factor = k_new;
                    } else {
                        unbend(ev, bi, &old)?;
                        data.bends.remove(b);
                        bend_corner(ev, bi, &mut data, old.parent, child, r_new, k_new)?;
                    }
                }
            }
        }
        None => {
            if is_face {
                return err("Select a bend face or a joint edge");
            }
            let Some((wa, wb)) = walls_at_edge(&body, &data, si) else { return err("The selection is not a sheet metal joint") };
            match ty.as_str() {
                "BEND" => bend_corner(ev, bi, &mut data, wa.min(wb), wa.max(wb), r_new, k_new)?,
                "RIP" => {
                    if data.rips.iter().any(|x| (x.a == wa && x.b == wb) || (x.a == wb && x.b == wa)) {
                        return err("The joint is already a rip");
                    }
                    let style = ev.enum_("jointStyle").to_string();
                    rip_corner(ev, bi, &mut data, wa, wb, &style)?
                }
                _ => return err("Tangent joints are only possible between rolled walls"),
            }
        }
    }
    put_data(ev, bi, data);
    Ok(())
}

// ------------------------------------------------------------------ tab

pub fn tab(ev: &mut EvalCtx) -> FResult<()> {
    let refs = ev.query("tabFaces");
    if refs.is_empty() {
        return err("Select tab profile regions or faces");
    }
    let profiles = ev.profiles(refs)?;
    if profiles.is_empty() {
        return err("No valid tab profiles");
    }
    let scope: Vec<(usize, usize)> = ev.resolve_faces(ev.query("booleanUnionScope"));
    let offset = ev.length("booleanOffset").unwrap_or(0.0);
    let subtract = ev.query("booleanSubtractScope").to_vec();
    for p in &profiles {
        // Wall face coplanar with the tab.
        let mut found: Option<(usize, usize, bool, DVec3)> = None;
        'outer: for bi in 0..ev.ctx.bodies.len() {
            let body = ev.ctx.bodies[bi].clone();
            let Some(data) = ev.ctx.sheet_metal.get(&body.id).filter(|d| d.active) else { continue };
            for (wi, w) in data.walls.iter().enumerate() {
                for (names, on_ref) in [(&w.faces, true), (&w.back, false)] {
                    for fi in faces_named(&body, names) {
                        if !scope.is_empty() && !scope.contains(&(bi, fi)) {
                            continue;
                        }
                        let Some((c, n)) = plane_of(&body.shape.sub_shape(ShapeType::Face, fi)?) else { continue };
                        if n.dot(p.frame.normal).abs() > 1.0 - 1e-9 && (p.frame.origin - c).dot(n).abs() < 1e-7 {
                            found = Some((bi, wi, on_ref, n));
                            break 'outer;
                        }
                    }
                }
            }
        }
        let Some((bi, wi, on_ref, n)) = found else { return err("The tab must lie in the plane of a sheet metal wall") };
        let mut data = (*active_data(ev, bi)?).clone();
        let t = data.thickness;
        let res = k::extrude(&p.shape, -n * t)?;
        let names = sweep_names(ev, p, &res, true, true);
        let prism = NamedShape::new(res.shape, names);
        let feature = ev.feature;
        let key: Name = match &p.key {
            crate::reference::Source::Topo(n) => n.clone(),
            other => Arc::new(TopoName::face(feature, Role::Profile(other.clone()))),
        };
        // Subtraction scope: walls (faces) or other parts crossing the tab.
        let grown = if offset > 1e-12 { k::offset_shape(&prism.shape, offset, k::JoinType::Intersection).map(|r| r.shape)? } else { prism.shape.clone() };
        for r in &subtract {
            let one = std::slice::from_ref(r);
            let targets: Vec<(usize, Option<Shape>)> = match r {
                Reference::Face(_) => ev
                    .resolve_faces(one)
                    .into_iter()
                    .filter_map(|(b2, f2)| {
                        let body = &ev.ctx.bodies[b2];
                        let face = body.shape.sub_shape(ShapeType::Face, f2).ok()?;
                        let (_, n2) = plane_of(&face)?;
                        let t2 = ev.ctx.sheet_metal.get(&body.id).map(|d| d.thickness).unwrap_or(t);
                        let slab = k::extrude(&face, -n2 * t2).ok()?.shape;
                        Some((b2, Some(slab)))
                    })
                    .collect(),
                _ => ev.resolve_bodies(one).into_iter().map(|b2| (b2, None)).collect(),
            };
            for (b2, slab) in targets {
                let region = match slab {
                    Some(s) => k::common(&grown, &s)?.shape,
                    None => grown.clone(),
                };
                if region.count(ShapeType::Solid) == 0 {
                    continue;
                }
                let body = ev.ctx.bodies[b2].clone();
                let tool = tool_named(region, |i| side_name(feature, &key, 200 + i as u16));
                let (s, names) = bool_named(ev, k::BooleanOp::Cut, &body_named(&body), &[tool])?;
                ev.replace_body(b2, s, names);
            }
        }
        let body = ev.ctx.bodies[bi].clone();
        let (shape, names) = bool_named(ev, k::BooleanOp::Fuse, &body_named(&body), &[prism])?;
        if solid_count(&shape) != 1 {
            return err("The tab does not touch the wall");
        }
        ev.replace_body(bi, shape, names);
        let nb = ev.ctx.bodies[bi].clone();
        let start: Name = Arc::new(TopoName::face(feature, Role::CapStart(Some(p.key.clone()))));
        let end: Name = Arc::new(TopoName::face(feature, Role::CapEnd(Some(p.key.clone()))));
        let (rf, bk) = if on_ref { (start, end) } else { (end, start) };
        let w = &mut data.walls[wi];
        w.faces.extend(current_names(&nb, &[rf]));
        w.back.extend(current_names(&nb, &[bk]));
        put_data(ev, bi, data);
    }
    Ok(())
}

// ------------------------------------------------------------------ bend

pub fn bend(ev: &mut EvalCtx) -> FResult<()> {
    let Some(fr) = ev.query("face").first().cloned() else { return err("Select the sheet metal face to bend") };
    let Some(&(bi, fi)) = ev.resolve_faces(std::slice::from_ref(&fr)).first() else { return err("The face to bend no longer exists") };
    let mut data = (*active_data(ev, bi)?).clone();
    let body = ev.ctx.bodies[bi].clone();
    let Some((w, on_ref)) = data.wall_of_face(&body.faces[fi]) else { return err("Select a planar face of a sheet metal wall") };
    let face = body.shape.sub_shape(ShapeType::Face, fi)?;
    let Some((fc, n)) = plane_of(&face) else { return err("The face to bend must be planar") };
    let Some(lr) = ev.query("bendReference").first() else { return err("Select a bend line") };
    let Some((lp, ld)) = ev.resolve_axis(lr) else { return err("The bend line is not a line") };
    let ld = ld - n * ld.dot(n);
    if ld.length() < 1e-9 {
        return err("The bend line is perpendicular to the face");
    }
    let d = ld.normalize();
    let lp = lp - n * (lp - fc).dot(n);
    let mut wdir = n.cross(d);
    if (fc - lp).dot(wdir) > 0.0 {
        wdir = -wdir;
    }
    if ev.bool("holdOtherSide") {
        wdir = -wdir;
    }
    let up = if ev.bool("oppositeAngle") { -n } else { n };
    let theta = match ev.enum_("angleControlType") {
        "ALIGN_GEOMETRY" | "ANGLE_FROM_DIRECTION" => {
            let id = if ev.is("angleControlType", "ALIGN_GEOMETRY") { "parallelEntity" } else { "directionEntity" };
            let r = ev.query(id).first().ok_or_else(|| fe("Select a direction"))?;
            let e = ev.resolve_direction(r).ok_or_else(|| fe("The selected entity has no direction"))?;
            let (x, y) = (e.dot(wdir), e.dot(up));
            if (x * x + y * y).sqrt() < 1e-9 {
                return err("The direction is parallel to the bend line");
            }
            let mut th = y.atan2(x);
            if ev.is("angleControlType", "ANGLE_FROM_DIRECTION") {
                let a = ev.angle("angleFromDirection")?;
                th += if ev.bool("oppositeAngleFromDirection") { -a } else { a };
                th = th.rem_euclid(2.0 * std::f64::consts::PI);
            } else if th <= 0.0 {
                th += std::f64::consts::PI;
            }
            th
        }
        _ => ev.angle("bendAngle")?,
    };
    if theta <= 1e-6 || theta >= 2.0 * std::f64::consts::PI - 1e-6 {
        return err("The bend angle is too small");
    }
    let r = if ev.bool("useDefaultRadius") { data.radius } else { ev.length("bendRadius")? };
    let kf = if ev.bool("useDefaultKFactor") { data.k_factor } else { ev.real("kFactor")? };
    if r <= 0.0 {
        return err("Bend radius must be positive");
    }
    let t = data.thickness;
    let ba = theta * (r + kf * t);
    let th = (theta.min(std::f64::consts::FRAC_PI_2) * 0.5).tan();
    let w0 = match ev.enum_("bendAlignment") {
        "HELD_EDGE" => 0.0,
        "BENT_EDGE" => -ba,
        "BENT_INSIDE" => -r * th,
        "BENT_OUTSIDE" => -(r + t) * th,
        "BENT_MIDPLANE" => -(r + t * 0.5) * th,
        _ => -ba * 0.5,
    };
    let big = ev.model_extent();
    let feature = ev.feature;
    let fname = body.faces[fi].clone();
    let side = |p: DVec3| (p - lp).dot(wdir);
    // The strip that becomes the bend must be part of this wall only.
    let strip = k::common(&body.shape, &oriented_box(lp, wdir, n, [w0, w0 + ba], [-big, big], [-big, big])?)?.shape;
    if strip.count(ShapeType::Solid) == 0 {
        return err("The bend line does not cross the face");
    }
    for v in strip.vertices() {
        let h = (v.vertex_point()? - fc).dot(n);
        if h > 1e-7 || h < -t - 1e-7 {
            return err("The bend region crosses other walls; only flat regions can be bent");
        }
    }
    let named = body_named(&body);
    let cut_h = tool_named(oriented_box(lp, wdir, n, [w0, big], [-big, big], [-big, big])?, |i| side_name(feature, &fname, 60 + i as u16));
    let cut_m = tool_named(oriented_box(lp, wdir, n, [-big, w0 + ba], [-big, big], [-big, big])?, |i| side_name(feature, &fname, 70 + i as u16));
    let (hs, hn) = bool_named(ev, k::BooleanOp::Cut, &named, &[cut_h])?;
    let (ms, mn) = bool_named(ev, k::BooleanOp::Cut, &named, &[cut_m])?;
    if hs.count(ShapeType::Solid) == 0 || ms.count(ShapeType::Solid) == 0 {
        return err("The bend line must split the face into two parts");
    }
    // Bend region: revolve the held part's end faces.
    let ip = if up.dot(n) > 0.0 { lp } else { lp - n * t };
    let axis_pt = ip + wdir * w0 + up * r;
    let a = wdir.cross(up);
    let ends: Vec<Shape> = hs
        .faces()
        .into_iter()
        .filter(|f| plane_of(f).is_some_and(|(c, nn)| nn.cross(wdir).length() < 1e-9 && (side(c) - w0).abs() < 1e-7))
        .collect();
    if ends.is_empty() {
        return err("The bend line does not cross the face");
    }
    let mut tools = Vec::new();
    for e in &ends {
        let res = k::revolve(e, axis_pt, a, theta)?;
        let mut roles = BTreeMap::new();
        for (j, f) in res.shape.faces().iter().enumerate() {
            let kk = match f.face_surface() {
                Ok(k::Surface::Cylinder { radius, .. }) if (radius - r).abs() < 1e-7 => 0,
                Ok(k::Surface::Cylinder { .. }) => 1,
                _ => 2,
            };
            roles.insert(j as u32, Role::Side(piece_source(&fname, feature, kk)));
        }
        let names = ev.name_result(&[&Default::default()], &res.history, &res.shape, &roles, None);
        tools.push(NamedShape::new(res.shape, names));
    }
    let m = DMat4::from_translation(axis_pt) * DMat4::from_axis_angle(a, theta) * DMat4::from_translation(-axis_pt) * DMat4::from_translation(-wdir * ba);
    let mv = k::transform(&ms, &k::Transform::Matrix(m), true)?;
    let m_names = crate::regen::eval::names_for(&ms, mn);
    let mv_faces = ev.name_result(&[&m_names], &mv.history, &mv.shape, &BTreeMap::new(), None);
    tools.push(NamedShape::new(mv.shape, mv_faces));
    let held = NamedShape::new(hs, hn);
    let (shape, names) = bool_named(ev, k::BooleanOp::Fuse, &held, &tools)?;
    if solid_count(&shape) != 1 {
        return err("The bent part is not connected");
    }
    ev.replace_body(bi, shape, names);
    // Update the sheet metal model.
    let nb = ev.ctx.bodies[bi].clone();
    let moving_wall = |wi: usize| -> bool {
        let Some(&f) = faces_named(&body, &data.walls[wi].faces).first() else { return false };
        body.shape.sub_shape(ShapeType::Face, f).ok().and_then(|f| f.centroid().ok()).is_some_and(|c| side(c) > w0 + ba * 0.5)
    };
    let moving: Vec<bool> = (0..data.walls.len()).map(|i| i != w && moving_wall(i)).collect();
    let split_faces = |names: &[Name], plane_h: f64| -> (Vec<Name>, Vec<Name>) {
        let (mut hold, mut moved) = (Vec::new(), Vec::new());
        for fi in faces_named(&nb, names) {
            let Ok(f) = nb.shape.sub_shape(ShapeType::Face, fi) else { continue };
            let Ok(c) = f.centroid() else { continue };
            if ((c - fc).dot(n) - plane_h).abs() < 1e-7 && side(c) < w0 + 1e-9 {
                hold.push(nb.faces[fi].clone());
            } else {
                moved.push(nb.faces[fi].clone());
            }
        }
        (hold, moved)
    };
    let (ref_h, back_h) = if on_ref { (0.0, -t) } else { (-t, 0.0) };
    let (rh, rm) = split_faces(&data.walls[w].faces, ref_h);
    let (bh, bm) = split_faces(&data.walls[w].back, back_h);
    data.walls[w].faces = rh;
    data.walls[w].back = bh;
    data.walls.push(Wall { feature, faces: rm, back: bm });
    let w2 = data.walls.len() - 1;
    for b in data.bends.iter_mut() {
        let mid = b.origin + b.axis * (b.s0 + b.s1) * 0.5;
        let on_moving_side = side(mid) > w0 + ba * 0.5;
        let touches_w = b.parent == w || b.child == Some(w);
        let all_moving = moving.get(b.parent).copied().unwrap_or(false) && b.child.is_none_or(|c| moving.get(c).copied().unwrap_or(false));
        if touches_w && on_moving_side {
            if b.parent == w {
                b.parent = w2;
            }
            if b.child == Some(w) {
                b.child = Some(w2);
            }
            b.transform(&m);
        } else if !touches_w && all_moving {
            b.transform(&m);
        }
    }
    let mut all_ends = ends[0].clone();
    if ends.len() > 1 {
        all_ends = Shape::compound(&ends);
    }
    let (s0, s1) = extent_along(&all_ends, axis_pt, a);
    let ref_inner = on_ref == (up.dot(n) > 0.0);
    data.bends.push(Bend {
        feature,
        kind: BendKind::Bend,
        parent: w,
        child: Some(w2),
        origin: axis_pt,
        axis: a,
        angle: theta,
        radius: r,
        k_factor: kf,
        dir_a: -wdir,
        radial_a: -up,
        s0,
        s1,
        ref_inner,
        inner_faces: current_names(&nb, &[side_name(feature, &fname, 0)]),
        outer_faces: current_names(&nb, &[side_name(feature, &fname, 1)]),
    });
    put_data(ev, bi, data);
    Ok(())
}

// ------------------------------------------------------------------ reliefs

/// Point and body of a vertex or edge reference.
fn point_of(ev: &EvalCtx, r: &Reference) -> Option<(usize, DVec3)> {
    match r {
        Reference::Vertex(n) => {
            for (bi, b) in ev.ctx.bodies.iter().enumerate() {
                if let Some(vi) = b.find_vertices(n).first() {
                    return Some((bi, b.shape.sub_shape(ShapeType::Vertex, *vi).ok()?.vertex_point().ok()?));
                }
            }
            None
        }
        Reference::Edge(_) => {
            let (bi, ei) = *ev.resolve_edges(std::slice::from_ref(r)).first()?;
            let (a, b) = ev.ctx.bodies[bi].shape.sub_shape(ShapeType::Edge, ei).ok()?.endpoints().ok()?;
            Some((bi, (a + b) * 0.5))
        }
        _ => None,
    }
}

/// Cut a bend relief at end `s_end` (outside toward `sign`) of a bend.
#[allow(clippy::too_many_arguments)]
pub fn cut_relief(ev: &mut EvalCtx, bi: usize, b: &Bend, t: f64, s_end: f64, sign: f64, style: BendReliefStyle, depth: f64, width: f64, key: &Name) -> FResult<()> {
    let u = -b.dir_a;
    let v = -b.radial_a;
    // Local frame: u = 0 at the bend start on the inner face plane.
    let o = b.origin - v * b.radius;
    let top = (b.radius + t) * 1.5 + t;
    let segs = relief_segments(style, -depth, top, s_end, sign * width);
    let face = local_face(o - v * t, u, b.axis, &segs)?;
    let prism = k::extrude(&face.shape, v * t)?.shape;
    let feature = ev.feature;
    let tool = tool_named(prism, |i| side_name(feature, key, 300 + i as u16));
    let body = ev.ctx.bodies[bi].clone();
    let (shape, names) = bool_named(ev, k::BooleanOp::Cut, &body_named(&body), &[tool])?;
    if solid_count(&shape) != 1 {
        return err("The relief would split the part");
    }
    ev.replace_body(bi, shape, names);
    Ok(())
}

pub fn bend_relief(ev: &mut EvalCtx) -> FResult<()> {
    let Some(r) = ev.query("bendRelief").first().cloned() else { return err("Select a vertex or edge at the end of a bend") };
    let Some((bi, p)) = point_of(ev, &r) else { return err("The selected bend relief corner no longer exists") };
    let data = (*active_data(ev, bi)?).clone();
    let t = data.thickness;
    let mut best: Option<(usize, f64, f64, f64)> = None;
    for (i, b) in data.bends.iter().enumerate() {
        for (s, sign) in [(b.s0, -1.0), (b.s1, 1.0)] {
            let c = b.origin + b.axis * s;
            let dist = (p - c).length();
            if dist < (b.radius + t) * 2.0 + t && best.is_none_or(|x| dist < x.1) {
                best = Some((i, dist, s, sign));
            }
        }
    }
    let Some((i, _, s_end, sign)) = best else { return err("The selection is not at the end of a bend") };
    let b = data.bends[i].clone();
    let style_id = ev.enum_("bendReliefStyle").to_string();
    let style = BendReliefStyle::from_id(&style_id);
    let (style, depth, width) = match style_id.as_str() {
        "TEAR" => (BendReliefStyle::Rectangle, 0.0, data.minimal_clearance.max(1e-5)),
        "SIZED_RECTANGLE" => {
            let dd = ev.length("bendReliefDepth")?;
            (style, dd, dd)
        }
        "SIZED_OBROUND" => (style, ev.length("bendReliefDepth")?, t * 1.0625),
        _ => (style, ((ev.real("bendReliefDepthScale")? - 1.0) * b.radius).max(t), t * ev.real("bendReliefWidthScale")?),
    };
    if ev.bool("extendBendRelief") {
        ev.warnings.push("Extend bend relief is not applied".into());
    }
    let key = b.inner_faces.first().cloned().unwrap_or_else(|| Arc::new(TopoName::face(ev.feature, Role::New(i as u32))));
    cut_relief(ev, bi, &b, t, s_end, sign, style, depth, width, &key)?;
    put_data(ev, bi, data);
    Ok(())
}

// ------------------------------------------------------------------ corner break

pub fn corner_break(ev: &mut EvalCtx) -> FResult<()> {
    let refs = ev.query("entities").to_vec();
    if refs.is_empty() {
        return err("Select corners (vertices or edges through the thickness) to fillet or chamfer");
    }
    let mut groups: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for r in &refs {
        match r {
            Reference::Edge(_) => {
                for (bi, ei) in ev.resolve_edges(std::slice::from_ref(r)) {
                    groups.entry(bi).or_default().push(ei);
                }
            }
            Reference::Vertex(n) => {
                let mut hit = false;
                for (bi, b) in ev.ctx.bodies.iter().enumerate() {
                    let Some(&vi) = b.find_vertices(n).first() else { continue };
                    let Some(data) = ev.ctx.sheet_metal.get(&b.id) else { continue };
                    let t = data.thickness;
                    let ve = b.shape.vertex_edges()?;
                    for &ei in ve.get(vi).map(|v| v.as_slice()).unwrap_or(&[]) {
                        let e = b.shape.sub_shape(ShapeType::Edge, ei as usize)?;
                        if let Some((p, q)) = line_of(&e)
                            && ((p - q).length() - t).abs() < t * 1e-3
                        {
                            groups.entry(bi).or_default().push(ei as usize);
                            hit = true;
                        }
                    }
                }
                if !hit {
                    return err("The selected vertex is not a sheet metal corner");
                }
            }
            _ => {}
        }
    }
    if groups.is_empty() {
        return err("Selected corners no longer exist");
    }
    let fillet = !ev.is("blendType", "CHAMFER");
    for (bi, mut edges) in groups.into_iter().rev() {
        active_data(ev, bi)?;
        edges.sort_unstable();
        edges.dedup();
        let body = ev.ctx.bodies[bi].clone();
        let shapes: Vec<Shape> = edges.iter().filter_map(|e| body.shape.sub_shape(ShapeType::Edge, *e).ok()).collect();
        let res = if fillet {
            let radius = if ev.is("blendControlType", "WIDTH") { ev.length("filletWidth")? / std::f64::consts::SQRT_2 } else { ev.length("radius")? };
            k::fillet(&body.shape, &shapes, radius).map_err(|e| fe(format!("Corner break failed (radius too large?): {e}")))?
        } else {
            let spec = match ev.enum_("chamferType") {
                "TWO_OFFSETS" => k::ChamferSpec::TwoDistances(ev.length("width1")?, ev.length("width2")?),
                "OFFSET_ANGLE" => k::ChamferSpec::DistanceAngle(ev.length("width")?, ev.angle("angle")?),
                _ => k::ChamferSpec::Equal(ev.length("width")?),
            };
            k::chamfer(&body.shape, &shapes, spec, &[]).map_err(|e| fe(format!("Corner break failed: {e}")))?
        };
        let names = ev.name_result(&[&body.shape_names()], &res.history, &res.shape, &BTreeMap::new(), Some(&|n: &Name| Role::Blend(n.clone())));
        ev.replace_body(bi, res.shape, names);
    }
    Ok(())
}
