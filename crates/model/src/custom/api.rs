//! The script API: value conversions, parameter constructors and the `ctx`
//! object whose methods run kernel operations against the regeneration
//! context of the feature being evaluated.
//!
//! Every operation goes through [`EvalCtx`] helpers (`resolve_*`,
//! `profiles`, `name_result`, `apply_operation`, `boolean_into`,
//! `replace_body`), so script-created faces get persistent names like
//! built-in features'. Roles are keyed by an operation key: the running
//! operation counter of the script run, or a hash of the `id` option.

use super::compile::{DeclKind, ParamDecl, VisibleValue};
use crate::display::{BodyKind, PlaneFrame};
use crate::naming::{Name, ShapeNames};
use crate::params::{ParamValues, QueryFilter};
use crate::reference::{BodyId, FeatureId, Reference, Role, Source, TopoName};
use crate::regen::context::Context;
use crate::regen::eval::{EvalCtx, FResult, FeatureError, NamedShape, Profile, err};
use crate::regen::features::helpers_a::{copy_names, dedup_names, push_body};
use glam::{DMat4, DVec3};
use noshape_kernel as k;
use noshape_kernel::{Shape, ShapeType};
use noshape_units::{Quantity, UnitPrefs};
use parking_lot::Mutex;
use rhai::{Array, Dynamic, Engine, EvalAltResult, Map};
use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

type R<T> = Result<T, Box<EvalAltResult>>;

fn fe(e: FeatureError) -> Box<EvalAltResult> {
    e.message.into()
}

// ---------------------------------------------------------------- values

/// A persistent reference to geometry (body, face, edge, vertex, sketch
/// region, plane...), as scripts see it.
#[derive(Clone, Debug, PartialEq)]
pub struct Entity(pub Reference);

impl Entity {
    fn kind(&self) -> &'static str {
        match &self.0 {
            Reference::Body(_) => "body",
            Reference::Face(_) => "face",
            Reference::Edge(_) => "edge",
            Reference::Vertex(_) => "vertex",
            Reference::SketchRegion { .. } => "region",
            Reference::SketchEntity { .. } => "sketch entity",
            Reference::Sketch(_) => "sketch",
            Reference::Plane(_) | Reference::FeaturePlane(_) => "plane",
            Reference::Origin => "point",
            Reference::MateConnector { .. } | Reference::Implicit { .. } => "mate connector",
            Reference::Feature(_) => "feature",
            Reference::QueryVariable(_) => "query variable",
        }
    }
}

fn num(d: &Dynamic, what: &str) -> R<f64> {
    let d = d.clone().flatten();
    if let Ok(f) = d.as_float() {
        return Ok(f);
    }
    if let Ok(i) = d.as_int() {
        return Ok(i as f64);
    }
    Err(format!("{what} must be a number (got {})", d.type_name()).into())
}

fn vec3(d: &Dynamic, what: &str) -> R<DVec3> {
    let d = d.clone().flatten();
    let arr = d.into_array().map_err(|_| format!("{what} must be a vector [x, y, z]"))?;
    if arr.len() != 3 {
        return Err(format!("{what} must be a vector [x, y, z]").into());
    }
    Ok(DVec3::new(num(&arr[0], what)?, num(&arr[1], what)?, num(&arr[2], what)?))
}

fn vdyn(v: DVec3) -> Dynamic {
    Dynamic::from_array(vec![Dynamic::from_float(v.x), Dynamic::from_float(v.y), Dynamic::from_float(v.z)])
}

fn refs_into(d: Dynamic, out: &mut Vec<Reference>) -> R<()> {
    let d = d.flatten();
    if d.is_unit() {
        return Ok(());
    }
    if d.is::<Entity>() {
        let e = d.cast::<Entity>();
        if !out.contains(&e.0) {
            out.push(e.0);
        }
        return Ok(());
    }
    if d.is_array() {
        for x in d.into_array().unwrap_or_default() {
            refs_into(x, out)?;
        }
        return Ok(());
    }
    Err(format!("expected an entity or an array of entities (got {})", d.type_name()).into())
}

fn refs(d: &Dynamic) -> R<Vec<Reference>> {
    let mut out = Vec::new();
    refs_into(d.clone(), &mut out)?;
    Ok(out)
}

fn ents(v: impl IntoIterator<Item = Reference>) -> Array {
    v.into_iter().map(|r| Dynamic::from(Entity(r))).collect()
}

fn map_get<'a>(m: &'a Map, k: &str) -> Option<&'a Dynamic> {
    m.get(k)
}

fn opt_bool(m: &Map, k: &str) -> R<bool> {
    match map_get(m, k) {
        None => Ok(false),
        Some(v) => v.clone().flatten().as_bool().map_err(|_| format!("option \"{k}\" must be true or false").into()),
    }
}

fn opt_text(m: &Map, k: &str) -> R<Option<String>> {
    match map_get(m, k) {
        None => Ok(None),
        Some(v) => v.clone().flatten().into_string().map(Some).map_err(|_| format!("option \"{k}\" must be a string").into()),
    }
}

fn frame_map(f: &PlaneFrame) -> Map {
    let mut m = Map::new();
    m.insert("origin".into(), vdyn(f.origin));
    m.insert("normal".into(), vdyn(f.normal));
    m.insert("x".into(), vdyn(f.x));
    m.insert("y".into(), vdyn(f.y));
    m
}

// ---------------------------------------------------------------- query filters

/// Query parameter filter names (usable as constants at the top level, or
/// as strings, combined with `|`).
pub(super) const FILTERS: &[(&str, QueryFilter)] = &[
    ("FACE", QueryFilter::ANY_FACE),
    ("PLANAR_FACE", QueryFilter::PLANAR_FACE),
    ("CYLINDRICAL_FACE", QueryFilter::CYLINDRICAL_FACE),
    ("EDGE", QueryFilter::ANY_EDGE),
    ("LINEAR_EDGE", QueryFilter::LINEAR_EDGE),
    ("CIRCULAR_EDGE", QueryFilter::CIRCULAR_EDGE),
    ("VERTEX", QueryFilter::POINT_LIKE),
    ("BODY", QueryFilter::BODY),
    ("SOLID", QueryFilter::SOLID_BODY),
    ("SURFACE", QueryFilter::SHEET_BODY),
    ("REGION", QueryFilter::SKETCH_REGION),
    ("PLANE", QueryFilter::PLANE_LIKE),
    ("AXIS", QueryFilter::AXIS),
    ("DIRECTION", QueryFilter::DIRECTION),
    ("POINT", QueryFilter::POINT_LIKE),
    ("MATE_CONNECTOR", QueryFilter::MATE_CONNECTOR),
    ("SKETCH", QueryFilter::SKETCH),
];

fn parse_filter(d: &Dynamic) -> R<u32> {
    let d = d.clone().flatten();
    let names: Vec<String> = if d.is_array() {
        d.into_array().unwrap_or_default().into_iter().map(|x| x.flatten().into_string().unwrap_or_default()).collect()
    } else {
        d.into_string().map_err(|_| "query filter must be a name like FACE or \"FACE|EDGE\"")?.split('|').map(|s| s.trim().to_string()).collect()
    };
    let mut bits = 0;
    for n in names {
        let up = n.to_uppercase();
        let Some((_, f)) = FILTERS.iter().find(|(name, _)| *name == up) else {
            let all: Vec<&str> = FILTERS.iter().map(|f| f.0).collect();
            return Err(format!("unknown query filter \"{n}\" (use {})", all.join(", ")).into());
        };
        bits |= f.0;
    }
    Ok(bits)
}

// ---------------------------------------------------------------- parameter constructors

fn decl(id: &str, label: &str, kind: DeclKind) -> ParamDecl {
    ParamDecl { id: id.to_string(), label: label.to_string(), kind, description: String::new(), visible_when: None }
}

fn num_decl(id: &str, label: &str, default: &Dynamic, min: Option<(&Dynamic, &Dynamic)>, make: fn(f64, f64, f64) -> DeclKind, lo: f64, hi: f64) -> R<ParamDecl> {
    let d = num(default, "default value")?;
    let (mn, mx) = match min {
        Some((a, b)) => (num(a, "minimum")?, num(b, "maximum")?),
        None => (lo.min(d), hi.max(d)),
    };
    if mn > mx || d < mn || d > mx {
        return Err(format!("parameter \"{id}\": default must lie between minimum and maximum").into());
    }
    Ok(decl(id, label, make(d, mn, mx)))
}

fn enum_options(d: &Dynamic) -> R<Vec<(String, String)>> {
    let arr = d.clone().flatten().into_array().map_err(|_| "enum options must be an array of strings or [id, label] pairs")?;
    let mut out = Vec::new();
    for o in arr {
        let o = o.flatten();
        if o.is_array() {
            let pair = o.into_array().unwrap_or_default();
            let id = pair.first().and_then(|x| x.clone().into_string().ok()).ok_or("enum option pairs must be [id, label]")?;
            let label = pair.get(1).and_then(|x| x.clone().into_string().ok()).unwrap_or_else(|| id.clone());
            out.push((id, label));
        } else {
            let s = o.into_string().map_err(|_| "enum options must be strings or [id, label] pairs")?;
            out.push((s.clone(), s));
        }
    }
    if out.is_empty() {
        return Err("an enum needs at least one option".into());
    }
    Ok(out)
}

fn register_params(e: &mut Engine) {
    e.register_type_with_name::<ParamDecl>("Param");
    e.register_fn("to_string", |d: &mut ParamDecl| format!("Param({})", d.id));
    e.register_fn("query", |id: &str, label: &str, filter: Dynamic| -> R<ParamDecl> { Ok(decl(id, label, DeclKind::Query { filter: parse_filter(&filter)?, max_picks: None })) });
    e.register_fn("query", |id: &str, label: &str, filter: Dynamic, max: i64| -> R<ParamDecl> {
        Ok(decl(id, label, DeclKind::Query { filter: parse_filter(&filter)?, max_picks: (max > 0).then_some(max as usize) }))
    });
    let length = |d, mn, mx| DeclKind::Length { default: d, min: mn, max: mx };
    let angle = |d, mn, mx| DeclKind::Angle { default: d, min: mn, max: mx };
    let real = |d, mn, mx| DeclKind::Real { default: d, min: mn, max: mx };
    e.register_fn("length", move |id: &str, label: &str, d: Dynamic| num_decl(id, label, &d, None, length, 0.0, 500.0));
    e.register_fn("length", move |id: &str, label: &str, d: Dynamic, mn: Dynamic, mx: Dynamic| num_decl(id, label, &d, Some((&mn, &mx)), length, 0.0, 500.0));
    let full = std::f64::consts::TAU;
    e.register_fn("angle", move |id: &str, label: &str, d: Dynamic| num_decl(id, label, &d, None, angle, -full, full));
    e.register_fn("angle", move |id: &str, label: &str, d: Dynamic, mn: Dynamic, mx: Dynamic| num_decl(id, label, &d, Some((&mn, &mx)), angle, -full, full));
    e.register_fn("real", move |id: &str, label: &str, d: Dynamic| num_decl(id, label, &d, None, real, -1e9, 1e9));
    e.register_fn("real", move |id: &str, label: &str, d: Dynamic, mn: Dynamic, mx: Dynamic| num_decl(id, label, &d, Some((&mn, &mx)), real, -1e9, 1e9));
    e.register_fn("int", |id: &str, label: &str, d: i64| decl(id, label, DeclKind::Int { default: d, min: d.min(0), max: d.max(1_000_000) }));
    e.register_fn("int", |id: &str, label: &str, d: i64, mn: i64, mx: i64| -> R<ParamDecl> {
        if mn > mx || d < mn || d > mx {
            return Err(format!("parameter \"{id}\": default must lie between minimum and maximum").into());
        }
        Ok(decl(id, label, DeclKind::Int { default: d, min: mn, max: mx }))
    });
    e.register_fn("bool", |id: &str, label: &str, d: bool| decl(id, label, DeclKind::Bool { default: d }));
    e.register_fn("text", |id: &str, label: &str, d: &str| decl(id, label, DeclKind::Text { default: d.to_string() }));
    e.register_fn("enum_", |id: &str, label: &str, opts: Dynamic| -> R<ParamDecl> {
        let options = enum_options(&opts)?;
        let default = options[0].0.clone();
        Ok(decl(id, label, DeclKind::Enum { options, default }))
    });
    e.register_fn("enum_", |id: &str, label: &str, opts: Dynamic, default: &str| -> R<ParamDecl> {
        let options = enum_options(&opts)?;
        if !options.iter().any(|o| o.0 == default) {
            return Err(format!("parameter \"{id}\": default \"{default}\" is not an option").into());
        }
        Ok(decl(id, label, DeclKind::Enum { options, default: default.to_string() }))
    });
    e.register_fn("describe", |d: &mut ParamDecl, text: &str| -> ParamDecl {
        let mut d = d.clone();
        d.description = text.to_string();
        d
    });
    e.register_fn("visible_when", |d: &mut ParamDecl, other: &str, value: Dynamic| -> R<ParamDecl> {
        let value = value.flatten();
        let v = if let Ok(b) = value.as_bool() {
            VisibleValue::Bool(b)
        } else if let Ok(s) = value.into_string() {
            VisibleValue::Option(s)
        } else {
            return Err("visible_when(param, value): value must be true/false or an enum option".into());
        };
        let mut d = d.clone();
        d.visible_when = Some((other.to_string(), v));
        Ok(d)
    });
}

// ---------------------------------------------------------------- session

/// State of one custom feature evaluation, owned by the `ctx` script value.
pub(super) struct Session {
    pub feature: FeatureId,
    pub name: String,
    pub params: ParamValues,
    pub units: UnitPrefs,
    pub outer_vars: BTreeMap<String, Quantity>,
    pub ctx: Context,
    pub xform: Option<DMat4>,
    /// Operation counter (operation keys of unnamed operations).
    pub op: u32,
    pub warnings: Vec<String>,
}

/// The `ctx` value passed to `evaluate(ctx, params)`.
#[derive(Clone)]
pub struct ScriptCtx(pub(super) Arc<Mutex<Session>>);

impl Session {
    /// Run `f` with an [`EvalCtx`] over the session's context.
    fn ev<T>(&mut self, f: impl FnOnce(&mut EvalCtx) -> FResult<T>) -> R<T> {
        let ctx = std::mem::take(&mut self.ctx);
        let mut ev = EvalCtx::new(self.feature, &self.name, &self.params, &self.units, ctx, &self.outer_vars);
        ev.xform = self.xform;
        let r = f(&mut ev);
        for w in ev.warnings.drain(..) {
            if !self.warnings.contains(&w) {
                self.warnings.push(w);
            }
        }
        self.ctx = ev.ctx;
        r.map_err(fe)
    }

    /// Key for the roles of the next operation: `opts.id` if given (stable
    /// even when the number of earlier operations changes), else a counter.
    fn next_key(&mut self, opts: Option<&Map>) -> R<u32> {
        if let Some(id) = opts.map(|m| opt_text(m, "id")).transpose()?.flatten() {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            id.hash(&mut h);
            return Ok(0x10000 + (h.finish() % 0xF0000) as u32);
        }
        self.op += 1;
        if self.op >= 0x10000 {
            return Err("too many operations in one feature".into());
        }
        Ok(self.op)
    }
}

fn with<T>(c: &mut ScriptCtx, f: impl FnOnce(&mut Session) -> R<T>) -> R<T> {
    let mut s = c.0.lock();
    f(&mut s)
}

/// Bodies created or modified since `before` was taken.
fn snapshot(ctx: &Context) -> Vec<(BodyId, usize)> {
    ctx.bodies.iter().map(|b| (b.id.clone(), Arc::as_ptr(b) as usize)).collect()
}

fn changed(before: &[(BodyId, usize)], ctx: &Context) -> Array {
    ents(ctx.bodies.iter().filter(|b| !before.iter().any(|(id, p)| *id == b.id && *p == Arc::as_ptr(b) as usize)).map(|b| Reference::Body(b.id.clone())))
}

/// Source tagging a profile/edge source with an operation key, so the
/// same profile used by two operations gives different face names.
fn keyed(key: u32, src: Source) -> Source {
    Source::Topo(Arc::new(TopoName::face(FeatureId(key as u64), Role::Profile(src))))
}

// ---------------------------------------------------------------- naming

/// Names for a primitive: planar faces by their outward normal along the
/// primitive's axes (slots 0..6), curved faces by surface type.
fn primitive_names(ev: &EvalCtx, shape: &Shape, axes: [DVec3; 3], key: u32) -> Vec<Name> {
    let names = shape
        .faces()
        .iter()
        .enumerate()
        .map(|(i, f)| {
            let slot = match f.face_surface() {
                Ok(k::Surface::Plane(_)) => {
                    let n = f.face_plane().ok().flatten().map(|p| p.unit_normal()).unwrap_or(DVec3::Z);
                    let (ax, d) = axes.iter().enumerate().map(|(a, v)| (a, n.dot(*v))).max_by(|a, b| a.1.abs().total_cmp(&b.1.abs())).unwrap_or((0, 0.0));
                    (ax as u32) * 2 + (d > 0.0) as u32
                }
                Ok(k::Surface::Cylinder { .. }) => 6,
                Ok(k::Surface::Cone { .. }) => 7,
                Ok(k::Surface::Sphere { .. }) => 8,
                Ok(k::Surface::Torus { .. }) => 9,
                _ => 10 + i as u32,
            };
            Arc::new(TopoName::face(ev.feature, Role::Datum((key << 8) | slot)))
        })
        .collect();
    dedup_names(ev, names)
}

/// Names for an extruded / revolved profile: caps and sides keyed by the
/// operation.
fn swept_names(ev: &EvalCtx, p: &Profile, res: &k::OpResult, key: u32, caps: bool) -> Vec<Name> {
    let mut roles = BTreeMap::new();
    if caps {
        for f in res.history.first_faces() {
            roles.insert(f, Role::CapStart(Some(keyed(key, p.key.clone()))));
        }
        for f in res.history.last_faces() {
            roles.insert(f, Role::CapEnd(Some(keyed(key, p.key.clone()))));
        }
    }
    for r in &res.history.records {
        if r.arg == 0 && r.input_kind == ShapeType::Edge && r.relation == k::Relation::Generated && r.output_kind == ShapeType::Face {
            if let Some(src) = p.edge_sources.get(r.input_index as usize) {
                roles.entry(r.output_index).or_insert_with(|| Role::Side(keyed(key, src.clone())));
            }
        }
    }
    let empty = ShapeNames::default();
    let names = ev.name_result(&[&empty], &res.history, &res.shape, &roles, None);
    dedup_names(ev, names)
}

// ---------------------------------------------------------------- operations

struct OpOpts {
    op: String,
    scope: Option<Vec<Reference>>,
}

fn op_opts(m: Option<&Map>) -> R<OpOpts> {
    let Some(m) = m else { return Ok(OpOpts { op: "NEW".into(), scope: None }) };
    let op = opt_text(m, "op")?.unwrap_or_else(|| "NEW".into()).to_uppercase();
    let op = match op.as_str() {
        "NEW" | "ADD" | "REMOVE" | "INTERSECT" => op,
        "UNION" => "ADD".into(),
        "SUBTRACT" => "REMOVE".into(),
        other => return Err(format!("unknown op \"{other}\" (use \"new\", \"add\", \"remove\" or \"intersect\")").into()),
    };
    let scope = match m.get("scope") {
        Some(s) => Some(refs(s)?),
        None => None,
    };
    Ok(OpOpts { op, scope })
}

/// Add tool solids with the result operation. New bodies get ids derived
/// from the operation key (unique within the feature).
fn apply(ev: &mut EvalCtx, key: u32, tools: Vec<NamedShape>, o: &OpOpts) -> FResult<()> {
    if tools.is_empty() {
        return err("Nothing was created");
    }
    let new_bodies = |ev: &mut EvalCtx, tools: Vec<NamedShape>| {
        let mut n = 0u32;
        for t in tools {
            for p in crate::regen::eval::split_solids(&t.shape, &t.names.faces) {
                push_body(ev, BodyId { feature: ev.feature, index: key * 1024 + n.min(1023) }, BodyKind::Solid, p.shape, p.names.faces);
                n += 1;
            }
        }
    };
    if o.op == "NEW" {
        new_bodies(ev, tools);
        return Ok(());
    }
    let scope = match &o.scope {
        Some(s) => ev.resolve_bodies(s),
        None => ev.auto_scope(&tools),
    };
    if scope.is_empty() {
        if o.op == "ADD" {
            new_bodies(ev, tools);
            return Ok(());
        }
        return err("Nothing to merge with: the result does not touch any part (set the scope option)");
    }
    ev.apply_operation(tools, &o.op, Some(scope), BodyKind::Solid)
}

fn primitive(c: &mut ScriptCtx, opts: Option<Map>, make: impl FnOnce() -> R<(Shape, [DVec3; 3])>) -> R<Array> {
    let (shape, axes) = make()?;
    with(c, |s| {
        let key = s.next_key(opts.as_ref())?;
        let o = op_opts(opts.as_ref())?;
        let before = snapshot(&s.ctx);
        s.ev(|ev| {
            let names = primitive_names(ev, &shape, axes, key);
            apply(ev, key, vec![NamedShape::new(shape, names)], &o)
        })?;
        Ok(changed(&before, &s.ctx))
    })
}

fn kerr(what: &str) -> impl Fn(k::KernelError) -> Box<EvalAltResult> + '_ {
    move |e| format!("{what} failed: {e}").into()
}

fn positive(v: f64, what: &str) -> R<f64> {
    if v > 0.0 && v.is_finite() { Ok(v) } else { Err(format!("{what} must be positive").into()) }
}

fn axis_frame(origin: DVec3, axis: DVec3) -> R<(k::Plane, [DVec3; 3])> {
    if axis.length_squared() < 1e-24 {
        return Err("axis direction must not be zero".into());
    }
    let p = k::Plane::from_normal(origin, axis);
    let axes = [p.unit_x(), p.unit_y(), p.unit_normal()];
    Ok((p, axes))
}

fn op_box(c: &mut ScriptCtx, corner: Dynamic, size: Dynamic, opts: Option<Map>) -> R<Array> {
    primitive(c, opts, || {
        let a = vec3(&corner, "box corner")?;
        let s = vec3(&size, "box size")?;
        let min = a.min(a + s);
        let d = s.abs();
        if d.min_element() <= 0.0 {
            return Err("box size must be non-zero in x, y and z".into());
        }
        let shape = k::make_box(&k::Plane::new(min, DVec3::Z, DVec3::X), d.x, d.y, d.z).map_err(kerr("Box"))?;
        Ok((shape, [DVec3::X, DVec3::Y, DVec3::Z]))
    })
}

fn op_cylinder(c: &mut ScriptCtx, base: Dynamic, axis: Dynamic, r: Dynamic, h: Dynamic, opts: Option<Map>) -> R<Array> {
    primitive(c, opts, || {
        let (o, mut a, r, mut h) = (vec3(&base, "cylinder base")?, vec3(&axis, "cylinder axis")?, positive(num(&r, "radius")?, "Cylinder radius")?, num(&h, "height")?);
        if h < 0.0 {
            (a, h) = (-a, -h);
        }
        let (p, axes) = axis_frame(o, a)?;
        let shape = k::make_cylinder(&p, r, positive(h, "Cylinder height")?, std::f64::consts::TAU).map_err(kerr("Cylinder"))?;
        Ok((shape, axes))
    })
}

fn op_cone(c: &mut ScriptCtx, base: Dynamic, axis: Dynamic, r1: Dynamic, r2: Dynamic, h: Dynamic, opts: Option<Map>) -> R<Array> {
    primitive(c, opts, || {
        let (o, a) = (vec3(&base, "cone base")?, vec3(&axis, "cone axis")?);
        let (r1, r2, h) = (num(&r1, "radius")?, num(&r2, "radius")?, positive(num(&h, "height")?, "Cone height")?);
        if r1 < 0.0 || r2 < 0.0 || (r1 == 0.0 && r2 == 0.0) {
            return Err("cone radii must be non-negative and not both zero".into());
        }
        let (p, axes) = axis_frame(o, a)?;
        let shape = k::make_cone(&p, r1, r2, h, std::f64::consts::TAU).map_err(kerr("Cone"))?;
        Ok((shape, axes))
    })
}

fn op_sphere(c: &mut ScriptCtx, center: Dynamic, r: Dynamic, opts: Option<Map>) -> R<Array> {
    primitive(c, opts, || {
        let (o, r) = (vec3(&center, "sphere center")?, positive(num(&r, "radius")?, "Sphere radius")?);
        let shape = k::make_sphere(&k::Plane::new(o, DVec3::Z, DVec3::X), r, std::f64::consts::TAU).map_err(kerr("Sphere"))?;
        Ok((shape, [DVec3::X, DVec3::Y, DVec3::Z]))
    })
}

fn op_torus(c: &mut ScriptCtx, center: Dynamic, axis: Dynamic, rmaj: Dynamic, rmin: Dynamic, opts: Option<Map>) -> R<Array> {
    primitive(c, opts, || {
        let (o, a) = (vec3(&center, "torus center")?, vec3(&axis, "torus axis")?);
        let (rmaj, rmin) = (positive(num(&rmaj, "radius")?, "Torus radius")?, positive(num(&rmin, "radius")?, "Torus tube radius")?);
        let (p, axes) = axis_frame(o, a)?;
        let shape = k::make_torus(&p, rmaj, rmin, std::f64::consts::TAU).map_err(kerr("Torus"))?;
        Ok((shape, axes))
    })
}

fn op_extrude(c: &mut ScriptCtx, profiles: Dynamic, depth: Dynamic, opts: Option<Map>) -> R<Array> {
    let refs = refs(&profiles)?;
    let depth = num(&depth, "extrude depth")?;
    if depth == 0.0 {
        return Err("extrude depth must not be zero".into());
    }
    let m = opts.clone().unwrap_or_default();
    let dir = match m.get("direction") {
        Some(d) => Some(vec3(d, "direction")?.normalize_or_zero()),
        None => None,
    };
    let symmetric = opt_bool(&m, "symmetric")?;
    with(c, |s| {
        let key = s.next_key(opts.as_ref())?;
        let o = op_opts(opts.as_ref())?;
        let before = snapshot(&s.ctx);
        s.ev(|ev| {
            let profiles = ev.profiles(&refs)?;
            if profiles.is_empty() {
                return err("Nothing to extrude: pass sketch regions or planar faces");
            }
            let mut tools = Vec::new();
            for p in &profiles {
                let mut d = dir.unwrap_or(p.frame.normal);
                if d.length_squared() < 0.5 || d.dot(p.frame.normal).abs() < 1e-9 {
                    return err("Extrude direction can't be parallel to the profile");
                }
                d *= depth;
                let res = if symmetric { k::extrude_between(&p.shape, -d * 0.5, d * 0.5, 0.0) } else { k::extrude(&p.shape, d) }
                    .map_err(|e| FeatureError::new(format!("Extrude failed: {e}")))?;
                let names = swept_names(ev, p, &res, key, true);
                tools.push(NamedShape::new(res.shape.clone(), names));
            }
            apply(ev, key, tools, &o)
        })?;
        Ok(changed(&before, &s.ctx))
    })
}

/// An axis from an entity or a `#{ origin, direction }` map.
fn axis_of(ev: &EvalCtx, d: &Dynamic) -> FResult<(DVec3, DVec3)> {
    let d = d.clone().flatten();
    if d.is_map() {
        let m = d.cast::<Map>();
        let get = |k: &str| m.get(k).and_then(|v| vec3(v, k).ok());
        return match (get("origin"), get("direction")) {
            (Some(o), Some(dir)) if dir.length_squared() > 0.0 => Ok((o, dir.normalize())),
            _ => err("An axis map needs origin: [x, y, z] and direction: [x, y, z]"),
        };
    }
    if d.is::<Entity>() {
        let e = d.cast::<Entity>();
        return ev.resolve_axis(&e.0).ok_or_else(|| FeatureError::new("Could not get an axis from this entity"));
    }
    err("An axis must be an entity (edge, cylindrical face...) or #{ origin: [..], direction: [..] }")
}

fn op_revolve(c: &mut ScriptCtx, profiles: Dynamic, axis: Dynamic, angle: Dynamic, opts: Option<Map>) -> R<Array> {
    let refs = refs(&profiles)?;
    let angle = num(&angle, "revolve angle")?;
    if angle == 0.0 {
        return Err("revolve angle must not be zero".into());
    }
    let full = angle.abs() >= std::f64::consts::TAU - 1e-9;
    with(c, |s| {
        let key = s.next_key(opts.as_ref())?;
        let o = op_opts(opts.as_ref())?;
        let before = snapshot(&s.ctx);
        s.ev(|ev| {
            let (origin, dir) = axis_of(ev, &axis)?;
            let profiles = ev.profiles(&refs)?;
            if profiles.is_empty() {
                return err("Nothing to revolve: pass sketch regions or planar faces");
            }
            let mut tools = Vec::new();
            for p in &profiles {
                let a = if full { std::f64::consts::TAU } else { angle };
                let res = k::revolve(&p.shape, origin, dir, a).map_err(|e| FeatureError::new(format!("Revolve failed: {e}")))?;
                let names = swept_names(ev, p, &res, key, !full);
                tools.push(NamedShape::new(res.shape.clone(), names));
            }
            apply(ev, key, tools, &o)
        })?;
        Ok(changed(&before, &s.ctx))
    })
}

#[derive(Clone, Copy, PartialEq)]
enum Bool {
    Union,
    Subtract,
    Intersect,
}

fn op_boolean(c: &mut ScriptCtx, kind: Bool, targets: Dynamic, tools: Dynamic, opts: Option<Map>) -> R<Array> {
    let t_refs = refs(&targets)?;
    let tool_refs = refs(&tools)?;
    let keep = opts.as_ref().map(|m| opt_bool(m, "keep_tools")).transpose()?.unwrap_or(false);
    with(c, |s| {
        let before = snapshot(&s.ctx);
        s.ev(|ev| {
            let ti = ev.resolve_bodies(&t_refs);
            if ti.is_empty() {
                return err("Boolean: no target bodies");
            }
            let target_ids: Vec<BodyId> = ti.iter().map(|i| ev.ctx.bodies[*i].id.clone()).collect();
            let tool_idx: Vec<usize> = ev.resolve_bodies(&tool_refs).into_iter().filter(|i| !ti.contains(i)).collect();
            if tool_idx.is_empty() && !(kind == Bool::Union && ti.len() > 1) {
                return err("Boolean: no tool bodies");
            }
            let mut tool_shapes: Vec<NamedShape> = tool_idx.iter().map(|i| crate::regen::features::helpers_a::body_named(&ev.ctx.bodies[*i])).collect();
            let tool_ids: Vec<BodyId> = tool_idx.iter().map(|i| ev.ctx.bodies[*i].id.clone()).collect();
            if !keep || kind == Bool::Union {
                ev.ctx.bodies.retain(|b| !tool_ids.contains(&b.id));
            }
            let index = |ev: &EvalCtx, id: &BodyId| ev.ctx.bodies.iter().position(|b| &b.id == id);
            match kind {
                Bool::Union => {
                    // Everything fuses into the first target.
                    let others: Vec<BodyId> = target_ids[1..].to_vec();
                    for id in &others {
                        if let Some(i) = index(ev, id) {
                            tool_shapes.push(crate::regen::features::helpers_a::body_named(&ev.ctx.bodies[i]));
                        }
                    }
                    ev.ctx.bodies.retain(|b| !others.contains(&b.id));
                    let t0 = index(ev, &target_ids[0]).ok_or_else(|| FeatureError::new("Boolean: target vanished"))?;
                    ev.boolean_into(&[t0], tool_shapes, k::BooleanOp::Fuse)
                }
                Bool::Subtract | Bool::Intersect => {
                    let idx: Vec<usize> = target_ids.iter().filter_map(|id| index(ev, id)).collect();
                    let op = if kind == Bool::Subtract { k::BooleanOp::Cut } else { k::BooleanOp::Common };
                    ev.boolean_into(&idx, tool_shapes, op)
                }
            }
        })?;
        Ok(changed(&before, &s.ctx))
    })
}

fn by_body(items: Vec<(usize, usize)>) -> BTreeMap<usize, Vec<usize>> {
    let mut m: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for (b, e) in items {
        m.entry(b).or_default().push(e);
    }
    m
}

fn op_blend(c: &mut ScriptCtx, edges: Dynamic, size: Dynamic, chamfer: bool) -> R<Array> {
    let refs = refs(&edges)?;
    let size = positive(num(&size, if chamfer { "chamfer distance" } else { "fillet radius" })?, if chamfer { "Chamfer distance" } else { "Fillet radius" })?;
    with(c, |s| {
        let before = snapshot(&s.ctx);
        s.ev(|ev| {
            let groups = by_body(ev.resolve_edges(&refs));
            if groups.is_empty() {
                return err(if chamfer { "Chamfer: no edges" } else { "Fillet: no edges" });
            }
            let blend = |n: &Name| Role::Blend(n.clone());
            for (bi, edges) in groups.into_iter().rev() {
                let body = ev.ctx.bodies[bi].clone();
                let shapes: Vec<Shape> = edges.iter().filter_map(|e| body.shape.sub_shape(ShapeType::Edge, *e).ok()).collect();
                let res = if chamfer { k::chamfer(&body.shape, &shapes, k::ChamferSpec::Equal(size), &[]) } else { k::fillet(&body.shape, &shapes, size) }
                    .map_err(|e| FeatureError::new(if chamfer { format!("Chamfer failed: {e}") } else { format!("Fillet failed (radius may be too large): {e}") }))?;
                let names = body.shape_names();
                let faces = ev.name_result(&[&names], &res.history, &res.shape, &BTreeMap::new(), Some(&blend));
                ev.replace_body(bi, res.shape, faces);
            }
            Ok(())
        })?;
        Ok(changed(&before, &s.ctx))
    })
}

/// Move bodies in place (names are kept).
fn transform_in_place(c: &mut ScriptCtx, bodies: Dynamic, t: R<k::Transform>) -> R<Array> {
    let refs = refs(&bodies)?;
    let t = t?;
    with(c, |s| {
        let before = snapshot(&s.ctx);
        s.ev(|ev| {
            let idx = ev.resolve_bodies(&refs);
            if idx.is_empty() {
                return err("Transform: no bodies");
            }
            for bi in idx {
                let body = ev.ctx.bodies[bi].clone();
                let res = k::transform(&body.shape, &t, true)?;
                let names = body.shape_names();
                let faces = ev.name_result(&[&names], &res.history, &res.shape, &BTreeMap::new(), None);
                ev.replace_body(bi, res.shape, faces);
            }
            Ok(())
        })?;
        Ok(changed(&before, &s.ctx))
    })
}

/// Copies of bodies, one per transform (instance numbers start at 1).
fn copies(c: &mut ScriptCtx, bodies: Dynamic, transforms: Vec<k::Transform>, opts: Option<Map>) -> R<Array> {
    let refs = refs(&bodies)?;
    if transforms.len() > 1000 {
        return Err("at most 1000 copies per operation".into());
    }
    with(c, |s| {
        let key = s.next_key(opts.as_ref())?;
        let before = snapshot(&s.ctx);
        s.ev(|ev| {
            let idx = ev.resolve_bodies(&refs);
            if idx.is_empty() {
                return err("Copy: no bodies");
            }
            let mut n = 0u32;
            for bi in idx {
                let body = ev.ctx.bodies[bi].clone();
                for (i, t) in transforms.iter().enumerate() {
                    let res = k::transform(&body.shape, t, true)?;
                    let instance = (key << 10) | ((i as u32 + 1) & 1023);
                    let faces = copy_names(ev, &body.faces, &res, instance);
                    push_body(ev, BodyId { feature: ev.feature, index: key * 1024 + (n & 1023) }, body.kind, res.shape, faces);
                    n += 1;
                }
            }
            Ok(())
        })?;
        Ok(changed(&before, &s.ctx))
    })
}

fn op_delete(c: &mut ScriptCtx, bodies: Dynamic) -> R<()> {
    let refs = refs(&bodies)?;
    with(c, |s| {
        s.ev(|ev| {
            let idx = ev.resolve_bodies(&refs);
            let ids: Vec<BodyId> = idx.iter().map(|i| ev.ctx.bodies[*i].id.clone()).collect();
            ev.ctx.bodies.retain(|b| !ids.contains(&b.id));
            Ok(())
        })
    })
}

// ---------------------------------------------------------------- queries & measurement

/// Kernel sub-shapes of references (faces, edges, vertices, bodies).
fn shapes_of(ev: &EvalCtx, refs: &[Reference]) -> Vec<(Reference, Shape)> {
    let mut out = Vec::new();
    for r in refs {
        match r {
            Reference::Face(_) => {
                for (bi, fi) in ev.resolve_faces(std::slice::from_ref(r)) {
                    if let Ok(s) = ev.face_shape(bi, fi) {
                        out.push((r.clone(), s));
                    }
                }
            }
            Reference::Edge(_) => {
                for (bi, ei) in ev.resolve_edges(std::slice::from_ref(r)) {
                    if let Ok(s) = ev.edge_shape(bi, ei) {
                        out.push((r.clone(), s));
                    }
                }
            }
            Reference::Vertex(n) => {
                for b in &ev.ctx.bodies {
                    for vi in b.find_vertices(n) {
                        if let Ok(s) = b.shape.sub_shape(ShapeType::Vertex, vi) {
                            out.push((r.clone(), s));
                        }
                    }
                }
            }
            Reference::Body(_) => {
                for bi in ev.resolve_bodies(std::slice::from_ref(r)) {
                    out.push((r.clone(), ev.ctx.bodies[bi].shape.clone()));
                }
            }
            _ => {}
        }
    }
    out
}

/// Geometry type name of an entity.
fn geometry_type(ev: &EvalCtx, r: &Reference) -> String {
    match r {
        Reference::Body(_) => match ev.resolve_bodies(std::slice::from_ref(r)).first().map(|i| ev.ctx.bodies[*i].kind) {
            Some(BodyKind::Solid) => "SOLID",
            Some(BodyKind::Surface) => "SURFACE",
            Some(BodyKind::Wire) => "WIRE",
            Some(BodyKind::Point) => "POINT",
            None => "NONE",
        }
        .into(),
        Reference::Face(_) => match shapes_of(ev, std::slice::from_ref(r)).first().map(|(_, s)| s.face_surface()) {
            Some(Ok(k::Surface::Plane(_))) => "PLANE",
            Some(Ok(k::Surface::Cylinder { .. })) => "CYLINDER",
            Some(Ok(k::Surface::Cone { .. })) => "CONE",
            Some(Ok(k::Surface::Sphere { .. })) => "SPHERE",
            Some(Ok(k::Surface::Torus { .. })) => "TORUS",
            Some(Ok(k::Surface::BSpline { .. } | k::Surface::Bezier { .. })) => "SPLINE",
            Some(_) => "OTHER",
            None => "NONE",
        }
        .into(),
        Reference::Edge(_) => match shapes_of(ev, std::slice::from_ref(r)).first().map(|(_, s)| s.edge_curve()) {
            Some(Ok(k::Curve::Line { .. })) => "LINE",
            Some(Ok(k::Curve::Circle { .. })) => "CIRCLE",
            Some(Ok(k::Curve::Ellipse { .. })) => "ELLIPSE",
            Some(Ok(k::Curve::BSpline { .. } | k::Curve::Bezier { .. })) => "SPLINE",
            Some(_) => "OTHER",
            None => "NONE",
        }
        .into(),
        Reference::Vertex(_) => "POINT".into(),
        Reference::Plane(_) | Reference::FeaturePlane(_) => "PLANE".into(),
        Reference::SketchRegion { .. } => "REGION".into(),
        _ => "OTHER".into(),
    }
}

fn faces_of(ev: &EvalCtx, refs: &[Reference]) -> Vec<Reference> {
    let mut out: Vec<Reference> = Vec::new();
    let mut push = |r: Reference| {
        if !out.contains(&r) {
            out.push(r)
        }
    };
    for r in refs {
        match r {
            Reference::Body(_) => {
                for bi in ev.resolve_bodies(std::slice::from_ref(r)) {
                    for n in &ev.ctx.bodies[bi].faces {
                        push(Reference::Face((**n).clone()));
                    }
                }
            }
            Reference::Face(_) => push(r.clone()),
            Reference::Edge(_) => {
                for (bi, ei) in ev.resolve_edges(std::slice::from_ref(r)) {
                    let b = &ev.ctx.bodies[bi];
                    for fi in b.shape.ancestors(ShapeType::Edge, ei, ShapeType::Face).unwrap_or_default() {
                        if let Some(n) = b.faces.get(fi) {
                            push(Reference::Face((**n).clone()));
                        }
                    }
                }
            }
            _ => {}
        }
    }
    out
}

fn edges_of(ev: &EvalCtx, refs: &[Reference]) -> Vec<Reference> {
    let mut out: Vec<Reference> = Vec::new();
    let mut push = |r: Reference| {
        if !out.contains(&r) {
            out.push(r)
        }
    };
    for r in refs {
        match r {
            Reference::Body(_) => {
                for bi in ev.resolve_bodies(std::slice::from_ref(r)) {
                    for n in &ev.ctx.bodies[bi].edges {
                        push(Reference::Edge((**n).clone()));
                    }
                }
            }
            // resolve_edges expands a face into its edges.
            Reference::Face(_) | Reference::Edge(_) => {
                for (bi, ei) in ev.resolve_edges(std::slice::from_ref(r)) {
                    if let Some(n) = ev.ctx.bodies[bi].edges.get(ei) {
                        push(Reference::Edge((**n).clone()));
                    }
                }
            }
            _ => {}
        }
    }
    out
}

fn vertices_of(ev: &EvalCtx, refs: &[Reference]) -> Vec<Reference> {
    let mut out: Vec<Reference> = Vec::new();
    for (r, shape) in shapes_of(ev, refs) {
        let _ = r;
        for b in &ev.ctx.bodies {
            for v in shape.vertices() {
                if let Some(vi) = b.shape.index_of(&v, ShapeType::Vertex)
                    && let Some(n) = b.vertices.get(vi)
                {
                    let r = Reference::Vertex((**n).clone());
                    if !out.contains(&r) {
                        out.push(r);
                    }
                }
            }
        }
    }
    out
}

fn bbox_of(ev: &EvalCtx, refs: &[Reference]) -> FResult<(DVec3, DVec3)> {
    let mut acc: Option<(DVec3, DVec3)> = None;
    let mut add = |a: DVec3, b: DVec3| {
        acc = Some(match acc {
            None => (a, b),
            Some((x, y)) => (x.min(a), y.max(b)),
        })
    };
    for (_, s) in shapes_of(ev, refs) {
        let b = s.bounding_box(true)?;
        add(b.min, b.max);
    }
    for r in refs {
        if !matches!(r, Reference::Face(_) | Reference::Edge(_) | Reference::Vertex(_) | Reference::Body(_))
            && let Some(p) = ev.resolve_vertex_point(r)
        {
            add(p, p);
        }
    }
    acc.ok_or_else(|| FeatureError::new("Bounding box: nothing to measure"))
}

fn query_fn(c: &mut ScriptCtx, of: Dynamic, f: fn(&EvalCtx, &[Reference]) -> Vec<Reference>) -> R<Array> {
    let refs = refs(&of)?;
    with(c, |s| s.ev(|ev| Ok(ents(f(ev, &refs)))))
}

fn measure(c: &mut ScriptCtx, of: Dynamic, f: impl FnOnce(&EvalCtx, &[Reference]) -> FResult<Dynamic>) -> R<Dynamic> {
    let refs = refs(&of)?;
    with(c, |s| s.ev(|ev| f(ev, &refs)))
}

fn single(d: &Dynamic, what: &str) -> R<Reference> {
    let r = refs(d)?;
    r.into_iter().next().ok_or_else(|| format!("{what}: expected an entity").into())
}

// ---------------------------------------------------------------- registration

fn register_math(e: &mut Engine) {
    for (name, f) in [("mm", 1e-3), ("cm", 1e-2), ("meter", 1.0), ("m", 1.0), ("inch", 0.0254), ("ft", 0.3048), ("deg", std::f64::consts::PI / 180.0), ("rad", 1.0)] {
        e.register_fn(name, move |x: Dynamic| -> R<f64> { Ok(num(&x, "value")? * f) });
    }
    for (name, f) in [("to_mm", 1e3), ("to_inch", 1.0 / 0.0254), ("to_deg", 180.0 / std::f64::consts::PI)] {
        e.register_fn(name, move |x: Dynamic| -> R<f64> { Ok(num(&x, "value")? * f) });
    }
    e.register_fn("vec", |x: Dynamic, y: Dynamic, z: Dynamic| -> R<Dynamic> { Ok(vdyn(DVec3::new(num(&x, "x")?, num(&y, "y")?, num(&z, "z")?))) });
    e.register_fn("vadd", |a: Dynamic, b: Dynamic| -> R<Dynamic> { Ok(vdyn(vec3(&a, "vector")? + vec3(&b, "vector")?)) });
    e.register_fn("vsub", |a: Dynamic, b: Dynamic| -> R<Dynamic> { Ok(vdyn(vec3(&a, "vector")? - vec3(&b, "vector")?)) });
    e.register_fn("vscale", |a: Dynamic, s: Dynamic| -> R<Dynamic> { Ok(vdyn(vec3(&a, "vector")? * num(&s, "scale")?)) });
    e.register_fn("vdot", |a: Dynamic, b: Dynamic| -> R<f64> { Ok(vec3(&a, "vector")?.dot(vec3(&b, "vector")?)) });
    e.register_fn("vcross", |a: Dynamic, b: Dynamic| -> R<Dynamic> { Ok(vdyn(vec3(&a, "vector")?.cross(vec3(&b, "vector")?))) });
    e.register_fn("vlen", |a: Dynamic| -> R<f64> { Ok(vec3(&a, "vector")?.length()) });
    e.register_fn("vnorm", |a: Dynamic| -> R<Dynamic> { Ok(vdyn(vec3(&a, "vector")?.normalize_or_zero())) });
}

fn register_entity(e: &mut Engine) {
    e.register_type_with_name::<Entity>("Entity");
    e.register_fn("to_string", |x: &mut Entity| format!("{} {:?}", x.kind(), x.0));
    e.register_fn("to_debug", |x: &mut Entity| format!("{} {:?}", x.kind(), x.0));
    e.register_fn("==", |a: &mut Entity, b: Entity| a.0 == b.0);
    e.register_fn("!=", |a: &mut Entity, b: Entity| a.0 != b.0);
    e.register_get("kind", |x: &mut Entity| x.kind().to_string());
}

fn register_ctx(e: &mut Engine) {
    e.register_type_with_name::<ScriptCtx>("Context");
    e.register_fn("to_string", |_: &mut ScriptCtx| "Context".to_string());

    // Solids without sketches.
    e.register_fn("box", |c: &mut ScriptCtx, a: Dynamic, s: Dynamic| op_box(c, a, s, None));
    e.register_fn("box", |c: &mut ScriptCtx, a: Dynamic, s: Dynamic, o: Map| op_box(c, a, s, Some(o)));
    e.register_fn("cylinder", |c: &mut ScriptCtx, b: Dynamic, a: Dynamic, r: Dynamic, h: Dynamic| op_cylinder(c, b, a, r, h, None));
    e.register_fn("cylinder", |c: &mut ScriptCtx, b: Dynamic, a: Dynamic, r: Dynamic, h: Dynamic, o: Map| op_cylinder(c, b, a, r, h, Some(o)));
    e.register_fn("cone", |c: &mut ScriptCtx, b: Dynamic, a: Dynamic, r1: Dynamic, r2: Dynamic, h: Dynamic| op_cone(c, b, a, r1, r2, h, None));
    e.register_fn("cone", |c: &mut ScriptCtx, b: Dynamic, a: Dynamic, r1: Dynamic, r2: Dynamic, h: Dynamic, o: Map| op_cone(c, b, a, r1, r2, h, Some(o)));
    e.register_fn("sphere", |c: &mut ScriptCtx, p: Dynamic, r: Dynamic| op_sphere(c, p, r, None));
    e.register_fn("sphere", |c: &mut ScriptCtx, p: Dynamic, r: Dynamic, o: Map| op_sphere(c, p, r, Some(o)));
    e.register_fn("torus", |c: &mut ScriptCtx, p: Dynamic, a: Dynamic, r1: Dynamic, r2: Dynamic| op_torus(c, p, a, r1, r2, None));
    e.register_fn("torus", |c: &mut ScriptCtx, p: Dynamic, a: Dynamic, r1: Dynamic, r2: Dynamic, o: Map| op_torus(c, p, a, r1, r2, Some(o)));

    // Profile-based.
    e.register_fn("extrude", |c: &mut ScriptCtx, p: Dynamic, d: Dynamic| op_extrude(c, p, d, None));
    e.register_fn("extrude", |c: &mut ScriptCtx, p: Dynamic, d: Dynamic, o: Map| op_extrude(c, p, d, Some(o)));
    e.register_fn("revolve", |c: &mut ScriptCtx, p: Dynamic, a: Dynamic, ang: Dynamic| op_revolve(c, p, a, ang, None));
    e.register_fn("revolve", |c: &mut ScriptCtx, p: Dynamic, a: Dynamic, ang: Dynamic, o: Map| op_revolve(c, p, a, ang, Some(o)));

    // Booleans.
    for (name, kind) in [("union", Bool::Union), ("subtract", Bool::Subtract), ("intersect", Bool::Intersect)] {
        e.register_fn(name, move |c: &mut ScriptCtx, t: Dynamic, u: Dynamic| op_boolean(c, kind, t, u, None));
        e.register_fn(name, move |c: &mut ScriptCtx, t: Dynamic, u: Dynamic, o: Map| op_boolean(c, kind, t, u, Some(o)));
    }

    // Detail.
    e.register_fn("fillet", |c: &mut ScriptCtx, edges: Dynamic, r: Dynamic| op_blend(c, edges, r, false));
    e.register_fn("chamfer", |c: &mut ScriptCtx, edges: Dynamic, d: Dynamic| op_blend(c, edges, d, true));

    // Transforms.
    e.register_fn("translate", |c: &mut ScriptCtx, b: Dynamic, v: Dynamic| transform_in_place(c, b, vec3(&v, "translation").map(k::Transform::Translate)));
    e.register_fn("rotate", |c: &mut ScriptCtx, b: Dynamic, o: Dynamic, a: Dynamic, ang: Dynamic| {
        let t = (|| -> R<k::Transform> { Ok(k::Transform::Rotate { origin: vec3(&o, "rotation origin")?, axis: vec3(&a, "rotation axis")?.normalize_or_zero(), angle: num(&ang, "angle")? }) })();
        transform_in_place(c, b, t)
    });
    fn linear(dir: &Dynamic, spacing: &Dynamic, count: i64) -> R<Vec<k::Transform>> {
        let d = vec3(dir, "pattern direction")?.normalize_or_zero();
        let s = num(spacing, "spacing")?;
        if d == DVec3::ZERO || count < 1 {
            return Err("linear_pattern needs a direction and count >= 1".into());
        }
        Ok((1..count).map(|i| k::Transform::Translate(d * s * i as f64)).collect())
    }
    fn circular(origin: &Dynamic, axis: &Dynamic, count: i64, total: &Dynamic) -> R<Vec<k::Transform>> {
        let (o, a) = (vec3(origin, "pattern origin")?, vec3(axis, "pattern axis")?.normalize_or_zero());
        let total = num(total, "angle")?;
        if a == DVec3::ZERO || count < 1 {
            return Err("circular_pattern needs an axis and count >= 1".into());
        }
        // A full turn spreads instances evenly; otherwise the angle spans first to last.
        let step = if (total.abs() - std::f64::consts::TAU).abs() < 1e-9 { total / count as f64 } else if count > 1 { total / (count - 1) as f64 } else { 0.0 };
        Ok((1..count).map(|i| k::Transform::Rotate { origin: o, axis: a, angle: step * i as f64 }).collect())
    }
    e.register_fn("linear_pattern", |c: &mut ScriptCtx, b: Dynamic, d: Dynamic, s: Dynamic, n: i64| -> R<Array> { copies(c, b, linear(&d, &s, n)?, None) });
    e.register_fn("linear_pattern", |c: &mut ScriptCtx, b: Dynamic, d: Dynamic, s: Dynamic, n: i64, o: Map| -> R<Array> { copies(c, b, linear(&d, &s, n)?, Some(o)) });
    e.register_fn("circular_pattern", |c: &mut ScriptCtx, b: Dynamic, o: Dynamic, a: Dynamic, n: i64, ang: Dynamic| -> R<Array> { copies(c, b, circular(&o, &a, n, &ang)?, None) });
    e.register_fn("circular_pattern", |c: &mut ScriptCtx, b: Dynamic, o: Dynamic, a: Dynamic, n: i64, ang: Dynamic, m: Map| -> R<Array> { copies(c, b, circular(&o, &a, n, &ang)?, Some(m)) });
    e.register_fn("mirror", |c: &mut ScriptCtx, b: Dynamic, plane: Dynamic| -> R<Array> {
        let frame = plane_of(c, &plane)?;
        copies(c, b, vec![k::Transform::Mirror(k::Plane::new(frame.origin, frame.normal, frame.x))], None)
    });
    e.register_fn("copy", |c: &mut ScriptCtx, b: Dynamic, v: Dynamic| -> R<Array> { copies(c, b, vec![k::Transform::Translate(vec3(&v, "translation")?)], None) });
    e.register_fn("delete", op_delete);

    // Resolution.
    e.register_fn("plane", |c: &mut ScriptCtx, x: Dynamic| -> R<Map> { Ok(frame_map(&plane_of(c, &x)?)) });
    e.register_fn("point", |c: &mut ScriptCtx, x: Dynamic| -> R<Dynamic> {
        let r = single(&x, "point")?;
        with(c, |s| {
            s.ev(|ev| {
                if let Some(p) = ev.resolve_vertex_point(&r) {
                    return Ok(vdyn(p));
                }
                match shapes_of(ev, std::slice::from_ref(&r)).first() {
                    Some((Reference::Face(_), f)) => Ok(vdyn(f.centroid()?)),
                    Some((_, sh)) => Ok(vdyn(k::volume_properties(sh).map(|m| m.center).or_else(|_| sh.bounding_box(false).map(|b| b.center()))?)),
                    None => err("Could not get a point from this entity"),
                }
            })
        })
    });
    e.register_fn("axis", |c: &mut ScriptCtx, x: Dynamic| -> R<Map> {
        with(c, |s| {
            s.ev(|ev| {
                let (o, d) = axis_of(ev, &x)?;
                let mut m = Map::new();
                m.insert("origin".into(), vdyn(o));
                m.insert("direction".into(), vdyn(d));
                Ok(m)
            })
        })
    });
    e.register_fn("direction", |c: &mut ScriptCtx, x: Dynamic| -> R<Dynamic> {
        let r = single(&x, "direction")?;
        with(c, |s| s.ev(|ev| ev.resolve_direction(&r).map(vdyn).ok_or_else(|| FeatureError::new("Could not get a direction from this entity"))))
    });

    // Measurement.
    e.register_fn("area", |c: &mut ScriptCtx, x: Dynamic| -> R<f64> {
        measure(c, x, |ev, r| {
            let mut a = 0.0;
            for (rr, s) in shapes_of(ev, r) {
                if matches!(rr, Reference::Face(_)) {
                    a += s.area()?;
                } else if matches!(rr, Reference::Body(_)) {
                    a += k::surface_properties(&s)?.mass;
                }
            }
            Ok(Dynamic::from_float(a))
        })
        .map(|d| d.as_float().unwrap_or(0.0))
    });
    e.register_fn("length", |c: &mut ScriptCtx, x: Dynamic| -> R<f64> {
        measure(c, x, |ev, r| {
            let mut l = 0.0;
            for (rr, s) in shapes_of(ev, r) {
                if matches!(rr, Reference::Edge(_)) {
                    l += s.length()?;
                }
            }
            Ok(Dynamic::from_float(l))
        })
        .map(|d| d.as_float().unwrap_or(0.0))
    });
    e.register_fn("volume", |c: &mut ScriptCtx, x: Dynamic| -> R<f64> {
        measure(c, x, |ev, r| {
            let mut v = 0.0;
            for (rr, s) in shapes_of(ev, r) {
                if matches!(rr, Reference::Body(_)) {
                    v += s.volume()?.abs();
                }
            }
            Ok(Dynamic::from_float(v))
        })
        .map(|d| d.as_float().unwrap_or(0.0))
    });
    e.register_fn("bbox", |c: &mut ScriptCtx, x: Dynamic| -> R<Map> {
        measure(c, x, |ev, r| {
            let (a, b) = bbox_of(ev, r)?;
            let mut m = Map::new();
            m.insert("min".into(), vdyn(a));
            m.insert("max".into(), vdyn(b));
            m.insert("size".into(), vdyn(b - a));
            m.insert("center".into(), vdyn((a + b) * 0.5));
            Ok(Dynamic::from_map(m))
        })
        .map(|d| d.cast::<Map>())
    });
    e.register_fn("normal", |c: &mut ScriptCtx, x: Dynamic| -> R<Dynamic> {
        measure(c, x, |ev, r| {
            let Some((_, f)) = shapes_of(ev, r).into_iter().find(|(rr, _)| matches!(rr, Reference::Face(_))) else { return err("normal: expected a face") };
            if let Some(p) = f.face_plane()? {
                return Ok(vdyn(p.unit_normal()));
            }
            let c = f.centroid()?;
            let proj = f.project_point(c)?;
            Ok(vdyn(f.normal_at_point(proj.point)?))
        })
    });

    // Queries over the current context.
    e.register_fn("bodies", |c: &mut ScriptCtx| -> R<Array> {
        with(c, |s| Ok(ents(s.ctx.bodies.iter().filter(|b| matches!(b.kind, BodyKind::Solid | BodyKind::Surface)).map(|b| Reference::Body(b.id.clone())))))
    });
    e.register_fn("parts", |c: &mut ScriptCtx| -> R<Array> { with(c, |s| Ok(ents(s.ctx.bodies.iter().filter(|b| b.kind == BodyKind::Solid).map(|b| Reference::Body(b.id.clone()))))) });
    e.register_fn("faces", |c: &mut ScriptCtx, x: Dynamic| query_fn(c, x, faces_of));
    e.register_fn("edges", |c: &mut ScriptCtx, x: Dynamic| query_fn(c, x, edges_of));
    e.register_fn("vertices", |c: &mut ScriptCtx, x: Dynamic| query_fn(c, x, vertices_of));
    e.register_fn("owner", |c: &mut ScriptCtx, x: Dynamic| -> R<Dynamic> {
        let r = single(&x, "owner")?;
        with(c, |s| {
            s.ev(|ev| {
                let bi = match &r {
                    Reference::Body(_) => ev.resolve_bodies(std::slice::from_ref(&r)).first().copied(),
                    Reference::Face(_) => ev.resolve_faces(std::slice::from_ref(&r)).first().map(|x| x.0),
                    Reference::Edge(_) => ev.resolve_edges(std::slice::from_ref(&r)).first().map(|x| x.0),
                    Reference::Vertex(n) => ev.ctx.bodies.iter().position(|b| !b.find_vertices(n).is_empty()),
                    _ => None,
                };
                Ok(bi.map(|i| Dynamic::from(Entity(Reference::Body(ev.ctx.bodies[i].id.clone())))).unwrap_or(Dynamic::UNIT))
            })
        })
    });
    e.register_fn("geometry", |c: &mut ScriptCtx, x: Dynamic| -> R<String> {
        let r = single(&x, "geometry")?;
        with(c, |s| s.ev(|ev| Ok(geometry_type(ev, &r))))
    });
    e.register_fn("filter", |c: &mut ScriptCtx, x: Dynamic, ty: &str| -> R<Array> {
        let refs = refs(&x)?;
        let want: Vec<String> = ty.split('|').map(|t| t.trim().to_uppercase()).collect();
        with(c, |s| {
            s.ev(|ev| {
                Ok(ents(refs.into_iter().filter(|r| {
                    let kind = Entity(r.clone()).kind().to_uppercase();
                    want.iter().any(|w| *w == kind || *w == geometry_type(ev, r))
                })))
            })
        })
    });

    // Diagnostics.
    e.register_fn("warn", |c: &mut ScriptCtx, msg: &str| {
        let mut s = c.0.lock();
        if !s.warnings.iter().any(|w| w == msg) {
            s.warnings.push(msg.to_string());
        }
    });
}

/// A plane from an entity or a `#{ origin, normal [, x] }` map.
fn plane_of(c: &mut ScriptCtx, x: &Dynamic) -> R<PlaneFrame> {
    let x = x.clone().flatten();
    if x.is_map() {
        let m = x.cast::<Map>();
        let o = m.get("origin").map(|v| vec3(v, "origin")).transpose()?.unwrap_or(DVec3::ZERO);
        let n = m.get("normal").map(|v| vec3(v, "normal")).transpose()?.ok_or("a plane map needs normal: [x, y, z]")?.normalize_or_zero();
        if n == DVec3::ZERO {
            return Err("plane normal must not be zero".into());
        }
        let p = k::Plane::from_normal(o, n);
        let xd = m.get("x").map(|v| vec3(v, "x")).transpose()?.map(|v| (v - n * v.dot(n)).normalize_or_zero()).filter(|v| *v != DVec3::ZERO).unwrap_or(p.unit_x());
        return Ok(PlaneFrame { origin: o, x: xd, y: n.cross(xd), normal: n });
    }
    let r = single(&x, "plane")?;
    with(c, |s| s.ev(|ev| ev.resolve_plane(&r).ok_or_else(|| FeatureError::new("Could not get a plane from this entity (use a planar face, plane or mate connector)"))))
}

pub(super) fn register(e: &mut Engine) {
    register_params(e);
    register_math(e);
    register_entity(e);
    register_ctx(e);
}

/// Script value of a query parameter.
pub(super) fn entities(r: &[Reference]) -> Dynamic {
    Dynamic::from_array(ents(r.iter().cloned()))
}
