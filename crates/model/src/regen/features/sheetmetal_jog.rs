//! Sheet metal Jog (03c SM-06, FS `sheetMetalJog`): offset part of a wall
//! with two opposite bends along a line. The first bend follows the Bend
//! inputs (alignment, angle control, radius, K factor); the second bends the
//! moved wall back by the same angle with "Hold line" alignment, at the
//! distance that gives the requested offset. Both bends are recorded in the
//! sheet metal model, so the flat pattern unrolls them like any bend.

use super::sheetmetal_ops::{BendInputs, BendSpec, alignment_start, bend_core, bend_inputs};
use super::sheetmetal_util::*;
use crate::reference::Reference;
use crate::regen::eval::{EvalCtx, FResult, err};
use crate::sheetmetal::SheetMetalData;
use glam::{DMat4, DVec3};
use noshape_kernel as k;
use noshape_kernel::{Shape, ShapeType};
use std::collections::BTreeMap;

/// Displacement of the jogged wall (the "nominal" offset) from the inputs.
fn jog_offset(ev: &EvalCtx, t: f64, fc: DVec3, up: DVec3) -> FResult<f64> {
    let anchor = ev.enum_("bendOffsetAnchor");
    match ev.enum_("offsetType") {
        "UP_TO_ENTITY" => {
            let Some(r) = ev.query("jogLimit").first() else { return err("Select the entity to jog up to") };
            let p = match r {
                Reference::Face(_) | Reference::Plane(_) | Reference::FeaturePlane(_) => ev.resolve_plane(r).map(|f| f.origin).or_else(|| {
                    let (bi, fi) = *ev.resolve_faces(std::slice::from_ref(r)).first()?;
                    ev.face_shape(bi, fi).ok()?.centroid().ok()
                }),
                Reference::Edge(_) => {
                    let (bi, ei) = *ev.resolve_edges(std::slice::from_ref(r)).first().ok_or_else(|| fe("The jog limit no longer exists"))?;
                    ev.edge_shape(bi, ei)?.endpoints().ok().map(|(a, b)| (a + b) * 0.5)
                }
                _ => ev.resolve_vertex_point(r),
            };
            let Some(p) = p else { return err("The jog limit no longer exists") };
            let mut d = (p - fc).dot(up);
            if d < -1e-9 {
                return err("The jog bends away from the selected entity; use Opposite angle");
            }
            if ev.bool("jogLimitOffset") {
                let o = ev.length("jogLimitDistance")?;
                d += if ev.bool("jogLimitOppositeDirection") { -o } else { o };
            }
            // `d` reaches the selected face's plane; the anchor picks which
            // sheet face of the jogged wall ends on the entity.
            Ok(match anchor {
                "INSIDE" => d + 0.5 * t,
                "OUTSIDE" => d - 0.5 * t,
                _ => d,
            })
        }
        ty => {
            let base = if ty == "THICKNESS" { t * ev.real("thicknessFactor")? } else { ev.length("bendOffset")? };
            Ok(match anchor {
                "INSIDE" => base + t,
                "OUTSIDE" => base - t,
                _ => base,
            })
        }
    }
}

/// Planar wall face of `wall` (reference or back side) containing `p`.
fn face_through(ev: &EvalCtx, bi: usize, data: &SheetMetalData, wall: usize, on_ref: bool, p: DVec3, n: DVec3) -> Option<usize> {
    let body = &ev.ctx.bodies[bi];
    let names = if on_ref { &data.walls[wall].faces } else { &data.walls[wall].back };
    let mut best: Option<(usize, f64)> = None;
    for fi in faces_named(body, names) {
        let f = body.shape.sub_shape(ShapeType::Face, fi).ok()?;
        let Some((c, nn)) = plane_of(&f) else { continue };
        if nn.dot(n) < 1.0 - 1e-7 || (p - c).dot(nn).abs() > 1e-6 {
            continue;
        }
        let d = f.project_point(p).map(|q| q.distance).unwrap_or(f64::INFINITY);
        if best.is_none_or(|b| d < b.1) {
            best = Some((fi, d));
        }
    }
    best.map(|b| b.0)
}

pub fn jog(ev: &mut EvalCtx) -> FResult<()> {
    let BendInputs { bi, fi, mut data, lp, wdir, up, n, mut theta, r, kf } = bend_inputs(ev)?;
    if theta >= std::f64::consts::PI - 1e-6 {
        return err("The jog angle must be smaller than 180°");
    }
    let t = data.thickness;
    let body = ev.ctx.bodies[bi].clone();
    let Some((_, on_ref)) = data.wall_of_face(&body.faces[fi]) else { return err("Select a planar face of a sheet metal wall") };
    let fc = body.shape.sub_shape(ShapeType::Face, fi)?.centroid()?;
    let offset = jog_offset(ev, t, fc, up)?;
    // Offset of two adjacent bends (no wall between them): R(1-cos) + (R+T)(1-cos).
    let adjacent = (2.0 * r + t) * (1.0 - theta.cos());
    // Std applyEditLogicForBendAngle: an offset the bends can't reach at this
    // angle sets the angle that makes them adjacent (Info).
    let cos_adjacent = 1.0 - offset / (2.0 * r + t);
    if ev.is("angleControlType", "BEND_ANGLE") && cos_adjacent.abs() < 1.0 && offset - adjacent <= 1e-8 {
        theta = cos_adjacent.acos();
        ev.infos.push("Updating bend angle in order to accommodate the specified jog offset.".into());
    }
    let adjacent = (2.0 * r + t) * (1.0 - theta.cos());
    let wall = ((offset - adjacent) / theta.sin()).max(0.0);
    let ba = theta * (r + kf * t);
    let w0 = alignment_start(ev.enum_("bendAlignment"), theta, r, t, ba);
    let spec1 = BendSpec { lp, n, wdir, up, theta, r, kf, w0, kbase: 0 };
    let (_, _, m1) = spec1.fold(t);
    let wdir2 = m1.transform_vector3(wdir).normalize();
    let n2 = m1.transform_vector3(n).normalize();
    let lp2 = m1.transform_point3(lp + wdir * (w0 + ba)) + wdir2 * wall;
    let spec2 = BendSpec { lp: lp2, n: n2, wdir: wdir2, up: -m1.transform_vector3(up).normalize(), theta, r, kf, w0: 0.0, kbase: 100 };
    let fi = if ev.bool("preserveMaterial") {
        fi
    } else {
        // Add the material the jog consumes so the far end stays in place.
        let (_, _, m2) = spec2.fold(t);
        let extra = -((m2 * m1).transform_point3(lp) - lp).dot(wdir);
        let w = data.wall_of_face(&body.faces[fi]).map(|x| x.0).unwrap_or(0);
        if extra > 1e-9 {
            add_material(ev, bi, &mut data, lp, wdir, n, w0 + 2.0 * ba + wall + t, extra, &body.faces[fi])?;
        }
        // The selected face may have been split by the insertion.
        face_through(ev, bi, &data, w, on_ref, lp + wdir * (w0 + ba * 0.5), n).ok_or_else(|| fe("The face to jog could not be found after adding material"))?
    };
    let (w2, _) = bend_core(ev, bi, fi, &mut data, &spec1)?;
    // Second bend on the moved wall, on the same sheet side.
    let probe = lp2 + wdir2 * (ba * 0.5);
    let Some(f2) = face_through(ev, bi, &data, w2, on_ref, probe, n2) else {
        return err("The jogged wall is too short for the second bend");
    };
    bend_core(ev, bi, f2, &mut data, &spec2).map_err(|e| fe(format!("Second jog bend: {}", e.message)))?;
    put_data(ev, bi, data);
    Ok(())
}

/// Split the body across `wdir` at `s_ins` (from `lp`), move the far side by
/// `extra` and bridge the gap with the wall's section (Preserve material off).
#[allow(clippy::too_many_arguments)]
fn add_material(ev: &mut EvalCtx, bi: usize, data: &mut SheetMetalData, lp: DVec3, wdir: DVec3, n: DVec3, s_ins: f64, extra: f64, key: &crate::naming::Name) -> FResult<()> {
    let body = ev.ctx.bodies[bi].clone();
    let big = ev.model_extent();
    let feature = ev.feature;
    let named = body_named(&body);
    let cut_far = tool_named(oriented_box(lp, wdir, n, [s_ins, big], [-big, big], [-big, big])?, |i| side_name(feature, key, 90 + i as u16));
    let cut_near = tool_named(oriented_box(lp, wdir, n, [-big, s_ins], [-big, big], [-big, big])?, |i| side_name(feature, key, 96 + i as u16));
    let (hs, hn) = bool_named(ev, k::BooleanOp::Cut, &named, &[cut_far])?;
    let (ms, mn) = bool_named(ev, k::BooleanOp::Cut, &named, &[cut_near])?;
    if hs.count(ShapeType::Solid) == 0 || ms.count(ShapeType::Solid) == 0 {
        return err("The face is too short to add material for the jog");
    }
    let side = |p: DVec3| (p - lp).dot(wdir);
    let ends: Vec<Shape> = hs.faces().into_iter().filter(|f| plane_of(f).is_some_and(|(c, nn)| nn.cross(wdir).length() < 1e-9 && (side(c) - s_ins).abs() < 1e-7)).collect();
    if ends.is_empty() {
        return err("The face is too short to add material for the jog");
    }
    let mv = DMat4::from_translation(wdir * extra);
    let moved = k::transform(&ms, &k::Transform::Matrix(mv), true)?;
    let m_names = crate::regen::eval::names_for(&ms, mn);
    let moved_names = ev.name_result(&[&m_names], &moved.history, &moved.shape, &BTreeMap::new(), None);
    let mut tools = vec![crate::regen::eval::NamedShape::new(moved.shape, moved_names)];
    let mut bridge_names = Vec::new();
    for (j, e) in ends.iter().enumerate() {
        let res = k::extrude(e, wdir * extra)?;
        let names: Vec<_> = (0..res.shape.count(ShapeType::Face) as u16).map(|i| side_name(feature, key, 110 + (j as u16) * 16 + i)).collect();
        bridge_names.extend(names.iter().cloned());
        tools.push(crate::regen::eval::NamedShape::new(res.shape, names));
    }
    let held = crate::regen::eval::NamedShape::new(hs, hn);
    let (shape, names) = bool_named(ev, k::BooleanOp::Fuse, &held, &tools)?;
    if solid_count(&shape) != 1 {
        return err("Adding material for the jog failed");
    }
    ev.replace_body(bi, shape, names);
    // Bridge faces in the wall planes join the wall they extend; far bends
    // and marks move with the far side.
    let nb = ev.ctx.bodies[bi].clone();
    let wall = data.wall_of_face(key).map(|x| x.0);
    if let Some(w) = wall {
        for fi in faces_named(&nb, &bridge_names) {
            let Ok(f) = nb.shape.sub_shape(ShapeType::Face, fi) else { continue };
            let Some((_, nn)) = plane_of(&f) else { continue };
            if nn.cross(n).length() > 1e-9 {
                continue;
            }
            // Same side as the wall's reference faces?
            let ref_face = faces_named(&nb, &data.walls[w].faces).first().and_then(|&i| nb.shape.sub_shape(ShapeType::Face, i).ok()).and_then(|f| plane_of(&f));
            let on_ref = ref_face.is_some_and(|(_, rn)| rn.dot(nn) > 0.0);
            let name = nb.faces[fi].clone();
            if on_ref {
                data.walls[w].faces.push(name);
            } else {
                data.walls[w].back.push(name);
            }
        }
    }
    for b in data.bends.iter_mut() {
        if side(b.origin + b.axis * (b.s0 + b.s1) * 0.5) > s_ins {
            b.transform(&mv);
        }
    }
    for mk in data.marks.iter_mut() {
        if mk.center().is_some_and(|c| side(c) > s_ins) {
            mk.transform(&mv);
        }
    }
    Ok(())
}
