//! The FeatureScript standard library as the oracle for the built-in core
//! features: each built-in dialog must equal the dialog Onshape builds from
//! the std feature's precondition (every parameter, label, default, bound,
//! hint and visibility rule), checked over many dialog states.

use glam::{DVec2, DVec3};
use noshape_model::configuration::ConfigValues;
use noshape_model::custom;
use noshape_model::display::{BodyKind, FeatureStatus};
use noshape_model::params::{Hints, ParamKind, ParamSpec, ParamValue, ParamValues, fill_defaults};
use noshape_model::partstudio::PartStudio;
use noshape_model::reference::{DatumPlane, FeatureId, Reference};
use noshape_model::regen::{RegenOptions, RegenResult, Regenerator};
use noshape_model::sketch_feature::SketchData;
use noshape_units::{ExprValue, UnitPrefs};

/// Core features whose dialogs equal std's in every state.
const STRICT: &[&str] = &[
    "extrude", "fillet", "hole", "loft", "transform", "revolve", "chamfer", "shell", "linearPattern", "circularPattern", "mirror", "sweep", "booleanBodies",
    "cPlane", "draft", "splitPart",
];

/// Hints only built-in dialogs carry (UI behaviour the std expresses
/// elsewhere, e.g. `GROUP_LABEL` for std's collapsible groups).
/// `INITIAL_FOCUS`: Onshape focuses the first selection field by itself.
const BUILTIN_ONLY_HINTS: Hints = Hints(Hints::GROUP_LABEL.0 | Hints::DRIVEN_QUERY.0 | Hints::NO_MATE_CONNECTOR_BUTTON.0 | Hints::INITIAL_FOCUS.0);

fn std_dialog(name: &str) -> Vec<ParamSpec> {
    custom::std_feature::std_dialog(name).unwrap_or_else(|| panic!("{name}: no std dialog"))
}

/// A lookup table's levels (names, options, defaults) as a digest: the
/// built-in tables and std's own must be the same cascade.
fn table_tree(table: &str) -> String {
    let s = table_text(table);
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    format!("{:016x} ({} bytes)", h.finish(), s.len())
}

/// Where two tables' cascades first differ (for the report).
fn table_diff(a: &str, b: &str) -> String {
    let (x, y) = (table_text(a), table_text(b));
    let i = x.bytes().zip(y.bytes()).position(|(p, q)| p != q).unwrap_or(x.len().min(y.len()));
    let w = |s: &str| s.get(i.saturating_sub(120)..(i + 80).min(s.len())).unwrap_or("").to_string();
    format!("    std:     …{}…\n    builtin: …{}…\n", w(&x), w(&y))
}

fn table_text(table: &str) -> String {
    use noshape_model::features::hole_tables as ht;
    fn walk(table: &str, path: &mut Vec<String>, out: &mut String) {
        let opts = ht::table_levels(table, path);
        if opts.is_empty() {
            return;
        }
        out.push_str(&format!("{:?}:{:?}={:?}[", ht::table_level_name(table, path), opts.iter().map(|o| &o.0).collect::<Vec<_>>(), ht::table_level_default(table, path)));
        for (o, _) in opts {
            path.push(o);
            walk(table, path, out);
            path.pop();
        }
        out.push(']');
    }
    let mut s = String::new();
    walk(table, &mut Vec::new(), &mut s);
    s
}

/// Everything about a parameter except its visibility.
fn describe(p: &ParamSpec) -> String {
    let kind = match &p.kind {
        ParamKind::Bool { default } => format!("bool={default}"),
        ParamKind::Enum { options, default } => format!("enum{options:?}={default}"),
        // Rounded: std computes `0.25 * inch` (0.006350000000000001).
        ParamKind::Num(n) => format!("{:?} {}..{} ={:.12} / {:.12}", n.kind, n.min, n.max, n.default_metric, n.default_imperial),
        ParamKind::Text { default } => format!("text={default:?}"),
        // Not the filter: the dialog generator maps std's filter expressions
        // only roughly (no SketchObject.NO, ConstructionObject.NO or
        // QueryFilterCompound); filters are compared by reading the std.
        ParamKind::Query { max_picks, .. } => format!("query max {max_picks:?}"),
        ParamKind::Array { item, item_label } => format!("array {item_label:?} [{}]", item.iter().map(describe).collect::<Vec<_>>().join(", ")),
        ParamKind::Lookup { table } => format!("lookup {}", table_tree(table)),
        ParamKind::Sketch => "sketch".into(),
        ParamKind::Reference { kind } => format!("reference {kind:?}"),
    };
    let mut hints = Hints(p.hints.0 & !BUILTIN_ONLY_HINTS.0);
    // A read-only field can't be configured either way.
    if hints.has(Hints::READ_ONLY) {
        hints = Hints(hints.0 & !Hints::UNCONFIGURABLE.0);
    }
    format!("{} {:?} {kind} hints {:#x} {:?} {:?}", p.id, p.label, hints.0, p.description, p.item_template)
}

fn shown(t: &[ParamSpec], v: &ParamValues) -> Vec<&'static str> {
    t.iter().filter(|p| !p.hints.has(Hints::ALWAYS_HIDDEN) && p.is_visible(v)).map(|p| p.id).collect()
}

/// The dialog states to compare: the defaults, every single choice, every
/// pair of choices and random combinations of all enums and booleans.
fn states(t: &[ParamSpec]) -> Vec<ParamValues> {
    let units = UnitPrefs::default();
    let mut base = ParamValues::default();
    fill_defaults(t, &mut base, &units);
    let choices: Vec<(&str, Vec<ParamValue>)> = t
        .iter()
        // Always-hidden choices (std Extrude's `domain`, the flat view) stay at their default.
        .filter(|p| !p.hints.has(Hints::ALWAYS_HIDDEN))
        .filter_map(|p| match &p.kind {
            ParamKind::Bool { .. } => Some((p.id, vec![ParamValue::Bool(false), ParamValue::Bool(true)])),
            ParamKind::Enum { options, .. } => Some((p.id, options.iter().map(|o| ParamValue::Enum(o.0.to_string())).collect())),
            _ => None,
        })
        .collect();
    // Every pair while that stays cheap (each state evaluates std's
    // conditions in the interpreter); big dialogs (Hole) get more random states.
    let pairs = choices.len() <= 24;
    let mut out = vec![base.clone()];
    for (i, (a, av)) in choices.iter().enumerate() {
        for x in av {
            let mut v = base.clone();
            v.set(a, x.clone());
            out.push(v.clone());
            if !pairs {
                continue;
            }
            for (b, bv) in &choices[i + 1..] {
                for y in bv {
                    let mut w = v.clone();
                    w.set(b, y.clone());
                    out.push(w);
                }
            }
        }
    }
    let mut seed = 0x9e37_79b9_7f4a_7c15_u64;
    for _ in 0..if pairs { 400 } else { 1500 } {
        let mut v = base.clone();
        for (id, vals) in &choices {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            v.set(id, vals[(seed % vals.len() as u64) as usize].clone());
        }
        out.push(v);
    }
    out
}

/// std's own invariants between definition fields a dialog state must
/// keep (hole.fs syncHoleDefinitionV2Params: the legacy fields follow the
/// V2 ones).
fn sync(name: &str, v: &mut ParamValues) {
    if name == "hole" {
        for (legacy, v2) in [("style", "styleV2"), ("endStyle", "endStyleV2"), ("showThreadClass", "showThreadClassV2")] {
            if let Some(x) = v.get(v2).cloned() {
                v.set(legacy, x);
            }
        }
    }
}

fn compare(name: &str) -> String {
    let std = std_dialog(name);
    let builtin = &noshape_model::features::get(name).unwrap_or_else(|| panic!("{name}: no built-in")).params;
    let mut report = String::new();
    // Visibility in every state; std's parameters that no state shows are
    // its internal or legacy fields (hole.fs V1 values), not dialog rows.
    let mut ever = std::collections::HashSet::new();
    let mut seen = std::collections::HashSet::new();
    let mut states_report = String::new();
    for mut v in states(&std) {
        sync(name, &mut v);
        let (a, b) = (shown(&std, &v), shown(builtin, &v));
        ever.extend(a.iter().copied());
        if a != b && seen.insert((a.clone(), b.clone())) && seen.len() <= 12 {
            let state: Vec<String> = v.0.iter().filter_map(|(k, x)| match x {
                ParamValue::Bool(true) => Some(k.clone()),
                ParamValue::Enum(e) => Some(format!("{k}={e}")),
                _ => None,
            }).collect();
            let only_std: Vec<_> = a.iter().filter(|i| !b.contains(i)).collect();
            let only_b: Vec<_> = b.iter().filter(|i| !a.contains(i)).collect();
            states_report.push_str(&format!("  in {state:?}: shown only by std {only_std:?}, only by builtin {only_b:?}\n"));
        }
    }
    let in_builtin = |p: &ParamSpec| builtin.iter().any(|b| b.id == p.id && !b.hints.has(Hints::ALWAYS_HIDDEN));
    // Always-hidden parameters (std's `domain`) aren't part of the dialog.
    let fs_all: Vec<String> = std.iter().filter(|p| !p.hints.has(Hints::ALWAYS_HIDDEN) && (ever.contains(p.id) || in_builtin(p))).map(describe).collect();
    let b_all: Vec<String> = builtin.iter().filter(|p| !p.hints.has(Hints::ALWAYS_HIDDEN)).map(describe).collect();
    let id = |s: &String| s.split(' ').next().unwrap_or_default().to_string();
    for x in &fs_all {
        match b_all.iter().find(|y| id(y) == id(x)) {
            None => report.push_str(&format!("  only std:     {x}\n")),
            Some(y) if y != x => {
                report.push_str(&format!("  std:     {x}\n  builtin: {y}\n"));
                let table = |specs: &[ParamSpec]| specs.iter().find(|p| p.id == id(x)).and_then(|p| match &p.kind {
                    ParamKind::Lookup { table } => Some(*table),
                    _ => None,
                });
                if let (Some(a), Some(b)) = (table(&std), table(builtin)) {
                    report.push_str(&table_diff(a, b));
                }
            }
            _ => {}
        }
    }
    for y in b_all.iter().filter(|y| !fs_all.iter().any(|x| id(x) == id(y))) {
        report.push_str(&format!("  only builtin: {y}\n"));
    }
    let ids = |v: &[String]| v.iter().map(id).collect::<Vec<_>>();
    if report.is_empty() && ids(&fs_all) != ids(&b_all) {
        report.push_str(&format!("  order std:     {:?}\n  order builtin: {:?}\n", ids(&fs_all), ids(&b_all)));
    }
    report.push_str(&states_report);
    if report.is_empty() { report } else { format!("== {name}\n{report}") }
}

#[test]
#[ignore = "T2: oracle vs live (exact values, known-difference lists)"]
fn core_dialogs_match_std_in_every_state() {
    let report: String = STRICT.iter().map(|n| compare(n)).collect();
    assert!(report.is_empty(), "built-in dialogs differ from the std:\n{report}");
}

/// Prints the comparison of one feature: `STD_ORACLE=fillet cargo test
/// -p noshape-model --test std_oracle print_one -- --nocapture`.
#[test]
#[ignore = "T2: oracle vs live (exact values, known-difference lists)"]
fn print_one() {
    if let Ok(name) = std::env::var("STD_ORACLE") {
        for n in name.split(',') {
            eprintln!("{}", compare(n));
        }
    }
}

// ------------------------------------------------------------------ regeneration

fn expr(s: &str) -> ParamValue {
    ParamValue::Expr(ExprValue::new(s))
}

fn en(s: &str) -> ParamValue {
    ParamValue::Enum(s.into())
}

/// One test at a time switches the std route (a process-wide setting).
static ROUTE: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// A Part Studio built step by step; the last feature is the one compared.
struct Studio {
    ps: PartStudio,
    units: UnitPrefs,
}

impl Studio {
    fn new() -> Self {
        Self { ps: PartStudio::new(), units: UnitPrefs::default() }
    }

    fn add(&mut self, ty: &str, set: impl FnOnce(&mut ParamValues)) -> FeatureId {
        let mut f = self.ps.new_feature(ty, &self.units).unwrap_or_else(|| panic!("unknown feature {ty}"));
        set(&mut f.params);
        let id = f.id;
        self.ps.features.push(f);
        id
    }

    fn regen(&self) -> RegenResult {
        Regenerator::new().regenerate(&self.ps, &ConfigValues::default(), &self.units, &RegenOptions::default())
    }

    /// A sketch of closed polylines (mm) on `plane`; its regions, largest first.
    fn sketch(&mut self, plane: Reference, loops: &[&[(f64, f64)]]) -> (FeatureId, Vec<Reference>) {
        let mut sk = noshape_sketch::Sketch::new();
        for pts in loops {
            let p: Vec<_> = pts.iter().map(|&(x, y)| sk.add_point(DVec2::new(x * 1e-3, y * 1e-3))).collect();
            for i in 0..p.len() {
                sk.add_line_between(p[i], p[(i + 1) % p.len()]).unwrap();
            }
        }
        let sketch = self.add("sketch", |pv| {
            pv.set("sketchPlane", ParamValue::Query(vec![plane]));
            pv.set("sketch", ParamValue::Sketch(Box::new(SketchData { sketch: sk, ..Default::default() })));
        });
        let r = self.regen();
        let sk = r.display.sketches.iter().find(|s| s.feature == sketch).expect("sketch");
        let mut regions: Vec<_> = sk.regions.iter().collect();
        regions.sort_by(|a, b| b.area.total_cmp(&a.area));
        (sketch, regions.into_iter().map(|g| Reference::SketchRegion { sketch, key: g.key.clone() }).collect())
    }

    /// A sketch's line segments as references (for surface extrudes).
    fn sketch_edges(&self, sketch: FeatureId) -> Vec<Reference> {
        let r = self.regen();
        let sk = r.display.sketches.iter().find(|s| s.feature == sketch).expect("sketch");
        sk.curves.iter().map(|c| Reference::SketchEntity { sketch, entity: c.entity }).collect()
    }

    /// A 40 mm cube with a corner at the origin.
    fn cube(&mut self) -> FeatureId {
        self.add("cube", |p| p.set("sideLength", expr("40 mm")))
    }
}

/// The face / vertex of a body through `p` (mm).
fn face_at(r: &RegenResult, p: DVec3) -> Reference {
    let v = noshape_kernel::make_vertex(p * 1e-3).unwrap();
    for b in &r.context.bodies {
        for (i, f) in b.shape.faces().iter().enumerate() {
            if noshape_kernel::distance(&v, f).is_ok_and(|d| d.distance < 1e-9) {
                return Reference::Face((*b.faces[i]).clone());
            }
        }
    }
    panic!("no face at {p:?}");
}

fn vertex_at(r: &RegenResult, p: DVec3) -> Reference {
    let v = noshape_kernel::make_vertex(p * 1e-3).unwrap();
    for b in &r.context.bodies {
        for (i, x) in b.shape.vertices().iter().enumerate() {
            if noshape_kernel::distance(&v, x).is_ok_and(|d| d.distance < 1e-9) {
                return Reference::Vertex((*b.vertices[i]).clone());
            }
        }
    }
    panic!("no vertex at {p:?}");
}

fn body_ref(r: &RegenResult, i: usize) -> Reference {
    Reference::Body(r.context.bodies[i].id.clone())
}

/// What the last feature did: its status, message, faulty fields and the
/// bodies afterwards (kind, faces, edges, volume or area).
fn outcome(r: &RegenResult, faulty: bool) -> String {
    let s = r.display.features.last().expect("a feature");
    let status = match s.status {
        FeatureStatus::Ok => "ok".to_string(),
        other if faulty => format!("{other:?} {:?} {:?}", s.message.clone().unwrap_or_default(), s.faulty),
        other => format!("{other:?} {:?}", s.message.clone().unwrap_or_default()),
    };
    let mut bodies: Vec<String> = r
        .context
        .bodies
        .iter()
        .map(|b| {
            let size = match b.kind {
                BodyKind::Solid => format!("{:.4} mm³", b.shape.volume().unwrap_or(0.0) * 1e9),
                _ => format!("{:.4} mm²", b.shape.area().unwrap_or(0.0) * 1e6),
            };
            format!("{:?} {} faces {} edges {size}", b.kind, b.shape.faces().len(), b.shape.edges().len())
        })
        .collect();
    bodies.sort();
    format!("{status}\n  {}", bodies.join("\n  "))
}

/// `STD_ORACLE_CASE=draft`: only the cases whose name contains it (with
/// `NOSHAPE_FS_TRACE=1` for std's operations).
fn only(case: &str) -> bool {
    std::env::var("STD_ORACLE_CASE").map_or(true, |c| case.contains(&c))
}

/// Regenerate natively and by std; the outcomes must agree.
fn same_as_std(case: &str, ty: &str, s: &Studio) -> Option<String> {
    same_as_std_but(case, ty, s, true)
}

/// [`same_as_std`]; `faulty`: compare the fields marked at fault too.
fn same_as_std_but(case: &str, ty: &str, s: &Studio, faulty: bool) -> Option<String> {
    let _g = ROUTE.lock().unwrap_or_else(|e| e.into_inner());
    custom::std_feature::set_std_route(&[]);
    let native = outcome(&s.regen(), faulty);
    custom::std_feature::set_std_route(&[ty]);
    let r = s.regen();
    custom::std_feature::set_std_route(&[]);
    let std = outcome(&r, faulty);
    if native == std {
        return None;
    }
    let notices = s.ps.features.last().and_then(|f| custom::run::output_of(f.id)).map(|o| o.notices.iter().map(|n| format!("{} {}: {}", n.module, n.pos, n.message)).collect::<Vec<_>>().join("\n    ")).unwrap_or_default();
    Some(format!("== {case}\n native: {native}\n std:    {std}\n    {notices}\n"))
}

const SQUARE: &[(f64, f64)] = &[(10.0, 10.0), (30.0, 10.0), (30.0, 30.0), (10.0, 30.0)];

/// Extrude scenarios: each regenerates the same natively and by std's
/// extrude.fs (status, message, faulty fields, resulting bodies).
fn extrude_cases() -> Vec<(&'static str, Studio)> {
    let mut cases = Vec::new();
    let region = |s: &mut Studio| s.sketch(Reference::Plane(DatumPlane::Top), &[SQUARE]).1[0].clone();
    let blind = |name: &'static str, set: fn(&mut ParamValues)| {
        let mut s = Studio::new();
        let r = region(&mut s);
        s.add("extrude", |p| {
            p.set("entities", ParamValue::Query(vec![r]));
            set(p);
        });
        (name, s)
    };
    cases.push(blind("blind", |_| {}));
    cases.push(blind("blind flipped 10 mm", |p| {
        p.set("depth", expr("10 mm"));
        p.set("oppositeDirection", ParamValue::Bool(true));
    }));
    cases.push(blind("symmetric", |p| p.set("symmetric", ParamValue::Bool(true))));
    cases.push(blind("second direction", |p| {
        p.set("hasSecondDirection", ParamValue::Bool(true));
        p.set("secondDirectionDepth", expr("5 mm"));
    }));
    cases.push(blind("draft", |p| {
        p.set("hasDraft", ParamValue::Bool(true));
        p.set("draftAngle", expr("5 deg"));
    }));
    cases.push(blind("draft inwards", |p| {
        p.set("hasDraft", ParamValue::Bool(true));
        p.set("draftAngle", expr("5 deg"));
        p.set("draftPullDirection", ParamValue::Bool(true));
    }));
    cases.push(blind("symmetric draft", |p| {
        p.set("symmetric", ParamValue::Bool(true));
        p.set("hasDraft", ParamValue::Bool(true));
        p.set("draftAngle", expr("5 deg"));
    }));
    cases.push(blind("second direction, both drafted", |p| {
        p.set("hasDraft", ParamValue::Bool(true));
        p.set("draftAngle", expr("5 deg"));
        p.set("hasSecondDirection", ParamValue::Bool(true));
        p.set("secondDirectionDepth", expr("5 mm"));
        p.set("hasSecondDirectionDraft", ParamValue::Bool(true));
        p.set("secondDirectionDraftAngle", expr("3 deg"));
    }));
    cases.push(blind("second direction drafted inwards", |p| {
        p.set("hasSecondDirection", ParamValue::Bool(true));
        p.set("secondDirectionDepth", expr("5 mm"));
        p.set("hasSecondDirectionDraft", ParamValue::Bool(true));
        p.set("secondDirectionDraftAngle", expr("3 deg"));
        p.set("secondDirectionDraftPullDirection", ParamValue::Bool(true));
    }));
    cases.push(blind("start offset", |p| {
        p.set("startOffset", ParamValue::Bool(true));
        p.set("startOffsetDistance", expr("3 mm"));
    }));
    cases.push(blind("thin, nothing selected", |p| p.set("bodyType", en("THIN"))));
    let thin = |name: &'static str, set: fn(&mut ParamValues)| {
        let mut s = Studio::new();
        let r = region(&mut s);
        s.add("extrude", |p| {
            p.set("bodyType", en("THIN"));
            p.set("wallShape", ParamValue::Query(vec![r]));
            set(p);
        });
        (name, s)
    };
    cases.push(thin("thin", |_| {}));
    cases.push(thin("thin, both sides", |p| {
        p.set("thickness2", expr("2 mm"));
        p.set("flipWall", ParamValue::Bool(true));
    }));
    cases.push(thin("thin, mid plane", |p| p.set("midplane", ParamValue::Bool(true))));
    cases.push(thin("thin, draft", |p| {
        p.set("hasDraft", ParamValue::Bool(true));
        p.set("draftAngle", expr("5 deg"));
    }));
    cases.push(blind("negative depth", |p| p.set("depth", expr("-10 mm"))));
    cases.push(blind("zero depth", |p| p.set("depth", expr("0 mm"))));
    cases.push(blind("up to face, none picked", |p| p.set("endBound", en("UP_TO_SURFACE"))));
    cases.push(blind("up to part, none picked", |p| p.set("endBound", en("UP_TO_BODY"))));
    cases.push(blind("up to vertex, none picked", |p| p.set("endBound", en("UP_TO_VERTEX"))));
    cases.push(blind("up to next, nothing there", |p| p.set("endBound", en("UP_TO_NEXT"))));
    cases.push(blind("through all", |p| p.set("endBound", en("THROUGH_ALL"))));
    cases.push(blind("direction picked, none given", |p| p.set("hasExtrudeDirection", ParamValue::Bool(true))));
    cases.push(blind("start offset entity, none given", |p| {
        p.set("startOffset", ParamValue::Bool(true));
        p.set("startOffsetBound", en("ENTITY"));
    }));

    let mut s = Studio::new();
    s.add("extrude", |_| {});
    cases.push(("nothing selected", s));

    let mut s = Studio::new();
    s.add("extrude", |p| p.set("bodyType", en("SURFACE")));
    cases.push(("surface, nothing selected", s));

    let mut s = Studio::new();
    let (sk, _) = s.sketch(Reference::Plane(DatumPlane::Top), &[SQUARE]);
    let edges = s.sketch_edges(sk);
    s.add("extrude", |p| {
        p.set("bodyType", en("SURFACE"));
        p.set("surfaceEntities", ParamValue::Query(edges));
    });
    cases.push(("surface", s));

    // Against a 40 mm cube at the origin.
    let with_cube = |name: &'static str, plane: fn(&RegenResult) -> Reference, set: fn(&mut ParamValues, &RegenResult)| {
        let mut s = Studio::new();
        s.cube();
        let r = s.regen();
        let r0 = region_on(&mut s, plane(&r));
        let r = s.regen();
        s.add("extrude", |p| {
            p.set("entities", ParamValue::Query(vec![r0]));
            set(p, &r);
        });
        (name, s)
    };
    fn region_on(s: &mut Studio, plane: Reference) -> Reference {
        s.sketch(plane, &[SQUARE]).1[0].clone()
    }
    let top_face = |r: &RegenResult| face_at(r, DVec3::new(20.0, 20.0, 40.0));
    let top_plane = |_: &RegenResult| Reference::Plane(DatumPlane::Top);
    cases.push(with_cube("boss on the cube", top_face, |_, _| {}));
    cases.push(with_cube("boss on the cube, Add", top_face, |p, r| {
        p.set("operationType", en("ADD"));
        p.set("booleanScope", ParamValue::Query(vec![body_ref(r, 0)]));
    }));
    cases.push(with_cube("cut through all", top_face, |p, r| {
        p.set("operationType", en("REMOVE"));
        p.set("endBound", en("THROUGH_ALL"));
        p.set("oppositeDirection", ParamValue::Bool(true));
        p.set("booleanScope", ParamValue::Query(vec![body_ref(r, 0)]));
    }));
    cases.push(with_cube("cut, merge with all", top_face, |p, _| {
        p.set("operationType", en("REMOVE"));
        p.set("depth", expr("10 mm"));
        p.set("oppositeDirection", ParamValue::Bool(true));
        p.set("defaultScope", ParamValue::Bool(true));
    }));
    cases.push(with_cube("cut, empty merge scope", top_face, |p, _| {
        p.set("operationType", en("REMOVE"));
        p.set("oppositeDirection", ParamValue::Bool(true));
    }));
    cases.push(with_cube("intersect", top_plane, |p, r| {
        p.set("operationType", en("INTERSECT"));
        p.set("depth", expr("60 mm"));
        p.set("booleanScope", ParamValue::Query(vec![body_ref(r, 0)]));
    }));
    cases.push(with_cube("up to face", top_plane, |p, r| {
        p.set("endBound", en("UP_TO_SURFACE"));
        p.set("endBoundEntityFace", ParamValue::Query(vec![face_at(r, DVec3::new(20.0, 20.0, 40.0))]));
    }));
    cases.push(with_cube("up to face with offset", top_plane, |p, r| {
        p.set("endBound", en("UP_TO_SURFACE"));
        p.set("endBoundEntityFace", ParamValue::Query(vec![face_at(r, DVec3::new(20.0, 20.0, 40.0))]));
        p.set("hasOffset", ParamValue::Bool(true));
        p.set("offsetDistance", expr("5 mm"));
    }));
    cases.push(with_cube("up to face behind", top_plane, |p, r| {
        p.set("endBound", en("UP_TO_SURFACE"));
        p.set("oppositeDirection", ParamValue::Bool(true));
        p.set("endBoundEntityFace", ParamValue::Query(vec![face_at(r, DVec3::new(20.0, 20.0, 40.0))]));
    }));
    cases.push(with_cube("up to part", top_plane, |p, r| {
        p.set("endBound", en("UP_TO_BODY"));
        p.set("endBoundEntityBody", ParamValue::Query(vec![body_ref(r, 0)]));
    }));
    cases.push(with_cube("up to vertex", top_plane, |p, r| {
        p.set("endBound", en("UP_TO_VERTEX"));
        p.set("endBoundEntityVertex", ParamValue::Query(vec![vertex_at(r, DVec3::new(40.0, 40.0, 40.0))]));
    }));
    cases.push(with_cube("up to next", top_plane, |p, _| {
        p.set("endBound", en("UP_TO_NEXT"));
    }));
    cases.push(with_cube("direction along a cube edge", top_plane, |p, r| {
        p.set("hasExtrudeDirection", ParamValue::Bool(true));
        p.set("extrudeDirection", ParamValue::Query(vec![face_at(r, DVec3::new(20.0, 20.0, 40.0))]));
    }));
    cases.push(with_cube("direction parallel to the profile", top_plane, |p, r| {
        p.set("hasExtrudeDirection", ParamValue::Bool(true));
        p.set("extrudeDirection", ParamValue::Query(vec![face_at(r, DVec3::new(0.0, 20.0, 20.0))]));
    }));
    cases.push(with_cube("start offset up to the cube top", top_plane, |p, r| {
        p.set("startOffset", ParamValue::Bool(true));
        p.set("startOffsetBound", en("ENTITY"));
        p.set("startOffsetEntity", ParamValue::Query(vec![face_at(r, DVec3::new(20.0, 20.0, 40.0))]));
    }));
    cases.push(with_cube("start offset at a side face", top_plane, |p, r| {
        p.set("startOffset", ParamValue::Bool(true));
        p.set("startOffsetBound", en("ENTITY"));
        p.set("startOffsetEntity", ParamValue::Query(vec![face_at(r, DVec3::new(0.0, 20.0, 20.0))]));
    }));
    cases.push(with_cube("extrude a cube face", |r| face_at(r, DVec3::new(20.0, 20.0, 40.0)), |p, r| {
        p.set("entities", ParamValue::Query(vec![face_at(r, DVec3::new(40.0, 20.0, 20.0))]));
        p.set("operationType", en("ADD"));
        p.set("booleanScope", ParamValue::Query(vec![body_ref(r, 0)]));
    }));
    cases
}

#[test]
#[ignore = "T2: oracle vs live (exact values, known-difference lists)"]
fn extrude_regenerates_as_std() {
    let report: String = extrude_cases().iter().filter(|(name, _)| only(name)).filter_map(|(name, s)| same_as_std(name, "extrude", s)).collect();
    assert!(report.is_empty(), "native Extrude differs from std:\n{report}");
}

/// What std's editing logic changes when the last feature's parameters go
/// from `old` to its current ones (`specified`: what the user set).
fn edit_logic(s: &Studio, old: &ParamValues, specified: &[&str]) -> Vec<String> {
    let index = s.ps.features.len() - 1;
    let new = s.ps.features[index].params.clone();
    let spec: Vec<String> = specified.iter().map(|x| x.to_string()).collect();
    let t = std::time::Instant::now();
    let out = custom::std_feature::editing_logic(&s.ps, index, &ConfigValues::default(), &s.units, &RegenOptions::default(), old, &new, &spec, true);
    eprintln!("editing logic: {:?}", t.elapsed());
    let mut v: Vec<String> = out.expect("editing logic ran").0.into_iter().map(|(k, v)| format!("{k}={v:?}")).collect();
    v.sort();
    v
}

/// std's extrudeEditLogic, as the dialog runs it (app part_studio
/// `std_edit_logic`), through the host natives.
#[test]
#[ignore = "T2: oracle vs live (exact values, known-difference lists)"]
fn extrude_editing_logic_runs_std() {
    let mut s = Studio::new();
    s.cube();
    let r = s.regen();
    let top = face_at(&r, DVec3::new(20.0, 20.0, 40.0));
    let bottom = face_at(&r, DVec3::new(20.0, 20.0, 0.0));
    let region = s.sketch(top, &[SQUARE]).1[0].clone();
    let cube = vec![body_ref(&s.regen(), 0)];
    let base = |s: &mut Studio, set: &dyn Fn(&mut ParamValues)| {
        if s.ps.features.last().is_some_and(|f| f.type_id == "extrude") {
            s.ps.features.pop();
        }
        s.add("extrude", |p| {
            p.set("entities", ParamValue::Query(vec![region.clone()]));
            set(p);
        });
    };

    // A boss on the cube's top face: Add, merging with the cube (live qa5 EXT1).
    base(&mut s, &|_| {});
    let got = edit_logic(&s, &ParamValues::default(), &["entities"]);
    assert_eq!(got, [format!("booleanScope={:?}", ParamValue::Query(cube.clone())), "operationType=Enum(\"ADD\")".to_string()]);

    // Picking Remove points the extrude into the part (canSetBooleanFlip).
    let mut old = ParamValues::default();
    old.set("entities", ParamValue::Query(vec![region.clone()]));
    old.set("operationType", en("ADD"));
    old.set("booleanScope", ParamValue::Query(cube.clone()));
    base(&mut s, &|p| {
        p.set("operationType", en("REMOVE"));
        p.set("booleanScope", ParamValue::Query(cube.clone()));
    });
    let got = edit_logic(&s, &old, &["entities", "operationType"]);
    assert!(got.contains(&"oppositeDirection=Bool(true)".to_string()), "{got:?}");

    // Up to the cube's bottom face: the direction turns towards it.
    base(&mut s, &|p| {
        p.set("endBound", en("UP_TO_SURFACE"));
        p.set("endBoundEntityFace", ParamValue::Query(vec![bottom.clone()]));
    });
    let got = edit_logic(&s, &ParamValues::default(), &["entities", "endBound", "endBoundEntityFace"]);
    assert!(got.contains(&"oppositeDirection=Bool(true)".to_string()), "{got:?}");

    // A second end position starts on the other side of a flipped first one.
    let mut old = ParamValues::default();
    old.set("entities", ParamValue::Query(vec![region.clone()]));
    old.set("oppositeDirection", ParamValue::Bool(true));
    base(&mut s, &|p| {
        p.set("oppositeDirection", ParamValue::Bool(true));
        p.set("hasSecondDirection", ParamValue::Bool(true));
    });
    let got = edit_logic(&s, &old, &["entities", "oppositeDirection", "hasSecondDirection"]);
    assert!(got.contains(&"secondDirectionOppositeDirection=Bool(false)".to_string()), "{got:?}");

    // In empty space: stays New.
    let mut s = Studio::new();
    let region = s.sketch(Reference::Plane(DatumPlane::Top), &[SQUARE]).1[0].clone();
    s.add("extrude", |p| p.set("entities", ParamValue::Query(vec![region])));
    assert_eq!(edit_logic(&s, &ParamValues::default(), &["entities"]), Vec::<String>::new());
}

/// The edge of a body through `p` (mm).
fn edge_at(r: &RegenResult, p: DVec3) -> Reference {
    let v = noshape_kernel::make_vertex(p * 1e-3).unwrap();
    for b in &r.context.bodies {
        for (i, e) in b.shape.edges().iter().enumerate() {
            if noshape_kernel::distance(&v, e).is_ok_and(|d| d.distance < 1e-9) {
                return Reference::Edge((*b.edges[i]).clone());
            }
        }
    }
    panic!("no edge at {p:?}");
}

/// Fillet scenarios on the 40 mm cube, native against std fillet.fs.
fn fillet_cases() -> Vec<(&'static str, Studio)> {
    let mut cases = Vec::new();
    let on_cube = |name: &'static str, set: fn(&mut ParamValues, &RegenResult)| {
        let mut s = Studio::new();
        s.cube();
        let r = s.regen();
        s.add("fillet", |p| set(p, &r));
        (name, s)
    };
    cases.push(on_cube("nothing selected", |_, _| {}));
    cases.push(on_cube("one edge", |p, r| p.set("entities", ParamValue::Query(vec![edge_at(r, DVec3::new(20.0, 0.0, 40.0))]))));
    cases.push(on_cube("a face", |p, r| p.set("entities", ParamValue::Query(vec![face_at(r, DVec3::new(20.0, 20.0, 40.0))]))));
    cases.push(on_cube("radius 0", |p, r| {
        p.set("entities", ParamValue::Query(vec![edge_at(r, DVec3::new(20.0, 0.0, 40.0))]));
        p.set("radius", expr("0 mm"));
    }));
    cases.push(on_cube("radius too big", |p, r| {
        p.set("entities", ParamValue::Query(vec![face_at(r, DVec3::new(20.0, 20.0, 40.0))]));
        p.set("radius", expr("30 mm"));
    }));
    cases.push(on_cube("conic", |p, r| {
        p.set("entities", ParamValue::Query(vec![edge_at(r, DVec3::new(20.0, 0.0, 40.0))]));
        p.set("crossSection", en("CONIC"));
        p.set("nonCircularRadius", expr("6 mm"));
        p.set("rho", expr("0.3"));
    }));
    cases.push(on_cube("curvature", |p, r| {
        p.set("entities", ParamValue::Query(vec![edge_at(r, DVec3::new(20.0, 0.0, 40.0))]));
        p.set("crossSection", en("CURVATURE"));
        p.set("nonCircularRadius", expr("6 mm"));
        p.set("magnitude", expr("0.7"));
    }));
    cases.push(on_cube("width", |p, r| {
        p.set("entities", ParamValue::Query(vec![edge_at(r, DVec3::new(20.0, 0.0, 40.0))]));
        p.set("blendControlType", en("WIDTH"));
        p.set("width", expr("6 mm"));
    }));
    cases.push(on_cube("asymmetric", |p, r| {
        p.set("entities", ParamValue::Query(vec![edge_at(r, DVec3::new(20.0, 0.0, 40.0))]));
        p.set("isAsymmetric", ParamValue::Bool(true));
        p.set("otherRadius", expr("9 mm"));
    }));
    cases.push(on_cube("asymmetric flipped", |p, r| {
        p.set("entities", ParamValue::Query(vec![edge_at(r, DVec3::new(20.0, 0.0, 40.0))]));
        p.set("isAsymmetric", ParamValue::Bool(true));
        p.set("otherRadius", expr("9 mm"));
        p.set("flipAsymmetric", ParamValue::Bool(true));
    }));
    cases.push(on_cube("partial, position", |p, r| {
        p.set("entities", ParamValue::Query(vec![edge_at(r, DVec3::new(20.0, 0.0, 40.0))]));
        p.set("isPartial", ParamValue::Bool(true));
        p.set("partialFirstEdgeTotalParameter", expr("0.25"));
    }));
    cases.push(on_cube("partial, both bounds", |p, r| {
        p.set("entities", ParamValue::Query(vec![edge_at(r, DVec3::new(20.0, 0.0, 40.0))]));
        p.set("isPartial", ParamValue::Bool(true));
        p.set("partialFirstEdgeTotalParameter", expr("0.25"));
        p.set("secondBound", ParamValue::Bool(true));
        p.set("partialSecondEdgeTotalParameter", expr("0.75"));
    }));
    cases.push(on_cube("partial, offset", |p, r| {
        p.set("entities", ParamValue::Query(vec![edge_at(r, DVec3::new(20.0, 0.0, 40.0))]));
        p.set("isPartial", ParamValue::Bool(true));
        p.set("startPartialType", en("OFFSET"));
        p.set("startPartialOffset", expr("10 mm"));
    }));
    cases.push(on_cube("partial, offsets from both ends", |p, r| {
        p.set("entities", ParamValue::Query(vec![edge_at(r, DVec3::new(20.0, 0.0, 40.0))]));
        p.set("isPartial", ParamValue::Bool(true));
        p.set("startPartialType", en("OFFSET"));
        p.set("startPartialOffset", expr("5 mm"));
        p.set("secondBound", ParamValue::Bool(true));
        p.set("endPartialType", en("OFFSET"));
        p.set("endPartialOffset", expr("10 mm"));
    }));
    cases.push(on_cube("partial, offset past the end", |p, r| {
        p.set("entities", ParamValue::Query(vec![edge_at(r, DVec3::new(20.0, 0.0, 40.0))]));
        p.set("isPartial", ParamValue::Bool(true));
        p.set("startPartialType", en("OFFSET"));
        p.set("startPartialOffset", expr("50 mm"));
    }));
    cases.push(on_cube("partial, no entity", |p, r| {
        p.set("entities", ParamValue::Query(vec![edge_at(r, DVec3::new(20.0, 0.0, 40.0))]));
        p.set("isPartial", ParamValue::Bool(true));
        p.set("startPartialType", en("ENTITY"));
    }));
    cases.push(on_cube("partial, two separate edges", |p, r| {
        p.set("entities", ParamValue::Query(vec![edge_at(r, DVec3::new(20.0, 0.0, 40.0)), edge_at(r, DVec3::new(20.0, 40.0, 40.0))]));
        p.set("isPartial", ParamValue::Bool(true));
    }));
    cases.push(on_cube("partial, same bounds", |p, r| {
        p.set("entities", ParamValue::Query(vec![edge_at(r, DVec3::new(20.0, 0.0, 40.0))]));
        p.set("isPartial", ParamValue::Bool(true));
        p.set("partialFirstEdgeTotalParameter", expr("0.5"));
        p.set("secondBound", ParamValue::Bool(true));
        p.set("partialSecondEdgeTotalParameter", expr("0.5"));
    }));
    cases.push(on_cube("variable", |p, r| {
        p.set("entities", ParamValue::Query(vec![edge_at(r, DVec3::new(20.0, 0.0, 40.0))]));
        p.set("isVariable", ParamValue::Bool(true));
        let item = |at: DVec3, radius: &str| {
            let mut v = ParamValues::default();
            v.set("vertex", ParamValue::Query(vec![vertex_at(r, at)]));
            v.set("vertexRadius", expr(radius));
            v
        };
        p.set("vertexSettings", ParamValue::Array(vec![item(DVec3::new(0.0, 0.0, 40.0), "3 mm"), item(DVec3::new(40.0, 0.0, 40.0), "8 mm")]));
    }));
    cases.push(on_cube("variable with a point on the edge", |p, r| {
        p.set("entities", ParamValue::Query(vec![edge_at(r, DVec3::new(20.0, 0.0, 40.0))]));
        p.set("isVariable", ParamValue::Bool(true));
        let mut v = ParamValues::default();
        v.set("vertex", ParamValue::Query(vec![vertex_at(r, DVec3::new(0.0, 0.0, 40.0))]));
        v.set("vertexRadius", expr("3 mm"));
        let mut e = ParamValues::default();
        e.set("edge", ParamValue::Query(vec![edge_at(r, DVec3::new(20.0, 0.0, 40.0))]));
        e.set("edgeParameter", expr("0.5"));
        e.set("pointOnEdgeRadius", expr("8 mm"));
        p.set("vertexSettings", ParamValue::Array(vec![v]));
        p.set("pointOnEdgeSettings", ParamValue::Array(vec![e]));
    }));
    cases.push(on_cube("full round", |p, r| {
        p.set("filletType", en("FULL_ROUND"));
        p.set("side1Face", ParamValue::Query(vec![face_at(r, DVec3::new(0.0, 20.0, 20.0))]));
        p.set("side2Face", ParamValue::Query(vec![face_at(r, DVec3::new(40.0, 20.0, 20.0))]));
        p.set("centerFaces", ParamValue::Query(vec![face_at(r, DVec3::new(20.0, 20.0, 40.0))]));
    }));
    cases.push(on_cube("full round, nothing selected", |p, _| p.set("filletType", en("FULL_ROUND"))));
    cases
}

#[test]
#[ignore = "T2: oracle vs live (exact values, known-difference lists)"]
fn fillet_regenerates_as_std() {
    let report: String = fillet_cases().iter().filter(|(name, _)| only(name)).filter_map(|(name, s)| same_as_std(name, "fillet", s)).collect();
    assert!(report.is_empty(), "native Fillet differs from std:\n{report}");
}

/// std's filletEditLogic: the first vertex of a variable fillet takes the
/// fillet's radius, second radius, flip (reversed) and rho.
#[test]
#[ignore = "T2: oracle vs live (exact values, known-difference lists)"]
fn fillet_editing_logic_runs_std() {
    let mut s = Studio::new();
    s.cube();
    let r = s.regen();
    let edge = edge_at(&r, DVec3::new(20.0, 0.0, 40.0));
    let vertex = vertex_at(&r, DVec3::new(0.0, 0.0, 40.0));
    let set = |p: &mut ParamValues| {
        p.set("entities", ParamValue::Query(vec![edge.clone()]));
        p.set("crossSection", en("CONIC"));
        p.set("nonCircularRadius", expr("7 mm"));
        p.set("rho", expr("0.3"));
        p.set("isAsymmetric", ParamValue::Bool(true));
        p.set("otherRadius", expr("9 mm"));
        p.set("isVariable", ParamValue::Bool(true));
    };
    let mut old = ParamValues::default();
    set(&mut old);
    s.add("fillet", |p| {
        set(p);
        let mut item = ParamValues::default();
        item.set("vertex", ParamValue::Query(vec![vertex.clone()]));
        p.set("vertexSettings", ParamValue::Array(vec![item]));
    });
    let got = edit_logic(&s, &old, &["entities", "crossSection", "nonCircularRadius", "rho", "isAsymmetric", "otherRadius", "isVariable", "vertexSettings"]);
    let item = got.iter().find(|x| x.starts_with("vertexSettings=")).unwrap_or_else(|| panic!("{got:?}"));
    for want in ["vertexRadius", "Expr", "0.007", "vertexOtherRadius", "0.009", "vertexFlipAsymmetric\": Bool(true)", "variableRho", "0.3"] {
        assert!(item.contains(want), "{want}: {item}");
    }
}

/// Hole scenarios: a sketch point on the 40 mm cube's top face at (20, 20)
/// (or on Top, under the cube), the cube as merge scope.
fn hole_cases() -> Vec<(&'static str, Studio)> {
    let mut cases = Vec::new();
    let hole = |name: &'static str, on_top: bool, set: fn(&mut ParamValues)| {
        let mut s = Studio::new();
        s.cube();
        let r = s.regen();
        let plane = if on_top { face_at(&r, DVec3::new(20.0, 20.0, 40.0)) } else { Reference::Plane(DatumPlane::Top) };
        let mut sk = noshape_sketch::Sketch::new();
        let p = sk.add_point(DVec2::new(0.02, 0.02));
        let sketch = s.add("sketch", |pv| {
            pv.set("sketchPlane", ParamValue::Query(vec![plane]));
            pv.set("sketch", ParamValue::Sketch(Box::new(SketchData { sketch: sk, ..Default::default() })));
        });
        let cube = body_ref(&r, 0);
        s.add("hole", |pv| {
            pv.set("locations", ParamValue::Query(vec![Reference::SketchEntity { sketch, entity: p.0 }]));
            pv.set("scope", ParamValue::Query(vec![cube]));
            pv.set("unitsSystem", en("METRIC"));
            set(pv);
            noshape_model::features::hole_tables::apply_table_to_params(pv);
        });
        (name, s)
    };
    cases.push(hole("simple blind", true, |_| {}));
    cases.push(hole("simple blind, flat tip", true, |p| p.set("tipAngleStyle", en("FLAT"))));
    cases.push(hole("through all", true, |p| p.set("endStyleV2", en("THROUGH"))));
    cases.push(hole("up to next", true, |p| p.set("endStyleV2", en("UP_TO_NEXT"))));
    cases.push(hole("counterbore", true, |p| {
        p.set("styleV2", en("C_BORE"));
        p.set("cBoreDiameterV3", expr("12 mm"));
        p.set("cBoreDepthV3", expr("4 mm"));
    }));
    cases.push(hole("countersink", true, |p| {
        p.set("styleV2", en("C_SINK"));
        p.set("cSinkDiameterV3", expr("12 mm"));
    }));
    cases.push(hole("counterbore too narrow", true, |p| {
        p.set("styleV2", en("C_BORE"));
        p.set("cBoreDiameterV3", expr("4 mm"));
    }));
    cases.push(hole("tapped M5", true, |p| p.set("isoHoleTableEx", ParamValue::Lookup(vec!["Tapped".into(), "Straight tap".into(), "M5".into(), "0.8 mm".into()]))));
    cases.push(hole("from the sketch plane below", false, |p| p.set("startStyle", en("SKETCH"))));
    cases.push(hole("from the part, sketch below", false, |_| {}));
    cases.push(hole("opposite direction", true, |p| p.set("oppositeDirection", ParamValue::Bool(true))));
    let mut s = Studio::new();
    s.cube();
    s.add("hole", |p| p.set("unitsSystem", en("METRIC")));
    cases.push(("nothing selected", s));
    cases
}

#[test]
#[ignore = "T2: oracle vs live (exact values, known-difference lists)"]
fn hole_regenerates_as_std() {
    // A hole that hits nothing flags Merge scope (live behaviours.md "Hole"
    // 2154): std maps opHole's faulty `targets` onto it.
    let report: String = hole_cases().iter().filter(|(name, _)| only(name)).filter_map(|(name, s)| same_as_std(name, "hole", s)).collect();
    assert!(report.is_empty(), "native Hole differs from std:\n{report}");
}

/// std's holeEditLogic keeps a blind tapped hole's Depth, Tapped depth and
/// Tap clearance linked (adjustDepthAndThreadParameters); the native rule
/// (hole_tables::link_tapped_depths, live L49/L50) must pick the same field.
#[test]
#[ignore = "T2: oracle vs live (exact values, known-difference lists)"]
fn hole_tapped_depths_link_as_std() {
    use noshape_model::features::hole_tables::link_tapped_depths;
    let units = UnitPrefs::default();
    let mut s = Studio::new();
    s.cube();
    let r = s.regen();
    let mut sk = noshape_sketch::Sketch::new();
    let p = sk.add_point(DVec2::new(0.02, 0.02));
    let top = face_at(&r, DVec3::new(20.0, 20.0, 40.0));
    let sketch = s.add("sketch", |pv| {
        pv.set("sketchPlane", ParamValue::Query(vec![top]));
        pv.set("sketch", ParamValue::Sketch(Box::new(SketchData { sketch: sk, ..Default::default() })));
    });
    let cube = body_ref(&r, 0);
    let base = move |depth: &str, tapped: &str, clearance: &str| {
        let mut v = ParamValues::default();
        fill_defaults(&noshape_model::features::get("hole").expect("hole").params, &mut v, &UnitPrefs::default());
        v.set("locations", ParamValue::Query(vec![Reference::SketchEntity { sketch, entity: p.0 }]));
        v.set("scope", ParamValue::Query(vec![cube.clone()]));
        v.set("unitsSystem", en("METRIC"));
        v.set("isoHoleTableEx", ParamValue::Lookup(vec!["Tapped".into(), "Straight tap".into(), "M6".into(), "1.00 mm".into()]));
        noshape_model::features::hole_tables::apply_table_to_params(&mut v);
        v.set("holeDepthV3", expr(depth));
        v.set("tappedDepthV3", expr(tapped));
        v.set("tapClearance", expr(clearance));
        // As the previous run of holeEditLogic left them (syncHoleDefinitionV3Params).
        v.set("holeDepth", expr(depth));
        v.set("tappedDepth", expr(tapped));
        v
    };
    let num = |p: &ParamValue| match p {
        ParamValue::Expr(e) => e.cached.map(|q| q.value).unwrap_or_else(|| noshape_units::evaluate_as(&e.text, if e.text.ends_with("mm") { noshape_units::Dim::LENGTH } else { noshape_units::Dim::UNITLESS }, &units, &std::collections::BTreeMap::new()).unwrap().value),
        other => panic!("{other:?}"),
    };
    // (field edited, its new value) from Depth 20, Tapped depth 5, clearance 15.
    for (field, value) in [("holeDepthV3", "25 mm"), ("holeDepthV3", "3 mm"), ("tapClearance", "2"), ("tappedDepthV3", "8 mm"), ("tappedDepthV3", "30 mm")] {
        let old = base("20 mm", "5 mm", "15");
        let mut new = old.clone();
        new.set(field, expr(value));
        let native: Vec<(String, f64)> = link_tapped_depths(&old, &new, &units).into_iter().map(|(k, v)| (k.to_string(), num(&v))).collect();
        s.ps.features.retain(|f| f.type_id != "hole");
        s.add("hole", |pv| *pv = new.clone());
        let index = s.ps.features.len() - 1;
        let spec: Vec<String> = ["locations", "scope", "unitsSystem", "isoHoleTableEx", "holeDepthV3", "tappedDepthV3", "tapClearance"].iter().map(|x| x.to_string()).collect();
        let out = custom::std_feature::editing_logic(&s.ps, index, &ConfigValues::default(), &s.units, &RegenOptions::default(), &old, &new, &spec, false).expect("holeEditLogic ran");
        let std: Vec<(String, f64)> = ["holeDepthV3", "tappedDepthV3", "tapClearance"]
            .iter()
            .filter(|k| **k != field)
            .filter_map(|k| out.get(k).map(|v| (k.to_string(), num(v))))
            .filter(|(k, v)| (v - num(new.get(k).unwrap())).abs() > 1e-9)
            .collect();
        let close = native.len() == std.len() && native.iter().zip(&std).all(|(a, b)| a.0 == b.0 && (a.1 - b.1).abs() < 1e-9);
        assert!(close, "{field} = {value}: native {native:?}, std {std:?}");
    }
}

/// Two profiles for a loft: a 20 mm square on Top and a 10 mm one 30 mm
/// above (an offset plane); their sketches and largest regions.
fn loft_profiles(s: &mut Studio) -> [(FeatureId, Reference); 2] {
    let (a, ra) = s.sketch(Reference::Plane(DatumPlane::Top), &[SQUARE]);
    let plane = s.add("cPlane", |p| {
        p.set("entities", ParamValue::Query(vec![Reference::Plane(DatumPlane::Top)]));
        p.set("cplaneType", en("OFFSET"));
        p.set("offset", expr("30 mm"));
    });
    let small: &[(f64, f64)] = &[(15.0, 15.0), (25.0, 15.0), (25.0, 25.0), (15.0, 25.0)];
    let (b, rb) = s.sketch(Reference::FeaturePlane(plane), &[small]);
    [(a, ra[0].clone()), (b, rb[0].clone())]
}

fn profile_items(id: &str, refs: Vec<Vec<Reference>>) -> ParamValue {
    ParamValue::Array(
        refs.into_iter()
            .map(|r| {
                let mut v = ParamValues::default();
                v.set(id, ParamValue::Query(r));
                v
            })
            .collect(),
    )
}

/// Loft scenarios, native against std loft.fs.
fn loft_cases() -> Vec<(&'static str, Studio)> {
    let mut cases = Vec::new();
    let solid = |name: &'static str, set: fn(&mut ParamValues)| {
        let mut s = Studio::new();
        let [(_, a), (_, b)] = loft_profiles(&mut s);
        s.add("loft", |p| {
            p.set("sheetProfilesArray", profile_items("sheetProfileEntities", vec![vec![a], vec![b]]));
            set(p);
        });
        (name, s)
    };
    cases.push(solid("two squares", |_| {}));
    cases.push(solid("normal to the start profile", |p| {
        p.set("startCondition", en("NORMAL_TO_PROFILE"));
        p.set("startMagnitude", expr("1.5"));
    }));
    cases.push(solid("normal to both profiles", |p| {
        p.set("startCondition", en("NORMAL_TO_PROFILE"));
        p.set("endCondition", en("NORMAL_TO_PROFILE"));
    }));
    cases.push(solid("thin", |p| p.set("bodyType", en("THIN"))));
    cases.push(solid("path, nothing selected", |p| p.set("addSections", ParamValue::Bool(true))));
    cases.push(solid("guides, nothing selected", |p| p.set("addGuides", ParamValue::Bool(true))));
    let mut s = Studio::new();
    s.add("loft", |_| {});
    cases.push(("nothing selected", s));
    let mut s = Studio::new();
    let [(_, a), _] = loft_profiles(&mut s);
    s.add("loft", |p| p.set("sheetProfilesArray", profile_items("sheetProfileEntities", vec![vec![a]])));
    cases.push(("one profile", s));
    let mut s = Studio::new();
    let [(a, _), (b, _)] = loft_profiles(&mut s);
    let (ea, eb) = (s.sketch_edges(a), s.sketch_edges(b));
    s.add("loft", |p| {
        p.set("bodyType", en("SURFACE"));
        p.set("wireProfilesArray", profile_items("wireProfileEntities", vec![ea, eb]));
    });
    cases.push(("surface from sketch edges", s));
    cases
}

#[test]
#[ignore = "T2: oracle vs live (exact values, known-difference lists)"]
fn loft_regenerates_as_std() {
    // Known gap: loft start/end conditions. Our kernel loft has no end
    // derivatives; native warns that it ignores them, the host's opLoft
    // ignores them silently. Onshape applies them (geometry differs).
    // Re-check when psb's Loft end conditions (3c25e9c9, ready/psb) land.
    let gaps = ["normal to the start profile", "normal to both profiles"];
    let report: String = loft_cases().iter().filter(|(name, _)| only(name) && !gaps.contains(name)).filter_map(|(name, s)| same_as_std(name, "loft", s)).collect();
    assert!(report.is_empty(), "native Loft differs from std:\n{report}");
}

/// Transform scenarios on the 40 mm cube, native against std transformCopy.fs.
fn transform_cases() -> Vec<(&'static str, Studio)> {
    let mut cases = Vec::new();
    let on_cube = |name: &'static str, set: fn(&mut ParamValues, &RegenResult)| {
        let mut s = Studio::new();
        s.cube();
        let r = s.regen();
        s.add("transform", |p| {
            p.set("entities", ParamValue::Query(vec![body_ref(&r, 0)]));
            set(p, &r);
        });
        (name, s)
    };
    cases.push(on_cube("translate by distance, no direction", |p, _| p.set("transformType", en("TRANSLATION_DISTANCE"))));
    cases.push(on_cube("translate by distance along an edge", |p, r| {
        p.set("transformType", en("TRANSLATION_DISTANCE"));
        p.set("transformDirection", ParamValue::Query(vec![edge_at(r, DVec3::new(20.0, 0.0, 0.0))]));
        p.set("distance", expr("15 mm"));
    }));
    cases.push(on_cube("translate by line", |p, r| {
        p.set("transformType", en("TRANSLATION_ENTITY"));
        p.set("transformLine", ParamValue::Query(vec![edge_at(r, DVec3::new(0.0, 0.0, 20.0))]));
    }));
    cases.push(on_cube("translate by line, opposite, copy", |p, r| {
        p.set("transformType", en("TRANSLATION_ENTITY"));
        p.set("transformLine", ParamValue::Query(vec![edge_at(r, DVec3::new(0.0, 0.0, 20.0))]));
        p.set("oppositeDirectionEntity", ParamValue::Bool(true));
        p.set("makeCopy", ParamValue::Bool(true));
    }));
    cases.push(on_cube("translate by XYZ", |p, _| {
        p.set("transformType", en("TRANSLATION_3D"));
        p.set("dx", expr("10 mm"));
        p.set("dz", expr("-5 mm"));
    }));
    cases.push(on_cube("rotate about an edge", |p, r| {
        p.set("transformType", en("ROTATION"));
        p.set("transformAxis", ParamValue::Query(vec![edge_at(r, DVec3::new(0.0, 0.0, 20.0))]));
        p.set("angle", expr("45 deg"));
    }));
    cases.push(on_cube("rotate, no axis", |p, _| p.set("transformType", en("ROTATION"))));
    cases.push(on_cube("copy in place", |p, _| p.set("transformType", en("COPY"))));
    cases.push(on_cube("scale about a vertex", |p, r| {
        p.set("transformType", en("SCALE_UNIFORMLY"));
        p.set("scalePoint", ParamValue::Query(vec![vertex_at(r, DVec3::new(0.0, 0.0, 0.0))]));
        p.set("scale", expr("0.5"));
    }));
    cases.push(on_cube("scale non-uniformly", |p, r| {
        p.set("transformType", en("SCALE_UNIFORMLY"));
        p.set("uniform", ParamValue::Bool(false));
        p.set("scalePoint", ParamValue::Query(vec![vertex_at(r, DVec3::new(0.0, 0.0, 0.0))]));
        p.set("scaleX", expr("2"));
    }));
    cases.push(on_cube("scale, no point", |p, _| p.set("transformType", en("SCALE_UNIFORMLY"))));
    let mut s = Studio::new();
    s.cube();
    s.add("transform", |p| p.set("transformType", en("TRANSLATION_3D")));
    cases.push(("nothing selected", s));
    cases
}

#[test]
#[ignore = "T2: oracle vs live (exact values, known-difference lists)"]
fn transform_regenerates_as_std() {
    let report: String = transform_cases().iter().filter(|(name, _)| only(name)).filter_map(|(name, s)| same_as_std(name, "transform", s)).collect();
    assert!(report.is_empty(), "native Transform differs from std:\n{report}");
}

/// std's loftEditLogic: a new loft whose start profile lies on the cube's
/// top face adds to the cube; one in empty space stays New.
#[test]
#[ignore = "T2: oracle vs live (exact values, known-difference lists)"]
fn loft_editing_logic_runs_std() {
    let mut s = Studio::new();
    s.cube();
    let r = s.regen();
    let top = face_at(&r, DVec3::new(20.0, 20.0, 40.0));
    let cube = vec![body_ref(&r, 0)];
    let (_, a) = s.sketch(top, &[SQUARE]);
    let plane = s.add("cPlane", |p| {
        p.set("entities", ParamValue::Query(vec![Reference::Plane(DatumPlane::Top)]));
        p.set("cplaneType", en("OFFSET"));
        p.set("offset", expr("70 mm"));
    });
    let small: &[(f64, f64)] = &[(15.0, 15.0), (25.0, 15.0), (25.0, 25.0), (15.0, 25.0)];
    let (_, b) = s.sketch(Reference::FeaturePlane(plane), &[small]);
    let profiles = profile_items("sheetProfileEntities", vec![vec![a[0].clone()], vec![b[0].clone()]]);
    s.add("loft", |p| p.set("sheetProfilesArray", profiles.clone()));
    let got = edit_logic(&s, &ParamValues::default(), &["sheetProfilesArray"]);
    assert!(got.contains(&"operationType=Enum(\"ADD\")".to_string()) && got.contains(&format!("booleanScope={:?}", ParamValue::Query(cube))), "{got:?}");

    let mut s = Studio::new();
    let [(_, a), (_, b)] = loft_profiles(&mut s);
    s.add("loft", |p| p.set("sheetProfilesArray", profile_items("sheetProfileEntities", vec![vec![a], vec![b]])));
    let got = edit_logic(&s, &ParamValues::default(), &["sheetProfilesArray"]);
    assert!(!got.iter().any(|x| x.starts_with("operationType=")), "{got:?}");
}

/// An open sketch polyline (mm) on `plane`; its sketch and edges.
fn polyline(s: &mut Studio, plane: Reference, pts: &[(f64, f64)]) -> (FeatureId, Vec<Reference>) {
    let mut sk = noshape_sketch::Sketch::new();
    let p: Vec<_> = pts.iter().map(|&(x, y)| sk.add_point(DVec2::new(x * 1e-3, y * 1e-3))).collect();
    for w in p.windows(2) {
        sk.add_line_between(w[0], w[1]).unwrap();
    }
    let sketch = s.add("sketch", |pv| {
        pv.set("sketchPlane", ParamValue::Query(vec![plane]));
        pv.set("sketch", ParamValue::Sketch(Box::new(SketchData { sketch: sk, ..Default::default() })));
    });
    let edges = s.sketch_edges(sketch);
    (sketch, edges)
}

/// The 40 mm cube with feature `ty` after it, set from the regenerated cube.
fn on_the_cube(ty: &'static str, name: &'static str, set: fn(&mut ParamValues, &RegenResult)) -> (&'static str, Studio) {
    let mut s = Studio::new();
    s.cube();
    let r = s.regen();
    s.add(ty, |p| set(p, &r));
    (name, s)
}

fn q(r: Reference) -> ParamValue {
    ParamValue::Query(vec![r])
}

fn oracle(ty: &str, cases: Vec<(&'static str, Studio)>) {
    let report: String = cases.iter().filter(|(name, _)| only(name)).filter_map(|(name, s)| same_as_std(name, ty, s)).collect();
    assert!(report.is_empty(), "native {ty} differs from std:\n{report}");
}

const TOP_FACE: DVec3 = DVec3::new(20.0, 20.0, 40.0);
const BOTTOM_FACE: DVec3 = DVec3::new(20.0, 20.0, 0.0);
const SIDE_FACE: DVec3 = DVec3::new(0.0, 20.0, 20.0);
const Z_EDGE: DVec3 = DVec3::new(0.0, 0.0, 20.0);
const X_EDGE: DVec3 = DVec3::new(20.0, 0.0, 0.0);
const TOP_EDGE: DVec3 = DVec3::new(20.0, 0.0, 40.0);

#[test]
#[ignore = "T2: oracle vs live (exact values, known-difference lists)"]
fn revolve_regenerates_as_std() {
    let mut cases = Vec::new();
    let revolve = |name: &'static str, set: fn(&mut ParamValues, &RegenResult)| {
        let mut s = Studio::new();
        s.cube();
        let profile: &[(f64, f64)] = &[(50.0, 0.0), (70.0, 0.0), (70.0, 20.0), (50.0, 20.0)];
        let (_, regions) = s.sketch(Reference::Plane(DatumPlane::Front), &[profile]);
        let r = s.regen();
        let region = regions[0].clone();
        s.add("revolve", |p| {
            p.set("entities", q(region));
            set(p, &r);
        });
        (name, s)
    };
    cases.push(revolve("full", |p, r| p.set("axis", q(edge_at(r, Z_EDGE)))));
    cases.push(revolve("90 deg", |p, r| {
        p.set("axis", q(edge_at(r, Z_EDGE)));
        p.set("fullRevolve", ParamValue::Bool(false));
        p.set("angle", expr("90 deg"));
    }));
    cases.push(revolve("no axis", |_, _| {}));
    cases.push(revolve("thin", |p, r| {
        p.set("axis", q(edge_at(r, Z_EDGE)));
        p.set("bodyType", en("THIN"));
    }));
    let mut s = Studio::new();
    s.add("revolve", |_| {});
    cases.push(("nothing selected", s));
    oracle("revolve", cases);
}

#[test]
#[ignore = "T2: oracle vs live (exact values, known-difference lists)"]
fn chamfer_regenerates_as_std() {
    oracle(
        "chamfer",
        vec![
            on_the_cube("chamfer", "nothing selected", |_, _| {}),
            on_the_cube("chamfer", "one edge", |p, r| p.set("entities", q(edge_at(r, TOP_EDGE)))),
            on_the_cube("chamfer", "a face", |p, r| p.set("entities", q(face_at(r, TOP_FACE)))),
            on_the_cube("chamfer", "two offsets", |p, r| {
                p.set("entities", q(edge_at(r, TOP_EDGE)));
                p.set("chamferType", en("TWO_OFFSETS"));
                p.set("width1", expr("3 mm"));
                p.set("width2", expr("6 mm"));
            }),
            on_the_cube("chamfer", "offset and angle", |p, r| {
                p.set("entities", q(edge_at(r, TOP_EDGE)));
                p.set("chamferType", en("OFFSET_ANGLE"));
                p.set("angle", expr("30 deg"));
            }),
            on_the_cube("chamfer", "too wide", |p, r| {
                p.set("entities", q(face_at(r, TOP_FACE)));
                p.set("width", expr("30 mm"));
            }),
        ],
    );
}

#[test]
#[ignore = "T2: oracle vs live (exact values, known-difference lists)"]
fn shell_regenerates_as_std() {
    oracle(
        "shell",
        vec![
            on_the_cube("shell", "nothing selected", |_, _| {}),
            on_the_cube("shell", "open top", |p, r| {
                p.set("entities", q(face_at(r, TOP_FACE)));
                p.set("thickness", expr("2 mm"));
            }),
            on_the_cube("shell", "outside", |p, r| {
                p.set("entities", q(face_at(r, TOP_FACE)));
                p.set("thickness", expr("2 mm"));
                p.set("oppositeDirection", ParamValue::Bool(true));
            }),
            on_the_cube("shell", "hollow", |p, r| {
                p.set("isHollow", ParamValue::Bool(true));
                p.set("parts", q(body_ref(r, 0)));
                p.set("thickness", expr("2 mm"));
            }),
            on_the_cube("shell", "hollow, no part", |p, _| p.set("isHollow", ParamValue::Bool(true))),
            ("open top of a 20 mm cube, 1.5 mm (std_reference shellTop, live 2653.5 mm³)", {
                let mut s = Studio::new();
                s.add("cube", |p| p.set("sideLength", expr("20 mm")));
                let r = s.regen();
                s.add("shell", |p| {
                    p.set("entities", q(face_at(&r, DVec3::new(10.0, 10.0, 20.0))));
                    p.set("thickness", expr("1.5 mm"));
                });
                s
            }),
            on_the_cube("shell", "too thick", |p, r| {
                p.set("entities", q(face_at(r, TOP_FACE)));
                p.set("thickness", expr("25 mm"));
            }),
        ],
    );
}

#[test]
#[ignore = "T2: oracle vs live (exact values, known-difference lists)"]
fn patterns_regenerate_as_std() {
    oracle(
        "linearPattern",
        vec![
            on_the_cube("linearPattern", "nothing selected", |_, _| {}),
            on_the_cube("linearPattern", "no direction", |p, r| p.set("entities", q(body_ref(r, 0)))),
            on_the_cube("linearPattern", "three along X", |p, r| {
                p.set("entities", q(body_ref(r, 0)));
                p.set("directionOne", q(edge_at(r, X_EDGE)));
                p.set("distance", expr("50 mm"));
                p.set("instanceCount", expr("3"));
            }),
            on_the_cube("linearPattern", "two directions, centered, skip", |p, r| {
                p.set("entities", q(body_ref(r, 0)));
                p.set("directionOne", q(edge_at(r, X_EDGE)));
                p.set("distance", expr("50 mm"));
                p.set("instanceCount", expr("3"));
                p.set("isCentered", ParamValue::Bool(true));
                p.set("hasSecondDir", ParamValue::Bool(true));
                p.set("directionTwo", q(edge_at(r, DVec3::new(0.0, 20.0, 0.0))));
                p.set("distanceTwo", expr("60 mm"));
                p.set("instanceCountTwo", expr("2"));
                p.set("skipInstances", ParamValue::Bool(true));
                let mut skip = ParamValues::default();
                skip.set("index1", expr("1"));
                skip.set("index2", expr("1"));
                p.set("skippedInstances", ParamValue::Array(vec![skip]));
            }),
        ],
    );
    oracle(
        "circularPattern",
        vec![
            on_the_cube("circularPattern", "no axis", |p, r| p.set("entities", q(body_ref(r, 0)))),
            on_the_cube("circularPattern", "four around an edge", |p, r| {
                p.set("entities", q(body_ref(r, 0)));
                p.set("axis", q(edge_at(r, Z_EDGE)));
                p.set("instanceCount", expr("4"));
                p.set("equalSpace", ParamValue::Bool(true));
                p.set("angle", expr("360 deg"));
            }),
            on_the_cube("circularPattern", "three at 100 deg", |p, r| {
                p.set("entities", q(body_ref(r, 0)));
                p.set("axis", q(edge_at(r, Z_EDGE)));
                p.set("instanceCount", expr("3"));
                p.set("angle", expr("100 deg"));
            }),
        ],
    );
}

#[test]
#[ignore = "T2: oracle vs live (exact values, known-difference lists)"]
fn mirror_regenerates_as_std() {
    oracle(
        "mirror",
        vec![
            on_the_cube("mirror", "nothing selected", |_, _| {}),
            on_the_cube("mirror", "no plane", |p, r| p.set("entities", q(body_ref(r, 0)))),
            on_the_cube("mirror", "about Right", |p, r| {
                p.set("entities", q(body_ref(r, 0)));
                p.set("mirrorPlane", q(Reference::Plane(DatumPlane::Right)));
            }),
            on_the_cube("mirror", "about its own face", |p, r| {
                p.set("entities", q(body_ref(r, 0)));
                p.set("mirrorPlane", q(face_at(r, TOP_FACE)));
            }),
        ],
    );
}

#[test]
#[ignore = "T2: oracle vs live (exact values, known-difference lists)"]
fn sweep_regenerates_as_std() {
    let mut cases = Vec::new();
    let sweep = |name: &'static str, set: fn(&mut ParamValues)| {
        let mut s = Studio::new();
        let square: &[(f64, f64)] = &[(-3.0, -3.0), (3.0, -3.0), (3.0, 3.0), (-3.0, 3.0)];
        let (_, regions) = s.sketch(Reference::Plane(DatumPlane::Front), &[square]);
        let (_, path) = polyline(&mut s, Reference::Plane(DatumPlane::Top), &[(0.0, 0.0), (0.0, 50.0), (40.0, 80.0)]);
        let region = regions[0].clone();
        s.add("sweep", |p| {
            p.set("profiles", q(region));
            p.set("path", ParamValue::Query(path));
            set(p);
        });
        (name, s)
    };
    cases.push(sweep("along a polyline", |_| {}));
    cases.push(sweep("keep orientation", |p| p.set("profileControl", en("KEEP_ORIENTATION"))));
    cases.push(sweep("thin", |p| p.set("bodyType", en("THIN"))));
    let mut s = Studio::new();
    s.add("sweep", |_| {});
    cases.push(("nothing selected", s));
    oracle("sweep", cases);
}

#[test]
#[ignore = "T2: oracle vs live (exact values, known-difference lists)"]
fn boolean_regenerates_as_std() {
    let mut cases = Vec::new();
    let two = |name: &'static str, set: fn(&mut ParamValues, &RegenResult)| {
        let mut s = Studio::new();
        s.cube();
        let (_, regions) = s.sketch(Reference::Plane(DatumPlane::Top), &[SQUARE]);
        let region = regions[0].clone();
        s.add("extrude", |p| {
            p.set("entities", q(region));
            p.set("depth", expr("60 mm"));
        });
        let r = s.regen();
        s.add("booleanBodies", |p| set(p, &r));
        (name, s)
    };
    cases.push(two("nothing selected", |_, _| {}));
    cases.push(two("union", |p, r| p.set("tools", ParamValue::Query(vec![body_ref(r, 0), body_ref(r, 1)]))));
    cases.push(two("union of one part", |p, r| p.set("tools", q(body_ref(r, 0)))));
    cases.push(two("subtract", |p, r| {
        p.set("operationType", en("SUBTRACTION"));
        p.set("targets", q(body_ref(r, 0)));
        p.set("tools", q(body_ref(r, 1)));
    }));
    cases.push(two("subtract, no targets", |p, r| {
        p.set("operationType", en("SUBTRACTION"));
        p.set("tools", q(body_ref(r, 1)));
    }));
    cases.push(two("subtract, keep tools", |p, r| {
        p.set("operationType", en("SUBTRACTION"));
        p.set("targets", q(body_ref(r, 0)));
        p.set("tools", q(body_ref(r, 1)));
        p.set("keepTools", ParamValue::Bool(true));
    }));
    cases.push(two("intersect", |p, r| {
        p.set("operationType", en("INTERSECTION"));
        p.set("tools", ParamValue::Query(vec![body_ref(r, 0), body_ref(r, 1)]));
    }));
    oracle("booleanBodies", cases);
}

#[test]
#[ignore = "T2: oracle vs live (exact values, known-difference lists)"]
fn plane_regenerates_as_std() {
    oracle(
        "cPlane",
        vec![
            on_the_cube("cPlane", "nothing selected", |_, _| {}),
            on_the_cube("cPlane", "offset from Top", |p, _| {
                p.set("entities", q(Reference::Plane(DatumPlane::Top)));
                p.set("offset", expr("20 mm"));
            }),
            on_the_cube("cPlane", "offset from a face, flipped", |p, r| {
                p.set("entities", q(face_at(r, TOP_FACE)));
                p.set("offset", expr("5 mm"));
                p.set("oppositeDirection", ParamValue::Bool(true));
            }),
            on_the_cube("cPlane", "mid plane of two faces", |p, r| {
                p.set("cplaneType", en("MID_PLANE"));
                p.set("entities", ParamValue::Query(vec![face_at(r, TOP_FACE), face_at(r, BOTTOM_FACE)]));
            }),
            on_the_cube("cPlane", "three points", |p, r| {
                p.set("cplaneType", en("THREE_POINT"));
                p.set("entities", ParamValue::Query(vec![vertex_at(r, DVec3::ZERO), vertex_at(r, DVec3::new(40.0, 0.0, 0.0)), vertex_at(r, DVec3::new(0.0, 40.0, 40.0))]));
            }),
            on_the_cube("cPlane", "three points, two given", |p, r| {
                p.set("cplaneType", en("THREE_POINT"));
                p.set("entities", ParamValue::Query(vec![vertex_at(r, DVec3::ZERO), vertex_at(r, DVec3::new(40.0, 0.0, 0.0))]));
            }),
            on_the_cube("cPlane", "plane point", |p, r| {
                p.set("cplaneType", en("PLANE_POINT"));
                p.set("entities", ParamValue::Query(vec![Reference::Plane(DatumPlane::Top), vertex_at(r, DVec3::new(40.0, 40.0, 40.0))]));
            }),
            on_the_cube("cPlane", "line angle", |p, r| {
                p.set("cplaneType", en("LINE_ANGLE"));
                p.set("entities", q(edge_at(r, Z_EDGE)));
                p.set("angle", expr("30 deg"));
            }),
        ],
    );
}

#[test]
#[ignore = "T2: oracle vs live (exact values, known-difference lists)"]
fn draft_regenerates_as_std() {
    oracle(
        "draft",
        vec![
            on_the_cube("draft", "nothing selected", |_, _| {}),
            on_the_cube("draft", "no faces", |p, r| p.set("neutralPlane", q(face_at(r, BOTTOM_FACE)))),
            on_the_cube("draft", "one side from the bottom", |p, r| {
                p.set("neutralPlane", q(face_at(r, BOTTOM_FACE)));
                p.set("draftFaces", q(face_at(r, SIDE_FACE)));
                p.set("angle", expr("5 deg"));
            }),
            on_the_cube("draft", "one side, other pull", |p, r| {
                p.set("neutralPlane", q(face_at(r, BOTTOM_FACE)));
                p.set("draftFaces", q(face_at(r, SIDE_FACE)));
                p.set("angle", expr("5 deg"));
                p.set("pullDirection", ParamValue::Bool(true));
            }),
        ],
    );
}

#[test]
#[ignore = "T2: oracle vs live (exact values, known-difference lists)"]
fn split_regenerates_as_std() {
    let mut cases = Vec::new();
    let split = |name: &'static str, set: fn(&mut ParamValues, &RegenResult, FeatureId)| {
        let mut s = Studio::new();
        s.cube();
        let plane = s.add("cPlane", |p| {
            p.set("entities", q(Reference::Plane(DatumPlane::Top)));
            p.set("offset", expr("15 mm"));
        });
        let r = s.regen();
        s.add("splitPart", |p| set(p, &r, plane));
        (name, s)
    };
    cases.push(split("nothing selected", |_, _, _| {}));
    cases.push(split("no tool", |p, r, _| p.set("targets", q(body_ref(r, 0)))));
    cases.push(split("by a plane", |p, r, plane| {
        p.set("targets", q(body_ref(r, 0)));
        p.set("tool", q(Reference::FeaturePlane(plane)));
    }));
    cases.push(split("by a plane, keep the front", |p, r, plane| {
        p.set("targets", q(body_ref(r, 0)));
        p.set("tool", q(Reference::FeaturePlane(plane)));
        p.set("keepBothSides", ParamValue::Bool(false));
    }));
    // Live "fs (10-25 06:15)": an info, the part unchanged.
    cases.push(split("by a plane that misses", |p, r, _| {
        p.set("targets", q(body_ref(r, 0)));
        p.set("tool", q(Reference::Plane(DatumPlane::Top)));
    }));
    oracle("splitPart", cases);
}

/// std's editing logic of Split, Plane, Revolve and Mirror (run as the
/// dialog runs it).
#[test]
#[ignore = "T2: oracle vs live (exact values, known-difference lists)"]
fn more_editing_logic_runs_std() {
    // Split: a plane tool turns Keep tools on (live "Split part by the Right plane").
    let mut s = Studio::new();
    s.cube();
    let r = s.regen();
    s.add("splitPart", |p| {
        p.set("targets", q(body_ref(&r, 0)));
        p.set("tool", q(Reference::Plane(DatumPlane::Right)));
    });
    let got = edit_logic(&s, &ParamValues::default(), &["targets", "tool"]);
    assert!(got.contains(&"keepTools=Bool(true)".to_string()), "{got:?}");

    // Plane: three vertices make a Three point plane, a line a Mid plane.
    let mut s = Studio::new();
    s.cube();
    let r = s.regen();
    s.add("cPlane", |p| p.set("entities", ParamValue::Query(vec![vertex_at(&r, DVec3::ZERO), vertex_at(&r, DVec3::new(40.0, 0.0, 0.0)), vertex_at(&r, DVec3::new(0.0, 40.0, 40.0))])));
    let got = edit_logic(&s, &ParamValues::default(), &["entities"]);
    assert!(got.contains(&"cplaneType=Enum(\"THREE_POINT\")".to_string()), "{got:?}");
    s.ps.features.pop();
    s.add("cPlane", |p| p.set("entities", q(edge_at(&r, X_EDGE))));
    let got = edit_logic(&s, &ParamValues::default(), &["entities"]);
    assert!(got.contains(&"cplaneType=Enum(\"MID_PLANE\")".to_string()), "{got:?}");

    // Mirror: the cube mirrored about its top face sits on it: Add.
    let mut s = Studio::new();
    s.cube();
    let r = s.regen();
    s.add("mirror", |p| {
        p.set("entities", q(body_ref(&r, 0)));
        p.set("mirrorPlane", q(face_at(&r, TOP_FACE)));
    });
    let got = edit_logic(&s, &ParamValues::default(), &["entities", "mirrorPlane"]);
    assert!(got.contains(&"operationType=Enum(\"ADD\")".to_string()), "{got:?}");

    // Revolve: a profile against the cube's side revolved about its edge: Add.
    let mut s = Studio::new();
    s.cube();
    let profile: &[(f64, f64)] = &[(40.0, 0.0), (60.0, 0.0), (60.0, 20.0), (40.0, 20.0)];
    let (_, regions) = s.sketch(Reference::Plane(DatumPlane::Front), &[profile]);
    let r = s.regen();
    s.add("revolve", |p| {
        p.set("entities", q(regions[0].clone()));
        p.set("axis", q(edge_at(&r, DVec3::new(40.0, 0.0, 20.0))));
    });
    let got = edit_logic(&s, &ParamValues::default(), &["entities", "axis"]);
    assert!(got.contains(&"operationType=Enum(\"ADD\")".to_string()), "{got:?}");
}

/// std's holeEditLogic through the host natives: a new hole's Merge scope is
/// the part it reaches, and one sketched below the part flips into it
/// (holeScopeFlipHeuristics: an opHole probe and evCollision containment,
/// live "fs (10-25 f)"; the native dialog logic does the same, live L57).
#[test]
#[ignore = "T2: oracle vs live (exact values, known-difference lists)"]
fn hole_editing_logic_runs_std() {
    for on_top in [true, false] {
        let mut s = Studio::new();
        s.cube();
        let r = s.regen();
        let plane = if on_top { face_at(&r, DVec3::new(20.0, 20.0, 40.0)) } else { Reference::Plane(DatumPlane::Top) };
        let mut sk = noshape_sketch::Sketch::new();
        let p = sk.add_point(DVec2::new(0.02, 0.02));
        let sketch = s.add("sketch", |pv| {
            pv.set("sketchPlane", ParamValue::Query(vec![plane]));
            pv.set("sketch", ParamValue::Sketch(Box::new(SketchData { sketch: sk, ..Default::default() })));
        });
        s.add("hole", |pv| {
            pv.set("locations", ParamValue::Query(vec![Reference::SketchEntity { sketch, entity: p.0 }]));
            pv.set("unitsSystem", en("METRIC"));
            noshape_model::features::hole_tables::apply_table_to_params(pv);
        });
        let got = edit_logic(&s, &ParamValues::default(), &["locations"]);
        let scope = format!("scope={:?}", ParamValue::Query(vec![body_ref(&r, 0)]));
        let want = if on_top { vec![scope] } else { vec!["oppositeDirection=Bool(true)".to_string(), scope] };
        assert_eq!(got, want, "hole sketched {}", if on_top { "on the top face" } else { "below the part" });
    }
}

/// Live L57 through std's holeEditLogic: two stacked parts, a hole on the
/// top one's face; Blind 5 fills Merge scope with the top part only, Depth
/// 15 adds the part below.
#[test]
#[ignore = "T2: oracle vs live (exact values, known-difference lists)"]
fn hole_scope_follows_depth_as_live() {
    let mut s = Studio::new();
    let a = s.sketch(Reference::Plane(DatumPlane::Top), &[SQUARE]).1[0].clone();
    s.add("extrude", |p| {
        p.set("entities", ParamValue::Query(vec![a]));
        p.set("depth", expr("10 mm"));
    });
    let r = s.regen();
    let b = s.sketch(face_at(&r, DVec3::new(20.0, 20.0, 10.0)), &[SQUARE]).1[0].clone();
    s.add("extrude", |p| {
        p.set("entities", ParamValue::Query(vec![b]));
        p.set("depth", expr("10 mm"));
        p.set("operationType", en("NEW"));
    });
    let r = s.regen();
    assert_eq!(r.context.bodies.len(), 2);
    let (lower, upper) = (body_ref(&r, 0), body_ref(&r, 1));
    let mut sk = noshape_sketch::Sketch::new();
    let p = sk.add_point(DVec2::new(0.02, 0.02));
    let sketch = s.add("sketch", |pv| {
        pv.set("sketchPlane", ParamValue::Query(vec![face_at(&r, DVec3::new(20.0, 20.0, 20.0))]));
        pv.set("sketch", ParamValue::Sketch(Box::new(SketchData { sketch: sk, ..Default::default() })));
    });
    s.add("hole", |pv| {
        pv.set("locations", ParamValue::Query(vec![Reference::SketchEntity { sketch, entity: p.0 }]));
        pv.set("unitsSystem", en("METRIC"));
        noshape_model::features::hole_tables::apply_table_to_params(pv);
    });
    // As the dialog: each run's changes applied before the next edit.
    let last = s.ps.features.len() - 1;
    let run = |s: &mut Studio, old: &ParamValues, specified: &[&str]| {
        let new = s.ps.features[last].params.clone();
        let spec: Vec<String> = specified.iter().map(|x| x.to_string()).collect();
        let out = custom::std_feature::editing_logic(&s.ps, last, &ConfigValues::default(), &s.units, &RegenOptions::default(), old, &new, &spec, true).expect("editing logic ran");
        for (k, v) in out.0 {
            s.ps.features[last].params.set(&k, v);
        }
        s.ps.features[last].params.query("scope").to_vec()
    };
    run(&mut s, &ParamValues::default(), &["locations"]);
    for (depth, want) in [("5 mm", vec![upper.clone()]), ("15 mm", vec![lower.clone(), upper.clone()])] {
        let old = s.ps.features[last].params.clone();
        s.ps.features[last].params.set("holeDepthV3", expr(depth));
        let got = run(&mut s, &old, &["locations", "holeDepthV3"]);
        assert!(got.len() == want.len() && want.iter().all(|w| got.contains(w)), "Merge scope at depth {depth}: {got:?}");
    }
}
