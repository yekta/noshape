//! `@op*` natives: modeling operations. Each runs through [`run_op`] (the
//! operation's marker feature id names what it creates) and reuses the
//! evaluator helpers of the built-in features.

use super::query::{evaluate_refs, with_ents};
use super::{FsContext, context_arg, convert::*, raise_error, run_op, run_op_with};
use crate::display::BodyKind;
use crate::naming::{Name, ShapeNames};
use crate::params::{ParamValue, ParamValues};
use crate::reference::{BodyId, Reference, Role, Source, TopoName};
use crate::regen::context::{Connector, PlaneEntity, PointEntity};
use crate::regen::eval::{EvalCtx, FResult, FeatureError, NamedShape, err, kernel_plane, split_solids, sweep_names};
use crate::regen::features::helpers_a::{copy_names, push_body};
use glam::{DMat4, DVec3};
use noshape_featurescript::interp::{Interp, NativeFn, R};
use noshape_featurescript::value::Value;
use noshape_kernel as k;
use noshape_kernel::{Shape, ShapeType};
use std::collections::BTreeMap;
use std::sync::Arc;

pub const NATIVES: &[(&str, NativeFn)] = &[
    ("opConstrainedSurface", op_constrained_surface),
    ("opWrap", op_wrap),
    ("opExtrude", op_extrude),
    ("opRevolve", op_revolve),
    ("opBoolean", op_boolean),
    ("opFillet", op_fillet),
    ("opChamfer", op_chamfer),
    ("opTransform", op_transform),
    ("opPattern", op_pattern),
    ("opDeleteBodies", op_delete_bodies),
    ("opPlane", op_plane),
    ("opPoint", op_point),
    ("opMateConnector", op_mate_connector),
    ("opShell", op_shell),
    ("opThicken", op_thicken),
    ("opSweep", op_sweep),
    ("opLoft", op_loft),
    ("opFitSpline", op_fit_spline),
    ("opPolyline", op_polyline),
    ("opHelix", op_helix),
    ("opSplitPart", op_split_part),
    ("opDraft", op_draft),
    ("opDeleteFace", op_delete_face),
    ("opMergeContexts", op_merge_contexts),
    ("opSphere", op_sphere),
    ("opOffsetFace", op_offset_face),
    ("opCreateBSplineCurve", op_create_bspline_curve),
    ("opExtractSurface", op_extract_surface),
    ("opSplitFace", op_split_face),
    ("opSplitEdges", op_split_edges),
    ("opExtendSheetBody", op_extend_sheet_body),
    ("opFillSurface", op_fill_surface),
    ("opBoundarySurface", op_boundary_surface),
    ("opIntersectFaces", op_intersect_faces),
    ("opFlipOrientation", op_flip_orientation),
    ("opReplaceFace", op_replace_face),
    ("opMoveFace", op_move_face),
    ("opExtractWires", op_extract_wires),
];

/// (context, id, definition) of an operation native.
fn args(i: &mut Interp, a: &[Value]) -> R<(Arc<FsContext>, super::FsId, Value)> {
    let cx = context_arg(i, a)?;
    let id = id_arg(i, a, 1)?;
    let def = def_arg(i, a, 2)?;
    Ok((cx, id, def))
}

fn fe(m: impl Into<String>) -> FeatureError {
    FeatureError::new(m)
}

/// Add new bodies (each solid/shell of each tool) with ids from a counter.
fn add_bodies(ev: &mut EvalCtx, tools: Vec<NamedShape>, kind: BodyKind) -> FResult<()> {
    if tools.is_empty() {
        return err("Nothing was created");
    }
    let mut n = ev.ctx.bodies.iter().filter(|b| b.id.feature == ev.feature).count() as u32;
    for t in tools {
        for p in split_solids(&t.shape, &t.names.faces) {
            let id = BodyId { feature: ev.feature, index: n };
            n += 1;
            push_body(ev, id, kind, p.shape, p.names.faces);
        }
    }
    Ok(())
}

fn solid_kind(s: &Shape) -> BodyKind {
    if !s.solids().is_empty() {
        BodyKind::Solid
    } else if !s.faces().is_empty() {
        BodyKind::Surface
    } else if !s.edges().is_empty() {
        BodyKind::Wire
    } else {
        BodyKind::Point
    }
}

/// Names for a shape created from nothing: faces ordered geometrically.
fn fresh_names(ev: &EvalCtx, shape: &Shape) -> Vec<Name> {
    let empty = ShapeNames::default();
    let hist = k::History::default();
    ev.name_result(&[&empty], &hist, shape, &BTreeMap::new(), None)
}

// ---------------------------------------------------------------- extrude / revolve

fn bound_depth(ev: &EvalCtx, def: &Value, bound: &str, depth_key: &str) -> FResult<Option<f64>> {
    match bound {
        "BLIND" => Ok(Some(length_of(def.get(depth_key)).ok_or_else(|| fe(format!("{depth_key} should be a length")))?)),
        "THROUGH_ALL" | "UP_TO_NEXT" => Ok(Some(ev.model_extent() * 2.0 + 1.0)),
        _ => Ok(None),
    }
}

fn op_extrude(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let (cx, id, def) = args(i, &a)?;
    let refs = evaluate_refs(i, &cx, def.get("entities"))?;
    let up_to = |key: &str, i: &mut Interp| -> R<Vec<Reference>> { evaluate_refs(i, &cx, def.get(key)) };
    let end_entity = up_to("endBoundEntity", i)?;
    let start_entity = up_to("startBoundEntity", i)?;
    let dir = vec3_of(def.get("direction")).map(|d| d.normalize_or_zero());
    let end_bound = def_enum(&def, "endBound").unwrap_or_else(|| "BLIND".into());
    let start_bound = def_enum(&def, "startBound");
    let start_opposite = def_bool(&def, "isStartBoundOpposite", true);
    run_op(i, &cx, &id, "EXTRUDE_FAILED", |ev| {
        let Some(d) = dir.filter(|d| *d != DVec3::ZERO) else { return err("Cannot compute extrude direction") };
        let end = bound_depth(ev, &def, &end_bound, "endDepth")?;
        let start = match &start_bound {
            Some(b) => bound_depth(ev, &def, b, "startDepth")?.map(|x| if start_opposite { x } else { -x }),
            None => Some(0.0),
        };
        let target = |refs: &[Reference]| -> FResult<Shape> {
            let faces = ev.resolve_faces(refs);
            let bodies = ev.resolve_bodies(refs);
            let shapes: Vec<Shape> = faces.iter().filter_map(|(b, f)| ev.face_shape(*b, *f).ok()).chain(bodies.iter().map(|b| ev.ctx.bodies[*b].shape.clone())).collect();
            match shapes.len() {
                0 => err("End condition must be a surface"),
                1 => Ok(shapes[0].clone()),
                _ => Ok(Shape::compound(&shapes)),
            }
        };
        let profiles = ev.profiles(&refs)?;
        let mut tools = Vec::new();
        for p in &profiles {
            let res = match (start, end) {
                (Some(s), Some(e)) => k::extrude_between(&p.shape, -d * s, d * e, 0.0),
                (Some(s), None) => k::extrude_up_to(&p.shape, d, &target(&end_entity)?, -s),
                (None, Some(e)) => k::extrude_up_to(&p.shape, -d, &target(&start_entity)?, 0.0).and_then(|r| {
                    let ext = k::extrude(&p.shape, d * e)?;
                    k::fuse(&r.shape, &ext.shape)
                }),
                (None, None) => return err("Both extrude ends can't be up to entities"),
            }
            .map_err(|e| fe(format!("Extrude failed: {e}")))?;
            let names = sweep_names(ev, p, &res, true, true);
            tools.push(NamedShape::new(res.shape.clone(), names));
        }
        // Edges extrude to sheets.
        for (n, edge) in edge_shapes(ev, &refs)?.into_iter().enumerate() {
            let (s, e) = (start.unwrap_or(0.0), end.unwrap_or(0.0));
            let res = k::extrude_between(&edge, -d * s, d * e, 0.0).map_err(|e| fe(format!("Extrude failed: {e}")))?;
            let mut roles = BTreeMap::new();
            for f in 0..res.shape.count(ShapeType::Face) {
                roles.insert(f as u32, Role::Side(Source::Index(n as u32 * 64 + f as u32)));
            }
            let names = ev.name_result(&[&ShapeNames::default()], &res.history, &res.shape, &roles, None);
            tools.push(NamedShape::new(res.shape, names));
        }
        if tools.is_empty() {
            return err("Could not use selected entities to extrude");
        }
        let kind = if tools.iter().all(|t| t.shape.solids().is_empty()) { BodyKind::Surface } else { BodyKind::Solid };
        add_bodies(ev, tools, kind)
    })?;
    Ok(Value::Undefined)
}

/// Kernel edges of curve references (sketch curves, body edges).
fn edge_shapes(ev: &EvalCtx, refs: &[Reference]) -> FResult<Vec<Shape>> {
    let curves: Vec<Reference> = refs.iter().filter(|r| matches!(r, Reference::Edge(_)) || matches!(r, Reference::SketchEntity { sketch, entity } if ev.ctx.sketch(*sketch).is_some_and(|s| s.sketch.kind(noshape_sketch::EntityId(*entity)).is_some_and(|k| k.is_curve())))).cloned().collect();
    if curves.is_empty() {
        return Ok(Vec::new());
    }
    crate::regen::features::bshared::curve_edges(ev, &curves)
}

fn op_revolve(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let (cx, id, def) = args(i, &a)?;
    let refs = evaluate_refs(i, &cx, def.get("entities"))?;
    let Some((origin, axis)) = line_of(def.get("axis")) else { return Err(i.error("opRevolve: axis should be a Line")) };
    let fwd = angle_of(def.get("angleForward")).unwrap_or(std::f64::consts::TAU);
    let back = angle_of(def.get("angleBack")).unwrap_or(0.0);
    run_op(i, &cx, &id, "REVOLVE_FAILED", |ev| {
        let tau = std::f64::consts::TAU;
        let norm = |x: f64| if x >= tau - 1e-12 { tau } else { x.rem_euclid(tau) };
        let (f, b) = (norm(fwd), norm(back));
        let total = (f + b).min(tau);
        let full = total >= tau - 1e-9;
        let profiles = ev.profiles(&refs)?;
        let mut tools = Vec::new();
        for p in &profiles {
            let res = if full { k::revolve(&p.shape, origin, axis, tau) } else { k::revolve_between(&p.shape, origin, axis, -b, f) }.map_err(|e| fe(format!("Revolve failed: {e}")))?;
            let names = sweep_names(ev, p, &res, !full, !full);
            tools.push(NamedShape::new(res.shape.clone(), names));
        }
        for edge in edge_shapes(ev, &refs)? {
            let res = if full { k::revolve(&edge, origin, axis, tau) } else { k::revolve_between(&edge, origin, axis, -b, f) }.map_err(|e| fe(format!("Revolve failed: {e}")))?;
            let names = fresh_names(ev, &res.shape);
            tools.push(NamedShape::new(res.shape, names));
        }
        if tools.is_empty() {
            return err("Could not use selected entities to revolve");
        }
        let kind = if tools.iter().all(|t| t.shape.solids().is_empty()) { BodyKind::Surface } else { BodyKind::Solid };
        add_bodies(ev, tools, kind)
    })?;
    Ok(Value::Undefined)
}

// ---------------------------------------------------------------- booleans and blends

fn op_boolean(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let (cx, id, def) = args(i, &a)?;
    let tool_refs = evaluate_refs(i, &cx, def.get("tools"))?;
    let target_refs = evaluate_refs(i, &cx, def.get("targets"))?;
    let kind = def_enum(&def, "operationType").unwrap_or_else(|| "UNION".into());
    let keep = def_bool(&def, "keepTools", false);
    let failure = match &*kind {
        "SUBTRACTION" => "BOOLEAN_SUBTRACT_NO_OP",
        "INTERSECTION" => "BOOLEAN_INTERSECT_FAIL",
        _ => "BOOLEAN_INVALID",
    };
    let merged = run_op(i, &cx, &id, failure, |ev| {
        let index = |ev: &EvalCtx, id: &BodyId| ev.ctx.bodies.iter().position(|b| &b.id == id);
        let body_named = |ev: &EvalCtx, i: usize| crate::regen::features::helpers_a::body_named(&ev.ctx.bodies[i]);
        let tools: Vec<BodyId> = ev.resolve_bodies(&tool_refs).into_iter().map(|i| ev.ctx.bodies[i].id.clone()).collect();
        let targets: Vec<BodyId> = ev.resolve_bodies(&target_refs).into_iter().map(|i| ev.ctx.bodies[i].id.clone()).filter(|t| !tools.contains(t) || kind.as_ref() == "UNION").collect();
        match &*kind {
            "UNION" => {
                let mut all: Vec<BodyId> = targets.clone();
                for t in &tools {
                    if !all.contains(t) {
                        all.push(t.clone());
                    }
                }
                if all.len() < 2 {
                    return err("Need at least two parts or surfaces for a Boolean operation.");
                }
                let first = index(ev, &all[0]).ok_or_else(|| fe("missing body"))?;
                let others: Vec<NamedShape> = all[1..].iter().filter_map(|id| index(ev, id)).map(|i| body_named(ev, i)).collect();
                let remove: Vec<BodyId> = all[1..].to_vec();
                ev.ctx.bodies.retain(|b| !remove.contains(&b.id));
                let first = index(ev, &all[0]).unwrap_or(first);
                ev.boolean_into(&[first], others, k::BooleanOp::Fuse)?;
                Ok(Some(all))
            }
            "INTERSECTION" if targets.is_empty() => {
                // Tools only: their common volume, kept in the first tool.
                if tools.len() < 2 {
                    return err("Need at least two parts or surfaces for a Boolean operation.");
                }
                let first = index(ev, &tools[0]).ok_or_else(|| fe("missing body"))?;
                let others: Vec<NamedShape> = tools[1..].iter().filter_map(|id| index(ev, id)).map(|i| body_named(ev, i)).collect();
                let remove: Vec<BodyId> = tools[1..].to_vec();
                ev.ctx.bodies.retain(|b| !remove.contains(&b.id));
                let first = index(ev, &tools[0]).unwrap_or(first);
                ev.boolean_into(&[first], others, k::BooleanOp::Common)?;
                Ok(Some(tools))
            }
            _ => {
                if targets.is_empty() || tools.is_empty() {
                    return err("Boolean needs targets and tools");
                }
                let tool_shapes: Vec<NamedShape> = tools.iter().filter_map(|id| index(ev, id)).map(|i| body_named(ev, i)).collect();
                if !keep {
                    ev.ctx.bodies.retain(|b| !tools.contains(&b.id));
                }
                let idx: Vec<usize> = targets.iter().filter_map(|id| index(ev, id)).collect();
                let op = if &*kind == "SUBTRACTION" { k::BooleanOp::Cut } else { k::BooleanOp::Common };
                ev.boolean_into(&idx, tool_shapes, op)?;
                Ok(None)
            }
        }
    })?;
    if let Some(all) = merged {
        cx.state.lock().record_merge(&all[0], &all, &id);
    }
    Ok(Value::Undefined)
}

fn by_body(items: Vec<(usize, usize)>) -> BTreeMap<usize, Vec<usize>> {
    let mut m: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for (b, e) in items {
        m.entry(b).or_default().push(e);
    }
    m
}

fn blend(i: &mut Interp, a: &[Value], chamfer: bool) -> R<Value> {
    let (cx, id, def) = args(i, a)?;
    let refs = evaluate_refs(i, &cx, def.get("entities"))?;
    let spec = if chamfer {
        let t = def_enum(&def, "chamferType").unwrap_or_else(|| "EQUAL_OFFSETS".into());
        match &*t {
            "TWO_OFFSETS" => {
                let (w1, w2) = (def_length(i, &def, "width1")?, def_length(i, &def, "width2")?);
                if def_bool(&def, "oppositeDirection", false) { k::ChamferSpec::TwoDistances(w2, w1) } else { k::ChamferSpec::TwoDistances(w1, w2) }
            }
            "OFFSET_ANGLE" => k::ChamferSpec::DistanceAngle(def_length(i, &def, "width")?, def_angle(i, &def, "angle")?),
            _ => k::ChamferSpec::Equal(def_length(i, &def, "width")?),
        }
    } else {
        k::ChamferSpec::Equal(def_length(i, &def, "radius")?)
    };
    run_op(i, &cx, &id, if chamfer { "CHAMFER_FAILED" } else { "FILLET_FAILED" }, |ev| {
        let groups = by_body(ev.resolve_edges(&refs));
        if groups.is_empty() {
            return Err(super::code_error(if chamfer { "CHAMFER_SELECT_EDGES" } else { "FILLET_SELECT_EDGES" }));
        }
        let blend_role = |n: &Name| Role::Blend(n.clone());
        for (bi, edges) in groups.into_iter().rev() {
            let body = ev.ctx.bodies[bi].clone();
            let shapes: Vec<Shape> = edges.iter().filter_map(|e| body.shape.sub_shape(ShapeType::Edge, *e).ok()).collect();
            let res = match (chamfer, spec) {
                (false, k::ChamferSpec::Equal(r)) => k::fillet(&body.shape, &shapes, r),
                (_, s) => k::chamfer(&body.shape, &shapes, s, &[]),
            }
            .map_err(|e| fe(e.to_string()))?;
            let names = body.shape_names();
            let faces = ev.name_result(&[&names], &res.history, &res.shape, &BTreeMap::new(), Some(&blend_role));
            ev.replace_body(bi, res.shape, faces);
        }
        Ok(())
    })?;
    Ok(Value::Undefined)
}

fn op_fillet(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    blend(i, &a, false)
}

fn op_chamfer(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    blend(i, &a, true)
}

// ---------------------------------------------------------------- transforms and copies

fn op_transform(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let (cx, id, def) = args(i, &a)?;
    let refs = evaluate_refs(i, &cx, def.get("bodies"))?;
    let Some(m) = transform_of(def.get("transform")) else { return Err(i.error("opTransform: transform should be a Transform")) };
    run_op(i, &cx, &id, "TRANSFORM_FAILED", |ev| {
        transform_refs(ev, &refs, &m)?;
        Ok(())
    })?;
    Ok(Value::Undefined)
}

/// Move bodies, sketches, planes, points and mate connectors in place.
fn transform_refs(ev: &mut EvalCtx, refs: &[Reference], m: &DMat4) -> FResult<()> {
    let t = k::Transform::Matrix(*m);
    for bi in ev.resolve_bodies(refs) {
        let body = ev.ctx.bodies[bi].clone();
        let res = k::transform(&body.shape, &t, true)?;
        let names = body.shape_names();
        let faces = ev.name_result(&[&names], &res.history, &res.shape, &BTreeMap::new(), None);
        ev.replace_body(bi, res.shape, faces);
    }
    for r in refs {
        match r {
            Reference::Sketch(f) | Reference::SketchEntity { sketch: f, .. } | Reference::SketchRegion { sketch: f, .. } => {
                if let Some(pos) = ev.ctx.sketches.iter().position(|s| s.feature == *f) {
                    let s = &ev.ctx.sketches[pos];
                    let frame = crate::regen::eval::transform_frame(&s.frame, m);
                    let new = crate::regen::context::SketchResult { frame, ..clone_sketch(s) };
                    ev.ctx.sketches[pos] = Arc::new(new);
                }
            }
            _ => {
                for p in ev.ctx.planes.iter_mut().filter(|p| &p.reference == r) {
                    p.frame = crate::regen::eval::transform_frame(&p.frame, m);
                }
                for c in ev.ctx.connectors.iter_mut().filter(|c| &c.reference == r) {
                    c.origin = m.transform_point3(c.origin);
                    c.x = m.transform_vector3(c.x).normalize_or_zero();
                    c.z = m.transform_vector3(c.z).normalize_or_zero();
                }
                for p in ev.ctx.points.iter_mut().filter(|p| &p.reference == r) {
                    p.position = m.transform_point3(p.position);
                }
            }
        }
    }
    Ok(())
}

pub fn clone_sketch(s: &crate::regen::context::SketchResult) -> crate::regen::context::SketchResult {
    crate::regen::context::SketchResult {
        feature: s.feature,
        name: s.name.clone(),
        frame: s.frame,
        sketch: s.sketch.clone(),
        report: s.report.clone(),
        regions: s.regions.clone(),
        wires: s.wires.clone(),
        dimension_text: s.dimension_text.clone(),
        images: s.images.clone(),
    }
}

/// Stable instance number of a pattern instance name.
fn instance_number(name: &str, fallback: usize) -> u32 {
    if name.is_empty() {
        return fallback as u32 + 1;
    }
    let mut h: u32 = 0x811c_9dc5;
    for b in name.bytes() {
        h ^= b as u32;
        h = h.wrapping_mul(0x0100_0193);
    }
    (h & 0x3FF_FFFF) | 1
}

fn op_pattern(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let (cx, id, def) = args(i, &a)?;
    let refs = evaluate_refs(i, &cx, def.get("entities"))?;
    let transforms: Vec<DMat4> = def.get("transforms").as_array().map(|v| v.iter().filter_map(transform_of).collect()).unwrap_or_default();
    let names: Vec<String> = def.get("instanceNames").as_array().map(|v| v.iter().map(|x| x.as_str().map(|s| s.to_string()).unwrap_or_default()).collect()).unwrap_or_default();
    if transforms.len() > 2500 {
        return Err(raise_error(i, "PATTERN_INPUT_TOO_MANY_INSTANCES", None));
    }
    run_op(i, &cx, &id, "PATTERN_BODY_FAILED", |ev| {
        let bodies = ev.resolve_bodies(&refs);
        if bodies.is_empty() {
            return err("Could not pattern selected parts.");
        }
        let mut n = 0u32;
        for bi in bodies {
            let body = ev.ctx.bodies[bi].clone();
            for (k_, m) in transforms.iter().enumerate() {
                let res = k::transform(&body.shape, &k::Transform::Matrix(*m), true)?;
                let inst = instance_number(names.get(k_).map(|s| s.as_str()).unwrap_or(""), k_);
                let faces = copy_names(ev, &body.faces, &res, inst);
                push_body(ev, BodyId { feature: ev.feature, index: n }, body.kind, res.shape, faces);
                n += 1;
            }
        }
        Ok(())
    })?;
    Ok(Value::Undefined)
}

fn op_delete_bodies(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let (cx, id, def) = args(i, &a)?;
    // Owner bodies of whatever was passed.
    let owners = with_ents(i, &cx, def.get("entities"), |_, q, ents| Ok(ents.iter().filter_map(|e| q.owner(e).or(Some(e.clone()))).map(|e| q.to_ref(&e)).collect::<Vec<_>>()))?;
    run_op(i, &cx, &id, "DELETE_PARTS_FAILED", |ev| {
        for r in &owners {
            match r {
                Reference::Body(b) => ev.ctx.bodies.retain(|x| &x.id != b),
                Reference::Sketch(f) => ev.ctx.sketches.retain(|s| s.feature != *f),
                other => {
                    ev.ctx.planes.retain(|p| &p.reference != other);
                    ev.ctx.connectors.retain(|p| &p.reference != other);
                    ev.ctx.points.retain(|p| &p.reference != other);
                }
            }
        }
        Ok(())
    })?;
    Ok(Value::Undefined)
}

// ---------------------------------------------------------------- construction geometry

fn op_plane(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let (cx, id, def) = args(i, &a)?;
    let Some(frame) = plane_of(def.get("plane")) else { return Err(i.error("opPlane: plane should be a Plane")) };
    let w = def_length_opt(&def, "width").unwrap_or(0.1524);
    let h = def_length_opt(&def, "height").unwrap_or(w);
    let name = id.last().map(|s| s.to_string()).unwrap_or_default();
    run_op(i, &cx, &id, "REGEN_ERROR", |ev| {
        ev.ctx.planes.push(PlaneEntity { reference: Reference::FeaturePlane(ev.feature), name, frame, half_width: w * 0.5, half_height: h * 0.5 });
        Ok(())
    })?;
    Ok(Value::Undefined)
}

fn op_point(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let (cx, id, def) = args(i, &a)?;
    let p = def_vec3(i, &def, "point")?;
    run_op(i, &cx, &id, "REGEN_ERROR", |ev| {
        ev.ctx.points.push(PointEntity { reference: Reference::Feature(ev.feature), position: p });
        Ok(())
    })?;
    Ok(Value::Undefined)
}

fn op_mate_connector(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let (cx, id, def) = args(i, &a)?;
    let Some((o, x, z)) = coord_system_of(def.get("coordSystem")) else { return Err(i.error("opMateConnector: coordSystem should be a CoordSystem")) };
    let name = id.last().map(|s| s.to_string()).unwrap_or_default();
    run_op(i, &cx, &id, "MATECONNECTOR_INVALID_MATE", |ev| {
        let index = ev.ctx.connectors.iter().filter(|c| matches!(c.reference, Reference::MateConnector { feature, .. } if feature == ev.feature)).count() as u32;
        ev.ctx.connectors.push(Connector { reference: Reference::MateConnector { feature: ev.feature, index }, name, origin: o, x, z });
        Ok(())
    })?;
    Ok(Value::Undefined)
}

// ---------------------------------------------------------------- solids

fn op_shell(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let (cx, id, def) = args(i, &a)?;
    let refs = evaluate_refs(i, &cx, def.get("entities"))?;
    let t = def_length(i, &def, "thickness")?;
    run_op(i, &cx, &id, "SHELL_FAILED", |ev| {
        let faces = ev.resolve_faces(&refs);
        let mut bodies: Vec<usize> = faces.iter().map(|f| f.0).collect();
        bodies.extend(refs.iter().filter(|r| matches!(r, Reference::Body(_))).flat_map(|r| ev.resolve_bodies(std::slice::from_ref(r))));
        bodies.sort_unstable();
        bodies.dedup();
        for bi in bodies.into_iter().rev() {
            let body = ev.ctx.bodies[bi].clone();
            let removed: Vec<Shape> = faces.iter().filter(|f| f.0 == bi).filter_map(|f| body.shape.sub_shape(ShapeType::Face, f.1).ok()).collect();
            let side = if t < 0.0 { k::ShellSide::Inside } else { k::ShellSide::Outside };
            let res = k::shell(&body.shape, &removed, t.abs(), side, k::JoinType::default())?;
            let names = body.shape_names();
            let offset = |n: &Name| Role::Offset(n.clone());
            let new = ev.name_result(&[&names], &res.history, &res.shape, &BTreeMap::new(), Some(&offset));
            ev.replace_body(bi, res.shape, new);
        }
        Ok(())
    })?;
    Ok(Value::Undefined)
}

fn op_thicken(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let (cx, id, def) = args(i, &a)?;
    let refs = evaluate_refs(i, &cx, def.get("entities"))?;
    let t1 = def_length(i, &def, "thickness1")?;
    let t2 = def_length(i, &def, "thickness2")?;
    run_op(i, &cx, &id, "THICKEN_FAILED", |ev| {
        let mut shapes: Vec<Shape> = ev.resolve_faces(&refs).iter().filter_map(|(b, f)| ev.face_shape(*b, *f).ok()).collect();
        for bi in ev.resolve_bodies(&refs) {
            if ev.ctx.bodies[bi].kind == BodyKind::Surface {
                shapes.push(ev.ctx.bodies[bi].shape.clone());
            }
        }
        for p in ev.profiles(&refs.iter().filter(|r| matches!(r, Reference::SketchRegion { .. })).cloned().collect::<Vec<_>>())? {
            shapes.push(p.shape);
        }
        let mut tools = Vec::new();
        for s in shapes {
            let res = k::thicken(&s, t1, t2)?;
            let names = fresh_names(ev, &res.shape);
            tools.push(NamedShape::new(res.shape, names));
        }
        add_bodies(ev, tools, BodyKind::Solid)
    })?;
    Ok(Value::Undefined)
}

fn path_wire(ev: &EvalCtx, refs: &[Reference]) -> FResult<Shape> {
    let edges = crate::regen::features::bshared::curve_edges(ev, refs)?;
    if edges.is_empty() {
        return err("Select a sweep path");
    }
    Ok(k::make_wire(&edges)?)
}

/// Sweep with the minimal-twist frame; paths where OCCT's corrected Frenet
/// frame fails (helices) are retried with the Frenet frame, which is the
/// minimal-twist frame of a helix, then the discrete one.
fn sweep_any_frame(profile: &Shape, path: &Shape, opts: &k::SweepOptions) -> k::Result<k::OpResult> {
    let mut last = None;
    for mode in [k::SweepMode::CorrectedFrenet, k::SweepMode::Frenet, k::SweepMode::Discrete] {
        match k::sweep(profile, path, &k::SweepOptions { mode, ..opts.clone() }) {
            Ok(r) if r.shape.is_valid() => return Ok(r),
            Ok(r) => last = Some(Ok(r)),
            Err(e) => last = Some(Err(e)),
        }
    }
    last.expect("at least one sweep attempt")
}

fn op_sweep(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let (cx, id, def) = args(i, &a)?;
    let profiles = evaluate_refs(i, &cx, def.get("profiles"))?;
    let path = evaluate_refs(i, &cx, def.get("path"))?;
    run_op(i, &cx, &id, "SWEEP_FAILED", |ev| {
        let wire = path_wire(ev, &path)?;
        let opts = k::SweepOptions { solid: true, ..Default::default() };
        let mut tools = Vec::new();
        for p in ev.profiles(&profiles)? {
            let res = sweep_any_frame(&p.shape, &wire, &opts).map_err(|e| fe(format!("Sweep failed: {e}")))?;
            let names = sweep_names(ev, &p, &res, true, true);
            tools.push(NamedShape::new(res.shape.clone(), names));
        }
        for e in edge_shapes(ev, &profiles)? {
            let res = k::sweep(&e, &wire, &k::SweepOptions { solid: false, ..Default::default() })?;
            let names = fresh_names(ev, &res.shape);
            tools.push(NamedShape::new(res.shape, names));
        }
        let kind = if tools.iter().all(|t| t.shape.solids().is_empty()) { BodyKind::Surface } else { BodyKind::Solid };
        add_bodies(ev, tools, kind)
    })?;
    Ok(Value::Undefined)
}

fn op_loft(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let (cx, id, def) = args(i, &a)?;
    let mut sections = Vec::new();
    if let Some(arr) = def.get("profileSubqueries").as_array().cloned() {
        for q in arr.iter() {
            sections.push(evaluate_refs(i, &cx, q)?);
        }
    }
    let solid = !def_bool(&def, "bodyType", false);
    run_op(i, &cx, &id, "LOFT_FAILED", |ev| {
        let mut shapes = Vec::new();
        for s in &sections {
            if let Some(p) = ev.profiles(s)?.into_iter().next() {
                shapes.push(p.shape.wires().into_iter().next().unwrap_or(p.shape));
            } else if let Some(pt) = s.first().and_then(|r| ev.resolve_vertex_point(r)) {
                shapes.push(k::make_vertex(pt)?);
            } else {
                let e = crate::regen::features::bshared::curve_edges(ev, s)?;
                shapes.push(k::make_wire(&e)?);
            }
        }
        if shapes.len() < 2 {
            return err("Select at least two profiles.");
        }
        let res = k::loft(&shapes, &k::LoftOptions { solid, ..Default::default() })?;
        let names = fresh_names(ev, &res.shape);
        let kind = solid_kind(&res.shape);
        add_bodies(ev, vec![NamedShape::new(res.shape, names)], kind)
    })?;
    Ok(Value::Undefined)
}

fn op_sphere(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let (cx, id, def) = args(i, &a)?;
    let c = def_vec3(i, &def, "center")?;
    let r = def_length(i, &def, "radius")?;
    run_op(i, &cx, &id, "REGEN_ERROR", |ev| {
        if r <= 0.0 {
            return err("Radius must be positive");
        }
        let s = k::make_sphere(&k::Plane::new(c, DVec3::Z, DVec3::X), r, std::f64::consts::TAU)?;
        let names = fresh_names(ev, &s);
        add_bodies(ev, vec![NamedShape::new(s, names)], BodyKind::Solid)
    })?;
    Ok(Value::Undefined)
}

// ---------------------------------------------------------------- curves

fn add_wire(ev: &mut EvalCtx, shape: Shape) -> FResult<()> {
    let id = BodyId { feature: ev.feature, index: ev.ctx.bodies.iter().filter(|b| b.id.feature == ev.feature).count() as u32 };
    push_body(ev, id, BodyKind::Wire, shape, Vec::new());
    Ok(())
}

fn op_fit_spline(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let (cx, id, def) = args(i, &a)?;
    let pts: Vec<DVec3> = def.get("points").as_array().map(|v| v.iter().filter_map(vec3_of).collect()).unwrap_or_default();
    let mut tangents = Vec::new();
    if let (Some(s), Some(e)) = (vec3_of(def.get("startDerivative")), vec3_of(def.get("endDerivative"))) {
        tangents = vec![s, e];
    }
    run_op(i, &cx, &id, "SKETCH_SPLINE_FAILED", |ev| {
        if pts.len() < 2 {
            return err("A spline needs at least two points");
        }
        let closed = pts.len() > 2 && (pts[0] - pts[pts.len() - 1]).length() < 1e-9;
        let p = if closed { &pts[..pts.len() - 1] } else { &pts[..] };
        let s = k::make_interpolated_spline(p, &tangents, closed)?;
        add_wire(ev, s)
    })?;
    Ok(Value::Undefined)
}

/// Wire bodies copied from edges (one per connected chain).
fn op_extract_wires(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let (cx, id, def) = args(i, &a)?;
    let refs = evaluate_refs(i, &cx, def.get("edges"))?;
    run_op(i, &cx, &id, "REGEN_ERROR", |ev| {
        let edges = crate::regen::features::bshared::curve_edges(ev, &refs)?;
        if edges.is_empty() {
            return err("Select edges");
        }
        for w in k::edges_to_wires(&Shape::compound(&edges), 1e-7)? {
            add_wire(ev, w)?;
        }
        Ok(())
    })?;
    Ok(Value::Undefined)
}

fn op_polyline(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let (cx, id, def) = args(i, &a)?;
    let pts: Vec<DVec3> = def.get("points").as_array().map(|v| v.iter().filter_map(vec3_of).collect()).unwrap_or_default();
    run_op(i, &cx, &id, "POLYLINE_CONSECUTIVE_EQUAL_POINTS", |ev| {
        if pts.windows(2).any(|w| (w[0] - w[1]).length() < 1e-12) {
            return err("Consecutive points are equal");
        }
        let closed = pts.len() > 2 && (pts[0] - pts[pts.len() - 1]).length() < 1e-9;
        let p = if closed { &pts[..pts.len() - 1] } else { &pts[..] };
        add_wire(ev, k::make_polyline(p, closed)?)
    })?;
    Ok(Value::Undefined)
}

fn op_helix(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let (cx, id, def) = args(i, &a)?;
    let dir = def_vec3(i, &def, "direction")?.normalize_or_zero();
    let axis_start = def_vec3(i, &def, "axisStart")?;
    let start = def_vec3(i, &def, "startPoint")?;
    let interval = def.get("interval").as_array().map(|v| v.iter().filter_map(|x| x.as_number()).collect::<Vec<_>>()).unwrap_or_else(|| vec![0.0, 1.0]);
    let clockwise = def_bool(&def, "clockwise", false);
    let pitch = def_length(i, &def, "helicalPitch")?;
    run_op(i, &cx, &id, "HELIX_FAILED", |ev| {
        let r_vec = (start - axis_start) - dir * (start - axis_start).dot(dir);
        let radius = r_vec.length();
        if radius <= 0.0 {
            return err("Both radii are zero");
        }
        let turns = interval.get(1).copied().unwrap_or(1.0) - interval.first().copied().unwrap_or(0.0);
        let frame = k::Plane::new(axis_start + dir * (start - axis_start).dot(dir), dir, r_vec / radius);
        add_wire(ev, k::make_helix(&frame, radius, pitch, pitch * turns, clockwise, 0.0)?)
    })?;
    Ok(Value::Undefined)
}

// ---------------------------------------------------------------- splits and direct edits

fn op_split_part(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let (cx, id, def) = args(i, &a)?;
    let targets = evaluate_refs(i, &cx, def.get("targets"))?;
    let tool_refs = evaluate_refs(i, &cx, def.get("tool"))?;
    let keep_tools = def_bool(&def, "keepTools", false);
    let split_targets = run_op(i, &cx, &id, "SPLIT_FAILED", |ev| {
        let mut tools: Vec<Shape> = Vec::new();
        for r in &tool_refs {
            if let Some(p) = matches!(r, Reference::Plane(_) | Reference::FeaturePlane(_)).then(|| ev.resolve_plane(r)).flatten() {
                tools.push(k::make_plane_face(&kernel_plane(&p), -1e3, 1e3, -1e3, 1e3)?);
            }
        }
        tools.extend(ev.resolve_faces(&tool_refs).iter().filter_map(|(b, f)| ev.face_shape(*b, *f).ok()));
        let tool_bodies = ev.resolve_bodies(&tool_refs.iter().filter(|r| matches!(r, Reference::Body(_))).cloned().collect::<Vec<_>>());
        tools.extend(tool_bodies.iter().map(|b| ev.ctx.bodies[*b].shape.clone()));
        if tools.is_empty() {
            return err("Select a tool");
        }
        let target_ids: Vec<BodyId> = ev.resolve_bodies(&targets).into_iter().map(|b| ev.ctx.bodies[b].id.clone()).collect();
        for bi in ev.resolve_bodies(&targets).into_iter().rev() {
            let body = ev.ctx.bodies[bi].clone();
            let res = k::split(std::slice::from_ref(&body.shape), &tools, &Default::default())?;
            let names = body.shape_names();
            let faces = ev.name_result(&[&names], &res.history, &res.shape, &BTreeMap::new(), None);
            ev.replace_body(bi, res.shape, faces);
        }
        if !keep_tools {
            let ids: Vec<BodyId> = tool_bodies.iter().filter_map(|b| ev.ctx.bodies.get(*b).map(|x| x.id.clone())).collect();
            ev.ctx.bodies.retain(|b| !ids.contains(&b.id));
        }
        Ok(target_ids)
    })?;
    // Split pieces are "created by" the split and by the original's creator
    // (std query.fs, qCreatedBy).
    let mut st = cx.state.lock();
    let pieces: Vec<BodyId> = st.ctx.bodies.iter().map(|b| b.id.clone()).filter(|b| !split_targets.contains(b) && st.body_creator(b).as_deref() == Some(&id[..])).collect();
    for piece in pieces {
        st.record_merge(&piece, &split_targets, &id);
    }
    Ok(Value::Undefined)
}

fn op_draft(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let (cx, id, def) = args(i, &a)?;
    let faces = evaluate_refs(i, &cx, def.get("draftFaces"))?;
    let neutral = evaluate_refs(i, &cx, def.get("neutralPlane"))?;
    let angle = def_angle(i, &def, "angle")?;
    let pull = vec3_of(def.get("pullDirection"));
    run_op(i, &cx, &id, "DRAFT_FAILED", |ev| {
        let Some(np) = neutral.first().and_then(|r| ev.resolve_plane(r)) else { return err("Could not resolve the neutral plane.") };
        let dir = pull.unwrap_or(np.normal).normalize_or_zero();
        let groups = by_body(ev.resolve_faces(&faces));
        for (bi, fs) in groups.into_iter().rev() {
            let body = ev.ctx.bodies[bi].clone();
            let shapes: Vec<Shape> = fs.iter().filter_map(|f| body.shape.sub_shape(ShapeType::Face, *f).ok()).collect();
            let res = k::draft(&body.shape, &shapes, dir, &kernel_plane(&np), angle)?;
            let names = body.shape_names();
            let new = ev.name_result(&[&names], &res.history, &res.shape, &BTreeMap::new(), None);
            ev.replace_body(bi, res.shape, new);
        }
        Ok(())
    })?;
    Ok(Value::Undefined)
}

fn op_delete_face(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let (cx, id, def) = args(i, &a)?;
    let faces = evaluate_refs(i, &cx, def.get("deleteFaces"))?;
    run_op(i, &cx, &id, "DIRECT_EDIT_DELETE_FACE_FAILED", |ev| {
        for (bi, fs) in by_body(ev.resolve_faces(&faces)).into_iter().rev() {
            let body = ev.ctx.bodies[bi].clone();
            let shapes: Vec<Shape> = fs.iter().filter_map(|f| body.shape.sub_shape(ShapeType::Face, *f).ok()).collect();
            let res = k::delete_faces(&body.shape, &shapes)?;
            let names = body.shape_names();
            let new = ev.name_result(&[&names], &res.history, &res.shape, &BTreeMap::new(), None);
            ev.replace_body(bi, res.shape, new);
        }
        Ok(())
    })?;
    Ok(Value::Undefined)
}

/// opCreateBSplineCurve: a wire body from a std `BSplineCurve`.
fn op_create_bspline_curve(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let (cx, id, def) = args(i, &a)?;
    let c = def.get("bSplineCurve");
    let num = |v: &Value| v.as_number().or_else(|| v.get("value").as_number());
    let poles: Vec<DVec3> = c.get("controlPoints").as_array().map(|a| a.iter().filter_map(vec3_of).collect()).unwrap_or_default();
    let flat: Vec<f64> = c.get("knots").as_array().map(|a| a.iter().filter_map(num).collect()).unwrap_or_default();
    let weights: Option<Vec<f64>> = c.get("isRational").as_bool().unwrap_or(false).then(|| c.get("weights").as_array().map(|a| a.iter().filter_map(num).collect()).unwrap_or_default());
    let degree = c.get("degree").as_number().unwrap_or(0.0) as u32;
    let periodic = c.get("isPeriodic").as_bool().unwrap_or(false);
    // Distinct knots with multiplicities.
    let (mut knots, mut mults): (Vec<f64>, Vec<u32>) = (Vec::new(), Vec::new());
    for k in flat {
        match knots.last() {
            Some(&last) if (k - last).abs() <= 1e-12 * (1.0 + k.abs()) => *mults.last_mut().expect("paired with knots") += 1,
            _ => {
                knots.push(k);
                mults.push(1);
            }
        }
    }
    run_op(i, &cx, &id, "BAD_BSPLINECURVE_DEFINITION", |ev| {
        if poles.len() < 2 || degree == 0 {
            return err("Invalid B-spline curve");
        }
        add_wire(ev, k::make_bspline(&poles, weights.as_deref(), &knots, &mults, degree, periodic)?)
    })?;
    Ok(Value::Undefined)
}

/// opExtractSurface: Offset surface's evaluator (offset 0 copies the faces).
/// Tangent propagation of the seed faces is not applied yet.
fn op_extract_surface(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let (cx, id, def) = args(i, &a)?;
    let faces = evaluate_refs(i, &cx, def.get("faces"))?;
    let offset = def_length_opt(&def, "offset").unwrap_or(0.0);
    let mut p = ParamValues::default();
    p.set("surfacesAndFaces", ParamValue::Query(faces));
    p.set("offset", length_param(offset.abs()));
    p.set("oppositeDirection", ParamValue::Bool(offset < 0.0));
    run_op_with(i, &cx, &id, "REGEN_ERROR", &p, crate::regen::features::surfaces::offset_surface)?;
    Ok(Value::Undefined)
}

/// opSplitFace: Split's face mode with every tool field (edges, faces,
/// bodies, planes). A projection `direction` vector is not supported yet
/// (normal-to-target projection is used).
fn op_split_face(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let (cx, id, def) = args(i, &a)?;
    let targets = evaluate_refs(i, &cx, def.get("faceTargets"))?;
    let mut tools = Vec::new();
    for key in ["edgeTools", "faceTools", "bodyTools", "planeTools"] {
        tools.extend(evaluate_refs(i, &cx, def.get(key))?);
    }
    let mut p = ParamValues::default();
    p.set("splitType", ParamValue::Enum("FACE".into()));
    p.set("faceTargets", ParamValue::Query(targets));
    p.set("faceTools", ParamValue::Query(tools));
    p.set("projectionType", ParamValue::Enum("NORMAL_TO_TARGET".into()));
    p.set("keepToolSurfaces", ParamValue::Bool(def_bool(&def, "keepToolSurfaces", false)));
    run_op_with(i, &cx, &id, "CANNOT_SPLIT_FACE", &p, crate::regen::features::split::split_part)?;
    Ok(Value::Undefined)
}

/// opSplitEdges: split edges at parameters (one array per edge, 0..1, by
/// arc length when asked) or where a surface / plane crosses them.
fn op_split_edges(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let (cx, id, def) = args(i, &a)?;
    let edges = evaluate_refs(i, &cx, def.get("edges"))?;
    let surface_refs = match plane_of(def.get("splittingSurface")) {
        Some(_) => Vec::new(),
        None => evaluate_refs(i, &cx, def.get("splittingSurface"))?,
    };
    let plane = plane_of(def.get("splittingSurface"));
    let params: Vec<Vec<f64>> = def.get("parameters").as_array().map(|a| a.iter().map(|p| p.as_array().map(|x| x.iter().filter_map(|v| v.as_number()).collect()).unwrap_or_default()).collect()).unwrap_or_default();
    let arc = def_bool(&def, "arcLengthParameterization", true);
    run_op(i, &cx, &id, "REGEN_ERROR", |ev| {
        let mut tools: Vec<Shape> = Vec::new();
        if let Some(p) = &plane {
            tools.push(k::make_plane_face(&kernel_plane(p), -1e3, 1e3, -1e3, 1e3)?);
        }
        tools.extend(ev.resolve_faces(&surface_refs).iter().filter_map(|(b, f)| ev.face_shape(*b, *f).ok()));
        let found = ev.resolve_edges(&edges);
        let mut per_body: BTreeMap<usize, Vec<Shape>> = BTreeMap::new();
        for (k_, (bi, ei)) in found.iter().enumerate() {
            let body = &ev.ctx.bodies[*bi];
            let edge = body.shape.sub_shape(ShapeType::Edge, *ei)?;
            let info = edge.edge_info()?;
            for t in params.get(k_).into_iter().flatten() {
                let u = super::evaluate::edge_param(&edge, &info, *t, arc);
                per_body.entry(*bi).or_default().push(k::make_vertex(edge.point_at(u)?)?);
            }
            if !tools.is_empty() {
                per_body.entry(*bi).or_default().extend(tools.iter().cloned());
            }
        }
        if per_body.is_empty() {
            return err("Select edges to split.");
        }
        for (bi, cutters) in per_body.into_iter().rev() {
            let body = ev.ctx.bodies[bi].clone();
            let res = k::split(std::slice::from_ref(&body.shape), &cutters, &Default::default())?;
            let names = body.shape_names();
            let faces = ev.name_result(&[&names], &res.history, &res.shape, &BTreeMap::new(), None);
            ev.replace_body(bi, res.shape, faces);
        }
        Ok(())
    })?;
    Ok(Value::Undefined)
}

/// opExtendSheetBody: Move boundary's evaluator (blind extension).
fn op_extend_sheet_body(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let (cx, id, def) = args(i, &a)?;
    let entities = evaluate_refs(i, &cx, def.get("entities"))?;
    let blind = def_enum(&def, "endCondition").is_none_or(|e| &*e == "EXTEND_BLIND");
    let dist = def_length_opt(&def, "extendDistance").unwrap_or(0.0);
    let mut p = ParamValues::default();
    p.set("entities", ParamValue::Query(entities));
    p.set("endCondition", ParamValue::Enum(if blind { "BLIND" } else { "UP_TO_BODY" }.into()));
    p.set("extendDistance", length_param(dist.abs()));
    p.set("oppositeDirection", ParamValue::Bool(dist < 0.0));
    p.set("tangentPropagation", ParamValue::Bool(def_bool(&def, "tangentPropagation", true)));
    p.set("maintainCurvature", ParamValue::Bool(def_enum(&def, "extensionShape").is_some_and(|e| &*e == "SOFT")));
    run_op_with(i, &cx, &id, "REGEN_ERROR", &p, crate::regen::features::surfaces::extend_surface)?;
    Ok(Value::Undefined)
}

/// opFillSurface: Fill's evaluator, one boundary item per continuity.
fn op_fill_surface(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let (cx, id, def) = args(i, &a)?;
    let mut items = Vec::new();
    for (key, continuity) in [("edgesG0", "G0"), ("edgesG1", "G1"), ("edgesG2", "G2")] {
        let edges = evaluate_refs(i, &cx, def.get(key))?;
        if edges.is_empty() {
            continue;
        }
        let mut item = ParamValues::default();
        item.set("entities", ParamValue::Query(edges));
        item.set("continuity", ParamValue::Enum(continuity.into()));
        items.push(item);
    }
    let guides = evaluate_refs(i, &cx, def.get("guideVertices"))?;
    let mut p = ParamValues::default();
    p.set("surfaceOperationType", ParamValue::Enum("NEW".into()));
    p.set("edges", ParamValue::Array(items));
    p.set("addGuides", ParamValue::Bool(!guides.is_empty()));
    p.set("guideEntities", ParamValue::Query(guides));
    p.set("constraintMode", ParamValue::Enum("PRECISE".into()));
    run_op_with(i, &cx, &id, "REGEN_ERROR", &p, crate::regen::features::surfaces::fill)?;
    Ok(Value::Undefined)
}

/// opBoundarySurface: Boundary surface's evaluator, one profile item per
/// subquery. `uDerivativeInfo` / `vDerivativeInfo` conditions are not
/// applied yet (profiles get the default condition).
fn op_boundary_surface(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let (cx, id, def) = args(i, &a)?;
    let mut p = ParamValues::default();
    p.set("surfaceOperationType", ParamValue::Enum("NEW".into()));
    for uv in ["u", "v"] {
        let mut items = Vec::new();
        for q in def.get(&format!("{uv}ProfileSubqueries")).as_array().map(|a| a.iter().cloned().collect::<Vec<_>>()).unwrap_or_default() {
            let mut item = ParamValues::default();
            item.set(&format!("{uv}ProfileEntities"), ParamValue::Query(evaluate_refs(i, &cx, &q)?));
            item.set(&format!("{uv}Condition"), ParamValue::Enum("DEFAULT".into()));
            items.push(item);
        }
        p.set(&format!("{uv}ProfilesArray"), ParamValue::Array(items));
    }
    run_op_with(i, &cx, &id, "BSURF_FAILED", &p, crate::regen::features::surfaces::boundary_surface)?;
    Ok(Value::Undefined)
}

/// opIntersectFaces: Intersection curve's evaluator (tools vs targets).
fn op_intersect_faces(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let (cx, id, def) = args(i, &a)?;
    let mut p = ParamValues::default();
    p.set("group1", ParamValue::Query(evaluate_refs(i, &cx, def.get("tools"))?));
    p.set("group2", ParamValue::Query(evaluate_refs(i, &cx, def.get("targets"))?));
    run_op_with(i, &cx, &id, "REGEN_ERROR", &p, crate::regen::features::curves::intersection_curve)?;
    Ok(Value::Undefined)
}

/// opFlipOrientation: reverse sheet bodies.
fn op_flip_orientation(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let (cx, id, def) = args(i, &a)?;
    let bodies = evaluate_refs(i, &cx, def.get("bodies"))?;
    run_op(i, &cx, &id, "REGEN_ERROR", |ev| {
        for bi in ev.resolve_bodies(&bodies) {
            let b = ev.ctx.bodies[bi].clone();
            if b.kind != BodyKind::Surface {
                continue;
            }
            ev.replace_body(bi, b.shape.reversed()?, b.faces.clone());
        }
        Ok(())
    })?;
    Ok(Value::Undefined)
}

/// A length as a parameter expression (meters).
fn length_param(x: f64) -> ParamValue {
    ParamValue::Expr(noshape_units::ExprValue::new(format!("{x} m")))
}

/// opOffsetFace: Move face's Offset (OCCT offset, booleans where it fails).
fn op_offset_face(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let (cx, id, def) = args(i, &a)?;
    let faces = evaluate_refs(i, &cx, def.get("moveFaces"))?;
    let dist = def_length(i, &def, "offsetDistance")?;
    let mut p = ParamValues::default();
    p.set("moveFaceType", ParamValue::Enum("OFFSET".into()));
    p.set("moveFaces", ParamValue::Query(faces));
    p.set("offsetDistance", length_param(dist));
    run_op_with(i, &cx, &id, "DIRECT_EDIT_OFFSET_FACE_FAILED", &p, |ev| {
        crate::regen::features::direct::move_face(ev)?;
        keep_modified_names(ev);
        Ok(())
    })?;
    Ok(Value::Undefined)
}

/// A direct edit modifies the part's faces, it doesn't create them (Onshape:
/// qCreatedBy(part feature) still finds them). OCCT's offset history reports
/// every face as generated, so the evaluator names them `Offset(parent)` under
/// the operation; give them their parents' names back.
fn keep_modified_names(ev: &mut EvalCtx) {
    let marker = ev.feature;
    let parent = |f: &Name| match &**f {
        TopoName::Face { feature, role: Role::Offset(p) } if *feature == marker => Some(p.clone()),
        _ => None,
    };
    for bi in (0..ev.ctx.bodies.len()).rev() {
        let b = ev.ctx.bodies[bi].clone();
        if !b.faces.iter().any(|f| parent(f).is_some()) {
            continue;
        }
        let faces: Vec<Name> = b.faces.iter().map(|f| parent(f).unwrap_or_else(|| f.clone())).collect();
        ev.replace_body(bi, b.shape.clone(), faces);
    }
}

/// opReplaceFace: Replace face's evaluator.
fn op_replace_face(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let (cx, id, def) = args(i, &a)?;
    let faces = evaluate_refs(i, &cx, def.get("replaceFaces"))?;
    let template = evaluate_refs(i, &cx, def.get("templateFace"))?;
    let mut p = ParamValues::default();
    p.set("replaceFaces", ParamValue::Query(faces));
    p.set("templateFace", ParamValue::Query(template));
    p.set("offset", length_param(def_length_opt(&def, "offset").unwrap_or(0.0)));
    run_op_with(i, &cx, &id, "DIRECT_EDIT_REPLACE_FACE_FAILED", &p, crate::regen::features::direct::replace_face)?;
    Ok(Value::Undefined)
}

/// opMoveFace: the faces' region moved rigidly by the transform.
fn op_move_face(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let (cx, id, def) = args(i, &a)?;
    let faces = evaluate_refs(i, &cx, def.get("moveFaces"))?;
    let Some(m) = transform_of(def.get("transform")) else {
        return Err(i.error("transform should be a Transform"));
    };
    run_op(i, &cx, &id, "DIRECT_EDIT_MOVE_FACE_FAILED", |ev| {
        let groups = by_body(ev.resolve_faces(&faces));
        if groups.is_empty() {
            return err("Select faces to move.");
        }
        for (bi, fs) in groups.into_iter().rev() {
            crate::regen::features::direct::move_rigid(ev, bi, &fs, &m)?;
        }
        Ok(())
    })?;
    Ok(Value::Undefined)
}

/// Bring the bodies of another context into this one.
/// `opMergeContexts`: the bodies of `contextFrom` join this context; the
/// result is, for each query of `trackThroughMerge` (evaluated in
/// `contextFrom`), the transient ids of its entities after the merge.
fn op_merge_contexts(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let (cx, id, def) = args(i, &a)?;
    let from = def.get("contextFrom").clone();
    let Some(other) = from.builtin::<FsContext>() else {
        return Err(raise_error(i, "REGEN_ERROR", None));
    };
    let mut tracked: Vec<Vec<Reference>> = Vec::new();
    for q in def.get("trackThroughMerge").as_array().iter().flat_map(|v| v.iter()) {
        tracked.push(evaluate_refs(i, other, q)?);
    }
    let source = other.state.lock().ctx.clone();
    let (marker, moved) = run_op(i, &cx, &id, "REGEN_ERROR", |ev| {
        let mut moved: Vec<(BodyId, u32)> = Vec::new();
        for b in &source.bodies {
            let index = ev.ctx.bodies.len() as u32;
            moved.push((b.id.clone(), index));
            push_body(ev, BodyId { feature: ev.feature, index }, b.kind, b.shape.clone(), b.faces.clone());
        }
        ev.ctx.sketches.extend(source.sketches.iter().cloned());
        Ok((ev.feature, moved))
    })?;
    let mut st = cx.state.lock();
    let feature = st.feature;
    let out: Vec<Value> = tracked
        .into_iter()
        .map(|refs| {
            let ids = refs
                .into_iter()
                .map(|r| {
                    let r = match &r {
                        Reference::Body(old) => match moved.iter().find(|(o, _)| o == old) {
                            Some((_, index)) => Reference::Body(super::wrap_body_id(&BodyId { feature: marker, index: *index }, marker, feature)),
                            None => r,
                        },
                        _ => r,
                    };
                    Value::str(st.transient(&r))
                })
                .collect();
            Value::array(ids)
        })
        .collect();
    Ok(Value::array(out))
}

/// `opConstrainedSurface`: a surface through `points` (each `{ point,
/// normal? }`), within `tolerance` (default 1e-6 m), smoothed on request.
fn op_constrained_surface(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let (cx, id, def) = args(i, &a)?;
    let mut points: Vec<DVec3> = Vec::new();
    let mut normals: Vec<(usize, DVec3)> = Vec::new();
    for item in def.get("points").as_array().iter().flat_map(|v| v.iter()) {
        let Some(p) = vec3_of(item.get("point")) else { continue };
        points.push(p);
        if let Some(n) = vec3_of(item.get("normal")).filter(|n| n.length() > 0.) {
            normals.push((points.len() - 1, n.normalize()));
        }
    }
    let tolerance = def_length_opt(&def, "tolerance").unwrap_or(1e-6).max(1e-9);
    let smooth = def_bool(&def, "smooth", false);
    run_op(i, &cx, &id, "REGEN_ERROR", |ev| {
        if points.len() < 3 {
            return err("Select at least three points (vertices) to fit a surface");
        }
        let opts = k::SurfaceFitOptions { tolerance, interpolate: true, smooth };
        let fit = k::fit_surface(&points, &normals, &opts).map_err(|e| FeatureError::new(format!("The surface could not be fitted: {e}")))?;
        let name: Name = Arc::new(TopoName::face(ev.feature, Role::New(0)));
        ev.apply_operation(vec![NamedShape::new(fit.face, vec![name])], "NEW", None, BodyKind::Surface)
    })?;
    Ok(Value::Undefined)
}

/// A std `WrapSurface`: `face`, `plane`, `cylinder` or `cone`, with
/// `anchorPoint` and `anchorDirection`.
struct WrapSurface {
    face: Vec<Reference>,
    surface: Option<k::Developable>,
    anchor: DVec3,
    direction: DVec3,
}

fn wrap_surface(i: &mut Interp, cx: &FsContext, v: &Value) -> R<WrapSurface> {
    let anchor = vec3_of(v.get("anchorPoint")).unwrap_or(DVec3::ZERO);
    let direction = vec3_of(v.get("anchorDirection")).unwrap_or(DVec3::X);
    let mut out = WrapSurface { face: Vec::new(), surface: None, anchor, direction };
    if !v.get("face").is_undefined() {
        out.face = evaluate_refs(i, cx, v.get("face"))?;
    } else if let Some(p) = plane_of(v.get("plane")) {
        out.surface = Some(k::Developable::Plane { origin: p.origin, x_dir: p.x, normal: p.normal });
    } else if let Some((origin, x, z)) = coord_system_of(v.get("cylinder").get("coordSystem")) {
        let radius = length_of(v.get("cylinder").get("radius")).unwrap_or(0.0);
        out.surface = Some(k::Developable::Cylinder { origin, axis: z, x_dir: x, radius });
    } else if let Some((origin, x, z)) = coord_system_of(v.get("cone").get("coordSystem")) {
        let half_angle = angle_of(v.get("cone").get("halfAngle")).unwrap_or(0.0);
        out.surface = Some(k::Developable::Cone { origin, axis: z, x_dir: x, radius: 0.0, half_angle });
    }
    Ok(out)
}

/// Unit flat-space direction of a 3D direction at flat point `f`.
fn wrap_flat_dir(d: &k::Developable, f: glam::DVec2, ref_angle: f64, v: DVec3) -> glam::DVec2 {
    let (ax, ay, _) = d.flat_axes(f, ref_angle);
    glam::DVec2::new(v.dot(ax), v.dot(ay)).normalize_or_zero()
}

/// `opWrap`: `entities` (faces on `source`) wrapped or unwrapped onto
/// `destination`, the anchor points and directions matched; `wrapType`
/// SIMPLE (new surface), TRIM (trimmed to the destination face) or IMPRINT
/// (edges imprinted on it).
fn op_wrap(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let (cx, id, def) = args(i, &a)?;
    let entities = evaluate_refs(i, &cx, def.get("entities"))?;
    let src = wrap_surface(i, &cx, def.get("source"))?;
    let dst = wrap_surface(i, &cx, def.get("destination"))?;
    let wrap_type = def_enum(&def, "wrapType").map(|s| s.to_string()).unwrap_or_else(|| "SIMPLE".into());
    run_op(i, &cx, &id, "REGEN_ERROR", |ev| {
        let faces: Vec<Shape> = ev.resolve_faces(&entities).into_iter().map(|(b, f)| ev.face_shape(b, f)).collect::<FResult<_>>()?;
        if faces.is_empty() {
            return err("Select faces to wrap");
        }
        let developable = |w: &WrapSurface, ev: &EvalCtx| -> FResult<(k::Developable, Option<(usize, usize)>)> {
            if let Some(d) = w.surface {
                return Ok((d, None));
            }
            let (bi, fi) = *ev.resolve_faces(&w.face).first().ok_or_else(|| FeatureError::new("The wrap surface no longer exists"))?;
            let d = k::Developable::of_face(&ev.face_shape(bi, fi)?).map_err(|_| FeatureError::new("The wrap surfaces must be planes, cylinders or cones"))?;
            Ok((d, Some((bi, fi))))
        };
        let (s, _) = developable(&src, ev)?;
        let (d, dst_face) = developable(&dst, ev)?;
        let unwrap = !matches!(s, k::Developable::Plane { .. });
        let src_ref = if unwrap { s.angle_of(src.anchor, 0.0) } else { 0.0 };
        let dst_ref = if unwrap { 0.0 } else { d.angle_of(dst.anchor, 0.0) };
        let f_src = if unwrap { s.project(src.anchor, src_ref) } else { s.flat(src.anchor, src_ref) };
        let f_dst = d.project(dst.anchor, dst_ref);
        let d_src = wrap_flat_dir(&s, f_src, src_ref, src.direction);
        let d_dst = wrap_flat_dir(&d, f_dst, dst_ref, dst.direction);
        if d_src.length() < 0.5 || d_dst.length() < 0.5 {
            return err("The wrap anchor directions are invalid");
        }
        let placement = k::Placement2d::align(f_src, d_src, f_dst, d_dst, false);
        let res = k::wrap_faces(&faces, &s, src_ref, &d, dst_ref, &placement, 1e-7).map_err(|e| FeatureError::new(format!("Wrap failed: {e}")))?;
        let names: Vec<Name> = (0..res.shape.faces().len()).map(|n| Arc::new(TopoName::face(ev.feature, Role::New(n as u32)))).collect();
        let mut wrapped = NamedShape::new(res.shape.clone(), names);
        match wrap_type.as_str() {
            "IMPRINT" => {
                let Some((bi, _)) = dst_face else { return err("IMPRINT needs a destination face") };
                let body = ev.ctx.bodies[bi].clone();
                let edges: Vec<Shape> = res.shape.faces().iter().flat_map(|f| f.edges()).collect();
                let imp = k::imprint(&body.shape, &edges).map_err(|e| FeatureError::new(format!("The wrapped edges could not split the destination: {e}")))?;
                let names = ev.name_result(&[&body.shape_names(), &ShapeNames::default()], &imp.history, &imp.shape, &BTreeMap::new(), None);
                ev.replace_body(bi, imp.shape, names);
                Ok(())
            }
            t => {
                if t == "TRIM"
                    && let Some((bi, fi)) = dst_face
                {
                    let target = ev.face_shape(bi, fi)?;
                    if let Ok(c) = k::common(&wrapped.shape, &target)
                        && c.shape.count(ShapeType::Face) > 0
                    {
                        let names = ev.name_result(&[&wrapped.names, &ShapeNames::default()], &c.history, &c.shape, &BTreeMap::new(), None);
                        wrapped = NamedShape::new(c.shape, names);
                    }
                }
                ev.apply_operation(vec![wrapped], "NEW", None, BodyKind::Surface)
            }
        }
    })?;
    Ok(Value::Undefined)
}
