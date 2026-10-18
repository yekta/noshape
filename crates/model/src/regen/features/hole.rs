//! Hole (simple / counterbore / countersink; blind, up to next, up to entity,
//! through all; start from part, sketch plane or selected plane) and External
//! thread (cosmetic).
//!
//! Dimensions: the numeric parameters are used as they are. The Hole dialog
//! copies the Standard table's values into them when the Standard changes
//! (`hole_tables::apply_table_to_params`, which also sets the hidden
//! `tableSynced` flag), so user edits made afterwards win. For holes saved
//! before that edit logic (a table path but no `tableSynced`), table values
//! replace every dimension that still holds its schema default. Tapped holes are cosmetic: the tap drill is
//! cut and no thread geometry is modeled. With a fastener fit and several
//! parts on a hole's axis, the deepest part gets the tap drill and the
//! others the clearance diameter (Onshape's multi-part tapped hole).

use super::helpers_a::*;
use crate::display::BodyKind;
use crate::features::hole_tables as ht;
use crate::reference::{BodyId, Reference, Role, Source};
use crate::regen::eval::{EvalCtx, FResult, NamedShape, err};
use glam::{DVec2, DVec3};
use noshape_kernel as k;
use noshape_kernel::ShapeType;
use std::collections::BTreeMap;

/// A hole location: point, outward normal (hole goes along `-normal`) and
/// the persistent source used to name the tool faces.
struct Location {
    point: DVec3,
    normal: DVec3,
    source: LocSource,
}

#[derive(Clone)]
enum LocSource {
    Sketch { sketch: crate::reference::FeatureId, entity: u32 },
    Index(u32),
}

impl LocSource {
    fn face_source(&self, ord: u32) -> Source {
        match self {
            LocSource::Sketch { sketch, entity } => Source::Sketch { sketch: *sketch, entity: *entity, segment: ord as u16 },
            LocSource::Index(i) => Source::Index(i * 64 + ord),
        }
    }
}

fn locations(ev: &EvalCtx) -> FResult<Vec<Location>> {
    let mut out: Vec<Location> = Vec::new();
    let refs = ev.query("locations");
    let mut push = |l: Location| {
        if !out.iter().any(|o| o.point.distance(l.point) < 1e-9) {
            out.push(l);
        }
    };
    for (i, r) in refs.iter().enumerate() {
        match r {
            Reference::SketchEntity { sketch, entity } => {
                let Some(s) = ev.ctx.sketch(*sketch) else { return err("A hole location sketch no longer exists") };
                let id = noshape_sketch::EntityId(*entity);
                let p2 = match s.sketch.point(id) {
                    Some(p) => Some(p),
                    None => match s.sketch.curve(id) {
                        Some(noshape_sketch::Curve2::Circle { center, .. }) | Some(noshape_sketch::Curve2::Arc { center, .. }) => Some(center),
                        _ => None,
                    },
                };
                let Some(p2) = p2 else { return err("A hole location no longer exists") };
                push(Location { point: s.frame.to_world(p2), normal: s.frame.normal, source: LocSource::Sketch { sketch: *sketch, entity: *entity } });
            }
            Reference::Sketch(sketch) => {
                // All standalone points and circle centers of the sketch.
                let Some(s) = ev.ctx.sketch(*sketch) else { return err("A hole location sketch no longer exists") };
                let mut used = std::collections::BTreeSet::new();
                let mut centers = std::collections::BTreeSet::new();
                for e in s.sketch.entities() {
                    let refs = e.point_refs();
                    if e.kind().has_center()
                        && let Some(c) = refs.first()
                    {
                        centers.insert(*c);
                    }
                    used.extend(refs);
                }
                for e in s.sketch.entities() {
                    if e.kind() == noshape_sketch::EntityKind::Point
                        && !e.construction
                        && (!used.contains(&e.id) || centers.contains(&e.id))
                        && let Some(p2) = s.sketch.point(e.id)
                    {
                        push(Location { point: s.frame.to_world(p2), normal: s.frame.normal, source: LocSource::Sketch { sketch: *sketch, entity: e.id.0 } });
                    }
                }
            }
            Reference::MateConnector { .. } | Reference::Implicit { .. } => {
                let c = ev.resolve_connector(r).ok_or_else(|| fe("A hole location mate connector no longer exists"))?;
                push(Location { point: c.origin, normal: c.z, source: LocSource::Index(i as u32) });
            }
            Reference::Vertex(n) => {
                let p = ev.resolve_vertex_point(r).ok_or_else(|| fe("A hole location vertex no longer exists"))?;
                // Normal of a planar face at the vertex.
                let mut normal = None;
                for b in &ev.ctx.bodies {
                    let Some(vi) = b.find_vertices(n).first().copied() else { continue };
                    if let Ok(vf) = b.shape.vertex_faces() {
                        for fi in vf.get(vi).cloned().unwrap_or_default() {
                            if let Ok(f) = b.shape.sub_shape(ShapeType::Face, fi as usize)
                                && let Ok(Some(pl)) = f.face_plane()
                            {
                                normal = Some(pl.unit_normal());
                                break;
                            }
                        }
                    }
                    break;
                }
                push(Location { point: p, normal: normal.unwrap_or(DVec3::Z), source: LocSource::Index(i as u32) });
            }
            _ => {}
        }
    }
    Ok(out)
}

/// Whether a numeric parameter still holds its schema default.
fn is_default(ev: &EvalCtx, feature: &str, id: &str) -> bool {
    let Some(spec) = crate::features::get(feature).and_then(|t| t.param(id)) else { return false };
    match (spec.default_value(ev.units), ev.params.expr(id)) {
        (crate::params::ParamValue::Expr(d), Some(e)) => d.text.trim() == e.text.trim(),
        _ => false,
    }
}

struct HoleDims {
    diameter: f64,
    tap_drill: Option<f64>,
    cbore: Option<(f64, f64)>,
    csink: Option<(f64, f64)>,
}

fn hole_dims(ev: &EvalCtx) -> FResult<HoleDims> {
    let style = ev.enum_("styleV2").to_string();
    let (pid, _) = ht::hole_table_param(ev.params);
    // The dialog copies table values into the parameters
    // (`hole_tables::apply_table_to_params`); those are used as they are.
    // Holes saved before that edit logic existed take table values for every
    // dimension that still holds its schema default.
    let legacy;
    let vals: &crate::params::ParamValues = if ev.params.bool(ht::TABLE_SYNCED_PARAM) || ev.params.lookup(pid).is_empty() {
        ev.params
    } else {
        let mut v = ev.params.clone();
        ht::apply_table_to_params(&mut v);
        for id in ht::HOLE_TABLE_DIMENSIONS {
            if !is_default(ev, "hole", id)
                && let Some(orig) = ev.params.get(id)
            {
                v.set(id, orig.clone());
            }
        }
        legacy = v;
        &legacy
    };
    let len = |id: &str| ev.eval_num_in(vals, id, crate::params::NumKind::Length);
    let diameter = len("holeDiameterV3")?;
    let tap_drill = if vals.bool("showTappedDepth") { Some(len("tapDrillDiameterV3")?) } else { None };
    let cbore = if style == "C_BORE" { Some((len("cBoreDiameterV3")?, len("cBoreDepthV3")?)) } else { None };
    let csink = if style == "C_SINK" { Some((len("cSinkDiameterV3")?, ev.eval_num_in(vals, "cSinkAngleV3", crate::params::NumKind::Angle)?)) } else { None };
    if diameter <= 0.0 {
        return err("Hole diameter must be positive");
    }
    Ok(HoleDims { diameter, tap_drill, cbore, csink })
}

/// Hole cross-section as (radius, depth) points, depth into the material
/// from the hole start; `over` extends the tool above the start.
fn hole_profile(d: f64, dims: &HoleDims, h: f64, tip: f64, over: f64) -> FResult<Vec<DVec2>> {
    let r = d * 0.5;
    let mut pts: Vec<DVec2> = vec![v2(0.0, -over)];
    if let Some((cd, cdepth)) = dims.cbore {
        let cr = cd * 0.5;
        if cr <= r {
            return err("Counterbore diameter must be larger than the hole diameter");
        }
        if cdepth >= h {
            return err("Counterbore is deeper than the hole");
        }
        pts.extend([v2(cr, -over), v2(cr, cdepth), v2(r, cdepth)]);
    } else if let Some((sd, sa)) = dims.csink {
        let sr = sd * 0.5;
        if sr <= r {
            return err("Countersink diameter must be larger than the hole diameter");
        }
        if !(sa > 0.0 && sa < std::f64::consts::PI) {
            return err("Countersink angle must be between 0 and 180 degrees");
        }
        let c = (sr - r) / (sa * 0.5).tan();
        if c >= h {
            return err("Countersink is deeper than the hole");
        }
        pts.extend([v2(sr, -over), v2(sr, 0.0), v2(r, c)]);
    } else {
        pts.push(v2(r, -over));
    }
    pts.push(v2(r, h));
    pts.push(v2(0.0, h + tip));
    pts.dedup_by(|a, b| a.distance(*b) < 1e-12);
    Ok(pts)
}

pub fn hole(ev: &mut EvalCtx) -> FResult<()> {
    let locs = locations(ev)?;
    if locs.is_empty() {
        return err("Select sketch points or mate connectors to place holes");
    }
    if locs.len() > 500 {
        return err("A hole feature can have at most 500 locations");
    }
    let dims = hole_dims(ev)?;
    let end = ev.enum_("endStyleV2").to_string();
    let start_style = ev.enum_("startStyle").to_string();
    let extent = ev.model_extent();
    let explicit_scope: Vec<usize> = ev.resolve_bodies(ev.query("scope")).into_iter().filter(|i| ev.ctx.bodies[*i].kind == BodyKind::Solid).collect();
    let candidates: Vec<usize> = if explicit_scope.is_empty() { (0..ev.ctx.bodies.len()).filter(|i| ev.ctx.bodies[*i].kind == BodyKind::Solid).collect() } else { explicit_scope.clone() };
    if candidates.is_empty() {
        return err("There are no parts to cut holes in");
    }
    let start_plane = if start_style == "PLANE" {
        let Some(r) = ev.query("startBoundEntity").first() else { return err("Select a hole start plane") };
        Some(ev.resolve_plane(r).ok_or_else(|| fe("The hole start plane could not be resolved"))?)
    } else {
        None
    };
    let tip_angle = match ev.enum_("tipAngleStyle") {
        "DEGREE118" => Some(118f64.to_radians()),
        "DEGREE135" => Some(135f64.to_radians()),
        "CUSTOM" => Some(ev.angle("tipAngleV3")?),
        _ => None,
    };
    let tip_offset = if (end == "UP_TO_NEXT" || end == "UP_TO_ENTITY") && ev.bool("offset") {
        let o = ev.length("offsetDistance")?;
        if ev.bool("oppositeOffsetDirection") { -o } else { o }
    } else {
        0.0
    };
    let flip = ev.bool("oppositeDirection");
    // Tools grouped by the body ids they cut.
    let mut groups: BTreeMap<Vec<BodyId>, Vec<NamedShape>> = BTreeMap::new();
    let mut any_hit = false;
    for loc in &locs {
        let dir = if flip { loc.normal } else { -loc.normal }.normalize();
        // Ray from far behind the location so bodies around it are seen.
        let back = extent;
        let origin = loc.point - dir * back;
        let hits: Vec<(usize, Vec<(f64, f64)>)> = candidates
            .iter()
            .map(|&bi| {
                let iv = ray_intervals(&ev.ctx.bodies[bi].shape, origin, dir, back + extent * 2.0).into_iter().map(|(a, b)| (a - back, b - back)).collect::<Vec<_>>();
                (bi, iv)
            })
            .collect();
        // Hole start (parameter along dir relative to the location).
        let t_start = match (&start_plane, start_style.as_str()) {
            (Some(f), _) => match f.intersect_ray(loc.point, dir) {
                Some(q) => (q - loc.point).dot(dir),
                None => (f.origin - loc.point).dot(dir) / dir.dot(f.normal).abs().max(1e-12),
            },
            (None, "PART") => {
                let first = hits.iter().flat_map(|(_, iv)| iv.iter()).filter(|(_, b)| *b > 1e-9).map(|(a, _)| a.max(0.0)).fold(f64::INFINITY, f64::min);
                if first.is_finite() { first } else { 0.0 }
            }
            _ => 0.0,
        };
        let s = loc.point + dir * t_start;
        // Intervals after the start, relative to it.
        let rel: Vec<(usize, Vec<(f64, f64)>)> = hits
            .iter()
            .map(|(bi, iv)| (*bi, iv.iter().map(|(a, b)| (a - t_start, b - t_start)).filter(|(_, b)| *b > 1e-9).map(|(a, b)| (a.max(0.0), b)).collect()))
            .collect();
        let hit_bodies: Vec<(usize, f64, f64)> = rel.iter().filter_map(|(bi, iv)| Some((*bi, iv.first()?.0, iv.last()?.1))).collect();
        if hit_bodies.is_empty() {
            continue;
        }
        any_hit = true;
        let r_main = dims.diameter * 0.5;
        let (h, tip) = match end.as_str() {
            "BLIND" => (ev.length("holeDepthV3")?, tip_len(tip_angle, r_main)),
            "THROUGH" => (hit_bodies.iter().map(|x| x.2).fold(0.0, f64::max) + r_main.max(1e-4), 0.0),
            "UP_TO_NEXT" => {
                let next = rel.iter().flat_map(|(_, iv)| iv.iter()).map(|(_, b)| *b).fold(f64::INFINITY, f64::min);
                (next - tip_offset, tip_len(tip_angle, r_main))
            }
            "UP_TO_ENTITY" => {
                let Some(r) = ev.query("endBoundEntity").first() else { return err("Select an entity to end the hole at") };
                (up_to_depth(ev, r, s, dir)? - tip_offset, tip_len(tip_angle, r_main))
            }
            other => return err(format!("Unsupported hole termination {other}")),
        };
        if h.is_nan() || h <= 1e-9 {
            return err("The hole has zero depth (check the termination and direction)");
        }
        // Overshoot above the start when that is outside every part.
        let over = {
            let o = r_main * 0.25;
            let q = s - dir * o * 0.5;
            let outside = candidates.iter().all(|&bi| !matches!(ev.ctx.bodies[bi].shape.classify_point(q, 1e-9), Ok(k::PointClass::Inside)));
            if outside { o } else { 0.0 }
        };
        let frame = k::Plane::new(s, -dir, perp(dir));
        let make_tool = |d: f64| -> FResult<NamedShape> {
            let pts = hole_profile(d, &dims, h, tip, over)?;
            let tool = k::revolve_profile_tool(&pts, &frame).map_err(kerr("Hole failed"))?;
            let src = loc.source.clone();
            let names = name_sorted(ev, &tool, |f| [f.centroid().map(|c| (c - s).dot(dir)).unwrap_or(0.0), f.area().unwrap_or(0.0)], |ord| Role::Side(src.face_source(ord)));
            Ok(NamedShape::new(tool, names))
        };
        // Bodies along the hole, ordered by depth.
        let mut ordered = hit_bodies.clone();
        ordered.sort_by(|a, b| a.1.total_cmp(&b.1));
        ordered.retain(|x| x.1 < h + tip);
        if ordered.is_empty() {
            continue;
        }
        let ids = |v: &[(usize, f64, f64)]| -> Vec<BodyId> { v.iter().map(|x| ev.ctx.bodies[x.0].id.clone()).collect() };
        match dims.tap_drill {
            Some(td) if (td - dims.diameter).abs() > 1e-9 && ordered.len() > 1 => {
                let (front, last) = ordered.split_at(ordered.len() - 1);
                let a = make_tool(dims.diameter)?;
                let b = make_tool(td)?;
                groups.entry(ids(front)).or_default().push(a);
                groups.entry(ids(last)).or_default().push(b);
            }
            Some(td) => {
                let t = make_tool(td)?;
                groups.entry(ids(&ordered)).or_default().push(t);
            }
            None => {
                let t = make_tool(dims.diameter)?;
                groups.entry(ids(&ordered)).or_default().push(t);
            }
        }
    }
    if !any_hit {
        return err(if explicit_scope.is_empty() { "The holes don't hit any part (try Opposite direction)" } else { "The holes don't hit any part in the merge scope" });
    }
    for (ids, tools) in groups {
        let targets: Vec<usize> = ids.iter().filter_map(|id| index_of_body(ev, id)).collect();
        if targets.is_empty() {
            continue;
        }
        ev.boolean_into(&targets, tools, k::BooleanOp::Cut).map_err(|e| fe(format!("Hole cut failed: {}", e.message)))?;
    }
    Ok(())
}

fn tip_len(angle: Option<f64>, r: f64) -> f64 {
    match angle {
        Some(a) if a > 1e-6 && a < std::f64::consts::PI - 1e-6 => r / (a * 0.5).tan(),
        _ => 0.0,
    }
}

/// Distance along `dir` from `s` to an "up to" entity.
fn up_to_depth(ev: &EvalCtx, r: &Reference, s: DVec3, dir: DVec3) -> FResult<f64> {
    if let Reference::Face(_) = r {
        let (bi, fi) = *ev.resolve_faces(std::slice::from_ref(r)).first().ok_or_else(|| fe("The hole end face no longer exists"))?;
        let face = ev.face_shape(bi, fi)?;
        if let Some(pl) = face.face_plane()? {
            let n = pl.unit_normal();
            let den = dir.dot(n);
            if den.abs() < 1e-12 {
                return err("The hole end face is parallel to the hole");
            }
            return Ok((pl.origin - s).dot(n) / den);
        }
        let line = k::make_line(s - dir * 1e-9, s + dir * ev.model_extent() * 2.0)?;
        let sec = k::section(&line, &face)?;
        let t = sec.shape.vertices().iter().filter_map(|v| v.vertex_point().ok()).map(|p| (p - s).dot(dir)).filter(|t| *t > 1e-9).fold(f64::INFINITY, f64::min);
        if t.is_finite() {
            return Ok(t);
        }
        return err("The hole doesn't reach the end face");
    }
    if let Some(p) = ev.resolve_vertex_point(r) {
        return Ok((p - s).dot(dir));
    }
    if let Some(f) = ev.resolve_plane(r)
        && let Some(q) = f.intersect_ray(s, dir)
    {
        return Ok((q - s).dot(dir));
    }
    err("The hole end entity could not be resolved")
}

// ------------------------------------------------------------------ external thread

/// Cosmetic external thread: validates the inputs, no geometry change.
pub fn external_thread(ev: &mut EvalCtx) -> FResult<()> {
    let refs = ev.query("entities");
    if refs.is_empty() {
        return err("Select circular edges of cylinders to thread");
    }
    let edges = ev.resolve_edges(refs);
    if edges.is_empty() {
        return err("The selected edges no longer exist");
    }
    let mut radius: Option<f64> = None;
    for (bi, ei) in edges {
        let e = ev.edge_shape(bi, ei)?;
        match e.edge_curve()? {
            k::Curve::Circle { radius: r, .. } => {
                if let Some(r0) = radius
                    && (r0 - r).abs() > 1e-7
                {
                    return err("All thread edges must have the same diameter");
                }
                radius = Some(r);
            }
            _ => return err("Thread edges must be circular"),
        }
        if !ev.ctx.bodies[bi].shape.edge_faces().ok().and_then(|ef| ef.get(ei).map(|f| f.len() == 2)).unwrap_or(false) {
            return err("Thread edges must be edges of a solid");
        }
        let cyl = ev.ctx.bodies[bi].shape.ancestors(ShapeType::Edge, ei, ShapeType::Face).unwrap_or_default().into_iter().any(|fi| {
            ev.ctx.bodies[bi].shape.sub_shape(ShapeType::Face, fi).ok().and_then(|f| f.face_surface().ok()).is_some_and(|s| matches!(s, k::Surface::Cylinder { .. }))
        });
        if !cyl {
            return err("Thread edges must bound a cylindrical face");
        }
    }
    let path = ht::complete_path(ht::TABLE_EXTERNAL_THREAD, ev.params.lookup("branchOfStandard"));
    if ht::table_values(ht::TABLE_EXTERNAL_THREAD, &path).is_none() {
        return err("Select a thread standard and size");
    }
    if ev.is("depthType", "Blind") && ev.length("threadDepth")? <= 0.0 {
        return err("Thread length must be positive");
    }
    Ok(())
}
