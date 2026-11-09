//! A Part Studio's tab menu › Show code: its FeatureScript representation, view only, as live writes it
//! (dom/fs-showcode.txt): the version and geometry.fs import, the "Default Units" annotation, main / build /
//! build(configuration) / buildPrivate = definePartStudio(…), then one `features.<id> = function(id) { … }` per
//! feature. A feature passes every parameter of std's precondition (enums as `Type.VALUE`, quantities as
//! `{ 'value' : try(…), 'expression' : "…" }.value`); a selection is a packed persistent query (`qCompressed`) in
//! Onshape's form, naming our features and sketch entities where live names its own; a sketch writes its entities,
//! constraints and initial guess.

use noshape_model::custom::ParamKindFs;
use noshape_model::custom::host::packed::{Packed, pack};
use noshape_model::params::{NumKind, ParamValue, ParamValues};
use noshape_model::partstudio::{Feature, PartStudio};
use noshape_model::reference::{DatumPlane, FeatureId, Reference, Role, Source, TopoName};
use noshape_sketch::{EntityId, Geometry};
use noshape_units::UnitPrefs;
use std::fmt::Write as _;

/// The code of `ps` in a document of units `units`.
pub fn part_studio_code(ps: &PartStudio, units: &UnitPrefs) -> String {
    let v = noshape_featurescript::STD_VERSION;
    let length = units.length.name().to_lowercase();
    let angle = units.angle.name().to_lowercase();
    let cap = |s: &str| format!("{}{}", s[..1].to_uppercase(), &s[1..]);
    let mut defaults = vec![format!("cubic{}", cap(&length)), angle, "degreePerSecond".into(), "footPoundForce".into(), "hertz".into(), "inchPerSecondSquared".into(), "inchPound".into(), length.clone(), "pound".into(), "poundForce".into(), "poundPerCubicInch".into(), "poundPerSquareInch".into(), "second".into(), format!("square{}", cap(&length))];
    defaults.sort();
    defaults.dedup();
    let defaults: Vec<String> = defaults.iter().map(|d| format!("\"{d}\"")).collect();
    let mut out = format!(
        "FeatureScript {v};\nimport(path : \"onshape/std/geometry.fs\", version : \"{v}.0\");\n\nannotation {{ \"Default Units\" : [{}] }}\n\
export function main()\n{{\n    return build({{}});\n}}\n\nexport function build()\n{{\n    return buildPrivate({{}});\n}}\n\n\
export function build(configuration is map)\n{{\n    return buildPrivate(configuration);\n}}\n\n\
const buildPrivate = definePartStudio(function(context is Context, configuration is map, lookup is function)\n    precondition\n    {{\n    }}\n    {{\n\
\x20       const id is Id = newId();\n        annotation {{ 'unused' : true }}\n        var features = {{}};\n",
        defaults.join(", ")
    );
    for f in &ps.features {
        feature(&mut out, ps, f);
    }
    let _ = write!(out, "        return context;\n    }}, {length}, {{}});\n");
    out
}

const IN: &str = "                    ";

fn feature(out: &mut String, ps: &PartStudio, f: &Feature) {
    let fid = feature_id(f.id);
    let _ = write!(out, "        features.{fid} = function(id)\n            {{\n                annotation {{ 'unused' : true }}\n                var features = features;\n                if ({})\n                {{\n", !f.suppressed);
    let mut queries = String::new();
    if let Some(ParamValue::Sketch(data)) = f.params.get("sketch").or_else(|| f.params.0.values().find(|v| matches!(v, ParamValue::Sketch(_)))) {
        let sk = &data.sketch;
        let _ = writeln!(out, "{IN}const initialGuess{fid} = {{ {} }};", initial_guess(sk).join(", "));
        out.push_str(&format!("{IN}{{\n{IN}}}\n"));
        let plane = query_param(&mut queries, ps, f, "sketchPlane", f.params.query("sketchPlane"));
        out.push_str(&queries);
        let _ = writeln!(out, "{IN}annotation {{ \"Feature Name\" : {} }}", quoted(&f.name));
        let _ = writeln!(out, "{IN}var sketch = newSketch(context, id + \"{fid}\", {{ \"sketchPlane\" : {plane} }});");
        sketch_body(out, sk);
        let _ = writeln!(out, "{IN}skSetInitialGuess(sketch, initialGuess{fid});\n{IN}skSolve(sketch);");
    } else {
        out.push_str(&format!("{IN}{{\n{IN}}}\n"));
        let fields = match noshape_model::custom::std_feature::std_params(&f.type_id) {
            Some(decls) => decls.iter().filter_map(|d| param(&mut queries, ps, f, d, &f.params).map(|v| format!("{} : {v}", quoted(&d.id)))).collect::<Vec<_>>(),
            None => f.params.0.iter().filter_map(|(k, v)| plain(&mut queries, ps, f, k, v).map(|v| format!("{} : {v}", quoted(k)))).collect(),
        };
        out.push_str(&queries);
        let _ = writeln!(out, "{IN}annotation {{ \"Feature Name\" : {} }}", quoted(&f.name));
        let map = if fields.is_empty() { "{}".to_string() } else { format!("{{ {} }}", fields.join(", ")) };
        let _ = writeln!(out, "{IN}{}(context, id + \"{fid}\", {map});", f.type_id);
    }
    let _ = write!(out, "                }}\n            }};\n        try(features.{fid}(id));\n");
}

fn feature_id(id: FeatureId) -> String {
    format!("F{}", id.0)
}

fn quoted(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| "\"\"".into())
}

/// A double as live (Java) writes it: always with a fraction ("0.0", "0.025", "3.0").
fn double(x: f64) -> String {
    let r = (x * 1e12).round() / 1e12;
    format!("{:?}", if r == 0.0 { 0.0 } else { r })
}

/// One precondition parameter's value as live writes it (`None`: not in the definition).
fn param(queries: &mut String, ps: &PartStudio, f: &Feature, d: &noshape_model::custom::ParamDecl, values: &ParamValues) -> Option<String> {
    let set = values.get(&d.id);
    Some(match &d.kind {
        ParamKindFs::Num { kind, spec } => match set {
            Some(ParamValue::Expr(e)) => quantity(*kind, &e.text),
            _ => default_quantity(*kind, spec.default_metric),
        },
        ParamKindFs::Bool { default } => match set {
            Some(ParamValue::Bool(b)) => b.to_string(),
            _ => default.to_string(),
        },
        ParamKindFs::Enum { tag, default, .. } => {
            let v = match set {
                Some(ParamValue::Enum(e)) if !e.is_empty() => e.as_str(),
                _ => default.as_str(),
            };
            format!("{}.{v}", noshape_featurescript::value::tag_name(*tag))
        }
        ParamKindFs::Text { default } => quoted(match set {
            Some(ParamValue::Text(t)) => t,
            _ => default,
        }),
        ParamKindFs::Query { .. } | ParamKindFs::FeatureList { .. } => query_param(queries, ps, f, &d.id, values.query(&d.id)),
        ParamKindFs::Array { items, .. } => {
            let rows: Vec<String> = values
                .array(&d.id)
                .iter()
                .map(|row| {
                    let fields: Vec<String> = items.iter().filter_map(|i| param(queries, ps, f, i, row).map(|v| format!("{} : {v}", quoted(&i.id)))).collect();
                    if fields.is_empty() { "{}".to_string() } else { format!("{{ {} }}", fields.join(", ")) }
                })
                .collect();
            format!("[{}]", rows.join(", "))
        }
        ParamKindFs::Lookup { .. } => quoted(&values.lookup(&d.id).join("/")),
        ParamKindFs::Reference { .. } => quoted(values.text(&d.id)),
        ParamKindFs::Anything | ParamKindFs::Button | ParamKindFs::Group { .. } => return None,
    })
}

/// A parameter of a feature std doesn't define (a custom one): its stored value.
fn plain(queries: &mut String, ps: &PartStudio, f: &Feature, id: &str, v: &ParamValue) -> Option<String> {
    Some(match v {
        ParamValue::Bool(b) => b.to_string(),
        ParamValue::Enum(e) | ParamValue::Text(e) => quoted(e),
        ParamValue::Query(refs) => query_param(queries, ps, f, id, refs),
        ParamValue::Lookup(path) => quoted(&path.join("/")),
        ParamValue::Array(items) => format!("[{}]", items.iter().map(|row| format!("{{ {} }}", row.0.iter().filter_map(|(k, v)| plain(queries, ps, f, k, v).map(|v| format!("{} : {v}", quoted(k)))).collect::<Vec<_>>().join(", "))).collect::<Vec<_>>().join(", ")),
        ParamValue::Expr(e) => quantity(NumKind::Real, &e.text),
        ParamValue::Sketch(_) => return None,
    })
}

/// A typed quantity: `{ 'value' : try(10 * millimeter), 'expression' : "10 mm" }.value` (an angle's ° as live's
/// "deg").
fn quantity(kind: NumKind, text: &str) -> String {
    let text = &text.replace('°', " deg").replace("  deg", " deg");
    let value = match kind {
        NumKind::Length | NumKind::Angle => fs_expression(text),
        NumKind::Integer | NumKind::Real => text.trim().to_string(),
    };
    format!("{{ 'value' : try({value}), 'expression' : {} }}.value", quoted(text))
}

/// An unset parameter: std's default as Onshape stores it ("0.025*m", "3.0*deg").
fn default_quantity(kind: NumKind, si: f64) -> String {
    let (value, text) = match kind {
        NumKind::Length => (format!("{} * meter", double(si)), format!("{}*m", double(si))),
        NumKind::Angle => {
            let deg = double(si.to_degrees());
            (format!("{deg} * degree"), format!("{deg}*deg"))
        }
        NumKind::Integer => ((si.round() as i64).to_string(), (si.round() as i64).to_string()),
        NumKind::Real => (double(si), double(si)),
    };
    format!("{{ 'value' : try({value}), 'expression' : {} }}.value", quoted(&text))
}

/// Our expression in FeatureScript: a number's unit as its FeatureScript name ("10 mm" → "10 * millimeter").
fn fs_expression(text: &str) -> String {
    const UNITS: [(&str, &str); 10] = [("mm", "millimeter"), ("cm", "centimeter"), ("um", "micrometer"), ("m", "meter"), ("in", "inch"), ("ft", "foot"), ("yd", "yard"), ("deg", "degree"), ("rad", "radian"), ("°", "degree")];
    let mut out = String::new();
    let mut after_number = false;
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_ascii_digit() || (c == '.' && chars.get(i + 1).is_some_and(char::is_ascii_digit)) {
            out.push(c);
            after_number = true;
            i += 1;
            continue;
        }
        if c.is_alphabetic() || c == '°' {
            let start = i;
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_' || chars[i] == '°') {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            match UNITS.iter().find(|(u, _)| *u == word) {
                Some((_, name)) if after_number => {
                    let trimmed = out.trim_end().len();
                    out.truncate(trimmed);
                    let _ = write!(out, " * {name}");
                }
                _ => out.push_str(&word),
            }
            after_number = false;
            continue;
        }
        if !c.is_whitespace() {
            after_number = false;
        }
        out.push(c);
        i += 1;
    }
    out
}

// ----------------------------------------------------------------- queries

/// A selection parameter: `qUnion([<var>_query])` after declaring the packed query, or `qUnion([])`.
fn query_param(queries: &mut String, ps: &PartStudio, f: &Feature, param: &str, refs: &[Reference]) -> String {
    if refs.is_empty() {
        return "qUnion([])".into();
    }
    let packed = if refs.len() == 1 {
        persistent(ps, &refs[0])
    } else {
        query(vec![("queryType", tagged("QueryType", s("UNION"))), ("subqueries", Packed::Array(refs.iter().map(|r| persistent(ps, r)).collect()))])
    };
    let var = format!("{}{}", param.replace('.', "_"), feature_id(f.id));
    let _ = write!(queries, "{IN}var {var}_query;\n{IN}{var}_query=qCompressed(1.0,{},id);\n", quoted(&pack(&packed)));
    format!("qUnion([{var}_query])")
}

fn s(x: &str) -> Packed {
    Packed::Str(x.into())
}

fn tagged(t: &str, v: Packed) -> Packed {
    Packed::Tagged(t.into(), Box::new(v))
}

/// A Query map, its keys in order (as live packs them).
fn query(mut entries: Vec<(&str, Packed)>) -> Packed {
    entries.sort_by(|a, b| a.0.cmp(b.0));
    tagged("Query", Packed::Map(entries.into_iter().map(|(k, v)| (s(k), v)).collect()))
}

fn op(id: String) -> Packed {
    tagged("Id", Packed::Array(vec![s(&id)]))
}

fn created(entity: &str, operation: String, query_type: &str, mut more: Vec<(&'static str, Packed)>) -> Packed {
    more.extend([("entityType", tagged("EntityType", s(entity))), ("historyType", s("CREATION")), ("operationId", op(operation)), ("queryType", s(query_type))]);
    query(more)
}

fn sketch_entity(sketch: FeatureId, entity: u32, kind: &str) -> Packed {
    created(kind, format!("{}.wireOp", feature_id(sketch)), "SKETCH_ENTITY", vec![("sketchEntityId", s(&EntityId(entity).to_string()))])
}

fn originals(sketch_entities: &[(FeatureId, u32)]) -> (&'static str, Packed) {
    let originals = sketch_entities.iter().map(|(sk, e)| sketch_entity(*sk, *e, "EDGE")).collect();
    ("disambiguationData", Packed::Array(vec![Packed::Map(vec![(s("disambiguationType"), s("ORIGINAL_DEPENDENCY")), (s("originals"), Packed::Array(originals))])]))
}

/// The operation a feature's geometry comes from (live: extrude's "opExtrude").
fn operation(ps: &PartStudio, f: FeatureId) -> String {
    let ty = ps.feature(f).map(|x| x.type_id.as_str()).unwrap_or("");
    let op = match ty {
        "extrude" => "opExtrude",
        "revolve" => "opRevolve",
        "sweep" => "opSweep",
        "loft" => "opLoft",
        "thicken" => "opThicken",
        "fillet" => "fillet",
        "chamfer" => "chamfer",
        "cPlane" => "plane",
        other => other,
    };
    format!("{}.{op}", feature_id(f))
}

/// The sketch entities bounding a region (its key: ordinal, then `(entity << 1) | forward` per edge), in order.
fn region_entities(sketch: FeatureId, key: &[u32]) -> Vec<(FeatureId, u32)> {
    let mut out: Vec<(FeatureId, u32)> = key.iter().skip(1).map(|k| (sketch, k >> 1)).collect();
    out.dedup();
    out
}

/// The sketch entities a profile names.
fn profile_entities(ps: &PartStudio, src: &Source) -> Vec<(FeatureId, u32)> {
    let mut out = match src {
        Source::Sketch { sketch, entity, .. } => vec![(*sketch, *entity)],
        Source::Region { sketch, key } => region_entities(*sketch, key),
        Source::Topo(_) | Source::Index(_) => Vec::new(),
    };
    out.sort();
    out.dedup();
    let _ = ps;
    out
}

/// The profiles' sketch entities of a feature's `entities` selection (a swept body's originals).
fn feature_profile(ps: &PartStudio, f: FeatureId) -> Vec<(FeatureId, u32)> {
    let mut out: Vec<(FeatureId, u32)> = ps
        .feature(f)
        .map(|x| x.params.query("entities"))
        .unwrap_or_default()
        .iter()
        .flat_map(|r| match r {
            Reference::SketchRegion { sketch, key } => region_entities(*sketch, key),
            Reference::SketchEntity { sketch, entity } => vec![(*sketch, *entity)],
            _ => Vec::new(),
        })
        .collect();
    out.sort();
    out.dedup();
    out
}

/// Live's persistent query for one of our references.
fn persistent(ps: &PartStudio, r: &Reference) -> Packed {
    match r {
        Reference::Origin => created("VERTEX", "Origin.pointOp".into(), "DUMMY", vec![]),
        Reference::Plane(p) => {
            let name = match p {
                DatumPlane::Top => "Top",
                DatumPlane::Front => "Front",
                DatumPlane::Right => "Right",
            };
            created("FACE", format!("{name}.planeOp"), "DUMMY", vec![])
        }
        Reference::SketchRegion { sketch, key } => {
            let imprint = format!("{}.imprint", feature_id(*sketch));
            let edges = region_entities(*sketch, key)
                .into_iter()
                .map(|(sk, e)| Packed::Array(vec![created("EDGE", imprint.clone(), "IMPRINT", vec![("derivedFrom", sketch_entity(sk, e, "EDGE"))]), Packed::Num(1.0)]))
                .collect();
            let topology = Packed::Array(vec![Packed::Map(vec![(s("disambiguationType"), s("TOPOLOGY")), (s("entities"), Packed::Array(edges))])]);
            created("FACE", imprint, "IMPRINT", vec![("disambiguationData", topology)])
        }
        Reference::SketchEntity { sketch, entity } => {
            let point = ps.feature(*sketch).and_then(|f| f.params.0.values().find_map(|v| if let ParamValue::Sketch(d) = v { Some(d) } else { None })).and_then(|d| d.sketch.entity(EntityId(*entity))).is_some_and(|e| matches!(e.geometry, Geometry::Point { .. }));
            sketch_entity(*sketch, *entity, if point { "VERTEX" } else { "EDGE" })
        }
        Reference::Body(b) => created("BODY", operation(ps, b.feature), "SWEPT_BODY", vec![originals(&feature_profile(ps, b.feature))]),
        Reference::Face(n) => topo(ps, n, "FACE"),
        Reference::Edge(n) => topo(ps, n, "EDGE"),
        Reference::Vertex(n) => topo(ps, n, "VERTEX"),
        other => fallback(other, "FACE"),
    }
}

/// A face, edge or vertex by its persistent name.
fn topo(ps: &PartStudio, n: &TopoName, entity: &str) -> Packed {
    let face = |n: &TopoName| match n {
        TopoName::Face { feature, role } => Some((*feature, role.clone())),
        _ => None,
    };
    match n {
        TopoName::Face { feature, role } => match role {
            Role::Side(src) => created("FACE", operation(ps, *feature), "SWEPT_FACE", vec![originals(&profile_entities(ps, src))]),
            Role::CapStart(src) | Role::CapEnd(src) => {
                let entities = src.as_ref().map(|s| profile_entities(ps, s)).unwrap_or_else(|| feature_profile(ps, *feature));
                created("FACE", operation(ps, *feature), "CAP_FACE", vec![originals(&entities), ("isStart", Packed::Bool(matches!(role, Role::CapStart(_))))])
            }
            _ => created(entity, operation(ps, *feature), "CREATED", vec![]),
        },
        TopoName::Edge { faces, .. } => match (face(&faces[0]), face(&faces[1])) {
            (Some((fa, Role::Side(a))), Some((fb, Role::Side(b)))) if fa == fb => {
                let mut e = profile_entities(ps, &a);
                e.extend(profile_entities(ps, &b));
                e.sort();
                e.dedup();
                created("EDGE", operation(ps, fa), "SWEPT_EDGE", vec![originals(&e)])
            }
            (Some((fa, Role::Side(side))), Some((fb, cap @ (Role::CapStart(_) | Role::CapEnd(_))))) | (Some((fb, cap @ (Role::CapStart(_) | Role::CapEnd(_)))), Some((fa, Role::Side(side)))) if fa == fb => {
                created("EDGE", operation(ps, fa), "CAP_EDGE", vec![originals(&profile_entities(ps, &side)), ("isStart", Packed::Bool(matches!(cap, Role::CapStart(_))))])
            }
            _ => fallback_topo(n, entity),
        },
        TopoName::Wire { feature, source: Source::Sketch { sketch, entity: e, .. } } if feature == sketch => sketch_entity(*sketch, *e, entity),
        _ => fallback_topo(n, entity),
    }
}

/// What live's form has no counterpart for here: the entity by our own name.
fn fallback_topo(n: &TopoName, entity: &str) -> Packed {
    created(entity, crate::refs::to_id(&match entity {
        "EDGE" => Reference::Edge(n.clone()),
        "VERTEX" => Reference::Vertex(n.clone()),
        _ => Reference::Face(n.clone()),
    }), "DUMMY", vec![])
}

fn fallback(r: &Reference, entity: &str) -> Packed {
    created(entity, crate::refs::to_id(r), "DUMMY", vec![])
}

// ----------------------------------------------------------------- sketches

/// A curve's or point's name in the sketch's code: its id, a curve's ends as `<id>.start` / `<id>.end`, a circle's
/// centre `<id>.center`.
fn point_names(sk: &noshape_sketch::Sketch) -> std::collections::HashMap<EntityId, Vec<String>> {
    let mut names: std::collections::HashMap<EntityId, Vec<String>> = Default::default();
    for e in sk.entities() {
        let id = e.id.to_string();
        match &e.geometry {
            Geometry::Line { start, end } | Geometry::Arc { start, end, .. } | Geometry::EllipticalArc { start, end, .. } => {
                names.entry(*start).or_default().push(format!("{id}.start"));
                names.entry(*end).or_default().push(format!("{id}.end"));
            }
            _ => {}
        }
        if let Geometry::Circle { center, .. } | Geometry::Arc { center, .. } | Geometry::Ellipse { center, .. } | Geometry::EllipticalArc { center, .. } = &e.geometry {
            names.entry(*center).or_default().push(format!("{id}.center"));
        }
    }
    names
}

/// The entities live writes: curves, and points no curve owns.
fn written(sk: &noshape_sketch::Sketch) -> Vec<&noshape_sketch::Entity> {
    let owned = point_names(sk);
    sk.entities().filter(|e| !matches!(e.geometry, Geometry::Point { .. }) || !owned.contains_key(&e.id)).filter(|e| !e.external).collect()
}

fn initial_guess(sk: &noshape_sketch::Sketch) -> Vec<String> {
    written(sk)
        .into_iter()
        .filter_map(|e| {
            let values: Vec<f64> = match sk.curve(e.id) {
                Some(noshape_sketch::Curve2::Line { a, b }) => {
                    let d = b - a;
                    let len = d.length();
                    let dir = if len > 0.0 { d / len } else { d };
                    vec![a.x, a.y, dir.x, dir.y, 0.0, len]
                }
                Some(noshape_sketch::Curve2::Circle { center, radius }) => vec![center.x, center.y, 1.0, 0.0, radius, -1.0],
                Some(noshape_sketch::Curve2::Arc { center, radius, start_angle, end_angle }) => vec![center.x, center.y, 1.0, 0.0, radius, -1.0, start_angle, end_angle],
                _ => match &e.geometry {
                    Geometry::Point { pos } => vec![pos.x, pos.y],
                    _ => return None,
                },
            };
            Some(format!("{} : [{}]", quoted(&e.id.to_string()), values.iter().map(|v| double(*v)).collect::<Vec<_>>().join(", ")))
        })
        .collect()
}

fn sketch_body(out: &mut String, sk: &noshape_sketch::Sketch) {
    let mut counts: std::collections::HashMap<&str, u32> = Default::default();
    for e in written(sk) {
        let call = match e.geometry {
            Geometry::Point { .. } => "skPoint",
            Geometry::Line { .. } => "skLineSegment",
            Geometry::Circle { .. } => "skCircle",
            Geometry::Arc { .. } => "skArc",
            Geometry::Ellipse { .. } => "skEllipse",
            Geometry::EllipticalArc { .. } => "skEllipticalArc",
            Geometry::BSpline { .. } => "skBezier",
            _ => "skFitSpline",
        };
        let n = counts.entry(call).or_default();
        *n += 1;
        let _ = writeln!(out, "{IN}{call}(sketch, {}, {{ \"construction\" : {}, \"index\" : \"{n}\" }});", quoted(&e.id.to_string()), e.construction);
    }
    // Shared ends: live's COINCIDENT between the curves (a rectangle's corners).
    let names = point_names(sk);
    let mut index: std::collections::HashMap<&str, u32> = Default::default();
    let mut k = 0;
    let mut constraint = |out: &mut String, ty: &'static str, id: String, fields: String| {
        let n = index.entry(ty).or_default();
        *n += 1;
        let _ = writeln!(out, "{IN}{{\n{IN}    skConstraint(sketch, {}, {{ \"constraintType\" : ConstraintType.{ty}, \"index\" : \"{n}\", \"name\" : \"\"{fields} }});\n{IN}}}", quoted(&id));
    };
    let mut ends: Vec<(EntityId, String)> = Vec::new();
    for e in sk.entities() {
        if let Geometry::Line { start, end } | Geometry::Arc { start, end, .. } = &e.geometry {
            for (p, which) in [(*start, "start"), (*end, "end")] {
                let me = format!("{}.{which}", e.id);
                if let Some((_, earlier)) = ends.iter().find(|(q, _)| *q == p) {
                    let (first, second) = if which == "end" && earlier.ends_with(".start") { (me.clone(), earlier.clone()) } else { (earlier.clone(), me.clone()) };
                    constraint(out, "COINCIDENT", format!("k{k}"), format!(", \"localFirst\" : {}, \"localSecond\" : {}", quoted(&first), quoted(&second)));
                    k += 1;
                } else {
                    ends.push((p, me));
                }
            }
        }
    }
    let name = |id: EntityId| names.get(&id).and_then(|n| n.first().cloned()).unwrap_or_else(|| id.to_string());
    use noshape_sketch::ConstraintKind as C;
    for c in sk.constraints() {
        let two = |ty: &'static str, a: EntityId, b: EntityId| Some((ty, format!(", \"localFirst\" : {}, \"localSecond\" : {}", quoted(&name(a)), quoted(&name(b)))));
        let one = |ty: &'static str, a: EntityId| Some((ty, format!(", \"localFirst\" : {}", quoted(&name(a)))));
        let dim = |ty: &'static str, ents: &[EntityId], key: &str, v: String| {
            let fields: String = ["localFirst", "localSecond"].iter().zip(ents).map(|(k, e)| format!(", \"{k}\" : {}", quoted(&name(*e)))).collect();
            Some((ty, format!("{fields}, \"{key}\" : {v}")))
        };
        let len = |v: f64| format!("{} * meter", double(v));
        let ang = |v: f64| format!("{} * degree", double(v.to_degrees()));
        let w = match &c.kind {
            C::Coincident { a, b } | C::Collinear { a, b } => two("COINCIDENT", *a, *b),
            C::Concentric { a, b } => two("CONCENTRIC", *a, *b),
            C::Parallel { a, b } => two("PARALLEL", *a, *b),
            C::Perpendicular { a, b } => two("PERPENDICULAR", *a, *b),
            C::Horizontal { a, b: None } => one("HORIZONTAL", *a),
            C::Horizontal { a, b: Some(b) } => two("HORIZONTAL", *a, *b),
            C::Vertical { a, b: None } => one("VERTICAL", *a),
            C::Vertical { a, b: Some(b) } => two("VERTICAL", *a, *b),
            C::Tangent { a, b } => two("TANGENT", *a, *b),
            C::Equal { a, b } => two("EQUAL", *a, *b),
            C::Midpoint { point, curve } => two("MIDPOINT", *point, *curve),
            C::Fix { entity } => one("FIX", *entity),
            C::Normal { line, curve } => two("NORMAL", *line, *curve),
            C::Curvature { a, b } => two("CURVATURE", *a, *b),
            C::Pierce { point, target } => two("PIERCE", *point, *target),
            C::Distance { a, b, value, .. } => dim("DISTANCE", &[*a, *b], "length", len(*value)),
            C::HorizontalDistance { a, b, value } => dim("DISTANCE", &[*a, *b], "length", len(*value)),
            C::VerticalDistance { a, b, value } => dim("DISTANCE", &[*a, *b], "length", len(*value)),
            C::Length { curve, value } => dim("LENGTH", &[*curve], "length", len(*value)),
            C::Radius { curve, value } => dim("RADIUS", &[*curve], "length", len(*value)),
            C::Diameter { curve, value } => dim("DIAMETER", &[*curve], "length", len(*value)),
            C::Angle { a, b, value, .. } => dim("ANGLE", &[*a, *b], "angle", ang(*value)),
            _ => None,
        };
        if let Some((ty, fields)) = w {
            constraint(out, ty, c.id.to_string(), fields);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Live's frame around the features (dom/fs-showcode.txt), one block per feature, in a millimeter document.
    #[test]
    fn a_part_studio_reads_as_live_frames_it() {
        let mut ps = PartStudio::default();
        let mut params = ParamValues::default();
        params.set("radius", ParamValue::Expr(noshape_units::ExprValue::new("3 mm")));
        ps.features.push(Feature { id: FeatureId(7), type_id: "fillet".into(), name: "Fillet 1".into(), suppressed: false, params, folder: None, custom_name: false, hidden: false, suppression_expr: None, sub_features: Vec::new() });
        let code = part_studio_code(&ps, &UnitPrefs::default());
        let v = noshape_featurescript::STD_VERSION;
        assert!(code.starts_with(&format!("FeatureScript {v};\nimport(path : \"onshape/std/geometry.fs\", version : \"{v}.0\");\n\nannotation {{ \"Default Units\" : [")), "{code}");
        assert!(code.contains("const buildPrivate = definePartStudio(function(context is Context, configuration is map, lookup is function)\n"), "{code}");
        assert!(code.contains("        features.F7 = function(id)\n            {\n                annotation { 'unused' : true }\n                var features = features;\n                if (true)\n"), "{code}");
        assert!(code.contains("\"radius\" : { 'value' : try(3 * millimeter), 'expression' : \"3 mm\" }.value"), "{code}");
        assert!(code.ends_with("        try(features.F7(id));\n        return context;\n    }, millimeter, {});\n"), "{code}");
    }

    #[test]
    fn expressions_name_their_units() {
        assert_eq!(fs_expression("10 mm"), "10 * millimeter");
        assert_eq!(fs_expression("0.5in"), "0.5 * inch");
        assert_eq!(fs_expression("30 deg"), "30 * degree");
        assert_eq!(fs_expression("#w + 2 mm"), "#w + 2 * millimeter");
        assert_eq!(default_quantity(NumKind::Length, 0.025), "{ 'value' : try(0.025 * meter), 'expression' : \"0.025*m\" }.value");
        assert_eq!(default_quantity(NumKind::Angle, 3f64.to_radians()), "{ 'value' : try(3.0 * degree), 'expression' : \"3.0*deg\" }.value");
    }
}
