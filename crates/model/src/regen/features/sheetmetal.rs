//! Sheet metal: `sheetMetalStart` builds the sheet metal part as a solid of
//! the model thickness (Extrude: sketch lines/arcs with model-radius bends at
//! line-line corners; Thicken: faces / sketch regions; Convert: shell solids
//! and thicken surfaces). The flat pattern, bend table, reliefs and the other
//! sheet metal features are not implemented yet and report clear errors.

use crate::display::BodyKind;
use crate::reference::{FeatureId, Role, Source};
use crate::regen::eval::{EvalCtx, FResult, FeatureError, NamedShape, Profile, err, kernel_plane, sweep_names};
use glam::DVec2;
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
    let (a, b) = if ev.bool("symmetric") {
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
    ev.apply_operation(vec![NamedShape::new(res.shape, names)], "NEW", None, BodyKind::Solid)
}

fn thicken(ev: &mut EvalCtx, t: f64) -> FResult<()> {
    let refs = ev.query("regions");
    if refs.is_empty() {
        return err("Select faces or sketch regions to thicken");
    }
    if !ev.query("bends").is_empty() {
        ev.warnings.push("Selected bends are kept as sharp corners".into());
    }
    let flip = ev.bool("oppositeDirection");
    let mut tools = Vec::new();
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
    ev.apply_operation(tools, "NEW", None, BodyKind::Solid)
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
        if keep {
            // New part next to the kept input.
            let names = names.iter().map(|n| std::sync::Arc::new(crate::reference::TopoName::face(ev.feature, Role::Copy { instance: 0, of: n.clone() }))).collect();
            ev.apply_operation(vec![NamedShape::new(shape, names)], "NEW", None, BodyKind::Solid)?;
        } else {
            ev.replace_body(bi, shape, names);
            if body.kind == BodyKind::Surface {
                let b = ev.ctx.bodies[bi].clone();
                let new = crate::regen::context::Body::new(b.id.clone(), b.name.clone(), BodyKind::Solid, b.shape.clone(), b.faces.clone(), b.color, ev.feature);
                ev.ctx.bodies[bi] = std::sync::Arc::new(new);
            }
        }
    }
    Ok(())
}

/// Finish sheet metal model: the model is a plain solid already; nothing to do.
pub fn sheet_metal_end(_ev: &mut EvalCtx) -> FResult<()> {
    Ok(())
}

macro_rules! unsupported {
    ($($f:ident => $name:literal),* $(,)?) => {
        $(
            pub fn $f(_ev: &mut EvalCtx) -> FResult<()> {
                err(concat!($name, " is not supported yet (sheet metal models are built as plain solids; only Sheet metal model is available)"))
            }
        )*
    };
}

// Sheet metal features that need the sheet metal model (flat pattern, bend
// table, joints), which is not implemented yet.
unsupported! {
    flange => "Flange",
    hem => "Hem",
    tab => "Tab",
    bend => "Bend",
    jog => "Jog",
    form => "Form",
    loft => "Sheet metal loft",
    make_joint => "Make joint",
    corner => "Corner",
    bend_relief => "Bend relief",
    corner_break => "Corner break",
    modify_joint => "Modify joint",
    rip => "Rip",
    recognize => "Recognize",
}
