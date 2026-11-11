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
use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;

pub const NATIVES: &[(&str, NativeFn)] = &[
    ("opConstrainedSurface", op_constrained_surface),
    ("opEditCurve", op_edit_curve),
    ("opModifyCompositePart", op_modify_composite_part),
    ("opWrap", op_wrap),
    // Older std's name for it (RollSurface: the same plane / cylinder and anchor fields, a SIMPLE wrap).
    ("opRoll", op_wrap),
    ("opExtrude", op_extrude),
    ("opRevolve", op_revolve),
    ("opBoolean", op_boolean),
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
    ("opCreateBSplineSurface", op_create_bspline_surface),
    ("opMoveCurveBoundary", op_move_curve_boundary),
    ("opExtractSurface", op_extract_surface),
    ("opCreateOutline", op_create_outline),
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
        for p in split_solids(&t.shape, &t.names.faces, Some(ev.feature)) {
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
        // As the built-in Extrude (regen/features/extrude.rs).
        "THROUGH_ALL" => {
            crate::regen::features::extrude::require_parts(ev).map_err(|_| super::code_error("CANNOT_BE_EMPTY"))?;
            Ok(Some(ev.model_extent()))
        }
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
        // The end: the picked faces / bodies, a plane std made (up to a
        // vertex or mate connector), or the next part (up to next).
        let target = |refs: &[Reference], bound: &str| -> FResult<Shape> {
            if bound == "UP_TO_NEXT" {
                crate::regen::features::extrude::require_parts(ev).map_err(|_| super::code_error("CANNOT_BE_EMPTY"))?;
                return crate::regen::features::extrude::next_parts(ev);
            }
            let faces = ev.resolve_faces(refs);
            // Bodies picked as bodies (a face's owner is not the bound: std's UP_TO_SURFACE start face).
            let body_refs: Vec<Reference> = refs.iter().filter(|r| matches!(r, Reference::Body(_))).cloned().collect();
            let bodies = ev.resolve_bodies(&body_refs);
            let mut shapes: Vec<Shape> = faces.iter().filter_map(|(b, f)| ev.face_shape(*b, *f).ok()).chain(bodies.iter().map(|b| ev.ctx.bodies[*b].shape.clone())).collect();
            if shapes.is_empty()
                && let Some(f) = refs.first().and_then(|r| ev.resolve_plane(r))
            {
                let ext = ev.model_extent() * 4.0;
                shapes.push(k::make_plane_face(&kernel_plane(&f), -ext, ext, -ext, ext)?);
            }
            match shapes.len() {
                0 => err("End condition must be a surface"),
                1 => Ok(shapes[0].clone()),
                _ => Ok(Shape::compound(&shapes)),
            }
        };
        // endTranslationalOffset moves the end along the direction.
        let end_offset = length_of(def.get("endTranslationalOffset")).unwrap_or(0.0);
        let end_target = |refs: &[Reference]| -> FResult<Shape> {
            let t = target(refs, &end_bound)?;
            if end_offset == 0.0 { Ok(t) } else { Ok(t.translated(d * end_offset)?) }
        };
        let profiles = ev.profiles(&refs)?;
        let mut tools = Vec::new();
        for p in &profiles {
            let res = match (start, end) {
                (Some(s), Some(e)) => k::extrude_between(&p.shape, -d * s, d * e, 0.0),
                (Some(s), None) => k::extrude_up_to(&p.shape, d, &end_target(&end_entity)?, -s),
                (None, Some(e)) => k::extrude_up_to(&p.shape, -d, &target(&start_entity, start_bound.as_deref().unwrap_or_default())?, 0.0).and_then(|r| {
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
        // An up-to end: the sheet runs far that way, then is cut back at the end surface (a hem between its two
        // bounding planes, std's sheet metal Hem).
        // Live 2026: startDepth / endDepth do nothing then; the translational offsets move the ends (the start back
        // toward the profile, the end on along the direction).
        let far = ev.model_extent() * 4.0 + 1.0;
        let start_offset = length_of(def.get("startTranslationalOffset")).unwrap_or(0.0);
        let bounds = Bounds {
            d,
            start: match start {
                None => Some(target(&start_entity, start_bound.as_deref().unwrap_or_default())?.translated(-d * start_offset)?),
                Some(_) => None,
            },
            end: match end {
                None => Some(end_target(&end_entity)?),
                Some(_) => None,
            },
        };
        for (n, edge) in edge_shapes(ev, &refs)?.into_iter().enumerate() {
            let (s, e) = (start.unwrap_or(far), end.unwrap_or(far));
            let mut res = k::extrude_between(&edge, -d * s, d * e, 0.0).map_err(|e| fe(format!("Extrude failed: {e}")))?;
            if bounds.start.is_some() || bounds.end.is_some() {
                res = bounds.trim(&res, &edge.translated(-d * s)?)?;
            }
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

/// An edge's up-to ends (live 2026): a surface, offset already, or a part, where each point of the sheet stops at the
/// first face it meets (one edge's sheet ends at two depths where the part steps); a start already on the part's
/// face runs through to its far side.
struct Bounds {
    d: DVec3,
    start: Option<Shape>,
    end: Option<Shape>,
}

impl Bounds {
    /// The edge's sheet `res`, run far toward its up-to ends, cut back to them; `start_line`: where it starts (the
    /// edge at its blind start, or far back).
    fn trim(&self, res: &k::OpResult, start_line: &Shape) -> FResult<k::OpResult> {
        // A planar face bounds as its whole plane (Onshape extends an up-to surface).
        let size = res.shape.bounding_box(false).map(|b| b.diagonal() * 2.0).unwrap_or(1.0);
        let extended = |b: &Shape| -> Shape {
            let faces = b.faces();
            match faces.first().and_then(|f| f.face_plane().ok().flatten()) {
                Some(pl) if faces.len() == 1 && b.solids().is_empty() => {
                    let c = res.shape.bounding_box(false).map(|x| x.center()).unwrap_or(pl.origin);
                    let o = c - pl.normal * (c - pl.origin).dot(pl.normal);
                    k::make_plane_face(&k::Plane::new(o, pl.normal, pl.x_dir), -size, size, -size, size).unwrap_or_else(|_| b.clone())
                }
                _ => b.clone(),
            }
        };
        let start = self.start.as_ref().map(extended);
        let end = self.end.as_ref().map(extended);
        let tools: Vec<Shape> = start.iter().chain(end.iter()).cloned().collect();
        let split = k::split(std::slice::from_ref(&res.shape), &tools, &k::BooleanOptions::default())?;
        let apart = |a: &Shape, b: &Shape| match (a.bounding_box(false), b.bounding_box(false)) {
            (Ok(x), Ok(y)) => (x.min - y.max).max(y.min - x.max).max_element() > 1e-7,
            _ => false,
        };
        let touches = |a: &Shape, b: &Shape| !apart(a, b) && k::distance(a, b).is_ok_and(|x| x.distance < 1e-7);
        // A planar bound: which side of it a point is, along the direction (+: further on).
        let side = |bound: &Shape, p: DVec3| -> Option<f64> {
            let pl = bound.faces().into_iter().next()?.face_plane().ok().flatten()?;
            let n = if pl.normal.dot(self.d) < 0.0 { -pl.normal } else { pl.normal };
            Some((p - pl.origin).dot(n))
        };
        let solid_end = end.as_ref().filter(|e| !e.solids().is_empty());
        let mut pieces: Vec<(Shape, DVec3)> = split.shape.faces().into_iter().filter_map(|f| Some((f.clone(), super::sheetmetal::inner_point(&f)?))).collect();
        if let Some(st) = &start {
            pieces.retain(|(_, c)| side(st, *c).is_none_or(|x| x > 0.0));
        }
        if let Some(end) = end.as_ref().filter(|_| solid_end.is_none()) {
            pieces.retain(|(_, c)| side(end, *c).is_none_or(|x| x < 0.0));
        }
        let begin = start.as_ref().unwrap_or(start_line);
        let kept: Vec<Shape> = match solid_end {
            // From the start, through pieces on the start's side of the part's boundary, up to where it is crossed.
            Some(part) => {
                let inside = |c: DVec3| part.classify_point(c, 1e-7).is_ok_and(|k| k == k::PointClass::Inside);
                let first: Vec<usize> = (0..pieces.len()).filter(|&i| touches(&pieces[i].0, begin)).collect();
                let Some(&f0) = first.first() else { return err("The extrude does not reach its end surface") };
                let mode = inside(pieces[f0].1);
                let mut keep = vec![false; pieces.len()];
                let mut todo: Vec<usize> = first.into_iter().filter(|&i| inside(pieces[i].1) == mode).collect();
                while let Some(i) = todo.pop() {
                    if std::mem::replace(&mut keep[i], true) {
                        continue;
                    }
                    for j in 0..pieces.len() {
                        if !keep[j] && inside(pieces[j].1) == mode && touches(&pieces[i].0, &pieces[j].0) {
                            todo.push(j);
                        }
                    }
                }
                pieces.into_iter().zip(keep).filter(|(_, k)| *k).map(|(p, _)| p.0).collect()
            }
            None => pieces.into_iter().map(|p| p.0).filter(|f| touches(f, begin)).collect(),
        };
        if kept.is_empty() {
            return err("The extrude does not reach its end surface");
        }
        let shape = if kept.len() == 1 { kept[0].clone() } else { k::sew(&kept, 1e-7, false).map(|r| r.shape).unwrap_or_else(|_| Shape::compound(&kept)) };
        // Its end along a periodic up-to face in one edge, as Parasolid's intersection is (live 2033: Sheet Metal Up to
        // Part's rolled wall ends in one edge along the Filling Part's step, where OCCT's is a curve, a line on the
        // step cylinder's seam and a curve).
        let shape = match solid_end.or(end.as_ref()) {
            Some(target) => k::join_target_seam_splits(&shape, target).map(|r| r.shape).unwrap_or(shape),
            None => shape,
        };
        // No history: these sheets are named by their roles alone.
        Ok(k::OpResult { shape, history: k::History::default() })
    }
}

/// Shell `p` with its faces agreeing, facing the way of the first of `sides` (a point and normal) that lies on it.
fn oriented_piece(p: &Shape, sides: &[(DVec3, DVec3)]) -> Shape {
    let Ok(fixed) = p.faces_oriented_consistently() else { return p.clone() };
    let faces = fixed.faces();
    let on = |f: &Shape, x: DVec3| k::make_vertex(x).ok().and_then(|v| k::distance(&v, f).ok()).is_some_and(|d| d.distance < 1e-7);
    let Some((f, n)) = sides.iter().find_map(|(x, n)| faces.iter().find(|f| on(f, *x)).map(|f| (f.normal_at_point(*x), *n))) else { return p.clone() };
    match f {
        Ok(m) if m.dot(n) < 0.0 => fixed.reversed().unwrap_or(fixed),
        Ok(_) => fixed,
        Err(_) => p.clone(),
    }
}

/// Whether an opFillet call asks for a detached surface (`createDetachedSurface`).
pub(super) fn detached_fillet(a: &[Value]) -> bool {
    a.get(2).is_some_and(|d| d.get("createDetachedSurface").as_bool() == Some(true))
}

/// opFillet with `createDetachedSurface`: the fillet faces alone as a new sheet, the bodies left as they are (std's
/// sheet metal measures a bend's angle on one: computeFilletAngle).
pub(super) fn op_fillet_detached(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let (cx, id, def) = args(i, &a)?;
    let refs = evaluate_refs(i, &cx, def.get("entities"))?;
    let radius = length_of(def.get("radius")).unwrap_or(0.0);
    run_op(i, &cx, &id, "FILLET_FAILED", |ev| {
        let mut by_body: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        for (b, e) in ev.resolve_edges(&refs) {
            by_body.entry(b).or_default().push(e);
        }
        if by_body.is_empty() || radius <= 0.0 {
            return Err(super::code_error("FILLET_FAILED"));
        }
        let mut sheets = Vec::new();
        for (b, edges) in by_body {
            let shape = ev.ctx.bodies[b].shape.clone();
            let shapes: Vec<Shape> = edges.iter().map(|e| shape.sub_shape(ShapeType::Edge, *e)).collect::<Result<_, _>>()?;
            let res = k::fillet(&shape, &shapes, radius)?;
            let faces: Vec<Shape> = edges
                .iter()
                .flat_map(|e| res.history.generated(0, ShapeType::Edge, *e as u32))
                .filter(|(t, _)| *t == ShapeType::Face)
                .filter_map(|(_, f)| res.shape.sub_shape(ShapeType::Face, f as usize).ok())
                .collect();
            if faces.is_empty() {
                return Err(super::code_error("FILLET_FAILED"));
            }
            let sheet = if faces.len() == 1 { faces[0].clone() } else { Shape::compound(&faces) };
            let names = fresh_names(ev, &sheet);
            sheets.push(NamedShape::new(sheet, names));
        }
        add_bodies(ev, sheets, BodyKind::Surface)
    })?;
    Ok(Value::Undefined)
}

/// A point inside a shape's first face and the face's normal there.
fn face_sample(s: &Shape) -> Option<(DVec3, DVec3)> {
    let f = s.faces().into_iter().next()?;
    let [u0, u1, v0, v1] = f.uv_bounds().ok()?;
    let (u, v) = ((u0 + u1) * 0.5, (v0 + v1) * 0.5);
    Some((f.point_at_uv(u, v).ok()?, f.normal_at_uv(u, v).ok()?))
}

/// Sheet `bi`'s faces joined where they touch along an edge (Parasolid's union shares it; the kernel's fuse leaves
/// two) and turned to agree with their neighbours, the face at `side`'s point facing `side`'s normal. The body is
/// left as it is when nothing changed (or its faces can't be matched back to their names).
fn orient_like(ev: &mut EvalCtx, bi: usize, (p, n): (DVec3, DVec3)) -> FResult<()> {
    let body = ev.ctx.bodies[bi].clone();
    let old = body.shape.faces();
    let sewn = k::sew(&old, 1e-6, false).ok().map(|s| s.shape).filter(|s| s.count(ShapeType::Face) == old.len()).unwrap_or_else(|| body.shape.clone());
    // Joined: more edges shared by two faces (a flange along part of a plate's edge splits that edge to share it).
    let shared = |s: &Shape| s.edge_faces().map(|ef| ef.iter().filter(|f| f.len() == 2).count()).unwrap_or(0);
    let joined = shared(&sewn) > shared(&body.shape);
    let Ok(mut fixed) = sewn.faces_oriented_consistently() else { return Ok(()) };
    let at_p = |f: &Shape| k::make_vertex(p).ok().and_then(|v| k::distance(&v, f).ok()).is_some_and(|d| d.distance < 1e-7);
    if fixed.faces().iter().find(|f| at_p(f)).is_some_and(|f| f.normal_at_point(p).is_ok_and(|m| m.dot(n) < 0.0)) {
        fixed = fixed.reversed()?;
    }
    let key = |f: &Shape| f.bounding_box(false).map(|b| (b.center(), b.diagonal())).ok();
    let keys: Vec<_> = old.iter().map(key).collect();
    let mut turned = false;
    let mut names = Vec::new();
    for f in fixed.faces() {
        let Some(kf) = key(&f) else { return Ok(()) };
        let Some(i) = keys.iter().position(|k| k.is_some_and(|k| k.0.distance(kf.0) < 1e-9 && (k.1 - kf.1).abs() < 1e-9)) else { return Ok(()) };
        turned |= f.orientation() != old[i].orientation();
        names.push(body.faces[i].clone());
    }
    if (turned || joined) && names.len() == old.len() {
        ev.replace_body(bi, fixed, names);
    }
    Ok(())
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
/// Sheet body `bi`, when its faces close a volume: that solid (its faces keep their names).
fn close_into_solid(ev: &mut EvalCtx, bi: usize) -> FResult<()> {
    let body = ev.ctx.bodies[bi].clone();
    if body.kind != BodyKind::Surface {
        return Ok(());
    }
    let faces = body.shape.faces();
    let Ok(sewn) = k::sew(&faces, 1e-6, true) else { return Ok(()) };
    if sewn.shape.count(ShapeType::Solid) == 0 {
        return Ok(());
    }
    let centroids: Vec<Option<DVec3>> = faces.iter().map(|f| f.centroid().ok()).collect();
    let names: Vec<Name> = sewn
        .shape
        .faces()
        .iter()
        .map(|f| {
            let c = f.centroid().ok();
            centroids.iter().position(|g| matches!((g, c), (Some(g), Some(c)) if g.distance(c) < 1e-7)).and_then(|j| body.faces.get(j).cloned()).unwrap_or_else(|| Arc::new(TopoName::face(ev.feature, Role::New(0))))
        })
        .collect();
    ev.replace_body(bi, sewn.shape, names);
    // (Just made by replace_body: ours alone.)
    if let Some(b) = Arc::get_mut(&mut ev.ctx.bodies[bi]) {
        b.kind = BodyKind::Solid;
    }
    Ok(())
}

fn sew_sheets(sheets: Vec<NamedShape>) -> FResult<Vec<NamedShape>> {
    if sheets.len() < 2 {
        return Ok(sheets);
    }
    let shapes: Vec<Shape> = sheets.iter().map(|s| s.shape.clone()).collect();
    let Ok(sewn) = k::sew(&shapes, 1e-7, false) else { return Ok(sheets) };
    let key = |f: &Shape| f.bounding_box(false).map(|b| (b.center(), b.diagonal())).ok();
    let named: Vec<(Option<(DVec3, f64)>, Name)> = sheets.iter().flat_map(|s| s.shape.faces().iter().map(key).zip(s.names.faces.iter().cloned()).collect::<Vec<_>>()).collect();
    // Each piece one way round (Parasolid's sheet from a chain of curves: a tangent line and arc drawn opposite ways
    // meet smooth), the side of its first curve's sheet.
    let sides: Vec<(DVec3, DVec3)> = sheets.iter().filter_map(|s| face_sample(&s.shape)).collect();
    let mut out = Vec::new();
    let shells = sewn.shape.shells();
    let pieces = if shells.is_empty() { vec![sewn.shape.clone()] } else { shells };
    for p in pieces {
        let p = oriented_piece(&p, &sides);
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
    // A sheet union that closes a volume becomes that solid (std's surface joins: Fill, Extrude ADD).
    let make_solid = def_bool(&def, "makeSolid", false);
    let failure = match &*kind {
        "SUBTRACTION" => "BOOLEAN_SUBTRACT_NO_OP",
        "INTERSECTION" => "BOOLEAN_INTERSECT_FAIL",
        _ => "BOOLEAN_INVALID",
    };
    // A subtraction's targets and its tools' faces (theirs in its result are the Boolean's).
    let mut before: Option<(Vec<BodyId>, HashSet<TopoName>)> = None;
    // A localized subtraction's new edges and vertices (bodies, edge midpoints, vertex points).
    let mut made: (Vec<BodyId>, Vec<DVec3>, Vec<DVec3>) = Default::default();
    let merged = run_op(i, &cx, &id, failure, |ev| {
        // localizedInFaces (std's sheet metal Tab, the community corner overcut): the tool faces (sheet faces or
        // sketch regions) cut out of the target faces only; the rest of their bodies stay as they were.
        if &*kind == "SUBTRACTION" && def_bool(&def, "localizedInFaces", false) {
            let mut tool_faces: Vec<Shape> = Vec::new();
            for (bi, fi) in ev.resolve_faces(&tool_refs) {
                tool_faces.push(ev.face_shape(bi, fi)?);
            }
            let regions: Vec<Reference> = tool_refs.iter().filter(|r| matches!(r, Reference::SketchRegion { .. })).cloned().collect();
            if !regions.is_empty() {
                for p in ev.profiles(&regions)? {
                    tool_faces.extend(p.shape.faces());
                }
            }
            let targets = by_body(ev.resolve_faces(&target_refs));
            if tool_faces.is_empty() || targets.is_empty() {
                return Err(super::code_error("CANNOT_RESOLVE_ENTITIES"));
            }
            let tool = Shape::compound(&tool_faces);
            for (bi, fis) in targets {
                let faces: Vec<Shape> = fis.iter().filter_map(|&fi| ev.face_shape(bi, fi).ok()).collect();
                let local = k::boolean(k::BooleanOp::Common, std::slice::from_ref(&tool), &[Shape::compound(&faces)], &k::BooleanOptions::default())?;
                if local.shape.faces().is_empty() {
                    continue;
                }
                let body = crate::regen::features::helpers_a::body_named(&ev.ctx.bodies[bi]);
                let res = k::boolean(k::BooleanOp::Cut, std::slice::from_ref(&body.shape), &[local.shape], &k::BooleanOptions::default())?;
                // What it made (Parasolid's created by the Boolean): the edges off every old edge (one it trimmed
                // keeps its identity) and the vertices that weren't there.
                let old_edges = body.shape.edges();
                let on_old = |p: DVec3| k::make_vertex(p).is_ok_and(|v| old_edges.iter().any(|e| k::distance(&v, e).is_ok_and(|d| d.distance < 1e-7)));
                let old_points = vertex_points(&body.shape);
                made.1.extend(edge_midpoints(&res.shape).into_iter().filter(|p| !on_old(*p)));
                made.2.extend(vertex_points(&res.shape).into_iter().filter(|p| !old_points.iter().any(|o| o.distance(*p) < 1e-7)));
                made.0.push(ev.ctx.bodies[bi].id.clone());
                let names = ev.name_result(&[&body.names], &res.history, &res.shape, &BTreeMap::new(), None);
                ev.replace_body(bi, res.shape, names);
            }
            return Ok(None);
        }
        // Onshape's opBoolean on empty selections (live qa5 boolean: a
        // Subtract without Targets reads "Cannot resolve entities.").
        if ev.resolve_bodies(&tool_refs).is_empty() {
            return Err(super::code_error("CANNOT_RESOLVE_ENTITIES"));
        }
        if &*kind == "SUBTRACTION" && ev.resolve_bodies(&target_refs).is_empty() {
            return Err(super::code_error("CANNOT_RESOLVE_ENTITIES"));
        }
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
                // Sheets only: the union faces the first one's way (Parasolid's: a hem's arc, rolled facing out,
                // turns to agree with the plate it joins).
                let sheets = all.iter().filter_map(|id| index(ev, id)).all(|i| ev.ctx.bodies[i].kind == BodyKind::Surface);
                let side = if sheets { face_sample(&ev.ctx.bodies[first].shape) } else { None };
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
                if let (Some(side), Some(bi)) = (side, index(ev, &all[0])) {
                    orient_like(ev, bi, side)?;
                }
                if erase && let Some(bi) = index(ev, &all[0]) {
                    merge_sheet_faces(ev, bi)?;
                }
                if make_solid && let Some(bi) = index(ev, &all[0]) {
                    close_into_solid(ev, bi)?;
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
                // keepTools: the tools stay, their common volume is a new body
                // (std's sheet metal boolean trims its tool by each thickened
                // definition face in turn, then deletes the trimmed piece: the
                // tool must outlive the first face, qa4 S1's slot).
                if keep {
                    let shapes: Vec<Shape> = tools.iter().filter_map(|id| index(ev, id)).map(|i| ev.ctx.bodies[i].shape.clone()).collect();
                    let res = k::boolean(k::BooleanOp::Common, &shapes[..1], &shapes[1..], &k::BooleanOptions::default())?;
                    if res.shape.faces().is_empty() {
                        return Ok(None);
                    }
                    let names = fresh_names(ev, &res.shape);
                    let kind = solid_kind(&res.shape);
                    add_bodies(ev, vec![NamedShape::new(res.shape, names)], kind)?;
                    return Ok(None);
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
                if &*kind == "SUBTRACTION" {
                    before = Some((targets.clone(), tool_shapes.iter().flat_map(|t| t.names.faces.iter().map(|n| (**n).clone())).collect()));
                }
                let op = if &*kind == "SUBTRACTION" { k::BooleanOp::Cut } else { k::BooleanOp::Common };
                let tool_names: HashSet<TopoName> = if keep { tool_shapes.iter().flat_map(|t| t.names.faces.iter().map(|n| (**n).clone())).collect() } else { HashSet::new() };
                ev.boolean_into(&idx, tool_shapes, op)?;
                // Kept tools keep their faces' names, so the faces they leave in the targets are the Boolean's own
                // (else a tool face would count as created by it: Butt Joint deleted the kept beam as a sliver).
                if keep {
                    let marker = ev.feature;
                    for bi in 0..ev.ctx.bodies.len() {
                        let b = ev.ctx.bodies[bi].clone();
                        if tools.contains(&b.id) || !b.faces.iter().any(|n| tool_names.contains(&**n)) {
                            continue;
                        }
                        let faces: Vec<Name> = b.faces.iter().map(|n| if tool_names.contains(&**n) { Arc::new(TopoName::face(marker, Role::Generated(n.clone()))) } else { n.clone() }).collect();
                        ev.replace_body(bi, b.shape.clone(), faces);
                    }
                }
                Ok(None)
            }
        }
    })?;
    if let Some(all) = merged {
        cx.state.lock().record_merge(&all[0], &all, &id);
    }
    let (bodies, mids, points) = made;
    if !mids.is_empty() || !points.is_empty() {
        let mut st = cx.state.lock();
        let seq = bodies.iter().filter_map(|b| st.touched.get(b).copied()).max().unwrap_or(u32::MAX);
        st.split_face_edges.push((id.clone(), bodies.clone(), mids, seq));
        st.made_vertices.push((id.clone(), bodies, points));
    }
    if let Some((targets, tools)) = before {
        let mut st = cx.state.lock();
        if let Some(op) = st.ops.values().find(|o| o.id == id).cloned() {
            let new: Vec<TopoName> = st.ctx.bodies.iter().filter(|b| targets.contains(&b.id)).flat_map(|b| b.faces.iter()).filter(|n| tools.contains(&**n) || tools.contains(n.root())).map(|n| (**n).clone()).collect();
            for n in new {
                st.adopt(n, op.clone());
            }
        }
    }
    Ok(Value::Undefined)
}

/// Each edge's middle.
fn edge_midpoints(s: &Shape) -> Vec<DVec3> {
    s.edges().iter().filter_map(|e| e.edge_info().ok().and_then(|i| e.point_at((i.first_param + i.last_param) * 0.5).ok())).collect()
}

/// Each vertex's point.
fn vertex_points(s: &Shape) -> Vec<DVec3> {
    s.vertices().iter().filter_map(|v| v.vertex_point().ok()).collect()
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
    // Tangent (apex) measurement, the sides flipped edge by edge.
    let apex = chamfer && def_enum(&def, "chamferMethod").is_some_and(|m| &*m == "APEX_RANGE");
    let override_refs = if chamfer { evaluate_refs(i, &cx, def.get("directionOverrides"))? } else { Vec::new() };
    run_op(i, &cx, &id, if chamfer { "CHAMFER_FAILED" } else { "FILLET_FAILED" }, |ev| {
        let overrides = ev.resolve_edges(&override_refs);
        let groups = by_body(ev.resolve_edges(&refs));
        if groups.is_empty() {
            return Err(super::code_error(if chamfer { "CHAMFER_SELECT_EDGES" } else { "FILLET_SELECT_EDGES" }));
        }
        let blend_role = |n: &Name| Role::Blend(n.clone());
        for (bi, edges) in groups.into_iter().rev() {
            let body = ev.ctx.bodies[bi].clone();
            let propagate = def_bool(&def, "tangentPropagation", false);
            // Without propagation a chamfer stops at the selection and runs straight on into a tangent neighbour (live
            // G7), as the Chamfer feature's: OCCT's would run on along it.
            let stops = chamfer && !propagate && crate::regen::features::detail::propagate_tangent(&body.shape, &edges).len() > edges.len();
            let edges = if chamfer && propagate { crate::regen::features::detail::propagate_tangent(&body.shape, &edges) } else { edges };
            let shapes: Vec<Shape> = edges.iter().filter_map(|e| body.shape.sub_shape(ShapeType::Edge, *e).ok()).collect();
            let flipped: HashSet<usize> = overrides.iter().filter(|(b, _)| *b == bi).map(|&(_, e)| e).collect();
            let (mut spec, mut reference) = (spec, Vec::new());
            if (chamfer && !matches!(spec, k::ChamferSpec::Equal(_))) || apex {
                reference = distance_faces(&body.shape, &edges, def_bool(&def, "oppositeDirection", false), &flipped).map_err(|e| fe(e.to_string()))?;
            }
            // Onshape's sides where OCCT reads them otherwise (a face curved across the edge): two distances.
            use crate::regen::features::blend_section::ChamferSides;
            let sides = match (apex, spec) {
                (true, k::ChamferSpec::Equal(d)) => Some(ChamferSides::Tangent(d, d)),
                (true, k::ChamferSpec::TwoDistances(a, b)) => Some(ChamferSides::Tangent(a, b)),
                (true, k::ChamferSpec::DistanceAngle(d, angle)) => Some(ChamferSides::TangentAngle(d, angle)),
                (false, k::ChamferSpec::DistanceAngle(d, angle)) if chamfer => Some(ChamferSides::OffsetAngle(d, angle)),
                _ => None,
            };
            if let Some(sides) = sides
                && let Some(two) = crate::regen::features::detail::sides_as_offsets(&body.shape, &edges, &reference, sides)?
            {
                spec = two;
            } else if matches!(spec, k::ChamferSpec::Equal(_)) {
                reference.clear();
            }
            let distances = match spec {
                _ if !stops => None,
                k::ChamferSpec::Equal(d) => Some((d, d)),
                k::ChamferSpec::TwoDistances(a, b) if edges.len() == 1 => {
                    let first = body.shape.edge_faces().map_err(|e| fe(e.to_string()))?.get(edges[0]).and_then(|f| f.first().copied()).map(|f| body.shape.sub_shape(ShapeType::Face, f as usize)).transpose().map_err(|e| fe(e.to_string()))?;
                    let on_first = first.is_some_and(|f| reference.first().is_some_and(|r| r.is_same(&f)));
                    Some(if on_first { (a, b) } else { (b, a) })
                }
                _ => None,
            };
            if let Some((r1, r2)) = distances {
                use crate::regen::features::blend_section::{BlendSpec, Section, section_blend};
                let spec = BlendSpec { section: Section::Chamfer, r1, r2, width: None, partial: None, run_on: true, cliff: false, sides: None, overflow: false, reference: None };
                section_blend(ev, bi, &edges, &spec)?;
                continue;
            }
            let res = match (chamfer, spec) {
                // The same blends as the Fillet and Chamfer features (with their fallbacks).
                (false, k::ChamferSpec::Equal(r)) => k::fillet_checked(&body.shape, &shapes, &[r]),
                (_, s) => k::chamfer_checked(&body.shape, &shapes, s, &reference),
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

/// The face each edge's distance lies on for opChamfer's distance and angle (live 2336 / 2393): the face all the edges
/// share (a blank's two rims: its cylinder), else each edge's face on the left of its curve's direction (a cylinder's
/// top rim: its cap, its bottom rim: the cylinder); Opposite direction the other face, and so a direction override for
/// its chain. Empty where an edge doesn't have two faces (OCCT's choice then).
fn distance_faces(shape: &Shape, edges: &[usize], opposite: bool, flipped: &HashSet<usize>) -> Result<Vec<Shape>, k::KernelError> {
    let ef = shape.edge_faces()?;
    let faces = shape.faces();
    let Some(pairs) = edges.iter().map(|&e| ef.get(e).and_then(|f| <[u32; 2]>::try_from(f.as_slice()).ok())).collect::<Option<Vec<_>>>() else { return Ok(Vec::new()) };
    let shared: Vec<u32> = pairs[0].iter().copied().filter(|f| pairs.iter().all(|p| p.contains(f))).collect();
    let scale = shape.bounding_box(false).map(|b| b.diagonal()).unwrap_or(1.0).max(1e-9);
    // A direction override flips its chain (the edges joined to it through shared vertices), as the Chamfer feature's.
    let ends: Vec<[DVec3; 2]> = edges.iter().map(|&e| shape.sub_shape(ShapeType::Edge, e).and_then(|s| s.edge_info()).map(|i| [i.start, i.end])).collect::<Result<_, _>>()?;
    let mut flip = vec![false; edges.len()];
    for i in (0..edges.len()).filter(|&i| flipped.contains(&edges[i])) {
        let mut chain = vec![i];
        let mut seen = vec![false; edges.len()];
        seen[i] = true;
        while let Some(a) = chain.pop() {
            flip[a] = !flip[a];
            for b in 0..edges.len() {
                if !seen[b] && ends[a].iter().any(|p| ends[b].iter().any(|q| p.distance(*q) < scale * 1e-9)) {
                    seen[b] = true;
                    chain.push(b);
                }
            }
        }
    }
    let mut out = Vec::with_capacity(edges.len());
    for ((&e, pair), flip) in edges.iter().zip(&pairs).zip(flip) {
        let pick = match shared.as_slice() {
            [f] if edges.len() > 1 => *f,
            _ => {
                let edge = shape.sub_shape(ShapeType::Edge, e)?;
                let info = edge.edge_info()?;
                let t = 0.5 * (info.first_param + info.last_param);
                let (p, along) = (edge.point_at(t)?, edge.tangent_at(t)?.normalize());
                // (The face the step `n × t` from the edge stays on is on its left.)
                let left = pair.iter().copied().find(|&f| {
                    let face = &faces[f as usize];
                    face.normal_at_point(p).is_ok_and(|n| face.contains_point(p + n.normalize().cross(along) * (scale * 1e-4), scale * 1e-7).unwrap_or(false))
                });
                match left {
                    Some(f) => f,
                    None => return Ok(Vec::new()),
                }
            }
        };
        let face = if opposite != flip { if pick == pair[0] { pair[1] } else { pair[0] } } else { pick };
        out.push(faces[face as usize].clone());
    }
    Ok(out)
}

fn op_chamfer(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    blend(i, &a, true)
}

// ---------------------------------------------------------------- transforms and copies

fn op_transform(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let (cx, id, def) = args(i, &a)?;
    let refs = evaluate_refs(i, &cx, def.get("bodies"))?;
    let Some(m) = transform_of(def.get("transform")) else { return Err(i.error("opTransform: transform should be a Transform")) };
    // The moved bodies' attributes go with them (Parasolid's): their entities' snapshots, which the remap after the
    // operation looks for, move first (a folded sheet metal piece kept its corner breaks' vertices, live 1188 Fold SM).
    let bodies: HashSet<BodyId> = with_ents(i, &cx, def.get("bodies"), |_, q, ents| {
        Ok(ents.iter().filter_map(|e| match q.owner(e)? {
            Ent::Body(b) => Some(q.st.ctx.bodies[b].id.clone()),
            _ => None,
        }).collect())
    })?;
    let before = move_attribute_snapshots(&cx, &bodies, &m);
    let moved = match run_op(i, &cx, &id, "TRANSFORM_FAILED", |ev| {
        transform_refs(ev, &refs, &m)?;
        Ok(ev.resolve_bodies(&refs).into_iter().map(|b| ev.ctx.bodies[b].id.clone()).collect::<Vec<_>>())
    }) {
        Ok(moved) => moved,
        Err(e) => {
            cx.state.lock().attr_snapshots.extend(before);
            return Err(e);
        }
    };
    // The edges and vertices a split made on a moved body go with it (they're found where they lie).
    let mut st = cx.state.lock();
    let follow = |owners: &[BodyId], points: &mut Vec<DVec3>| {
        if owners.iter().any(|o| moved.contains(o)) {
            let more: Vec<DVec3> = points.iter().map(|p| m.transform_point3(*p)).collect();
            points.extend(more);
        }
    };
    for (_, owners, mids, _) in st.split_face_edges.iter_mut() {
        follow(owners, mids);
    }
    for (_, owners, points) in st.made_vertices.iter_mut() {
        follow(owners, points);
    }
    Ok(Value::Undefined)
}

/// The attribute snapshots of `bodies`' entities moved by `m`; their old ones returned (to put back if the
/// operation fails).
fn move_attribute_snapshots(cx: &FsContext, bodies: &HashSet<BodyId>, m: &DMat4) -> Vec<(Reference, (BodyId, super::query::Snapshot))> {
    let mut st = cx.state.lock();
    let mut before = Vec::new();
    for (r, (body, snap)) in st.attr_snapshots.iter_mut().filter(|(_, (b, _))| bodies.contains(b)) {
        before.push((r.clone(), (body.clone(), snap.clone())));
        for p in snap.points.iter_mut() {
            *p = m.transform_point3(*p);
        }
        snap.face = snap.face.as_ref().and_then(|f| k::transform(f, &k::Transform::Matrix(*m), true).ok()).map(|t| t.shape);
    }
    before
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
    // Sketch bodies: those only (a sketch's others stay; live moves one curve's wire alone).
    for (i, take) in ev.ctx.sketch_bodies_of(refs) {
        let j = ev.ctx.split_sketch(i, &take);
        let s = &ev.ctx.sketches[j];
        let frame = crate::regen::eval::transform_frame(&s.frame, m);
        ev.ctx.sketches[j] = Arc::new(crate::regen::context::SketchResult { frame, ..(**s).clone() });
    }
    for r in refs {
        match r {
            r if is_sketch_ref(r) => {}
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

/// A reference to a sketch, one of its bodies or entities.
fn is_sketch_ref(r: &Reference) -> bool {
    matches!(r, Reference::Sketch(_) | Reference::SketchBody { .. } | Reference::SketchEntity { .. } | Reference::SketchRegion { .. } | Reference::SketchRegionEdge { .. } | Reference::SketchCurveEnd { .. })
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
    let only_edges = !refs.is_empty() && refs.iter().all(|r| matches!(r, Reference::Edge(_)));
    let only_faces = !refs.is_empty() && refs.iter().all(|r| matches!(r, Reference::Face(_)));
    let (made, sketches) = run_op(i, &cx, &id, "PATTERN_BODY_FAILED", |ev| {
        // A sheet's hole loops (std sheet metal's edge pattern: a Fill pattern of holes): the holes cut again
        // where each instance puts them, in the sheet itself.
        if only_edges && let Some(done) = pattern_sheet_holes(ev, &refs, &transforms)? {
            return Ok(done);
        }
        // A solid's faces (std's FACE pattern: a Fill pattern of holes in a part): repeated in their own body, as
        // Parasolid's face pattern (not copies of the body).
        if only_faces && let Some(done) = pattern_solid_faces(ev, &refs, &transforms, &names)? {
            return Ok(done);
        }
        let bodies = ev.resolve_bodies(&refs);
        // Sketch bodies too, and the bodies of sketch regions and entities (a part pattern copies their owner body:
        // cf-transform-pattern's Transform pattern 4 makes no part in live; std Form's flat sketches go with its
        // tools): copies of those bodies only.
        let pieces: Vec<Arc<crate::regen::context::SketchResult>> = ev
            .ctx
            .sketch_bodies_of(&refs)
            .into_iter()
            .map(|(i, take)| {
                let s = &ev.ctx.sketches[i];
                if take == s.held() { s.clone() } else { Arc::new(crate::regen::context::SketchResult { bodies: Some(take), ..(**s).clone() }) }
            })
            .collect();
        let sources: Vec<(Arc<crate::regen::context::SketchResult>, DMat4)> = pieces.into_iter().flat_map(|s| transforms.iter().map(move |m| (s.clone(), *m))).collect();
        if bodies.is_empty() && sources.is_empty() {
            return err("Could not pattern selected parts.");
        }
        let sketches = copy_sketches(ev, &id, sources.into_iter(), false);
        let mut n = 0u32;
        let mut made: Vec<(String, BodyId, BodyId)> = Vec::new();
        for bi in bodies {
            let body = ev.ctx.bodies[bi].clone();
            for (k_, m) in transforms.iter().enumerate() {
                // Located instances sharing the body's shapes (mirrors share one mirrored copy), as
                // Onshape's instances: a boolean with many of them intersects each congruent pair once.
                let res = k::transform(&body.shape, &k::Transform::Matrix(*m), false)?;
                let inst = instance_number(names.get(k_).map(|s| s.as_str()).unwrap_or(""), k_);
                let faces = copy_names(ev, &body.faces, &res, inst);
                push_body(ev, BodyId { feature: ev.feature, index: n }, body.kind, res.shape, faces);
                made.push((names.get(k_).cloned().unwrap_or_default(), body.id.clone(), BodyId { feature: ev.feature, index: n }));
                n += 1;
            }
        }
        Ok((made, sketches))
    })?;
    // Instance bodies as renamed after the operation.
    let mut st = cx.state.lock();
    let (marker, feature) = (super::marker_of(&id), st.feature);
    if !made.is_empty() {
        let seq = st.next_seq;
        st.pattern_moves.extend(transforms.iter().filter(|m| !m.abs_diff_eq(DMat4::IDENTITY, 1e-12)).map(|m| (seq, *m)));
    }
    own_sketches(&mut st, marker, &sketches);
    for (source, copy) in &sketches {
        copy_sketch_attributes(&mut st, *source, *copy);
    }
    for (name, source, b) in made {
        let inst = super::wrap_body_id(&b, marker, st.op_bits(marker), feature);
        // A copy keeps its body's attributes (std Form's tool instances keep the Tag's form attributes).
        let copied: Vec<Value> = st.attributes_of(&Reference::Body(source.clone())).into_iter().cloned().collect();
        for v in copied {
            st.push_attribute(Reference::Body(inst.clone()), v);
        }
        st.pattern_instances.entry((id.clone(), name)).or_default().push(inst);
    }
    Ok(Value::Undefined)
}

/// The instances of a sheet's hole loops (`edges`: closed loops within one of its planar faces): each cut out of
/// the sheet where its transform puts it, all at once. Faces keep their names, edges theirs where they stay; the
/// holes' edges are new. `None`: not a sheet's hole loops (copies of their bodies instead).
type PatternMade = (Vec<(String, BodyId, BodyId)>, Vec<(crate::reference::FeatureId, crate::reference::FeatureId)>);
fn pattern_sheet_holes(ev: &mut EvalCtx, refs: &[Reference], transforms: &[DMat4]) -> FResult<Option<PatternMade>> {
    let edges = ev.resolve_edges(refs);
    let Some(&(b, _)) = edges.first() else { return Ok(None) };
    let body = ev.ctx.bodies[b].clone();
    if body.kind != BodyKind::Surface || edges.iter().any(|(x, _)| *x != b) {
        return Ok(None);
    }
    let all = body.shape.edges();
    let picked: Vec<Shape> = edges.iter().filter_map(|(_, e)| all.get(*e).cloned()).collect();
    let fe = |e: k::KernelError| FeatureError::new(e.to_string());
    let loops = k::edges_to_wires(&Shape::compound(&picked), 1e-7).map_err(fe)?;
    let faces = body.shape.faces();
    // Each loop closed, and an inner loop of one planar face of the sheet.
    let inner: Vec<Shape> = faces.iter().filter(|f| f.face_plane().is_ok_and(|p| p.is_some())).flat_map(|f| {
        let ws = f.wires();
        let outer = ws.iter().max_by(|x, y| x.bounding_box(false).map(|b| b.diagonal()).unwrap_or(0.0).total_cmp(&y.bounding_box(false).map(|b| b.diagonal()).unwrap_or(0.0))).cloned();
        ws.into_iter().filter(move |w| outer.as_ref().is_none_or(|o| !o.is_same(w)))
    }).collect();
    let holds = |w: &Shape| w.is_closed().unwrap_or(false) && w.edges().iter().all(|e| inner.iter().any(|h| h.edges().iter().any(|x| x.is_same(e))));
    if loops.is_empty() || !loops.iter().all(holds) {
        return Ok(None);
    }
    let mut tools = Vec::with_capacity(loops.len() * transforms.len());
    for w in &loops {
        let hole = k::make_face_from_wire(w, true).map_err(fe)?;
        for m in transforms {
            tools.push(k::transform(&hole, &k::Transform::Matrix(*m), true).map_err(fe)?.shape);
        }
    }
    let cut = k::boolean(k::BooleanOp::Cut, std::slice::from_ref(&body.shape), &tools, &k::BooleanOptions::default()).map_err(fe)?.shape;
    // Faces by their planes (a cut leaves each face where it was), edges by where they lie.
    let plane_of = |f: &Shape| f.face_plane().ok().flatten().map(|p| (p.normal.normalize(), p.origin.dot(p.normal.normalize())));
    let old_planes: Vec<_> = faces.iter().map(plane_of).collect();
    let mut used = vec![false; faces.len()];
    let mut names = Vec::new();
    for f in cut.faces() {
        let pl = plane_of(&f);
        let j = (0..faces.len()).find(|&j| !used[j] && match (pl, old_planes[j]) {
            (Some((n, d)), Some((m, e))) => n.dot(m).abs() > 1.0 - 1e-9 && (d - e * n.dot(m).signum()).abs() < 1e-7,
            _ => false,
        });
        let Some(j) = j else { return err("Could not pattern selected parts.") };
        used[j] = true;
        names.push(body.faces[j].clone());
    }
    let mid = |e: &Shape| e.edge_info().ok().and_then(|i| e.point_at((i.first_param + i.last_param) * 0.5).ok());
    let old_mids: Vec<Option<DVec3>> = all.iter().map(mid).collect();
    let edge_from: Vec<Option<usize>> = cut.edges().iter().map(|e| {
        let m = mid(e)?;
        old_mids.iter().position(|o| o.is_some_and(|o| o.distance(m) < 1e-7))
    }).collect();
    ev.replace_body(b, cut, names);
    super::edge_change::keep_edge_names(ev, b, &body.edges, &edge_from);
    Ok(Some((Vec::new(), Vec::new())))
}

/// The instances of solids' faces (`refs`), each set repeated in its own body by Face pattern's evaluator. `None`:
/// not all faces of solids.
fn pattern_solid_faces(ev: &mut EvalCtx, refs: &[Reference], transforms: &[DMat4], names: &[String]) -> FResult<Option<PatternMade>> {
    let mut by_body: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for (b, f) in ev.resolve_faces(refs) {
        by_body.entry(b).or_default().push(f);
    }
    if by_body.is_empty() || by_body.keys().any(|&b| ev.ctx.bodies[b].kind != BodyKind::Solid) {
        return Ok(None);
    }
    use crate::regen::features::pattern::{Instance, pattern_faces};
    let instances: Vec<Instance> = transforms.iter().enumerate().map(|(k_, m)| Instance { code: instance_number(names.get(k_).map(|s| s.as_str()).unwrap_or(""), k_), t: k::Transform::Matrix(*m) }).collect();
    let bodies: Vec<(BodyId, Vec<usize>)> = by_body.into_iter().map(|(b, fs)| (ev.ctx.bodies[b].id.clone(), fs)).collect();
    for (id, faces) in bodies {
        pattern_faces(ev, &id, &faces, &instances, true)?;
    }
    Ok(Some((Vec::new(), Vec::new())))
}

fn op_delete_bodies(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let (cx, id, def) = args(i, &a)?;
    // Owner bodies of whatever was passed.
    let owners = with_ents(i, &cx, def.get("entities"), |_, q, ents| Ok(ents.iter().filter_map(|e| q.owner(e).or(Some(e.clone()))).map(|e| q.to_ref(&e)).collect::<Vec<_>>()))?;
    run_op(i, &cx, &id, "DELETE_PARTS_FAILED", |ev| {
        // Sketch bodies: those only (live: one curve's wire goes, the sketch's others stay).
        let bodies = ev.ctx.sketch_bodies_of(&owners);
        ev.ctx.delete_sketch_bodies(&bodies);
        for r in &owners {
            match r {
                Reference::Body(b) => ev.ctx.bodies.retain(|x| &x.id != b),
                Reference::Sketch(f) => ev.ctx.sketches.retain(|s| s.feature != *f),
                r if is_sketch_ref(r) => {}
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
            // As the Shell feature (live 1765: std's own opShell is what the feature calls).
            let res = crate::regen::features::detail::shell_part(&body.shape, &removed, t.abs(), side)?;
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
        // Faces of one sheet thicken together where they join (cf-belt: a belt's ten faces make one solid, live).
        let mut by_body: BTreeMap<usize, Vec<Shape>> = BTreeMap::new();
        for (b, f) in ev.resolve_faces(&refs) {
            if let Ok(face) = ev.face_shape(b, f) {
                by_body.entry(b).or_default().push(face);
            }
        }
        let mut shapes: Vec<Shape> = Vec::new();
        for (_, faces) in by_body {
            match faces.len() {
                1 => shapes.extend(faces),
                n => {
                    // Every face in a shell, else each on its own.
                    let shells = k::sew(&faces, 1e-7, false).map(|r| r.shape.shells()).unwrap_or_default();
                    if !shells.is_empty() && shells.iter().map(|s| s.faces().len()).sum::<usize>() == n {
                        shapes.extend(shells);
                    } else {
                        shapes.extend(faces);
                    }
                }
            }
        }
        // Sheet bodies given as bodies; a face stands for itself, not its
        // body (std's sheet metal boolean thickens one definition face of a
        // many-face sheet: the whole sheet came out too, qa4 S1's slot cut
        // nothing).
        let bodies: Vec<Reference> = refs.iter().filter(|r| !matches!(r, Reference::Face(_) | Reference::Edge(_))).cloned().collect();
        for bi in ev.resolve_bodies(&bodies) {
            if ev.ctx.bodies[bi].kind == BodyKind::Surface {
                shapes.push(ev.ctx.bodies[bi].shape.clone());
            }
        }
        for p in ev.profiles(&refs.iter().filter(|r| matches!(r, Reference::SketchRegion { .. })).cloned().collect::<Vec<_>>())? {
            shapes.push(p.shape);
        }
        let mut tools = Vec::new();
        for s in shapes {
            let mut res = k::thicken(&s, t1, t2)?;
            // The sides along a many-face sheet's boundary: one face per plane (cf-belt's end rings, live 22 faces).
            let caps = if s.faces().len() > 1 {
                res.shape = k::unify_same_domain(&res.shape)?.shape;
                None
            } else {
                from_faces(&s, &res)
            };
            let names = thicken_names(ev, &s, &res.shape, t1, t2, caps.as_deref());
            tools.push(NamedShape::new(res.shape, names));
        }
        add_bodies(ev, tools, BodyKind::Solid)
    })?;
    Ok(Value::Undefined)
}

/// Which of a thickened sheet's faces come from its faces (the caps can only be those; the sides come from its edges),
/// by the thicken's history and the sheet's faces kept as they are. `None` when the history names none.
fn from_faces(sheet: &Shape, res: &k::OpResult) -> Option<Vec<bool>> {
    let out = res.shape.faces();
    let mut from = vec![false; out.len()];
    for r in &res.history.records {
        if r.input_kind == ShapeType::Face && r.output_kind == ShapeType::Face && r.relation != k::Relation::Deleted && let Some(x) = from.get_mut(r.output_index as usize) {
            *x = true;
        }
    }
    let sheet_faces = sheet.faces();
    for (x, f) in from.iter_mut().zip(&out) {
        *x = *x || sheet_faces.iter().any(|g| g.is_same(f));
    }
    from.contains(&true).then_some(from)
}

/// A thickened sheet's names: its caps (the faces `t1` along the sheet's normal and `t2` against it) as
/// qCapEntity finds them (std's sheet metal boolean checks a cut through a rolled wall by them), the sides fresh.
/// `from_faces` ([`from_faces`]): the faces that can be caps (a side's foot isn't looked for: on a lofted wave
/// spring's band each search took ~27 ms).
fn thicken_names(ev: &EvalCtx, sheet: &Shape, out: &Shape, t1: f64, t2: f64, from_faces: Option<&[bool]>) -> Vec<Name> {
    let sheet_faces = sheet.faces();
    let mut roles = BTreeMap::new();
    for (i, f) in out.faces().iter().enumerate() {
        if from_faces.is_some_and(|c| !c.get(i).copied().unwrap_or(true)) {
            continue;
        }
        let Some(p) = super::sheetmetal::inner_point(f) else { continue };
        let Ok(n) = f.normal_at_point(p) else { continue };
        // The nearest sheet face, nearest boxes first (a face's box farther than the best distance can't be nearer).
        // (Ties to the first face, as a plain minimum.)
        let mut order: Vec<(f64, usize, &Shape)> = sheet_faces.iter().enumerate().map(|(i, sf)| (sf.bounding_box(false).map_or(0.0, |b| (b.min - p).max(p - b.max).max(DVec3::ZERO).length()), i, sf)).collect();
        order.sort_by(|a, b| a.0.total_cmp(&b.0));
        // A cap: its point's foot a thickness along its normal lies on a sheet face (the nearest point's offset runs
        // along the normal); no foot, no cap (the sides: projecting a point beside a rolled wall takes ~1.4 ms). The
        // classifier and a foot on the face are cheap.
        let nn = n.normalize_or_zero();
        let foot = [(t1, 1.0), (t1, -1.0), (t2, 1.0), (t2, -1.0)].into_iter().find_map(|(t, sign)| {
            let q = p - nn * (sign * t);
            let in_box = |sf: &Shape| sf.bounding_box(false).is_ok_and(|b| (b.min - q).max(q - b.max).max_element() <= 1e-7);
            let (j, sf) = order.iter().map(|(_, j, sf)| (*j, *sf)).find(|(_, sf)| in_box(sf) && sf.contains_point(q, 1e-7).unwrap_or(false))?;
            sf.project_point(q).ok().filter(|pr| pr.distance < 1e-7).map(|pr| (j, sf, k::FaceProjection { distance: t, ..pr }))
        });
        // (The foot's face is the best so far: only faces whose boxes come nearer are projected.)
        let Some(foot) = foot else { continue };
        let mut near = Some(foot);
        for (gap, i, sf) in order {
            if near.as_ref().is_some_and(|(j, _, _)| *j == i) {
                continue;
            }
            if near.as_ref().is_some_and(|(_, _, pr)| gap > pr.distance) {
                break;
            }
            if let Ok(pr) = sf.project_point(p)
                && near.as_ref().is_none_or(|(j, _, b)| pr.distance < b.distance || (pr.distance == b.distance && i < *j))
            {
                near = Some((i, sf, pr));
            }
        }
        let near = near.map(|(_, sf, pr)| (sf, pr));
        let Some((sf, pr)) = near else { continue };
        let Ok(m) = sf.normal_at_uv(pr.u, pr.v) else { continue };
        let m = m.normalize_or_zero();
        if n.normalize_or_zero().cross(m).length() > 1e-6 {
            continue;
        }
        let along = (p - pr.point).dot(m);
        let role = if (along - t1).abs() < 1e-7 {
            Role::CapEnd(None)
        } else if (along + t2).abs() < 1e-7 {
            Role::CapStart(None)
        } else {
            continue;
        };
        roles.insert(i as u32, role);
    }
    ev.name_result(&[&ShapeNames::default()], &k::History::default(), out, &roles, None)
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
/// Whether `profile` is circles about one end of `path`, square to it there (a hose's): its sweep's shape doesn't
/// depend on the frame, only how the frame turns the surfaces' parameters.
fn round_about_path(profile: &Shape, path: &Shape) -> bool {
    let tol = path.bounding_box(false).map(|b| b.diagonal()).unwrap_or(1.0) * 1e-9;
    let mut circle: Option<(DVec3, DVec3)> = None;
    for e in profile.edges() {
        let Ok(k::Curve::Circle { center, axis, .. }) = e.edge_curve() else { return false };
        match circle {
            None => circle = Some((center, axis)),
            Some((c, a)) if (c - center).length() > tol || a.cross(axis).length() > 1e-9 => return false,
            _ => {}
        }
    }
    let Some((c, a)) = circle else { return false };
    path.edges().iter().any(|e| {
        let Ok(i) = e.edge_info() else { return false };
        [i.first_param, i.last_param].into_iter().any(|t| {
            e.point_at(t).is_ok_and(|p| (p - c).length() <= tol) && e.tangent_at(t).is_ok_and(|d| d.normalize_or_zero().cross(a).length() < 1e-9)
        })
    })
}

fn sweep_any_frame(profile: &Shape, path: &Shape, opts: &k::SweepOptions) -> k::Result<k::OpResult> {
    let mut last = None;
    // A round profile by the discrete frame first: the corrected Frenet frame turns back and forth along a hose's
    // straight pieces (cf-hose-routing: 2 mm³ off the exact tube, its seam 0.25 mm off its plane).
    let modes = if round_about_path(profile, path) {
        [k::SweepMode::Discrete, k::SweepMode::CorrectedFrenet, k::SweepMode::Frenet]
    } else {
        [k::SweepMode::CorrectedFrenet, k::SweepMode::Frenet, k::SweepMode::Discrete]
    };
    for mode in modes {
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
    // The profile's own orientation kept, or kept square to a direction (sweep.fs sets these from Profile control).
    let keep = def_bool(&def, "keepProfileOrientation", false) || def_enum(&def, "profileControl").is_some_and(|c| &*c == "KEEP_ORIENTATION");
    let lock_dir = vec3_of(def.get("lockDirection")).filter(|d| d.length() > 1e-12).map(|d| d.normalize());
    run_op(i, &cx, &id, "SWEEP_FAILED", |ev| {
        // Onshape's opSweep on empty selections, path first (as the Sweep feature).
        if path.is_empty() {
            return Err(super::code_error_faulty("SWEEP_SELECT_PATH", &["path"]));
        }
        if profiles.is_empty() {
            return Err(super::code_error_faulty("SWEEP_SELECT_PROFILE", &["profiles"]));
        }
        let wire = path_wire(ev, &path)?;
        let opts = k::SweepOptions { solid: true, ..Default::default() };
        let mut tools = Vec::new();
        // A sharp corner of the path mitred, as the Sweep feature (Onshape's: the pieces meet in the plane that halves
        // it); the transformed sections where that fails.
        let mitred = k::SweepOptions { transition: k::SweepTransition::RightCorner, ..opts.clone() };
        for p in ev.profiles(&profiles)? {
            let mode = if keep { Some(k::SweepMode::Fixed { z: p.frame.normal, x: p.frame.x }) } else { lock_dir.map(k::SweepMode::Binormal) };
            let run = |o: &k::SweepOptions| match &mode {
                Some(m) => k::sweep(&p.shape, &wire, &k::SweepOptions { mode: *m, ..o.clone() }),
                None => sweep_any_frame(&p.shape, &wire, o),
            };
            let res = match run(&mitred) {
                Ok(r) if r.shape.is_valid() && r.shape.count(ShapeType::Solid) > 0 => r,
                _ => run(&opts).map_err(|e| fe(format!("Sweep failed: {e}")))?,
            };
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
    // `bodyType` is a ToolBodyType (SOLID / SURFACE).
    let solid = def_enum(&def, "bodyType").is_none_or(|t| &*t != "SURFACE");
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
            return Err(super::code_error("LOFT_TWO_PROFILES"));
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
    // Onshape's derivatives are with respect to the spline's parameter on [0, 1] (geomOperations.fs:
    // arcLengthParameterization false).
    let ends = vec3_of(def.get("startDerivative")).zip(vec3_of(def.get("endDerivative")));
    run_op(i, &cx, &id, "SKETCH_SPLINE_FAILED", |ev| {
        if pts.len() < 2 {
            return err("A spline needs at least two points");
        }
        // Through two points, nothing else: a line (std Jog reads its fitted
        // bend reference with evLine).
        if pts.len() == 2 && ends.is_none() {
            return add_wire(ev, k::make_line(pts[0], pts[1])?);
        }
        let closed = pts.len() > 2 && (pts[0] - pts[pts.len() - 1]).length() < 1e-9;
        // Closed: periodic at the centripetal parameters, as a closed sketch spline (cf-3d-spline's box as live's).
        let fit = if closed && ends.is_none() { noshape_sketch::nurbs::fit_3d_closed(&pts) } else { None };
        if closed && fit.is_none() {
            // (OCCT's periodic interpolation runs over the chord length.)
            let chord: f64 = pts.windows(2).map(|w| (w[1] - w[0]).length()).sum::<f64>().max(1e-300);
            let tangents: Vec<DVec3> = ends.map(|(s, e)| vec![s / chord, e / chord]).unwrap_or_default();
            return add_wire(ev, k::make_interpolated_spline(&pts[..pts.len() - 1], &tangents, true)?);
        }
        // Live 2052: knots at the centripetal parameters on [0, 1], natural ends without derivatives.
        let (knots, poles) = fit.or_else(|| noshape_sketch::nurbs::fit_3d(&pts, ends)).ok_or_else(|| FeatureError::new("The spline could not be fitted"))?;
        let mut unique: Vec<f64> = Vec::new();
        let mut mults: Vec<u32> = Vec::new();
        for k in knots {
            match mults.last_mut() {
                Some(m) if unique.last().is_some_and(|&u| (k - u).abs() < 1e-15) => *m += 1,
                _ => {
                    unique.push(k);
                    mults.push(1);
                }
            }
        }
        add_wire(ev, k::make_bspline(&poles, None, &unique, &mults, 3, false)?)
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
            // std's Edit curve reads this code as "no selection" (its Curves field faulty, live edit-curve-tool).
            return Err(super::code_error("EXTRACT_WIRES_NEEDS_EDGES"));
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
    // Bend i is at point i + 1 (the closing point's last for a closed one).
    let bends: Option<Vec<f64>> = def.get("bendRadii").as_array().map(|v| v.iter().map(|r| length_of(r).unwrap_or(0.0)).collect());
    run_op(i, &cx, &id, "POLYLINE_CONSECUTIVE_EQUAL_POINTS", |ev| {
        if pts.windows(2).any(|w| (w[0] - w[1]).length() < 1e-12) {
            return Err(super::code_error("POLYLINE_CONSECUTIVE_EQUAL_POINTS"));
        }
        let closed = pts.len() > 2 && (pts[0] - pts[pts.len() - 1]).length() < 1e-9;
        let p = if closed { &pts[..pts.len() - 1] } else { &pts[..] };
        let Some(bends) = &bends else { return add_wire(ev, k::make_polyline(p, closed)?) };
        if bends.len() != if closed { p.len() } else { p.len().saturating_sub(2) } {
            return Err(super::code_error("POLYLINE_BEND_RADII_INVALID_SIZE"));
        }
        if bends.iter().all(|r| *r <= 0.0) {
            return add_wire(ev, k::make_polyline(p, closed)?);
        }
        let mut radii = vec![0.0; p.len()];
        for (k, r) in bends.iter().enumerate() {
            radii[(k + 1) % p.len()] = *r;
        }
        // The Routing curve's arcs, the lines listed first as Onshape (curves.rs).
        let wire = crate::regen::features::curves::filleted_polyline(p, &radii, closed).map_err(|_| super::code_error("POLYLINE_INVALID_BEND"))?;
        add_wire(ev, crate::regen::features::curves::lines_before_arcs(&wire)?)
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
    // KEEP_FRONT: the pieces on the side the plane's normal points to (live
    // "Split" ☐ Keep both sides); KEEP_BACK the others.
    let keep_side = match def_enum(&def, "keepType").as_deref() {
        Some("KEEP_FRONT") => Some(true),
        Some("KEEP_BACK") => Some(false),
        _ => None,
    };
    let before_ids: Vec<BodyId> = cx.state.lock().ctx.bodies.iter().map(|b| b.id.clone()).collect();
    let (split_targets, plane, mids, centres) = run_op(i, &cx, &id, "SPLIT_FAILED", |ev| {
        // Onshape's opSplitPart texts (std splitpart.fs leaves them to it).
        if ev.resolve_bodies(&targets).is_empty() {
            return Err(super::code_error_faulty("SPLIT_SELECT_TARGETS", &["targets"]));
        }
        if tool_refs.is_empty() {
            return Err(super::code_error_faulty("SPLIT_SELECT_TOOL", &["tool"]));
        }
        let mut tools: Vec<Shape> = Vec::new();
        let mut plane = None;
        for r in &tool_refs {
            if let Some(p) = matches!(r, Reference::Plane(_) | Reference::FeaturePlane(_) | Reference::ConstructionPlane { .. }).then(|| ev.resolve_plane(r)).flatten() {
                plane.get_or_insert((p.origin, p.normal));
                tools.push(k::make_plane_face(&kernel_plane(&p), -1e3, 1e3, -1e3, 1e3)?);
            }
        }
        tools.extend(ev.resolve_faces(&tool_refs).iter().filter_map(|(b, f)| ev.face_shape(*b, *f).ok()));
        let tool_bodies = ev.resolve_bodies(&tool_refs.iter().filter(|r| matches!(r, Reference::Body(_))).cloned().collect::<Vec<_>>());
        tools.extend(tool_bodies.iter().map(|b| ev.ctx.bodies[*b].shape.clone()));
        if tools.is_empty() {
            return Err(super::code_error_faulty("SPLIT_SELECT_TOOL", &["tool"]));
        }
        let target_ids: Vec<BodyId> = ev.resolve_bodies(&targets).into_iter().map(|b| ev.ctx.bodies[b].id.clone()).collect();
        let mut changed = false;
        // Each target's pieces by their boxes' centres (which piece came from which target, once their ids are set).
        let mut centres: Vec<(BodyId, DVec3)> = Vec::new();
        // The edges the split made (midpoints on none of the target's old ones): qCreatedBy(id, EDGE).
        let mut mids = Vec::new();
        for bi in ev.resolve_bodies(&targets).into_iter().rev() {
            let body = ev.ctx.bodies[bi].clone();
            let mut res = k::split(std::slice::from_ref(&body.shape), &tools, &Default::default())?;
            // The old edges with their boxes: a middle is measured only against the edges whose box holds it (a
            // distance to all of a 328-face board's edges at once took 0.9 s per split, cf-fix-pcb).
            // An edge the split left whole keeps its middle: only the others are measured.
            let mid = |e: &Shape| e.edge_info().ok().and_then(|info| e.point_at((info.first_param + info.last_param) * 0.5).ok());
            let old: Vec<Shape> = body.shape.edges();
            let old_mids: Vec<DVec3> = old.iter().filter_map(mid).collect();
            let boxes = std::cell::OnceCell::new();
            let on_old = |m: DVec3| {
                if old_mids.iter().any(|p| p.distance(m) < 1e-9) {
                    return true;
                }
                let Ok(v) = k::make_vertex(m) else { return false };
                let boxes: &Vec<(&Shape, k::BoundingBox)> = boxes.get_or_init(|| old.iter().filter_map(|e| e.bounding_box(false).ok().map(|b| (e, b))).collect());
                boxes.iter().any(|(e, b)| (0..3).all(|i| m[i] >= b.min[i] - 1e-7 && m[i] <= b.max[i] + 1e-7) && k::distance(&v, e).is_ok_and(|d| d.distance <= 1e-7))
            };
            let made: Vec<bool> = res
                .shape
                .edges()
                .iter()
                .map(|edge| {
                    let Some(m) = edge.edge_info().ok().and_then(|info| edge.point_at((info.first_param + info.last_param) * 0.5).ok()) else { return false };
                    let new = !on_old(m);
                    if new {
                        mids.push(m);
                    }
                    new
                })
                .collect();
            // A sheet comes apart where it was split: a body per piece (Onshape's split surfaces).
            if body.kind != BodyKind::Solid && made.contains(&true) {
                res.shape = sheet_pieces(&res.shape, &made)?;
            }
            let solids = res.shape.solids();
            changed |= solids.len() > 1 || body.kind != BodyKind::Solid;
            if let (Some(front), Some((o, n)), true) = (keep_side, plane, solids.len() > 1) {
                let kept: Vec<Shape> = solids.into_iter().filter(|s| s.bounding_box(false).is_ok_and(|b| ((b.center() - o).dot(n) > 0.0) == front)).collect();
                res.shape = Shape::compound(&kept);
            }
            centres.extend(res.shape.solids().iter().filter_map(|s| s.bounding_box(false).ok()).map(|b| (body.id.clone(), b.center())));
            let names = body.shape_names();
            let faces = ev.name_result(&[&names], &res.history, &res.shape, &BTreeMap::new(), None);
            ev.replace_body(bi, res.shape, faces);
        }
        // A tool that splits nothing: an info, nothing changes (live "fs
        // (10-25 06:15)").
        if !changed {
            ev.infos.push(super::error_text("SPLIT_NO_CHANGE"));
        }
        if !keep_tools {
            let ids: Vec<BodyId> = tool_bodies.iter().filter_map(|b| ev.ctx.bodies.get(*b).map(|x| x.id.clone())).collect();
            ev.ctx.bodies.retain(|b| !ids.contains(&b.id));
        }
        Ok((target_ids, plane, mids, centres))
    })?;
    // Split pieces are "created by" the split and by the original's creator
    // (std query.fs, qCreatedBy).
    let mut st = cx.state.lock();
    let pieces: Vec<BodyId> = st.ctx.bodies.iter().map(|b| b.id.clone()).filter(|b| !split_targets.contains(b) && st.body_creator(b).as_deref() == Some(&id[..])).collect();
    for piece in &pieces {
        st.record_merge(piece, &split_targets, &id);
        let centre = st.ctx.bodies.iter().find(|b| &b.id == piece).and_then(|b| b.shape.bounding_box(false).ok()).map(|b| b.center());
        if let Some((from, _)) = centre.and_then(|c| centres.iter().find(|(_, x)| x.distance(c) < 1e-9)) {
            st.split_from.insert(piece.clone(), from.clone());
        }
    }
    let all: Vec<BodyId> = split_targets.iter().chain(&pieces).cloned().collect();
    // Pieces of a hidden body (a sheet metal definition) are hidden as it is.
    if split_targets.iter().any(|t| st.hidden.contains(t)) {
        let before: HashSet<BodyId> = before_ids.iter().cloned().collect();
        let new: Vec<BodyId> = st.ctx.bodies.iter().map(|b| b.id.clone()).filter(|b| !before.contains(b)).collect();
        st.hidden.extend(new);
    }
    if !mids.is_empty() {
        // The bodies the new edges are on now (the pieces' ids are settled only after the operation).
        let on_edges: Vec<BodyId> = st
            .ctx
            .bodies
            .iter()
            .filter(|b| {
                b.shape.edges().iter().any(|e| {
                    e.edge_info().ok().and_then(|i| e.point_at((i.first_param + i.last_param) * 0.5).ok()).is_some_and(|m| mids.iter().any(|p| p.distance(m) < 1e-7))
                })
            })
            .map(|b| b.id.clone())
            .collect();
        let seq = on_edges.iter().filter_map(|b| st.touched.get(b).copied()).max().unwrap_or(u32::MAX);
        st.split_face_edges.push((id.clone(), on_edges, mids, seq));
    }
    st.splits.insert(id.clone(), (all, plane));
    Ok(Value::Undefined)
}

/// A sheet's faces as shells of the faces joined by edges not `cut` (by edge index): one per piece.
fn sheet_pieces(sheet: &Shape, cut: &[bool]) -> FResult<Shape> {
    let faces = sheet.faces();
    let mut group: Vec<usize> = (0..faces.len()).collect();
    fn root(g: &mut [usize], mut i: usize) -> usize {
        while g[i] != i {
            g[i] = g[g[i]];
            i = g[i];
        }
        i
    }
    for (e, fs) in sheet.edge_faces()?.iter().enumerate() {
        if cut.get(e).copied().unwrap_or(false) {
            continue;
        }
        for w in fs.windows(2) {
            let (a, b) = (root(&mut group, w[0] as usize), root(&mut group, w[1] as usize));
            group[a] = b;
        }
    }
    let mut pieces: BTreeMap<usize, Vec<Shape>> = BTreeMap::new();
    for (i, f) in faces.into_iter().enumerate() {
        pieces.entry(root(&mut group, i)).or_default().push(f);
    }
    let shells = pieces.into_values().map(|fs| Shape::shell(&fs)).collect::<Result<Vec<_>, _>>()?;
    Ok(Shape::compound(&shells))
}

fn op_draft(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let (cx, id, def) = args(i, &a)?;
    let faces = evaluate_refs(i, &cx, def.get("draftFaces"))?;
    let neutral = evaluate_refs(i, &cx, def.get("neutralPlane"))?;
    let angle = def_angle(i, &def, "angle")?;
    // std passes `pullVec` (extrude's draft flips it for the second side).
    let pull = vec3_of(def.get("pullVec")).or_else(|| vec3_of(def.get("pullDirection")));
    let (refillet, tangent) = (def_bool(&def, "reFillet", false), def_bool(&def, "tangentPropagation", false));
    run_op(i, &cx, &id, "DRAFT_FAILED", |ev| {
        if neutral.is_empty() {
            return Err(super::code_error_faulty("DRAFT_SELECT_NEUTRAL", &["neutralPlane"]));
        }
        if faces.is_empty() {
            return Err(super::code_error_faulty("DRAFT_SELECT_FACES", &["draftFaces"]));
        }
        let Some(np) = neutral.first().and_then(|r| ev.resolve_plane(r)) else { return Err(super::code_error("DRAFT_NO_NEUTRAL_PLANE")) };
        let dir = pull.unwrap_or(np.normal).normalize_or_zero();
        let groups = by_body(ev.resolve_faces(&faces));
        if super::trace_ops() {
            eprintln!("[fs op] draft faces {:?} -> {} bodies, angle {angle}, pull {dir:?}, neutral {:?}", faces.len(), groups.len(), np.origin);
        }
        // (Live L3: a 0° draft succeeds and changes nothing.)
        if angle.abs() < 1e-12 {
            return Ok(());
        }
        // As the Draft feature: Reapply fillet, tangent propagation, steep fillets beside the faces drafted with them.
        crate::regen::features::detail_a::draft_faces(ev, groups, dir, &kernel_plane(&np), angle, refillet, tangent)
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
            if leave_open {
                // Just remove the faces; each connected rest is its own sheet
                // (std sheet metal Bend: the fixed and the moving side; a
                // solid opened up, cf-setback-fillet's corners).
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
                        // (Just made by replace_body: ours alone.)
                        if let Some(b) = Arc::get_mut(&mut ev.ctx.bodies[bi]) {
                            b.kind = BodyKind::Surface;
                        }
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
        // One straight span (degree 1, two poles) is a line, as Parasolid makes it (live: c2-hunting-knife's Bridging
        // curve 1 between two vertices, matched by position at both ends).
        if degree == 1 && poles.len() == 2 && !periodic && poles[0].distance(poles[1]) > 0.0 {
            return add_wire(ev, k::make_line(poles[0], poles[1])?);
        }
        add_wire(ev, k::make_bspline(&poles, weights.as_deref(), &knots, &mults, degree, periodic)?)
    })?;
    Ok(Value::Undefined)
}

fn knot_values(v: &Value) -> Vec<f64> {
    let num = |v: &Value| v.as_number().or_else(|| v.get("value").as_number());
    v.as_array().map(|a| a.iter().filter_map(num).collect()).unwrap_or_default()
}

/// A rational spline's weights: a surface's matrix row by row, a curve's array.
fn weight_values(c: &Value) -> Option<Vec<f64>> {
    c.get("isRational").as_bool().unwrap_or(false).then(|| c.get("weights").as_array().map(|rows| rows.iter().flat_map(|r| knot_values(r).into_iter().chain(r.as_number())).collect()).unwrap_or_default())
}

/// opCreateBSplineSurface: a sheet (or with `makeSolid` a solid, when it encloses a region) of a std `BSplineSurface`,
/// whole or inside `boundaryBSplineCurves` (2D curves in its (u, v) space). std's bSplineSurface has written out the
/// knots and the periodic overlap.
fn op_create_bspline_surface(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let (cx, id, def) = args(i, &a)?;
    let c = def.get("bSplineSurface");
    let rows: Vec<Vec<DVec3>> = c.get("controlPoints").as_array().map(|rows| rows.iter().map(|r| r.as_array().map(|r| r.iter().filter_map(vec3_of).collect()).unwrap_or_default()).collect()).unwrap_or_default();
    let (nu, nv) = (rows.len(), rows.first().map_or(0, Vec::len));
    let degree = |k: &str| c.get(k).as_number().unwrap_or(0.0) as u32;
    let surface = k::BSplineSurfaceData {
        poles: rows.iter().flatten().copied().collect(),
        nu,
        nv,
        weights: weight_values(c),
        u_knots: knot_values(c.get("uKnots")),
        v_knots: knot_values(c.get("vKnots")),
        u_degree: degree("uDegree"),
        v_degree: degree("vDegree"),
        u_periodic: c.get("isUPeriodic").as_bool().unwrap_or(false),
        v_periodic: c.get("isVPeriodic").as_bool().unwrap_or(false),
    };
    let boundary: Vec<k::BSplineCurve2dData> = def
        .get("boundaryBSplineCurves")
        .as_array()
        .map(|cs| {
            cs.iter()
                .map(|b| k::BSplineCurve2dData {
                    poles: b.get("controlPoints").as_array().map(|a| a.iter().filter_map(vec2_of).collect()).unwrap_or_default(),
                    weights: weight_values(b),
                    knots: knot_values(b.get("knots")),
                    degree: b.get("degree").as_number().unwrap_or(0.0) as u32,
                    periodic: b.get("isPeriodic").as_bool().unwrap_or(false),
                })
                .collect()
        })
        .unwrap_or_default();
    let make_solid = def_bool(&def, "makeSolid", false);
    run_op(i, &cx, &id, "CANNOT_MAKE_BSPLINESURFACE", |ev| {
        if rows.iter().any(|r| r.len() != nv) || nu * nv == 0 {
            return err("Invalid B-spline surface");
        }
        let shape = k::make_bspline_surface(&surface, &boundary, make_solid)?;
        let kind = if shape.solids().is_empty() { BodyKind::Surface } else { BodyKind::Solid };
        let names = crate::regen::features::bshared::new_face_names(ev, &shape);
        ev.apply_operation(vec![NamedShape::new(shape, names)], "NEW", None, kind)
    })?;
    Ok(Value::Undefined)
}

/// opMoveCurveBoundary: Trim curve's evaluator (trim to entities, extend by a distance or up to an entity), and std
/// sheet metal Loft's trim of a wire at points (`trimToPoints`; the piece by `helpPointPosition` kept).
fn op_move_curve_boundary(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let (cx, id, def) = args(i, &a)?;
    let wires = evaluate_refs(i, &cx, def.get("wires"))?;
    let flip = def_bool(&def, "flipHeuristics", false);
    let points: Vec<DVec3> = def.get("trimToPoints").as_array().map(|a| a.iter().filter_map(vec3_of).collect()).unwrap_or_default();
    if !points.is_empty() {
        let help = vec3_of(def.get("helpPointPosition"));
        run_op(i, &cx, &id, "MOVE_CURVE_BOUNDARY_TRIM_NO_INTERSECTION", |ev| {
            for bi in ev.resolve_bodies(&wires) {
                let shape = ev.ctx.bodies[bi].shape.clone();
                let tools = points.iter().map(|p| Ok(k::make_vertex(k::distance(&shape, &k::make_vertex(*p)?)?.point_a)?)).collect::<FResult<Vec<_>>>()?;
                crate::regen::features::curves::trim_wire_at(ev, bi, &tools, help, flip)?;
            }
            Ok(())
        })?;
        return Ok(Value::Undefined);
    }
    let mut p = ParamValues::default();
    p.set("wires", ParamValue::Query(wires));
    p.set("moveBoundaryType", ParamValue::Enum(def_enum(&def, "moveBoundaryType").map_or("TRIM".into(), |e| e.to_string())));
    p.set("endCondition", ParamValue::Enum(def_enum(&def, "endCondition").map_or("BLIND".into(), |e| e.to_string())));
    p.set("extensionShape", ParamValue::Enum(def_enum(&def, "extensionShape").map_or("CURVATURE".into(), |e| e.to_string())));
    p.set("extensionDistance", length_param(def_length_opt(&def, "extensionDistance").unwrap_or(0.0)));
    for key in ["trimTo", "extendTo", "helpPoint"] {
        let q = def.get(key);
        let refs = if q.is_undefined() { Vec::new() } else { evaluate_refs(i, &cx, q)? };
        p.set(key, ParamValue::Query(refs));
    }
    p.set("flipHeuristics", ParamValue::Bool(flip));
    run_op_with(i, &cx, &id, "MOVE_CURVE_BOUNDARY_FAILED", &p, crate::regen::features::curves::trim_curve)?;
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

/// @opCreateOutline (@internal; std sheet metal booleans cut a definition
/// face with it): each tool's outline on the target face's plane, what of
/// the plane lies inside the tool, as a sheet. Planar targets only (std also
/// takes cylinders and extruded surfaces); `offsetFaces` is not used.
fn op_create_outline(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let (cx, id, def) = args(i, &a)?;
    let tools = evaluate_refs(i, &cx, def.get("tools"))?;
    let target = evaluate_refs(i, &cx, def.get("target"))?;
    let offset_faces = evaluate_refs(i, &cx, def.get("offsetFaces"))?;
    run_op(i, &cx, &id, "REGEN_ERROR", |ev| {
        let Some(&(b, f)) = ev.resolve_faces(&target).first() else { return err("Select a face to outline on.") };
        let caps = ev.resolve_faces(&offset_faces);
        let face = ev.face_shape(b, f)?;
        let plane = face.face_plane().map_err(|e| FeatureError::new(e.to_string()))?;
        let mut made = Vec::new();
        for t in ev.resolve_bodies(&tools) {
            let tool = ev.ctx.bodies[t].shape.clone();
            let Ok(bb) = tool.bounding_box(false) else { continue };
            // A square of the plane round the tool's shadow; a curved face (a rolled wall: std's sheet metal boolean
            // outlines its tools trimmed to the thickened face) itself.
            let sheet = match &plane {
                Some(plane) => {
                    let (c, r) = (plane.to_local(bb.center()), bb.diagonal());
                    let corners: Vec<DVec3> = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)].iter().map(|(u, v)| plane.to_world(c + glam::DVec2::new(*u, *v) * r)).collect();
                    let wire = k::make_polyline(&corners, true).map_err(|e| FeatureError::new(e.to_string()))?;
                    k::make_face_from_wire(&wire, true).map_err(|e| FeatureError::new(e.to_string()))?
                }
                None => face.clone(),
            };
            let own: Vec<Shape> = caps.iter().filter(|(cb, _)| *cb == t).filter_map(|&(cb, cf)| ev.face_shape(cb, cf).ok()).collect();
            // On a curved face, the tool's cap lying on it is its section there (the Common with the rolled face took
            // 40-70 ms: model-plus's Extrude 4).
            let on_face = || own.iter().find(|c| plane.is_none() && cap_on_face(c, &face)).and_then(|c| oriented_like(c, &face));
            let inside = match on_face() {
                Some(cap) => Shape::compound(&[cap]),
                None => k::boolean(k::BooleanOp::Common, &[sheet], &[tool], &k::BooleanOptions::default()).map_err(|e| FeatureError::new(format!("Failed to outline the tool: {e}")))?.shape,
            };
            let inside = if inside.faces().is_empty() { inside } else { outline_shadow(&face, &inside, &own).unwrap_or(inside) };
            if !inside.faces().is_empty() {
                let names = fresh_names(ev, &inside);
                made.push(NamedShape { shape: inside, names: ShapeNames { faces: names, ..Default::default() } });
            }
        }
        if made.is_empty() {
            return Ok(());
        }
        add_bodies(ev, made, BodyKind::Surface)
    })?;
    Ok(Value::Undefined)
}

/// Whether face `cap` lies on `face`: its inner point on it, its vertices within 1e-5 (a thickened face's corners
/// come out 6e-7 off its edges).
fn cap_on_face(cap: &Shape, face: &Shape) -> bool {
    let on = |p: DVec3| face.project_point(p).is_ok_and(|pr| pr.distance < 1e-5);
    super::sheetmetal::inner_point(cap).is_some_and(|p| face.contains_point(p, 1e-7).unwrap_or(false)) && cap.vertices().iter().all(|v| v.vertex_point().is_ok_and(on))
}

/// `cap` facing the way `face` does there.
fn oriented_like(cap: &Shape, face: &Shape) -> Option<Shape> {
    let p = super::sheetmetal::inner_point(cap)?;
    let (a, b) = (cap.normal_at_point(p).ok()?, face.normal_at_point(p).ok()?);
    if a.dot(b) >= 0.0 { Some(cap.clone()) } else { cap.reversed().ok() }
}

/// A tool's outline on `face`: its section there with its cap faces (the thickened face's offsets it keeps)
/// brought onto the face along its normals, one region: a tool whose end slants across the sheet outlines its widest,
/// as Parasolid's (cf-sheet-metal-model-plus's Extrude 4: 5 mm along the flat wall, not 4.58). None when the caps add
/// nothing (a cut square to the sheet). On a curved face only a shadow holding the section and the others is taken
/// (two regions on a rolled surface fuse with a seam left across them).
fn outline_shadow(face: &Shape, section: &Shape, caps: &[Shape]) -> Option<Shape> {
    let plane = face.face_plane().ok()?;
    let area = section.area().ok()?;
    let mut shadows = Vec::new();
    for cap in caps {
        let p = super::sheetmetal::inner_point(cap)?;
        let on = face.project_point(p).ok()?.point;
        let shadow = match plane {
            Some(_) => cap.translated(on - p).ok()?,
            None => {
                // Offset back along its normals (a normal projection, exactly), rebuilt on the face's own surface.
                let d = (on - p).dot(cap.normal_at_point(p).ok()?.normalize_or_zero());
                if d.abs() < 1e-9 {
                    continue;
                }
                let back = k::offset_shape(cap, d, k::JoinType::Intersection).ok()?.shape;
                let wire = back.faces().first()?.wires().into_iter().next()?;
                // The loop's way round decides which side is the face's: the one holding the cap's foot.
                [Some(wire.clone()), wire.reversed().ok()].into_iter().flatten().filter_map(|w| k::make_face_on(face, &w).ok()).find(|f| f.contains_point(on, 1e-7).unwrap_or(false))?
            }
        };
        if shadow.area().ok()? > area * (1.0 + 1e-9) {
            shadows.push(shadow);
        }
    }
    if shadows.is_empty() {
        return None;
    }
    if plane.is_none() {
        let widest = shadows.iter().max_by(|a, b| a.area().unwrap_or(0.0).total_cmp(&b.area().unwrap_or(0.0)))?;
        let holds = |f: &Shape| f.faces().iter().all(|g| g.vertices().iter().filter_map(|v| v.vertex_point().ok()).chain(super::sheetmetal::inner_point(g)).all(|p| widest.contains_point(p, 1e-6).unwrap_or(false)));
        return (holds(section) && shadows.iter().all(holds)).then(|| oriented_like(widest, face)).flatten().map(|w| Shape::compound(&[w]));
    }
    let fused = k::boolean(k::BooleanOp::Fuse, std::slice::from_ref(section), &shadows, &k::BooleanOptions::default()).ok()?.shape;
    let fused = k::unify_edges(&k::unify_same_domain(&fused).ok()?.shape).ok()?.shape;
    let clipped = k::boolean(k::BooleanOp::Common, &[fused], std::slice::from_ref(face), &k::BooleanOptions::default()).ok()?.shape;
    (!clipped.faces().is_empty()).then_some(clipped)
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
            Err(e) if own_existing && e.message == crate::regen::features::split::NO_SPLIT => {}
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
        Ok((made, mids, tools, pieces, owners))
    })?;
    let (new_edges, mids, tools, pieces, owners) = new_edges;
    let mut st = cx.state.lock();
    if own_existing {
        // Its edges derive from this split even where it changed nothing
        // (tracking keeps what operations since its start made or changed).
        let seq = st.next_seq;
        for b in &owners {
            st.touched.insert(b.clone(), seq);
        }
    }
    st.split_tools.extend(tools);
    st.split_face_pieces.insert(id.clone(), pieces);
    if !mids.is_empty() {
        let seq = owners.iter().filter_map(|b| st.touched.get(b).copied()).max().unwrap_or(u32::MAX);
        st.split_face_edges.push((id.clone(), owners.clone(), mids, seq));
    }
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
    if def_enum(&def, "extendMethod").is_some_and(|m| &*m == "EXTEND_TO_SURFACE") {
        return super::edge_change::extend_to_surface(i, &cx, &id, &def);
    }
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
    let mut boundary = Vec::new();
    for (key, continuity) in [("edgesG0", "G0"), ("edgesG1", "G1"), ("edgesG2", "G2")] {
        let edges = evaluate_refs(i, &cx, def.get(key))?;
        if edges.is_empty() {
            continue;
        }
        boundary.extend(edges.iter().cloned());
        let mut item = ParamValues::default();
        item.set("entities", ParamValue::Query(edges));
        item.set("continuity", ParamValue::Enum(continuity.into()));
        items.push(item);
    }
    // fillSurface.fs passes its own definition: guideEntities, useSampling
    // and sampleSize; the documented field is guideVertices.
    let mut guides = evaluate_refs(i, &cx, def.get("guideVertices"))?;
    guides.extend(evaluate_refs(i, &cx, def.get("guideEntities"))?);
    let sampled = def.get("useSampling").as_bool() == Some(true);
    let mut p = ParamValues::default();
    p.set("surfaceOperationType", ParamValue::Enum("NEW".into()));
    p.set("edges", ParamValue::Array(items));
    p.set("addGuides", ParamValue::Bool(!guides.is_empty()));
    p.set("guideEntities", ParamValue::Query(guides));
    p.set("constraintMode", ParamValue::Enum(if sampled { "SAMPLED" } else { "PRECISE" }.into()));
    if let Some(n) = def.get("sampleSize").as_number() {
        p.set("sampleSize", ParamValue::Expr(noshape_units::ExprValue::new(n.to_string())));
    }
    // Its edges depend on the boundary they were built along (qDependency: std joins the patch to the sheet there).
    cx.state.lock().dependencies.insert(id.clone(), boundary);
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
    // (opOffsetFace moves the faces only: Move face's "Reapply fillet" is the feature's, not the operation's.)
    p.set("reFillet", ParamValue::Bool(false));
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
    p.set("oppositeSense", ParamValue::Bool(def.get("oppositeSense").as_bool() == Some(true)));
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
        (o.ctx.clone(), o.attributes().to_vec())
    };
    let (marker, moved, sketches) = run_op(i, &cx, &id, "REGEN_ERROR", |ev| {
        let mut moved: Vec<(BodyId, u32)> = Vec::new();
        for b in &source.bodies {
            let index = ev.ctx.bodies.len() as u32;
            moved.push((b.id.clone(), index));
            push_body(ev, BodyId { feature: ev.feature, index }, b.kind, b.shape.clone(), b.faces.clone());
        }
        // The sketches under ids of this operation (created by it, as its
        // bodies; the source's ids may be this studio's features'), hidden.
        let sketches = copy_sketches(ev, &id, source.sketches.iter().map(|s| (s.clone(), DMat4::IDENTITY)), true);
        Ok((ev.feature, moved, sketches))
    })?;
    let mut st = cx.state.lock();
    let feature = st.feature;
    let bits = st.op_bits(marker);
    own_sketches(&mut st, marker, &sketches);
    let merged = |r: Reference| match &r {
        Reference::Body(old) => match moved.iter().find(|(o, _)| o == old) {
            Some((_, index)) => Reference::Body(super::wrap_body_id(&BodyId { feature: marker, index: *index }, marker, bits, feature)),
            None => r,
        },
        Reference::Sketch(old) => sketches.iter().find(|(o, _)| o == old).map_or(r.clone(), |(_, n)| Reference::Sketch(*n)),
        Reference::SketchBody { sketch: old, entity } => sketches.iter().find(|(o, _)| o == old).map_or(r.clone(), |(_, n)| Reference::SketchBody { sketch: *n, entity: *entity }),
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

/// Copies of `sketches` (each moved by its matrix) under ids made from
/// operation `id` (marker ids: [`own_sketches`] makes them created by it);
/// hidden when `hide` or the source is. (source, copy) ids.
fn copy_sketches(ev: &mut EvalCtx, id: &[Arc<str>], sketches: impl Iterator<Item = (Arc<crate::regen::context::SketchResult>, DMat4)>, hide: bool) -> Vec<(crate::reference::FeatureId, crate::reference::FeatureId)> {
    let mut out = Vec::new();
    for (s, m) in sketches {
        let mut sub = id.to_vec();
        sub.push(Arc::from(format!("sketch{}", out.len())));
        let new = super::marker_of(&sub);
        let frame = crate::regen::eval::transform_frame(&s.frame, &m);
        ev.ctx.sketches.retain(|x| x.feature != new);
        ev.ctx.sketches.push(Arc::new(crate::regen::context::SketchResult { feature: new, frame, ..(*s).clone() }));
        if hide || ev.ctx.hidden_sketches.contains(&s.feature) {
            ev.ctx.hidden_sketches.push(new);
        }
        out.push((s.feature, new));
    }
    out
}

/// A sketch copy keeps its source's attributes, the sketch's and its bodies'.
fn copy_sketch_attributes(st: &mut super::CtxState, source: crate::reference::FeatureId, copy: crate::reference::FeatureId) {
    let copied: Vec<(Reference, Value)> = st
        .attributes()
        .iter()
        .filter_map(|(r, v)| match r {
            Reference::Sketch(f) if *f == source => Some((Reference::Sketch(copy), v.clone())),
            Reference::SketchBody { sketch, entity } if *sketch == source => Some((Reference::SketchBody { sketch: copy, entity: *entity }, v.clone())),
            _ => None,
        })
        .collect();
    for (r, v) in copied {
        st.push_attribute(r, v);
    }
}

/// The sketch copies' ids name operation `marker` (qCreatedBy finds them).
fn own_sketches(st: &mut super::CtxState, marker: crate::reference::FeatureId, copies: &[(crate::reference::FeatureId, crate::reference::FeatureId)]) {
    if let Some(op) = st.ops.get(&marker).cloned() {
        for (_, new) in copies {
            st.alias_op(*new, op.clone());
        }
    }
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

/// The wrapped faces' normals the destination's way (`along`, std's orientWithDestination) or against it: a face's
/// own normal, an infinite cylinder's or cone's outward.
fn orient_wrapped(shape: &Shape, d: &k::Developable, face: Option<&Shape>, along: bool) -> FResult<Shape> {
    let Some(f) = shape.faces().into_iter().next() else { return Ok(shape.clone()) };
    let [u0, u1, v0, v1] = f.uv_bounds()?;
    let (u, v) = ((u0 + u1) * 0.5, (v0 + v1) * 0.5);
    let (p, n) = (f.point_at_uv(u, v)?, f.normal_at_uv(u, v)?);
    let radial = |origin: DVec3, axis: DVec3| {
        let r = p - origin;
        (r - axis * r.dot(axis)).normalize_or_zero()
    };
    let want = match (face, d) {
        (Some(df), _) => df.normal_at_point(p)?,
        (None, k::Developable::Plane { normal, .. }) => *normal,
        (None, k::Developable::Cylinder { origin, axis, .. }) => radial(*origin, *axis),
        (None, k::Developable::Cone { origin, axis, half_angle, .. }) => radial(*origin, *axis) * half_angle.cos() - *axis * half_angle.sin(),
    };
    if (n.dot(want) < 0.0) == along { Ok(shape.reversed()?) } else { Ok(shape.clone()) }
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
    let along = def_bool(&def, "orientWithDestination", true);
    run_op(i, &cx, &id, "REGEN_ERROR", |ev| {
        let mut faces: Vec<Shape> = ev.resolve_faces(&entities).into_iter().map(|(b, f)| ev.face_shape(b, f)).collect::<FResult<_>>()?;
        // Sketch regions too (std's sheet metal Hem rolls one onto its arc's cylinder).
        let regions: Vec<Reference> = entities.iter().filter(|r| matches!(r, Reference::SketchRegion { .. })).cloned().collect();
        if !regions.is_empty() {
            for p in ev.profiles(&regions)? {
                faces.extend(p.shape.faces());
            }
        }
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
        let mut res = k::wrap_faces(&faces, &s, src_ref, &d, dst_ref, &placement, 1e-7).map_err(|e| FeatureError::new(format!("Wrap failed: {e}")))?;
        let dst_shape = match dst_face {
            Some((bi, fi)) => Some(ev.face_shape(bi, fi)?),
            None => None,
        };
        res.shape = orient_wrapped(&res.shape, &d, dst_shape.as_ref(), along)?;
        let names: Vec<Name> = (0..res.shape.faces().len()).map(|n| Arc::new(TopoName::face(ev.feature, Role::New(n as u32)))).collect();
        let mut wrapped = NamedShape::new(res.shape.clone(), names);
        match wrap_type.as_str() {
            "IMPRINT" => {
                let Some((bi, _)) = dst_face else { return err("IMPRINT needs a destination face") };
                let body = ev.ctx.bodies[bi].clone();
                let edges: Vec<Shape> = res.shape.faces().iter().flat_map(|f| f.edges()).collect();
                let imp = k::imprint_kept(&body.shape, &edges).map_err(|e| FeatureError::new(format!("The wrapped edges could not split the destination: {e}")))?;
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
