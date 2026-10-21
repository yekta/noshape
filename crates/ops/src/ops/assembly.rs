//! Assemblies: inserting parts, mates, moving and fixing instances.

use super::{doc_field, s, strings};
use crate::error::OpError;
use crate::ops::query::{len_scale, pt, units_json};
use crate::registry::{Args, Op, OpOutput, opt, req};
use crate::{Ctx, OpenDoc, assembly as sa, doc, refs};
use glam::{DMat4, DQuat, DVec3};
use noshape_kernel::{Curve, ShapeType, Surface};
use noshape_model::assembly::eval::{AssemblyResult, SolveOptions, inference_candidates};
use noshape_model::assembly::{Assembly, InstanceId, InstanceSource, Mate, MateConnectorRef, MateId, MateType, Placement};
use noshape_model::configuration::ConfigValues;
use noshape_model::document::{TabContent, TabId};
use noshape_model::reference::Reference;
use serde_json::{Value, json};

fn asm_field() -> crate::registry::Field {
    opt("tab", s(), "Assembly tab, by name or id. Default: the first Assembly.")
}

mod features;
mod more;
mod positions;

const CONNECTOR_DOC: &str = "A mate connector: {\"instance\": instance id or name, \"entity\": a face/edge/vertex id of that instance (from assembly_entities), \"inference\": where on it (center, centroid, midpoint, vertex, top_axis, mid_axis, bottom_axis, origin; default: the entity's first implicit point, its centroid or center)}, or a candidate from assembly_connectors {\"instance\", \"reference\", \"align\"}, or {\"origin\": true} for the assembly origin, or {\"connector\": an assembly mate connector id or name}. Optional \"flip\": true flips that connector's primary axis.";

pub fn ops() -> Vec<Op> {
    let connector = json!({ "type": "object" });
    vec![
        Op {
            group: "assembly",
            verb: "insert",
            summary: "Insert a part (or a whole Part Studio, or a subassembly) into an Assembly",
            description: "The Insert dialog. Inserting the same item again offsets the new instance along X so they don't overlap. Returns the instance id; place it with mates or assembly_transform.",
            fields: vec![
                doc_field(),
                asm_field(),
                req("source", s(), "Part Studio or Assembly tab (name or id) to insert from."),
                opt("part", s(), "A part of that Part Studio (`body:…` id or part name); default: the whole Part Studio as one instance."),
                opt("configuration", json!({ "type": "object" }), "Configuration of the source: {input id: value}."),
            ],
            mutates: true,
            destructive: false,
            run: insert,
        },
        Op {
            group: "assembly",
            verb: "list",
            summary: "An assembly's instances, mates and degrees of freedom",
            description: "Solves the assembly and lists the instances (id, name, source, placement: translation in workspace units and rotation quaternion xyzw, fixed/hidden/suppressed, remaining DOF) and the mates (id, name, type, ok, error message).",
            fields: vec![doc_field(), asm_field()],
            mutates: false,
            destructive: false,
            run: list,
        },
        Op {
            group: "assembly",
            verb: "entities",
            summary: "An instance's faces, edges or vertices, in assembly (world) coordinates, with ids for mates",
            description: "Like query_entities for one instance: ids, surface/curve type, centroid/midpoint, normal, axis and radius, all placed where the instance is in the assembly.",
            fields: vec![
                doc_field(),
                asm_field(),
                req("instance", s(), "Instance id or name."),
                req("kind", json!({ "type": "string", "enum": ["faces", "edges", "vertices"] }), "Which entities."),
                opt("surface", json!({ "type": "string" }), "Faces: only this surface type (plane, cylinder, ...)."),
                opt("normal", json!({ "type": "array", "items": { "type": "number" } }), "Faces: outward normal in assembly coordinates."),
                opt("limit", json!({ "type": "integer" }), "At most this many (default 200)."),
            ],
            mutates: false,
            destructive: false,
            run: entities,
        },
        Op {
            group: "assembly",
            verb: "connectors",
            summary: "The implicit mate connector points on an instance's face, edge or vertex",
            description: "The points Onshape offers while hovering an entity (centroid, vertices, edge midpoints, circle centers, cylinder axis ends...), each with its world position and Z axis, ready to pass as a mate connector.",
            fields: vec![doc_field(), asm_field(), req("instance", s(), "Instance id or name."), req("entity", s(), "Face, edge or vertex id from assembly_entities.")],
            mutates: false,
            destructive: false,
            run: connectors,
        },
        Op {
            group: "assembly",
            verb: "mate",
            summary: "Add a mate between two mate connectors (Fastened, Revolute, Slider, Planar, Cylindrical, Pin slot, Ball, Parallel, Tangent, Width)",
            description: Box::leak(format!("The mate dialog: picks two connectors, solves the mate (the first connector's instance moves to the second, as in Onshape), and commits. {CONNECTOR_DOC} Offsets are expressions (\"5 mm\", \"90 deg\").").into_boxed_str()),
            fields: vec![
                doc_field(),
                asm_field(),
                req("type", json!({ "type": "string", "enum": MateType::ALL.iter().map(|t| mate_type_id(*t)).collect::<Vec<_>>() }), "Mate type."),
                req("a", connector.clone(), "First connector (its instance moves)."),
                req("b", connector, "Second connector."),
                opt("name", s(), "Name (default \"Fastened 1\", ...)."),
                opt("flip_primary", json!({ "type": "boolean" }), "Flip the primary axis (default: keep the moving instance's current orientation, like the dialog)."),
                opt("offset", json!({ "type": "object" }), "{\"x\", \"y\", \"z\": length, \"rotation\": angle} expressions where the mate type allows them."),
                opt("solve_all", json!({ "type": "boolean" }), "Solve every mate (the dialog's Solve) instead of only moving this mate's first instance."),
            ],
            mutates: true,
            destructive: false,
            run: mate,
        },
        Op {
            group: "assembly",
            verb: "transform",
            summary: "Move an instance to a placement",
            description: "Sets the instance's translation (workspace units) and optional rotation (axis + angle) as when dragging it or using Transform; mates then pull mated instances.",
            fields: vec![
                doc_field(),
                asm_field(),
                req("instance", s(), "Instance id or name."),
                req("translation", json!({ "type": "array", "items": { "type": ["number", "string"] }, "minItems": 3, "maxItems": 3 }), "[x, y, z]."),
                opt("axis", json!({ "type": "array", "items": { "type": "number" }, "minItems": 3, "maxItems": 3 }), "Rotation axis."),
                opt("angle", json!({ "type": ["number", "string"] }), "Rotation angle (\"90 deg\")."),
            ],
            mutates: true,
            destructive: false,
            run: transform,
        },
        Op {
            group: "assembly",
            verb: "set_state",
            summary: "Fix/unfix, hide/show or suppress/unsuppress instances",
            description: "",
            fields: vec![
                doc_field(),
                asm_field(),
                req("instances", strings(), "Instance ids or names."),
                opt("fixed", json!({ "type": "boolean" }), "Fix or unfix."),
                opt("hidden", json!({ "type": "boolean" }), "Hide or show."),
                opt("suppressed", json!({ "type": "boolean" }), "Suppress or unsuppress."),
            ],
            mutates: true,
            destructive: false,
            run: set_state,
        },
        Op {
            group: "assembly",
            verb: "delete",
            summary: "Delete instances, mates or other assembly features",
            description: "",
            fields: vec![doc_field(), asm_field(), opt("instances", strings(), "Instance ids or names."), opt("mates", strings(), "Mate, relation, group, mate connector or pattern ids or names (a pattern takes its instances with it).")],
            mutates: true,
            destructive: true,
            run: delete,
        },
    ]
    .into_iter()
    .chain(more::ops())
    .chain(features::ops())
    .chain(positions::ops())
    .collect()
}

fn mate_type_id(t: MateType) -> String {
    format!("{t:?}").to_lowercase()
}

fn is_assembly(c: &TabContent) -> bool {
    matches!(c, TabContent::Assembly(_))
}

fn target<'a>(ctx: &'a mut Ctx, a: &Args) -> Result<(&'a mut OpenDoc, TabId), OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let tab = doc::tab_of_kind(od.doc(), a.opt_str("tab")?, "Assembly", is_assembly)?;
    Ok((od, tab))
}

fn assembly(od: &OpenDoc, tab: TabId) -> Result<&Assembly, OpError> {
    match od.doc().tab(tab).map(|t| &t.content) {
        Some(TabContent::Assembly(a)) => Ok(a),
        _ => Err(OpError::param("tab", "not an Assembly")),
    }
}

/// Solve the assembly in its default configuration.
fn solve(od: &mut OpenDoc, tab: TabId, opts: &SolveOptions) -> Result<AssemblyResult, OpError> {
    let asm = sa::effective(assembly(od, tab)?, &assembly(od, tab)?.configuration.defaults());
    Ok(od.evaluate_assembly(tab, &asm, opts))
}

/// Commit an assembly edit the way the view does: fold in the latest
/// solved placements (`result`), then apply `f`.
fn commit<R>(od: &mut OpenDoc, tab: TabId, desc: &str, result: &AssemblyResult, f: impl FnOnce(&mut Assembly) -> R) -> Result<R, OpError> {
    let r = od.session.edit_tab(tab, desc, |c| match c {
        TabContent::Assembly(a) => {
            sa::fold_solved(a, result);
            Some(f(a))
        }
        _ => None,
    })?;
    r.flatten().ok_or_else(|| OpError::param("tab", "not an Assembly"))
}

fn find_instance(a: &Assembly, key: &str, field: &str) -> Result<InstanceId, OpError> {
    let k = key.trim();
    let by_id = k.strip_prefix("inst:").unwrap_or(k).parse::<u64>().ok().map(InstanceId);
    a.instances
        .iter()
        .find(|i| Some(i.id) == by_id)
        .or_else(|| a.instances.iter().find(|i| i.name == k || i.name.eq_ignore_ascii_case(k)))
        .map(|i| i.id)
        .ok_or_else(|| OpError::not_found(format!("no instance `{key}`; instances: {}", a.instances.iter().map(|i| format!("{} (inst:{})", i.name, i.id.0)).collect::<Vec<_>>().join(", "))).with_param(field))
}

fn find_mate(a: &Assembly, key: &str) -> Result<MateId, OpError> {
    let k = key.trim();
    let by_id = k.strip_prefix("mate:").unwrap_or(k).parse::<u64>().ok().map(MateId);
    a.mates
        .iter()
        .find(|m| Some(m.id) == by_id || m.name == k)
        .map(|m| m.id)
        .or_else(|| a.relations.iter().find(|r| Some(r.id) == by_id || r.name == k).map(|r| r.id))
        .or_else(|| more::find_feature(a, k))
        .ok_or_else(|| OpError::not_found(format!("no mate `{key}`")).with_param("mates"))
}

fn source_json(s: &InstanceSource, d: &noshape_model::Document) -> Value {
    let tab = |t: &TabId| d.tab(*t).map(|x| x.name.clone()).unwrap_or_else(|| t.to_string());
    match s {
        InstanceSource::Part { studio, body, .. } => json!({ "part_studio": tab(studio), "part": refs::body_id(body) }),
        InstanceSource::PartStudio { studio, .. } => json!({ "part_studio": tab(studio) }),
        InstanceSource::Assembly { assembly, .. } => json!({ "assembly": tab(assembly) }),
        other => json!({ "other": format!("{other:?}").chars().take(80).collect::<String>() }),
    }
}

fn describe(od: &mut OpenDoc, tab: TabId) -> Result<Value, OpError> {
    let units = od.doc().settings.units;
    let k = len_scale(&units);
    let r = solve(od, tab, &SolveOptions::default())?;
    let a = assembly(od, tab)?;
    let instances: Vec<Value> = a
        .instances
        .iter()
        .map(|i| {
            let p = r.solved.placement(i.id).unwrap_or(i.placement);
            json!({
                "id": format!("inst:{}", i.id.0),
                "name": i.name,
                "source": source_json(&i.source, od.doc()),
                "translation": pt(DVec3::from(p.translation), k),
                "rotation": p.rotation,
                "fixed": i.fixed,
                "hidden": i.hidden,
                "suppressed": i.suppressed,
                "dof": r.solved.instance_dof.get(&i.id),
            })
        })
        .collect();
    let mates: Vec<Value> = a
        .mates
        .iter()
        .map(|m| {
            let st = r.mates.iter().find(|s| s.id == m.id);
            json!({ "id": format!("mate:{}", m.id.0), "name": m.name, "type": mate_type_id(m.mate_type), "a": format!("inst:{}", m.a.instance.0), "b": format!("inst:{}", m.b.instance.0), "ok": st.map(|s| s.ok), "message": st.and_then(|s| s.message.clone()), "suppressed": m.suppressed })
        })
        .collect();
    let (named_positions, exploded_views) = positions::json(a);
    Ok(json!({ "units": units_json(&units), "instances": instances, "mates": mates, "patterns": more::patterns_json(a), "named_positions": named_positions, "exploded_views": exploded_views, "dof": r.solved.dof, "converged": r.solved.converged }))
}

fn insert(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = target(ctx, a)?;
    let src = doc::find_tab(od.doc(), a.str("source")?).map_err(|e| e.with_param("source"))?;
    let config = ConfigValues(a.object("configuration")?.map(|m| m.iter().map(|(k, v)| (k.clone(), v.as_str().map(str::to_string).unwrap_or_else(|| v.to_string()))).collect()).unwrap_or_default());
    let src_name = od.doc().tab(src).map(|t| t.name.clone()).unwrap_or_default();
    let result = solve(od, tab, &SolveOptions::default())?;
    let id = match &od.doc().tab(src).map(|t| &t.content) {
        Some(TabContent::PartStudio(_)) => {
            let part = match a.opt_str("part")? {
                Some(p) => {
                    let r = od.regenerate(src, Some(&config))?;
                    let b = r.display.bodies.iter().find(|b| matches!(refs::parse_str(p), Some(Reference::Body(id)) if id == b.id) || b.name == p).ok_or_else(|| OpError::param("part", "no such part in the source Part Studio (see query_parts)"))?;
                    Some((b.id.clone(), b.name.clone(), b.bounds))
                }
                None => None,
            };
            let desc = sa::insert_description(&src_name, part.as_ref());
            commit(od, tab, &desc, &result, |asm| sa::insert_part(asm, src, &src_name, part, config))?
        }
        Some(TabContent::Assembly(_)) if src != tab => commit(od, tab, &format!("Insert : {src_name}"), &result, |asm| {
            let id = InstanceId(asm.next_id());
            let name = asm.next_instance_name(&src_name);
            asm.instances.push(noshape_model::assembly::Instance::new(id, name, InstanceSource::Assembly { assembly: src, config }, Placement::default()));
            id
        })?,
        _ => return Err(OpError::param("source", "a Part Studio or another Assembly")),
    };
    let mut v = describe(od, tab)?;
    v["instance"] = json!(format!("inst:{}", id.0));
    Ok(v.into())
}

fn list(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = target(ctx, a)?;
    Ok(describe(od, tab)?.into())
}

fn occurrence(r: &AssemblyResult, inst: InstanceId) -> Option<&noshape_model::assembly::eval::PartOccurrence> {
    r.occurrences.iter().find(|o| o.path.first() == Some(&inst))
}

fn entities(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = target(ctx, a)?;
    let units = od.doc().settings.units;
    let k = len_scale(&units);
    let inst = find_instance(assembly(od, tab)?, a.str("instance")?, "instance")?;
    let r = solve(od, tab, &SolveOptions::default())?;
    let kind = a.str("kind")?;
    let surface = a.opt_str("surface")?;
    let normal = a.opt_array("normal")?.map(|n| DVec3::new(n.first().and_then(Value::as_f64).unwrap_or(0.0), n.get(1).and_then(Value::as_f64).unwrap_or(0.0), n.get(2).and_then(Value::as_f64).unwrap_or(0.0)).normalize_or_zero());
    let limit = a.opt_u64("limit")?.unwrap_or(200) as usize;
    let mut out = Vec::new();
    for o in r.occurrences.iter().filter(|o| o.path.first() == Some(&inst)) {
        let Some(body) = o.context.body(&o.body) else { continue };
        let m: DMat4 = o.transform;
        match kind {
            "faces" => {
                for (i, name) in body.faces.iter().enumerate() {
                    let Ok(f) = body.shape.sub_shape(ShapeType::Face, i) else { continue };
                    let st = f.face_surface().ok();
                    let sname = match &st {
                        Some(Surface::Plane(_)) => "plane",
                        Some(Surface::Cylinder { .. }) => "cylinder",
                        Some(Surface::Cone { .. }) => "cone",
                        Some(Surface::Sphere { .. }) => "sphere",
                        Some(Surface::Torus { .. }) => "torus",
                        _ => "other",
                    };
                    if surface.is_some_and(|s| s != sname) {
                        continue;
                    }
                    let n = f.face_plane().ok().flatten().map(|p| m.transform_vector3(p.normal).normalize());
                    if let Some(want) = normal
                        && !n.is_some_and(|n| n.angle_between(want) < 1f64.to_radians())
                    {
                        continue;
                    }
                    let mut v = json!({ "id": refs::to_id(&Reference::Face((**name).clone())), "surface": sname, "centroid": pt(m.transform_point3(f.centroid().unwrap_or_default()), k) });
                    if let Some(n) = n {
                        v["normal"] = json!([n.x, n.y, n.z]);
                    }
                    if let Some(Surface::Cylinder { origin, axis, radius, .. }) = st {
                        let d = m.transform_vector3(axis);
                        v["axis"] = json!({ "origin": pt(m.transform_point3(origin), k), "direction": [d.x, d.y, d.z] });
                        v["radius"] = json!(radius * k);
                    }
                    if let Ok(ar) = f.area() {
                        v["area"] = json!(ar * k * k);
                    }
                    out.push(v);
                }
            }
            "edges" => {
                for (i, name) in body.edges.iter().enumerate() {
                    let Ok(e) = body.shape.sub_shape(ShapeType::Edge, i) else { continue };
                    let Ok(info) = e.edge_info() else { continue };
                    let mid = noshape_kernel::linear_properties(&e).map(|p| p.center).unwrap_or((info.start + info.end) * 0.5);
                    let mut v = json!({ "id": refs::to_id(&Reference::Edge((**name).clone())), "length": info.length * k, "midpoint": pt(m.transform_point3(mid), k) });
                    match info.curve {
                        Curve::Circle { center, radius, .. } => {
                            v["curve"] = json!("circle");
                            v["center"] = pt(m.transform_point3(center), k);
                            v["radius"] = json!(radius * k);
                        }
                        Curve::Line { direction, .. } => {
                            v["curve"] = json!("line");
                            let d = m.transform_vector3(direction);
                            v["direction"] = json!([d.x, d.y, d.z]);
                        }
                        _ => v["curve"] = json!("other"),
                    }
                    out.push(v);
                }
            }
            "vertices" => {
                for (i, name) in body.vertices.iter().enumerate() {
                    let Ok(p) = body.shape.sub_shape(ShapeType::Vertex, i).and_then(|x| x.vertex_point()) else { continue };
                    out.push(json!({ "id": refs::to_id(&Reference::Vertex((**name).clone())), "position": pt(m.transform_point3(p), k) }));
                }
            }
            _ => return Err(OpError::param("kind", "faces, edges or vertices")),
        }
    }
    if out.is_empty() && occurrence(&r, inst).is_none() {
        return Err(OpError::invalid("the instance has no parts (suppressed or failed?)").with_param("instance"));
    }
    let n = out.len();
    out.truncate(limit);
    Ok(json!({ "units": units_json(&units), "instance": format!("inst:{}", inst.0), "count": n, kind: out }).into())
}

fn connectors(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = target(ctx, a)?;
    let units = od.doc().settings.units;
    let k = len_scale(&units);
    let inst = find_instance(assembly(od, tab)?, a.str("instance")?, "instance")?;
    let on = refs::parse(a.get("entity").expect("required")).map_err(|e| e.with_param("entity"))?;
    let r = solve(od, tab, &SolveOptions::default())?;
    let o = occurrence(&r, inst).ok_or_else(|| OpError::param("instance", "the instance has no parts"))?;
    let cands = inference_candidates(&o.context, Some(&o.body), &on, &units);
    let out: Vec<Value> = cands
        .iter()
        .map(|c| {
            let mc = c.connector(&o.path);
            let mut v = json!({ "instance": format!("inst:{}", inst.0), "reference": refs::to_id(&c.reference) });
            if let Some(al) = &c.align {
                v["align"] = json!(refs::to_id(al));
            }
            if let Some(f) = r.connector_world(&mc, &units) {
                v["position"] = pt(f.origin, k);
                let z = f.z();
                v["z"] = json!([z.x, z.y, z.z]);
            }
            v
        })
        .collect();
    Ok(json!({ "units": units_json(&units), "connectors": out }).into())
}

/// A mate connector argument.
fn parse_connector(a: &Assembly, od: &OpenDoc, tab: TabId, v: &Value, field: &str) -> Result<MateConnectorRef, OpError> {
    let _ = (od, tab);
    let o = v.as_object().ok_or_else(|| OpError::param(field, CONNECTOR_DOC))?;
    let mut c = if o.get("origin").and_then(Value::as_bool) == Some(true) {
        MateConnectorRef::origin()
    } else if let Some(k) = o.get("connector").and_then(Value::as_str) {
        // An assembly mate connector (assembly_mate_connector).
        let mc = a.connectors.iter().find(|c| c.name == k || format!("mc:{}", c.id.0) == k || c.id.0.to_string() == k).ok_or_else(|| OpError::not_found(format!("no assembly mate connector `{k}`")).with_param(format!("{field}.connector")))?;
        MateConnectorRef::assembly_connector(mc.id, mc.owner())
    } else {
        let inst = find_instance(a, o.get("instance").and_then(Value::as_str).ok_or_else(|| OpError::param(&format!("{field}.instance"), "instance id or name"))?, &format!("{field}.instance"))?;
        let reference = match (o.get("reference"), o.get("entity")) {
            (Some(r), _) => refs::parse(r).map_err(|e| e.with_param(format!("{field}.reference")))?,
            (None, Some(e)) => {
                let on = refs::parse(e).map_err(|e| e.with_param(format!("{field}.entity")))?;
                match o.get("inference").and_then(Value::as_str) {
                    Some(name) => {
                        let inf = refs::INFERENCES.iter().find(|i| refs::to_id(&Reference::Implicit { on: Box::new(Reference::Origin), inference: **i }).split(':').nth(1) == Some(name)).copied().ok_or_else(|| OpError::param(&format!("{field}.inference"), "center, centroid, midpoint, vertex, top_axis, mid_axis, bottom_axis or origin"))?;
                        Reference::Implicit { on: Box::new(on), inference: inf }
                    }
                    None => on,
                }
            }
            _ => return Err(OpError::param(field, "give `entity` (with optional `inference`) or `reference`")),
        };
        let mut c = MateConnectorRef::on(inst, Vec::new(), reference);
        if let Some(al) = o.get("align") {
            c.align = Some(Box::new(refs::parse(al).map_err(|e| e.with_param(format!("{field}.align")))?));
        }
        c
    };
    if o.get("flip").and_then(Value::as_bool) == Some(true) {
        c.flip = true;
    }
    Ok(c)
}

/// A bare entity on an instance stands for its first implicit point (the
/// one the dialog highlights first), unless it is already a connector.
fn resolve_entity(r: &AssemblyResult, c: MateConnectorRef, units: &noshape_units::UnitPrefs) -> MateConnectorRef {
    if c.connector.is_some() || c.instance == InstanceId::ORIGIN || matches!(c.reference, Reference::Implicit { .. } | Reference::MateConnector { .. }) {
        return c;
    }
    let Some(o) = occurrence(r, c.instance) else { return c };
    match inference_candidates(&o.context, Some(&o.body), &c.reference, units).first() {
        Some(p) => {
            let mut n = p.connector(&o.path);
            n.flip = c.flip;
            n
        }
        None => c,
    }
}

fn expr(o: &serde_json::Map<String, Value>, key: &str) -> Option<noshape_units::ExprValue> {
    match o.get(key)? {
        Value::Number(n) => Some(noshape_units::ExprValue::new(n.to_string())),
        Value::String(s) => Some(noshape_units::ExprValue::new(s.clone())),
        _ => None,
    }
}

fn mate(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = target(ctx, a)?;
    let units = od.doc().settings.units;
    let ty = a.str("type")?;
    let mate_type = MateType::ALL.into_iter().find(|t| mate_type_id(*t) == ty.to_lowercase().replace([' ', '_'], "")).ok_or_else(|| OpError::param("type", "unknown mate type"))?;
    let asm = assembly(od, tab)?.clone();
    let before = solve(od, tab, &SolveOptions::default())?;
    let ca = resolve_entity(&before, parse_connector(&asm, od, tab, a.get("a").expect("required"), "a")?, &units);
    let cb = resolve_entity(&before, parse_connector(&asm, od, tab, a.get("b").expect("required"), "b")?, &units);
    if ca.instance == cb.instance && ca.instance != InstanceId::ORIGIN {
        return Err(OpError::param("b", "both connectors are on the same instance"));
    }
    let id = MateId(asm.next_id());
    let name = match a.opt_str("name")? {
        Some(n) => n.trim().to_string(),
        None => asm.next_feature_name(mate_type.name()),
    };
    let mut m = Mate::new(id, name.clone(), mate_type, ca.clone(), cb.clone());
    m.flip_primary = match a.get("flip_primary").and_then(Value::as_bool) {
        Some(f) => f,
        None => sa::default_flip_primary(&before, &ca, &cb, &units).unwrap_or(false),
    };
    if let Some(off) = a.object("offset")? {
        m.offset_x = expr(off, "x");
        m.offset_y = expr(off, "y");
        m.offset_z = expr(off, "z");
        m.rotation = expr(off, "rotation");
    }
    // Preview solve with the new mate, as the dialog does, then commit with
    // those placements folded in.
    let mut preview = sa::effective(&asm, &asm.configuration.defaults());
    preview.mates.push(m.clone());
    let opts = if a.bool_or("solve_all", false)? { SolveOptions::default() } else { sa::mate_preview_options(&preview, &m) };
    let solved = od.evaluate_assembly(tab, &preview, &opts);
    let ok = solved.mates.iter().find(|s| s.id == id).map(|s| (s.ok, s.message.clone()));
    let values = asm.configuration.defaults();
    commit(od, tab, &sa::mate_description(&m, false), &solved, |a| sa::commit_mate(a, m, None, &values))?;
    let mut v = describe(od, tab)?;
    v["mate"] = json!(format!("mate:{}", id.0));
    v["name"] = json!(name);
    if let Some((ok, msg)) = ok {
        v["mate_ok"] = json!(ok);
        if let Some(msg) = msg {
            v["mate_message"] = json!(msg);
        }
    }
    Ok(v.into())
}

fn transform(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = target(ctx, a)?;
    let units = od.doc().settings.units;
    let asm = assembly(od, tab)?;
    let inst = find_instance(asm, a.str("instance")?, "instance")?;
    let name = asm.instance(inst).map(|i| i.name.clone()).unwrap_or_default();
    let t = a.array("translation")?;
    let mut xyz = [0.0; 3];
    for (i, v) in t.iter().take(3).enumerate() {
        xyz[i] = crate::ops::sketch::length_value(v, &units).map_err(|e| e.with_param(format!("translation[{i}]")))?;
    }
    let rotation = match (a.opt_array("axis")?, a.get("angle")) {
        (Some(ax), Some(ang)) => {
            let axis = DVec3::new(ax.first().and_then(Value::as_f64).unwrap_or(0.0), ax.get(1).and_then(Value::as_f64).unwrap_or(0.0), ax.get(2).and_then(Value::as_f64).unwrap_or(1.0)).normalize_or_zero();
            let text = match ang {
                Value::Number(n) => n.to_string(),
                Value::String(s) => s.clone(),
                _ => return Err(OpError::param("angle", "an angle")),
            };
            let q = noshape_units::evaluate_as(&text, noshape_units::Dim::ANGLE, &units, &noshape_units::EmptyScope).map_err(|e| OpError::param("angle", &e.to_string()))?;
            let r = DQuat::from_axis_angle(axis, q.value);
            Some([r.x, r.y, r.z, r.w])
        }
        _ => None,
    };
    let result = solve(od, tab, &SolveOptions::default())?;
    commit(od, tab, &format!("Move : {name}"), &result, |a| {
        if let Some(i) = a.instance_mut(inst) {
            i.placement.translation = xyz;
            if let Some(r) = rotation {
                i.placement.rotation = r;
            }
        }
    })?;
    Ok(describe(od, tab)?.into())
}

fn set_state(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = target(ctx, a)?;
    let asm = assembly(od, tab)?;
    let ids = a.array("instances")?.iter().enumerate().map(|(i, v)| find_instance(asm, v.as_str().unwrap_or_default(), &format!("instances[{i}]"))).collect::<Result<Vec<_>, _>>()?;
    let values = asm.configuration.defaults();
    let mut changed = false;
    if let Some(f) = a.get("fixed").and_then(Value::as_bool) {
        let r = solve(od, tab, &SolveOptions::default())?;
        commit(od, tab, sa::fix_description(f), &r, |a| sa::set_fixed(a, &ids, f))?;
        changed = true;
    }
    if let Some(h) = a.get("hidden").and_then(Value::as_bool) {
        let r = solve(od, tab, &SolveOptions::default())?;
        commit(od, tab, sa::hide_description(h), &r, |a| sa::set_hidden(a, &ids, h))?;
        changed = true;
    }
    if let Some(s) = a.get("suppressed").and_then(Value::as_bool) {
        let r = solve(od, tab, &SolveOptions::default())?;
        commit(od, tab, sa::suppress_description(s), &r, |a| sa::set_suppressed(a, &ids, s, &values))?;
        changed = true;
    }
    if !changed {
        return Err(OpError::invalid("give fixed, hidden or suppressed"));
    }
    Ok(describe(od, tab)?.into())
}

fn delete(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = target(ctx, a)?;
    let asm = assembly(od, tab)?.clone();
    let insts = match a.opt_array("instances")? {
        Some(l) => l.iter().enumerate().map(|(i, v)| find_instance(&asm, v.as_str().unwrap_or_default(), &format!("instances[{i}]"))).collect::<Result<Vec<_>, _>>()?,
        None => Vec::new(),
    };
    let mates = match a.opt_array("mates")? {
        Some(l) => l.iter().map(|v| find_mate(&asm, v.as_str().unwrap_or_default())).collect::<Result<Vec<_>, _>>()?,
        None => Vec::new(),
    };
    if insts.is_empty() && mates.is_empty() {
        return Err(OpError::invalid("give instances or mates"));
    }
    for m in mates {
        let r = solve(od, tab, &SolveOptions::default())?;
        let desc = sa::delete_mate_description(&asm, m);
        commit(od, tab, &desc, &r, |a| sa::delete_mate(a, m))?;
    }
    if !insts.is_empty() {
        let r = solve(od, tab, &SolveOptions::default())?;
        commit(od, tab, sa::DELETE_INSTANCES, &r, |a| sa::delete_instances(a, &insts))?;
    }
    Ok(describe(od, tab)?.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mate_type_ids_are_unique() {
        let ids: Vec<String> = MateType::ALL.iter().map(|t| mate_type_id(*t)).collect();
        let mut d = ids.clone();
        d.dedup();
        assert_eq!(ids.len(), d.len());
    }
}
