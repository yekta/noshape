//! Sweep, Loft, Thicken, Enclose, Rib and the FeatureScript primitives
//! (Cube, Sphere).
//!
//! Thin sweeps sweep the offset wall regions of `thin`; thin lofts loft the
//! wire profiles into a surface and thicken it. Unsupported options return a
//! clear error (see each evaluator): sweep twist and "lock profile faces",
//! loft guides and path, rib draft.

use super::helpers_a::*;
use crate::display::BodyKind;
use crate::naming::Name;
use crate::params::ParamValues;
use crate::reference::{BodyId, Reference, Role, Source};
use crate::regen::eval::{EvalCtx, FResult, NamedShape, Profile, err, kernel_plane, sweep_names};
use glam::DVec3;
use noshape_kernel as k;
use noshape_kernel::{Shape, ShapeType};
use std::collections::BTreeMap;

// ------------------------------------------------------------------ sweep

/// Onshape's SWEEP_FAILED.
const SWEEP_FAILED: &str = "Could not create valid swept body, check input.";

/// The part of the sweep path a profile in its middle sweeps along (live
/// L12): the piece from the profile toward the path's start, or toward its
/// end with the path's Opposite direction. Unchanged when the profile's
/// plane doesn't cut the path.
fn half_path(path: &Shape, origin: DVec3, normal: DVec3, opposite: bool) -> Shape {
    let x = if normal.cross(DVec3::X).length() > 0.1 { DVec3::X } else { DVec3::Y };
    let plane = k::Plane::new(origin, normal, x);
    let Some(start) = path.edges().first().and_then(|e| e.endpoints().ok()).map(|e| e.0) else { return path.clone() };
    let Ok(cut) = k::split_by_plane(path, &plane) else { return path.clone() };
    let edges = cut.shape.edges();
    if edges.len() <= path.edges().len() {
        return path.clone();
    }
    // The edges on the start's side of the profile's plane (or the other).
    let side = |p: DVec3| (p - origin).dot(normal) >= 0.0;
    let start_side = side(start);
    let kept: Vec<Shape> = edges
        .into_iter()
        .filter(|e| e.edge_info().ok().and_then(|i| e.point_at((i.first_param + i.last_param) * 0.5).ok()).is_some_and(|m| (side(m) == start_side) != opposite))
        .collect();
    if kept.is_empty() {
        return path.clone();
    }
    k::make_wire(&kept).unwrap_or_else(|_| path.clone())
}

/// Onshape's LOFT_FAILED.
const LOFT_FAILED: &str = "Could not create loft with given information. Check profile order, guide/profile intersections or end conditions.";

pub fn sweep(ev: &mut EvalCtx) -> FResult<()> {
    let body_type = ev.enum_("bodyType").to_string();
    // sweep.fs: a thin sweep checks its wall before the path.
    if body_type == "THIN" && ev.query("wallShape").is_empty() {
        return crate::regen::eval::err_at(&["wallShape"], "Select faces or sketch regions to sweep.");
    }
    let path_edges = resolve_curves(ev, ev.query("path"))?;
    if path_edges.is_empty() {
        return crate::regen::eval::err_at(&["path"], "Select entities for sweep path.");
    }
    let wires = wires_from(&path_edges)?;
    if wires.len() != 1 {
        // Live L13: no field is flagged.
        return err("Sweep path curves are not all connected.");
    }
    let path = wires[0].clone();
    // Twist: total rotation about the path (Turns, Angle, or one turn per Pitch length).
    let twist = if ev.bool("hasTwist") && matches!(ev.enum_("profileControl"), "NONE" | "KEEP_ORIENTATION") {
        let total = match ev.enum_("twistType") {
            "ANGLE" => ev.angle("angle")?,
            "PITCH" => {
                let len: f64 = path.edges().iter().filter_map(|e| e.length().ok()).sum();
                let turns = len / ev.length("pitch")?;
                if !(0.0..=1000.0).contains(&turns) {
                    return crate::regen::eval::err_at(&["pitch"], format!("Computed number of revolutions outside limit (between 0 and 1000). Current value is {turns}."));
                }
                std::f64::consts::TAU * turns
            }
            _ => std::f64::consts::TAU * ev.real("turns")?,
        };
        if ev.bool("ccw") { total } else { -total }
    } else {
        0.0
    };
    let mut opts = k::SweepOptions { scale_end: if ev.bool("hasScale") { ev.real("scaleFactor")? } else { 1.0 }, twist, ..Default::default() };
    let lock_dir = match ev.enum_("profileControl") {
        "LOCK_DIRECTION" => {
            let no_dir = || crate::regen::eval::err_at(&["lockDirectionQuery"], "Select direction to lock.");
            let Some(r) = ev.query("lockDirectionQuery").first() else { return no_dir() };
            let Some(d) = ev.resolve_direction(r) else { return no_dir() };
            Some(d)
        }
        _ => None,
    };
    let keep = ev.is("profileControl", "KEEP_ORIENTATION");
    // Lock profile faces: the profile keeps its orientation relative to the
    // faces' normal along the path (which lies on them).
    let lock_faces = if ev.is("profileControl", "LOCK_FACES") {
        let faces: Vec<Shape> = ev.resolve_faces(ev.query("lockFaces")).into_iter().filter_map(|(bi, fi)| ev.ctx.bodies[bi].shape.sub_shape(ShapeType::Face, fi).ok()).collect();
        // Live L13: no faces to lock sweeps as with no profile control.
        (!faces.is_empty()).then(|| Shape::compound(&faces))
    } else {
        None
    };
    let (extend, path_opposite) = (ev.bool("extendToFullPath"), ev.bool("pathOppositeDirection"));
    let sweep = |profile: &Shape, frame: Option<&crate::display::PlaneFrame>, opts: &k::SweepOptions| {
        let path = match frame {
            Some(f) if !extend => half_path(&path, f.origin, f.normal, path_opposite),
            _ => path.clone(),
        };
        match &lock_faces {
            Some(f) => k::sweep_locked_to_faces(std::slice::from_ref(profile), &path, f, opts),
            None => k::sweep(profile, &path, opts),
        }
    };
    let mut tools = Vec::new();
    if body_type == "SURFACE" {
        let edges = resolve_curves(ev, ev.query("surfaceProfiles"))?;
        if edges.is_empty() {
            return err("Select sketch curves to sweep.");
        }
        opts.solid = false;
        for (i, w) in wires_from(&edges)?.into_iter().enumerate() {
            let p = wire_profile(w, &edges, Source::Index(i as u32));
            if keep {
                opts.mode = fixed_mode_for(&p.shape)?;
            } else if let Some(d) = lock_dir {
                opts.mode = k::SweepMode::Binormal(d);
            }
            let res = sweep(&p.shape, Some(&p.frame), &opts).map_err(|_| fe(SWEEP_FAILED))?;
            let names = sweep_names(ev, &p, &res, false, false);
            tools.push(NamedShape::new(res.shape, names));
        }
        let op = ev.enum_("surfaceOperationType").to_string();
        return ev.apply_operation(tools, &op, None, BodyKind::Surface);
    }
    // Thin: the wall curves are offset into planar regions (see `thin`),
    // which are swept like solid profiles.
    let thin = body_type == "THIN";
    let refs = ev.query(if thin { "wallShape" } else { "profiles" });
    if refs.is_empty() {
        return crate::regen::eval::err_at(&[if thin { "wallShape" } else { "profiles" }], "Select faces or sketch regions to sweep.");
    }
    let profiles = if thin { super::thin::thin_profiles(ev, refs)? } else { ev.profiles(refs)? };
    if profiles.is_empty() {
        return err("No valid profiles to sweep");
    }
    for p in &profiles {
        if keep {
            opts.mode = k::SweepMode::Fixed { z: p.frame.normal, x: p.frame.x };
        } else if let Some(d) = lock_dir {
            opts.mode = k::SweepMode::Binormal(d);
        }
        let res = sweep(&p.shape, Some(&p.frame), &opts).map_err(|_| fe(SWEEP_FAILED))?;
        if res.shape.count(ShapeType::Solid) == 0 {
            return err(SWEEP_FAILED);
        }
        // OCCT sweeps a profile round a bend tighter than the profile into a
        // self-intersecting solid; Onshape refuses it.
        if !res.shape.is_valid() || res.shape.self_intersects() {
            return err("Could not create valid swept body, check input.");
        }
        let names = sweep_names(ev, p, &res, true, true);
        tools.push(NamedShape::new(res.shape, names));
    }
    let op = ev.enum_("operationType").to_string();
    let scope = super::extrude::scope(ev);
    ev.apply_operation(tools, &op, scope, BodyKind::Solid)
}

/// Fixed trihedron for a wire profile: its plane if planar, else world Z.
fn fixed_mode_for(wire: &Shape) -> FResult<k::SweepMode> {
    match k::make_face_from_wire(wire, true).ok().and_then(|f| f.face_plane().ok().flatten()) {
        Some(pl) => Ok(k::SweepMode::Fixed { z: pl.unit_normal(), x: pl.unit_x() }),
        None => Ok(k::SweepMode::Fixed { z: DVec3::Z, x: DVec3::X }),
    }
}

// ------------------------------------------------------------------ loft

/// One loft section: a shape and (for faces/wires) its profile naming.
struct Section {
    shape: Shape,
    profile: Option<Profile>,
}

fn loft_sections(ev: &EvalCtx, items: &[ParamValues], query_id: &str, wire: bool) -> FResult<Vec<Section>> {
    let mut out = Vec::new();
    for (i, item) in items.iter().enumerate() {
        let refs = item.query(query_id);
        if refs.is_empty() {
            continue;
        }
        // Point profile.
        if (refs.len() == 1 && matches!(refs[0], Reference::Vertex(_) | Reference::Origin | Reference::MateConnector { .. } | Reference::Implicit { .. }))
            || matches!(&refs[0], Reference::SketchEntity { sketch, entity } if ev.ctx.sketch(*sketch).is_some_and(|s| s.sketch.point(noshape_sketch::EntityId(*entity)).is_some()))
        {
            let p = ev.resolve_vertex_point(&refs[0]).ok_or_else(|| fe("Could not create valid profiles from selections."))?;
            out.push(Section { shape: k::make_vertex(p)?, profile: None });
            continue;
        }
        if wire {
            let edges = resolve_curves(ev, refs)?;
            let mut wires = wires_from(&edges)?;
            // Faces in a wire profile: use their outer boundary.
            for (bi, fi) in ev.resolve_faces(refs) {
                let f = ev.face_shape(bi, fi)?;
                if let Some(w) = f.wires().first() {
                    wires.push(w.clone());
                }
            }
            if wires.len() != 1 {
                return err("Could not create valid profiles from selections.");
            }
            let w = wires.remove(0);
            out.push(Section { shape: w.clone(), profile: Some(wire_profile(w, &edges, Source::Index(i as u32))) });
            continue;
        }
        let mut profiles = ev.profiles(refs)?;
        // Sheet bodies with a single face.
        for r in refs {
            if let Reference::Body(id) = r
                && let Some(b) = ev.ctx.body(id)
                && b.kind == BodyKind::Surface
            {
                let faces = b.shape.faces();
                if faces.len() != 1 {
                    return err("Select a single sketch region or face per profile.");
                }
                let f = faces[0].clone();
                let edge_sources = f.edges().iter().map(|e| b.shape.index_of(e, ShapeType::Edge).map(|k| Source::Topo(b.edges[k].clone())).unwrap_or(Source::Index(0))).collect();
                let frame = match f.face_plane()? {
                    Some(pl) => crate::display::PlaneFrame { origin: pl.origin, x: pl.unit_x(), y: pl.unit_y(), normal: pl.unit_normal() },
                    None => crate::display::PlaneFrame::TOP,
                };
                profiles.push(Profile { shape: f, frame, edge_sources, key: Source::Topo(b.faces[0].clone()), from_face: true });
            }
        }
        if profiles.len() != 1 {
            return err("Select a single sketch region or face per profile.");
        }
        let p = profiles.remove(0);
        if p.shape.wires().len() > 1 {
            return err("Cannot use faces or regions with inner loops as profiles.");
        }
        out.push(Section { shape: p.shape.clone(), profile: Some(p) });
    }
    Ok(out)
}

fn loft_names(ev: &EvalCtx, sections: &[Section], res: &k::OpResult, caps: bool) -> Vec<Name> {
    let mut roles = BTreeMap::new();
    if caps {
        if let Some(p) = sections.first().and_then(|s| s.profile.as_ref()) {
            for f in res.history.first_faces() {
                roles.insert(f, Role::CapStart(Some(p.key.clone())));
            }
        }
        if let Some(p) = sections.last().and_then(|s| s.profile.as_ref()) {
            for f in res.history.last_faces() {
                roles.insert(f, Role::CapEnd(Some(p.key.clone())));
            }
        }
    }
    let mut recs: Vec<&k::HistoryRecord> = res.history.records.iter().filter(|r| r.input_kind == ShapeType::Edge && r.relation == k::Relation::Generated && r.output_kind == ShapeType::Face).collect();
    recs.sort_by_key(|r| (r.arg, r.input_index));
    for r in recs {
        if let Some(src) = sections.get(r.arg as usize).and_then(|s| s.profile.as_ref()).and_then(|p| p.edge_sources.get(r.input_index as usize)) {
            roles.entry(r.output_index).or_insert_with(|| Role::Side(src.clone()));
        }
    }
    let empty = crate::naming::ShapeNames::default();
    ev.name_result(&[&empty], &res.history, &res.shape, &roles, None)
}

pub fn loft(ev: &mut EvalCtx) -> FResult<()> {
    let body_type = ev.enum_("bodyType").to_string();
    if ev.bool("addGuides") && ev.params.array("guidesArray").iter().any(|g| !g.query("guideEntities").is_empty()) {
        return err("Loft guides are not supported yet");
    }
    if ev.bool("addSections") && !ev.query("spine").is_empty() {
        return err("Loft path is not supported yet");
    }
    let solid = body_type == "SOLID";
    let sections = if solid {
        loft_sections(ev, ev.params.array("sheetProfilesArray"), "sheetProfileEntities", false)?
    } else {
        loft_sections(ev, ev.params.array("wireProfilesArray"), "wireProfileEntities", true)?
    };
    // Live L15: no profile marks the field; one profile has its own text.
    if sections.is_empty() {
        return crate::regen::eval::err_at(&["sheetProfilesArray", "wireProfilesArray"], "Select profiles in order from start to finish.");
    }
    if sections.len() < 2 {
        return err("Select at least two profiles, a closed guide curve or a closed path.");
    }
    if sections[1..sections.len() - 1].iter().any(|s| s.profile.is_none()) {
        return err("Point profiles can only exist as first or last profiles.");
    }
    if !ev.is("startCondition", "DEFAULT") || !ev.is("endCondition", "DEFAULT") {
        ev.warnings.push("Loft start/end conditions are not supported yet and were ignored".into());
    }
    let shapes: Vec<Shape> = sections.iter().map(|s| s.shape.clone()).collect();
    let mut opts = k::LoftOptions { solid, ruled: false, closed: ev.bool("makePeriodic"), start_points: None };
    // Match connections: the first connection's vertices set each profile's start.
    if ev.bool("matchConnections")
        && let Some(c) = ev.params.array("connections").first()
    {
        let pts: Vec<DVec3> = c.query("connectionEntities").iter().filter_map(|r| ev.resolve_vertex_point(r).or_else(|| edge_ref_mid(ev, r))).collect();
        if !pts.is_empty() {
            let mut starts = vec![DVec3::NAN; shapes.len()];
            for (si, s) in shapes.iter().enumerate() {
                if s.shape_type() == ShapeType::Vertex {
                    continue;
                }
                let best = pts.iter().filter_map(|p| k::make_vertex(*p).ok().and_then(|v| k::distance(&v, s).ok()).map(|d| (*p, d.distance))).min_by(|a, b| a.1.total_cmp(&b.1));
                if let Some((p, d)) = best
                    && d < 1e-6
                {
                    starts[si] = p;
                }
            }
            opts.start_points = Some(starts);
        }
    }
    let res = k::loft(&shapes, &opts).map_err(|_| fe(LOFT_FAILED))?;
    if solid && res.shape.count(ShapeType::Solid) == 0 {
        return err(LOFT_FAILED);
    }
    let names = loft_names(ev, &sections, &res, solid);
    if body_type == "THIN" {
        // Thin loft: the lofted surface (through the wire profiles) is
        // thickened by thickness 1 / 2 (Flip wall swaps the sides).
        let (t1, t2) = super::thin::thicknesses(ev)?;
        let inp = ThickenInput {
            shape: res.shape.clone(),
            face_sources: names.iter().map(|n| Source::Topo(n.clone())).collect(),
            edge_sources: crate::regen::eval::names_for(&res.shape, names.clone()).edges.iter().map(|n| Source::Topo(n.clone())).collect(),
            group: None,
        };
        let th = k::thicken(&res.shape, t1, t2).map_err(|_| fe("Failed to thicken lofted body, check thickness."))?;
        if th.shape.count(ShapeType::Solid) == 0 {
            return err("Thin loft did not produce a solid");
        }
        let names = thicken_names(ev, &inp, &th.shape, t1.max(t2));
        let tools = vec![NamedShape::new(th.shape, names)];
        let op = ev.enum_("operationType").to_string();
        let scope = super::extrude::scope(ev);
        return ev.apply_operation(tools, &op, scope, BodyKind::Solid);
    }
    let tools = vec![NamedShape::new(res.shape, names)];
    if solid {
        let op = ev.enum_("operationType").to_string();
        let scope = super::extrude::scope(ev);
        ev.apply_operation(tools, &op, scope, BodyKind::Solid)
    } else {
        let op = ev.enum_("surfaceOperationType").to_string();
        ev.apply_operation(tools, &op, None, BodyKind::Surface)
    }
}

fn edge_ref_mid(ev: &EvalCtx, r: &Reference) -> Option<DVec3> {
    let (bi, ei) = *ev.resolve_edges(std::slice::from_ref(r)).first()?;
    edge_mid(&ev.edge_shape(bi, ei).ok()?)
}

// ------------------------------------------------------------------ thicken

/// A face set to thicken with the persistent sources of its faces/edges.
struct ThickenInput {
    shape: Shape,
    face_sources: Vec<Source>,
    edge_sources: Vec<Source>,
    /// Faces of one body picked together: their slabs become one part.
    group: Option<usize>,
}

pub fn thicken(ev: &mut EvalCtx) -> FResult<()> {
    let refs = ev.query("entities");
    if refs.is_empty() {
        return crate::regen::eval::err_at(&["entities"], "Select surfaces or faces to thicken.");
    }
    let (mut t1, mut t2) = if ev.bool("midplane") {
        let t = ev.length("thickness")? * 0.5;
        (t, t)
    } else {
        (ev.length("thickness1")?, ev.length("thickness2")?)
    };
    if ev.bool("oppositeDirection") && !ev.bool("midplane") {
        std::mem::swap(&mut t1, &mut t2);
    }
    // Live L19: both thicknesses 0 fail with this text and no field.
    if t1 + t2 <= 0.0 {
        return err("Invalid input selections.");
    }
    let mut inputs: Vec<ThickenInput> = Vec::new();
    let mut consumed: Vec<BodyId> = Vec::new();
    // Faces picked per body: one body's faces are thickened together (live
    // L17: two adjacent faces give one part), and a surface is used up only
    // when all of its faces are (L16).
    let mut picked: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for r in refs {
        match r {
            Reference::SketchRegion { .. } => {
                for p in ev.profiles(std::slice::from_ref(r))? {
                    inputs.push(ThickenInput { face_sources: vec![p.key.clone()], edge_sources: p.edge_sources.clone(), shape: p.shape, group: None });
                }
            }
            Reference::Face(_) => {
                for (bi, fi) in ev.resolve_faces(std::slice::from_ref(r)) {
                    let v = picked.entry(bi).or_default();
                    if !v.contains(&fi) {
                        v.push(fi);
                    }
                }
            }
            Reference::Body(id) => {
                if let Some(b) = ev.ctx.body(id)
                    && b.kind == BodyKind::Surface
                {
                    consumed.push(b.id.clone());
                    inputs.push(ThickenInput {
                        shape: b.shape.clone(),
                        face_sources: b.faces.iter().map(|x| Source::Topo(x.clone())).collect(),
                        edge_sources: b.edges.iter().map(|x| Source::Topo(x.clone())).collect(),
                        group: None,
                    });
                }
            }
            _ => {}
        }
    }
    for (bi, fis) in picked {
        let b = &ev.ctx.bodies[bi];
        let faces: Vec<Shape> = fis.iter().map(|&fi| b.shape.sub_shape(ShapeType::Face, fi)).collect::<k::Result<_>>()?;
        if b.kind == BodyKind::Surface && fis.len() >= b.shape.faces().len() {
            consumed.push(b.id.clone());
        }
        // Several faces of one body make one part (live L17). Each face is
        // thickened and the slabs are fused; Onshape's square corner fill
        // between them is still missing (kernel).
        let group = (fis.len() > 1).then_some(bi);
        for f in faces {
            let n = sub_names(b, &f);
            inputs.push(ThickenInput {
                face_sources: n.faces.iter().map(|x| Source::Topo(x.clone())).collect(),
                edge_sources: n.edges.iter().map(|x| Source::Topo(x.clone())).collect(),
                shape: f,
                group,
            });
        }
    }
    if inputs.is_empty() {
        return err("Select surfaces or faces to thicken.");
    }
    let mut tools: Vec<NamedShape> = Vec::new();
    let mut grouped: BTreeMap<usize, usize> = BTreeMap::new();
    for inp in &inputs {
        let res = k::thicken(&inp.shape, t1, t2).map_err(kerr("Thicken failed"))?;
        if res.shape.count(ShapeType::Solid) == 0 {
            return err("Thicken did not produce a solid");
        }
        let shape = res.shape;
        let names = thicken_names(ev, inp, &shape, t1.max(t2));
        let slab = NamedShape::new(shape, names);
        match inp.group.and_then(|g| grouped.get(&g).copied()) {
            Some(ti) => {
                let a = tools[ti].clone();
                let res = k::boolean(k::BooleanOp::Fuse, std::slice::from_ref(&a.shape), std::slice::from_ref(&slab.shape), &k::BooleanOptions::default())?;
                let names = ev.name_result(&[&a.names, &slab.names], &res.history, &res.shape, &BTreeMap::new(), None);
                tools[ti] = NamedShape::new(res.shape, names);
            }
            None => {
                if let Some(g) = inp.group {
                    grouped.insert(g, tools.len());
                }
                tools.push(slab);
            }
        }
    }
    let op = ev.enum_("operationType").to_string();
    let scope = scope_ids(ev);
    if !ev.bool("keepTools") {
        consumed.sort();
        consumed.dedup();
        remove_bodies(ev, &consumed);
    }
    let scope = ids_to_indices(ev, &scope);
    ev.apply_operation(tools, &op, scope, BodyKind::Solid)
}

/// Geometric naming for thicken: faces on the input are start caps, faces
/// parallel to it are end caps, the rest are sides of the nearest input edge.
fn thicken_names(ev: &EvalCtx, inp: &ThickenInput, out: &Shape, tmax: f64) -> Vec<Name> {
    let in_faces = inp.shape.faces();
    let in_edges = inp.shape.edges();
    let tol = 1e-7_f64.max(tmax * 1e-6);
    let mut roles = BTreeMap::new();
    for (j, f) in out.faces().iter().enumerate() {
        let Some(p) = point_on_face(f) else { continue };
        let Ok(v) = k::make_vertex(p) else { continue };
        let nearest = |shapes: &[Shape]| shapes.iter().enumerate().filter_map(|(i, s)| k::distance(&v, s).ok().map(|d| (i, d.distance))).min_by(|a, b| a.1.total_cmp(&b.1));
        let Some((fi, d)) = nearest(&in_faces) else { continue };
        let src_f = inp.face_sources.get(fi).cloned().unwrap_or(Source::Index(fi as u32));
        if d < tol {
            roles.insert(j as u32, Role::CapStart(Some(src_f)));
            continue;
        }
        let n_out = f.normal_at_point(p).unwrap_or(DVec3::Z);
        let n_in = face_normal_near(&in_faces[fi], p).map(|x| x.1).unwrap_or(DVec3::Z);
        if n_out.dot(n_in).abs() > 0.5 {
            roles.insert(j as u32, Role::CapEnd(Some(src_f)));
        } else if let Some((ei, _)) = nearest(&in_edges) {
            roles.insert(j as u32, Role::Side(inp.edge_sources.get(ei).cloned().unwrap_or(Source::Index(ei as u32))));
        }
    }
    let empty = crate::naming::ShapeNames::default();
    let names = ev.name_result(&[&empty], &k::History::default(), out, &roles, None);
    dedup_names(ev, names)
}

// ------------------------------------------------------------------ enclose

pub fn enclose(ev: &mut EvalCtx) -> FResult<()> {
    let refs = ev.query("entities");
    if refs.is_empty() {
        return err("Select faces, parts, or planes to form a solid.");
    }
    let mut shapes: Vec<Shape> = Vec::new();
    let mut names: Vec<crate::naming::ShapeNames> = Vec::new();
    let mut owners: Vec<BodyId> = Vec::new();
    for r in refs {
        match r {
            Reference::Body(id) => {
                if let Some(b) = ev.ctx.body(id) {
                    shapes.push(b.shape.clone());
                    names.push(b.shape_names());
                    owners.push(b.id.clone());
                }
            }
            Reference::Face(_) => {
                for (bi, fi) in ev.resolve_faces(std::slice::from_ref(r)) {
                    let b = &ev.ctx.bodies[bi];
                    let f = b.shape.sub_shape(ShapeType::Face, fi)?;
                    names.push(sub_names(b, &f));
                    shapes.push(f);
                    owners.push(b.id.clone());
                }
            }
            _ => {
                if let Some(frame) = ev.resolve_plane(r) {
                    shapes.push(big_plane_face(ev, &kernel_plane(&frame))?);
                    names.push(Default::default());
                }
            }
        }
    }
    if shapes.is_empty() {
        return err("Select faces, parts, or planes to form a solid.");
    }
    let res = k::enclose(&shapes).map_err(|_| fe("Failed to create a part from enclosed region."))?;
    if res.shape.count(ShapeType::Solid) == 0 {
        return err("Selections do not enclose a region.");
    }
    let arg_refs: Vec<&crate::naming::ShapeNames> = names.iter().collect();
    let faces = ev.name_result(&arg_refs, &res.history, &res.shape, &BTreeMap::new(), None);
    let faces = rebrand(ev, faces);
    let tools = vec![NamedShape::new(res.shape, faces)];
    let op = ev.enum_("operationType").to_string();
    let scope = scope_ids(ev);
    if !ev.bool("keepTools") {
        owners.sort();
        owners.dedup();
        remove_bodies(ev, &owners);
    }
    let scope = ids_to_indices(ev, &scope);
    ev.apply_operation(tools, &op, scope, BodyKind::Solid)
}

// ------------------------------------------------------------------ rib

pub fn rib(ev: &mut EvalCtx) -> FResult<()> {
    let refs = ev.query("profiles");
    if refs.is_empty() {
        return crate::regen::eval::err_at(&["profiles"], "Select sketch profiles to create the ribs.");
    }
    if ev.bool("hasDraft") {
        return err("Rib draft is not supported yet");
    }
    let t = ev.length("thickness")?;
    if t <= 0.0 {
        return err("Rib thickness must be positive");
    }
    let frame = refs
        .iter()
        .find_map(|r| match r {
            Reference::SketchEntity { sketch, .. } | Reference::Sketch(sketch) => ev.ctx.sketch(*sketch).map(|s| s.frame),
            _ => None,
        })
        .ok_or_else(|| fe("Rib profiles must be sketch curves"))?;
    let plane = kernel_plane(&frame);
    let edges = resolve_curves(ev, refs)?;
    let wires = wires_from(&edges)?;
    if wires.is_empty() {
        return err("The rib profiles no longer exist");
    }
    let normal = ev.is("ribExtrusionDirection", "NORMAL_TO_SKETCH_PLANE");
    // Live L23/L24: parallel to the sketch plane a closed profile or a chain
    // of several edges fails on the profiles.
    if !normal && wires.iter().any(|w| w.is_closed().unwrap_or(false) || w.edges().len() > 1) {
        return crate::regen::eval::err_at(&["profiles"], RIB_FAILED);
    }
    let dir = if ev.bool("oppositeDirection") { -frame.normal } else { frame.normal };
    let mut parts = ev.resolve_bodies(ev.query("parts"));
    parts.retain(|i| ev.ctx.bodies[*i].kind == BodyKind::Solid);
    // Live L22: no Parts is an error, no field marked (the dialog fills
    // Parts when it opens). Ribs saved before that used every part.
    if parts.is_empty() && ev.params.bool("legacyAllParts") {
        parts = (0..ev.ctx.bodies.len()).filter(|i| ev.ctx.bodies[*i].kind == BodyKind::Solid).collect();
    }
    if parts.is_empty() {
        return err("Select parts where the rib will be fitted into.");
    }
    let merge = ev.bool("mergeRibs");
    let mut made = 0;
    let mut last_err = None;
    let mut new_ribs: Vec<NamedShape> = Vec::new();
    for bi in parts.into_iter().rev() {
        let body = ev.ctx.bodies[bi].clone();
        let mut shape = body.shape.clone();
        for w in &wires {
            let res = if normal { rib_normal(&shape, w, dir, t) } else { k::rib(&shape, w, &plane, t).map_err(|_| fe(RIB_FAILED)) };
            match res {
                Ok(r) => {
                    shape = r.shape;
                    made += 1;
                }
                Err(e) => last_err = Some(e),
            }
        }
        if shape.is_same(&body.shape) {
            continue;
        }
        if merge {
            let res = k::boolean(k::BooleanOp::Fuse, std::slice::from_ref(&body.shape), std::slice::from_ref(&shape), &k::BooleanOptions::default()).map_err(|_| fe("Failed to merge ribs into parts."))?;
            let empty = crate::naming::ShapeNames::default();
            let names = ev.name_result(&[&body.shape_names(), &empty], &res.history, &res.shape, &BTreeMap::new(), None);
            ev.replace_body(bi, res.shape, names);
        } else {
            let res = k::cut(&shape, &body.shape)?;
            let names = name_new(ev, &res.shape);
            new_ribs.push(NamedShape::new(res.shape, names));
        }
    }
    if made == 0 {
        // Onshape: RIB_PROFILE_FAILED when a profile failed, else RIB_NO_INTERSECTIONS.
        return Err(last_err.filter(|e| e.message != RIB_MISS).unwrap_or_else(|| fe(RIB_MISS)));
    }
    if !new_ribs.is_empty() {
        for (i, t) in new_ribs.into_iter().enumerate() {
            for (j, p) in crate::regen::eval::split_solids(&t.shape, &t.names.faces).into_iter().enumerate() {
                push_body(ev, BodyId { feature: ev.feature, index: (i * 64 + j) as u32 }, BodyKind::Solid, p.shape, p.names.faces);
            }
        }
    }
    Ok(())
}

/// Onshape's RIB_PROFILE_FAILED / RIB_NO_INTERSECTIONS.
const RIB_FAILED: &str = "Failed to create a rib from a selected profile.";
const RIB_MISS: &str = "None of the ribs intersected a part.";

/// Rib normal to the sketch plane: the profile thickened in-plane, extruded
/// along `dir` up to the part, fused with it.
fn rib_normal(base: &Shape, wire: &Shape, dir: DVec3, t: f64) -> FResult<k::OpResult> {
    let outline = k::offset_wire_2d(wire, t * 0.5, k::JoinType::Arc, false).map_err(|_| fe(RIB_FAILED))?;
    let outline = outline.wires().into_iter().next().unwrap_or(outline);
    let face = k::make_face_from_wire(&outline, true).map_err(|_| fe(RIB_FAILED))?;
    let body = k::extrude_up_to(&face, dir, base, 0.0).map_err(|_| fe(RIB_MISS))?;
    if body.shape.count(ShapeType::Solid) == 0 {
        return err(RIB_MISS);
    }
    let res = k::boolean(k::BooleanOp::Fuse, std::slice::from_ref(base), &[body.shape], &k::BooleanOptions::default())?;
    if res.shape.count(ShapeType::Solid) != base.count(ShapeType::Solid) {
        return err(RIB_MISS);
    }
    Ok(res)
}

// ------------------------------------------------------------------ primitives

pub fn cube(ev: &mut EvalCtx) -> FResult<()> {
    let s = ev.length("sideLength")?;
    if s <= 0.0 {
        return err("Side length must be positive");
    }
    let shape = k::make_box(&k::Plane::xy(), s, s, s)?;
    let names = name_new(ev, &shape);
    ev.apply_operation(vec![NamedShape::new(shape, names)], "NEW", None, BodyKind::Solid)
}

pub fn sphere(ev: &mut EvalCtx) -> FResult<()> {
    let r = ev.length("radius")?;
    if r <= 0.0 {
        return err("Radius must be positive");
    }
    let c = match ev.query("center").first() {
        Some(q) => ev.resolve_vertex_point(q).ok_or_else(|| fe("The sphere center no longer exists"))?,
        None => return err("Select a center point"),
    };
    let shape = k::make_sphere(&k::Plane::new(c, DVec3::Z, DVec3::X), r, std::f64::consts::TAU)?;
    let names = name_new(ev, &shape);
    ev.apply_operation(vec![NamedShape::new(shape, names)], "NEW", None, BodyKind::Solid)
}
