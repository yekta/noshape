//! Looking at the model: parts, their faces, edges and vertices with stable
//! ids and geometry, datums, measurements and mass properties.

use super::{doc_field, s, strings, studio_target, tab_field};
use crate::error::OpError;
use crate::registry::{Args, Op, OpOutput, opt, req};
use crate::{Ctx, refs};
use glam::DVec3;
use noshape_kernel::{Curve, ShapeType, Surface};
use noshape_model::reference::{BodyId, Reference};
use noshape_model::regen::context::Body;
use noshape_units::{Dim, UnitPrefs};
use serde_json::{Value, json};

pub fn ops() -> Vec<Op> {
    vec![
        Op {
            group: "query",
            verb: "parts",
            summary: "The parts, surfaces and curves of a Part Studio",
            description: "Each body with its id (`body:…`), name, kind, volume, surface area, bounding box and face/edge/vertex counts. Lengths in the workspace units (see `units`).",
            fields: vec![doc_field(), tab_field()],
            mutates: false,
            destructive: false,
            run: parts,
        },
        Op {
            group: "query",
            verb: "entities",
            summary: "Faces, edges or vertices with stable ids and their geometry, to select for features",
            description: "Lists the faces / edges / vertices of the parts (or of `part`) with an id to pass to selection parameters, the feature that made them, and geometry: faces: surface type, area, centroid, normal (planar), axis and radius (cylinders, cones...); edges: curve type, length, endpoints, midpoint, center/radius (circles); vertices: position. Filters narrow the list: `surface`/`curve` type, `normal` (planar faces whose outward normal is within 1° of it), `direction` (line edges along it, either way, or circles around it), `created_by` (feature id or name), `near` (closest first, max `limit`). `at` picks several entities in one call: for each point, the closest entity that passes the filters (in the points' order). With `context: true`, the shown Assembly context's geometry instead (Modeling in context: assembly_edit_in_context); `part` is then an instance name.",
            fields: vec![
                doc_field(),
                tab_field(),
                req("kind", json!({ "type": "string", "enum": ["faces", "edges", "vertices"] }), "Which entities."),
                opt("part", s(), "Only this part (`body:…` id or part name)."),
                opt("surface", json!({ "type": "string", "enum": ["plane", "cylinder", "cone", "sphere", "torus", "bspline", "extruded", "revolved", "other"] }), "Faces: only this surface type."),
                opt("curve", json!({ "type": "string", "enum": ["line", "circle", "ellipse", "bspline", "other"] }), "Edges: only this curve type."),
                opt("normal", json!({ "type": "array", "items": { "type": "number" }, "minItems": 3, "maxItems": 3 }), "Faces: outward normal direction, e.g. [0,0,1] for faces facing +Z."),
                opt("created_by", s(), "Only entities made by this feature (id or name)."),
                opt("direction", json!({ "type": "array", "items": { "type": "number" }, "minItems": 3, "maxItems": 3 }), "Edges: lines parallel to this direction (either way) or circles whose axis it is, within 1°; e.g. [0,0,1] for vertical edges."),
                opt("near", json!({ "type": "array", "items": { "type": ["number", "string"] }, "minItems": 3, "maxItems": 3 }), "Sort by distance from this point (workspace units)."),
                opt("at", json!({ "type": "array", "items": { "type": "array", "items": { "type": ["number", "string"] }, "minItems": 3, "maxItems": 3 }, "minItems": 1 }), "Points (workspace units): the closest entity to each, in order (replaces `near` / `limit`)."),
                opt("limit", json!({ "type": "integer", "minimum": 1 }), "At most this many results (default 200)."),
                opt("context", json!({ "type": "boolean", "default": false }), "The shown Assembly context's instances instead of the parts."),
            ],
            mutates: false,
            destructive: false,
            run: entities,
        },
        Op {
            group: "query",
            verb: "datums",
            summary: "Planes, the origin, sketches and mate connectors of a Part Studio",
            description: "Default planes (`plane:top`, `plane:front`, `plane:right`), Plane features (`fplane:…`), sketches (`sketch:…`) with their plane frames, and mate connectors (`mc:…`).",
            fields: vec![doc_field(), tab_field()],
            mutates: false,
            destructive: false,
            run: datums,
        },
        Op {
            group: "query",
            verb: "measure",
            summary: "Measure entities (lengths, areas, radii, distances, angles)",
            description: "The Measure tool: one entity gives its length/area/radius...; two give the minimum distance (with dx, dy, dz) and the angle between them. Values in workspace units.",
            fields: vec![doc_field(), tab_field(), req("entities", strings(), "Reference ids.")],
            mutates: false,
            destructive: false,
            run: measure,
        },
        Op {
            group: "query",
            verb: "mass_properties",
            summary: "Mass properties: volume, surface area, mass, center of mass, inertia",
            description: "Of the given parts, or all parts. Mass, center of mass and inertia need a material (or density) on every part, as in Onshape; mass overrides apply. Inertia (kg·length²) is about the center of mass, or about mate connector `reference`'s axes.",
            fields: vec![doc_field(), tab_field(), opt("parts", strings(), "Part ids (`body:…`) or names; default all."), opt("reference", s(), "Mate connector id (`mc:…`) for the inertia axes.")],
            mutates: false,
            destructive: false,
            run: mass,
        },
    ]
}

/// Workspace length unit per meter.
pub(crate) fn len_scale(u: &UnitPrefs) -> f64 {
    1.0 / u.si_factor(Dim::LENGTH)
}

pub(crate) fn units_json(u: &UnitPrefs) -> Value {
    json!({ "length": u.length.to_string(), "angle": u.angle.to_string() })
}

fn r6(x: f64) -> f64 {
    (x * 1e6).round() / 1e6
}

pub(crate) fn pt(p: DVec3, k: f64) -> Value {
    json!([r6(p.x * k), r6(p.y * k), r6(p.z * k)])
}

fn dir(d: DVec3) -> Value {
    json!([r6(d.x), r6(d.y), r6(d.z)])
}

/// A part by id or by the name the parts list shows (a renamed part's
/// name included).
fn find_part<'a>(r: &'a noshape_model::regen::RegenResult, key: &str) -> Result<&'a Body, OpError> {
    let id = refs::parse_str(key);
    let shown = |b: &Body| r.display.bodies.iter().find(|d| d.id == b.id).map(|d| d.name.clone()).unwrap_or_else(|| b.name.clone());
    let bodies = &r.context.bodies;
    bodies
        .iter()
        .find(|b| matches!(&id, Some(Reference::Body(i)) if *i == b.id) || shown(b) == key || shown(b).eq_ignore_ascii_case(key))
        .map(|b| &**b)
        .ok_or_else(|| OpError::not_found(format!("no part `{key}`; parts: {}", bodies.iter().map(|b| format!("{} ({})", shown(b), refs::body_id(&b.id))).collect::<Vec<_>>().join(", "))).with_param("part"))
}

fn parts(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = studio_target(ctx, a)?;
    let units = od.doc().settings.units;
    let k = len_scale(&units);
    let r = od.regenerate(tab, None)?;
    let ps = crate::doc::studio(od.doc(), tab)?;
    let out: Vec<Value> = r
        .context
        .bodies
        .iter()
        .map(|b| {
            let d = r.display.body(&b.id);
            let mut v = json!({
                "id": refs::body_id(&b.id),
                "name": d.map(|d| d.name.clone()).unwrap_or_else(|| b.name.clone()),
                "kind": format!("{:?}", b.kind),
                "faces": b.shape.count(ShapeType::Face),
                "edges": b.shape.count(ShapeType::Edge),
                "vertices": b.shape.count(ShapeType::Vertex),
                "hidden": d.is_some_and(|d| d.hidden),
            });
            if let Ok(vol) = b.shape.volume()
                && matches!(b.kind, noshape_model::display::BodyKind::Solid)
            {
                v["volume"] = json!(r6(vol * k * k * k));
            }
            if let Ok(area) = b.shape.area() {
                v["surface_area"] = json!(r6(area * k * k));
            }
            if let Ok(bb) = b.shape.bounding_box(true) {
                v["bounding_box"] = json!({ "min": pt(bb.min, k), "max": pt(bb.max, k) });
            }
            if let Some(p) = ps.part_properties.get(&b.id) {
                if let Some(m) = &p.material {
                    v["material"] = json!(m);
                }
                if let Some(n) = &p.part_number {
                    v["part_number"] = json!(n);
                }
            }
            v
        })
        .collect();
    Ok(json!({ "units": units_json(&units), "parts": out }).into())
}

fn surface_name(s: &Surface) -> &'static str {
    match s {
        Surface::Plane(_) => "plane",
        Surface::Cylinder { .. } => "cylinder",
        Surface::Cone { .. } => "cone",
        Surface::Sphere { .. } => "sphere",
        Surface::Torus { .. } => "torus",
        Surface::BSpline { .. } | Surface::Bezier { .. } => "bspline",
        // Onshape's EXTRUDED and REVOLVED (a B-surface that is an exact
        // extrusion reports as one, as after Parasolid's import).
        Surface::Extrusion { .. } => "extruded",
        Surface::Revolution { .. } => "revolved",
        _ => "other",
    }
}

fn curve_name(c: &Curve) -> &'static str {
    match c {
        Curve::Line { .. } => "line",
        Curve::Circle { .. } => "circle",
        Curve::Ellipse { .. } => "ellipse",
        Curve::BSpline { .. } | Curve::Bezier { .. } => "bspline",
        _ => "other",
    }
}

fn face_json(b: &Body, i: usize, k: f64, names: &dyn Fn(Option<noshape_model::reference::FeatureId>) -> Option<String>) -> Option<(Value, DVec3, &'static str, Option<DVec3>)> {
    let f = b.shape.sub_shape(ShapeType::Face, i).ok()?;
    let name = b.faces.get(i)?;
    let mut v = json!({ "id": refs::to_id(&Reference::Face((**name).clone())), "part": refs::body_id(&b.id) });
    if let Some(n) = names(name.creator()) {
        v["created_by"] = json!(n);
    }
    let c = f.centroid().unwrap_or_default();
    v["centroid"] = pt(c, k);
    if let Ok(a) = f.area() {
        v["area"] = json!(r6(a * k * k));
    }
    let mut kind = "other";
    let mut normal = None;
    if let Ok(s) = f.face_surface() {
        kind = surface_name(&s);
        match s {
            Surface::Cylinder { origin, axis, radius, .. } => {
                v["axis"] = json!({ "origin": pt(origin, k), "direction": dir(axis) });
                v["radius"] = json!(r6(radius * k));
            }
            Surface::Cone { origin, axis, half_angle, .. } => {
                v["axis"] = json!({ "origin": pt(origin, k), "direction": dir(axis) });
                v["half_angle_deg"] = json!(r6(half_angle.to_degrees()));
            }
            Surface::Sphere { center, radius, .. } => {
                v["center"] = pt(center, k);
                v["radius"] = json!(r6(radius * k));
            }
            Surface::Torus { center, axis, major_radius, minor_radius } => {
                v["axis"] = json!({ "origin": pt(center, k), "direction": dir(axis) });
                v["major_radius"] = json!(r6(major_radius * k));
                v["minor_radius"] = json!(r6(minor_radius * k));
            }
            _ => {}
        }
    }
    if let Ok(Some(p)) = f.face_plane() {
        v["normal"] = dir(p.normal);
        normal = Some(p.normal);
    }
    v["surface"] = json!(kind);
    Some((v, c, kind, normal))
}

fn edge_json(b: &Body, i: usize, k: f64, names: &dyn Fn(Option<noshape_model::reference::FeatureId>) -> Option<String>) -> Option<(Value, DVec3, &'static str)> {
    let e = b.shape.sub_shape(ShapeType::Edge, i).ok()?;
    let name = b.edges.get(i)?;
    let mut v = json!({ "id": refs::to_id(&Reference::Edge((**name).clone())), "part": refs::body_id(&b.id) });
    if let Some(n) = names(name.creator()) {
        v["created_by"] = json!(n);
    }
    let info = e.edge_info().ok()?;
    let kind = curve_name(&info.curve);
    v["curve"] = json!(kind);
    v["length"] = json!(r6(info.length * k));
    v["start"] = pt(info.start, k);
    v["end"] = pt(info.end, k);
    let mid = noshape_kernel::linear_properties(&e).map(|p| p.center).unwrap_or((info.start + info.end) * 0.5);
    v["midpoint"] = pt(mid, k);
    match info.curve {
        Curve::Circle { center, axis, radius, .. } => {
            v["center"] = pt(center, k);
            v["axis"] = dir(axis);
            v["radius"] = json!(r6(radius * k));
        }
        Curve::Line { direction, .. } => v["direction"] = dir(direction),
        _ => {}
    }
    if info.closed {
        v["closed"] = json!(true);
    }
    Some((v, mid, kind))
}

fn entities(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = studio_target(ctx, a)?;
    let units = od.doc().settings.units;
    let k = len_scale(&units);
    let kind = a.str("kind")?;
    let r = od.regenerate(tab, None)?;
    let ps = crate::doc::studio(od.doc(), tab)?.clone();
    let names = |f: Option<noshape_model::reference::FeatureId>| f.and_then(|f| ps.feature(f)).map(|f| f.name.clone());
    let created_by = match a.opt_str("created_by")? {
        Some(c) => Some(crate::doc::find_feature(&ps, c).map_err(|e| e.with_param("created_by"))?),
        None => None,
    };
    let context_bodies: Vec<std::sync::Arc<Body>> = if a.bool_or("context", false)? {
        let c = ps.active_context.and_then(|id| ps.contexts.iter().find(|c| c.id == id)).ok_or_else(|| OpError::param("context", "no Assembly context is shown (assembly_edit_in_context, context_show)"))?;
        c.regen_bodies()
    } else {
        Vec::new()
    };
    let bodies: Vec<&Body> = match (a.opt_str("part")?, a.bool_or("context", false)?) {
        (Some(p), true) => vec![&**context_bodies.iter().find(|b| b.name == p).ok_or_else(|| OpError::param("part", &format!("the context's instances: {}", context_bodies.iter().map(|b| b.name.as_str()).collect::<Vec<_>>().join(", "))))?],
        (None, true) => context_bodies.iter().map(|b| &**b).collect(),
        (Some(p), false) => vec![find_part(&r, p)?],
        (None, false) => r.context.bodies.iter().map(|b| &**b).collect(),
    };
    let normal = match a.opt_array("normal")? {
        Some(n) => {
            let v: Vec<f64> = n.iter().filter_map(Value::as_f64).collect();
            if v.len() != 3 {
                return Err(OpError::param("normal", "three numbers"));
            }
            Some(DVec3::new(v[0], v[1], v[2]).normalize_or_zero())
        }
        None => None,
    };
    let near = match a.opt_array("near")? {
        Some(n) => {
            let v: Vec<f64> = n.iter().map(|x| crate::ops::sketch::length_value(x, &units).map(|m| m * k)).collect::<Result<_, _>>().map_err(|e| e.with_param("near"))?;
            if v.len() != 3 {
                return Err(OpError::param("near", "three coordinates"));
            }
            Some(DVec3::new(v[0], v[1], v[2]) / k)
        }
        None => None,
    };
    let direction = match a.opt_array("direction")? {
        Some(n) => {
            let v: Vec<f64> = n.iter().filter_map(Value::as_f64).collect();
            if v.len() != 3 || DVec3::new(v[0], v[1], v[2]).length() == 0.0 {
                return Err(OpError::param("direction", "three numbers, not all 0"));
            }
            Some(DVec3::new(v[0], v[1], v[2]).normalize())
        }
        None => None,
    };
    let at: Option<Vec<DVec3>> = match a.opt_array("at")? {
        Some(points) => Some(
            points
                .iter()
                .map(|p| {
                    let c = p.as_array().filter(|c| c.len() == 3).ok_or_else(|| OpError::param("at", "points of three coordinates"))?;
                    let v: Vec<f64> = c.iter().map(|x| crate::ops::sketch::length_value(x, &units).map(|m| m * k)).collect::<Result<_, _>>().map_err(|e| e.with_param("at"))?;
                    Ok(DVec3::new(v[0], v[1], v[2]) / k)
                })
                .collect::<Result<_, OpError>>()?,
        ),
        None => None,
    };
    let surface = a.opt_str("surface")?;
    let curve = a.opt_str("curve")?;
    let limit = a.opt_u64("limit")?.unwrap_or(200) as usize;
    // (distance from `near`, the entity's point, its JSON, its shape for `at`)
    let mut out: Vec<(f64, DVec3, Value, Option<noshape_kernel::Shape>)> = Vec::new();
    let shape_of = |b: &Body, t: ShapeType, i: usize| if at.is_some() { b.shape.sub_shape(t, i).ok() } else { None };
    for b in bodies {
        match kind {
            "faces" => {
                for i in 0..b.faces.len() {
                    if created_by.is_some_and(|f| b.faces[i].creator() != Some(f)) {
                        continue;
                    }
                    let Some((v, c, sk, n)) = face_json(b, i, k, &names) else { continue };
                    if surface.is_some_and(|s| s != sk) {
                        continue;
                    }
                    if let Some(want) = normal
                        && !n.is_some_and(|n| n.angle_between(want) < 1f64.to_radians())
                    {
                        continue;
                    }
                    out.push((near.map(|p| p.distance(c)).unwrap_or(0.0), c, v, shape_of(b, ShapeType::Face, i)));
                }
            }
            "edges" => {
                for i in 0..b.edges.len() {
                    if created_by.is_some_and(|f| b.edges[i].creator() != Some(f) && !edge_touches(&b.edges[i], f)) {
                        continue;
                    }
                    let Some((v, m, ck)) = edge_json(b, i, k, &names) else { continue };
                    if curve.is_some_and(|c| c != ck) {
                        continue;
                    }
                    if let Some(want) = direction {
                        let along = |x: &Value| x.as_array().map(|c| DVec3::new(c[0].as_f64().unwrap_or(0.0), c[1].as_f64().unwrap_or(0.0), c[2].as_f64().unwrap_or(0.0)));
                        let d = along(&v["direction"]).or_else(|| along(&v["axis"]));
                        if !d.is_some_and(|d| d.length() > 0.0 && d.angle_between(want).min(d.angle_between(-want)) < 1f64.to_radians()) {
                            continue;
                        }
                    }
                    out.push((near.map(|p| p.distance(m)).unwrap_or(0.0), m, v, shape_of(b, ShapeType::Edge, i)));
                }
            }
            "vertices" => {
                for i in 0..b.vertices.len() {
                    let Ok(vx) = b.shape.sub_shape(ShapeType::Vertex, i) else { continue };
                    let Ok(p) = vx.vertex_point() else { continue };
                    let v = json!({ "id": refs::to_id(&Reference::Vertex((*b.vertices[i]).clone())), "part": refs::body_id(&b.id), "position": pt(p, k) });
                    out.push((near.map(|q| q.distance(p)).unwrap_or(0.0), p, v, None));
                }
            }
            _ => return Err(OpError::param("kind", "faces, edges or vertices")),
        }
    }
    if let Some(points) = at {
        // The entity closest to each point; between entities it touches
        // alike (a point on an edge touches both faces), the one whose
        // centroid / midpoint is closer.
        let mut list = Vec::new();
        for p in &points {
            let probe = noshape_kernel::make_vertex(*p).map_err(|e| OpError::new("kernel", e.to_string()))?;
            let gap = |x: &(f64, DVec3, Value, Option<noshape_kernel::Shape>)| match &x.3 {
                Some(sh) => noshape_kernel::distance(sh, &probe).map(|d| d.distance).unwrap_or(f64::INFINITY),
                None => x.1.distance(*p),
            };
            let best = out.iter().map(|x| ((gap(x) / 1e-7).round(), x.1.distance(*p), x)).min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
            list.push(best.map(|(_, _, x)| x.2.clone()).unwrap_or(Value::Null));
        }
        return Ok(json!({ "units": units_json(&units), "count": list.len(), kind: list }).into());
    }
    if near.is_some() {
        out.sort_by(|x, y| x.0.total_cmp(&y.0));
    }
    let total = out.len();
    let list: Vec<Value> = out.into_iter().take(limit).map(|(_, _, v, _)| v).collect();
    Ok(json!({ "units": units_json(&units), "count": total, kind: list }).into())
}

/// An edge between faces one of which `f` created (edges of a feature).
fn edge_touches(n: &noshape_model::reference::TopoName, f: noshape_model::reference::FeatureId) -> bool {
    match n {
        noshape_model::reference::TopoName::Edge { faces, .. } => faces.iter().any(|x| x.creator() == Some(f)),
        _ => false,
    }
}

fn frame_json(fr: &noshape_model::display::PlaneFrame, k: f64) -> Value {
    json!({ "origin": pt(fr.origin, k), "x": dir(fr.x), "y": dir(fr.y), "normal": dir(fr.normal) })
}

fn datums(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = studio_target(ctx, a)?;
    let units = od.doc().settings.units;
    let k = len_scale(&units);
    let r = od.regenerate(tab, None)?;
    let planes: Vec<Value> = r.display.planes.iter().map(|p| json!({ "id": refs::to_id(&p.reference), "name": p.name, "frame": frame_json(&p.frame, k), "hidden": p.hidden })).collect();
    let sketches: Vec<Value> = r.display.sketches.iter().map(|s| json!({ "id": refs::to_id(&Reference::Sketch(s.feature)), "feature": s.feature.to_string(), "name": s.name, "frame": frame_json(&s.frame, k), "regions": s.regions.len(), "hidden": s.hidden })).collect();
    let connectors: Vec<Value> = r.display.connectors.iter().map(|c| json!({ "id": refs::to_id(&c.reference), "name": c.name, "origin": pt(c.origin, k), "x": dir(c.x), "z": dir(c.z) })).collect();
    Ok(json!({ "units": units_json(&units), "origin": "origin", "planes": planes, "sketches": sketches, "mate_connectors": connectors }).into())
}

fn parse_refs(a: &Args, name: &str) -> Result<Vec<Reference>, OpError> {
    a.array(name)?.iter().enumerate().map(|(i, v)| refs::parse(v).map_err(|e| e.with_param(format!("{name}[{i}]")))).collect()
}

fn measure(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    use noshape_model::regen::measure::{Measurement, measure};
    let (od, tab) = studio_target(ctx, a)?;
    let units = od.doc().settings.units;
    let k = len_scale(&units);
    let sel = parse_refs(a, "entities")?;
    let r = od.regenerate(tab, None)?;
    let out: Vec<Value> = measure(&r.context, &sel)
        .into_iter()
        .map(|(label, m)| {
            let v = match m {
                Measurement::Length(x) => json!({ "length": r6(x * k) }),
                Measurement::Area(x) => json!({ "area": r6(x * k * k) }),
                Measurement::Radius(x) => json!({ "radius": r6(x * k) }),
                Measurement::Diameter(x) => json!({ "diameter": r6(x * k) }),
                Measurement::Distance { distance, a, b, dx, dy, dz } => json!({ "distance": r6(distance * k), "from": pt(a, k), "to": pt(b, k), "dx": r6(dx * k), "dy": r6(dy * k), "dz": r6(dz * k) }),
                Measurement::Angle(x) => json!({ "angle_deg": r6(x.to_degrees()) }),
                Measurement::Percent(x) => json!({ "percent": r6(x) }),
                Measurement::Point(p) => json!({ "point": pt(p, k) }),
                Measurement::Volume(x) => json!({ "volume": r6(x * k * k * k) }),
            };
            json!({ "label": label, "value": v })
        })
        .collect();
    Ok(json!({ "units": units_json(&units), "measurements": out }).into())
}

fn mass(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = studio_target(ctx, a)?;
    let units = od.doc().settings.units;
    let k = len_scale(&units);
    let r = od.regenerate(tab, None)?;
    let ps = crate::doc::studio(od.doc(), tab)?;
    let bodies: Vec<BodyId> = match a.opt_array("parts")? {
        Some(list) => list.iter().enumerate().map(|(i, v)| v.as_str().ok_or_else(|| OpError::param(&format!("parts[{i}]"), "part id or name")).and_then(|s| find_part(&r, s).map(|b| b.id.clone()).map_err(|e| e.with_param(format!("parts[{i}]"))))).collect::<Result<_, _>>()?,
        None => Vec::new(),
    };
    let reference = match a.opt_str("reference")? {
        Some(id) => {
            let rf = refs::parse_str(id).ok_or_else(|| OpError::param("reference", "not a reference id"))?;
            Some(r.display.connectors.iter().find(|c| c.reference == rf).ok_or_else(|| OpError::param("reference", "not a mate connector of this Part Studio"))?.clone())
        }
        None => None,
    };
    let mp = crate::analysis::studio_mass_properties(&r.context, &r.display, ps, &bodies, reference.as_ref()).ok_or_else(|| OpError::invalid("nothing to measure: no solid parts"))?;
    let mut v = json!({
        "units": units_json(&units),
        "volume": r6(mp.props.volume * k * k * k),
        "surface_area": r6(mp.props.surface_area * k * k),
    });
    if let Some(m) = mp.props.mass {
        let c = mp.props.center_of_mass;
        let i = mp.props.inertia;
        let f = k * k;
        v["mass_kg"] = json!(r6(m));
        v["center_of_mass"] = pt(c, k);
        v["inertia"] = json!([[r6(i.x_axis.x * f), r6(i.y_axis.x * f), r6(i.z_axis.x * f)], [r6(i.x_axis.y * f), r6(i.y_axis.y * f), r6(i.z_axis.y * f)], [r6(i.x_axis.z * f), r6(i.y_axis.z * f), r6(i.z_axis.z * f)]]);
        v["inertia_unit"] = json!(format!("kg·{}²", units.length));
    } else {
        v["note"] = json!("Assign a material (or density) to every part to get mass, center of mass and inertia.");
    }
    if let Some(n) = mp.reference {
        v["reference"] = json!(n);
    }
    Ok(v.into())
}
