//! Frames (03b FR-01, FR-03..07): Frame, Frame trim, Gusset, End cap, Cut
//! list and Tag.
//!
//! Frame sweeps a profile (built-in library, see `frames_profiles`, or a
//! sketch of the Part Studio named in `profileSketch`) along chained path
//! edges / sketch curves; each path edge becomes its own part. Corners at
//! shared vertices are mitered, butted, coped or left alone. Every frame
//! part (segments, gussets, end caps) is recorded in
//! [`Context::frame_members`] so [`cut_list`] can compute the cut list rows
//! (profile, length, end angles) from the current geometry.

use super::frames_profiles::{self, FrameProfile};
use super::sheetmetal_util::{bool_named, oriented_box, piece_source, plane_of, tool_named};
use crate::display::BodyKind;
use crate::naming::Name;
use crate::reference::{BodyId, FeatureId, Reference, Role, Source, TopoName};
use crate::regen::context::{Body, Context};
use crate::regen::eval::{EvalCtx, FResult, FeatureError, NamedShape, Profile, err, region_profile, sweep_names};
use glam::{DQuat, DVec2, DVec3};
use noshape_kernel as k;
use noshape_kernel::{Shape, ShapeType};
use std::collections::BTreeMap;
use std::sync::Arc;

/// What a frame part is (cut list grouping and description).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemberKind {
    Segment,
    Gusset,
    EndCap,
}

/// Frame metadata of a part.
#[derive(Clone, Debug)]
pub struct FrameMember {
    pub feature: FeatureId,
    pub kind: MemberKind,
    pub standard: String,
    pub description: String,
    /// Straight segment axis (point, unit direction).
    pub axis: Option<(DVec3, DVec3)>,
    /// Length of the swept path (approximate length for curved members).
    pub path_length: f64,
}

/// One row of a cut list.
#[derive(Clone, Debug, PartialEq)]
pub struct CutListRow {
    pub item: usize,
    pub qty: usize,
    pub standard: String,
    pub description: String,
    /// Overall (long point) length of straight members.
    pub length: Option<f64>,
    /// End cut angles relative to a square cut (radians).
    pub angle1: Option<f64>,
    pub angle2: Option<f64>,
    pub bodies: Vec<BodyId>,
}

fn fe(m: impl Into<String>) -> FeatureError {
    FeatureError::new(m)
}

fn face(feature: FeatureId, role: Role) -> Name {
    Arc::new(TopoName::face(feature, role))
}

/// Measured length and end angles of a straight member.
fn measure_member(body: &Body, axis: (DVec3, DVec3)) -> (f64, Option<f64>, Option<f64>) {
    let (o, a) = axis;
    let mut lo = f64::INFINITY;
    let mut hi = f64::NEG_INFINITY;
    for v in body.shape.vertices() {
        if let Ok(p) = v.vertex_point() {
            let s = (p - o).dot(a);
            lo = lo.min(s);
            hi = hi.max(s);
        }
    }
    if lo > hi {
        return (0.0, None, None);
    }
    let mid = (lo + hi) * 0.5;
    // End faces: planar faces not parallel to the axis, largest per end.
    let mut ends: [(f64, Option<f64>); 2] = [(0.0, None), (0.0, None)];
    let mut curved_end = [false, false];
    for f in body.shape.faces() {
        let Ok(c) = f.centroid() else { continue };
        let end = usize::from((c - o).dot(a) > mid);
        match plane_of(&f) {
            Some((_, n)) => {
                let cosv = n.dot(a).abs();
                if cosv < 1e-6 {
                    continue;
                }
                let area = f.area().unwrap_or(0.0);
                if area > ends[end].0 {
                    ends[end] = (area, Some(cosv.clamp(0.0, 1.0).acos()));
                }
            }
            None => {
                // A cylindrical side face of a round tube is parallel to the axis.
                if let Ok(k::Surface::Cylinder { axis: ca, .. }) = f.face_surface()
                    && ca.normalize().dot(a).abs() > 1.0 - 1e-6
                {
                    continue;
                }
                curved_end[end] = true;
            }
        }
    }
    let ang = |i: usize| if curved_end[i] { None } else { ends[i].1 };
    (hi - lo, ang(0), ang(1))
}

/// Cut list rows for the frame parts among `bodies` (all frame parts when
/// `None`), grouped by profile, length and end angles.
pub fn cut_list(ctx: &Context, bodies: Option<&[BodyId]>) -> Vec<CutListRow> {
    struct Item {
        key: (String, String, i64, i64, i64),
        row: CutListRow,
    }
    let mut items: Vec<Item> = Vec::new();
    for b in &ctx.bodies {
        if bodies.is_some_and(|ids| !ids.contains(&b.id)) {
            continue;
        }
        let Some(m) = ctx.frame_members.get(&b.id) else { continue };
        let (length, a1, a2) = match (m.kind, m.axis) {
            (MemberKind::Segment, Some(axis)) => {
                let (l, a1, a2) = measure_member(b, axis);
                (Some(l), a1, a2)
            }
            (MemberKind::Segment, None) => (Some(m.path_length), None, None),
            _ => (None, None, None),
        };
        // Angles are unordered for grouping.
        let q = |v: Option<f64>| v.map(|x| (x.to_degrees() * 100.0).round() as i64).unwrap_or(-1);
        let (qa, qb) = (q(a1).min(q(a2)), q(a1).max(q(a2)));
        let ql = length.map(|l| (l * 1e5).round() as i64).unwrap_or(-1);
        let key = (m.standard.clone(), m.description.clone(), ql, qa, qb);
        if let Some(it) = items.iter_mut().find(|it| it.key == key) {
            it.row.qty += 1;
            it.row.bodies.push(b.id.clone());
            continue;
        }
        let row = CutListRow { item: 0, qty: 1, standard: m.standard.clone(), description: m.description.clone(), length, angle1: a1, angle2: a2, bodies: vec![b.id.clone()] };
        items.push(Item { key, row });
    }
    items
        .into_iter()
        .enumerate()
        .map(|(i, mut it)| {
            it.row.item = i + 1;
            it.row
        })
        .collect()
}

// ------------------------------------------------------------------ paths

#[derive(Clone)]
struct PathSeg {
    edge: Shape,
    name: Name,
    start: DVec3,
    end: DVec3,
    t0: DVec3,
    t1: DVec3,
    line: bool,
    length: f64,
}

struct FramePath {
    segs: Vec<PathSeg>,
    closed: bool,
}

fn source_name(src: &Source, feature: FeatureId) -> Name {
    match src {
        Source::Topo(n) => n.clone(),
        Source::Sketch { sketch, .. } => Arc::new(TopoName::Wire { feature: *sketch, source: src.clone() }),
        other => Arc::new(TopoName::Wire { feature, source: other.clone() }),
    }
}

fn oriented_seg(edge: &Shape, name: Name, reversed: bool) -> FResult<PathSeg> {
    let info = edge.edge_info()?;
    let ta = edge.tangent_at(info.first_param)?.normalize();
    let tb = edge.tangent_at(info.last_param)?.normalize();
    let line = matches!(info.curve, k::Curve::Line { .. });
    let (start, end, t0, t1, e) = if reversed { (info.end, info.start, -tb, -ta, edge.reversed()?) } else { (info.start, info.end, ta, tb, edge.clone()) };
    Ok(PathSeg { edge: e, name, start, end, t0, t1, line, length: info.length })
}

/// Chain path edges into paths (keeping the first edge's direction).
fn chain_paths(mut edges: Vec<(Shape, Name)>) -> FResult<Vec<FramePath>> {
    let tol = 1e-7;
    let mut paths = Vec::new();
    // Branch check: no point may be shared by more than two edges.
    let mut pts: Vec<DVec3> = Vec::new();
    for (e, _) in &edges {
        let (a, b) = e.endpoints()?;
        pts.push(a);
        pts.push(b);
    }
    for p in &pts {
        if pts.iter().filter(|q| (**q - *p).length() < tol).count() > 2 {
            return err("Frame paths must not branch");
        }
    }
    while !edges.is_empty() {
        let (e0, n0) = edges.remove(0);
        let mut segs = vec![oriented_seg(&e0, n0, false)?];
        loop {
            let end = segs.last().map(|s| s.end).unwrap_or_default();
            let start = segs[0].start;
            let mut progressed = false;
            let mut i = 0;
            while i < edges.len() {
                let (a, b) = edges[i].0.endpoints()?;
                if (a - end).length() < tol || (b - end).length() < tol {
                    let (e, n) = edges.remove(i);
                    let rev = (b - end).length() < tol && (a - end).length() >= tol;
                    segs.push(oriented_seg(&e, n, rev)?);
                    progressed = true;
                    break;
                }
                if (a - start).length() < tol || (b - start).length() < tol {
                    let (e, n) = edges.remove(i);
                    let rev = (a - start).length() < tol;
                    segs.insert(0, oriented_seg(&e, n, rev)?);
                    progressed = true;
                    break;
                }
                i += 1;
            }
            if !progressed {
                break;
            }
        }
        let closed = segs.len() > 1 && (segs[0].start - segs[segs.len() - 1].end).length() < tol;
        paths.push(FramePath { segs, closed });
    }
    Ok(paths)
}

/// Path edges from the selections: edges, sketch curves, wire bodies, face
/// boundaries and straight lines between consecutive selected vertices.
fn path_edges(ev: &EvalCtx) -> FResult<Vec<(Shape, Name)>> {
    let refs = ev.query("selections");
    let mut out: Vec<(Shape, Name)> = Vec::new();
    let mut verts: Vec<DVec3> = Vec::new();
    for r in refs {
        let one = std::slice::from_ref(r);
        match r {
            Reference::Face(_) => {
                for (bi, fi) in ev.resolve_faces(one) {
                    let b = &ev.ctx.bodies[bi];
                    let f = b.shape.sub_shape(ShapeType::Face, fi)?;
                    let ef = b.shape.edge_faces()?;
                    for e in f.edges() {
                        if let Some(ei) = b.shape.index_of(&e, ShapeType::Edge)
                            && ef.get(ei).is_some_and(|v| v.contains(&(fi as u32)))
                        {
                            out.push((e, b.edges[ei].clone()));
                        }
                    }
                }
            }
            Reference::Vertex(_) => {
                if let Some(p) = ev.resolve_vertex_point(r) {
                    verts.push(p);
                }
            }
            Reference::SketchEntity { sketch, entity } => {
                let is_point = ev.ctx.sketch(*sketch).is_some_and(|s| s.sketch.curve(noshape_sketch::EntityId(*entity)).is_none());
                if is_point {
                    if let Some(p) = ev.resolve_vertex_point(r) {
                        verts.push(p);
                    }
                    continue;
                }
                for se in super::helpers_a::resolve_curves(ev, one)? {
                    out.push((se.shape, source_name(&se.source, ev.feature)));
                }
            }
            _ => {
                for se in super::helpers_a::resolve_curves(ev, one)? {
                    out.push((se.shape, source_name(&se.source, ev.feature)));
                }
            }
        }
    }
    for (i, w) in verts.windows(2).enumerate() {
        if (w[1] - w[0]).length() > 1e-9 {
            out.push((k::make_line(w[0], w[1])?, Arc::new(TopoName::Wire { feature: ev.feature, source: Source::Index(i as u32) })));
        }
    }
    let mut uniq: Vec<(Shape, Name)> = Vec::new();
    for e in out {
        if !uniq.iter().any(|u| u.0.is_same(&e.0)) {
            uniq.push(e);
        }
    }
    Ok(uniq)
}

/// Profile from the `profileSketch` text: a sketch of this Part Studio with
/// that name, else the built-in library.
fn resolve_profile(ev: &EvalCtx, text: &str) -> FResult<(FrameProfile, Shape)> {
    let text = text.trim();
    if text.is_empty() {
        return err("Select a frame profile (for example \"SHS 40x40x3\" or the name of a profile sketch)");
    }
    if let Some(s) = ev.ctx.sketches.iter().find(|s| s.name == text).cloned() {
        // Regions that are not holes of other regions; use the largest.
        let mut faces: Vec<Shape> = Vec::new();
        for reg in &s.regions {
            let key = crate::regen::eval::encode_region_key(&reg.id);
            if let Ok(p) = region_profile(&s, reg, Source::Region { sketch: s.feature, key }) {
                faces.push(p.shape);
            }
        }
        let outers: Vec<Option<Shape>> = faces.iter().map(|f| outer_wire(f).and_then(|w| k::make_face_from_wire(&w, true).ok())).collect();
        let on = |s: &Shape, p: DVec3| k::make_vertex(p).ok().and_then(|v| k::distance(s, &v).ok()).is_some_and(|d| d.distance < 1e-9);
        let mut keep: Vec<Shape> = Vec::new();
        for (j, f) in faces.iter().enumerate() {
            let Ok(c) = f.centroid() else { continue };
            // A region filling a hole of another region is not material.
            let fills_hole = faces.iter().enumerate().any(|(i, g)| i != j && outers[i].as_ref().is_some_and(|o| on(o, c)) && !on(g, c));
            if !fills_hole {
                keep.push(f.clone());
            }
        }
        let face = keep.into_iter().max_by(|a, b| a.area().unwrap_or(0.0).total_cmp(&b.area().unwrap_or(0.0))).ok_or_else(|| fe("The profile sketch has no regions"))?;
        let plane = crate::regen::eval::kernel_plane(&s.frame);
        let to_xy = k::Transform::frame_to_frame(&plane, &k::Plane::xy());
        let face = face.transformed(&to_xy)?;
        let mut lo = DVec2::splat(f64::INFINITY);
        let mut hi = DVec2::splat(f64::NEG_INFINITY);
        for v in face.vertices() {
            let p = v.vertex_point()?;
            lo = lo.min(p.truncate());
            hi = hi.max(p.truncate());
        }
        if let Ok(bb) = face.bounding_box(true) {
            lo = lo.min(bb.min.truncate());
            hi = hi.max(bb.max.truncate());
        }
        let prof = FrameProfile { standard: String::new(), description: s.name.clone(), outer: Vec::new(), holes: Vec::new(), min: lo, max: hi };
        return Ok((prof, face));
    }
    let p = frames_profiles::parse(text).map_err(fe)?;
    let f = p.face()?;
    Ok((p, f))
}

fn outer_wire(f: &Shape) -> Option<Shape> {
    f.wires().into_iter().max_by(|a, b| {
        let d = |w: &Shape| w.bounding_box(false).map(|bb| bb.diagonal()).unwrap_or(0.0);
        d(a).total_cmp(&d(b))
    })
}

/// Rotation-minimizing frame transport along a segment.
fn transport(x: DVec3, seg: &PathSeg) -> DVec3 {
    if seg.line {
        return x;
    }
    let Ok(info) = seg.edge.edge_info() else { return x };
    let n = 32;
    let mut cur = x;
    let mut prev_t = seg.t0;
    for i in 1..=n {
        let p = info.first_param + (info.last_param - info.first_param) * i as f64 / n as f64;
        let Ok(t) = seg.edge.tangent_at(p) else { continue };
        let mut t = t.normalize();
        if seg.edge.orientation() == k::Orientation::Reversed {
            t = -t;
        }
        cur = DQuat::from_rotation_arc(prev_t, t) * cur;
        prev_t = t;
    }
    cur
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Corner {
    Miter,
    Butt,
    Coped,
    None,
}

fn corner_of(id: &str) -> Corner {
    match id {
        "BUTT" => Corner::Butt,
        "COPED_BUTT" => Corner::Coped,
        "NONE" => Corner::None,
        _ => Corner::Miter,
    }
}

#[derive(Default, Clone)]
struct Plan {
    ext0: f64,
    ext1: f64,
    /// (point, normal of the removed half-space, at start).
    cuts: Vec<(DVec3, DVec3, bool)>,
    /// (through segment, at start) to subtract (coped butt).
    copes: Vec<(usize, bool)>,
}

pub fn frame(ev: &mut EvalCtx) -> FResult<()> {
    let text = ev.params.text("profileSketch").to_string();
    let (profile, canon) = resolve_profile(ev, &text)?;
    let edges = path_edges(ev)?;
    if edges.is_empty() {
        return err("Select edges, curves or vertices defining the frame path");
    }
    let paths = chain_paths(edges)?;
    let feature = ev.feature;
    let align = profile.alignment_point(ev.integer("index").unwrap_or(4));
    let rp = profile.radius_about(align).max(1e-6);
    let big = ev.model_extent() + rp * 10.0;
    // Canonical profile with the alignment point at the origin.
    let mut canon = canon.translated(DVec3::new(-align.x, -align.y, 0.0))?;
    if ev.bool("mirrorProfile") {
        canon = canon.transformed(&k::Transform::Mirror(k::Plane::yz()))?;
    }
    let corners: Vec<(DVec2, f64)> = [profile.min, profile.max, DVec2::new(profile.min.x, profile.max.y), DVec2::new(profile.max.x, profile.min.y)].iter().map(|c| (*c - align, 0.0)).collect();
    let angle = ev.angle("angle").unwrap_or(0.0);
    let reference = ev.query("angleReference").first().and_then(|r| ev.resolve_direction(r));
    let default_corner = corner_of(ev.enum_("defaultCornerType"));
    let default_flip = ev.bool("defaultButtFlip");
    let mut overrides: Vec<(DVec3, Corner, bool)> = Vec::new();
    for item in ev.params.array("cornerOverrides") {
        if let Some(r) = item.query("vertex").first()
            && let Some(p) = ev.resolve_vertex_point(r)
        {
            overrides.push((p, corner_of(item.enum_("cornerType")), item.bool("cornerButtFlip")));
        }
    }
    let trim_planes: Vec<(DVec3, DVec3)> = if ev.bool("trim") { ev.query("trimPlanes").iter().filter_map(|r| ev.resolve_plane(r)).map(|p| (p.origin, p.normal)).collect() } else { Vec::new() };
    let trim_bodies: Vec<Shape> = if ev.bool("trim") { ev.resolve_bodies(ev.query("trimBodies")).into_iter().map(|i| ev.ctx.bodies[i].shape.clone()).collect() } else { Vec::new() };
    let merge_tangent = ev.bool("mergeTangentSegments");

    for (pi, path) in paths.iter().enumerate() {
        let n = path.segs.len();
        // Profile frames at segment starts / ends (x axis; y = t × x).
        let t0 = path.segs[0].t0;
        let up = reference.unwrap_or(if t0.z.abs() < 0.999 { DVec3::Z } else { DVec3::Y });
        let mut y = up - t0 * up.dot(t0);
        if y.length() < 1e-9 {
            y = t0.any_orthonormal_vector();
        }
        let y = y.normalize();
        let mut x = y.cross(t0).normalize();
        x = DQuat::from_axis_angle(t0, angle) * x;
        let mut x_start = Vec::with_capacity(n);
        let mut x_end = Vec::with_capacity(n);
        for (i, s) in path.segs.iter().enumerate() {
            if i > 0 {
                x = DQuat::from_rotation_arc(path.segs[i - 1].t1, s.t0) * x;
            }
            x = (x - s.t0 * x.dot(s.t0)).normalize();
            x_start.push(x);
            x = transport(x, s);
            x_end.push(x);
        }
        let offsets = |xa: DVec3, t: DVec3| -> Vec<DVec3> {
            let ya = t.cross(xa);
            corners.iter().map(|(c, _)| xa * c.x + ya * c.y).collect()
        };
        let mut plans = vec![Plan::default(); n];
        let mut tangent_join = vec![false; n];
        let junctions: Vec<usize> = if path.closed { (0..n).collect() } else { (0..n.saturating_sub(1)).collect() };
        for i in junctions {
            let j = (i + 1) % n;
            let (si, sj) = (&path.segs[i], &path.segs[j]);
            let v = si.end;
            let (t1, t2) = (si.t1, sj.t0);
            if t1.dot(t2) > 1.0 - 1e-9 {
                tangent_join[i] = true;
                continue;
            }
            let (ctype, flip) = overrides.iter().find(|o| (o.0 - v).length() < 1e-7).map(|o| (o.1, o.2)).unwrap_or((default_corner, default_flip));
            let phi = t1.dot(t2).clamp(-1.0, 1.0).acos();
            match ctype {
                Corner::None => {}
                Corner::Miter => {
                    let nm = (t1 + t2).normalize_or_zero();
                    if nm.length() < 0.5 || phi > 175f64.to_radians() {
                        return err("Segments fold back on themselves; a miter is not possible");
                    }
                    let ext = rp * (phi * 0.5).tan() * 1.1 + 1e-5;
                    if si.line {
                        plans[i].ext1 += ext;
                    }
                    if sj.line {
                        plans[j].ext0 += ext;
                    }
                    plans[i].cuts.push((v, nm, false));
                    plans[j].cuts.push((v, -nm, true));
                }
                Corner::Butt | Corner::Coped => {
                    // Through member continues past the vertex along `a`;
                    // the butting member leaves the vertex along `b`.
                    let (th, bt, a, b, xb, xt) = if !flip { (i, j, t1, t2, x_start[j], x_end[i]) } else { (j, i, -t2, -t1, x_end[i], x_start[j]) };
                    let pb = offsets(xb, if !flip { t2 } else { t1 });
                    let pt = offsets(xt, if !flip { t1 } else { t2 });
                    let e_t = pb.iter().map(|p| p.dot(a)).fold(0.0, f64::max) + rp * a.dot(b).abs();
                    let through_at_start = flip;
                    if path.segs[th].line {
                        if through_at_start { plans[th].ext0 += e_t } else { plans[th].ext1 += e_t }
                    }
                    let nn = (b - a * a.dot(b)).normalize_or_zero();
                    let h = pt.iter().map(|p| p.dot(nn)).fold(0.0, f64::max);
                    let butt_at_start = !flip;
                    if path.segs[bt].line {
                        let ext = if ctype == Corner::Coped { e_t + rp } else { rp };
                        if butt_at_start { plans[bt].ext0 += ext } else { plans[bt].ext1 += ext }
                    }
                    if ctype == Corner::Butt {
                        plans[bt].cuts.push((v + nn * h, -nn, butt_at_start));
                    } else {
                        plans[bt].copes.push((th, butt_at_start));
                    }
                }
            }
        }
        // Limit frame ends to planes: each plane trims the free end nearest to it.
        if !path.closed && !trim_planes.is_empty() {
            let ends = [(0usize, true), (n - 1, false)];
            let end_geom = |e: (usize, bool)| {
                let s = &path.segs[e.0];
                if e.1 { (s.start, -s.t0) } else { (s.end, s.t1) }
            };
            let mut best: [Option<(f64, DVec3, DVec3)>; 2] = [None, None];
            for &(q, m) in &trim_planes {
                let mut cand: Option<(usize, f64)> = None;
                for (k2, e) in ends.iter().enumerate() {
                    let (p, out) = end_geom(*e);
                    let den = out.dot(m);
                    if den.abs() < 1e-9 {
                        continue;
                    }
                    let lam = (q - p).dot(m) / den;
                    if cand.is_none_or(|c| lam.abs() < c.1.abs()) {
                        cand = Some((k2, lam));
                    }
                }
                if let Some((k2, lam)) = cand {
                    if best[k2].is_some() {
                        ev.warnings.push("Several trim planes apply to one frame end; the nearest is used".into());
                    }
                    if best[k2].is_none_or(|b| lam.abs() < b.0.abs()) {
                        best[k2] = Some((lam, q, m));
                    }
                }
            }
            for (k2, &(si, at_start)) in ends.iter().enumerate() {
                let Some((lam, q, m)) = best[k2] else { continue };
                let s = &path.segs[si];
                let (_, out) = end_geom((si, at_start));
                let c = out.dot(m).abs();
                let ext = lam + rp * (1.0 - c * c).max(0.0).sqrt() / c.max(1e-6) + 1e-5;
                if s.line {
                    if at_start { plans[si].ext0 += ext } else { plans[si].ext1 += ext }
                }
                let removed = if out.dot(m) > 0.0 { m } else { -m };
                plans[si].cuts.push((q, removed, at_start));
            }
        }
        // Build the segments.
        let mut built: Vec<NamedShape> = Vec::with_capacity(n);
        for (i, s) in path.segs.iter().enumerate() {
            let plan = &plans[i];
            let xs = x_start[i];
            let start = if s.line { s.start - s.t0 * plan.ext0 } else { s.start };
            let to = k::Plane::new(start, s.t0, xs);
            let place = k::Transform::frame_to_frame(&k::Plane::xy(), &to);
            let pf = canon.transformed(&place)?;
            let n_edges = pf.count(ShapeType::Edge);
            let edge_sources = (0..n_edges).map(|k| piece_source(&s.name, feature, k as u16)).collect();
            let frame = crate::display::PlaneFrame { origin: start, x: xs, y: s.t0.cross(xs), normal: s.t0 };
            let prof = Profile { shape: pf, frame, edge_sources, key: Source::Topo(s.name.clone()), from_face: false };
            let res = if s.line {
                k::extrude(&prof.shape, s.t0 * (s.length + plan.ext0 + plan.ext1))?
            } else {
                let wire = k::make_wire(std::slice::from_ref(&s.edge))?;
                k::sweep(&prof.shape, &wire, &k::SweepOptions::default()).map_err(|e| fe(format!("Frame sweep failed: {e}")))?
            };
            let names = sweep_names(ev, &prof, &res, true, true);
            let mut cur = NamedShape::new(res.shape, names);
            if !plan.cuts.is_empty() {
                let mut tools = Vec::new();
                for &(p, nrm, at_start) in &plan.cuts {
                    let ex = nrm.any_orthonormal_vector();
                    let ey = nrm.cross(ex);
                    let bx = oriented_box(p, ex, ey, [-big, big], [-big, big], [0.0, big])?;
                    let role = if at_start { Role::CapStart(Some(Source::Topo(s.name.clone()))) } else { Role::CapEnd(Some(Source::Topo(s.name.clone()))) };
                    tools.push(tool_named(bx, |_| face(feature, role.clone())));
                }
                let (sh, nm) = bool_named(ev, k::BooleanOp::Cut, &cur, &tools)?;
                if sh.count(ShapeType::Solid) == 0 {
                    return err("A frame segment was fully trimmed away");
                }
                cur = NamedShape::new(sh, nm);
            }
            built.push(cur);
        }
        // Coped butts: subtract the through member.
        for i in 0..n {
            for &(th, at_start) in &plans[i].copes.clone() {
                let tool_shape = built[th].shape.clone();
                let role = if at_start { Role::CapStart(Some(Source::Topo(path.segs[i].name.clone()))) } else { Role::CapEnd(Some(Source::Topo(path.segs[i].name.clone()))) };
                let tool = tool_named(tool_shape, |_| face(feature, role.clone()));
                let (sh, nm) = bool_named(ev, k::BooleanOp::Cut, &built[i], &[tool])?;
                built[i] = largest_piece(&sh, &nm).ok_or_else(|| fe("A coped frame segment was fully removed"))?;
            }
        }
        // Trim to parts.
        if !path.closed && !trim_bodies.is_empty() {
            for si in [0usize, n - 1] {
                let tools: Vec<NamedShape> = trim_bodies.iter().map(|s| tool_named(s.clone(), |_| face(feature, Role::CapEnd(Some(Source::Topo(path.segs[si].name.clone())))))).collect();
                let (sh, nm) = bool_named(ev, k::BooleanOp::Cut, &built[si], &tools)?;
                built[si] = largest_piece(&sh, &nm).ok_or_else(|| fe("A frame segment was fully trimmed away"))?;
            }
        }
        // Parts: one per segment, tangent runs merged if requested.
        let mut groups: Vec<Vec<usize>> = Vec::new();
        for i in 0..n {
            let join_prev = i > 0 && merge_tangent && tangent_join[i - 1];
            if join_prev {
                groups.last_mut().unwrap().push(i);
            } else {
                groups.push(vec![i]);
            }
        }
        if path.closed && merge_tangent && tangent_join[n - 1] && groups.len() > 1 {
            let first = groups.remove(0);
            groups.last_mut().unwrap().extend(first);
        }
        for g in groups {
            let named = if g.len() == 1 {
                built[g[0]].clone()
            } else {
                let tools: Vec<NamedShape> = g[1..].iter().map(|i| built[*i].clone()).collect();
                let (sh, nm) = bool_named(ev, k::BooleanOp::Fuse, &built[g[0]], &tools)?;
                NamedShape::new(sh, nm)
            };
            let id = BodyId { feature, index: (pi * 1000 + g[0]) as u32 };
            super::helpers_a::push_body(ev, id.clone(), BodyKind::Solid, named.shape, named.names.faces);
            let s0 = &path.segs[g[0]];
            let straight = g.len() == 1 && s0.line;
            let member = FrameMember {
                feature,
                kind: MemberKind::Segment,
                standard: profile.standard.clone(),
                description: profile.description.clone(),
                axis: straight.then_some((s0.start, s0.t0)),
                path_length: g.iter().map(|i| path.segs[*i].length).sum(),
            };
            ev.ctx.frame_members.insert(id, Arc::new(member));
        }
    }
    Ok(())
}

/// Largest solid of a result (with its face names).
fn largest_piece(shape: &Shape, names: &[Name]) -> Option<NamedShape> {
    let pieces = crate::regen::eval::split_solids(shape, names);
    pieces.into_iter().filter(|p| p.shape.count(ShapeType::Solid) > 0).max_by(|a, b| a.shape.volume().unwrap_or(0.0).total_cmp(&b.shape.volume().unwrap_or(0.0)))
}

/// Replace a body's shape keeping its id, name and color.
fn set_body(ev: &mut EvalCtx, bi: usize, p: NamedShape) {
    let old = ev.ctx.bodies[bi].clone();
    ev.ctx.bodies[bi] = Arc::new(Body::new(old.id.clone(), old.name.clone(), old.kind, p.shape, p.names.faces, old.color, ev.feature));
}

// ------------------------------------------------------------------ frame trim

pub fn frame_trim(ev: &mut EvalCtx) -> FResult<()> {
    let feature = ev.feature;
    if ev.is("trimType", "FACE") {
        let targets = ev.resolve_bodies(ev.query("targets"));
        if targets.is_empty() {
            return err("Select parts to trim");
        }
        let Some(r) = ev.query("tool").first() else { return err("Select a face or plane to trim to") };
        let Some(p) = ev.resolve_plane(r) else { return err("Only planar faces, planes and mate connectors can be used to trim") };
        if ev.bool("useToolExtents") {
            ev.warnings.push("Trim to face boundaries is not applied; the face's plane is used".into());
        }
        let removed = if ev.bool("flipTrim") { -p.normal } else { p.normal };
        let big = ev.model_extent();
        for bi in targets {
            let body = ev.ctx.bodies[bi].clone();
            let ex = removed.any_orthonormal_vector();
            let bx = oriented_box(p.origin, ex, removed.cross(ex), [-big, big], [-big, big], [0.0, big])?;
            let tool = tool_named(bx, |i| face(feature, Role::New(i)));
            let (sh, nm) = bool_named(ev, k::BooleanOp::Cut, &super::sheetmetal_util::body_named(&body), &[tool])?;
            let piece = largest_piece(&sh, &nm).ok_or_else(|| fe("A part was fully trimmed away"))?;
            set_body(ev, bi, piece);
        }
        return Ok(());
    }
    let groups: Vec<Vec<usize>> = ev.params.array("frameGroups").iter().map(|g| ev.resolve_bodies(g.query("frames"))).collect();
    if groups.iter().filter(|g| !g.is_empty()).count() < 2 {
        return err("Select at least two groups of frames to trim against each other");
    }
    let originals: Vec<Shape> = ev.ctx.bodies.iter().map(|b| b.shape.clone()).collect();
    for gi in 1..groups.len() {
        let tools_idx: Vec<usize> = groups[..gi].iter().flatten().copied().collect();
        for &bi in &groups[gi] {
            if tools_idx.contains(&bi) {
                continue;
            }
            let body = ev.ctx.bodies[bi].clone();
            let tools: Vec<NamedShape> = tools_idx
                .iter()
                .map(|&t| {
                    let tb = &ev.ctx.bodies[t];
                    let names: Vec<Name> = tb.faces.iter().map(|n| face(feature, Role::Generated(n.clone()))).collect();
                    NamedShape::new(originals[t].clone(), names)
                })
                .collect();
            let (sh, nm) = bool_named(ev, k::BooleanOp::Cut, &super::sheetmetal_util::body_named(&body), &tools)?;
            let piece = largest_piece(&sh, &nm).ok_or_else(|| fe(format!("{} was fully trimmed away", body.name)))?;
            if (piece.shape.volume().unwrap_or(0.0) - body.shape.volume().unwrap_or(0.0)).abs() < 1e-15 {
                ev.warnings.push(format!("{} does not touch the frames of earlier groups", body.name));
            }
            set_body(ev, bi, piece);
        }
    }
    Ok(())
}

// ------------------------------------------------------------------ gusset

fn is_cap(n: &TopoName) -> bool {
    let mut cur = n;
    loop {
        match cur {
            TopoName::Face { role: Role::CapStart(_) | Role::CapEnd(_), .. } => return true,
            TopoName::Split { parent, .. } => cur = parent,
            _ => return false,
        }
    }
}

pub fn gusset(ev: &mut EvalCtx) -> FResult<()> {
    let refs = ev.query("edges").to_vec();
    if refs.is_empty() {
        return err("Select edges at frame junctions for the gussets");
    }
    let len = ev.length("length")?;
    let th = ev.length("thickness")?;
    if len <= 0.0 || th <= 0.0 {
        return err("Gusset length and thickness must be positive");
    }
    let feature = ev.feature;
    let mut count = 0u32;
    for r in refs.iter().rev() {
        let Some(&(bi, ei)) = ev.resolve_edges(std::slice::from_ref(r)).first() else { return err("A selected edge no longer exists") };
        let body = ev.ctx.bodies[bi].clone();
        let edge = body.shape.sub_shape(ShapeType::Edge, ei)?;
        let Some((p, q)) = super::sheetmetal_util::line_of(&edge) else { return err("Gusset edges must be linear") };
        let e = (q - p).normalize();
        let m = (p + q) * 0.5;
        let ename = body.edges[ei].clone();
        // Candidate base faces: planar, containing the edge line, not caps.
        let mut cands: Vec<(usize, DVec3, DVec3)> = Vec::new();
        for (bj, b) in ev.ctx.bodies.iter().enumerate() {
            if b.kind != BodyKind::Solid {
                continue;
            }
            for (fi, f) in b.shape.faces().iter().enumerate() {
                if is_cap(&b.faces[fi]) {
                    continue;
                }
                let Some((c, n)) = plane_of(f) else { continue };
                if n.dot(e).abs() > 1e-6 || (p - c).dot(n).abs() > 1e-7 || (q - c).dot(n).abs() > 1e-7 {
                    continue;
                }
                // The face must touch the edge line.
                let Ok(d) = k::distance(f, &edge) else { continue };
                if d.distance > 1e-7 {
                    continue;
                }
                let mut dir = e.cross(n).normalize();
                if (c - m).dot(dir) < 0.0 {
                    dir = -dir;
                }
                if !cands.iter().any(|x| x.1.dot(n) > 1.0 - 1e-9) {
                    cands.push((bj, n, dir));
                }
            }
        }
        let mut pair = None;
        'search: for a in 0..cands.len() {
            for b in 0..cands.len() {
                let (na, da) = (cands[a].1, cands[a].2);
                let (nb, db) = (cands[b].1, cands[b].2);
                if a != b && na.cross(nb).length() > 1e-6 && na.dot(db) > 1e-6 && nb.dot(da) > 1e-6 {
                    pair = Some((da, db));
                    break 'search;
                }
            }
        }
        let Some((d1, d2)) = pair else { return err("Cannot fit a gusset at the selected edge (two adjacent frame faces are needed)") };
        let (d1, d2) = if ev.bool("shouldFlipBaseSides") { (d2, d1) } else { (d1, d2) };
        let pts: Vec<DVec3> = if ev.is("gussetType", "RECTANGLE") {
            let far = m + d1 * len + d2 * len;
            if ev.bool("chamfer") {
                let c = ev.length("chamferSize")?;
                if c >= len {
                    return err("The chamfer size exceeds the gusset size");
                }
                vec![m, m + d1 * len, far - d2 * c, far - d1 * c, m + d2 * len]
            } else {
                vec![m, m + d1 * len, far, m + d2 * len]
            }
        } else {
            vec![m, m + d1 * len, m + d2 * len]
        };
        // Position along the edge.
        let off = ev.length("offset").unwrap_or(0.0) * if ev.bool("shouldFlipOffset") { -1.0 } else { 1.0 };
        let mut s0 = -th * 0.5;
        if ev.is("gussetPosition", "ALIGNED")
            && let Some(ar) = ev.query("alignedReference").first()
            && let Some(&(ab, ae)) = ev.resolve_edges(std::slice::from_ref(ar)).first()
            && let Ok((a0, a1)) = ev.ctx.bodies[ab].shape.sub_shape(ShapeType::Edge, ae)?.endpoints()
        {
            let s_al = ((a0 + a1) * 0.5 - m).dot(e);
            s0 = if ev.bool("shouldFlipAlignment") { s_al - th } else { s_al };
        }
        let base: Vec<DVec3> = pts.iter().map(|x| *x + e * (s0 + off)).collect();
        let wire = k::make_polyline(&base, true)?;
        let fshape = k::make_face_from_wire(&wire, true)?;
        let n_edges = fshape.count(ShapeType::Edge);
        let frame = crate::display::PlaneFrame { origin: base[0], x: d1, y: e.cross(d1), normal: e };
        let prof = Profile { shape: fshape, frame, edge_sources: (0..n_edges).map(|i| piece_source(&ename, feature, i as u16)).collect(), key: Source::Topo(ename.clone()), from_face: false };
        let res = k::extrude(&prof.shape, e * th)?;
        let names = sweep_names(ev, &prof, &res, true, true);
        let id = BodyId { feature, index: count };
        count += 1;
        super::helpers_a::push_body(ev, id.clone(), BodyKind::Solid, res.shape, names);
        let desc = format!("Gusset {}", noshape_units::format_quantity(noshape_units::Quantity::meters(th), ev.units));
        ev.ctx.frame_members.insert(id, Arc::new(FrameMember { feature, kind: MemberKind::Gusset, standard: String::new(), description: desc, axis: None, path_length: 0.0 }));
    }
    Ok(())
}

// ------------------------------------------------------------------ end cap

pub fn end_cap(ev: &mut EvalCtx) -> FResult<()> {
    let refs = ev.query("faces").to_vec();
    if refs.is_empty() {
        return err("Select open end faces of frame segments");
    }
    let feature = ev.feature;
    let t = ev.length("thickness")?;
    let ptype = ev.enum_("profileType").to_string();
    for (count, r) in refs.iter().enumerate() {
        let Some(&(bi, fi)) = ev.resolve_faces(std::slice::from_ref(r)).first() else { return err("A selected face no longer exists") };
        let body = ev.ctx.bodies[bi].clone();
        let f = body.shape.sub_shape(ShapeType::Face, fi)?;
        let Some(pl) = f.face_plane()? else { return err("End cap faces must be planar") };
        let n = pl.unit_normal();
        let fname = body.faces[fi].clone();
        let frame_plane = k::Plane::new(f.centroid()?, n, pl.unit_x());
        let wires = f.wires();
        let outer = outer_wire(&f).ok_or_else(|| fe("The face has no boundary"))?;
        let (profile_face, start_shift, dir) = match ptype.as_str() {
            "INTERNAL" => {
                if wires.len() != 2 {
                    return err("Internal end caps need a hollow profile with exactly one inner boundary");
                }
                let inner = wires.into_iter().find(|w| !w.is_same(&outer)).ok_or_else(|| fe("The face has no inner boundary"))?;
                let mut fc = k::make_face_from_wire(&inner, true)?;
                let off = ev.length("internalOffsetDistance").unwrap_or(0.0);
                if off > 1e-12 {
                    let w = k::offset_wire_2d(&fc, -off, k::JoinType::Intersection, false)?;
                    fc = k::make_face_from_wire(&w, true)?;
                }
                let inset = ev.length("internalShiftDistance").unwrap_or(0.0);
                (fc, -n * inset, -n)
            }
            _ => {
                let mut fc = match ptype.as_str() {
                    "RECTANGLE" | "CIRCLE" => {
                        let mut lo = DVec2::splat(f64::INFINITY);
                        let mut hi = DVec2::splat(f64::NEG_INFINITY);
                        let mut samples: Vec<DVec3> = Vec::new();
                        for e in outer.edges() {
                            if let Ok(info) = e.edge_info() {
                                for i in 0..=16 {
                                    if let Ok(p) = e.point_at(info.first_param + (info.last_param - info.first_param) * i as f64 / 16.0) {
                                        samples.push(p);
                                    }
                                }
                            }
                        }
                        for p in &samples {
                            let q = frame_plane.to_local(*p);
                            lo = lo.min(q);
                            hi = hi.max(q);
                        }
                        if ptype == "RECTANGLE" {
                            k::make_plane_face(&frame_plane, lo.x, hi.x, lo.y, hi.y)?
                        } else {
                            let c = (lo + hi) * 0.5;
                            let rad = samples.iter().map(|p| (frame_plane.to_local(*p) - c).length()).fold(0.0, f64::max);
                            k::make_face_2d(&frame_plane, &[k::Segment2d::Circle { center: c, radius: rad }], &[])?
                        }
                    }
                    _ => k::make_face_from_wire(&outer, true)?,
                };
                let off = ev.length("offsetDistance").unwrap_or(0.0);
                if off > 1e-12 {
                    let d = if ev.bool("offsetDirection") { -off } else { off };
                    let w = k::offset_wire_2d(&fc, d, k::JoinType::Intersection, false)?;
                    fc = k::make_face_from_wire(&w, true)?;
                }
                let dir = if ev.bool("thicknessDirection") { -n } else { n };
                (fc, DVec3::ZERO, dir)
            }
        };
        let pf = if start_shift.length() > 0.0 { profile_face.translated(start_shift)? } else { profile_face };
        let n_edges = pf.count(ShapeType::Edge);
        let frame = crate::display::PlaneFrame { origin: frame_plane.origin, x: frame_plane.unit_x(), y: frame_plane.unit_y(), normal: n };
        let prof = Profile { shape: pf, frame, edge_sources: (0..n_edges).map(|i| piece_source(&fname, feature, i as u16)).collect(), key: Source::Topo(fname.clone()), from_face: false };
        let res = k::extrude(&prof.shape, dir * t)?;
        let mut names = sweep_names(ev, &prof, &res, true, true);
        let mut shape = res.shape;
        if ptype == "RECTANGLE" && !ev.is("cornersType", "NONE") {
            let lateral: Vec<Shape> = shape.edges().into_iter().filter(|e| super::sheetmetal_util::line_of(e).is_some_and(|(a, b)| (b - a).normalize().cross(dir).length() < 1e-6)).collect();
            let r2 = if ev.is("cornersType", "FILLET") {
                k::fillet(&shape, &lateral, ev.length("cornerRadius")?)
            } else {
                k::chamfer(&shape, &lateral, k::ChamferSpec::Equal(ev.length("cornerDistance")?), &[])
            }
            .map_err(|e| fe(format!("End cap corner treatment failed: {e}")))?;
            let sn = crate::regen::eval::names_for(&shape, names);
            names = ev.name_result(&[&sn], &r2.history, &r2.shape, &BTreeMap::new(), Some(&|n: &Name| Role::Blend(n.clone())));
            shape = r2.shape;
        }
        let id = BodyId { feature, index: count as u32 };
        super::helpers_a::push_body(ev, id.clone(), BodyKind::Solid, shape, names);
        let desc = format!("End cap {}", noshape_units::format_quantity(noshape_units::Quantity::meters(t), ev.units));
        ev.ctx.frame_members.insert(id, Arc::new(FrameMember { feature, kind: MemberKind::EndCap, standard: String::new(), description: desc, axis: None, path_length: 0.0 }));
    }
    Ok(())
}

// ------------------------------------------------------------------ cut list / tag

/// Cut list: validates the selection (the table is computed on demand by
/// [`cut_list`]); stores nothing.
pub fn cutlist(ev: &mut EvalCtx) -> FResult<()> {
    let ids: Option<Vec<BodyId>> = if ev.bool("selectAllFrames") { None } else { Some(ev.resolve_bodies(ev.query("frames")).into_iter().map(|i| ev.ctx.bodies[i].id.clone()).collect()) };
    if ids.as_ref().is_some_and(|v| v.is_empty()) {
        return err("Select frames for the cut list");
    }
    let rows = cut_list(&ev.ctx, ids.as_deref());
    if rows.is_empty() {
        return err("The selection contains no frame parts");
    }
    if let Some(ids) = &ids {
        let non_frame = ids.iter().filter(|id| !ev.ctx.frame_members.contains_key(id)).count();
        if non_frame > 0 {
            ev.warnings.push(format!("{non_frame} selected part(s) are not frame parts and are not listed"));
        }
    }
    Ok(())
}

/// Tag: metadata only (profile tags are read by name from the sketch). Form
/// tags record the form tool in the context for sheet metal Form.
pub fn tag(ev: &mut EvalCtx) -> FResult<()> {
    if !ev.is("tagPurpose", "FORM") {
        return Ok(());
    }
    let body = |ev: &EvalCtx, id: &str| ev.resolve_bodies(ev.query(id)).first().map(|&i| ev.ctx.bodies[i].id.clone());
    let add = body(ev, "partToAdd");
    let remove = body(ev, "partToRemove");
    if add.is_none() && remove.is_none() {
        return err("Select the part to add and/or the part to remove");
    }
    let sketch = ev.query("flatSketch").iter().find_map(|r| match r {
        Reference::Sketch(f) | Reference::Feature(f) => Some(*f),
        Reference::SketchEntity { sketch, .. } | Reference::SketchRegion { sketch, .. } => Some(*sketch),
        _ => None,
    });
    let origin = ev.query("formOrigin").first().and_then(|r| ev.resolve_connector(r)).map(|c| (c.origin, c.x, c.z));
    let feature = ev.feature;
    ev.ctx.form_tools.push(crate::sheetmetal::FormTool { feature, add, remove, sketch, origin });
    Ok(())
}
