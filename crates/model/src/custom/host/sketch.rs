//! Sketch natives: `@newSketch`, `@sk*` entities and constraints, `@skSolve`.
//!
//! A FeatureScript sketch builds a regular [`noshape_sketch::Sketch`]; its
//! string entity ids (`"line1"`, `"line1.start"`) map to entity ids so
//! constraints and `sketchEntityQuery` find them. `skSolve` solves it and
//! adds a [`SketchResult`] (feature = the sketch id's marker) to the
//! context, exactly like a Sketch feature.

use super::{CtxState, FsContext, FsId, context_arg, convert::*, raise_error, std_tag};
use crate::display::PlaneFrame;
use crate::reference::FeatureId;
use crate::regen::context::SketchResult;
use glam::{DVec2, DVec3};
use noshape_featurescript::interp::{Interp, NativeFn, R};
use noshape_featurescript::value::{Builtin, Value, next_serial};
use noshape_sketch::{ConstraintKind, EntityId, EntityKind, Sketch};
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

pub const NATIVES: &[(&str, NativeFn)] = &[
    ("newSketch", new_sketch),
    ("isSketch", is_sketch),
    ("skSolve", sk_solve),
    ("skSetInitialGuess", |_, _| Ok(Value::Undefined)),
    ("skPoint", sk_point),
    ("skLineSegment", sk_line_segment),
    ("skCircle", sk_circle),
    ("skArc", sk_arc),
    ("skEllipse", sk_ellipse),
    ("skConicSegment", sk_conic_segment),
    ("skText", sk_text),
    ("skImage", sk_image),
    ("skEllipticalArc", sk_elliptical_arc),
    ("skFitSpline", sk_fit_spline),
    ("skInterpolatedSpline", sk_fit_spline),
    ("skInterpolatedSplineSegment", sk_fit_spline),
    ("skSpline", sk_bspline),
    ("skSplineSegment", sk_bspline),
    ("skBezier", sk_bspline),
    ("skConstraint", sk_constraint),
    ("alignCanonically", align_canonically),
    ("getFullPatternTransform", identity_transform),
    ("getRemainderPatternTransform", identity_transform),
];

/// A sketch in progress.
pub struct SketchBuild {
    pub frame: PlaneFrame,
    pub sketch: Sketch,
    pub marker: FeatureId,
    pub solved: bool,
    /// skImage pictures (drawn in their solved boxes).
    pub images: Vec<crate::regen::context::SketchResultImage>,
}

/// The `Sketch` builtin.
pub struct FsSketch {
    serial: u64,
    cx: Arc<FsContext>,
    id: FsId,
}

impl std::fmt::Debug for FsSketch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Sketch({})", self.id.join("."))
    }
}

impl Builtin for FsSketch {
    fn type_name(&self) -> &'static str {
        "Sketch"
    }
    fn serial(&self) -> u64 {
        self.serial
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// String entity ids of FeatureScript sketches, by sketch marker.
pub type SketchNames = HashMap<FeatureId, HashMap<String, u32>>;

fn names(st: &mut CtxState) -> &mut SketchNames {
    &mut st.sketch_names
}

/// Onshape's canonical plane axes: x horizontal (perpendicular to world Z),
/// so y points up; planes facing ±Z keep x = +X.
pub fn canonical_frame(origin: DVec3, normal: DVec3) -> PlaneFrame {
    let n = normal.normalize_or_zero();
    let mut x = DVec3::Z.cross(n);
    if x.length_squared() < 1e-18 {
        x = DVec3::X;
    }
    let x = x.normalize();
    PlaneFrame { origin, x, y: n.cross(x), normal: n }
}

fn align_canonically(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let Some(p) = a.get(1).and_then(plane_of) else { return Err(i.error("alignCanonically: argument 2 should be a Plane")) };
    Ok(plane_value(&canonical_frame(p.origin, p.normal)))
}

fn identity_transform(_: &mut Interp, _: Vec<Value>) -> R<Value> {
    let row = |r: usize| Value::array((0..3).map(|c| Value::num(if r == c { 1.0 } else { 0.0 })).collect());
    Ok(Value::map_from([("linear", Value::array(vec![row(0), row(1), row(2)])), ("translation", Value::array(vec![Value::num(0.0); 3]))]))
}

fn new_sketch(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let cx = context_arg(i, &a)?;
    let id = id_arg(i, &a, 1)?;
    let def = def_arg(i, &a, 2)?;
    let Some(mut frame) = plane_of(def.get("sketchPlane")) else { return Err(raise_error(i, "SKETCH_NO_PLANE", None)) };
    if let Some(m) = transform_of(def.get("transform")) {
        frame = crate::regen::eval::transform_frame(&frame, &m);
    }
    let marker = {
        let mut st = cx.state.lock();
        let Some(m) = st.begin_op(&id) else {
            drop(st);
            return Err(raise_error(i, "DUPLICATE_ID", None));
        };
        st.sketches.insert(id.clone(), SketchBuild { frame, sketch: Sketch::new(), marker: m, solved: false, images: Vec::new() });
        names(&mut st).insert(m, HashMap::new());
        m
    };
    let _ = marker;
    let sk = Arc::new(FsSketch { serial: next_serial(), cx, id });
    Ok(Value::Builtin(sk, std_tag("sketch.fs", "Sketch")))
}

fn is_sketch(_: &mut Interp, a: Vec<Value>) -> R<Value> {
    Ok(Value::bool(a.first().and_then(|v| v.builtin::<FsSketch>()).is_some()))
}

fn sketch_arg(i: &mut Interp, a: &[Value]) -> R<(Arc<FsContext>, FsId)> {
    match a.first().and_then(|v| v.builtin::<FsSketch>()) {
        Some(s) => Ok((s.cx.clone(), s.id.clone())),
        None => Err(i.error("Argument 1 should be a Sketch")),
    }
}

/// Run `f` on the sketch being built, then record new string ids.
fn with_sketch<T>(i: &mut Interp, a: &[Value], f: impl FnOnce(&mut Interp, &mut SketchBuild, &mut HashMap<String, u32>) -> R<T>) -> R<T> {
    let (cx, id) = sketch_arg(i, a)?;
    let mut st = cx.state.lock();
    let Some(mut build) = st.sketches.remove(&id) else {
        drop(st);
        return Err(raise_error(i, "SKETCH_NOT_ACTIVE", None));
    };
    let mut map = names(&mut st).remove(&build.marker).unwrap_or_default();
    drop(st);
    let r = f(i, &mut build, &mut map);
    let mut st = cx.state.lock();
    names(&mut st).insert(build.marker, map);
    st.sketches.insert(id, build);
    r
}

fn p2(v: &Value) -> Option<DVec2> {
    vec2_of(v)
}

fn need_p2(i: &mut Interp, def: &Value, key: &str) -> R<DVec2> {
    match p2(def.get(key)) {
        Some(p) => Ok(p),
        None => Err(i.error(format!("{key} should be a 2D point"))),
    }
}

fn entity_id(i: &mut Interp, a: &[Value]) -> R<String> {
    match a.get(1).and_then(|v| v.as_str()) {
        Some(s) => Ok(s.to_string()),
        None => Err(i.error("Sketch entity id should be a string")),
    }
}

fn check_new(i: &mut Interp, map: &HashMap<String, u32>, id: &str) -> R<()> {
    if map.contains_key(id) {
        return Err(i.error(format!("Sketch entity id \"{id}\" is already used")));
    }
    Ok(())
}

/// Register a curve's id and its point ids (`.start`, `.end`, `.center`).
fn register(sk: &Sketch, map: &mut HashMap<String, u32>, id: &str, e: EntityId) {
    map.insert(id.to_string(), e.0);
    if let Some(ent) = sk.entity(e) {
        if let Some((s, t)) = ent.endpoints() {
            map.insert(format!("{id}.start"), s.0);
            map.insert(format!("{id}.end"), t.0);
        }
        if let Some(c) = ent.center() {
            map.insert(format!("{id}.center"), c.0);
        }
    }
}

fn construction(def: &Value) -> bool {
    def.get("construction").as_bool().unwrap_or(false)
}

fn sk_point(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let id = entity_id(i, &a)?;
    let def = def_arg(i, &a, 2)?;
    with_sketch(i, &a, |i, b, map| {
        check_new(i, map, &id)?;
        let p = need_p2(i, &def, "position")?;
        let e = b.sketch.add_point(p);
        if construction(&def) {
            let _ = b.sketch.set_construction(e, true);
        }
        map.insert(id.clone(), e.0);
        Ok(Value::Undefined)
    })
}

fn sk_line_segment(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let id = entity_id(i, &a)?;
    let def = def_arg(i, &a, 2)?;
    with_sketch(i, &a, |i, b, map| {
        check_new(i, map, &id)?;
        let (s, e) = (need_p2(i, &def, "start")?, need_p2(i, &def, "end")?);
        let l = b.sketch.add_line(s, e);
        if construction(&def) {
            let _ = b.sketch.set_construction(l, true);
        }
        register(&b.sketch, map, &id, l);
        Ok(Value::Undefined)
    })
}

fn sk_circle(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let id = entity_id(i, &a)?;
    let def = def_arg(i, &a, 2)?;
    with_sketch(i, &a, |i, b, map| {
        check_new(i, map, &id)?;
        let c = need_p2(i, &def, "center")?;
        let Some(r) = magnitude_of(def.get("radius")) else { return Err(i.error("radius should be a length")) };
        let e = b.sketch.add_circle(c, r);
        if construction(&def) {
            let _ = b.sketch.set_construction(e, true);
        }
        register(&b.sketch, map, &id, e);
        Ok(Value::Undefined)
    })
}

/// Center of the circle through three points.
fn circumcenter(a: DVec2, b: DVec2, c: DVec2) -> Option<DVec2> {
    let d = 2.0 * (a.x * (b.y - c.y) + b.x * (c.y - a.y) + c.x * (a.y - b.y));
    if d.abs() < 1e-18 {
        return None;
    }
    let (a2, b2, c2) = (a.length_squared(), b.length_squared(), c.length_squared());
    Some(DVec2::new((a2 * (b.y - c.y) + b2 * (c.y - a.y) + c2 * (a.y - b.y)) / d, (a2 * (c.x - b.x) + b2 * (a.x - c.x) + c2 * (b.x - a.x)) / d))
}

fn sk_arc(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let id = entity_id(i, &a)?;
    let def = def_arg(i, &a, 2)?;
    with_sketch(i, &a, |i, b, map| {
        check_new(i, map, &id)?;
        let (s, m, e) = (need_p2(i, &def, "start")?, need_p2(i, &def, "mid")?, need_p2(i, &def, "end")?);
        let Some(c) = circumcenter(s, m, e) else { return Err(raise_error(i, "SKETCH_ARC_FAILED", None)) };
        // Our arcs run CCW from start to end; a clockwise 3-point arc swaps them.
        let ccw = (m - s).perp_dot(e - m) > 0.0;
        let arc = if ccw { b.sketch.add_arc(c, s, e) } else { b.sketch.add_arc(c, e, s) };
        if construction(&def) {
            let _ = b.sketch.set_construction(arc, true);
        }
        register(&b.sketch, map, &id, arc);
        if !ccw {
            // Keep Onshape's start/end naming.
            let (st, en) = (map.get(&format!("{id}.start")).copied(), map.get(&format!("{id}.end")).copied());
            if let (Some(st), Some(en)) = (st, en) {
                map.insert(format!("{id}.start"), en);
                map.insert(format!("{id}.end"), st);
            }
        }
        Ok(Value::Undefined)
    })
}

/// Font family and style for an Onshape font file name
/// ("OpenSans-BoldItalic.ttf"): a Text tool font when it is one, else serif /
/// mono families map to Liberation Serif / Mono, everything else to
/// Liberation Sans.
fn font_of(name: &str) -> (&'static str, bool, bool) {
    let n = name.to_ascii_lowercase();
    // A Text tool font by its file name ("OpenSans-Bold.ttf"), else the
    // closest Liberation family.
    let family = if let Some(f) = noshape_sketch::text::sketch_font_of_file(name) {
        f
    } else if ["serif", "slab", "tinos", "times", "georgia"].iter().any(|s| n.contains(s)) && !n.contains("sans") {
        "Liberation Serif"
    } else if ["mono", "cousine", "courier", "code"].iter().any(|s| n.contains(s)) {
        "Liberation Mono"
    } else {
        noshape_sketch::text::DEFAULT_FONT
    };
    (family, n.contains("bold"), n.contains("italic") || n.contains("oblique"))
}

/// Text outlines in the rectangle between `firstCorner` and `secondCorner`:
/// the first line fills its height from its lower-left corner.
fn sk_text(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let id = entity_id(i, &a)?;
    let def = def_arg(i, &a, 2)?;
    let text = def.get("text").as_str().map(|s| s.to_string()).unwrap_or_default();
    let (family, bold, italic) = font_of(&def.get("fontName").as_str().unwrap_or_default());
    with_sketch(i, &a, |i, b, map| {
        check_new(i, map, &id)?;
        let c0 = p2(def.get("firstCorner")).unwrap_or(DVec2::ZERO);
        let c1 = p2(def.get("secondCorner")).unwrap_or(c0 + DVec2::splat(0.0254));
        let (lo, hi) = (c0.min(c1), c0.max(c1));
        let place = noshape_sketch::text::TextPlacement {
            origin: lo,
            x: DVec2::X,
            y: DVec2::Y,
            height: (hi.y - lo.y).max(1e-6),
            mirror_horizontal: def.get("mirrorHorizontal").as_bool().unwrap_or(false),
            mirror_vertical: def.get("mirrorVertical").as_bool().unwrap_or(false),
        };
        let layout = noshape_sketch::text::layout_text(&text, family, bold, italic);
        let curves = b.sketch.add_text_outlines(&layout, &place, None);
        for (k, c) in curves.iter().enumerate() {
            if construction(&def) {
                let _ = b.sketch.set_construction(*c, true);
            }
            register(&b.sketch, map, &format!("{id}.{k}"), *c);
        }
        Ok(Value::map_from([("textId", Value::str(id.as_str()))]))
    })
}

/// A conic from start, control point, end and rho (rational quadratic
/// Bézier). `lowParameter` / `highParameter` trims are not applied yet.
fn sk_conic_segment(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let id = entity_id(i, &a)?;
    let def = def_arg(i, &a, 2)?;
    with_sketch(i, &a, |i, b, map| {
        check_new(i, map, &id)?;
        let (s, c, e) = (need_p2(i, &def, "start")?, need_p2(i, &def, "controlPoint")?, need_p2(i, &def, "end")?);
        let rho = def.get("rho").as_number().unwrap_or(0.5);
        // `lowParameter` / `highParameter`: only that part of the conic.
        let (lo, hi) = (def.get("lowParameter").as_number().unwrap_or(0.0), def.get("highParameter").as_number().unwrap_or(1.0));
        let ((s, c, e), rho) = if (lo, hi) == (0.0, 1.0) { ((s, c, e), rho) } else { conic_segment(s, c, e, rho, lo, hi) };
        let Ok(conic) = b.sketch.add_conic(s, c, e, rho) else { return Err(raise_error(i, "REGEN_ERROR", None)) };
        if construction(&def) {
            let _ = b.sketch.set_construction(conic, true);
        }
        register(&b.sketch, map, &id, conic);
        Ok(Value::Undefined)
    })
}

/// The part `lo..hi` of a conic (start, control point, end, rho) as a conic
/// of its own: the rational quadratic Bézier's blossom in homogeneous
/// coordinates, the weight renormalized (w = rho / (1 - rho)).
fn conic_segment(s: DVec2, c: DVec2, e: DVec2, rho: f64, lo: f64, hi: f64) -> ((DVec2, DVec2, DVec2), f64) {
    let w = rho / (1.0 - rho).max(1e-12);
    let pts = [(s, 1.0), (c * w, w), (e, 1.0)];
    let blossom = |u: f64, v: f64| {
        let k = [(1.0 - u) * (1.0 - v), (1.0 - u) * v + u * (1.0 - v), u * v];
        let p = pts[0].0 * k[0] + pts[1].0 * k[1] + pts[2].0 * k[2];
        let wt = pts[0].1 * k[0] + pts[1].1 * k[1] + pts[2].1 * k[2];
        (p, wt)
    };
    let (q0, q1, q2) = (blossom(lo, lo), blossom(lo, hi), blossom(hi, hi));
    let w1 = q1.1 / (q0.1 * q2.1).sqrt();
    ((q0.0 / q0.1, q1.0 / q1.1, q2.0 / q2.1), w1 / (1.0 + w1))
}

fn sk_ellipse(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let id = entity_id(i, &a)?;
    let def = def_arg(i, &a, 2)?;
    with_sketch(i, &a, |i, b, map| {
        check_new(i, map, &id)?;
        let c = need_p2(i, &def, "center")?;
        let (Some(maj), Some(min)) = (magnitude_of(def.get("majorRadius")), magnitude_of(def.get("minorRadius"))) else { return Err(i.error("majorRadius and minorRadius should be lengths")) };
        let axis = p2(def.get("majorAxis")).unwrap_or(DVec2::X).normalize_or_zero();
        let e = b.sketch.add_ellipse(c, axis * maj, min);
        if construction(&def) {
            let _ = b.sketch.set_construction(e, true);
        }
        register(&b.sketch, map, &id, e);
        Ok(Value::Undefined)
    })
}

fn sk_elliptical_arc(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let id = entity_id(i, &a)?;
    let def = def_arg(i, &a, 2)?;
    with_sketch(i, &a, |i, b, map| {
        check_new(i, map, &id)?;
        let c = need_p2(i, &def, "center")?;
        let (Some(maj), Some(min)) = (magnitude_of(def.get("majorRadius")), magnitude_of(def.get("minorRadius"))) else { return Err(i.error("majorRadius and minorRadius should be lengths")) };
        let axis = p2(def.get("majorAxis")).unwrap_or(DVec2::X).normalize_or_zero();
        let (Some(s), Some(e)) = (def.get("startParameter").as_number(), def.get("endParameter").as_number()) else { return Err(i.error("startParameter and endParameter should be numbers")) };
        let tau = std::f64::consts::TAU;
        let x = b.sketch.add_elliptical_arc(c, axis * maj, min, s * tau, e * tau);
        register(&b.sketch, map, &id, x);
        Ok(Value::Undefined)
    })
}

fn sk_fit_spline(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let id = entity_id(i, &a)?;
    let def = def_arg(i, &a, 2)?;
    with_sketch(i, &a, |i, b, map| {
        check_new(i, map, &id)?;
        let pts: Vec<DVec2> = def.get("points").as_array().map(|v| v.iter().filter_map(p2).collect()).unwrap_or_default();
        if pts.len() < 2 {
            return Err(raise_error(i, "SKETCH_SPLINE_FAILED", None));
        }
        // The same first and last point: a closed (periodic) spline.
        let closed = pts.len() >= 4 && (pts[0] - pts[pts.len() - 1]).length() <= 1e-12 * (1.0 + pts[0].length());
        let e = if closed { b.sketch.add_closed_fit_spline(&pts) } else { b.sketch.add_fit_spline(&pts) }.map_err(|_| raise_error(i, "SKETCH_SPLINE_FAILED", None))?;
        if construction(&def) {
            let _ = b.sketch.set_construction(e, true);
        }
        register(&b.sketch, map, &id, e);
        Ok(Value::Undefined)
    })
}

fn sk_bspline(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let id = entity_id(i, &a)?;
    let def = def_arg(i, &a, 2)?;
    with_sketch(i, &a, |i, b, map| {
        check_new(i, map, &id)?;
        let pts: Vec<DVec2> = def.get("controlPoints").as_array().or(def.get("points").as_array()).map(|v| v.iter().filter_map(p2).collect()).unwrap_or_default();
        let degree = def.get("degree").as_number().unwrap_or(3.0) as usize;
        let e = b.sketch.add_bspline(&pts, degree.min(pts.len().saturating_sub(1)).max(1)).map_err(|_| raise_error(i, "SKETCH_SPLINE_FAILED", None))?;
        register(&b.sketch, map, &id, e);
        Ok(Value::Undefined)
    })
}

fn sk_constraint(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let def = def_arg(i, &a, 2)?;
    with_sketch(i, &a, |i, b, map| {
        let ty = def.get("constraintType").as_str().unwrap_or_default();
        let ent = |key: &str| -> Option<EntityId> { def.get(key).as_str().and_then(|s| map.get(&*s)).map(|x| EntityId(*x)) };
        let first = ent("localFirst").or_else(|| ent("local0"));
        let second = ent("localSecond").or_else(|| ent("local1"));
        let need = |i: &mut Interp, e: Option<EntityId>| -> R<EntityId> {
            match e {
                Some(e) => Ok(e),
                None => Err(raise_error(i, "SKETCH_MISSING_ENTITY", None)),
            }
        };
        let len = magnitude_of(def.get("length"));
        let kind = match &*ty {
            "COINCIDENT" => ConstraintKind::Coincident { a: need(i, first)?, b: need(i, second)? },
            "PARALLEL" => ConstraintKind::Parallel { a: need(i, first)?, b: need(i, second)? },
            "PERPENDICULAR" => ConstraintKind::Perpendicular { a: need(i, first)?, b: need(i, second)? },
            "VERTICAL" => ConstraintKind::Vertical { a: need(i, first)?, b: second },
            "HORIZONTAL" => ConstraintKind::Horizontal { a: need(i, first)?, b: second },
            "CONCENTRIC" => ConstraintKind::Concentric { a: need(i, first)?, b: need(i, second)? },
            "TANGENT" => ConstraintKind::Tangent { a: need(i, first)?, b: need(i, second)? },
            "EQUAL" => ConstraintKind::Equal { a: need(i, first)?, b: need(i, second)? },
            "MIDPOINT" => {
                let (x, y) = (need(i, first)?, need(i, second)?);
                if b.sketch.kind(x) == Some(EntityKind::Point) { ConstraintKind::Midpoint { point: x, curve: y } } else { ConstraintKind::Midpoint { point: y, curve: x } }
            }
            "FIX" => {
                let e = need(i, first)?;
                // An arc's FIX holds its centre and radius, not where it ends (std
                // sheetMetalHem: "This does not fix the end position of the arc").
                if let Some(noshape_sketch::Geometry::Arc { center, start, .. }) = b.sketch.entity(e).map(|x| x.geometry.clone()) {
                    let radius = match (b.sketch.point(center), b.sketch.point(start)) {
                        (Some(c), Some(p)) => (p - c).length(),
                        _ => 0.0,
                    };
                    b.sketch.add_constraint(ConstraintKind::Fix { entity: center }).map_err(|_| raise_error(i, "SKETCH_ADD_CONSTRAINT_FAILED", None))?;
                    ConstraintKind::Radius { curve: e, value: radius }
                } else {
                    ConstraintKind::Fix { entity: e }
                }
            }
            "NORMAL" => ConstraintKind::Normal { line: need(i, first)?, curve: need(i, second)? },
            "MIRROR" => ConstraintKind::Symmetric { a: need(i, first)?, b: need(i, second)?, axis: need(i, ent("localMirror"))? },
            "LENGTH" => ConstraintKind::Length { curve: need(i, first)?, value: len.unwrap_or(0.0) },
            "DISTANCE" => match (second, def.get("direction").as_str().as_deref()) {
                (Some(s), Some("HORIZONTAL")) => ConstraintKind::HorizontalDistance { a: need(i, first)?, b: s, value: len.unwrap_or(0.0) },
                (Some(s), Some("VERTICAL")) => ConstraintKind::VerticalDistance { a: need(i, first)?, b: s, value: len.unwrap_or(0.0) },
                (Some(s), Some("MINIMUM")) => ConstraintKind::Distance { a: need(i, first)?, b: s, value: len.unwrap_or(0.0), mode: noshape_sketch::DistanceMode::Minimum },
                (Some(s), _) => ConstraintKind::Distance { a: need(i, first)?, b: s, value: len.unwrap_or(0.0), mode: Default::default() },
                (None, _) => ConstraintKind::Length { curve: need(i, first)?, value: len.unwrap_or(0.0) },
            },
            "RADIUS" => ConstraintKind::Radius { curve: need(i, first)?, value: len.unwrap_or(0.0) },
            "DIAMETER" => ConstraintKind::Diameter { curve: need(i, first)?, value: len.unwrap_or(0.0) },
            "ANGLE" => ConstraintKind::Angle { a: need(i, first)?, b: need(i, second)?, value: angle_of(def.get("angle")).unwrap_or(0.0) },
            "MAJOR_DIAMETER" | "MINOR_DIAMETER" => ConstraintKind::EllipseRadius { ellipse: need(i, first)?, major: &*ty == "MAJOR_DIAMETER", value: len.unwrap_or(0.0) * 0.5 },
            _ => return Err(raise_error(i, "SKETCH_CONSTRAINT_UNKNOWN", None)),
        };
        b.sketch.add_constraint(kind).map_err(|_| raise_error(i, "SKETCH_ADD_CONSTRAINT_FAILED", None))?;
        Ok(Value::Undefined)
    })
}

fn sk_solve(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let (cx, id) = sketch_arg(i, &a)?;
    let mut st = cx.state.lock();
    let Some(mut b) = st.sketches.remove(&id) else {
        drop(st);
        return Err(raise_error(i, "SKETCH_NOT_ACTIVE", None));
    };
    let report = b.sketch.solve();
    let regions = b.sketch.regions();
    let wires = b.sketch.wires();
    b.solved = true;
    let name = id.last().map(|s| s.to_string()).unwrap_or_default();
    st.ctx.sketches.retain(|s| s.feature != b.marker);
    st.ctx.sketches.push(Arc::new(SketchResult { feature: b.marker, name, frame: b.frame, sketch: b.sketch.clone(), report, regions, wires, dimension_text: BTreeMap::new(), images: b.images.clone() }));
    st.sketches.insert(id, b);
    Ok(Value::Undefined)
}

/// Entity ids of a FeatureScript sketch entity (`sketchEntityQuery`).
pub fn sketch_entity(st: &CtxState, marker: FeatureId, name: &str) -> Option<u32> {
    st.sketch_names.get(&marker)?.get(name).copied()
}

/// `skImage`: an imported image (`BLOB_DATA` merged into the value) in the
/// box from `firstCorner` towards `secondCorner`, keeping the picture's
/// proportions (live document/91: a 16×8 image in a 1×1 in box is 1 × ½ in).
fn sk_image(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let id = entity_id(i, &a)?;
    let def = def_arg(i, &a, 2)?;
    let Some(data) = def.get("dataId").as_str().and_then(|d| super::super::reference::blob_of(&d)) else {
        return Err(i.error("skImage needs an imported image (blobInfo)"));
    };
    let (w, h) = (def.get("imageWidth").as_number().unwrap_or(1.0), def.get("imageHeight").as_number().unwrap_or(1.0));
    let aspect = if h > 0.0 { w / h } else { 1.0 };
    with_sketch(i, &a, |i, b, map| {
        check_new(i, map, &id)?;
        let c0 = p2(def.get("firstCorner")).unwrap_or(DVec2::ZERO);
        let c1 = p2(def.get("secondCorner")).unwrap_or(c0 + DVec2::splat(0.0254));
        let size = (c1 - c0).abs();
        // The largest box of the image's proportions inside the corners.
        let height = size.y.min(size.x / aspect).max(1e-6);
        let tb = b.sketch.add_text_box(c0.min(c1), height, aspect).map_err(|_| i.error("The image rectangle could not be created"))?;
        let n = b.images.len() as u32 + 1;
        b.images.push(crate::regen::context::SketchResultImage { id: n, box_points: tb.points.iter().map(|p| p.0).collect(), data_base64: data.clone() });
        for (k, p) in tb.points.iter().enumerate() {
            register(&b.sketch, map, &format!("{id}.corner{k}"), *p);
        }
        Ok(Value::map_from([("imageId", Value::str(id.as_str()))]))
    })
}
