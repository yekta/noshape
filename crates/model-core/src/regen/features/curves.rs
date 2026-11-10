//! Curve features (wire bodies): Helix, 3D fit spline, Projected curve,
//! Bridging curve, Composite curve, Intersection curve, Trim curve,
//! Isoparametric curve, Edit curve, Routing curve, Offset curve; Isocline
//! reports a clear error.

use super::bshared::{add_wire_bodies, canonical_x, chain_wires, curve_edges, extent_along, face_shapes, order_chain, replace_wire_body, sample_chain, sketch_edge};
use crate::display::BodyKind;
use crate::naming::ShapeNames;
use crate::reference::Reference;
use crate::regen::eval::{EvalCtx, FResult, FeatureError, err, err_at};
use glam::{DQuat, DVec3};
use noshape_kernel as k;
use noshape_kernel::{Shape, ShapeType};
use std::collections::BTreeMap;
use std::f64::consts::{PI, TAU};

// ------------------------------------------------------------------ helix

/// A helix's frame as helix.fs's local coordinate system: where it starts (on its axis), its way (Opposite
/// direction applied), its start direction and radius, its taper, and the face's height for Cylinder / Cone.
pub(crate) fn helix_frame(ev: &EvalCtx) -> FResult<(DVec3, DVec3, DVec3, f64, f64, Option<f64>)> {
    let axis_type = ev.enum_("axisType").to_string();
    let opposite = ev.bool("oppositeDirection");
    Ok(match axis_type.as_str() {
        "AXIS" => {
            // helix.fs: "Select an object with an axis." on the axis field.
            let no_axis = || crate::regen::eval::err_at(&["axis"], "Select an object with an axis.");
            let Some(r) = ev.query("axis").first() else { return no_axis() };
            let Some((o, d)) = ev.resolve_axis(r) else { return no_axis() };
            let opp_origin = ev.bool("oppositeOrigin");
            // Axis start at one end of a linear edge.
            let mut origin = o;
            let mut dir = d;
            if let Some(&(bi, ei)) = ev.resolve_edges(std::slice::from_ref(r)).first()
                && let Ok(e) = ev.edge_shape(bi, ei)
                && let Ok((a, b)) = e.endpoints()
                && matches!(e.edge_curve(), Ok(k::Curve::Line { .. }))
            {
                origin = if opp_origin { b } else { a };
                dir = (b - a).normalize();
                if opp_origin {
                    dir = -dir;
                }
            } else if opp_origin {
                dir = -dir;
            }
            if opposite {
                dir = -dir;
            }
            (origin, dir, canonical_x(dir), ev.length("startRadius")?, 0.0, None)
        }
        "CIRCLE" => {
            let no_circle = || crate::regen::eval::err_at(&["edge"], "Select a circle or arc.");
            // A model edge or a sketch circle / arc (the field takes both, as live).
            let Some(e) = curve_edges(ev, ev.query("edge")).ok().and_then(|v| v.into_iter().next()) else { return no_circle() };
            let k::Curve::Circle { center, axis, x_dir, radius } = e.edge_curve()? else { return no_circle() };
            let z = if opposite { -axis } else { axis };
            (center, z.normalize(), x_dir, radius, 0.0, None)
        }
        _ => {
            let Some(&(bi, fi)) = ev.resolve_faces(ev.query("entities")).first() else { return crate::regen::eval::err_at(&["entities"], "Select a cone or cylinder.") };
            let face = ev.face_shape(bi, fi)?;
            match face.face_surface()? {
                k::Surface::Cylinder { origin, axis, x_dir, radius } => {
                    let a = axis.normalize();
                    let (lo, hi) = extent_along(&face, origin, a)?;
                    if opposite { (origin + a * hi, -a, x_dir, radius, 0.0, Some(hi - lo)) } else { (origin + a * lo, a, x_dir, radius, 0.0, Some(hi - lo)) }
                }
                k::Surface::Cone { origin, axis, x_dir, ref_radius, half_angle, .. } => {
                    let a = axis.normalize();
                    let (lo, hi) = extent_along(&face, origin, a)?;
                    let rad = |t: f64| ref_radius + t * half_angle.tan();
                    if opposite {
                        (origin + a * hi, -a, x_dir, rad(hi), -half_angle, Some(hi - lo))
                    } else {
                        (origin + a * lo, a, x_dir, rad(lo), half_angle, Some(hi - lo))
                    }
                }
                _ => return crate::regen::eval::err_at(&["entities"], "Select a cone or cylinder."),
            }
        }
    })
}

pub fn helix(ev: &mut EvalCtx) -> FResult<()> {
    let axis_type = ev.enum_("axisType").to_string();
    let (mut base, mut z, mut x, mut r0, mut taper, geom_height) = helix_frame(ev)?;
    // The axis frame's own x (the circle's), which helix.fs measures the end's angle from.
    let x0 = x;
    let left_handed = ev.is("handedness", "CCW");
    // Start condition.
    if ev.is("startType", "START_POINT") {
        // Live L34: no start point reads "Cannot resolve entities.", no field.
        let Some(p) = ev.query("startPoint").first().and_then(|r| ev.resolve_vertex_point(r)) else { return err("Cannot resolve entities.") };
        // helix.fs clampWorldPointToRadius: the point must lie in the plane
        // of the axis start; it is then moved radially onto the helix.
        if (p - base).dot(z).abs() > 1e-9 {
            return crate::regen::eval::err_at(&["startPoint"], "Start point and axis start should be coplanar normal to helix direction.");
        }
        let radial = p - base;
        if radial.length() < 1e-9 {
            return err("The start point lies on the helix axis");
        }
        if axis_type == "AXIS" {
            r0 = radial.length();
        }
        x = radial.normalize();
    } else {
        let a = ev.angle("startAngle")?;
        x = DQuat::from_axis_angle(z, a) * (x - z * z.dot(x)).normalize();
    }
    let path = ev.enum_("pathType").to_string();
    let (height, revs) = if path == "TURNS_PITCH" {
        let revs = ev.real("revolutions")?;
        (ev.length("helicalPitch")? * revs, revs)
    } else {
        let mut end_frac = None;
        let height = if ev.is("endType", "END_POINT") {
            let Some(p) = ev.query("endPoint").first().and_then(|r| ev.resolve_vertex_point(r)) else { return err("Cannot resolve entities.") };
            let mut h = (p - base).dot(z);
            // helix.fs: on a face the end point must be level with the face's
            // end (on an axis the point sets the height).
            if let Some(gh) = geom_height
                && (h.abs() - gh).abs() > 1e-9
            {
                return crate::regen::eval::err_at(&["endPoint"], "End point and axis end should be coplanar normal to helix direction.");
            }
            if h < 0.0 {
                // End point below the start: run the other way.
                z = -z;
                taper = -taper;
                h = -h;
            }
            let radial = p - (base + z * h);
            if axis_type == "AXIS" && radial.length() > 1e-9 && h > 1e-12 {
                taper = ((radial.length() - r0) / h).atan();
            }
            // As helix.fs: both points in the axis frame (fromWorld), angleBetween from its x, start to end. Its
            // coordinates are taken clean of float noise (to 1e-12 m), as Parasolid's are: an end point on the axis then
            // gives live's angle (c2-fan-blade-sweep's Helix 1, live 2440: the end's offset purely along the axis,
            // projected on the circle's x axis -1.8e-18 m, so π and 9.5 turns, not ours' 10.108 from 1e-17 m of noise).
            let clean = |v: DVec3| (v * 1e12).round() / 1e12;
            let y0 = z.cross(x0);
            let d = clean(p) - clean(base);
            let to_end = d.dot(y0).atan2(d.dot(x0));
            let to_start = x.dot(y0).atan2(x.dot(x0));
            let mut ang = to_end - to_start;
            if left_handed {
                ang = -ang;
            }
            end_frac = Some(ang.rem_euclid(TAU) / TAU);
            h
        } else {
            let h = match geom_height {
                Some(h) => h,
                None => ev.length("height")?,
            };
            if axis_type == "AXIS" && ev.bool("endRadToggle") && h > 1e-12 {
                taper = ((ev.length("endRadius")? - r0) / h).atan();
            }
            h
        };
        let raw = if path == "PITCH" {
            let p = ev.length("helicalPitch")?;
            if p < 1e-12 {
                return crate::regen::eval::err_at(&["helicalPitch"], "Error regenerating: a parameter is out of range.");
            }
            height / p
        } else {
            ev.real("revolutions")?
        };
        let revs = match end_frac {
            Some(frac) => {
                let round_up = if path == "PITCH" { ev.bool("roundUpPitch") } else { ev.bool("roundUpTurns") };
                let n = if round_up { (raw - frac).ceil() } else { (raw - frac).floor() }.max(0.0);
                let r = n + frac;
                let r = if r < 1e-9 { 1.0 } else { r };
                // helix.fs: an end point sets the revolutions and pitch shown
                // read-only in the dialog.
                ev.computed.push(("actualRevolutions".into(), crate::display::ComputedValue::Text(noshape_units::format_number(r, 6))));
                ev.computed.push(("actualPitch".into(), crate::display::ComputedValue::Length(height / r)));
                r
            }
            None => raw,
        };
        (height, revs)
    };
    if height < 1e-9 {
        return flat_spiral(ev, base, z, x, r0, revs, left_handed, &path);
    }
    if revs < 1e-9 {
        return crate::regen::eval::err_at(&["revolutions"], "Error regenerating: a parameter is out of range.");
    }
    // helix.fs: a zero start radius with an end radius swaps the ends (live
    // L33: a conical spiral out from the axis); both zero fails.
    if r0 < 1e-9 {
        let r_end = height * taper.tan();
        if r_end.abs() < 1e-9 {
            return err("Start or end radius should be greater than 0.");
        }
        let turn = std::f64::consts::TAU * revs * if left_handed { -1.0 } else { 1.0 };
        let x_end = DQuat::from_axis_angle(z, turn) * x;
        let (b2, z2, x2) = (base + z * height, -z, x_end);
        base = b2;
        z = z2;
        x = x2;
        r0 = r_end;
        taper = -taper;
    }
    let pitch = height / revs;
    let frame = k::Plane::new(base, z, x);
    let wire = k::make_helix(&frame, r0, pitch, height, left_handed, taper)?;
    // helix.fs drawConstructionEnds: the end profiles as magenta debug
    // geometry, a circle across the axis at each end (a point at a zero
    // radius) and its centre (live 2111).
    if ev.bool("showStartEndProfiles") {
        let y = z.cross(x).normalize_or_zero();
        for (c, r) in [(base, r0), (base + z * height, r0 + height * taper.tan())] {
            if r > 1e-9 {
                ev.debug.lines.push((0..=96).map(|i| {
                    let a = TAU * i as f64 / 96.0;
                    let p = c + (x * a.cos() + y * a.sin()) * r;
                    [p.x as f32, p.y as f32, p.z as f32]
                }).collect());
            }
            ev.debug.points.push([c.x as f32, c.y as f32, c.z as f32]);
        }
    }
    add_wire_bodies(ev, vec![wire])
}

/// Flat (Archimedean) spiral in the plane normal to `z`: radius from `r0`
/// to the end radius / end point over the revolutions (Pitch = radial growth per turn).
#[allow(clippy::too_many_arguments)]
fn flat_spiral(ev: &mut EvalCtx, base: DVec3, z: DVec3, x: DVec3, r0: f64, revs: f64, left_handed: bool, path: &str) -> FResult<()> {
    let r1 = if ev.is("endType", "END_POINT") {
        let Some(p) = ev.query("endPoint").first().and_then(|r| ev.resolve_vertex_point(r)) else { return err("Cannot resolve entities.") };
        let radial = p - base - z * (p - base).dot(z);
        radial.length()
    } else if ev.bool("endRadToggle") {
        ev.length("endRadius")?
    } else {
        return crate::regen::eval::err_at(&["height"], "Error regenerating: a parameter is out of range.");
    };
    let dr = r1 - r0;
    let revs = if path == "PITCH" {
        let p = ev.length("helicalPitch")?;
        if p < 1e-12 {
            return crate::regen::eval::err_at(&["helicalPitch"], "Error regenerating: a parameter is out of range.");
        }
        dr.abs() / p
    } else {
        revs
    };
    if revs < 1e-9 || dr.abs() < 1e-12 {
        return crate::regen::eval::err_at(&["revolutions"], "Error regenerating: a parameter is out of range.");
    }
    let y = z.cross(x);
    let sgn = if left_handed { -1.0 } else { 1.0 };
    let n = ((revs * 48.0).ceil() as usize).max(8);
    let pts: Vec<DVec3> = (0..=n)
        .map(|i| {
            let t = i as f64 / n as f64;
            let a = sgn * TAU * revs * t;
            base + (x * a.cos() + y * a.sin()) * (r0 + dr * t)
        })
        .collect();
    let wire = k::make_interpolated_spline(&pts, &[], false)?;
    add_wire_bodies(ev, vec![wire])
}

// ------------------------------------------------------------------ fit spline

pub fn fit_spline(ev: &mut EvalCtx) -> FResult<()> {
    let edge = if ev.is("fitType", "EDGES") {
        let edges = curve_edges(ev, ev.query("edges"))?;
        if edges.is_empty() {
            return crate::regen::eval::err_at(&["edges"], "Select tangent continuous edges to be approximated by a spline.");
        }
        let chain = order_chain(&edges, 1e-6)?;
        let closed = {
            let pts = sample_chain(&chain, 1)?;
            pts.len() > 2 && (pts[0] - pts[pts.len() - 1]).length() < 1e-6
        };
        let mut pts = sample_chain(&chain, 24)?;
        if closed {
            pts.pop();
        }
        k::make_interpolated_spline(&pts, &[], closed)?
    } else {
        let refs = ev.query("vertices");
        let mut pts = Vec::new();
        for r in refs {
            match ev.resolve_vertex_point(r) {
                Some(p) => pts.push(p),
                None => return crate::regen::eval::err_at(&["vertices"], "Select at least two vertices to make a spline."),
            }
        }
        pts.dedup_by(|a, b| (*a - *b).length() < 1e-9);
        let closed = ev.bool("closed");
        if closed && pts.len() > 1 && (pts[0] - pts[pts.len() - 1]).length() < 1e-9 {
            pts.pop();
        }
        if closed && pts.len() < 3 {
            return crate::regen::eval::err_at(&["vertices"], "Select at least three vertices to make a closed spline.");
        }
        if pts.len() < 2 {
            return crate::regen::eval::err_at(&["vertices"], "Select at least two vertices to make a spline.");
        }
        let mut tangents = Vec::new();
        if !closed {
            // The picked direction and the magnitude, negated by its flip (std).
            let dir = |id: &'static str, mag: &'static str, flip: &str| -> FResult<Option<(DVec3, f64)>> {
                let Some(r) = ev.query(id).first() else { return Ok(None) };
                let d = ev.resolve_direction(r).ok_or_else(|| FeatureError::at(&[id], "Cannot evaluate tangency for end condition."))?;
                let m = ev.real(mag)?;
                if m.abs() < 1e-12 {
                    return crate::regen::eval::err_at(&[mag], if mag.starts_with("start") { "Start magnitude cannot be 0." } else { "End magnitude cannot be 0." });
                }
                Ok(Some((d.normalize_or_zero(), m * if ev.bool(flip) { -1.0 } else { 1.0 })))
            };
            let s = dir("startDirection", "startMagnitude", "oppositeDirectionStart")?;
            let e = dir("endDirection", "endMagnitude", "oppositeDirectionEnd")?;
            // std's addFitSplineManipulators: at each end with a direction, an arrow along the picked direction
            // (back from the end) whose offset is the signed magnitude times the points' box diagonal / 3.
            let (lo, hi) = pts.iter().fold((pts[0], pts[0]), |(lo, hi), p| (lo.min(*p), hi.max(*p)));
            let span = (hi - lo).length();
            for (key, end, at, back, primary) in [("startManipulator", s, pts[0], false, "startMagnitude"), ("endManipulator", e, pts[pts.len() - 1], true, "endMagnitude")] {
                let Some((d, m)) = end else { continue };
                let direction = if back { -d } else { d };
                ev.manipulators.push((key.into(), crate::display::FeatureManipulator::Linear { base: at, direction, offset: m * span / 3.0, min: f64::NEG_INFINITY, max: f64::INFINITY, secondary: false, primary: Some(primary.into()) }));
            }
            if s.is_some() || e.is_some() {
                let mut t = vec![DVec3::ZERO; pts.len()];
                t[0] = s.map_or(DVec3::ZERO, |(d, m)| d * m);
                if let Some(last) = t.last_mut() {
                    *last = e.map_or(DVec3::ZERO, |(d, m)| d * m);
                }
                tangents = t;
            }
        }
        k::make_interpolated_spline(&pts, &tangents, closed)?
    };
    add_wire_bodies(ev, vec![k::make_wire(&[edge])?])
}

// ------------------------------------------------------------------ projected curve

pub fn project_curves(ev: &mut EvalCtx) -> FResult<()> {
    if ev.is("curveProjectionType", "CURVE_TO_FACE") {
        let tools = curve_edges(ev, ev.query("dropTools"))?;
        if tools.is_empty() {
            return crate::regen::eval::err_at(&["dropTools"], "Cannot resolve entities.");
        }
        let targets = face_shapes(ev, ev.query("targets"), true, "targets")?;
        if targets.is_empty() {
            return crate::regen::eval::err_at(&["targets"], "Cannot resolve entities.");
        }
        let dir = if ev.is("projectionType", "DIRECTION") {
            let Some(r) = ev.query("directionQuery").first() else { return crate::regen::eval::err_at(&["directionQuery"], "Cannot resolve entities.") };
            let d = ev.resolve_direction(r).ok_or_else(|| crate::regen::eval::FeatureError::at(&["directionQuery"], "Cannot resolve entities."))?;
            Some(if ev.bool("oppositeDirection") { -d } else { d })
        } else {
            None
        };
        let target = Shape::compound(&targets);
        let mut projected = Vec::new();
        for w in chain_wires(&tools)? {
            let res = k::project_curves(&w, &target, dir)?;
            projected.extend(res.edges());
        }
        let wires = chain_wires(&projected)?;
        if wires.is_empty() {
            return err("No projections found with current selections.");
        }
        return add_wire_bodies(ev, wires);
    }
    // Two sketches: intersect the extrusions of both sketch curve sets.
    let mut sets = Vec::new();
    for id in ["sketchEdges1", "sketchEdges2"] {
        let refs = ev.query(id);
        let mut sketch = None;
        let mut edges = Vec::new();
        for r in refs {
            let Reference::SketchEntity { sketch: s, entity } = r else { continue };
            if sketch.is_some_and(|x| x != *s) {
                return crate::regen::eval::err_at(&[id], "The curves in each list must be from the same sketch.");
            }
            sketch = Some(*s);
            if let Some(e) = sketch_edge(ev, *s, *entity)? {
                edges.push(e);
            }
        }
        let Some(s) = sketch else { return crate::regen::eval::err_at(&[id], "Cannot resolve entities.") };
        let frame = ev.ctx.sketch(s).map(|x| x.frame).ok_or_else(|| FeatureError::at(&[id], "Cannot resolve entities."))?;
        if edges.is_empty() {
            return crate::regen::eval::err_at(&[id], "Cannot resolve entities.");
        }
        sets.push((frame, edges));
    }
    if sets[0].0.normal.cross(sets[1].0.normal).length() < 1e-9 {
        return err("The sketches cannot be parallel.");
    }
    let ext = ev.model_extent() * 2.0;
    let mut prisms = Vec::new();
    for (frame, edges) in &sets {
        let mut faces = Vec::new();
        for e in edges {
            let r = k::extrude_between(e, -frame.normal * ext, frame.normal * ext, 0.0)?;
            faces.push(r.shape);
        }
        prisms.push(Shape::compound(&faces));
    }
    let sec = k::section(&prisms[0], &prisms[1])?;
    let wires = chain_wires(&sec.shape.edges())?;
    if wires.is_empty() {
        return err("Error regenerating.");
    }
    add_wire_bodies(ev, wires)
}

// ------------------------------------------------------------------ bridging curve

struct Side {
    pos: DVec3,
    tangent: Option<DVec3>,
}

/// Tangent of an edge at `pos` (one of its ends or an interior point),
/// pointing away from the edge interior when `pos` is an end.
fn edge_tangent_at(e: &Shape, pos: DVec3, toward: Option<DVec3>) -> FResult<DVec3> {
    let info = e.edge_info()?;
    let (f, l) = (info.first_param, info.last_param);
    let pf = e.point_at(f)?;
    let pl = e.point_at(l)?;
    let span = (l - f).abs().max(1e-12);
    let (t, inner) = if (pos - pf).length() < 1e-7 {
        (e.tangent_at(f)?, Some(e.point_at(f + span * 0.01)?))
    } else if (pos - pl).length() < 1e-7 {
        (e.tangent_at(l)?, Some(e.point_at(l - span * 0.01)?))
    } else {
        // Interior point: tangent at the closest sampled parameter.
        let n = 64;
        let mut best = (f64::INFINITY, f);
        for i in 0..=n {
            let p = f + (l - f) * i as f64 / n as f64;
            let d = (e.point_at(p)? - pos).length();
            if d < best.0 {
                best = (d, p);
            }
        }
        (e.tangent_at(best.1)?, None)
    };
    let t = t.normalize_or(DVec3::X);
    let sign = match (inner, toward) {
        (Some(q), _) => (pos - q).dot(t).signum(),
        (None, Some(o)) => (o - pos).dot(t).signum(),
        _ => 1.0,
    };
    Ok(t * if sign == 0.0 { 1.0 } else { sign })
}

/// One end of a bridging curve from `field`'s selections (side1 the start, side2 the end).
fn bridge_side(ev: &EvalCtx, field: &'static str, refs: &[Reference], other: Option<DVec3>, param: Option<f64>) -> FResult<Option<Side>> {
    // std bridgingCurve.fs: a selection that no longer resolves leaves the side empty.
    let gone = || {
        FeatureError::at(&[field], if field == "side1" {
            "Select a vertex or edge at the start position, and optionally an edge or face with the starting direction/curvature."
        } else {
            "Select a vertex or edge at the ending position, and optionally an edge or face with the ending direction/curvature."
        })
    };
    let mut point = None;
    let mut edge = None;
    let mut face = None;
    let mut mc_z = None;
    for r in refs {
        match r {
            Reference::MateConnector { .. } | Reference::Implicit { .. } => {
                let c = ev.resolve_connector(r).ok_or_else(gone)?;
                point = Some(c.origin);
                mc_z = Some(c.z);
            }
            Reference::Edge(_) => {
                let &(bi, ei) = ev.resolve_edges(std::slice::from_ref(r)).first().ok_or_else(gone)?;
                edge = Some(ev.edge_shape(bi, ei)?);
            }
            Reference::Face(_) => {
                let &(bi, fi) = ev.resolve_faces(std::slice::from_ref(r)).first().ok_or_else(gone)?;
                face = Some(ev.face_shape(bi, fi)?);
            }
            Reference::SketchEntity { sketch, entity } => match ev.resolve_vertex_point(r) {
                Some(p) => point = Some(p),
                None => edge = sketch_edge(ev, *sketch, *entity)?,
            },
            _ => point = ev.resolve_vertex_point(r).or(point),
        }
    }
    if let Some(z) = mc_z {
        return Ok(Some(Side { pos: point.unwrap_or_default(), tangent: Some(z) }));
    }
    if let Some(e) = edge {
        let info = e.edge_info()?;
        let pos = match (point, param) {
            (Some(p), _) => p,
            (None, Some(t)) => e.point_at(info.first_param + (info.last_param - info.first_param) * t)?,
            (None, None) => {
                let (a, b) = (e.point_at(info.first_param)?, e.point_at(info.last_param)?);
                match other {
                    Some(o) if (a - o).length() < (b - o).length() => a,
                    _ => b,
                }
            }
        };
        let t = edge_tangent_at(&e, pos, other)?;
        return Ok(Some(Side { pos, tangent: Some(t) }));
    }
    if let (Some(f), Some(p)) = (&face, point) {
        let n = f.normal_at_point(p)?;
        let t = other.map(|o| o - p).map(|d| d - n * d.dot(n)).filter(|d| d.length() > 1e-12).map(|d| d.normalize());
        return Ok(Some(Side { pos: p, tangent: t }));
    }
    if face.is_some() {
        return crate::regen::eval::err_at(&[field], "Select a vertex or an edge on each side.");
    }
    Ok(point.map(|pos| Side { pos, tangent: None }))
}

fn rough_position(ev: &EvalCtx, refs: &[Reference]) -> Option<DVec3> {
    refs.iter().find_map(|r| {
        ev.resolve_vertex_point(r).or_else(|| ev.resolve_edges(std::slice::from_ref(r)).first().and_then(|&(bi, ei)| ev.edge_shape(bi, ei).ok()?.edge_info().ok().map(|i| (i.start + i.end) * 0.5)))
    })
}

pub fn bridging_curve(ev: &mut EvalCtx) -> FResult<()> {
    // std's bridgingCurve.fs (computeBridgingControlPoints: real curvature and flow matching, std's control-point
    // manipulators); natively only without the FeatureScript host.
    if crate::regen::custom_features::host().is_some() && std::env::var_os("NOSHAPE_BRIDGE_NATIVE").is_none() {
        return crate::regen::custom_features::evaluate_std(ev, "bridgingCurve.fs", "bridgingCurve");
    }
    let (q1, q2) = (ev.query("side1"), ev.query("side2"));
    if q1.is_empty() {
        return crate::regen::eval::err_at(&["side1"], "Select a vertex or edge at the start position, and optionally an edge or face with the starting direction/curvature.");
    }
    if q2.is_empty() {
        return crate::regen::eval::err_at(&["side2"], "Select a vertex or edge at the ending position, and optionally an edge or face with the ending direction/curvature.");
    }
    let edit_pos = ev.bool("editEdgePositions");
    let p1 = if edit_pos { Some(ev.real("startEdgeParameter")?) } else { None };
    let p2 = if edit_pos { Some(ev.real("endEdgeParameter")?) } else { None };
    let hint2 = rough_position(ev, q2);
    let s1 = bridge_side(ev, "side1", q1, hint2, p1)?.ok_or_else(|| crate::regen::eval::FeatureError::at(&["side1"], "Select a vertex or edge at the start position, and optionally an edge or face with the starting direction/curvature."))?;
    let s2 = bridge_side(ev, "side2", q2, Some(s1.pos), p2)?.ok_or_else(|| crate::regen::eval::FeatureError::at(&["side2"], "Select a vertex or edge at the ending position, and optionally an edge or face with the ending direction/curvature."))?;
    let chord = (s2.pos - s1.pos).length();
    if chord < 1e-9 {
        return err("Bridging curve start and end points must be distinct.");
    }
    let (m1, m2) = if ev.is("method", "MAGNITUDE_BIAS") {
        let m = ev.real("magnitude")?;
        let b = ev.real("bias")?;
        (m * 2.0 * (1.0 - b), m * 2.0 * b)
    } else if ev.bool("editControlPoints") {
        (ev.real("side1Magnitude")?, ev.real("side2Magnitude")?)
    } else {
        (1.0, 1.0)
    };
    for (side, flip, mag, which) in [("match1", "flip1", m1, "start"), ("match2", "flip2", m2, "end")] {
        let _ = flip;
        if !ev.is(side, "POSITION") && mag.abs() < 1e-12 {
            return err(format!("The {which} magnitude can't be zero"));
        }
    }
    if ["match1", "match2"].iter().any(|s| ev.is(s, "CURVATURE") || ev.is(s, "G3")) {
        ev.warnings.push("Curvature and flow matching are approximated with tangency".into());
    }
    let t1 = if ev.is("match1", "POSITION") { None } else { s1.tangent.map(|t| if ev.bool("flip1") { -t } else { t }) };
    let t2 = if ev.is("match2", "POSITION") { None } else { s2.tangent.map(|t| if ev.bool("flip2") { -t } else { t }) };
    let poles = match (t1, t2) {
        (Some(a), Some(b)) => vec![s1.pos, s1.pos + a * chord / 3.0 * m1, s2.pos + b * chord / 3.0 * m2, s2.pos],
        (Some(a), None) => vec![s1.pos, s1.pos + a * chord / 2.0 * m1, s2.pos],
        (None, Some(b)) => vec![s1.pos, s2.pos + b * chord / 2.0 * m2, s2.pos],
        (None, None) => vec![s1.pos, s2.pos],
    };
    let deg = poles.len() as u32 - 1;
    let edge = k::make_bspline(&poles, None, &[0.0, 1.0], &[deg + 1, deg + 1], deg, false)?;
    add_wire_bodies(ev, vec![k::make_wire(&[edge])?])
}

// ------------------------------------------------------------------ composite / intersection

pub fn composite_curve(ev: &mut EvalCtx) -> FResult<()> {
    let edges = curve_edges(ev, ev.query("edges"))?;
    if edges.is_empty() {
        return crate::regen::eval::err_at(&["edges"], "At least one edge is required.");
    }
    let wires = chain_wires(&edges)?;
    add_wire_bodies(ev, wires)
}

pub fn intersection_curve(ev: &mut EvalCtx) -> FResult<()> {
    let g1 = face_shapes(ev, ev.query("group1"), true, "group1")?;
    let g2 = face_shapes(ev, ev.query("group2"), true, "group2")?;
    if g1.is_empty() || g2.is_empty() {
        return crate::regen::eval::err_at(if g1.is_empty() { &["group1"] } else { &["group2"] }, "Cannot resolve entities.");
    }
    if g1.iter().any(|a| g2.iter().any(|b| a.is_same(b))) {
        return err("Selections must be unique.");
    }
    let sec = k::section(&Shape::compound(&g1), &Shape::compound(&g2))?;
    let wires = chain_wires(&sec.shape.edges())?;
    if wires.is_empty() {
        return err("No intersections found with current selections.");
    }
    add_wire_bodies(ev, wires)
}

// ------------------------------------------------------------------ trim curve

/// End of a chain: point, outward tangent, the end edge.
fn chain_end(chain: &[(Shape, bool)], at_end: bool) -> FResult<(DVec3, DVec3, Shape)> {
    let (e, rev) = if at_end { chain.last() } else { chain.first() }.cloned().ok_or_else(|| FeatureError::new("Empty curve"))?;
    let (a, b) = e.endpoints()?;
    // Chain direction runs a -> b unless reversed.
    let p = match (at_end, rev) {
        (true, false) | (false, true) => b,
        _ => a,
    };
    let t = edge_tangent_at(&e, p, None)?;
    Ok((p, t, e))
}

/// Move curve boundary › Extend: the end of a wire's chain that grows (the one nearer the help point, the other with
/// Opposite direction; the chain's last without a help point), whether it is the chain's last, and its point, outward
/// tangent and edge.
fn extended_end(chain: &[(Shape, bool)], help: Option<DVec3>, flip: bool) -> FResult<(bool, (DVec3, DVec3, Shape))> {
    let (ps, ..) = chain_end(chain, false)?;
    let (pe, ..) = chain_end(chain, true)?;
    let at_end = match help {
        Some(h) => (h - pe).length() <= (h - ps).length(),
        None => true,
    } != flip;
    Ok((at_end, chain_end(chain, at_end)?))
}

/// Move curve boundary's distance arrow (moveCurveBoundary.fs addExtendCurveManipulator: Extend, Blind, with a help
/// point; the first curve): the original end that grows and its outward tangent.
pub(crate) fn extend_curve_anchor(ev: &EvalCtx) -> Option<(DVec3, DVec3)> {
    if !ev.is("moveBoundaryType", "EXTEND") || !ev.is("endCondition", "BLIND") {
        return None;
    }
    let help = ev.resolve_vertex_point(ev.query("helpPoint").first()?)?;
    let bi = *wire_body_indices(ev).ok()?.first()?;
    let chain = order_chain(&ev.ctx.bodies[bi].shape.edges(), 1e-6).ok()?;
    let (_, (p, t, _)) = extended_end(&chain, Some(help), ev.bool("flipHeuristics")).ok()?;
    Some((p, t))
}

fn wire_body_indices(ev: &EvalCtx) -> FResult<Vec<usize>> {
    let refs = ev.query("wires");
    let idx: Vec<usize> = ev.resolve_bodies(refs).into_iter().filter(|&i| ev.ctx.bodies[i].kind == BodyKind::Wire).collect();
    if idx.is_empty() {
        return crate::regen::eval::err_at(&["wires"], "Select a curve to adjust.");
    }
    Ok(idx)
}

/// Distance along the ray (p, t) to the first hit of `target`, if any.
fn ray_hit(ev: &EvalCtx, r: &Reference, p: DVec3, t: DVec3) -> FResult<Option<f64>> {
    if let Some(f) = ev.resolve_plane(r)
        && matches!(r, Reference::Plane(_) | Reference::FeaturePlane(_) | Reference::ConstructionPlane { .. } | Reference::MateConnector { .. } | Reference::Implicit { .. })
    {
        let den = t.dot(f.normal);
        if den.abs() < 1e-12 {
            return Ok(None);
        }
        let s = (f.origin - p).dot(f.normal) / den;
        return Ok((s > 1e-9).then_some(s));
    }
    if matches!(r, Reference::Vertex(_) | Reference::SketchEntity { .. } | Reference::Origin)
        && let Some(q) = ev.resolve_vertex_point(r)
    {
        let s = (q - p).dot(t);
        return Ok((s > 1e-9).then_some(s));
    }
    let shapes = match r {
        Reference::Edge(_) => curve_edges(ev, std::slice::from_ref(r))?,
        _ => face_shapes(ev, std::slice::from_ref(r), false, "extendTo")?,
    };
    if shapes.is_empty() {
        return Ok(None);
    }
    let long = k::make_line(p, p + t * ev.model_extent() * 4.0)?;
    let sec = k::section(&long, &Shape::compound(&shapes))?;
    let best = sec.shape.vertices().iter().filter_map(|v| v.vertex_point().ok()).map(|q| (q - p).dot(t)).filter(|s| *s > 1e-9).fold(f64::INFINITY, f64::min);
    Ok(best.is_finite().then_some(best))
}

pub fn trim_curve(ev: &mut EvalCtx) -> FResult<()> {
    let bodies = wire_body_indices(ev)?;
    let help = ev.query("helpPoint").first().and_then(|r| ev.resolve_vertex_point(r));
    let flip = ev.bool("flipHeuristics");
    if ev.is("moveBoundaryType", "EXTEND") {
        for bi in bodies {
            let edges = ev.ctx.bodies[bi].shape.edges();
            let chain = order_chain(&edges, 1e-6)?;
            let (at_end, (p, t, end_edge)) = extended_end(&chain, help, flip)?;
            let dist = if ev.is("endCondition", "UP_TO_ENTITY") {
                let mut best = f64::INFINITY;
                for r in ev.query("extendTo") {
                    if let Some(s) = ray_hit(ev, r, p, t)? {
                        best = best.min(s);
                    }
                }
                if !best.is_finite() {
                    return err("Curve extension does not intersect boundary.");
                }
                best
            } else {
                ev.length("extensionDistance")?
            };
            if dist < 1e-12 {
                continue;
            }
            let ext = match end_edge.edge_curve()? {
                k::Curve::Circle { center, axis, radius, .. } if ev.is("extensionShape", "CURVATURE") => {
                    // Continue along the circle by the arc length.
                    let ang = dist / radius;
                    let r0 = p - center;
                    let sense = if axis.cross(r0).dot(t) >= 0.0 { 1.0 } else { -1.0 };
                    let rot = |a: f64| center + DQuat::from_axis_angle(axis.normalize(), sense * a) * r0;
                    k::make_arc_3pts(p, rot(ang * 0.5), rot(ang))?
                }
                _ => k::make_line(p, p + t * dist)?,
            };
            let mut all: Vec<Shape> = chain.iter().map(|(e, _)| e.clone()).collect();
            if at_end {
                all.push(ext);
            } else {
                all.insert(0, ext);
            }
            let wire = k::make_wire(&all)?;
            replace_wire_body(ev, bi, wire);
        }
        return Ok(());
    }
    // Trim: split at the boundary entities and keep the piece nearest the help point.
    let refs = ev.query("trimTo");
    if refs.is_empty() {
        return crate::regen::eval::err_at(&["trimTo"], "Select a boundary to trim to.");
    }
    for bi in bodies {
        let shape = ev.ctx.bodies[bi].shape.clone();
        let mut tools = Vec::new();
        for r in refs {
            match r {
                Reference::Edge(_) | Reference::SketchEntity { .. } if ev.resolve_vertex_point(r).is_none() => tools.extend(curve_edges(ev, std::slice::from_ref(r))?),
                Reference::Vertex(_) | Reference::SketchEntity { .. } | Reference::Origin => {
                    let q = ev.resolve_vertex_point(r).ok_or_else(|| crate::regen::eval::FeatureError::at(&["trimTo"], "Select a boundary to trim to."))?;
                    let d = k::distance(&shape, &k::make_vertex(q)?)?;
                    tools.push(k::make_vertex(d.point_a)?);
                }
                Reference::Body(_) => {
                    for b in ev.resolve_bodies(std::slice::from_ref(r)) {
                        if b != bi {
                            tools.push(ev.ctx.bodies[b].shape.clone());
                        }
                    }
                }
                _ => tools.extend(face_shapes(ev, std::slice::from_ref(r), true, "trimTo")?),
            }
        }
        if tools.is_empty() {
            return crate::regen::eval::err_at(&["trimTo"], "Select a boundary to trim to.");
        }
        trim_wire_at(ev, bi, &tools, help, flip)?;
    }
    Ok(())
}

/// Wire body `bi` split by `tools` (shapes it meets), the piece nearest `help` kept (with `flip`, the others; without
/// a help point, the piece at the chain's start). Trim curve's trim and std's opMoveCurveBoundary to points.
pub fn trim_wire_at(ev: &mut EvalCtx, bi: usize, tools: &[Shape], help: Option<DVec3>, flip: bool) -> FResult<()> {
    let shape = ev.ctx.bodies[bi].shape.clone();
    let res = k::split(std::slice::from_ref(&shape), tools, &k::BooleanOptions::default())?;
    let pieces = chain_split_pieces(&res.shape, tools)?;
    if pieces.len() < 2 {
        return err("Curve does not intersect boundary.");
    }
    let anchor = match help {
        Some(h) => h,
        None => order_chain(&shape.edges(), 1e-6).and_then(|c| chain_end(&c, false)).map(|x| x.0)?,
    };
    let keep_idx = pieces
        .iter()
        .enumerate()
        .map(|(i, p)| (i, k::distance(p, &k::make_vertex(anchor).unwrap_or_default()).map(|d| d.distance).unwrap_or(f64::INFINITY)))
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|x| x.0)
        .unwrap_or(0);
    let kept: Vec<Shape> = if flip { pieces.iter().enumerate().filter(|(i, _)| *i != keep_idx).map(|(_, p)| p.clone()).collect() } else { vec![pieces[keep_idx].clone()] };
    let shape = if kept.len() == 1 { kept[0].clone() } else { Shape::compound(&kept) };
    replace_wire_body(ev, bi, shape);
    Ok(())
}

/// Wires of the split result: edges of the curve only, regrouped into
/// chains separated at split vertices.
fn chain_split_pieces(split: &Shape, tools: &[Shape]) -> FResult<Vec<Shape>> {
    let tool_edges: Vec<Shape> = tools.iter().flat_map(|t| t.edges()).collect();
    let edges: Vec<Shape> = split.edges().into_iter().filter(|e| !tool_edges.iter().any(|t| t.is_same(e))).collect();
    // Split vertices: points where the curve met the tools.
    let cut_points: Vec<DVec3> = {
        let mut v = Vec::new();
        for e in &edges {
            for vx in e.vertices() {
                if let Ok(p) = vx.vertex_point()
                    && tools.iter().any(|t| k::distance(t, &vx).map(|d| d.distance < 1e-6).unwrap_or(false))
                {
                    v.push(p);
                }
            }
        }
        v
    };
    // Union-find over edges sharing a non-cut endpoint.
    let n = edges.len();
    let ends: Vec<(DVec3, DVec3)> = edges.iter().map(|e| e.endpoints()).collect::<Result<_, _>>()?;
    let mut parent: Vec<usize> = (0..n).collect();
    fn find(p: &mut [usize], i: usize) -> usize {
        let mut i = i;
        while p[i] != i {
            p[i] = p[p[i]];
            i = p[i];
        }
        i
    }
    let is_cut = |q: DVec3| cut_points.iter().any(|c| (*c - q).length() < 1e-6);
    for i in 0..n {
        for j in (i + 1)..n {
            for a in [ends[i].0, ends[i].1] {
                for b in [ends[j].0, ends[j].1] {
                    if (a - b).length() < 1e-6 && !is_cut(a) {
                        let (ri, rj) = (find(&mut parent, i), find(&mut parent, j));
                        parent[ri] = rj;
                    }
                }
            }
        }
    }
    let mut groups: BTreeMap<usize, Vec<Shape>> = BTreeMap::new();
    for (i, e) in edges.iter().enumerate() {
        let r = find(&mut parent, i);
        groups.entry(r).or_default().push(e.clone());
    }
    let mut out = Vec::new();
    for (_, g) in groups {
        let chain = order_chain(&g, 1e-6)?;
        let es: Vec<Shape> = chain.into_iter().map(|(e, _)| e).collect();
        out.push(k::make_wire(&es).unwrap_or_else(|_| Shape::compound(&es)));
    }
    Ok(out)
}

// ------------------------------------------------------------------ isocline / offset curve

/// Offset curve on planar faces (geodesic = Euclidean on a plane).
pub fn offset_curve(ev: &mut EvalCtx) -> FResult<()> {
    let split = ev.is("scope", "OFFSET_EXTEND_AND_SPLIT");
    let extend = split || ev.is("scope", "OFFSET_AND_EXTEND");
    let refs = ev.query("edges");
    if refs.is_empty() {
        return crate::regen::eval::err_at(&["edges"], "Select edges to offset.");
    }
    let found = ev.resolve_edges(refs);
    if found.is_empty() {
        return crate::regen::eval::err_at(&["edges"], "Select edges to offset.");
    }
    let d = ev.length("distance")?;
    if d < 1e-12 {
        return crate::regen::eval::err_at(&["distance"], "Error regenerating: a parameter is out of range.");
    }
    let targets = face_shapes(ev, ev.query("targets"), false, "targets")?;
    let edges: Vec<Shape> = found.iter().map(|&(bi, ei)| ev.edge_shape(bi, ei)).collect::<FResult<_>>()?;
    // Candidate faces: targets, else the faces adjacent to the first edge.
    let face = if let Some(f) = targets.first() {
        f.clone()
    } else {
        let (bi, ei) = found[0];
        let b = &ev.ctx.bodies[bi];
        let ef = b.shape.edge_faces()?;
        let adj = ef.get(ei).cloned().unwrap_or_default();
        let pick = if ev.bool("oppositeDirection") { adj.get(1).or(adj.first()) } else { adj.first() };
        let Some(&fi) = pick else { return crate::regen::eval::err_at(&["edges"], "Offset only works on non-curve edges.") };
        b.shape.sub_shape(ShapeType::Face, fi as usize)?
    };
    if face.face_plane()?.is_none() {
        return err("Offset curve currently supports planar faces only");
    }
    let Some(pl) = face.face_plane()? else { return err("Offset curve currently supports planar faces only") };
    let n = pl.unit_normal();
    let chain = order_chain(&edges, 1e-6)?;
    let round = ev.is("gapFill", "ROUND");
    // Base samples of each edge in chain direction (2 points for lines).
    let mut bases: Vec<(Vec<DVec3>, bool)> = Vec::new();
    for c in &chain {
        let pts = sample_chain(std::slice::from_ref(c), 24)?;
        let linear = matches!(c.0.edge_curve()?, k::Curve::Line { .. });
        bases.push((if linear { vec![pts[0], pts[pts.len() - 1]] } else { pts }, linear));
    }
    let offset_pts = |pts: &[DVec3], dist: f64| -> Vec<DVec3> {
        let m = pts.len();
        (0..m).map(|i| pts[i] + n.cross((pts[(i + 1).min(m - 1)] - pts[i.saturating_sub(1)]).normalize_or(DVec3::X)) * dist).collect()
    };
    // Side: the offset of the first edge's middle must lie on the face.
    let on_face = |dist: f64| -> f64 {
        let (b, _) = &bases[0];
        let mid = (b[0] + b[b.len() - 1]) * 0.5;
        let o = offset_pts(b, dist);
        let q = (o[0] + o[o.len() - 1]) * 0.5 + (mid - (b[0] + b[b.len() - 1]) * 0.5);
        k::make_vertex(q).and_then(|v| k::distance(&face, &v)).map(|x| x.distance).unwrap_or(f64::INFINITY)
    };
    let inward = if on_face(d) <= on_face(-d) { d } else { -d };
    // With target faces (live 1813): Flip off offsets a boundary edge away from the face, Flip on onto it; without,
    // the side on the face its edge borders.
    let dist = if targets.is_empty() || ev.bool("oppositeDirection") { inward } else { -inward };
    let mut new_edges: Vec<Shape> = Vec::new();
    let mut prev_end: Option<(DVec3, DVec3)> = None;
    for (base, linear) in &bases {
        let pts = offset_pts(base, dist);
        if let Some((pe, corner)) = prev_end
            && (pe - pts[0]).length() > 1e-9
        {
            if round {
                let mid = corner + ((pe - corner) + (pts[0] - corner)).normalize_or(DVec3::X) * d;
                new_edges.push(k::make_arc_3pts(pe, mid, pts[0])?);
            } else {
                new_edges.push(k::make_line(pe, pts[0])?);
            }
        }
        new_edges.push(if *linear { k::make_line(pts[0], pts[pts.len() - 1])? } else { k::make_interpolated_spline(&pts, &[], false)? });
        prev_end = Some((pts[pts.len() - 1], base[base.len() - 1]));
    }
    // The offset as far as it lies on the face (live: a side leaning in stops it there).
    if !targets.is_empty() {
        let tol = 1e-7 * ev.model_extent().max(1e-3);
        let inside = k::common(&k::make_wire(&new_edges)?, &face).ok().map(|r| r.shape.edges()).unwrap_or_default();
        let inside: Vec<Shape> = inside.into_iter().filter(|e| e.length().is_ok_and(|l| l > tol)).collect();
        if inside.is_empty() {
            ev.warnings.push("The highlighted chains did not produce any offset with the given parameters.".into());
            return Ok(());
        }
        let length = |es: &[Shape]| es.iter().map(|e| e.length()).sum::<Result<f64, _>>();
        if length(&inside)? < length(&new_edges)? - tol {
            // Rebuilt to run forward along the offset (the extension and the trim walk them so).
            let start = new_edges[0].endpoints()?.0;
            let mut chain = order_chain(&inside, 1e-6)?;
            let ends = sample_chain(&chain, 2)?;
            if (ends[ends.len() - 1] - start).length() < (ends[0] - start).length() {
                chain.reverse();
                chain.iter_mut().for_each(|c| c.1 = !c.1);
            }
            let mut fwd = Vec::new();
            for c in &chain {
                let pts = sample_chain(std::slice::from_ref(c), 24)?;
                fwd.push(if matches!(c.0.edge_curve()?, k::Curve::Line { .. }) { k::make_line(pts[0], pts[pts.len() - 1])? } else { k::make_interpolated_spline(&pts, &[], false)? });
            }
            new_edges = fwd;
        }
    }
    if extend {
        extend_to_face_boundary(&mut new_edges, &face, ev.model_extent())?;
    }
    if split {
        return split_by_offset(ev, &face, &new_edges);
    }
    let wire = k::make_wire(&new_edges)?;
    let wire = if extend && ev.bool("trim") { trim_offset(ev, &wire, &new_edges)? } else { wire };
    add_wire_bodies(ev, vec![wire])
}

/// Offset, extend and split (live 1813): the offset's edges imprinted on the face's body, no wire body.
fn split_by_offset(ev: &mut EvalCtx, face: &Shape, edges: &[Shape]) -> FResult<()> {
    let Some(bi) = ev.ctx.bodies.iter().position(|b| b.shape.index_of(face, ShapeType::Face).is_some()) else {
        return err("Select target faces");
    };
    let body = ev.ctx.bodies[bi].clone();
    let res = k::imprint_kept(&body.shape, edges).map_err(|e| FeatureError::new(format!("The faces could not be split: {e}")))?;
    let names = ev.name_result(&[&body.shape_names(), &ShapeNames::default()], &res.history, &res.shape, &BTreeMap::new(), None);
    ev.replace_body(bi, res.shape, names);
    Ok(())
}

/// The point and unit tangent at arc length `s` along a chain of forward edges.
fn chain_point_at_length(edges: &[Shape], s: f64) -> FResult<(DVec3, DVec3)> {
    let mut left = s.max(0.0);
    for (i, e) in edges.iter().enumerate() {
        let len = e.length()?;
        if left > len && i + 1 < edges.len() {
            left -= len;
            continue;
        }
        let info = e.edge_info()?;
        let (a, b) = (info.first_param, info.last_param);
        // Arc length along the edge by its chords, then the parameter there.
        let n = 256;
        let mut prev = e.point_at(a)?;
        let mut acc = 0.0;
        let mut u = b;
        for j in 1..=n {
            let t = a + (b - a) * j as f64 / n as f64;
            let p = e.point_at(t)?;
            let d = (p - prev).length();
            if acc + d >= left {
                let f = if d > 0.0 { (left - acc) / d } else { 0.0 };
                u = t - (b - a) / n as f64 * (1.0 - f);
                break;
            }
            acc += d;
            prev = p;
        }
        return Ok((e.point_at(u)?, e.tangent_at(u)?.normalize_or_zero()));
    }
    err("Unable to trim to specifications.")
}

/// Offset and extend's Trim (std offsetCurveOnFace.fs): Start trim and End trim (Equal trim: the start's) off the
/// curve's ends, with std's two trim arrows along the curve at the cuts.
fn trim_offset(ev: &mut EvalCtx, wire: &Shape, edges: &[Shape]) -> FResult<Shape> {
    let (start, end) = edges.first().zip(edges.last()).map(|(f, l)| (f.endpoints().map(|x| x.0), l.endpoints().map(|x| x.1))).ok_or_else(|| FeatureError::new("Unable to trim to specifications."))?;
    if (start? - end?).length() < 1e-9 {
        return err("Cannot trim closed offset curve.");
    }
    let s0 = ev.length("startTrim")?;
    let s1 = if ev.bool("equalTrim") { s0 } else { ev.length("endTrim")? };
    let total: f64 = edges.iter().map(|e| e.length()).sum::<Result<f64, _>>()?;
    let fail = s0 + s1 > total - 1e-9;
    let (p0, t0) = chain_point_at_length(edges, s0.min(total))?;
    let (p1, t1) = chain_point_at_length(edges, (total - s1).max(0.0))?;
    use crate::display::FeatureManipulator as M;
    let arrow = |base: DVec3, direction: DVec3, primary: &str| M::Linear { base, direction, offset: 0.0, min: f64::NEG_INFINITY, max: f64::INFINITY, secondary: false, primary: Some(primary.into()) };
    ev.manipulators.push(("startTrimManipulator".into(), arrow(p0 + t0 * (s0 - total).max(0.0), t0, "startTrim")));
    if !ev.bool("equalTrim") {
        ev.manipulators.push(("endTrimManipulator".into(), arrow(p1 + t1 * (total - s1).min(0.0), t1, "endTrim")));
    }
    if fail {
        return Err(FeatureError { faulty: vec!["startTrim", "endTrim"], ..FeatureError::new("Unable to trim to specifications.") });
    }
    let cuts: Vec<DVec3> = [(s0, p0), (s1, p1)].into_iter().filter(|(d, _)| *d > 1e-12).map(|(_, p)| p).collect();
    if cuts.is_empty() {
        return Ok(wire.clone());
    }
    let tools: Vec<Shape> = cuts.iter().map(|p| k::make_vertex(*p)).collect::<Result<_, _>>()?;
    let res = k::split(std::slice::from_ref(wire), &tools, &k::BooleanOptions::default())?;
    let (mid, _) = chain_point_at_length(edges, s0 + (total - s0 - s1) / 2.0)?;
    let at = k::make_vertex(mid)?;
    chain_split_pieces(&res.shape, &tools)?
        .into_iter()
        .min_by(|a, b| k::distance(a, &at).map(|d| d.distance).unwrap_or(f64::INFINITY).total_cmp(&k::distance(b, &at).map(|d| d.distance).unwrap_or(f64::INFINITY)))
        .ok_or_else(|| FeatureError::new("Unable to trim to specifications."))
}

/// Offset and extend: each open end of the offset chain continues along its tangent to the face's boundary.
fn extend_to_face_boundary(edges: &mut Vec<Shape>, face: &Shape, extent: f64) -> FResult<()> {
    let (Some(first), Some(last)) = (edges.first(), edges.last()) else { return Ok(()) };
    let start = first.endpoints()?.0;
    let end = last.endpoints()?.1;
    if (start - end).length() < 1e-9 {
        return Ok(());
    }
    let boundary = Shape::compound(&face.edges());
    // The nearest boundary crossing ahead of `p` along `t`.
    let hit = |p: DVec3, t: DVec3| -> FResult<Option<DVec3>> {
        let ray = k::make_line(p, p + t * extent * 4.0)?;
        let sec = k::section(&ray, &boundary)?;
        let best = sec.shape.vertices().iter().filter_map(|v| v.vertex_point().ok()).map(|q| (q - p).dot(t)).filter(|s| *s > 1e-9).fold(f64::INFINITY, f64::min);
        Ok(best.is_finite().then(|| p + t * best))
    };
    // The chain's edges are new and run forward, start to end.
    let t0 = -first.tangent_at(first.edge_info()?.first_param)?.normalize_or_zero();
    let t1 = last.tangent_at(last.edge_info()?.last_param)?.normalize_or_zero();
    let (start_hit, end_hit) = (hit(start, t0)?, hit(end, t1)?);
    // Each extension its own edge (live 1813: three edges).
    if let Some(q) = start_hit {
        edges.insert(0, k::make_line(q, start)?);
    }
    if let Some(q) = end_hit {
        edges.push(k::make_line(end, q)?);
    }
    Ok(())
}

// ------------------------------------------------------------------ isoparametric curve

pub fn isoparametric_curve(ev: &mut EvalCtx) -> FResult<()> {
    let Some(&(bi, fi)) = ev.resolve_faces(ev.query("face")).first() else { return crate::regen::eval::err_at(&["face"], "Select face to create isoparametric curve.") };
    let face = ev.face_shape(bi, fi)?;
    let [u0, u1, v0, v1] = face.uv_bounds()?;
    let along_u = ev.is("directionType", "U_DIRECTION");
    let mut positions = Vec::new();
    if ev.bool("equalSpacing") {
        let n = ev.integer("nCurves")?.max(1);
        for i in 1..=n {
            positions.push(i as f64 / (n + 1) as f64);
        }
    } else {
        let items = ev.params.array("uvParamList");
        if items.is_empty() {
            return err("Select an option for position.");
        }
        for item in items {
            if item.bool("selectPoint") {
                let Some(p) = item.query("point").first().and_then(|r| ev.resolve_vertex_point(r)) else {
                    ev.warnings.push("A position has no point selected".into());
                    continue;
                };
                let pr = face.project_point(p)?;
                positions.push(if along_u { (pr.v - v0) / (v1 - v0) } else { (pr.u - u0) / (u1 - u0) });
            } else {
                positions.push(ev.eval_num_in(item, "uvValue", crate::params::NumKind::Real)?);
            }
        }
    }
    let mut wires = Vec::new();
    let n = 32;
    for t in positions {
        let t = t.clamp(0.0, 1.0);
        let mut pts = Vec::with_capacity(n + 1);
        for i in 0..=n {
            let s = i as f64 / n as f64;
            let (u, v) = if along_u { (u0 + (u1 - u0) * s, v0 + (v1 - v0) * t) } else { (u0 + (u1 - u0) * t, v0 + (v1 - v0) * s) };
            pts.push(face.point_at_uv(u, v)?);
        }
        let closed = (pts[0] - pts[n]).length() < 1e-9;
        if closed {
            pts.pop();
        }
        let e = k::make_interpolated_spline(&pts, &[], closed)?;
        wires.push(k::make_wire(&[e])?);
    }
    add_wire_bodies(ev, wires)
}

// ------------------------------------------------------------------ edit curve

pub fn edit_curve(ev: &mut EvalCtx) -> FResult<()> {
    // std's editCurve.fs (its approximation, elevation, planarization and control point edits, its triad and UVN
    // manipulators); natively only without the FeatureScript host.
    if crate::regen::custom_features::host().is_some() && std::env::var_os("NOSHAPE_EDIT_CURVE_NATIVE").is_none() {
        return crate::regen::custom_features::evaluate_std(ev, "editCurve.fs", "editCurve");
    }
    let refs = ev.query("wire");
    let edges = curve_edges(ev, refs)?;
    if edges.is_empty() {
        return crate::regen::eval::err_at(&["wire"], "Select curves.");
    }
    let single_body = match refs {
        [r @ Reference::Body(_)] => ev.resolve_bodies(std::slice::from_ref(r)).first().copied().filter(|&i| ev.ctx.bodies[i].kind == BodyKind::Wire),
        _ => None,
    };
    if single_body.is_none() && edges.len() > 1 && !ev.bool("approximate") {
        return crate::regen::eval::err_at(&["wire"], "Input curve has multiple edges. It needs to be reapproximated.");
    }
    let chain = order_chain(&edges, 1e-6)?;
    let ends = sample_chain(&chain, 1)?;
    let closed = ends.len() > 2 && (ends[0] - ends[ends.len() - 1]).length() < 1e-7;
    let mut pts = if ev.bool("approximate") {
        let n = ev.integer("approximationMaxCPs")?.max(4) as usize;
        resample(&sample_chain(&chain, 64)?, n)
    } else {
        sample_chain(&chain, 48)?
    };
    let mut changed = ev.bool("approximate");
    if ev.bool("planarize") {
        let (o, n) = match ev.enum_("planeReference") {
            "YZPLANE" => (DVec3::ZERO, DVec3::X),
            "XZPLANE" => (DVec3::ZERO, DVec3::Y),
            "XYPLANE" => (DVec3::ZERO, DVec3::Z),
            "CUSTOM" => {
                let Some(r) = ev.query("referencePlane").first() else { return crate::regen::eval::err_at(&["referencePlane"], "Select reference plane.") };
                let f = ev.resolve_plane(r).ok_or_else(|| FeatureError::new("Cannot resolve plane."))?;
                (f.origin, f.normal)
            }
            _ => best_fit_plane(&pts).ok_or_else(|| crate::regen::eval::FeatureError::at(&["wire"], "Cannot compute best fit plane."))?,
        };
        for p in &mut pts {
            *p -= n * (*p - o).dot(n);
        }
        changed = true;
    }
    let mut shape = if changed {
        if closed {
            pts.pop();
        }
        k::make_wire(&[k::make_interpolated_spline(&pts, &[], closed)?])?
    } else {
        k::make_wire(&chain.into_iter().map(|(e, _)| e).collect::<Vec<_>>())?
    };
    if ev.bool("editControlPoints") && !ev.params.array("controlPointEdits").is_empty() {
        shape = edit_control_points(ev, &shape)?;
    }
    match single_body {
        Some(bi) => {
            replace_wire_body(ev, bi, shape);
            Ok(())
        }
        None => add_wire_bodies(ev, vec![shape]),
    }
}

/// Apply the "Points overrides" of Edit curve: the curve (one edge) is
/// converted to its exact B-spline (`Shape::edge_bspline`), each override
/// moves control point `index` to its reference point (default: the
/// original control point) plus the X/Y/Z offset, and multiplies its weight.
/// Offsets are along world axes in XYZ mode; in the UVN modes U follows the
/// curve tangent at the closest curve point (UVN tangent) or the control
/// polygon leg (UVN control polygon), N is the curve plane normal (or a
/// perpendicular), V = N x U.
fn edit_control_points(ev: &EvalCtx, wire: &Shape) -> FResult<Shape> {
    let edges = wire.edges();
    let [edge] = edges.as_slice() else { return crate::regen::eval::err_at(&["wire"], "Input curve has multiple edges. It needs to be reapproximated.") };
    let mut d = edge.edge_bspline()?;
    let n = d.poles.len();
    let orig = d.poles.clone();
    let normal = wire.find_plane(1e-7).ok().flatten().map(|p| p.unit_normal());
    let mode = ev.enum_("editPointMode").to_string();
    let info = edge.edge_info()?;
    for item in ev.params.array("controlPointEdits") {
        let idx = ev.eval_num_in(item, "index", crate::params::NumKind::Integer)?.round();
        if idx < 0.0 || idx as usize >= n {
            return err(format!("Control point index {} is too large (the curve has {n} control points)", idx as i64));
        }
        let i = idx as usize;
        let base = match item.query("reference").first() {
            Some(r) => ev.resolve_vertex_point(r).ok_or_else(|| FeatureError::new("A control point reference no longer exists"))?,
            None => orig[i],
        };
        let off = DVec3::new(
            ev.eval_num_in(item, "x", crate::params::NumKind::Length).unwrap_or(0.0),
            ev.eval_num_in(item, "y", crate::params::NumKind::Length).unwrap_or(0.0),
            ev.eval_num_in(item, "z", crate::params::NumKind::Length).unwrap_or(0.0),
        );
        let off = if mode == "XYZ" {
            off
        } else {
            let u = if mode == "UVN_CONTROL_POLYGON" {
                let (a, b) = if i + 1 < n { (orig[i], orig[i + 1]) } else { (orig[i.saturating_sub(1)], orig[i]) };
                (b - a).normalize_or_zero()
            } else {
                // Tangent at the curve point closest to the control point.
                let samples = 64;
                let (mut best, mut best_d) = (info.first_param, f64::INFINITY);
                for k2 in 0..=samples {
                    let t = info.first_param + (info.last_param - info.first_param) * k2 as f64 / samples as f64;
                    if let Ok(p) = edge.point_at(t)
                        && p.distance(orig[i]) < best_d
                    {
                        best_d = p.distance(orig[i]);
                        best = t;
                    }
                }
                edge.tangent_at(best).unwrap_or(DVec3::X).normalize_or_zero()
            };
            let u = if u.length_squared() < 0.5 { DVec3::X } else { u };
            let nn = normal.filter(|v| v.cross(u).length() > 1e-6).unwrap_or_else(|| u.any_orthonormal_vector());
            let nn = (nn - u * nn.dot(u)).normalize();
            let v = nn.cross(u);
            u * off.x + v * off.y + nn * off.z
        };
        d.poles[i] = base + off;
        let w = ev.eval_num_in(item, "weight", crate::params::NumKind::Real).unwrap_or(1.0);
        if (w - 1.0).abs() > 1e-12 {
            if w <= 0.0 {
                return crate::regen::eval::err_at(&["controlPointEdits"], "Error regenerating: a parameter is out of range.");
            }
            let ws = d.weights.get_or_insert_with(|| vec![1.0; n]);
            ws[i] *= w;
        }
    }
    Ok(k::make_wire(&[k::make_bspline_edge(&d)?])?)
}

/// Resample a polyline to `n` points evenly spaced by arc length.
fn resample(pts: &[DVec3], n: usize) -> Vec<DVec3> {
    let mut cum = vec![0.0];
    for w in pts.windows(2) {
        cum.push(cum.last().copied().unwrap_or(0.0) + (w[1] - w[0]).length());
    }
    let total = cum.last().copied().unwrap_or(0.0);
    if total <= 0.0 || n < 2 {
        return pts.to_vec();
    }
    let mut out = Vec::with_capacity(n);
    let mut j = 0;
    for i in 0..n {
        let s = total * i as f64 / (n - 1) as f64;
        while j + 1 < cum.len() - 1 && cum[j + 1] < s {
            j += 1;
        }
        let seg = (cum[j + 1] - cum[j]).max(1e-300);
        let t = ((s - cum[j]) / seg).clamp(0.0, 1.0);
        out.push(pts[j].lerp(pts[j + 1], t));
    }
    out
}

/// Least-squares plane through points: (centroid, unit normal).
fn best_fit_plane(pts: &[DVec3]) -> Option<(DVec3, DVec3)> {
    if pts.len() < 3 {
        return None;
    }
    let c = pts.iter().fold(DVec3::ZERO, |a, p| a + *p) / pts.len() as f64;
    let mut m = nalgebra::Matrix3::<f64>::zeros();
    for p in pts {
        let d = *p - c;
        let v = nalgebra::Vector3::new(d.x, d.y, d.z);
        m += v * v.transpose();
    }
    let eig = m.symmetric_eigen();
    let (imin, _) = eig.eigenvalues.iter().enumerate().min_by(|a, b| a.1.total_cmp(b.1))?;
    let n = eig.eigenvectors.column(imin);
    let n = DVec3::new(n[0], n[1], n[2]);
    (n.length() > 1e-12).then(|| (c, n.normalize()))
}

// ------------------------------------------------------------------ routing curve

pub fn routing_curve(ev: &mut EvalCtx) -> FResult<()> {
    let base = match ev.query("curveBaseCSys").first() {
        Some(r) => match ev.resolve_connector(r) {
            Some(c) => (c.origin, c.x, c.z.cross(c.x), c.z),
            None => (ev.resolve_vertex_point(r).ok_or_else(|| FeatureError::new("Curve reference could not be resolved"))?, DVec3::X, DVec3::Y, DVec3::Z),
        },
        None => (DVec3::ZERO, DVec3::X, DVec3::Y, DVec3::Z),
    };
    let items = ev.params.array("points");
    let mut pts: Vec<DVec3> = Vec::new();
    let mut tangents: Vec<DVec3> = Vec::new();
    let mut radii: Vec<Option<f64>> = Vec::new();
    for item in items {
        let num = |id: &str| ev.eval_num_in(item, id, crate::params::NumKind::Length);
        let (dx, dy, dz) = (num("dx")?, num("dy")?, num("dz")?);
        let p = match item.enum_("referenceType") {
            "VERTEX" => {
                let r = item.query("vertex").first().ok_or_else(|| FeatureError::new("Select a vertex for each vertex-referenced point"))?;
                ev.resolve_vertex_point(r).ok_or_else(|| FeatureError::new("A point's vertex no longer exists"))? + DVec3::new(dx, dy, dz)
            }
            "CURVE" => {
                let edges = curve_edges(ev, item.query("curve"))?;
                if edges.is_empty() {
                    return crate::regen::eval::err_at(&["points"], "No curve selected.");
                }
                let samples = sample_chain(&order_chain(&edges, 1e-6)?, 64)?;
                let t = ev.eval_num_in(item, "curveParameter", crate::params::NumKind::Real)?.clamp(0.0, 1.0);
                point_at_fraction(&samples, t) + DVec3::new(dx, dy, dz)
            }
            "RELATIVE" => pts.last().copied().unwrap_or(base.0) + DVec3::new(dx, dy, dz),
            _ => base.0 + base.1 * dx + base.2 * dy + base.3 * dz,
        };
        let tangent = if item.bool("hasDerivative") {
            let d = match item.enum_("derivativeAlignment") {
                "X" => base.1,
                "Y" => base.2,
                "DIRECTION" => item.query("derivativeDirection").first().and_then(|r| ev.resolve_direction(r)).ok_or_else(|| FeatureError::new("Select a derivative direction"))?,
                _ => base.3,
            };
            let m = ev.eval_num_in(item, "magnitude", crate::params::NumKind::Real)?;
            d * m * if item.bool("oppositeDirection") { -1.0 } else { 1.0 }
        } else {
            DVec3::ZERO
        };
        let radius = if item.bool("overrideBendRadius") { Some(ev.eval_num_in(item, "pointBendRadius", crate::params::NumKind::Length)?) } else { None };
        pts.push(p);
        tangents.push(tangent);
        radii.push(radius);
    }
    if pts.is_empty() {
        match ev.enum_("inputType") {
            "VERTEX" => {
                for r in ev.query("inputVertices") {
                    pts.push(ev.resolve_vertex_point(r).ok_or_else(|| FeatureError::new("A selected vertex no longer exists"))?);
                }
            }
            "CURVE" => {
                let edges = curve_edges(ev, ev.query("inputCurve"))?;
                if !edges.is_empty() {
                    let samples = sample_chain(&order_chain(&edges, 1e-6)?, 64)?;
                    let n = ev.integer("samples")?.max(2);
                    for i in 0..n {
                        pts.push(point_at_fraction(&samples, i as f64 / (n - 1) as f64));
                    }
                }
            }
            _ => {}
        }
        tangents = vec![DVec3::ZERO; pts.len()];
        radii = vec![None; pts.len()];
    }
    // Drop consecutive duplicates.
    let mut keep = vec![true; pts.len()];
    for i in 1..pts.len() {
        if (pts[i] - pts[i - 1]).length() < 1e-9 {
            keep[i] = false;
        }
    }
    let filt = |v: Vec<DVec3>| -> Vec<DVec3> { v.into_iter().zip(keep.iter()).filter(|(_, k)| **k).map(|(x, _)| x).collect() };
    let radii: Vec<Option<f64>> = radii.into_iter().zip(keep.iter()).filter(|(_, k)| **k).map(|(x, _)| x).collect();
    let mut pts = filt(pts);
    let mut tangents = filt(tangents);
    let closed = ev.bool("isPeriodic");
    if closed && pts.len() > 2 && (pts[0] - pts[pts.len() - 1]).length() < 1e-9 {
        pts.pop();
        tangents.pop();
    }
    // std's texts on the Points list (live 1722); the title reads
    // "Routing curve (0 mm)" then.
    ev.computed.push(("curveLength".into(), crate::display::ComputedValue::Length(0.0)));
    if !closed && pts.len() < 2 {
        return err_at(&["points"], "Create at least two non-coincident points.");
    }
    if closed && pts.len() < 3 {
        return err_at(&["points"], "Create at least three non-coincident points.");
    }
    let wire = if ev.is("curveType", "POLYLINE") {
        let global = ev.length("bendRadius")?;
        let r: Vec<f64> = (0..pts.len()).map(|i| radii.get(i).copied().flatten().unwrap_or(global)).collect();
        lines_before_arcs(&filleted_polyline(&pts, &r, closed)?)?
    } else {
        let t = if tangents.iter().any(|t| t.length_squared() > 0.0) { tangents } else { Vec::new() };
        k::make_wire(&[k::make_interpolated_spline(&pts, &t, closed)?])?
    };
    // The feature's name: "Routing curve (61.825 mm)" (std's name template).
    let length = wire.length().unwrap_or(0.0);
    ev.computed.retain(|(k, _)| k != "curveLength");
    ev.computed.push(("curveLength".into(), crate::display::ComputedValue::Length(length)));
    add_wire_bodies(ev, vec![wire])
}

fn point_at_fraction(samples: &[DVec3], t: f64) -> DVec3 {
    let r = resample(samples, 201);
    let i = ((r.len() - 1) as f64 * t).round() as usize;
    r[i.min(r.len() - 1)]
}

/// A bent polyline's edges as Onshape lists them: the lines, then the arcs,
/// each in path order (live "fs (10-25 d)": opPolyline, the Routing curve).
pub fn lines_before_arcs(wire: &Shape) -> FResult<Shape> {
    let is_line: Vec<bool> = wire.edges().iter().map(|e| e.edge_info().is_ok_and(|i| matches!(i.curve, k::Curve::Line { .. }))).collect();
    let n = is_line.len() as u32;
    let order: Vec<u32> = (0..n).filter(|i| is_line[*i as usize]).chain((0..n).filter(|i| !is_line[*i as usize])).collect();
    Ok(k::wire_with_edge_order(wire, &order)?)
}

/// Polyline with tangent fillet arcs of radius `r[i]` at corner i (0 = sharp).
pub fn filleted_polyline(pts: &[DVec3], r: &[f64], closed: bool) -> FResult<Shape> {
    let n = pts.len();
    // Corner geometry: (tangent-in point, arc mid, tangent-out point).
    let mut corners: Vec<Option<(DVec3, DVec3, DVec3)>> = vec![None; n];
    for i in 0..n {
        if !closed && (i == 0 || i == n - 1) {
            continue;
        }
        let rad = r[i];
        if rad <= 0.0 {
            continue;
        }
        let p = pts[i];
        let a = pts[(i + n - 1) % n];
        let b = pts[(i + 1) % n];
        let (u1, u2) = ((a - p).normalize(), (b - p).normalize());
        let theta = u1.dot(u2).clamp(-1.0, 1.0).acos();
        if theta < 1e-6 || (PI - theta) < 1e-6 {
            continue;
        }
        let dist = rad / (theta * 0.5).tan();
        let bis = (u1 + u2).normalize();
        let c = p + bis * (rad / (theta * 0.5).sin());
        corners[i] = Some((p + u1 * dist, c - bis * rad, p + u2 * dist));
    }
    // A segment must hold the trims of the bends at both its ends (a bend
    // trims both its segments by the same length; live opPolyline: [30, 30]
    // on a 40 mm segment is POLYLINE_INVALID_BEND, [5, 25] fits).
    let trim = |i: usize| corners[i].map_or(0.0, |(t0, _, _)| (t0 - pts[i]).length());
    for i in 0..if closed { n } else { n - 1 } {
        let j = (i + 1) % n;
        if trim(i) + trim(j) > (pts[j] - pts[i]).length() + 1e-12 {
            return err("Cannot create bend at given point.");
        }
    }
    let mut edges = Vec::new();
    let segs = if closed { n } else { n - 1 };
    for i in 0..segs {
        let j = (i + 1) % n;
        let start = corners[i].map(|c| c.2).unwrap_or(pts[i]);
        let end = corners[j].map(|c| c.0).unwrap_or(pts[j]);
        if (end - start).length() > 1e-9 {
            edges.push(k::make_line(start, end)?);
        }
        if let Some((t0, m, t1)) = corners[j]
            && (closed || j != n - 1)
        {
            edges.push(k::make_arc_3pts(t0, m, t1)?);
        }
    }
    Ok(k::make_wire(&edges)?)
}
