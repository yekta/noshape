//! `@op*` natives: modeling operations. Each runs through [`run_op`] (the
//! operation's marker feature id names what it creates) and reuses the
//! evaluator helpers of the built-in features.

use super::query::{Ent, evaluate_refs, with_ents};
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
    ("opEditCurve", op_edit_curve),
    ("opModifyCompositePart", op_modify_composite_part),
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
    ("opOffsetWire", op_offset_wire),
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
pub(super) fn fresh_names(ev: &EvalCtx, shape: &Shape) -> Vec<Name> {
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
    cx.state.lock().dependencies.insert(id.clone(), refs.clone());
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
        // Edges extrude to sheets; connected curves give one sheet (live: an
        // open polyline extrudes to one sheet body, dom/fs-smprobe-3.txt).
        let mut sheets = Vec::new();
        for (n, edge) in edge_shapes(ev, &refs)?.into_iter().enumerate() {
            let (s, e) = (start.unwrap_or(0.0), end.unwrap_or(0.0));
            let mut res = k::extrude_between(&edge, -d * s, d * e, 0.0).map_err(|e| fe(format!("Extrude failed: {e}")))?;
            // Onshape's sheet faces the side `tangent × direction` (live sm3:
            // (0,0)→(30,0) extruded +z faces −y).
            if let Ok(info) = edge.edge_info() {
                let t_mid = (info.first_param + info.last_param) * 0.5;
                if let (Ok(p), Ok(t)) = (edge.point_at(t_mid), edge.tangent_at(t_mid))
                    && let Some(face) = res.shape.faces().first()
                    && face.normal_at_point(p).is_ok_and(|nf| nf.dot(t.cross(d)) < 0.0)
                    && let Ok(r) = res.shape.reversed()
                {
                    res.shape = r;
                }
            }
            let mut roles = BTreeMap::new();
            for f in 0..res.shape.count(ShapeType::Face) {
                roles.insert(f as u32, Role::Side(Source::Index(n as u32 * 64 + f as u32)));
            }
            let names = ev.name_result(&[&ShapeNames::default()], &res.history, &res.shape, &roles, None);
            sheets.push(NamedShape::new(res.shape, names));
        }
        tools.extend(sew_sheets(sheets)?);
        if tools.is_empty() {
            return err("Could not use selected entities to extrude");
        }
        let kind = if tools.iter().all(|t| t.shape.solids().is_empty()) { BodyKind::Surface } else { BodyKind::Solid };
        add_bodies(ev, tools, kind)
    })?;
    Ok(Value::Undefined)
}

/// A united sheet's faces sewn into shells and coplanar neighbours merged:
/// the fuse leaves loose faces, which the same-domain unify does not join
/// (std Tab: the tab and its wall become one 850 mm² face, live
/// dom/fs-smprobe3-8.txt).
/// A sheet union whose pieces only touch along (part of) an edge, as std's
/// sheet metal merges are (a hem's arc on a plate edge, tangent): Parasolid
/// stitches them there (std passes the edge `matches`). Sew the union's
/// pieces (sewing cuts partly shared edges); one connected sheet replaces
/// them, its faces keeping their names.
fn stitch_sheet_pieces(ev: &mut EvalCtx, first: &BodyId) -> FResult<()> {
    let pieces: Vec<usize> = (0..ev.ctx.bodies.len()).filter(|&i| {
        let b = &ev.ctx.bodies[i];
        b.kind == BodyKind::Surface && (&b.id == first || (b.id.feature == ev.feature && b.id.index >= 10_000))
    }).collect();
    if pieces.len() < 2 || !pieces.iter().any(|&i| &ev.ctx.bodies[i].id == first) {
        return Ok(());
    }
    let mut faces = Vec::new();
    let mut names = Vec::new();
    for &i in &pieces {
        let b = &ev.ctx.bodies[i];
        faces.extend(b.shape.faces());
        names.extend(b.faces.iter().cloned());
    }
    let Ok(sewn) = k::sew(&faces, 1e-7, false) else { return Ok(()) };
    if sewn.shape.shells().len() != 1 || sewn.shape.count(ShapeType::Face) != faces.len() {
        return Ok(());
    }
    let key = |f: &Shape| f.bounding_box(false).map(|b| (b.center(), b.diagonal())).ok();
    let keys: Vec<_> = faces.iter().map(key).collect();
    let Some(sewn_names) = sewn
        .shape
        .faces()
        .iter()
        .map(|f| {
            let kf = key(f)?;
            let i = keys.iter().position(|k| k.is_some_and(|k| k.0.distance(kf.0) < 1e-9 && (k.1 - kf.1).abs() < 1e-9))?;
            names.get(i).cloned()
        })
        .collect::<Option<Vec<Name>>>()
    else {
        return Ok(());
    };
    let keep = pieces.iter().copied().find(|&i| &ev.ctx.bodies[i].id == first).unwrap_or(pieces[0]);
    ev.replace_body(keep, sewn.shape, sewn_names);
    let drop: Vec<BodyId> = pieces.iter().filter(|&&i| i != keep).map(|&i| ev.ctx.bodies[i].id.clone()).collect();
    ev.ctx.bodies.retain(|b| !drop.contains(&b.id));
    Ok(())
}

fn merge_sheet_faces(ev: &mut EvalCtx, bi: usize) -> FResult<()> {
    let body = ev.ctx.bodies[bi].clone();
    if body.kind != BodyKind::Surface {
        return Ok(());
    }
    let faces = body.shape.faces();
    if faces.len() < 2 {
        return Ok(());
    }
    let Ok(sewn) = k::sew(&faces, 1e-7, false) else { return Ok(()) };
    let key = |f: &Shape| f.bounding_box(false).map(|b| (b.center(), b.diagonal())).ok();
    let keys: Vec<_> = faces.iter().map(key).collect();
    let names: Option<Vec<Name>> = sewn
        .shape
        .faces()
        .iter()
        .map(|f| {
            let kf = key(f)?;
            let i = keys.iter().position(|k| k.is_some_and(|k| k.0.distance(kf.0) < 1e-9 && (k.1 - kf.1).abs() < 1e-9))?;
            body.faces.get(i).cloned()
        })
        .collect();
    let Some(names) = names else { return Ok(()) };
    let unified = k::unify_same_domain(&sewn.shape)?;
    if unified.shape.count(ShapeType::Face) >= sewn.shape.count(ShapeType::Face) {
        return Ok(());
    }
    let before = crate::regen::eval::names_for(&sewn.shape, names);
    let merged = ev.name_result(&[&before], &unified.history, &unified.shape, &BTreeMap::new(), None);
    ev.replace_body(bi, unified.shape, merged);
    Ok(())
}

/// Sheets sewn along their shared edges (each connected set one shell);
/// faces keep their names.
fn sew_sheets(sheets: Vec<NamedShape>) -> FResult<Vec<NamedShape>> {
    if sheets.len() < 2 {
        return Ok(sheets);
    }
    let shapes: Vec<Shape> = sheets.iter().map(|s| s.shape.clone()).collect();
    let Ok(sewn) = k::sew(&shapes, 1e-7, false) else { return Ok(sheets) };
    let key = |f: &Shape| f.bounding_box(false).map(|b| (b.center(), b.diagonal())).ok();
    let named: Vec<(Option<(DVec3, f64)>, Name)> = sheets.iter().flat_map(|s| s.shape.faces().iter().map(key).zip(s.names.faces.iter().cloned()).collect::<Vec<_>>()).collect();
    let mut out = Vec::new();
    let shells = sewn.shape.shells();
    let pieces = if shells.is_empty() { vec![sewn.shape.clone()] } else { shells };
    for p in pieces {
        let names: Option<Vec<Name>> = p
            .faces()
            .iter()
            .map(|f| {
                let kf = key(f)?;
                named.iter().find(|(k, _)| k.is_some_and(|k| k.0.distance(kf.0) < 1e-9 && (k.1 - kf.1).abs() < 1e-9)).map(|(_, n)| n.clone())
            })
            .collect();
        match names {
            Some(names) => out.push(NamedShape::new(p, names)),
            None => return Ok(sheets),
        }
    }
    Ok(out)
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
    // std revolve.fs passes blind bounds as `endBoundAngle` and
    // `startBoundAngle`, the start's position in [0, 2π) (symmetric: 2π - a/2).
    let (fwd, back) = match angle_of(def.get("endBoundAngle")) {
        Some(end) if angle_of(def.get("angleForward")).is_none() => {
            let start = angle_of(def.get("startBoundAngle")).unwrap_or(0.0).rem_euclid(std::f64::consts::TAU);
            (end, if start > 0.0 { std::f64::consts::TAU - start } else { 0.0 })
        }
        _ => (angle_of(def.get("angleForward")).unwrap_or(std::f64::consts::TAU), angle_of(def.get("angleBack")).unwrap_or(0.0)),
    };
    cx.state.lock().dependencies.insert(id.clone(), refs.clone());
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
    // false: a sheet union keeps the edges imprinted on its faces (std Jog
    // splits its faces up front, then unions the stretched pieces).
    let erase = def_bool(&def, "eraseImprintedEdges", true);
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
                if erase {
                    ev.boolean_into(&[first], others, k::BooleanOp::Fuse)?;
                } else {
                    // Imprinted edges kept: a plain fuse, no face merging.
                    let target = body_named(ev, first);
                    let shapes: Vec<Shape> = others.iter().map(|o| o.shape.clone()).collect();
                    let res = k::boolean(k::BooleanOp::Fuse, std::slice::from_ref(&target.shape), &shapes, &k::BooleanOptions::default())?;
                    let mut args: Vec<&crate::naming::ShapeNames> = vec![&target.names];
                    args.extend(others.iter().map(|o| &o.names));
                    let names = ev.name_result(&args, &res.history, &res.shape, &BTreeMap::new(), None);
                    ev.replace_body(first, res.shape, names);
                }
                stitch_sheet_pieces(ev, &all[0])?;
                if erase && let Some(bi) = index(ev, &all[0]) {
                    merge_sheet_faces(ev, bi)?;
                }
                Ok(Some(all))
            }
            "INTERSECTION" => {
                // Tools only: their common volume, kept in the first tool
                // (live dom/fs-stdref.txt: a box target and one sphere tool
                // give BOOLEAN_BAD_INPUT, as the feature's status too).
                if tools.len() < 2 {
                    return Err(super::code_error("BOOLEAN_BAD_INPUT"));
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
                // The same blends as the Fillet and Chamfer features (with their fallbacks).
                (false, k::ChamferSpec::Equal(r)) => k::fillet_checked(&body.shape, &shapes, &[r]),
                (_, s) => k::chamfer_checked(&body.shape, &shapes, s, &[]),
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
    let made = run_op(i, &cx, &id, "PATTERN_BODY_FAILED", |ev| {
        let bodies = ev.resolve_bodies(&refs);
        if bodies.is_empty() {
            return err("Could not pattern selected parts.");
        }
        let mut n = 0u32;
        let mut made: Vec<(String, BodyId, BodyId)> = Vec::new();
        for bi in bodies {
            let body = ev.ctx.bodies[bi].clone();
            for (k_, m) in transforms.iter().enumerate() {
                let res = k::transform(&body.shape, &k::Transform::Matrix(*m), true)?;
                let inst = instance_number(names.get(k_).map(|s| s.as_str()).unwrap_or(""), k_);
                let faces = copy_names(ev, &body.faces, &res, inst);
                push_body(ev, BodyId { feature: ev.feature, index: n }, body.kind, res.shape, faces);
                made.push((names.get(k_).cloned().unwrap_or_default(), body.id.clone(), BodyId { feature: ev.feature, index: n }));
                n += 1;
            }
        }
        Ok(made)
    })?;
    // Instance bodies as renamed after the operation.
    let mut st = cx.state.lock();
    let (marker, feature) = (super::marker_of(&id), st.feature);
    for (name, source, b) in made {
        let inst = super::wrap_body_id(&b, marker, feature);
        // A copy keeps its body's attributes (std Form's tool instances keep the Tag's form attributes).
        let copied: Vec<Value> = st.attributes.iter().filter(|(r, _)| *r == Reference::Body(source.clone())).map(|(_, v)| v.clone()).collect();
        for v in copied {
            st.push_attribute(Reference::Body(inst.clone()), v);
        }
        st.pattern_instances.entry((id.clone(), name)).or_default().push(inst);
    }
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
        // Through two points, nothing else: a line (std Jog reads its fitted
        // bend reference with evLine).
        if pts.len() == 2 && tangents.is_empty() {
            return add_wire(ev, k::make_line(pts[0], pts[1])?);
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
    let (split_targets, plane) = run_op(i, &cx, &id, "SPLIT_FAILED", |ev| {
        let mut tools: Vec<Shape> = Vec::new();
        let mut plane = None;
        for r in &tool_refs {
            if let Some(p) = matches!(r, Reference::Plane(_) | Reference::FeaturePlane(_)).then(|| ev.resolve_plane(r)).flatten() {
                plane.get_or_insert((p.origin, p.normal));
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
        Ok((target_ids, plane))
    })?;
    // Split pieces are "created by" the split and by the original's creator
    // (std query.fs, qCreatedBy).
    let mut st = cx.state.lock();
    let pieces: Vec<BodyId> = st.ctx.bodies.iter().map(|b| b.id.clone()).filter(|b| !split_targets.contains(b) && st.body_creator(b).as_deref() == Some(&id[..])).collect();
    for piece in &pieces {
        st.record_merge(piece, &split_targets, &id);
    }
    let all = split_targets.iter().chain(&pieces).cloned().collect();
    st.splits.insert(id.clone(), (all, plane));
    Ok(Value::Undefined)
}

fn op_draft(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let (cx, id, def) = args(i, &a)?;
    let faces = evaluate_refs(i, &cx, def.get("draftFaces"))?;
    let neutral = evaluate_refs(i, &cx, def.get("neutralPlane"))?;
    let angle = def_angle(i, &def, "angle")?;
    // std passes `pullVec` (extrude's draft flips it for the second side).
    let pull = vec3_of(def.get("pullVec")).or_else(|| vec3_of(def.get("pullDirection")));
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
    let leave_open = def_bool(&def, "leaveOpen", false);
    run_op(i, &cx, &id, "DIRECT_EDIT_DELETE_FACE_FAILED", |ev| {
        let mut fresh = 0u32;
        for (bi, fs) in by_body(ev.resolve_faces(&faces)).into_iter().rev() {
            let body = ev.ctx.bodies[bi].clone();
            if leave_open && body.kind == BodyKind::Surface {
                // Just remove the faces; each connected rest is its own sheet
                // (std sheet metal Bend: the fixed and the moving side).
                let keep: Vec<(Shape, Name)> = body.shape.faces().into_iter().enumerate().filter(|(j, _)| !fs.contains(j)).filter_map(|(j, f)| Some((f, body.faces.get(j)?.clone()))).collect();
                if keep.is_empty() {
                    ev.ctx.bodies.remove(bi);
                    continue;
                }
                let shapes: Vec<Shape> = keep.iter().map(|k| k.0.clone()).collect();
                let sewn = if shapes.len() == 1 { shapes[0].clone() } else { k::sew(&shapes, 1e-7, false)?.shape };
                let key = |f: &Shape| f.bounding_box(false).map(|b| (b.center(), b.diagonal())).ok();
                let keys: Vec<_> = shapes.iter().map(key).collect();
                let name_of = |f: &Shape| {
                    let kf = key(f)?;
                    let j = keys.iter().position(|k| k.is_some_and(|k| k.0.distance(kf.0) < 1e-9 && (k.1 - kf.1).abs() < 1e-9))?;
                    Some(keep[j].1.clone())
                };
                let mut pieces: Vec<Shape> = sewn.shells();
                let shelled: Vec<Shape> = pieces.iter().flat_map(|s| s.faces()).collect();
                pieces.extend(sewn.faces().into_iter().filter(|f| !shelled.iter().any(|g| g.is_same(f))));
                for (n, piece) in pieces.into_iter().enumerate() {
                    let names: Vec<Name> = piece.faces().iter().map(|f| name_of(f).unwrap_or_else(|| Arc::new(TopoName::face(ev.feature, Role::New(0))))).collect();
                    if n == 0 {
                        ev.replace_body(bi, piece, names);
                    } else {
                        push_body(ev, BodyId { feature: ev.feature, index: fresh }, BodyKind::Surface, piece, names);
                        fresh += 1;
                    }
                }
                continue;
            }
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
    let faces = if def_bool(&def, "tangentPropagation", false) {
        // The faces tangent-connected to the selection (smooth edges) too.
        with_ents(i, &cx, def.get("faces"), |_, q, ents| {
            let mut set: Vec<(usize, usize)> = ents.iter().filter_map(|e| if let Ent::Face(b, f) = e { Some((*b, *f)) } else { None }).collect();
            let mut k = 0;
            while k < set.len() {
                let (b, f) = set[k];
                k += 1;
                let Ok(ef) = q.st.ctx.bodies[b].shape.edge_faces() else { continue };
                for (x, fs) in ef.iter().enumerate() {
                    if fs.len() != 2 || (fs[0] as usize != f && fs[1] as usize != f) {
                        continue;
                    }
                    let other = if fs[0] as usize == f { fs[1] as usize } else { fs[0] as usize };
                    if !set.contains(&(b, other)) && super::evaluate::convexity(q, &Ent::Edge(b, x)) == Some("SMOOTH") {
                        set.push((b, other));
                    }
                }
            }
            Ok(set.into_iter().map(|(b, f)| q.to_ref(&Ent::Face(b, f))).collect::<Vec<_>>())
        })?
    } else {
        evaluate_refs(i, &cx, def.get("faces"))?
    };
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
    p.set("extendToCompletion", ParamValue::Bool(def_bool(&def, "extendToCompletion", false)));
    let own_existing = def_bool(&def, "ownExistingImprints", false);
    if super::trace_ops() {
        eprintln!("[fs op] splitFace targets {:?} tools {:?}", p.query("faceTargets"), p.query("faceTools"));
    }
    let new_edges = run_op_with(i, &cx, &id, "CANNOT_SPLIT_FACE", &p, |ev| {
        // The target bodies' edges before the split.
        let owners: Vec<BodyId> = ev.resolve_faces(ev.query("faceTargets")).into_iter().map(|(b, _)| ev.ctx.bodies[b].id.clone()).collect();
        let before: Vec<(BodyId, Shape)> = ev.ctx.bodies.iter().filter(|b| owners.contains(&b.id)).map(|b| (b.id.clone(), Shape::compound(&b.shape.edges()))).collect();
        let targets: Vec<Shape> = ev.resolve_faces(ev.query("faceTargets")).into_iter().filter_map(|(b, f)| ev.face_shape(b, f).ok()).collect();
        let n_targets = targets.len();
        let targets = Shape::compound(&targets);
        match crate::regen::features::split::split_part(ev) {
            // Owning existing imprints (std Jog splits up front): nothing new
            // to split is fine; the edges already there are this split's.
            Err(e) if own_existing && e.message == "The tools don't split the selected faces" => {}
            r => r?,
        }
        // The pieces of the target faces (their snapshot points: uv middle, on the face).
        let mut pieces = Vec::new();
        for (bid, _) in &before {
            let Some(b) = ev.ctx.bodies.iter().find(|b| &b.id == bid) else { continue };
            for f in b.shape.faces() {
                let Ok([u0, u1, v0, v1]) = f.uv_bounds() else { continue };
                let Some(p) = f.point_at_uv((u0 + u1) * 0.5, (v0 + v1) * 0.5).ok().and_then(|p| f.project_point(p).ok()).map(|p| p.point) else { continue };
                if k::make_vertex(p).ok().and_then(|v| k::distance(&v, &targets).ok()).is_some_and(|d| d.distance < 1e-7) {
                    pieces.push(p);
                }
            }
        }
        // Nothing split: no pieces.
        if pieces.len() <= n_targets {
            pieces.clear();
        }
        // splittingEdges: the edges the split made (midpoints on no old edge).
        let mut made = Vec::new();
        let mut mids = Vec::new();
        for (bid, old) in &before {
            let Some(b) = ev.ctx.bodies.iter().find(|b| &b.id == bid) else { continue };
            for (e, edge) in b.shape.edges().iter().enumerate() {
                let Ok(info) = edge.edge_info() else { continue };
                let mid = edge.point_at((info.first_param + info.last_param) * 0.5).ok().and_then(|p| k::make_vertex(p).ok());
                if let Some(v) = mid.filter(|v| k::distance(v, old).is_ok_and(|d| d.distance > 1e-7)) {
                    made.push(Reference::Edge((*b.edges[e]).clone()));
                    mids.extend(v.vertex_point().ok());
                }
            }
        }
        // `ownExistingImprints` (std Jog): the targets' edges already on a
        // tool count as its cut too (the face ends where the tool lies).
        let mut existing = Vec::new();
        if own_existing {
            for (bid, _) in &before {
                let Some(b) = ev.ctx.bodies.iter().find(|b| &b.id == bid) else { continue };
                for edge in b.shape.edges() {
                    let (Ok(info), Ok((a, z))) = (edge.edge_info(), edge.endpoints()) else { continue };
                    if let Ok(m) = edge.point_at((info.first_param + info.last_param) * 0.5) {
                        existing.push((a, m, z));
                    }
                }
            }
        }
        // Which planar tool cut which of them.
        let mut tools = Vec::new();
        for (bi, fi) in ev.resolve_faces(ev.query("faceTools")) {
            let Ok(Some(pl)) = ev.face_shape(bi, fi).and_then(|f| f.face_plane().map_err(|e| crate::regen::eval::FeatureError::new(e.to_string()))) else { continue };
            let n = pl.unit_normal();
            let in_plane = |m: DVec3| (m - pl.origin).dot(n).abs() < 1e-7;
            let mut on: Vec<DVec3> = mids.iter().copied().filter(|m| in_plane(*m)).collect();
            on.extend(existing.iter().filter(|(a, m, z)| in_plane(*a) && in_plane(*m) && in_plane(*z)).map(|e| e.1));
            tools.push((pl.origin, n, on));
        }
        Ok((made, tools, pieces, owners, mids))
    })?;
    let (new_edges, tools, pieces, owners, mids) = new_edges;
    let mut st = cx.state.lock();
    // The edges it made are its (names lose the operation once wrapped):
    // std Rip finds its new edge by qCreatedBy(split, EDGE), as live.
    let made: Vec<crate::naming::Name> = st
        .ctx
        .bodies
        .iter()
        .filter(|b| owners.contains(&b.id))
        .flat_map(|b| {
            let names = b.edges.clone();
            b.shape.edges().into_iter().zip(names).filter_map(|(e, n)| {
                let info = e.edge_info().ok()?;
                let m = e.point_at((info.first_param + info.last_param) * 0.5).ok()?;
                mids.iter().any(|x| x.distance(m) < 1e-7).then_some(n)
            }).collect::<Vec<_>>()
        })
        .collect();
    for n in made {
        st.made_edges.insert(n, id.clone());
    }
    if own_existing {
        // Its edges derive from this split even where it changed nothing
        // (tracking keeps what operations since its start made or changed).
        let seq = st.next_seq;
        for b in owners {
            st.touched.insert(b, seq);
        }
    }
    st.split_tools.extend(tools);
    st.split_face_pieces.insert(id.clone(), pieces);
    drop(st);
    Ok(Value::map_from([("splittingEdges", super::query::transient_queries(&cx, &new_edges))]))
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
    let originals = run_op(i, &cx, &id, "REGEN_ERROR", |ev| {
        let mut originals = Vec::new();
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
            originals.push((body.id.clone(), edge.clone()));
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
        Ok(originals)
    })?;
    // `qSplitBy(id, EDGE)`: the pieces of these edges (std partial flange).
    cx.state.lock().split_edges.insert(id.clone(), originals);
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

/// opOffsetWire (@internal, the Thin option's wall regions): the Thin
/// wall builder with `offset1` / `offset2` as the wall thicknesses. Open
/// chains put `offset1` to the left of the first edge seen from `normal`;
/// closed loops put it outside. `makeRegions` keeps the planar wall regions
/// (sheet bodies), otherwise the offset curves become wire bodies.
fn op_offset_wire(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let (cx, id, def) = args(i, &a)?;
    let refs = evaluate_refs(i, &cx, def.get("edges"))?;
    let t1 = length_of(def.get("offset1")).unwrap_or(0.0);
    let t2 = length_of(def.get("offset2")).unwrap_or(0.0);
    let normal = vec3_of(def.get("normal"));
    let regions = def_bool(&def, "makeRegions", false);
    let mut p = ParamValues::default();
    p.set("thickness1", length_param(t1));
    p.set("thickness2", length_param(t2));
    p.set("flipWall", ParamValue::Bool(def_bool(&def, "flip", false)));
    if refs.is_empty() {
        return Err(raise_error(i, "OFFSET_WIRE_SELECT_WALL_PATH", None));
    }
    run_op_with(i, &cx, &id, "OFFSET_WIRE_GENERIC_ERROR", &p, |ev| {
        let mut profiles = crate::regen::features::thin::thin_profiles(ev, &refs)?;
        // The wall builder measures "left" of open chains around its own
        // plane normal (closed loops: walls with a hole keep "outside").
        let opposed = |p: &crate::regen::eval::Profile| p.shape.wires().len() == 1 && normal.is_some_and(|n| p.frame.normal.dot(n) < 0.0);
        if profiles.iter().any(opposed) {
            let mut flipped = p.clone();
            flipped.set("flipWall", ParamValue::Bool(!def_bool(&def, "flip", false)));
            let ev2 = EvalCtx::new(ev.feature, "", &flipped, ev.units, std::mem::take(&mut ev.ctx), ev.outer_vars);
            let r = crate::regen::features::thin::thin_profiles(&ev2, &refs);
            ev.ctx = ev2.ctx;
            let again = r?;
            profiles = profiles.into_iter().zip(again).map(|(a, b)| if opposed(&a) { b } else { a }).collect();
        }
        let source = Shape::compound(&edge_shapes(ev, &refs)?);
        let ends: Vec<DVec3> = source.vertices().iter().filter_map(|v| v.vertex_point().ok()).collect();
        for prof in profiles {
            if regions {
                let names = fresh_names(ev, &prof.shape);
                add_bodies(ev, vec![NamedShape::new(prof.shape, names)], BodyKind::Surface)?;
                continue;
            }
            // The offset curves: region edges off the selected curves.
            let offs: Vec<Shape> = prof
                .shape
                .edges()
                .into_iter()
                .filter(|e| {
                    let on_source = e.edge_info().ok().and_then(|inf| e.point_at((inf.first_param + inf.last_param) * 0.5).ok()).and_then(|m| k::make_vertex(m).ok()).and_then(|v| k::distance(&v, &source).ok()).is_some_and(|d| d.distance < 1e-9);
                    let cap = ends.iter().any(|p| k::make_vertex(*p).ok().and_then(|v| k::distance(&v, e).ok()).is_some_and(|d| d.distance < 1e-9));
                    !on_source && !cap
                })
                .collect();
            if !offs.is_empty() {
                add_wire(ev, Shape::compound(&offs))?;
            }
        }
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
        // A translation along planar faces' normals offsets them (their
        // neighbours follow, like the Move face feature); otherwise rigid.
        let t = m.w_axis.truncate();
        let pure = (m.x_axis.truncate() - DVec3::X).length() < 1e-12 && (m.y_axis.truncate() - DVec3::Y).length() < 1e-12 && (m.z_axis.truncate() - DVec3::Z).length() < 1e-12;
        let mut offset = false;
        for (bi, fs) in groups.into_iter().rev() {
            let shapes: Vec<Shape> = fs.iter().map(|&f| ev.face_shape(bi, f)).collect::<FResult<_>>()?;
            let each: Option<Vec<f64>> = (pure && t.length() > 1e-12)
                .then(|| shapes.iter().map(|s| s.face_plane().ok().flatten().map(|p| p.unit_normal()).filter(|n| (n.dot(t.normalize()).abs() - 1.0).abs() < 1e-6).map(|n| n.dot(t))).collect::<Option<Vec<f64>>>())
                .flatten();
            match each {
                Some(each) => {
                    let body = ev.ctx.bodies[bi].clone();
                    let res = k::offset_faces_each(&body.shape, &shapes, &each).map_err(|e| FeatureError::new(format!("Move face failed: {e}")))?;
                    let offset_role = |n: &Name| Role::Offset(n.clone());
                    let names = ev.name_result(&[&body.shape_names()], &res.history, &res.shape, &BTreeMap::new(), Some(&offset_role));
                    ev.replace_body(bi, res.shape, names);
                    offset = true;
                }
                None => crate::regen::features::direct::move_rigid(ev, bi, &fs, &m)?,
            }
        }
        if offset {
            keep_modified_names(ev);
        }
        Ok(())
    })?;
    Ok(Value::Undefined)
}

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
    let (source, source_attributes) = {
        let o = other.state.lock();
        (o.ctx.clone(), o.attributes.clone())
    };
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
    let merged = |r: Reference| match &r {
        Reference::Body(old) => match moved.iter().find(|(o, _)| o == old) {
            Some((_, index)) => Reference::Body(super::wrap_body_id(&BodyId { feature: marker, index: *index }, marker, feature)),
            None => r,
        },
        _ => r,
    };
    // The merged entities keep their attributes (std Form imports its tools by
    // the Tag feature's form attributes).
    for (r, v) in source_attributes {
        st.push_attribute(merged(r), v);
    }
    let out: Vec<Value> = tracked
        .into_iter()
        .map(|refs| {
            let ids = refs
                .into_iter()
                .map(|r| {
                    let r = merged(r);
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
    let interpolate = def.get("references").is_undefined();
    run_op(i, &cx, &id, "REGEN_ERROR", |ev| {
        if points.len() < 3 {
            return err("Select at least three points (vertices) to fit a surface");
        }
        // Mesh points (std passes the meshes as `references`) are fitted, picked points interpolated.
        let opts = k::SurfaceFitOptions { tolerance, interpolate, smooth };
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
    /// A cylinder's `allowanceRadius`: flat lengths are arcs at this radius
    /// (std sheet metal Bend wraps the bend allowance onto the bend radius).
    allowance: Option<f64>,
}

fn wrap_surface(i: &mut Interp, cx: &FsContext, v: &Value) -> R<WrapSurface> {
    let anchor = vec3_of(v.get("anchorPoint")).unwrap_or(DVec3::ZERO);
    let direction = vec3_of(v.get("anchorDirection")).unwrap_or(DVec3::X);
    let mut out = WrapSurface { face: Vec::new(), surface: None, anchor, direction, allowance: None };
    if !v.get("face").is_undefined() {
        out.face = evaluate_refs(i, cx, v.get("face"))?;
    } else if let Some(p) = plane_of(v.get("plane")) {
        out.surface = Some(k::Developable::Plane { origin: p.origin, x_dir: p.x, normal: p.normal });
    } else if let Some((origin, x, z)) = coord_system_of(v.get("cylinder").get("coordSystem")) {
        let radius = length_of(v.get("cylinder").get("radius")).unwrap_or(0.0);
        out.surface = Some(k::Developable::Cylinder { origin, axis: z, x_dir: x, radius });
        out.allowance = length_of(v.get("allowanceRadius")).filter(|a| *a > 0.0);
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
        let mut placement = k::Placement2d::align(f_src, d_src, f_dst, d_dst, false);
        // Flat lengths around the destination cylinder measured at its
        // allowance radius: scale them by radius / allowance about the anchor.
        if let (Some(a), k::Developable::Cylinder { radius, .. }) = (dst.allowance, &d)
            && !unwrap
        {
            let s = glam::DMat2::from_diagonal(glam::DVec2::new(radius / a, 1.0));
            placement = k::Placement2d { m: s * placement.m, t: f_dst + s * (placement.t - f_dst) };
        }
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

/// `opEditCurve`: the wire body of `wire` takes the curve of `edge`, in
/// place (std editCurve.fs: "replaces queryToReplace's edges in place").
fn op_edit_curve(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let (cx, id, def) = args(i, &a)?;
    let wire = evaluate_refs(i, &cx, def.get("wire"))?;
    let edge = evaluate_refs(i, &cx, def.get("edge"))?;
    run_op(i, &cx, &id, "REGEN_ERROR", |ev| {
        let mut bodies = ev.resolve_bodies(&wire);
        bodies.extend(ev.resolve_edges(&wire).into_iter().map(|(b, _)| b));
        let Some(&bi) = bodies.first() else { return err("Select a curve to edit") };
        let Some(&(eb, ei)) = ev.resolve_edges(&edge).first() else { return err("The new curve no longer exists") };
        let curve = ev.edge_shape(eb, ei)?;
        let shape = k::make_wire(&[curve]).map_err(|e| FeatureError::new(format!("The curve could not be edited: {e}")))?;
        let old = ev.ctx.bodies[bi].clone();
        let creator = old.edges.first().and_then(|n| n.creator()).unwrap_or(ev.feature);
        let mut body = crate::regen::context::Body::new(old.id.clone(), old.name.clone(), old.kind, shape, Vec::new(), old.color, creator);
        // Same number of edges: references to them keep working.
        if body.edges.len() == old.edges.len() {
            body.edges = old.edges.clone();
            body.vertices = old.vertices.clone();
        }
        ev.ctx.bodies[bi] = Arc::new(body);
        Ok(())
    })?;
    Ok(Value::Undefined)
}

/// `opModifyCompositePart`: `toAdd` / `toRemove` bodies of `composite`. The
/// composite keeps its id and name. A closed composite consumed its members,
/// so only adding to it is possible.
fn op_modify_composite_part(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let (cx, id, def) = args(i, &a)?;
    let composite = evaluate_refs(i, &cx, def.get("composite"))?;
    let add = evaluate_refs(i, &cx, def.get("toAdd"))?;
    let remove = evaluate_refs(i, &cx, def.get("toRemove"))?;
    run_op(i, &cx, &id, "CUSTOM_ERROR", |ev| {
        // The composite itself (resolving it would give its members).
        let ci = composite.iter().find_map(|r| match r {
            Reference::Body(id) => ev.ctx.bodies.iter().position(|b| &b.id == id && b.composite.is_some()),
            _ => None,
        });
        let Some(ci) = ci.or_else(|| ev.resolve_bodies(&composite).into_iter().find(|&b| ev.ctx.bodies[b].composite.is_some())) else { return err("Select a composite part") };
        let old = ev.ctx.bodies[ci].clone();
        let Some(info) = old.composite.clone() else { return err("Select a composite part") };
        let adding: Vec<BodyId> = ev.resolve_bodies(&add).into_iter().map(|b| ev.ctx.bodies[b].id.clone()).filter(|b| *b != old.id).collect();
        let removing: Vec<BodyId> = ev.resolve_bodies(&remove).into_iter().map(|b| ev.ctx.bodies[b].id.clone()).collect();
        if info.closed && !removing.is_empty() {
            return err("Bodies cannot be removed from a closed composite part");
        }
        let mut members: Vec<BodyId> = if info.closed { Vec::new() } else { info.members.iter().filter(|m| !removing.contains(m)).cloned().collect() };
        for b in adding {
            if !members.contains(&b) {
                members.push(b);
            }
        }
        // A closed composite's own shape stands for its consumed members.
        ev.ctx.bodies.remove(ci);
        let mut idx: Vec<usize> = members.iter().filter_map(|m| ev.ctx.bodies.iter().position(|b| &b.id == m)).collect();
        if info.closed {
            ev.ctx.bodies.push(old.clone());
            idx.insert(0, ev.ctx.bodies.len() - 1);
        }
        if idx.is_empty() {
            return err("A composite part needs at least one body");
        }
        crate::regen::features::refgeom::make_composite(ev, &idx, info.closed, 0)?;
        // The new composite (last, or where the closed one was put) takes the old identity.
        let pos = ev.ctx.bodies.iter().rposition(|b| b.composite.is_some() && b.id.feature == ev.feature).ok_or_else(|| FeatureError::new("The composite part could not be rebuilt"))?;
        let made = ev.ctx.bodies[pos].clone();
        let mut body = crate::regen::context::Body::new(old.id.clone(), old.name.clone(), made.kind, made.shape.clone(), made.faces.clone(), old.color, ev.feature);
        body.edges = made.edges.clone();
        body.vertices = made.vertices.clone();
        body.composite = made.composite.clone().map(|mut c| {
            if info.closed {
                c.members = info.members.iter().cloned().chain(c.members.into_iter().filter(|m| *m != old.id)).collect();
            }
            c
        });
        ev.ctx.bodies[pos] = Arc::new(body);
        Ok(())
    })?;
    Ok(Value::Undefined)
}
