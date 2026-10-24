//! Sheet metal Rip (split a wall between vertex pairs), Recognize (turn a
//! constant-thickness solid into a sheet metal model) and Unfold / Refold
//! (flatten selected bends in the model, e.g. to cut across them, and fold
//! them back).
//!
//! Unfold replaces each bend region by the developed flat strip (the near
//! side's end section extruded by the bend allowance) and moves everything
//! past the bend into its flat position; the bend stays in the model marked
//! `flat`, so the flat pattern is unchanged. Refold removes the strip and
//! re-bends from the strip's start: cuts made inside the strip only carry
//! into the bend where they cross the strip's start line.

use super::sheetmetal_util::*;
use crate::naming::Name;
use crate::reference::{Reference, Role, TopoName};
use crate::regen::eval::{EvalCtx, FResult, NamedShape, err, err_at};
use crate::sheetmetal::{Bend, BendKind, SheetMetalData, Wall};
use glam::{DMat4, DVec3};
use noshape_kernel as k;
use noshape_kernel::{Shape, ShapeType};
use std::collections::{BTreeMap, VecDeque};
use std::sync::Arc;

// ------------------------------------------------------------------ rip

/// Rip (std sheetMetalRip, hidden in the UI): splits a wall between vertex
/// pairs. Each pair splits one planar wall into coplanar pieces, and live
/// rebuilds the same part (behaviours.md "fs (10-23 13:30)" / "(17:30)": no
/// gap, no joint in the table), so after std's checks nothing changes.
pub fn rip(ev: &mut EvalCtx) -> FResult<()> {
    let refs = ev.query("vertices").to_vec();
    if refs.len() < 2 || !refs.len().is_multiple_of(2) {
        return err_at(&["vertices"], "Select pairs of vertices.");
    }
    let mut pts = Vec::new();
    for r in &refs {
        let found = match r {
            Reference::Vertex(n) => ev.ctx.bodies.iter().enumerate().find_map(|(bi, b)| {
                let vi = *b.find_vertices(n).first()?;
                Some((bi, b.shape.sub_shape(ShapeType::Vertex, vi).ok()?.vertex_point().ok()?))
            }),
            _ => None,
        };
        let Some(p) = found else { return err_at(&["vertices"], "Sheet metal corner not found.") };
        pts.push(p);
    }
    for ch in pts.chunks(2) {
        let ((bi, p), (bj, q)) = (ch[0], ch[1]);
        if bi != bj {
            return err_at(&["vertices"], "Selected parts should be from the same active sheet metal model.");
        }
        let data = active_data(ev, bi)?;
        let body = &ev.ctx.bodies[bi];
        // One wall face holding both vertices.
        let mut walls = 0;
        for w in &data.walls {
            for fi in faces_named(body, &w.faces).into_iter().chain(faces_named(body, &w.back)) {
                let f = body.shape.sub_shape(ShapeType::Face, fi)?;
                let Some((c, n)) = plane_of(&f) else { continue };
                if (p - c).dot(n).abs() < 1e-7 && (q - c).dot(n).abs() < 1e-7 && f.project_point(p).is_ok_and(|x| x.distance < 1e-7) && f.project_point(q).is_ok_and(|x| x.distance < 1e-7) {
                    walls += 1;
                }
            }
        }
        if walls == 0 {
            return err("Cannot find a unique, planar wall to split.");
        }
        if (p - q).length() < 1e-9 {
            return err("Same underlying vertex selected in pair.");
        }
    }
    Ok(())
}

// ------------------------------------------------------------------ recognize

/// Sheet faces of a constant-thickness solid: (face, partner faces) for
/// planes with an anti-parallel face at distance `t` and cylinders with a
/// coaxial face of radius ± `t`.
fn sheet_partners(shape: &Shape, t: f64) -> Vec<Vec<usize>> {
    let faces = shape.faces();
    let surf: Vec<Option<k::Surface>> = faces.iter().map(|f| f.face_surface().ok()).collect();
    let planes: Vec<Option<(DVec3, DVec3)>> = faces.iter().map(plane_of).collect();
    let mut out = vec![Vec::new(); faces.len()];
    for i in 0..faces.len() {
        for j in 0..faces.len() {
            if i == j {
                continue;
            }
            let ok = match (planes[i], planes[j]) {
                (Some((ci, ni)), Some((cj, nj))) => ni.dot(nj) < -1.0 + 1e-7 && ((cj - ci).dot(ni) + t).abs() < t * 1e-4,
                _ => match (&surf[i], &surf[j]) {
                    (Some(k::Surface::Cylinder { origin: oi, axis: ai, radius: ri, .. }), Some(k::Surface::Cylinder { origin: oj, axis: aj, radius: rj, .. })) => {
                        let a = ai.normalize();
                        a.cross(aj.normalize()).length() < 1e-7 && ((*oj - *oi) - a * (*oj - *oi).dot(a)).length() < t * 1e-4 && ((ri - rj).abs() - t).abs() < t * 1e-4
                    }
                    _ => false,
                },
            };
            if ok {
                out[i].push(j);
            }
        }
    }
    out
}

/// Most common plane-pair distance (the sheet thickness) of a solid.
fn detect_thickness(shape: &Shape) -> Option<f64> {
    let planes: Vec<(DVec3, DVec3)> = shape.faces().iter().filter_map(plane_of).collect();
    let mut cands: Vec<(f64, usize)> = Vec::new();
    for (i, a) in planes.iter().enumerate() {
        for b in &planes[i + 1..] {
            if a.1.dot(b.1) < -1.0 + 1e-7 {
                let d = (b.0 - a.0).dot(a.1).abs();
                if d > 1e-9 {
                    match cands.iter_mut().find(|c| (c.0 - d).abs() < d * 1e-5) {
                        Some(c) => c.1 += 1,
                        None => cands.push((d, 1)),
                    }
                }
            }
        }
    }
    // Most frequent; ties -> thinnest.
    cands.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.total_cmp(&b.0)));
    cands.first().map(|c| c.0)
}

pub fn recognize(ev: &mut EvalCtx) -> FResult<()> {
    let idx = ev.resolve_bodies(ev.query("bodies"));
    if idx.is_empty() {
        return err("Select the parts to recognize as sheet metal");
    }
    for bi in idx.into_iter().rev() {
        let mut bi = bi;
        if ev.ctx.bodies[bi].kind != crate::display::BodyKind::Solid {
            return err("Only solid parts can be recognized as sheet metal");
        }
        if ev.bool("keepInputParts") {
            let b = ev.ctx.bodies[bi].clone();
            let names = b.faces.iter().map(|n| Arc::new(TopoName::face(ev.feature, Role::Copy { instance: 0, of: n.clone() }))).collect();
            let before = ev.ctx.bodies.len();
            ev.apply_operation(vec![NamedShape::new(b.shape.clone(), names)], "NEW", None, crate::display::BodyKind::Solid)?;
            if ev.ctx.bodies.len() <= before {
                return err("The part could not be copied");
            }
            bi = ev.ctx.bodies.len() - 1;
        }
        let body = ev.ctx.bodies[bi].clone();
        let Some(t) = detect_thickness(&body.shape) else { return err(format!("{} has no parallel faces of a constant thickness", body.name)) };
        if ev.bool("changeThickness") {
            ev.warnings.push("Changing the thickness while recognizing is not supported; the part's thickness is kept".into());
        }
        let partners = sheet_partners(&body.shape, t);
        let nf = partners.len();
        // Two-color the sheet faces: neighbours (sharing an edge) on the same side, partners opposite.
        let ef = body.shape.edge_faces()?;
        let mut adj: Vec<Vec<usize>> = vec![Vec::new(); nf];
        for fs in &ef {
            if fs.len() == 2 {
                let (a, b) = (fs[0] as usize, fs[1] as usize);
                if !partners[a].is_empty() && !partners[b].is_empty() {
                    adj[a].push(b);
                    adj[b].push(a);
                }
            }
        }
        let mut side: Vec<Option<bool>> = vec![None; nf];
        for start in 0..nf {
            if side[start].is_some() || partners[start].is_empty() {
                continue;
            }
            side[start] = Some(true);
            let mut q = VecDeque::from([start]);
            while let Some(f) = q.pop_front() {
                let s = side[f].unwrap_or(true);
                for &g in &adj[f] {
                    if side[g].is_none() {
                        side[g] = Some(s);
                        q.push_back(g);
                    }
                }
                for &g in &partners[f] {
                    if side[g].is_none() {
                        side[g] = Some(!s);
                        q.push_back(g);
                    }
                }
            }
        }
        let faces = body.shape.faces();
        let mut data = SheetMetalData::new(ev.feature, t, ev.length("radius").unwrap_or(0.0).max(1e-5), ev.real("kFactor").unwrap_or(0.45).clamp(0.0, 1.0));
        data.minimal_clearance = ev.length("minimalClearance").unwrap_or(2e-5).max(1e-6);
        // Walls: reference-side planar faces with their partners as back faces.
        let mut wall_of: BTreeMap<usize, usize> = BTreeMap::new();
        for (fi, f) in faces.iter().enumerate() {
            if side[fi] != Some(true) || plane_of(f).is_none() {
                continue;
            }
            let back: Vec<Name> = partners[fi].iter().filter(|&&j| plane_of(&faces[j]).is_some()).map(|&j| body.faces[j].clone()).collect();
            data.walls.push(Wall { feature: ev.feature, faces: vec![body.faces[fi].clone()], back });
            wall_of.insert(fi, data.walls.len() - 1);
        }
        if data.walls.is_empty() {
            return err(format!("{} has no sheet walls", body.name));
        }
        // Bends: reference-side cylinders between two walls.
        let mut radii = Vec::new();
        for (fi, f) in faces.iter().enumerate() {
            if side[fi] != Some(true) {
                continue;
            }
            let Ok(k::Surface::Cylinder { origin, axis, radius, .. }) = f.face_surface() else { continue };
            let Some(&pj) = partners[fi].first() else { continue };
            let Ok(k::Surface::Cylinder { radius: r2, .. }) = faces[pj].face_surface() else { continue };
            let walls: Vec<usize> = adj[fi].iter().filter_map(|g| wall_of.get(g).copied()).collect();
            if walls.len() != 2 || walls[0] == walls[1] {
                continue;
            }
            let (pa_w, pb_w) = (walls[0].min(walls[1]), walls[0].max(walls[1]));
            let r = radius.min(r2);
            let axis = axis.normalize();
            let wall_plane = |w: usize| faces_named(&body, &data.walls[w].faces).first().and_then(|&i| plane_of(&faces[i]));
            let (Some((pa, na)), Some((cb, nb))) = (wall_plane(pa_w), wall_plane(pb_w)) else { continue };
            let ext = extent_along(f, origin, axis);
            let m = measure_bend(origin, axis, r, t, pa, na, pa, nb, cb, ext);
            let (inner, outer) = if (radius - r).abs() < 1e-9 { (fi, pj) } else { (pj, fi) };
            radii.push(r);
            data.bends.push(Bend {
                feature: ev.feature,
                kind: BendKind::Bend,
                parent: pa_w,
                child: Some(pb_w),
                origin,
                axis: m.axis,
                angle: m.angle,
                radius: r,
                k_factor: data.k_factor,
                dir_a: m.dir_a,
                radial_a: m.radial_a,
                s0: m.s0,
                s1: m.s1,
                ref_inner: m.ref_inner,
                inner_faces: vec![body.faces[inner].clone()],
                outer_faces: vec![body.faces[outer].clone()],
                flat: false,
            });
        }
        if ev.length("radius").unwrap_or(0.0) <= 0.0
            && let Some(r) = radii.first()
        {
            data.radius = *r;
        }
        put_data(ev, bi, data);
    }
    Ok(())
}

// ------------------------------------------------------------------ unfold / refold

/// Walls reachable from `from` without crossing bend `skip`.
fn side_walls(data: &SheetMetalData, from: usize, skip: usize) -> Vec<usize> {
    let mut seen = vec![false; data.walls.len()];
    let mut q = VecDeque::from([from]);
    seen[from] = true;
    while let Some(w) = q.pop_front() {
        for (bi, b) in data.bends.iter().enumerate() {
            if bi == skip {
                continue;
            }
            let other = if b.parent == w { b.child } else if b.child == Some(w) { Some(b.parent) } else { None };
            if let Some(o) = other
                && !seen[o]
            {
                seen[o] = true;
                q.push_back(o);
            }
        }
    }
    (0..data.walls.len()).filter(|&i| seen[i]).collect()
}

/// Order walls by distance (in bends) from `hold`.
fn wall_depth(data: &SheetMetalData, hold: usize) -> Vec<usize> {
    let mut depth = vec![usize::MAX; data.walls.len()];
    depth[hold] = 0;
    let mut q = VecDeque::from([hold]);
    while let Some(w) = q.pop_front() {
        for b in &data.bends {
            let other = if b.parent == w { b.child } else if b.child == Some(w) { Some(b.parent) } else { None };
            if let Some(o) = other
                && depth[o] == usize::MAX
            {
                depth[o] = depth[w] + 1;
                q.push_back(o);
            }
        }
    }
    depth
}

/// Split body `bi` by removing `region`; returns (near pieces, far pieces)
/// where far pieces contain a face of `far_walls`.
fn split_by(ev: &EvalCtx, bi: usize, data: &SheetMetalData, region: Shape, far_walls: &[usize], key: &Name, slot: u16) -> FResult<(NamedShape, Vec<NamedShape>)> {
    let body = ev.ctx.bodies[bi].clone();
    let feature = ev.feature;
    let tool = tool_named(region, |i| side_name(feature, key, slot + i as u16));
    let (rest, names) = bool_named(ev, k::BooleanOp::Cut, &body_named(&body), &[tool])?;
    let pieces = crate::regen::eval::split_solids(&rest, &names);
    let far_names: Vec<Name> = far_walls.iter().flat_map(|&w| data.walls[w].faces.iter().chain(data.walls[w].back.iter()).cloned()).collect();
    let (mut near, mut far) = (Vec::new(), Vec::new());
    for p in pieces {
        if p.names.faces.iter().any(|n| far_names.iter().any(|f| n.descends_from(f))) {
            far.push(p);
        } else {
            near.push(p);
        }
    }
    if near.is_empty() || far.is_empty() {
        return err("The bend region could not be separated from its walls");
    }
    let near = if near.len() == 1 { near.remove(0) } else { fuse_pieces(ev, near)? };
    Ok((near, far))
}

fn fuse_pieces(ev: &EvalCtx, mut v: Vec<NamedShape>) -> FResult<NamedShape> {
    let first = v.remove(0);
    let (s, n) = bool_named(ev, k::BooleanOp::Fuse, &first, &v)?;
    Ok(NamedShape::new(s, n))
}

/// Faces of `shape` lying on the plane (point, normal).
fn faces_on_plane(shape: &Shape, p: DVec3, n: DVec3) -> Vec<Shape> {
    shape.faces().into_iter().filter(|f| plane_of(f).is_some_and(|(c, nn)| nn.cross(n).length() < 1e-7 && (c - p).dot(n).abs() < 1e-7)).collect()
}

fn transform_named(ev: &EvalCtx, s: &NamedShape, m: &DMat4) -> FResult<NamedShape> {
    let mv = k::transform(&s.shape, &k::Transform::Matrix(*m), true)?;
    let names = ev.name_result(&[&s.names], &mv.history, &mv.shape, &BTreeMap::new(), None);
    Ok(NamedShape::new(mv.shape, names))
}

/// Move walls' bends and marks of `walls` by `m` (bend `skip` excluded).
fn move_data(data: &mut SheetMetalData, walls: &[usize], skip: usize, m: &DMat4) {
    for (i, b) in data.bends.iter_mut().enumerate() {
        if i != skip && walls.contains(&b.parent) && b.child.is_none_or(|c| walls.contains(&c)) {
            b.transform(m);
        }
    }
    for mk in data.marks.iter_mut() {
        if walls.contains(&mk.wall) {
            mk.transform(m);
        }
    }
}

/// Body, model and hold wall of an Unfold / Refold selection.
fn hold_wall(ev: &EvalCtx, bi: usize, data: &SheetMetalData) -> usize {
    let Some(r) = ev.query("holdEntity").first() else { return 0 };
    let body = &ev.ctx.bodies[bi];
    let face = match r {
        Reference::Face(_) => ev.resolve_faces(std::slice::from_ref(r)).first().map(|x| x.1),
        Reference::Edge(_) => ev.resolve_edges(std::slice::from_ref(r)).first().and_then(|&(_, ei)| body.shape.edge_faces().ok()?.get(ei)?.iter().map(|f| *f as usize).find(|&f| data.wall_of_face(&body.faces[f]).is_some())),
        _ => None,
    };
    face.and_then(|f| data.wall_of_face(&body.faces[f])).map(|x| x.0).unwrap_or(0)
}

/// Bends of body `bi` owning the selected faces (or every bend with `all`).
fn selected_bends(ev: &EvalCtx, bi: usize, data: &SheetMetalData, refs: &[Reference], all: bool) -> Vec<usize> {
    if all {
        return (0..data.bends.len()).collect();
    }
    let body = &ev.ctx.bodies[bi];
    let mut out: Vec<usize> = Vec::new();
    for (b, f) in ev.resolve_faces(refs) {
        if b == bi
            && let Some(i) = data.bend_of_face(&body.faces[f])
            && !out.contains(&i)
        {
            out.push(i);
        }
    }
    for (b, e) in ev.resolve_edges(refs) {
        if b != bi {
            continue;
        }
        if let Some(i) = body.shape.edge_faces().ok().and_then(|ef| ef.get(e).cloned()).and_then(|fs| fs.iter().find_map(|f| data.bend_of_face(&body.faces[*f as usize])))
            && !out.contains(&i)
        {
            out.push(i);
        }
    }
    out
}

fn body_of_selection(ev: &EvalCtx, ids: &[&str]) -> Option<usize> {
    for id in ids {
        let refs = ev.query(id);
        if let Some(&b) = ev.resolve_bodies(refs).first() {
            return Some(b);
        }
        if let Some(&(b, _)) = ev.resolve_faces(refs).first() {
            return Some(b);
        }
        if let Some(&(b, _)) = ev.resolve_edges(refs).first() {
            return Some(b);
        }
    }
    None
}

pub fn unfold(ev: &mut EvalCtx) -> FResult<()> {
    let all = ev.bool("unfoldAll");
    let Some(bi) = body_of_selection(ev, &[if all { "partToUnfold" } else { "entitiesToUnfold" }, "holdEntity"]) else {
        return err(if all { "Select the sheet metal part to unfold" } else { "Select the bends to unfold" });
    };
    let mut data = (*active_data(ev, bi)?).clone();
    let t = data.thickness;
    let refs = ev.query("entitiesToUnfold").to_vec();
    let mut todo: Vec<usize> = selected_bends(ev, bi, &data, &refs, all).into_iter().filter(|&i| !data.bends[i].flat).collect();
    if todo.is_empty() {
        return err("No folded bends to unfold in the selection");
    }
    let hold = hold_wall(ev, bi, &data);
    let depth = wall_depth(&data, hold);
    // Nearest bends first (the hold wall never moves).
    todo.sort_by_key(|&i| {
        let b = &data.bends[i];
        depth[b.parent].min(b.child.map(|c| depth[c]).unwrap_or(usize::MAX))
    });
    let feature = ev.feature;
    for &i in &todo {
        let b = data.bends[i].clone();
        if b.kind != BendKind::Bend {
            return err("Only regular bends can be unfolded (not hems or rolled walls)");
        }
        let Some(child) = b.child else { return err("Only bends between two walls can be unfolded") };
        if b.angle >= std::f64::consts::PI - 1e-6 {
            return err("Bends of 180° or more cannot be unfolded yet");
        }
        // Near side: the side of the hold wall.
        let parent_side = side_walls(&data, b.parent, i);
        if parent_side.contains(&child) {
            return err("A bend inside a closed loop of walls cannot be unfolded (add a rip)");
        }
        let child_near = !parent_side.contains(&hold) && depth[child] < depth[b.parent];
        let far_walls = if child_near { parent_side } else { side_walls(&data, child, i) };
        let key = b.inner_faces.first().cloned().unwrap_or_else(|| Arc::new(TopoName::face(feature, Role::New(i as u32))));
        // Bend sector: between the two tangent planes through the axis, within R + T.
        let dir_b = DMat4::from_axis_angle(b.axis, b.angle).transform_vector3(-b.dir_a);
        let reach = b.radius + t;
        let s = (b.s0 - t, b.s1 + t);
        let half = |n_out: DVec3| -> FResult<Shape> {
            // Half space {(p - o)·n_out <= 0} bounded to the bend's neighbourhood.
            let ey = b.axis.cross(n_out).normalize();
            oriented_box(b.origin, n_out, ey, [-3.0 * reach, 0.0], [-3.0 * reach, 3.0 * reach], [s.0, s.1])
        };
        // Sector = {(p-o)·dir_a <= 0} ∩ {(p-o)·dir_b <= 0} ∩ cylinder(reach).
        let cyl = k::make_cylinder(&k::Plane::new(b.origin + b.axis * s.0, b.axis, b.dir_a), reach * (1.0 + 1e-6) + 1e-7, s.1 - s.0, 2.0 * std::f64::consts::PI)?;
        let h1 = half(b.dir_a)?;
        let h2 = half(dir_b)?;
        let sector = k::common(&k::common(&cyl, &h1)?.shape, &h2)?.shape;
        if sector.count(ShapeType::Solid) == 0 {
            return err("The bend region could not be found");
        }
        let (near, far) = split_by(ev, bi, &data, sector, &far_walls, &key, 700)?;
        // Flat transform of the far side and the developed strip.
        let fold_inv = b.fold_inverse(t);
        let (m, strip_dir, plane_n) = if child_near { (fold_inv.inverse(), -dir_b, dir_b) } else { (fold_inv, -b.dir_a, b.dir_a) };
        let ends = faces_on_plane(&near.shape, b.origin, plane_n);
        if ends.is_empty() {
            return err("The bend's end section could not be found");
        }
        let ba = b.allowance(t);
        let mut tools = Vec::new();
        for (j, e) in ends.iter().enumerate() {
            let res = k::extrude(e, strip_dir * ba)?;
            let names: Vec<Name> = (0..res.shape.count(ShapeType::Face) as u16).map(|f| side_name(feature, &key, 720 + j as u16 * 16 + f)).collect();
            tools.push(NamedShape::new(res.shape, names));
        }
        for p in &far {
            tools.push(transform_named(ev, p, &m)?);
        }
        let (shape, names) = bool_named(ev, k::BooleanOp::Fuse, &near, &tools)?;
        if solid_count(&shape) != 1 {
            return err("The unfolded part is not connected");
        }
        ev.replace_body(bi, shape, names);
        move_data(&mut data, &far_walls, i, &m);
        if child_near {
            data.bends[i].transform(&m);
        }
        let nb = ev.ctx.bodies[bi].clone();
        let strip: Vec<Name> = (0..(ends.len() as u16 * 16)).map(|f| side_name(feature, &key, 720 + f)).collect();
        let strip_now = current_names(&nb, &strip);
        let bm = &mut data.bends[i];
        bm.flat = true;
        bm.inner_faces = strip_now.clone();
        bm.outer_faces = Vec::new();
    }
    put_data(ev, bi, data);
    Ok(())
}

pub fn refold(ev: &mut EvalCtx) -> FResult<()> {
    let refs = ev.query("toRefold").to_vec();
    if refs.is_empty() {
        return err("Select the unfolded parts or bends to refold");
    }
    let Some(bi) = body_of_selection(ev, &["toRefold"]) else { return err("The selection no longer exists") };
    let mut data = (*active_data(ev, bi)?).clone();
    let t = data.thickness;
    let whole = refs.iter().any(|r| matches!(r, Reference::Body(_)));
    let mut todo: Vec<usize> = if whole {
        (0..data.bends.len()).filter(|&i| data.bends[i].flat).collect()
    } else {
        let body = &ev.ctx.bodies[bi];
        let mut v = Vec::new();
        for (b, f) in ev.resolve_faces(&refs) {
            if b == bi
                && let Some(i) = data.bend_of_face(&body.faces[f])
                && data.bends[i].flat
                && !v.contains(&i)
            {
                v.push(i);
            }
        }
        for (b, e) in ev.resolve_edges(&refs) {
            if b == bi
                && let Some(fs) = body.shape.edge_faces().ok().and_then(|ef| ef.get(e).cloned())
            {
                for f in fs {
                    if let Some(i) = data.bend_of_face(&body.faces[f as usize])
                        && data.bends[i].flat
                        && !v.contains(&i)
                    {
                        v.push(i);
                    }
                }
            }
        }
        v
    };
    if todo.is_empty() {
        return err("No unfolded bends in the selection");
    }
    // Refold far bends first (from the outside in, relative to wall 0).
    let depth = wall_depth(&data, 0);
    todo.sort_by_key(|&i| std::cmp::Reverse(data.bends[i].child.map(|c| depth[c]).unwrap_or(0)));
    let feature = ev.feature;
    for &i in &todo {
        let b = data.bends[i].clone();
        let Some(child) = b.child else { continue };
        let far_walls = side_walls(&data, child, i);
        if far_walls.contains(&b.parent) {
            return err("A bend inside a closed loop of walls cannot be refolded");
        }
        let key = Arc::new(TopoName::face(feature, Role::New(900 + i as u32)));
        let ba = b.allowance(t);
        // Strip region in the parent's flat extension (both sheet faces + margin).
        let p0 = b.tangent_point(t, 0.0);
        let strip = oriented_box(p0, -b.dir_a, b.axis, [0.0, ba], [b.s0 - t, b.s1 + t], [-2.0 * t, 2.0 * t])?;
        let (near, far) = split_by(ev, bi, &data, strip, &far_walls, &key, 740)?;
        let m = b.fold_inverse(t).inverse();
        let ends = faces_on_plane(&near.shape, p0, b.dir_a);
        if ends.is_empty() {
            return err("The unfolded strip's start section could not be found");
        }
        let mut tools = Vec::new();
        for (j, e) in ends.iter().enumerate() {
            let res = k::revolve(e, b.origin, b.axis, b.angle)?;
            let mut roles = BTreeMap::new();
            for (fj, f) in res.shape.faces().iter().enumerate() {
                let kk = match f.face_surface() {
                    Ok(k::Surface::Cylinder { radius, .. }) if (radius - b.radius).abs() < 1e-7 => 0,
                    Ok(k::Surface::Cylinder { .. }) => 1,
                    _ => 2 + j as u16,
                };
                roles.insert(fj as u32, Role::Side(piece_source(&key, feature, 760 + kk)));
            }
            let names = ev.name_result(&[&Default::default()], &res.history, &res.shape, &roles, None);
            tools.push(NamedShape::new(res.shape, names));
        }
        for p in &far {
            tools.push(transform_named(ev, p, &m)?);
        }
        let (shape, names) = bool_named(ev, k::BooleanOp::Fuse, &near, &tools)?;
        if solid_count(&shape) != 1 {
            return err("The refolded part is not connected");
        }
        ev.replace_body(bi, shape, names);
        move_data(&mut data, &far_walls, i, &m);
        let nb = ev.ctx.bodies[bi].clone();
        let bm = &mut data.bends[i];
        bm.flat = false;
        bm.inner_faces = current_names(&nb, &[side_name(feature, &key, 760)]);
        bm.outer_faces = current_names(&nb, &[side_name(feature, &key, 761)]);
    }
    put_data(ev, bi, data);
    Ok(())
}
