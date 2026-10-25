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
use crate::display::{BodyKind, ComputedValue};
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
    // Coincident points make one hole only when they face the same way
    // (live L60: points on the top and front faces of a corner make two).
    let mut push = |l: Location| {
        if !out.iter().any(|o| o.point.distance(l.point) < 1e-9 && o.normal.normalize().dot(l.normal.normalize()) > 1.0 - 1e-9) {
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
    /// The tap drill diameter of a tapped hole (drawing callouts).
    tap_drill: Option<f64>,
    /// Cut with the tap drill in the last part: only with a fastener fit
    /// (hole.fs `hasClearance`); otherwise the whole hole is Diameter.
    clearance: bool,
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
    // hole.fs checks these before the locations.
    if cbore.is_some_and(|(cd, _)| cd <= diameter) {
        return crate::regen::eval::err_at(&["holeDiameterV3", "cBoreDiameterV3"], "The counterbore diameter must be larger than the hole diameter.");
    }
    if let Some((_, cdepth)) = cbore
        && ev.is("endStyleV2", "BLIND")
        && cdepth >= len("holeDepthV3")?
    {
        return crate::regen::eval::err_at(&["holeDepthV3", "cBoreDepthV3"], "The counterbore depth must be smaller than the hole depth.");
    }
    if csink.is_some_and(|(sd, _)| sd <= diameter) {
        return crate::regen::eval::err_at(&["holeDiameterV3", "cSinkDiameterV3"], "The countersink diameter must be larger than the hole diameter.");
    }
    Ok(HoleDims { diameter, tap_drill, clearance: vals.bool("hasClearance"), cbore, csink })
}

/// Hole cross-section as (radius, depth) points, depth into the material
/// from the hole start; `over` extends the tool above the start.
fn hole_profile(d: f64, dims: &HoleDims, h: f64, tip: f64, over: f64) -> FResult<Vec<DVec2>> {
    let r = d * 0.5;
    let mut pts: Vec<DVec2> = vec![v2(0.0, -over)];
    if let Some((cd, cdepth)) = dims.cbore {
        let cr = cd * 0.5;
        if cdepth > h {
            return crate::regen::eval::err_at(&["holeDepthV3", "cBoreDepthV3"], "The counterbore depth must be smaller than the hole depth.");
        }
        pts.extend([v2(cr, -over), v2(cr, cdepth), v2(r, cdepth)]);
    } else if let Some((sd, sa)) = dims.csink {
        let sr = sd * 0.5;
        if !(sa > 0.0 && sa < std::f64::consts::PI) {
            return err("Countersink angle must be between 0 and 180 degrees");
        }
        let c = (sr - r) / (sa * 0.5).tan();
        // Live L47: a countersink deeper than the hole is allowed; the cone
        // ends at the hole diameter with a flat bottom around the drill.
        if c >= h + tip {
            pts.extend([v2(sr, -over), v2(sr, 0.0), v2(r, c), v2(0.0, c)]);
            pts.dedup_by(|a, b| a.distance(*b) < 1e-12);
            return Ok(pts);
        }
        pts.extend([v2(sr, -over), v2(sr, 0.0), v2(r, c)]);
    } else {
        pts.push(v2(r, -over));
    }
    if h > pts.last().map_or(0.0, |p| p.y) {
        pts.push(v2(r, h));
    }
    pts.push(v2(0.0, (h + tip).max(pts.last().map_or(0.0, |p| p.y))));
    pts.dedup_by(|a, b| a.distance(*b) < 1e-12);
    Ok(pts)
}

/// Radius of a hole's entry: the counterbore's or countersink's, else the hole's.
fn entry_radius(dims: &HoleDims) -> f64 {
    let mut d = dims.diameter;
    if let Some((cd, _)) = dims.cbore {
        d = d.max(cd);
    }
    if let Some((sd, _)) = dims.csink {
        d = d.max(sd);
    }
    d * 0.5
}

/// Where the hole's entry circle (radius `r` about the axis through `p` along
/// `dir`) meets the parts: (deepest first contact, highest contact) as
/// parameters along `dir` from `p`. One section of the parts with the entry
/// cylinder; the curves are sampled, binned by angle (the first contact of
/// each ruling), and the deepest refined along its edge. None when part of
/// the circle misses the parts (the axis decides then).
fn entry_contacts(ev: &EvalCtx, bodies: &[usize], p: DVec3, dir: DVec3, r: f64, back: f64, extent: f64) -> Option<(f64, f64)> {
    let (u, v) = (perp(dir), dir.cross(perp(dir)).normalize());
    let frame = k::Plane::new(p - dir * back, dir, u);
    let tube = k::make_cylinder(&frame, r, back + extent * 2.0, std::f64::consts::TAU).ok()?;
    let t_of = |q: DVec3| (q - p).dot(dir);
    let angle_of = |q: DVec3| {
        let w = q - p;
        let a = w.dot(v).atan2(w.dot(u));
        if a < 0.0 { a + std::f64::consts::TAU } else { a }
    };
    const BINS: usize = 90;
    // Per angle bin: (first contact t, edge, edge parameter).
    let mut first: Vec<Option<(f64, k::Shape, f64)>> = vec![None; BINS];
    let mut top = f64::INFINITY;
    for &bi in bodies {
        let Ok(sec) = k::section(&ev.ctx.bodies[bi].shape, &tube) else { continue };
        for e in sec.shape.edges() {
            let Ok(info) = e.edge_info() else { continue };
            // Enough samples that every angle bin of a one-edge curve gets some.
            let n = 512;
            for i in 0..=n {
                let prm = info.first_param + (info.last_param - info.first_param) * i as f64 / n as f64;
                let Ok(q) = e.point_at(prm) else { continue };
                let t = t_of(q);
                top = top.min(t);
                let bin = ((angle_of(q) / std::f64::consts::TAU * BINS as f64) as usize).min(BINS - 1);
                if first[bin].as_ref().is_none_or(|f| t < f.0) {
                    first[bin] = Some((t, e.clone(), prm));
                }
            }
        }
    }
    if first.iter().any(|f| f.is_none()) || !top.is_finite() {
        return None;
    }
    let (t0, _, prm_e) = first.iter().flatten().max_by(|a, b| a.0.total_cmp(&b.0))?.clone();
    let (_, e0, _) = first.iter().flatten().max_by(|a, b| a.0.total_cmp(&b.0))?;
    // A flat entry square to the hole: every contact at one depth.
    let low = first.iter().flatten().map(|f| f.0).fold(f64::INFINITY, f64::min);
    if t0 - low < 1e-12 {
        return Some((t0, top));
    }
    // The section's curves are approximations (to about a micron): the
    // deepest first contact refined with exact rays about the best angle.
    let a0 = angle_of(e0.point_at(prm_e).ok()?);
    let ray = |a: f64| -> f64 {
        let q = p + (u * a.cos() + v * a.sin()) * r;
        let origin = q - dir * back;
        bodies
            .iter()
            .flat_map(|&bi| super::helpers_a::ray_intervals(&ev.ctx.bodies[bi].shape, origin, dir, back + extent * 2.0))
            .filter(|(_, b)| *b - back > 1e-9)
            .map(|(a, _)| (a - back).max(0.0))
            .fold(f64::INFINITY, f64::min)
    };
    let width = std::f64::consts::TAU / BINS as f64;
    let (mut lo, mut hi) = (a0 - width, a0 + width);
    let g = (5f64.sqrt() - 1.0) / 2.0;
    let mut best = f64::NEG_INFINITY;
    for _ in 0..48 {
        let (m1, m2) = (hi - g * (hi - lo), lo + g * (hi - lo));
        let (a1, a2) = (ray(m1), ray(m2));
        for x in [a1, a2] {
            if x.is_finite() {
                best = best.max(x);
            }
        }
        if a1 > a2 { hi = m2 } else { lo = m1 }
    }
    if !best.is_finite() {
        best = t0;
    }
    Some((best, top))
}

/// Live L51: a fastener fit on a hole through one part.
const FIT_INFO: &str = "Fastener fit will not be applied if there are fewer than two parts per hole.";
/// hole.fs HOLE_TAP_TOO_DEEP (its help link kept as text).
const TAP_TOO_DEEP: &str = "Tapped depth readjusted. Tapped depth input is greater than minimum computed Hole depth. Learn more";
/// hole.fs HOLE_CBORE_CSINK_VALUES_NON_STD.
const PIPE_TAP_HEADS: &str = "Counterbore and countersink values are non standard for tapered holes.";

pub fn hole(ev: &mut EvalCtx) -> FResult<()> {
    // hole.fs order: sizes, then the end and start entities, then the points.
    let dims = hole_dims(ev)?;
    if ev.is("endStyleV2", "UP_TO_ENTITY") && ev.query("endBoundEntity").is_empty() {
        return crate::regen::eval::err_at(&["endBoundEntity"], "Select an entity or mate connector to terminate the hole.");
    }
    if ev.is("startStyle", "PLANE") && ev.query("startBoundEntity").is_empty() {
        return crate::regen::eval::err_at(&["startBoundEntity"], "Select a start entity.");
    }
    let locs = locations(ev)?;
    if locs.is_empty() {
        return crate::regen::eval::err_at(&["locations"], "No hole points selected.");
    }
    if locs.len() > 500 {
        return crate::regen::eval::err_at(&["locations"], "Too many hole locations were selected, maximum: 500.");
    }
    // Onshape (hole.fs → getFitToleranceLimits): a Fit with tolerance /
    // Fit (tolerance only) whose class has no limits at this size fails the
    // feature with the table's message.
    let diameter_fit = match crate::fits::Tolerance::parse(ev.params.get("holeDiameterV3__tolerance")).map(|t| t.hole_fit(dims.diameter)) {
        Some(Err(e)) => return err(e.message()),
        Some(Ok(f)) => f,
        None => None,
    };
    let end = ev.enum_("endStyleV2").to_string();
    let start_style = ev.enum_("startStyle").to_string();
    let extent = ev.model_extent();
    let explicit_scope: Vec<usize> = ev.resolve_bodies(ev.query("scope")).into_iter().filter(|i| ev.ctx.bodies[*i].kind == BodyKind::Solid).collect();
    let candidates: Vec<usize> = if explicit_scope.is_empty() { (0..ev.ctx.bodies.len()).filter(|i| ev.ctx.bodies[*i].kind == BodyKind::Solid).collect() } else { explicit_scope.clone() };
    if candidates.is_empty() {
        return err("There are no parts to cut holes in");
    }
    let start_plane = if start_style == "PLANE" {
        let no_start = || crate::regen::eval::err_at(&["startBoundEntity"], "Select a start entity.");
        let Some(r) = ev.query("startBoundEntity").first() else { return no_start() };
        let Some(f) = ev.resolve_plane(r) else { return no_start() };
        // Live L54: every hole must be perpendicular to the start plane.
        if locs.iter().any(|l| l.normal.normalize().dot(f.normal.normalize()).abs() < 1.0 - 1e-9) {
            return err("Some Hole directions are not perpendicular to selected inputs.");
        }
        Some(f)
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
    // Locations that made no hole (hole.fs warns when only some miss).
    let mut missed = 0;
    // A fastener fit on a hole through fewer than two parts (live L51).
    let mut fit_unused = false;
    // A tapped depth deeper than a hole gets in its parts (hole.fs).
    let mut tap_too_deep = false;
    let tapped_input = if ev.params.bool("showTappedDepth") && end != "THROUGH" { ev.length("tappedDepthV3").ok() } else { None };
    // Depth of an Up to next / Up to entity hole, shown read-only.
    let mut up_to_depth_found: Option<f64> = None;
    // Each hole's start, direction and length (to the tip): the parts it
    // reaches, inside the scope or not, are the dialog's default scope.
    let mut spans: Vec<(DVec3, DVec3, f64)> = Vec::new();
    // Each location's start and direction, also for those that reach
    // nothing (the parts behind them may flip the hole).
    let mut probes: Vec<(DVec3, DVec3)> = Vec::new();
    for loc in &locs {
        let dir = if flip { loc.normal } else { -loc.normal }.normalize();
        // Ray from far behind the location so bodies around it are seen.
        let back = extent;
        let origin = loc.point - dir * back;
        let cast = |o: DVec3| -> Vec<(usize, Vec<(f64, f64)>)> {
            candidates
                .iter()
                .map(|&bi| {
                    let iv = ray_intervals(&ev.ctx.bodies[bi].shape, o, dir, back + extent * 2.0).into_iter().map(|(a, b)| (a - back, b - back)).collect::<Vec<_>>();
                    (bi, iv)
                })
                .collect()
        };
        let mut hits = cast(origin);
        // An axis beside the part with the hole still crossing it cuts a notch
        // (live "qa4 round 5 v2 s1 follow-up": a pattern instance at the
        // part's edge): rays across the hole's disk, each part's span from
        // the first entry to the last exit.
        if hits.iter().all(|(_, iv)| iv.is_empty()) {
            let (u, w) = (perp(dir), dir.cross(perp(dir)));
            let r = dims.diameter * 0.5;
            for f in [0.5, 0.98] {
                for k in 0..16 {
                    let a = k as f64 * std::f64::consts::TAU / 16.0;
                    for ((_, iv), (_, more)) in hits.iter_mut().zip(cast(origin + (u * a.cos() + w * a.sin()) * r * f)) {
                        if let (Some(first), Some(last)) = (more.first(), more.last()) {
                            let span = match iv.first() {
                                Some(&(a0, b0)) => (a0.min(first.0), b0.max(last.1)),
                                None => (first.0, last.1),
                            };
                            *iv = vec![span];
                        }
                    }
                }
            }
        }
        // Hole start (parameter along dir relative to the location).
        let mut axis_first = f64::INFINITY;
        let mut entry: Option<(f64, f64)> = None;
        let t_start = match (&start_plane, start_style.as_str()) {
            (Some(f), _) => match f.intersect_ray(loc.point, dir) {
                Some(q) => (q - loc.point).dot(dir),
                None => (f.origin - loc.point).dot(dir) / dir.dot(f.normal).abs().max(1e-12),
            },
            (None, "PART") => {
                let first = hits.iter().flat_map(|(_, iv)| iv.iter()).filter(|(_, b)| *b > 1e-9).map(|(a, _)| a.max(0.0)).fold(f64::INFINITY, f64::min);
                axis_first = first;
                if first.is_finite() {
                    // Live "kern (10-23 23:00)" H1-H5: the depth counts from
                    // where the whole entry (counterbore / countersink / hole
                    // diameter) is in the part: the deepest first contact on
                    // the entry circle (a Ø6 hole on a Ø40 cylinder starts
                    // 0.226 below its top line).
                    entry = entry_contacts(ev, &candidates, loc.point, dir, entry_radius(&dims), back, extent);
                    entry.map_or(first, |(deep, _)| deep.max(first))
                } else {
                    0.0
                }
            }
            _ => 0.0,
        };
        let s = loc.point + dir * t_start;
        probes.push((s, dir));
        // Intervals after the start, relative to it.
        let rel: Vec<(usize, Vec<(f64, f64)>)> = hits
            .iter()
            .map(|(bi, iv)| (*bi, iv.iter().map(|(a, b)| (a - t_start, b - t_start)).filter(|(_, b)| *b > 1e-9).map(|(a, b)| (a.max(0.0), b)).collect()))
            .collect();
        let hit_bodies: Vec<(usize, f64, f64)> = rel.iter().filter_map(|(bi, iv)| Some((*bi, iv.first()?.0, iv.last()?.1))).collect();
        if hit_bodies.is_empty() {
            missed += 1;
            continue;
        }
        let r_main = dims.diameter * 0.5;
        let (h, tip) = match end.as_str() {
            "BLIND" => (ev.length("holeDepthV3")?, tip_len(tip_angle, r_main)),
            "THROUGH" => (hit_bodies.iter().map(|x| x.2).fold(0.0, f64::max) + r_main.max(1e-4), 0.0),
            "UP_TO_NEXT" => {
                let next = rel.iter().flat_map(|(_, iv)| iv.iter()).map(|(_, b)| *b).fold(f64::INFINITY, f64::min);
                // Live L61: the tip's apex is at the entity minus the offset.
                let tip = tip_len(tip_angle, r_main);
                (next - tip_offset - if tip_offset != 0.0 { tip } else { 0.0 }, tip)
            }
            "UP_TO_ENTITY" => {
                let Some(r) = ev.query("endBoundEntity").first() else { return crate::regen::eval::err_at(&["endBoundEntity"], "Select an entity or mate connector to terminate the hole.") };
                let tip = tip_len(tip_angle, r_main);
                (up_to_depth(ev, r, s, dir)? - tip_offset - if tip_offset != 0.0 { tip } else { 0.0 }, tip)
            }
            other => return err(format!("Unsupported hole termination {other}")),
        };
        if h.is_nan() || h <= 1e-9 {
            return err("The hole has zero depth (check the termination and direction)");
        }
        // Live L61: with an offset the Depth field shows the shaft depth less
        // the offset (4.092 for a 6.092 shaft and a 2 mm offset).
        up_to_depth_found.get_or_insert(h - tip_offset);
        spans.push((s, dir, h + tip));
        // Overshoot above the start when that is outside every part; past
        // the part's highest point over the entry (a start below a sloped or
        // curved face).
        let over = {
            let o = r_main * 0.25;
            let q = s - dir * o * 0.5;
            let outside = candidates.iter().all(|&bi| !matches!(ev.ctx.bodies[bi].shape.classify_point(q, 1e-9), Ok(k::PointClass::Inside)));
            let above = match entry {
                Some((_, top)) if t_start > axis_first.min(top) + 1e-12 => t_start - axis_first.min(top),
                _ => 0.0,
            };
            if above > 1e-12 { above + o } else if outside { o } else { 0.0 }
        };
        // The hole for drawing callouts.
        {
            let (pid, _) = ht::hole_table_param(ev.params);
            let path = ev.params.lookup(pid);
            let iso = !ev.params.is("unitsSystem", "INCH");
            let thread = match path {
                [t, _, size, pitch, ..] if t == "Tapped" => Some(if iso { format!("{size}x{pitch}") } else { format!("{size}-{pitch}") }),
                _ => None,
            };
            let class_param = if iso { "isoThreadClassV2" } else { "ansiThreadClassV2" };
            let thread_class = (ev.params.bool("showThreadClassV2")).then(|| ev.params.lookup(class_param).last().cloned()).flatten();
            let tapped_depth = if ev.params.bool("showTappedDepth") && end != "THROUGH" { ev.length("tappedDepthV3").ok() } else { None };
            ev.ctx.holes.push(crate::regen::context::HoleRecord {
                feature: ev.feature,
                start: s,
                dir,
                diameter: dims.diameter,
                depth: (end != "THROUGH").then_some(h),
                through: end == "THROUGH",
                tap_drill: dims.tap_drill,
                tapped_depth,
                thread,
                thread_class,
                cbore: dims.cbore,
                csink: dims.csink,
                inch: !iso,
                diameter_fit: diameter_fit.clone(),
                exits: end == "THROUGH" || h + 1e-9 >= hit_bodies.iter().map(|x| x.2).fold(0.0, f64::max),
                parts: {
                    let mut v: Vec<(usize, f64, f64)> = hit_bodies.iter().filter(|x| x.1 < h + tip).cloned().collect();
                    v.sort_by(|a, b| a.1.total_cmp(&b.1));
                    v.iter().map(|x| ev.ctx.bodies[x.0].id.clone()).collect()
                },
                tip_angle,
            });
        }
        // Live H5: a bore tangent to a side face of the part (a Ø10
        // counterbore whose rim touches the front face along a line) is
        // invalid geometry for Parasolid.
        {
            let mut bores: Vec<(f64, f64, f64)> = vec![(r_main, dims.cbore.map_or(0.0, |c| c.1), h)];
            if let Some((cd, cdepth)) = dims.cbore {
                bores.push((cd * 0.5, -over, cdepth));
            }
            for &bi in &candidates {
                for f in ev.ctx.bodies[bi].shape.faces() {
                    let Ok(k::Surface::Plane(pl)) = f.face_surface() else { continue };
                    let n = pl.normal.normalize();
                    if n.dot(dir).abs() > 1e-9 {
                        continue;
                    }
                    let dist = (s - pl.origin).dot(n);
                    for &(r, a, b) in &bores {
                        if (dist.abs() - r).abs() > 1e-9 || b <= a {
                            continue;
                        }
                        let touches = (1..8).any(|i| {
                            let q = s + dir * (a + (b - a) * i as f64 / 8.0) - n * dist;
                            k::make_vertex(q).and_then(|v| k::distance(&f, &v)).is_ok_and(|d| d.distance < 1e-9)
                        });
                        if touches {
                            return err("Hole creates invalid geometry.");
                        }
                    }
                }
            }
        }
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
            missed += 1;
            continue;
        }
        any_hit = true;
        let ids = |v: &[(usize, f64, f64)]| -> Vec<BodyId> { v.iter().map(|x| ev.ctx.bodies[x.0].id.clone()).collect() };
        fit_unused |= dims.clearance && ordered.len() < 2;
        // The depth in the parts: the hole's length, or less where it leaves them.
        let in_part = h.min(ordered.iter().map(|x| x.2).fold(0.0, f64::max));
        tap_too_deep |= tapped_input.is_some_and(|td| td > in_part + 1e-9);
        match dims.tap_drill.filter(|_| dims.clearance) {
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
    // Live L57: the parts the holes reach (Blind 5 into the top of two
    // stacked parts: the top one; 15: both), whatever the scope.
    let mut reached: Vec<usize> = groups.keys().flatten().filter_map(|id| index_of_body(ev, id)).collect();
    for bi in (0..ev.ctx.bodies.len()).filter(|i| ev.ctx.bodies[*i].kind == BodyKind::Solid && !candidates.contains(i)) {
        let shape = &ev.ctx.bodies[bi].shape;
        if spans.iter().any(|(s, dir, len)| ray_intervals(shape, *s, *dir, *len).iter().any(|(a, b)| *b > 1e-9 && *a < *len)) {
            reached.push(bi);
        }
    }
    reached.sort_unstable();
    reached.dedup();
    ev.touched = reached.iter().map(|&i| ev.ctx.bodies[i].id.clone()).collect();
    // The parts behind the locations (hole.fs probes both ways).
    let back = extent * 2.0;
    ev.touched_behind = (0..ev.ctx.bodies.len())
        .filter(|i| ev.ctx.bodies[*i].kind == BodyKind::Solid && !reached.contains(i))
        .filter(|&bi| probes.iter().any(|(s, dir)| ray_intervals(&ev.ctx.bodies[bi].shape, *s, -*dir, back).iter().any(|(a, b)| *b > 1e-9 && *a < back)))
        .map(|i| ev.ctx.bodies[i].id.clone())
        .collect();
    if !any_hit {
        return crate::regen::eval::err_at(&["scope"], "None of the holes intersected a part.");
    }
    // Live L56: an empty Merge scope is an error (the dialog fills it with
    // the parts reached). Holes saved before that cut every part.
    if explicit_scope.is_empty() && !ev.params.bool("legacyAllParts") {
        return crate::regen::eval::err_at(&["scope"], "No target parts selected for hole.");
    }
    // Live L53: a location that makes no hole fails the whole feature.
    if missed > 0 {
        return err("Hole creates invalid geometry.");
    }
    if tap_too_deep {
        ev.infos.push(TAP_TOO_DEEP.into());
    }
    // A counterbore / countersink on a pipe tap.
    let (pid, _) = ht::hole_table_param(ev.params);
    if !ev.params.is("styleV2", "SIMPLE") && ev.params.lookup(pid).iter().any(|k| k.contains("Pipe Tap")) {
        ev.infos.push(PIPE_TAP_HEADS.into());
    }
    if fit_unused {
        ev.infos.push(FIT_INFO.into());
    }
    // Live L52/L61: a Diameter (or tap drill) other than the standard's.
    // Counterbore / countersink values don't count.
    if ev.params.bool(ht::TABLE_SYNCED_PARAM) {
        let (pid, table) = ht::hole_table_param(ev.params);
        if let Some(spec) = ht::table_values(table, &ht::complete_path(table, ev.params.lookup(pid))) {
            let off = |a: f64, b: f64| (a - b).abs() > 1e-9;
            let tap = matches!((dims.tap_drill, spec.tap_drill_diameter), (Some(a), Some(b)) if off(a, b));
            if off(dims.diameter, spec.hole_diameter) || tap {
                ev.infos.push("Parameters have been overridden. Check input.".into());
            }
        }
    }
    for (ids, tools) in groups {
        let targets: Vec<usize> = ids.iter().filter_map(|id| index_of_body(ev, id)).collect();
        if targets.is_empty() {
            continue;
        }
        // hole.fs: a failed cut is HOLE_CUT_FAIL, "Hole creates invalid
        // geometry." (live H5: a counterbore tangent to a side face; live
        // "qa4 round 5 v2 s1 follow-up": a hole tangent to a face, both
        // non-manifold).
        if let Err(e) = ev.boolean_into(&targets, tools, k::BooleanOp::Cut) {
            return Err(if ev.cancelled() { e } else { fe("Hole creates invalid geometry.") });
        }
    }
    // hole.fs: the computed Depth reads "Multiple" for several locations.
    if let Some(h) = up_to_depth_found.filter(|_| end == "UP_TO_NEXT" || end == "UP_TO_ENTITY") {
        let v = if locs.len() > 1 { ComputedValue::Text("Multiple".into()) } else { ComputedValue::Length(h) };
        ev.computed.push(("holeDepthComputedV3".into(), v));
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

/// Cosmetic external thread: validates the inputs; with "Split face" (Blind,
/// no undercut) the cylindrical face is split where the thread ends, as live
/// (behaviours.md "External thread": 3 → 4 faces, area unchanged).
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
    // Per edge: body, circle centre and axis, and the thread's cylindrical face.
    let mut threads: Vec<(BodyId, DVec3, DVec3, f64, usize)> = Vec::new();
    for (bi, ei) in edges {
        let e = ev.edge_shape(bi, ei)?;
        let (center, axis) = match e.edge_curve()? {
            k::Curve::Circle { radius: r, center, axis, .. } => {
                if let Some(r0) = radius
                    && (r0 - r).abs() > 1e-7
                {
                    return err("All thread edges must have the same diameter");
                }
                radius = Some(r);
                (center, axis.normalize())
            }
            _ => return err("Thread edges must be circular"),
        };
        if !ev.ctx.bodies[bi].shape.edge_faces().ok().and_then(|ef| ef.get(ei).map(|f| f.len() == 2)).unwrap_or(false) {
            return err("Thread edges must be edges of a solid");
        }
        let cyl = ev.ctx.bodies[bi].shape.ancestors(ShapeType::Edge, ei, ShapeType::Face).unwrap_or_default().into_iter().find(|&fi| {
            ev.ctx.bodies[bi].shape.sub_shape(ShapeType::Face, fi).ok().and_then(|f| f.face_surface().ok()).is_some_and(|s| matches!(s, k::Surface::Cylinder { .. }))
        });
        let Some(cyl) = cyl else {
            return err("Thread edges must bound a cylindrical face");
        };
        threads.push((ev.ctx.bodies[bi].id.clone(), center, axis, radius.unwrap_or_default(), cyl));
    }
    let path = ht::complete_path(ht::TABLE_EXTERNAL_THREAD, ev.params.lookup("branchOfStandard"));
    if ht::table_values(ht::TABLE_EXTERNAL_THREAD, &path).is_none() {
        return err("Select a thread standard and size");
    }
    if ev.is("depthType", "Blind") && ev.length("threadDepth")? <= 0.0 {
        return err("Thread length must be positive");
    }
    if ev.is("depthType", "Blind") && !ev.bool("addUndercut") && ev.bool("splitFace") {
        let depth = ev.length("threadDepth")?;
        for (id, center, axis, r, fi) in threads {
            split_thread_face(ev, &id, center, axis, r, fi, depth)?;
        }
    }
    Ok(())
}

/// Split the cylindrical face `fi` of body `id` by the circle `depth` from
/// the thread's start edge (centre `center`, axis `axis`), measured into the
/// face. A thread that reaches the face's far end leaves it whole.
fn split_thread_face(ev: &mut EvalCtx, id: &BodyId, center: DVec3, axis: DVec3, r: f64, fi: usize, depth: f64) -> FResult<()> {
    let Some(bi) = index_of_body(ev, id) else { return Ok(()) };
    let body = ev.ctx.bodies[bi].clone();
    let face = body.shape.sub_shape(ShapeType::Face, fi).map_err(kerr("Thread failed"))?;
    // The face's extent along the axis from the start edge (signed).
    let ts: Vec<f64> = face.vertices().iter().filter_map(|v| v.vertex_point().ok()).map(|p| (p - center).dot(axis)).collect();
    let far = ts.iter().copied().fold(0.0_f64, |a, t| if t.abs() > a.abs() { t } else { a });
    if far.abs() <= depth + 1e-9 {
        return Ok(());
    }
    let circle = k::make_circle(center + axis * depth.copysign(far), axis, r).map_err(kerr("Thread failed"))?;
    let res = k::imprint(&body.shape, &[circle]).map_err(kerr("Thread failed"))?;
    let res_solid = if body.kind == BodyKind::Solid { res.shape.solids().into_iter().next().unwrap_or(res.shape.clone()) } else { res.shape.clone() };
    if res_solid.count(ShapeType::Face) <= body.shape.count(ShapeType::Face) {
        return Ok(());
    }
    let empty = crate::naming::ShapeNames::default();
    let names = ev.name_result(&[&body.shape_names(), &empty], &res.history, &res.shape, &BTreeMap::new(), None);
    let names = res_solid
        .faces()
        .iter()
        .map(|f| res.shape.index_of(f, ShapeType::Face).and_then(|i| names.get(i).cloned()).unwrap_or_else(|| std::sync::Arc::new(crate::reference::TopoName::face(ev.feature, Role::New(0)))))
        .collect();
    let names = dedup_names(ev, names);
    ev.replace_body(bi, res_solid, names);
    Ok(())
}
