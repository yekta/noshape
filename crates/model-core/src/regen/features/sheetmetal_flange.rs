//! Flange and Hem (03c SM-03, SM-05): a bend plus an optional straight wall
//! added along free edges of active sheet metal walls.
//!
//! Each selected edge gets a local frame: `U` = outward normal of the side
//! face (the free edge), `V` = the side the flange bends toward, `d` = edge
//! direction; the base wall occupies `u <= 0, -T <= v <= 0`. The bend region
//! starts at `u = u_t` (alignment), so the base is trimmed back to `u_t`
//! when needed, and the bend + wall cross-section is extruded along the edge
//! and fused with the part. Faces are named from the selected edge:
//! `Side(piece(edge, k))` for profile segment `k` (0 inner bend, 1 inner
//! wall face, 2 tip, 3 outer wall face, 4 outer bend), caps from the edge.

use super::sheetmetal_util::*;
use crate::naming::Name;
use crate::reference::{Reference, Source};
use crate::regen::eval::{EvalCtx, FResult, FeatureError, NamedShape, Profile, err, err_at, sweep_names};
use crate::sheetmetal::{Bend, BendKind, BendReliefStyle, SheetMetalData, Wall};
use glam::{DVec2, DVec3};
use noshape_kernel as k;
use noshape_kernel::ShapeType;
use std::collections::BTreeMap;
use std::f64::consts::{FRAC_PI_2, PI};

/// Local frame at a selected free edge.
#[derive(Clone, Debug)]
pub struct EdgeFrame {
    pub bi: usize,
    pub wall: usize,
    /// The edge lies on the wall's reference-side face.
    pub on_ref: bool,
    /// The flange bends toward the face containing the edge.
    pub inner_is_wall_face: bool,
    pub o: DVec3,
    pub u: DVec3,
    pub v: DVec3,
    pub d: DVec3,
    pub len: f64,
    pub p0: DVec3,
    pub p1: DVec3,
    /// Name of the selected edge (identity of the created faces).
    pub name: Name,
    /// "flange" or "hem", for the error texts.
    pub op: &'static str,
}

impl EdgeFrame {
    pub fn at(&self, u: f64, v: f64, s: f64) -> DVec3 {
        self.o + self.u * u + self.v * v + self.d * s
    }
    pub fn n(&self) -> DVec3 {
        self.u.cross(self.v)
    }
    /// Parameter of a world point along the edge.
    pub fn param(&self, p: DVec3) -> f64 {
        (p - self.o).dot(self.d)
    }
}

/// Build the frame of edge `ei` of body `bi`.
pub fn edge_frame(ev: &EvalCtx, data: &SheetMetalData, bi: usize, ei: usize, opposite: bool, op: &'static str) -> FResult<EdgeFrame> {
    let body = &ev.ctx.bodies[bi];
    let t = data.thickness;
    let edge = body.shape.sub_shape(ShapeType::Edge, ei)?;
    let Some((p0, p1)) = line_of(&edge) else { return err_at(&["edges"], format!("Only linear edges can be used to {op}.")) };
    let len = (p1 - p0).length();
    if len < 1e-9 {
        return err("Selected edge is too short");
    }
    let d = (p1 - p0) / len;
    let ef = body.shape.edge_faces()?;
    let faces = ef.get(ei).cloned().unwrap_or_default();
    if faces.len() != 2 {
        return err_at(&["edges"], format!("Cannot create {op} on an edge that is a joint, fillet, chamfer, or internal to the sheet metal model."));
    }
    let (fa, fb) = (faces[0] as usize, faces[1] as usize);
    let (wa, wb) = (data.wall_of_face(&body.faces[fa]), data.wall_of_face(&body.faces[fb]));
    let (wall_face, side_face, (wall, on_ref)) = match (wa, wb) {
        (Some(_), Some(_)) => return err_at(&["edges"], format!("Cannot create {op} on an edge that is a joint, fillet, chamfer, or internal to the sheet metal model.")),
        (Some(w), None) => (fa, fb, w),
        (None, Some(w)) => (fb, fa, w),
        (None, None) => {
            if data.bend_of_face(&body.faces[fa]).is_some() || data.bend_of_face(&body.faces[fb]).is_some() {
                return err_at(&["edges"], format!("Cannot create {op} on a bend."));
            }
            return err_at(&["edges"], format!("Cannot create {op} on an edge that is a joint, fillet, chamfer, or internal to the sheet metal model."));
        }
    };
    let wf = body.shape.sub_shape(ShapeType::Face, wall_face)?;
    let sf = body.shape.sub_shape(ShapeType::Face, side_face)?;
    let Some((_, nw)) = plane_of(&wf) else { return err("The wall face must be planar") };
    let Some((_, u)) = plane_of(&sf) else { return err("The edge must be on a planar side face") };
    if nw.dot(u).abs() > 1e-6 || u.dot(d).abs() > 1e-6 {
        return err("The side face at the edge must be perpendicular to the wall");
    }
    let v = if opposite { -nw } else { nw };
    let o = if opposite { p0 - nw * t } else { p0 };
    Ok(EdgeFrame { bi, wall, on_ref, inner_is_wall_face: !opposite, o, u, v, d, len, p0, p1, name: body.edges[ei].clone(), op })
}

/// Cross-section and placement of one flange/hem.
#[derive(Clone, Debug)]
pub struct FlangeGeom {
    pub angle: f64,
    pub radius: f64,
    pub k_factor: f64,
    /// Start of the bend region on the base (`<= 0` trims the base).
    pub u_t: f64,
    /// Length of the straight wall after the bend (0 = none).
    pub straight: f64,
    /// Extent of the flange along the edge.
    pub s_flange: (f64, f64),
    /// Extent of the base trim along the edge.
    pub s_trim: (f64, f64),
    pub kind: BendKind,
    /// Cut automatic bend reliefs where the base continues past the flange.
    pub relief: bool,
}

impl FlangeGeom {
    fn center(&self) -> DVec2 {
        DVec2::new(self.u_t, self.radius)
    }
    /// Inner / outer end points of the bend in (u, v).
    fn bend_end(&self, t: f64) -> (DVec2, DVec2) {
        let psi = self.angle - FRAC_PI_2;
        let r = DVec2::from_angle(psi);
        (self.center() + r * self.radius, self.center() + r * (self.radius + t))
    }
    fn dir(&self) -> DVec2 {
        DVec2::from_angle(self.angle)
    }
}

/// Setback factor for alignment and length (angles past 90 degrees use 90).
fn tan_half(angle: f64) -> f64 {
    (angle.min(FRAC_PI_2) * 0.5).tan()
}

/// Bend start for a flange alignment.
pub fn alignment_offset(alignment: &str, angle: f64, r: f64, t: f64) -> f64 {
    let th = tan_half(angle);
    match alignment {
        "INNER" => -r * th,
        "MIDDLE" => -(r + t * 0.5) * th,
        "BEND" => 0.0,
        _ => -(r + t) * th,
    }
}

/// Create the flange: trim, extrude the section, fuse, reliefs, record.
pub fn build(ev: &mut EvalCtx, ef: &EdgeFrame, g: &FlangeGeom, data: &mut SheetMetalData) -> FResult<()> {
    let t = data.thickness;
    let feature = ev.feature;
    let body = ev.ctx.bodies[ef.bi].clone();
    let mut cur = body_named(&body);
    let dn = ef.d.dot(ef.n());
    let zr = |s: (f64, f64)| [s.0 * dn, s.1 * dn];
    // Trim the base back to the bend start.
    if g.u_t < -1e-12 {
        let tool = oriented_box(ef.o, ef.u, ef.v, [g.u_t, t * 0.5], [-t, 0.0], zr(g.s_trim))?;
        let tool = tool_named(tool, |i| side_name(feature, &ef.name, 20 + i as u16));
        let (s, n) = bool_named(ev, k::BooleanOp::Cut, &cur, &[tool])?;
        cur = NamedShape::new(s, n);
    }
    // Bend + wall cross-section.
    let (sa, sb) = g.s_flange;
    if sb - sa < 1e-9 {
        return if ef.op == "hem" { err("Cannot create hem.") } else { err_at(&["edges"], "Cannot create automatic miter between selected edges.") };
    }
    let c = g.center();
    let (pi, po) = g.bend_end(t);
    let f = g.dir();
    let mut segs: Vec<(k::Segment2d, u16)> = vec![(arc3(c, g.radius, -FRAC_PI_2, g.angle), 0)];
    if g.straight > 1e-12 {
        let (qi, qo) = (pi + f * g.straight, po + f * g.straight);
        segs.push((k::Segment2d::Line { a: pi, b: qi }, 1));
        segs.push((k::Segment2d::Line { a: qi, b: qo }, 2));
        segs.push((k::Segment2d::Line { a: qo, b: po }, 3));
    } else {
        segs.push((k::Segment2d::Line { a: pi, b: po }, 2));
    }
    segs.push((arc3(c, g.radius + t, g.angle - FRAC_PI_2, -g.angle), 4));
    segs.push((k::Segment2d::Line { a: DVec2::new(g.u_t, -t), b: DVec2::new(g.u_t, 0.0) }, 5));
    let plain: Vec<k::Segment2d> = segs.iter().map(|s| s.0.clone()).collect();
    let origin = ef.at(0.0, 0.0, sa);
    let tracked = local_face(origin, ef.u, ef.v, &plain)?;
    let n_edges = tracked.shape.count(ShapeType::Edge);
    let mut edge_sources: Vec<Source> = (0..n_edges as u32).map(|i| Source::Index(100 + i)).collect();
    for (i, &e) in tracked.segment_edges.iter().enumerate() {
        if let Some(slot) = edge_sources.get_mut(e as usize) {
            *slot = piece_source(&ef.name, feature, segs[i].1);
        }
    }
    let frame = crate::display::PlaneFrame { origin, x: ef.u, y: ef.v, normal: ef.n() };
    let profile = Profile { shape: tracked.shape, frame, edge_sources, key: Source::Topo(ef.name.clone()), from_face: false };
    let res = k::extrude(&profile.shape, ef.d * (sb - sa))?;
    let names = sweep_names(ev, &profile, &res, true, true);
    let tool = NamedShape::new(res.shape, names);
    let (shape, names) = bool_named(ev, k::BooleanOp::Fuse, &cur, &[tool])?;
    if solid_count(&shape) != 1 {
        return err_at(&["edges"], format!("Cannot create {}.", ef.op));
    }
    cur = NamedShape::new(shape, names);
    // Automatic bend reliefs where the base continues past the flange ends.
    if g.relief && g.u_t < -1e-12 {
        let (style, depth, width) = relief_size(data, g.radius);
        for (end, s_end, sign) in [(0u16, g.s_trim.0, -1.0), (1u16, g.s_trim.1, 1.0)] {
            let probe = ef.at(g.u_t * 0.5, -t * 0.5, s_end + sign * (t * 0.5).min(width * 0.5));
            if !matches!(cur.shape.classify_point(probe, 1e-9), Ok(k::PointClass::Inside)) {
                continue;
            }
            let u_deep = g.u_t - depth;
            let segs = relief_segments(style, u_deep, t * 0.5, s_end, sign * width);
            let face = local_face(ef.at(0.0, -t, 0.0), ef.u, ef.d, &segs)?;
            let prism = k::extrude(&face.shape, ef.v * t)?.shape;
            let tool = tool_named(prism, |i| side_name(feature, &ef.name, 40 + end * 10 + i as u16));
            let (s, n) = bool_named(ev, k::BooleanOp::Cut, &cur, &[tool])?;
            if solid_count(&s) == 1 {
                cur = NamedShape::new(s, n);
            }
        }
    }
    ev.replace_body(ef.bi, cur.shape, cur.names.faces);
    // Record the wall and the bend.
    let body = ev.ctx.bodies[ef.bi].clone();
    let ref_inner = ef.on_ref == ef.inner_is_wall_face;
    let child = if g.straight > 1e-12 {
        let (rk, bk) = if ref_inner { (1, 3) } else { (3, 1) };
        data.walls.push(Wall {
            feature,
            faces: current_names(&body, &[side_name(feature, &ef.name, rk)]),
            back: current_names(&body, &[side_name(feature, &ef.name, bk)]),
        });
        Some(data.walls.len() - 1)
    } else {
        None
    };
    let n = ef.n();
    let (s0, s1) = (sa * dn, sb * dn);
    data.bends.push(Bend {
        feature,
        kind: g.kind,
        parent: ef.wall,
        child,
        // Axis points: at(u_t, R, s) = origin + axis * (s * dn).
        origin: ef.at(g.u_t, g.radius, 0.0),
        axis: n,
        angle: g.angle,
        radius: g.radius,
        k_factor: g.k_factor,
        dir_a: -ef.u,
        radial_a: -ef.v,
        s0: s0.min(s1),
        s1: s0.max(s1),
        ref_inner,
        inner_faces: current_names(&body, &[side_name(feature, &ef.name, 0)]),
        outer_faces: current_names(&body, &[side_name(feature, &ef.name, 4)]),
        flat: false,
        birth: 0, joint: None,
    });
    Ok(())
}

/// Model relief style, depth past the bend start and width.
pub fn relief_size(data: &SheetMetalData, radius: f64) -> (BendReliefStyle, f64, f64) {
    let t = data.thickness;
    match data.bend_relief {
        BendReliefStyle::Tear => (BendReliefStyle::Rectangle, 0.0, data.minimal_clearance.max(1e-5)),
        s => (s, ((data.bend_relief_depth_scale - 1.0) * radius).max(t), (t * data.bend_relief_width_scale).max(1e-5)),
    }
}

/// A resolved edge selection: (body, edge index).
fn resolve_edges(ev: &EvalCtx, id: &str, op: &str) -> FResult<Vec<(usize, usize)>> {
    let refs = ev.query(id);
    let mut out = Vec::new();
    for r in refs {
        let got = resolve_flange_edge(ev, r, op)?;
        if got.is_empty() && matches!(r, Reference::Edge(_) | Reference::Face(_)) {
            return err_at(&["edges"], format!("Select edges to {op}."));
        }
        for g in got {
            if !out.contains(&g) {
                out.push(g);
            }
        }
    }
    Ok(out)
}

fn by_body(items: &[(usize, usize)]) -> BTreeMap<usize, Vec<usize>> {
    let mut m: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for &(b, e) in items {
        m.entry(b).or_default().push(e);
    }
    m
}

/// Flange angle from the angle control options.
fn flange_angle(ev: &EvalCtx, ef: &EdgeFrame) -> FResult<f64> {
    let in_uv = |x: DVec3| DVec2::new(x.dot(ef.u), x.dot(ef.v));
    let theta = match ev.enum_("angleControlType") {
        "ALIGN_GEOMETRY" => {
            let r = ev.query("parallelEntity").first().ok_or_else(|| crate::regen::eval::FeatureError::at(&["parallelEntity"], "Select an entity to align to."))?;
            let plane = match r {
                Reference::Plane(_) | Reference::FeaturePlane(_) | Reference::ConstructionPlane { .. } | Reference::Face(_) => ev.resolve_plane(r).map(|p| p.normal),
                _ => None,
            };
            let f = match plane {
                Some(m) => m.cross(ef.d),
                None => {
                    let e = ev.resolve_direction(r).ok_or_else(|| crate::regen::eval::FeatureError::at(&["parallelEntity"], "Select an entity to align to."))?;
                    e - ef.d * e.dot(ef.d)
                }
            };
            let mut f2 = in_uv(f);
            if f2.length() < 1e-9 {
                return err_at(&["edges", "parallelEntity"], if plane.is_some() { "Flange edge and alignment plane must be parallel." } else { "Flange edge and alignment edge cannot be parallel." });
            }
            if f2.y < 0.0 || (f2.y.abs() < 1e-12 && f2.x < 0.0) {
                f2 = -f2;
            }
            f2.y.atan2(f2.x)
        }
        "ANGLE_FROM_DIRECTION" => {
            let r = ev.query("directionEntity").first().ok_or_else(|| crate::regen::eval::FeatureError::at(&["directionEntity"], "Select an entity to align from."))?;
            let e = ev.resolve_direction(r).ok_or_else(|| crate::regen::eval::FeatureError::at(&["directionEntity"], "Select an entity to align from."))?;
            let e2 = in_uv(e);
            if e2.length() < 1e-9 {
                return crate::regen::eval::err_at(&["edges", "directionEntity"], "Flange edge and alignment direction cannot be parallel.");
            }
            let a = ev.angle("angleFromDirection")?;
            let mut th = e2.y.atan2(e2.x) + if ev.bool("angleFromDirectionOppositeAngle") { -a } else { a };
            while th > PI {
                th -= 2.0 * PI;
            }
            while th <= -PI {
                th += 2.0 * PI;
            }
            th
        }
        _ => ev.angle("bendAngle")?,
    };
    if theta < 1e-6 || theta > PI - 1e-6 {
        return match ev.enum_("angleControlType") {
            "ALIGN_GEOMETRY" => err_at(&["edges", "parallelEntity"], "Cannot create flange with a 0 degree bend angle."),
            "ANGLE_FROM_DIRECTION" => err_at(&["edges", "directionEntity"], "Cannot create flange with a 0 degree bend angle."),
            _ => err_at(&["bendAngle"], "Error regenerating: a parameter is out of range."),
        };
    }
    Ok(theta)
}

/// Straight wall length from the end type.
fn flange_straight(ev: &EvalCtx, ef: &EdgeFrame, angle: f64, r: f64, t: f64, u_t: f64) -> FResult<f64> {
    let limit = ev.enum_("limitType");
    let ls = if limit == "UP_TO_ENTITY" || limit == "UP_TO_ENTITY_OFFSET" {
        let ent = ev.query("limitEntity").first().ok_or_else(|| crate::regen::eval::FeatureError::at(&["limitEntity"], "Cannot resolve limit entity for flange end condition."))?;
        let c = DVec2::new(u_t, r);
        let po2 = c + DVec2::from_angle(angle - FRAC_PI_2) * (r + t);
        let po = ef.at(po2.x, po2.y, ef.len * 0.5);
        let f = ef.u * angle.cos() + ef.v * angle.sin();
        let mut l = if let Some(q) = ev.resolve_vertex_point(ent) {
            (q - po).dot(f)
        } else if let Some(p) = ev.resolve_plane(ent) {
            let den = f.dot(p.normal);
            if den.abs() < 1e-9 {
                return crate::regen::eval::err_at(&["limitEntity"], "Cannot create flange up to selected entity.");
            }
            (p.origin - po).dot(p.normal) / den
        } else {
            return crate::regen::eval::err_at(&["limitEntity"], "Cannot resolve limit entity for flange end condition.");
        };
        if limit == "UP_TO_ENTITY_OFFSET" {
            let o = ev.length("offset")?;
            l += if ev.bool("oppositeOffsetDirection") { -o } else { o };
        }
        l
    } else {
        ev.length("distance")? - (r + t) * tan_half(angle)
    };
    if ls <= 1e-9 {
        return if limit == "BLIND" { err("Cannot create flange.") } else { err_at(&["oppositeDirection"], "Flange direction is opposite of limit entity.") };
    }
    Ok(ls)
}

/// Partial flange bounds along the edge.
fn partial_bounds(ev: &EvalCtx, ef: &EdgeFrame) -> FResult<(f64, f64)> {
    let bound = |ty: &str, dist: &str, ent: &'static str, off: &str, flip: &str| -> FResult<f64> {
        match ev.enum_(ty) {
            "UP_TO_ENTITY" | "UP_TO_ENTITY_OFFSET" => {
                let r = ev.query(ent).first().ok_or_else(|| FeatureError::at(&[ent], "Select a limit entity for flange bound."))?;
                let mut s = if let Some(q) = ev.resolve_vertex_point(r) {
                    ef.param(q)
                } else if let Some(p) = ev.resolve_plane(r) {
                    let den = ef.d.dot(p.normal);
                    if den.abs() < 1e-9 {
                        return err_at(&[ent], "Limit entity for flange bound cannot be parallel to flange edge.");
                    }
                    (p.origin - ef.o).dot(p.normal) / den
                } else {
                    return err_at(&[ent], "Limit entity for flange bound must be a vertex or plane.");
                };
                if ev.is(ty, "UP_TO_ENTITY_OFFSET") {
                    let o = ev.length(off)?;
                    s += if ev.bool(flip) { -o } else { o };
                }
                Ok(s)
            }
            _ => Ok(ev.length(dist)?),
        }
    };
    let first = bound("firstBoundType", "firstBoundOffset", "firstBoundingEntity", "firstBoundingEntityOffset", "firstBoundingEntityOppositeOffsetDirection")?;
    let blind1 = !matches!(ev.enum_("firstBoundType"), "UP_TO_ENTITY" | "UP_TO_ENTITY_OFFSET");
    let (mut a, mut b) = (first, ef.len);
    if ev.bool("hasSecondBound") {
        let second = bound("secondBoundType", "secondBoundOffset", "secondBoundingEntity", "secondBoundingEntityOffset", "secondBoundingEntityOppositeOffsetDirection")?;
        let blind2 = !matches!(ev.enum_("secondBoundType"), "UP_TO_ENTITY" | "UP_TO_ENTITY_OFFSET");
        b = if blind2 { ef.len - second } else { second };
    }
    if ev.bool("flipFlangeBounds") {
        // Measure the blind bounds from the other end.
        if blind1 {
            let len = ef.len;
            let (na, nb) = (len - b, len - a);
            a = na;
            b = nb;
        }
    }
    let (a, b) = (a.min(b), a.max(b));
    if a < -1e-9 || b > ef.len + 1e-9 {
        return err("Cannot extend partial flange beyond base edge.");
    }
    if b - a < 1e-9 {
        return err("Cannot create zero width partial flange.");
    }
    Ok((a.max(0.0), b.min(ef.len)))
}

/// Shorten flanges meeting at shared vertices so their bends do not collide.
fn shorten_corners(frames: &[EdgeFrame], geoms: &mut [FlangeGeom], gap: f64) {
    let tol = 1e-7;
    let n = frames.len();
    let orig: Vec<FlangeGeom> = geoms.to_vec();
    for i in 0..n {
        for j in 0..n {
            if i == j || frames[i].bi != frames[j].bi {
                continue;
            }
            let other = (-orig[j].u_t).max(0.0) + gap;
            for (end, p) in [(0, frames[i].p0), (1, frames[i].p1)] {
                if (p - frames[j].p0).length() < tol || (p - frames[j].p1).length() < tol {
                    if end == 0 && geoms[i].s_flange.0 < tol {
                        geoms[i].s_flange.0 = other;
                    } else if end == 1 && geoms[i].s_flange.1 > frames[i].len - tol {
                        geoms[i].s_flange.1 = frames[i].len - other;
                    }
                }
            }
        }
    }
}

pub fn flange(ev: &mut EvalCtx) -> FResult<()> {
    if ev.query("edges").is_empty() {
        return crate::regen::eval::err_at(&["edges"], "Select edges to flange.");
    }
    let edges = resolve_edges(ev, "edges", "flange")?;
    if edges.is_empty() {
        return crate::regen::eval::err_at(&["edges"], "Select edges to flange.");
    }
    for (bi, eis) in by_body(&edges).into_iter().rev() {
        let mut data = (*active_data(ev, bi)?).clone();
        let t = data.thickness;
        let r = if ev.bool("useDefaultRadius") { data.radius } else { ev.length("bendRadius")? };
        if r <= 0.0 {
            return crate::regen::eval::err_at(&["bendRadius"], "Error regenerating: a parameter is out of range.");
        }
        let opposite = ev.bool("oppositeDirection");
        let mut frames = Vec::new();
        let mut geoms = Vec::new();
        for &ei in &eis {
            let ef = edge_frame(ev, &data, bi, ei, opposite, "flange")?;
            let angle = flange_angle(ev, &ef)?;
            let u_t = alignment_offset(ev.enum_("flangeAlignment"), angle, r, t);
            let straight = flange_straight(ev, &ef, angle, r, t, u_t)?;
            let partial = ev.bool("isPartialFlange");
            let s = if partial { partial_bounds(ev, &ef)? } else { (0.0, ef.len) };
            geoms.push(FlangeGeom { angle, radius: r, k_factor: data.k_factor, u_t, straight, s_flange: s, s_trim: s, kind: BendKind::Bend, relief: true });
            frames.push(ef);
        }
        shorten_corners(&frames, &mut geoms, data.minimal_clearance);
        for (ef, g) in frames.iter().zip(&geoms) {
            build(ev, ef, g, &mut data)?;
        }
        put_data(ev, bi, data);
    }
    Ok(())
}

/// Tear drop: bend angle and straight length for a total length and tip gap.
fn tear_drop(r: f64, t: f64, gap: f64, total: f64) -> FResult<(f64, f64)> {
    // std (sheetMetalHem.fs): at least twice the inner radius plus the sheet (live hem-type-tear-drop: 8 mm with R 2.3
    // on a 2 mm plate is refused).
    if total < 2.0 * (r + t) + 1e-12 {
        return crate::regen::eval::err_at(&["length"], "Cannot create hem, increase total length.");
    }
    if gap >= r {
        return crate::regen::eval::err_at(&["tipGap"], "Cannot create hem, decrease gap.");
    }
    let eval = |th: f64| -> (f64, f64) {
        let psi = th - FRAC_PI_2;
        let ls = (gap - r - r * psi.sin()) / th.sin();
        let tot = r + t - (r + t) * psi.cos() - ls * th.cos();
        (ls, tot)
    };
    let (mut lo, mut hi) = (PI + 1e-4, 1.5 * PI);
    if eval(hi).1 > total {
        return crate::regen::eval::err_at(&["length"], "Cannot create hem, increase total length.");
    }
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if eval(mid).1 > total { lo = mid } else { hi = mid }
    }
    let th = 0.5 * (lo + hi);
    let (ls, _) = eval(th);
    if ls <= 1e-9 {
        return crate::regen::eval::err_at(&["length"], "Cannot create hem, increase total length.");
    }
    Ok((th, ls))
}

pub fn hem(ev: &mut EvalCtx) -> FResult<()> {
    if ev.query("edges").is_empty() {
        return crate::regen::eval::err_at(&["edges"], "Select edges to hem.");
    }
    let edges = resolve_edges(ev, "edges", "hem")?;
    if edges.is_empty() {
        return crate::regen::eval::err_at(&["edges"], "Select edges to hem.");
    }
    for (bi, eis) in by_body(&edges).into_iter().rev() {
        let mut data = (*active_data(ev, bi)?).clone();
        let t = data.thickness;
        let ty = ev.enum_("hemType").to_string();
        let (angle, r, straight, kind, k_factor) = match ty.as_str() {
            "ROLLED" => {
                let a = ev.angle("angle")?;
                if !(PI - 1e-9..2.0 * PI).contains(&a) {
                    return crate::regen::eval::err_at(&["angle"], "Error regenerating: a parameter is out of range.");
                }
                // A bend, so the bend K factor (the Rolled K factor is for rolled walls, Onshape
                // Help "Sheet metal model"; std's host develops it with the model's K factor).
                (a, ev.length("innerRadius")?, 0.0, BendKind::Rolled, data.k_factor)
            }
            "TEAR_DROP" => {
                let r = ev.length("innerRadius")?;
                let gap = if ev.bool("useMinimalGapForTipGap") { data.minimal_clearance } else { ev.length("tipGap")? };
                let (a, ls) = tear_drop(r, t, gap, ev.length("length")?)?;
                (a, r, ls, BendKind::Hem, data.k_factor)
            }
            _ => {
                let r = if ev.bool("useMinimalGapForStraight") { data.minimal_clearance * 0.5 } else { ev.length("innerRadius")? };
                let ls = ev.length("length")? - (r + t);
                if ls <= 1e-9 {
                    return crate::regen::eval::err_at(&["length"], "Cannot create hem, increase total length.");
                }
                (PI, r, ls, BendKind::Hem, data.k_factor)
            }
        };
        if r <= 0.0 {
            return crate::regen::eval::err_at(&["innerRadius"], "Error regenerating: a parameter is out of range.");
        }
        let u_t = if ev.is("hemAlignment", "IN_PLACE") { 0.0 } else { -(r + t) };
        let mut frames = Vec::new();
        let mut geoms = Vec::new();
        for &ei in &eis {
            let ef = edge_frame(ev, &data, bi, ei, ev.bool("oppositeDirection"), "hem")?;
            let s = (0.0, ef.len);
            geoms.push(FlangeGeom { angle, radius: r, k_factor, u_t, straight, s_flange: s, s_trim: s, kind, relief: true });
            frames.push(ef);
        }
        shorten_corners(&frames, &mut geoms, data.minimal_clearance);
        for (ef, g) in frames.iter().zip(&geoms) {
            build(ev, ef, g, &mut data)?;
        }
        put_data(ev, bi, data);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// std's sheetMetalHem.fs: a tear drop needs a total length of at least twice the inner radius plus the sheet (live
    /// hem-type-tear-drop: 8 mm at R 2.3 on a 2 mm plate is refused, Total length marked).
    #[test]
    fn a_tear_drop_shorter_than_twice_radius_and_sheet_is_refused() {
        let (r, t, gap) = (0.0023, 0.002, 0.0005);
        assert!(tear_drop(r, t, gap, 0.008).is_err());
        assert!(tear_drop(r, t, gap, 0.0125).is_ok());
    }
}
