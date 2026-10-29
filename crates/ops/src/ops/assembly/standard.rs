//! Standard content: the Insert dialog's Standard content tab (catalogue,
//! insert into holes with auto-size and stacking) and Edit standard content
//! instance.

use super::{assembly, asm_field, commit, describe, find_instance, occurrence, solve, target};
use crate::error::OpError;
use crate::ops::{doc_field, s, strings};
use crate::registry::{Args, Op, OpOutput, opt, req};
use crate::{Ctx, assembly as sa, refs};
use glam::DVec3;
use noshape_model::assembly::eval::SolveOptions;
use noshape_model::assembly::standard::{self, Component, StandardSpec};
use noshape_model::assembly::{Assembly, InstanceId, MateConnectorRef};
use serde_json::{Map, Value, json};

const SPEC_DOC: &str = "The component: `standard` (\"ANSI\", \"ISO\", \"DIN\"…), `category`, `class` and `component` as the tab's pickers name them (see assembly_standard_content; `component` alone is enough when its name is unique), `size`, `length`, `params` ({label: option} for the component's other selects, e.g. {\"Head slot\": \"checked\", \"Series\": \"Wide\"}), `material`, `finish`. Missing values are the dialog's defaults.";

pub fn ops() -> Vec<Op> {
    let spec_fields = || {
        vec![
            opt("standard", s(), "Standard."),
            opt("category", s(), "Category."),
            opt("class", s(), "Class."),
            opt("component", s(), "Component."),
            opt("size", s(), "Size (default: the component's default, or auto-size from the first hole with `auto_size`)."),
            opt("length", s(), "Length (default: the size's first)."),
            opt("params", json!({ "type": "object" }), "Other selects: {label: option}."),
            opt("material", s(), "Material."),
            opt("finish", s(), "Finish."),
        ]
    };
    let mut insert_fields = vec![doc_field(), asm_field()];
    insert_fields.extend(spec_fields());
    insert_fields.extend([
        opt("holes", json!({ "type": "array", "items": { "type": "object" } }), "Faces or edges to fasten into: [{\"instance\", \"entity\"}] (a circular edge or cylindrical face: one hole; a planar face: every hole in it). One instance per hole, each with a Fastened mate. Without holes: one unplaced instance."),
        opt("toward", json!({ "type": "array", "items": { "type": "number" }, "minItems": 3, "maxItems": 3 }), "For an edge or cylinder: the hole end whose face points most this way (assembly coordinates; the dialog uses the view direction). Default [0, 0, 1]."),
        opt("auto_size", json!({ "type": "boolean", "default": false }), "Auto-size: the size fitting the first hole (bolts and screws round down, nuts and washers up)."),
        opt("placement", json!({ "type": "string", "enum": ["insert", "closest", "furthest"], "default": "insert" }), "Insert, Insert closest to selection (against the face, under what is there) or Insert furthest from selection (at the end of the stack)."),
        opt("flip", json!({ "type": "boolean", "default": false }), "Flip the part on its mate connector (the `a` key)."),
    ]);
    let mut edit_fields = vec![doc_field(), asm_field(), req("instances", strings(), "Standard content instance ids or names (same component).")];
    edit_fields.extend(spec_fields().into_iter().skip(4));
    vec![
        Op {
            group: "assembly",
            verb: "standard_content",
            summary: "Browse the standard content catalogue",
            description: "The Insert dialog's Standard content pickers. With nothing: the standards; with `standard`: its categories; then classes, components; with a full component: its sizes, lengths, other selects, materials and finishes and the defaults.",
            fields: vec![opt("standard", s(), "Standard."), opt("category", s(), "Category."), opt("class", s(), "Class."), opt("component", s(), "Component."), opt("size", s(), "Size (for its lengths; default the component's default size).")],
            mutates: false,
            destructive: false,
            run: catalogue,
        },
        Op {
            group: "assembly",
            verb: "insert_standard",
            summary: "Insert standard content (bolts, screws, nuts, washers, pins…)",
            description: SPEC_DOC,
            fields: insert_fields,
            mutates: true,
            destructive: false,
            run: insert,
        },
        Op {
            group: "assembly",
            verb: "edit_standard",
            summary: "Edit standard content instances (size, length, selects, material, finish)",
            description: "Edit standard content instance…: the instances take the new values and keep their mates.",
            fields: edit_fields,
            mutates: true,
            destructive: false,
            run: edit,
        },
    ]
}

fn ci_eq(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b.trim())
}

/// The component named by the arguments (a unique `component` name is enough).
fn find_component(a: &Args) -> Result<&'static Component, OpError> {
    let (st, cat, cl, comp) = (a.opt_str("standard")?, a.opt_str("category")?, a.opt_str("class")?, a.opt_str("component")?);
    let hits: Vec<&'static Component> = standard::CATALOGUE
        .iter()
        .filter(|c| st.is_none_or(|v| ci_eq(&c.standard, v)) && cat.is_none_or(|v| ci_eq(&c.category, v)) && cl.is_none_or(|v| ci_eq(&c.class, v)) && comp.is_none_or(|v| ci_eq(&c.name, v)))
        .collect();
    match hits.as_slice() {
        [one] => Ok(one),
        [] => Err(OpError::param("component", "no such component (see assembly_standard_content)")),
        many if comp.is_none() => Err(OpError::param("component", &format!("give the component ({} match)", many.len()))),
        many => Err(OpError::param("standard", &format!("{} components have that name; give `standard` (one of {})", many.len(), many.iter().map(|c| c.standard.as_str()).collect::<Vec<_>>().join(", ")))),
    }
}

/// The spec from the arguments over `base` (checked against the catalogue).
fn apply_spec(mut spec: StandardSpec, c: &Component, a: &Args) -> Result<StandardSpec, OpError> {
    if let Some(sz) = a.opt_str("size")? {
        let names = c.size_names();
        let n = names.iter().find(|n| ci_eq(n, sz)).ok_or_else(|| OpError::param("size", &format!("one of {}", names.join(", "))))?;
        if let Some(msg) = c.failure(n) {
            return Err(OpError::param("size", &format!("Insertable standard content failed to generate. {msg}")));
        }
        spec.set_size(n);
    }
    for (k, v) in a.object("params")?.into_iter().flatten() {
        let p = c.params.iter().find(|p| ci_eq(&p.label, k)).ok_or_else(|| OpError::param(&format!("params.{k}"), &format!("the component's selects are {}", c.params.iter().map(|p| p.label.as_str()).collect::<Vec<_>>().join(", "))))?;
        let v = match v {
            Value::Bool(b) if p.checkbox => if *b { standard::CHECKED } else { standard::UNCHECKED }.to_string(),
            Value::String(t) => t.clone(),
            _ => return Err(OpError::param(&format!("params.{k}"), "an option name")),
        };
        let options = c.param_options(p, spec.size_row());
        let o = options.iter().find(|o| ci_eq(o, &v)).ok_or_else(|| OpError::param(&format!("params.{k}"), &format!("one of {}", options.join(", "))))?;
        spec.params.insert(p.label.clone(), o.clone());
        spec.fix_length();
    }
    if let Some(l) = a.opt_str("length")? {
        let lengths = spec.size_row().map(|s| c.lengths(s)).unwrap_or_default();
        let hit = lengths.iter().find(|x| ci_eq(x, l)).ok_or_else(|| OpError::param("length", &format!("one of {}", lengths.join(", "))))?;
        spec.length = Some(hit.clone());
    }
    if let Some(v) = a.opt_str("material")? {
        let list = c.materials();
        let hit = list.iter().find(|x| ci_eq(x, v)).ok_or_else(|| OpError::param("material", &format!("one of {}", list.join(", "))))?;
        spec.material = Some(hit.to_string());
    }
    // The finishes the chosen material offers (PEM®: one).
    if let Some(v) = a.opt_str("finish")? {
        let list = c.finishes_for(spec.material());
        let hit = list.iter().find(|x| ci_eq(x, v)).ok_or_else(|| OpError::param("finish", &format!("one of {}", list.join(", "))))?;
        spec.finish = Some(hit.to_string());
    }
    Ok(spec)
}

fn spec_json(spec: &StandardSpec) -> Value {
    let mut v = json!({ "standard": spec.standard, "category": spec.category, "class": spec.class, "component": spec.component, "size": spec.size, "material": spec.material(), "finish": spec.finish(), "part_name": spec.part_name() });
    if let Some(l) = &spec.length {
        v["length"] = json!(l);
    }
    if !spec.params.is_empty() {
        v["params"] = json!(spec.params);
    }
    v
}

fn catalogue(_: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (st, cat, cl, comp) = (a.opt_str("standard")?, a.opt_str("category")?, a.opt_str("class")?, a.opt_str("component")?);
    let pick = |list: Vec<&'static str>, v: &str, field: &str| -> Result<&'static str, OpError> { list.iter().copied().find(|x| ci_eq(x, v)).ok_or_else(|| OpError::param(field, &format!("one of {}", list.join(", ")))) };
    let Some(st) = st else { return Ok(json!({ "standards": standard::standards() }).into()) };
    let st = pick(standard::standards(), st, "standard")?;
    let Some(cat) = cat else { return Ok(json!({ "standard": st, "categories": standard::categories(st) }).into()) };
    let cat = pick(standard::categories(st), cat, "category")?;
    let Some(cl) = cl else { return Ok(json!({ "standard": st, "category": cat, "classes": standard::classes(st, cat) }).into()) };
    let cl = pick(standard::classes(st, cat), cl, "class")?;
    let comps = standard::components(st, cat, cl);
    let Some(comp) = comp else { return Ok(json!({ "standard": st, "category": cat, "class": cl, "components": comps.iter().map(|c| c.name.as_str()).collect::<Vec<_>>() }).into()) };
    let c = comps.iter().find(|c| ci_eq(&c.name, comp)).ok_or_else(|| OpError::param("component", &format!("one of {}", comps.iter().map(|c| c.name.as_str()).collect::<Vec<_>>().join(", "))))?;
    let spec = apply_spec(StandardSpec::new(c), c, a)?;
    let size = spec.size_row();
    let params: Vec<Value> = c.params.iter().map(|p| json!({ "label": p.label, "options": c.param_options(p, size), "default": p.default, "checkbox": p.checkbox })).collect();
    let mut out = Map::new();
    out.insert("size_label".into(), json!(c.size_label()));
    out.insert("sizes".into(), json!(c.size_names()));
    if c.shape.has_length() {
        out.insert("length_label".into(), json!(c.length_label()));
        out.insert("lengths".into(), json!(size.map(|s| c.lengths(s)).unwrap_or_default()));
    }
    out.insert("params".into(), json!(params));
    out.insert("materials".into(), json!(c.materials()));
    out.insert("finishes".into(), json!(c.finishes()));
    out.insert("default".into(), spec_json(&spec));
    Ok(Value::Object(out).into())
}

fn placement(a: &Args) -> Result<sa::Stack, OpError> {
    Ok(match a.opt_str("placement")?.unwrap_or("insert") {
        "insert" => sa::Stack::Plain,
        "closest" => sa::Stack::Closest,
        "furthest" => sa::Stack::Furthest,
        _ => return Err(OpError::param("placement", "insert, closest or furthest")),
    })
}

fn insert(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = target(ctx, a)?;
    let units = od.doc().settings.units;
    let c = find_component(a)?;
    let mut spec = apply_spec(StandardSpec::new(c), c, a)?;
    let mode = placement(a)?;
    let r = solve(od, tab, &SolveOptions::default())?;
    let asm = assembly(od, tab)?;
    let toward = match a.opt_array("toward")? {
        Some(t) => DVec3::new(t[0].as_f64().unwrap_or(0.0), t.get(1).and_then(Value::as_f64).unwrap_or(0.0), t.get(2).and_then(Value::as_f64).unwrap_or(1.0)),
        None => DVec3::Z,
    };
    // Holes: (connector, diameter m); planar faces give every hole in them.
    let mut holes: Vec<(MateConnectorRef, f64)> = Vec::new();
    let mut planar = true;
    for (i, h) in a.opt_array("holes")?.into_iter().flatten().enumerate() {
        let field = format!("holes[{i}]");
        let o = h.as_object().ok_or_else(|| OpError::param(&field, "{\"instance\", \"entity\"}"))?;
        let inst = find_instance(asm, o.get("instance").and_then(Value::as_str).unwrap_or_default(), &format!("{field}.instance"))?;
        let on = refs::parse(o.get("entity").ok_or_else(|| OpError::param(&format!("{field}.entity"), "a face or edge id (assembly_entities)"))?).map_err(|e| e.with_param(format!("{field}.entity")))?;
        let occ = occurrence(&r, inst).ok_or_else(|| OpError::param(&format!("{field}.instance"), "no geometry"))?;
        let local = occ.transform.inverse().transform_vector3(toward).normalize_or_zero();
        let found = standard::hole_connectors(&occ.context, Some(&occ.body), &on, local, &units);
        if found.is_empty() {
            return Err(OpError::param(&format!("{field}.entity"), "Selection must be a circular edge or cylindrical face, or a planar face with holes"));
        }
        planar &= standard::planar_face(&occ.context, Some(&occ.body), &on);
        holes.extend(found.iter().map(|(p, d)| (p.connector(&occ.path), *d)));
    }
    if mode != sa::Stack::Plain && (holes.is_empty() || !planar) {
        return Err(OpError::param("placement", "closest and furthest take planar faces with holes (`holes`)"));
    }
    if a.bool_or("auto_size", false)? {
        let (_, d) = holes.first().ok_or_else(|| OpError::param("auto_size", "give `holes`"))?;
        let size = standard::auto_size(c, *d).ok_or_else(|| OpError::param("auto_size", standard::NO_MATCHING_SIZE))?;
        if a.get("size").is_some() {
            return Err(OpError::param("size", "give `size` or `auto_size`, not both"));
        }
        spec.set_size(size);
    }
    let holes: Vec<MateConnectorRef> = holes.into_iter().map(|h| h.0).collect();
    if mode == sa::Stack::Furthest && sa::stack_topped(asm, &holes) {
        return Err(OpError::invalid(sa::STACK_TOPPED).with_param("placement"));
    }
    let flip = a.bool_or("flip", false)?;
    let ids = commit(od, tab, &sa::insert_standard_description(&spec), &r, |x| sa::insert_standard(x, &spec, &holes, flip, mode))?;
    let mut v = describe(od, tab)?;
    v["inserted"] = json!(ids.iter().map(|i| format!("inst:{}", i.0)).collect::<Vec<_>>());
    v["spec"] = spec_json(&spec);
    Ok(v.into())
}

fn edit(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = target(ctx, a)?;
    let asm: &Assembly = assembly(od, tab)?;
    let ids: Vec<InstanceId> = a.array("instances")?.iter().enumerate().map(|(i, v)| find_instance(asm, v.as_str().unwrap_or_default(), &format!("instances[{i}]"))).collect::<Result<_, _>>()?;
    let first = asm.instance(ids[0]).and_then(|i| i.source.standard()).cloned().ok_or_else(|| OpError::param("instances[0]", "not standard content"))?;
    if let Some(i) = ids.iter().position(|id| asm.instance(*id).and_then(|x| x.source.standard()).is_none_or(|s| s.component != first.component)) {
        return Err(OpError::param(&format!("instances[{i}]"), "not the same standard content component"));
    }
    let c = standard::component(&first).ok_or_else(|| OpError::invalid("the component is not in the catalogue"))?;
    let spec = apply_spec(first, c, a)?;
    let r = solve(od, tab, &SolveOptions::default())?;
    commit(od, tab, &sa::edit_standard_description(&spec), &r, |x| sa::edit_standard(x, &ids, &spec))?;
    let mut v = describe(od, tab)?;
    v["spec"] = spec_json(&spec);
    Ok(v.into())
}
