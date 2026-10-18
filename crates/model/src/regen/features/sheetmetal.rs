//! Sheet metal: `sheetMetalStart` builds the sheet metal part as a solid of
//! the model thickness (Extrude: sketch lines/arcs with model-radius bends at
//! line-line corners; Thicken: faces / sketch regions; Convert: shell solids
//! and thicken surfaces) and records the sheet metal model (walls, bends,
//! parameters) in the context for later sheet metal features and the flat
//! pattern (`crate::sheetmetal`). Flange/Hem live in `sheetmetal_flange`,
//! joints, tab, bend, reliefs and corner breaks in `sheetmetal_ops`.

use crate::display::BodyKind;
use super::sheetmetal_util::{faces_named, model_from_params, put_data};
use crate::reference::{FeatureId, Role, Source, TopoName};
use crate::regen::eval::{EvalCtx, FResult, FeatureError, NamedShape, Profile, err, kernel_plane, sweep_names};
use crate::sheetmetal::{Bend, BendKind, SheetMetalData, Wall};
use glam::{DVec2, DVec3};
use std::sync::Arc;
use noshape_kernel as k;
use noshape_kernel::ShapeType;
use std::collections::BTreeMap;

pub fn sheet_metal_start(ev: &mut EvalCtx) -> FResult<()> {
    let t = ev.length("thickness")?;
    if t <= 0.0 {
        return err("Thickness must be positive");
    }
    match ev.enum_("process") {
        "EXTRUDE" => extrude(ev, t),
        "THICKEN" => thicken(ev, t),
        _ => convert(ev, t),
    }
}

/// One element of a 2D sheet path in sketch coordinates.
#[derive(Clone, Copy, Debug)]
enum Elem {
    Line { a: DVec2, b: DVec2 },
    /// Arc from `start_angle` sweeping `sweep` (signed, CCW positive).
    Arc { center: DVec2, radius: f64, start_angle: f64, sweep: f64 },
}

impl Elem {
    fn start(&self) -> DVec2 {
        match *self {
            Elem::Line { a, .. } => a,
            Elem::Arc { center, radius, start_angle, .. } => center + DVec2::from_angle(start_angle) * radius,
        }
    }
    fn end(&self) -> DVec2 {
        match *self {
            Elem::Line { b, .. } => b,
            Elem::Arc { center, radius, start_angle, sweep } => center + DVec2::from_angle(start_angle + sweep) * radius,
        }
    }
    fn reversed(&self) -> Elem {
        match *self {
            Elem::Line { a, b } => Elem::Line { a: b, b: a },
            Elem::Arc { center, radius, start_angle, sweep } => Elem::Arc { center, radius, start_angle: start_angle + sweep, sweep: -sweep },
        }
    }
    /// Unit tangent at the start / end (travel direction).
    fn tangent(&self, at_end: bool) -> DVec2 {
        match *self {
            Elem::Line { a, b } => (b - a).normalize_or_zero(),
            Elem::Arc { start_angle, sweep, .. } => {
                let ang = if at_end { start_angle + sweep } else { start_angle };
                DVec2::from_angle(ang).perp() * sweep.signum()
            }
        }
    }
    /// Offset by `d` to the left of the travel direction.
    fn offset_left(&self, d: f64) -> FResult<Elem> {
        Ok(match *self {
            Elem::Line { a, b } => {
                let n = (b - a).normalize_or_zero().perp() * d;
                Elem::Line { a: a + n, b: b + n }
            }
            Elem::Arc { center, radius, start_angle, sweep } => {
                let r = if sweep > 0.0 { radius - d } else { radius + d };
                if r <= 1e-9 {
                    return err("A bend or arc radius is smaller than the sheet thickness");
                }
                Elem::Arc { center, radius: r, start_angle, sweep }
            }
        })
    }
    fn segment(&self) -> k::Segment2d {
        match *self {
            Elem::Line { a, b } => k::Segment2d::Line { a, b },
            Elem::Arc { center, radius, start_angle, sweep } => {
                let at = |t: f64| center + DVec2::from_angle(t) * radius;
                k::Segment2d::Arc3 { a: at(start_angle), mid: at(start_angle + sweep * 0.5), b: at(start_angle + sweep) }
            }
        }
    }
    fn samples(&self) -> Vec<DVec2> {
        match *self {
            Elem::Line { a, .. } => vec![a],
            Elem::Arc { center, radius, start_angle, sweep } => (0..8).map(|i| center + DVec2::from_angle(start_angle + sweep * i as f64 / 8.0) * radius).collect(),
        }
    }
}

fn signed_area(pts: &[DVec2]) -> f64 {
    let n = pts.len();
    (0..n).map(|i| pts[i].perp_dot(pts[(i + 1) % n])).sum::<f64>() * 0.5
}

/// Order elements (with their sources) into one chain; returns (chain, closed).
fn chain(mut items: Vec<(Elem, u32)>) -> FResult<(Vec<(Elem, u32)>, bool)> {
    let tol = 1e-8;
    let n = items.len();
    // Start at an element end that touches no other element (open chain).
    let touches = |p: DVec2, items: &[(Elem, u32)], skip: usize| items.iter().enumerate().any(|(j, (e, _))| j != skip && ((e.start() - p).length() < tol || (e.end() - p).length() < tol));
    let mut start = None;
    for (i, (e, _)) in items.iter().enumerate() {
        if !touches(e.start(), &items, i) {
            start = Some((i, false));
            break;
        }
        if !touches(e.end(), &items, i) {
            start = Some((i, true));
            break;
        }
    }
    let closed = start.is_none();
    let (i0, rev) = start.unwrap_or((0, false));
    let (e0, s0) = items.remove(i0);
    let mut out = vec![(if rev { e0.reversed() } else { e0 }, s0)];
    while !items.is_empty() {
        let cur = out.last().map(|x| x.0.end()).unwrap_or_default();
        let Some(j) = items.iter().position(|(e, _)| (e.start() - cur).length() < tol || (e.end() - cur).length() < tol) else {
            return err("Sketch curves must form one connected chain");
        };
        let (e, s) = items.remove(j);
        out.push((if (e.start() - cur).length() < tol { e } else { e.reversed() }, s));
    }
    if out.len() != n {
        return err("Sketch curves must form one connected chain");
    }
    let closed = closed && (out[0].0.start() - out[out.len() - 1].0.end()).length() < tol;
    Ok((out, closed))
}

/// Path element source kinds for naming.
#[derive(Clone, Copy)]
enum Src {
    Entity(u32),
    Bend(u32),
}

fn extrude(ev: &mut EvalCtx, t: f64) -> FResult<()> {
    let refs = ev.query("sketchCurves");
    if refs.is_empty() {
        return err("Select sketch curves to extrude");
    }
    let mut sketch: Option<FeatureId> = None;
    let mut items = Vec::new();
    for r in refs {
        let crate::reference::Reference::SketchEntity { sketch: s, entity } = r else { continue };
        if sketch.is_some_and(|x| x != *s) {
            return err("Select curves from a single sketch");
        }
        sketch = Some(*s);
        let sk = ev.ctx.sketch(*s).ok_or_else(|| FeatureError::new("The sketch no longer exists"))?;
        let Some(curve) = sk.sketch.curve(noshape_sketch::EntityId(*entity)) else { return err("A selected sketch curve no longer exists") };
        use noshape_sketch::Curve2 as C;
        let e = match curve {
            C::Line { a, b } => Elem::Line { a, b },
            C::Arc { center, radius, start_angle, end_angle } => {
                let mut sweep = end_angle - start_angle;
                if sweep <= 0.0 {
                    sweep += std::f64::consts::TAU;
                }
                Elem::Arc { center, radius, start_angle, sweep }
            }
            _ => return err("Sheet metal extrude supports lines and arcs only"),
        };
        items.push((e, *entity));
    }
    let Some(sketch) = sketch else { return err("Select sketch curves to extrude") };
    let frame = ev.ctx.sketch(sketch).map(|s| s.frame).ok_or_else(|| FeatureError::new("The sketch no longer exists"))?;
    let (path, closed) = chain(items)?;
    let side = if ev.bool("oppositeDirection") { -1.0 } else { 1.0 };
    let radius = ev.length("radius")?;
    // Bends at line-line corners.
    let n = path.len();
    let mut elems: Vec<(Elem, Src)> = path.iter().map(|(e, s)| (*e, Src::Entity(*s))).collect();
    let corners: Vec<usize> = if closed { (0..n).collect() } else { (0..n.saturating_sub(1)).collect() };
    let mut bends: Vec<Option<Elem>> = vec![None; n];
    for &i in &corners {
        let j = (i + 1) % n;
        let (ti, tj) = (elems[i].0.tangent(true), elems[j].0.tangent(false));
        let cross = ti.perp_dot(tj);
        if cross.abs() < 1e-9 && ti.dot(tj) > 0.0 {
            continue; // tangent joint
        }
        let (Elem::Line { a: a0, b: p }, Elem::Line { b: b1, .. }) = (elems[i].0, elems[j].0) else {
            return err("Corners between arcs and other curves must be tangent");
        };
        let left_turn = cross > 0.0;
        // Material on the inside of the turn: the path is the outer bend face.
        let inside = (side > 0.0) == left_turn;
        let r = if inside { radius + t } else { radius };
        let theta = (-ti).dot(tj).clamp(-1.0, 1.0).acos();
        if theta < 1e-6 {
            return err("Sketch curves fold back on themselves");
        }
        let dist = r / (theta * 0.5).tan();
        let len_i = (p - a0).length();
        let len_j = (b1 - p).length();
        if dist > len_i + 1e-12 || dist > len_j + 1e-12 {
            return err("Bend radius is too large for the wall lengths");
        }
        let t0 = p - ti * dist;
        let t1 = p + tj * dist;
        let bis = (tj - ti).normalize();
        let center = p + bis * (r / (theta * 0.5).sin());
        let a_start = (t0 - center).to_angle();
        let a_end = (t1 - center).to_angle();
        let mut sweep = a_end - a_start;
        let sign = if left_turn { 1.0 } else { -1.0 };
        while sweep * sign <= 0.0 {
            sweep += sign * std::f64::consts::TAU;
        }
        while sweep.abs() > std::f64::consts::PI {
            sweep -= sign * std::f64::consts::TAU;
        }
        bends[i] = Some(Elem::Arc { center, radius: r, start_angle: a_start, sweep });
        if let Elem::Line { a, .. } = elems[i].0 {
            elems[i].0 = Elem::Line { a, b: t0 };
        }
        if let Elem::Line { b, .. } = elems[j].0 {
            elems[j].0 = Elem::Line { a: t1, b };
        }
    }
    // Path with bends inserted.
    let mut full: Vec<(Elem, Src)> = Vec::new();
    for (i, e) in elems.iter().enumerate() {
        full.push(*e);
        if let Some(b) = bends[i] {
            full.push((b, Src::Bend(i as u32)));
        }
    }
    let offset: Vec<(Elem, Src)> = full.iter().map(|(e, s)| e.offset_left(side * t).map(|o| (o, *s))).collect::<FResult<_>>()?;
    // Loops of (segment, source) with a per-segment naming source.
    let src_of = |s: Src, inner: bool| match s {
        Src::Entity(e) => Source::Sketch { sketch, entity: e, segment: inner as u16 },
        Src::Bend(i) => Source::Index(1000 + 1000 * inner as u32 + i),
    };
    let (outer, holes, sources): (Vec<k::Segment2d>, Vec<Vec<k::Segment2d>>, Vec<Source>) = if closed {
        let pa: Vec<DVec2> = full.iter().flat_map(|e| e.0.samples()).collect();
        let pb: Vec<DVec2> = offset.iter().flat_map(|e| e.0.samples()).collect();
        let a_outer = signed_area(&pa).abs() >= signed_area(&pb).abs();
        let (o, h, oi) = if a_outer { (&full, &offset, false) } else { (&offset, &full, true) };
        let mut src: Vec<Source> = o.iter().map(|e| src_of(e.1, oi)).collect();
        src.extend(h.iter().map(|e| src_of(e.1, !oi)));
        (o.iter().map(|e| e.0.segment()).collect(), vec![h.iter().map(|e| e.0.segment()).collect()], src)
    } else {
        let mut segs: Vec<k::Segment2d> = full.iter().map(|e| e.0.segment()).collect();
        let mut src: Vec<Source> = full.iter().map(|e| src_of(e.1, false)).collect();
        let end_a = full.last().map(|e| e.0.end()).unwrap_or_default();
        let end_b = offset.last().map(|e| e.0.end()).unwrap_or_default();
        segs.push(k::Segment2d::Line { a: end_a, b: end_b });
        src.push(Source::Index(3001));
        for e in offset.iter().rev() {
            segs.push(e.0.reversed().segment());
            src.push(src_of(e.1, true));
        }
        let st_b = offset.first().map(|e| e.0.start()).unwrap_or_default();
        let st_a = full.first().map(|e| e.0.start()).unwrap_or_default();
        segs.push(k::Segment2d::Line { a: st_b, b: st_a });
        src.push(Source::Index(3000));
        (segs, Vec::new(), src)
    };
    let tracked = k::make_face_2d_tracked(&kernel_plane(&frame), &outer, &holes)?;
    let n_edges = tracked.shape.count(ShapeType::Edge);
    let mut edge_sources: Vec<Source> = (0..n_edges as u32).map(Source::Index).collect();
    for (i, &e) in tracked.segment_edges.iter().enumerate() {
        if let (Some(slot), Some(s)) = (edge_sources.get_mut(e as usize), sources.get(i)) {
            *slot = s.clone();
        }
    }
    let profile = Profile { shape: tracked.shape, frame, edge_sources, key: Source::Index(9000), from_face: false };
    // Extrude (Blind, symmetric, second direction Blind).
    let dir = if ev.bool("oppositeExtrudeDirection") { -frame.normal } else { frame.normal };
    if !ev.is("endBound", "BLIND") {
        return err("Sheet metal extrude currently supports only the Blind end type");
    }
    let depth = ev.length("depth")?;
    let (a, b): (DVec3, DVec3) = if ev.bool("symmetric") {
        (-dir * depth * 0.5, dir * depth * 0.5)
    } else if ev.bool("hasSecondDirection") {
        if !ev.is("secondDirectionBound", "BLIND") {
            return err("Sheet metal extrude currently supports only the Blind end type");
        }
        let d2 = ev.length("secondDirectionDepth")?;
        let s2 = if ev.bool("secondDirectionOppositeExtrudeDirection") { -1.0 } else { 1.0 };
        (dir * d2 * s2, dir * depth)
    } else {
        (glam::DVec3::ZERO, dir * depth)
    };
    let res = if a.length() < 1e-12 { k::extrude(&profile.shape, b)? } else { k::extrude_between(&profile.shape, a, b, 0.0)? };
    let names = sweep_names(ev, &profile, &res, true, true);
    let before = ev.ctx.bodies.len();
    ev.apply_operation(vec![NamedShape::new(res.shape, names)], "NEW", None, BodyKind::Solid)?;
    if ev.ctx.bodies.len() <= before {
        return Ok(());
    }
    // Sheet metal model: one wall per line, one bend per arc.
    let mut data = model_from_params(ev, t)?;
    let bend_arcs: Vec<u32> = ev.query("bendArcs").iter().filter_map(|r| if let crate::reference::Reference::SketchEntity { entity, .. } = r { Some(*entity) } else { None }).collect();
    let feature = ev.feature;
    let name = |src: Source| Arc::new(TopoName::face(feature, Role::Side(src)));
    let mut last_wall: Option<usize> = None;
    let mut pending: Option<usize> = None;
    for (e, s) in &full {
        match *e {
            Elem::Line { .. } => {
                data.walls.push(Wall { feature, faces: vec![name(src_of(*s, false))], back: vec![name(src_of(*s, true))] });
                let w = data.walls.len() - 1;
                if let Some(b) = pending.take() {
                    data.bends[b].child = Some(w);
                }
                last_wall = Some(w);
            }
            Elem::Arc { center, radius: r, start_angle, sweep } => {
                let parent = match (last_wall, pending) {
                    (Some(w), None) => w,
                    _ => {
                        data.walls.push(Wall { feature, faces: Vec::new(), back: Vec::new() });
                        let w = data.walls.len() - 1;
                        if let Some(b) = pending.take() {
                            data.bends[b].child = Some(w);
                        }
                        w
                    }
                };
                let n = frame.normal;
                let axis = if sweep > 0.0 { n } else { -n };
                let path_outer = (sweep > 0.0) == (side > 0.0);
                let rin = if path_outer { r - t } else { r };
                let rad = DVec2::from_angle(start_angle);
                let tan = e.tangent(false);
                let (kind, kf) = match s {
                    Src::Entity(id) if !bend_arcs.contains(id) => (BendKind::Rolled, data.k_factor_rolled),
                    _ => (BendKind::Bend, data.k_factor),
                };
                let (pa, pb) = (a.dot(axis), b.dot(axis));
                let (path_name, off_name) = (name(src_of(*s, false)), name(src_of(*s, true)));
                let ref_inner = !path_outer;
                data.bends.push(Bend {
                    feature,
                    kind,
                    parent,
                    child: None,
                    origin: frame.to_world(center),
                    axis,
                    angle: sweep.abs(),
                    radius: rin,
                    k_factor: kf,
                    dir_a: -(frame.x * tan.x + frame.y * tan.y).normalize(),
                    radial_a: (frame.x * rad.x + frame.y * rad.y).normalize(),
                    s0: pa.min(pb),
                    s1: pa.max(pb),
                    ref_inner,
                    inner_faces: vec![if ref_inner { path_name.clone() } else { off_name.clone() }],
                    outer_faces: vec![if ref_inner { off_name } else { path_name }],
                    flat: false,
                });
                pending = Some(data.bends.len() - 1);
                last_wall = None;
            }
        }
    }
    if closed
        && let Some(b) = pending
        && !data.walls.is_empty()
    {
        data.bends[b].child = Some(0);
    }
    let bi = ev.ctx.bodies.len() - 1;
    put_data(ev, bi, data);
    Ok(())
}

fn thicken(ev: &mut EvalCtx, t: f64) -> FResult<()> {
    let refs = ev.query("regions");
    if refs.is_empty() {
        return err("Select faces or sketch regions to thicken");
    }
    let flip = ev.bool("oppositeDirection");
    let mut tools = Vec::new();
    let mut walls: Vec<Wall> = Vec::new();
    for r in refs {
        let one = std::slice::from_ref(r);
        let planar = match r {
            crate::reference::Reference::Face(_) => ev.resolve_faces(one).iter().all(|&(bi, fi)| ev.face_shape(bi, fi).ok().and_then(|f| f.face_plane().ok().flatten()).is_some()),
            _ => true,
        };
        if planar {
            for p in ev.profiles(one)? {
                let dir = if flip { -p.frame.normal } else { p.frame.normal };
                let res = k::extrude(&p.shape, dir * t)?;
                let names = sweep_names(ev, &p, &res, true, true);
                let cap = |end: bool| {
                    let role = if end { Role::CapEnd(Some(p.key.clone())) } else { Role::CapStart(Some(p.key.clone())) };
                    Arc::new(TopoName::face(ev.feature, role))
                };
                walls.push(Wall { feature: ev.feature, faces: vec![cap(false)], back: vec![cap(true)] });
                tools.push(NamedShape::new(res.shape, names));
            }
        } else {
            for (bi, fi) in ev.resolve_faces(one) {
                let face = ev.face_shape(bi, fi)?;
                let src = ev.ctx.bodies[bi].faces[fi].clone();
                let (t1, t2) = if flip { (0.0, t) } else { (t, 0.0) };
                let res = k::thicken(&face, t1, t2)?;
                let names = super::bshared::names_from_sources(ev, &res.history, &res.shape, 0, std::slice::from_ref(&src), |s| Role::Offset(s.clone()));
                tools.push(NamedShape::new(res.shape, names));
            }
        }
    }
    if tools.is_empty() {
        return err("Nothing to thicken");
    }
    let before = ev.ctx.bodies.len();
    ev.apply_operation(tools, "NEW", None, BodyKind::Solid)?;
    let model = model_from_params(ev, t)?;
    for bi in before..ev.ctx.bodies.len() {
        let body = ev.ctx.bodies[bi].clone();
        let mut data = model.clone();
        data.walls = walls.iter().filter(|w| !faces_named(&body, &w.faces).is_empty()).cloned().collect();
        put_data(ev, bi, data);
        bend_edges(ev, bi)?;
    }
    Ok(())
}

/// Walls of a converted body: each planar face paired with its offset.
fn convert_walls(body: &crate::regen::context::Body, feature: FeatureId) -> Vec<Wall> {
    let mut walls = Vec::new();
    for (fi, n) in body.faces.iter().enumerate() {
        let (inner, wrap) = match &**n {
            TopoName::Face { role: Role::Copy { of, .. }, .. } => (of.clone(), true),
            _ => (n.clone(), false),
        };
        let TopoName::Face { role: Role::Offset(src), .. } = &*inner else { continue };
        let src_name = if wrap { Arc::new(TopoName::face(feature, Role::Copy { instance: 0, of: src.clone() })) } else { src.clone() };
        let planar = |i: usize| body.shape.sub_shape(ShapeType::Face, i).ok().is_some_and(|f| f.is_planar());
        let refs = faces_named(body, std::slice::from_ref(&src_name));
        if !planar(fi) || refs.is_empty() || !refs.iter().all(|&i| planar(i)) {
            continue;
        }
        walls.push(Wall { feature, faces: vec![src_name], back: vec![n.clone()] });
    }
    walls
}

/// Convert / Thicken "Edges or cylinders to bend": sharp wall corners
/// selected by edge become bends of the model radius.
fn bend_edges(ev: &mut EvalCtx, bi: usize) -> FResult<()> {
    let refs = ev.query("bends").to_vec();
    if refs.is_empty() {
        return Ok(());
    }
    let mut data = (**ev.ctx.sheet_metal.get(&ev.ctx.bodies[bi].id).ok_or_else(|| FeatureError::new("Sheet metal model missing"))?).clone();
    let mut corners: Vec<(usize, usize)> = Vec::new();
    for r in &refs {
        if let crate::reference::Reference::Edge(n) = r {
            let body = &ev.ctx.bodies[bi];
            for ei in body.find_edges(n) {
                if let Some((a, b)) = super::sheetmetal_ops::walls_at_edge(body, &data, ei) {
                    let c = (a.min(b), a.max(b));
                    if !corners.contains(&c) {
                        corners.push(c);
                    }
                }
            }
        }
    }
    if corners.is_empty() {
        ev.warnings.push("Selected bend edges are not sharp corners between walls".into());
    }
    let (r, kf) = (data.radius, data.k_factor);
    for (a, b) in corners {
        super::sheetmetal_ops::bend_corner(ev, bi, &mut data, a, b, r, kf)?;
    }
    put_data(ev, bi, data);
    Ok(())
}

fn convert(ev: &mut EvalCtx, t: f64) -> FResult<()> {
    let idx = ev.resolve_bodies(ev.query("partToConvert"));
    if idx.is_empty() {
        return err("Select parts or surfaces to convert");
    }
    if ev.length("clearance").unwrap_or(0.0) > 0.0 {
        ev.warnings.push("Clearance from input is not applied yet".into());
    }
    let exclude: Vec<(usize, usize)> = ev.resolve_faces(ev.query("facesToExclude"));
    let keep = ev.bool("keepInputParts");
    let flip = ev.bool("oppositeDirection");
    for bi in idx.into_iter().rev() {
        let body = ev.ctx.bodies[bi].clone();
        let (shape, names) = match body.kind {
            BodyKind::Solid => {
                let faces: Vec<k::Shape> = exclude.iter().filter(|(b, _)| *b == bi).map(|&(b, f)| ev.face_shape(b, f)).collect::<FResult<_>>()?;
                let sidek = if flip { k::ShellSide::Outside } else { k::ShellSide::Inside };
                let res = k::shell(&body.shape, &faces, t, sidek, k::JoinType::Arc)?;
                let names = ev.name_result(&[&body.shape_names()], &res.history, &res.shape, &BTreeMap::new(), Some(&|n| Role::Offset(n.clone())));
                (res.shape, names)
            }
            BodyKind::Surface => {
                let (t1, t2) = if flip { (0.0, t) } else { (t, 0.0) };
                let res = k::thicken(&body.shape, t1, t2)?;
                let names = ev.name_result(&[&body.shape_names()], &res.history, &res.shape, &BTreeMap::new(), Some(&|n| Role::Offset(n.clone())));
                (res.shape, names)
            }
            _ => return err("Only parts and surfaces can be converted to sheet metal"),
        };
        let model = model_from_params(ev, t)?;
        if keep {
            // New part next to the kept input.
            let names = names.iter().map(|n| std::sync::Arc::new(crate::reference::TopoName::face(ev.feature, Role::Copy { instance: 0, of: n.clone() }))).collect();
            ev.apply_operation(vec![NamedShape::new(shape, names)], "NEW", None, BodyKind::Solid)?;
            let nbi = ev.ctx.bodies.len() - 1;
            let mut data = model;
            data.walls = convert_walls(&ev.ctx.bodies[nbi], ev.feature);
            put_data(ev, nbi, data);
            bend_edges(ev, nbi)?;
        } else {
            ev.replace_body(bi, shape, names);
            if body.kind == BodyKind::Surface {
                let b = ev.ctx.bodies[bi].clone();
                let new = crate::regen::context::Body::new(b.id.clone(), b.name.clone(), BodyKind::Solid, b.shape.clone(), b.faces.clone(), b.color, ev.feature);
                ev.ctx.bodies[bi] = std::sync::Arc::new(new);
            }
            let mut data = model;
            data.walls = convert_walls(&ev.ctx.bodies[bi], ev.feature);
            put_data(ev, bi, data);
            bend_edges(ev, bi)?;
        }
    }
    Ok(())
}

/// Finish sheet metal model: the selected (or all) models become inactive;
/// the parts stay as they are and keep their (frozen) flat pattern data.
pub fn sheet_metal_end(ev: &mut EvalCtx) -> FResult<()> {
    let refs = ev.query("sheetMetalParts");
    let ids: Vec<crate::reference::BodyId> = if refs.is_empty() {
        ev.ctx.sheet_metal.iter().filter(|(_, d)| d.active).map(|(id, _)| id.clone()).collect()
    } else {
        let idx = ev.resolve_bodies(refs);
        if idx.is_empty() {
            return err("Select sheet metal parts to finish");
        }
        idx.into_iter().map(|i| ev.ctx.bodies[i].id.clone()).collect()
    };
    let mut any = false;
    for id in ids {
        if let Some(d) = ev.ctx.sheet_metal.get_mut(&id)
            && d.active
        {
            Arc::make_mut(d).active = false;
            any = true;
        }
    }
    if !any && !refs.is_empty() {
        return err("The selected parts are not active sheet metal models");
    }
    Ok(())
}

pub fn flange(ev: &mut EvalCtx) -> FResult<()> {
    super::sheetmetal_flange::flange(ev)
}
pub fn hem(ev: &mut EvalCtx) -> FResult<()> {
    super::sheetmetal_flange::hem(ev)
}
pub fn tab(ev: &mut EvalCtx) -> FResult<()> {
    super::sheetmetal_ops::tab(ev)
}
pub fn bend(ev: &mut EvalCtx) -> FResult<()> {
    super::sheetmetal_ops::bend(ev)
}
pub fn make_joint(ev: &mut EvalCtx) -> FResult<()> {
    super::sheetmetal_ops::make_joint(ev)
}
pub fn modify_joint(ev: &mut EvalCtx) -> FResult<()> {
    super::sheetmetal_ops::modify_joint(ev)
}
pub fn bend_relief(ev: &mut EvalCtx) -> FResult<()> {
    super::sheetmetal_ops::bend_relief(ev)
}
pub fn corner_break(ev: &mut EvalCtx) -> FResult<()> {
    super::sheetmetal_ops::corner_break(ev)
}

/// Data of the sheet metal model a feature created (for callers that hold
/// only the context).
pub fn model_of(ctx: &crate::regen::context::Context, id: &crate::reference::BodyId) -> Option<Arc<SheetMetalData>> {
    ctx.sheet_metal.get(id).cloned()
}

pub fn jog(ev: &mut EvalCtx) -> FResult<()> {
    super::sheetmetal_jog::jog(ev)
}
pub fn form(ev: &mut EvalCtx) -> FResult<()> {
    super::sheetmetal_form::form(ev)
}
pub fn corner(ev: &mut EvalCtx) -> FResult<()> {
    super::sheetmetal_form::corner(ev)
}
pub fn loft(ev: &mut EvalCtx) -> FResult<()> {
    super::sheetmetal_loft::loft(ev)
}
pub fn rip(ev: &mut EvalCtx) -> FResult<()> {
    super::sheetmetal_unfold::rip(ev)
}
pub fn recognize(ev: &mut EvalCtx) -> FResult<()> {
    super::sheetmetal_unfold::recognize(ev)
}
pub fn unfold(ev: &mut EvalCtx) -> FResult<()> {
    super::sheetmetal_unfold::unfold(ev)
}
pub fn refold(ev: &mut EvalCtx) -> FResult<()> {
    super::sheetmetal_unfold::refold(ev)
}
