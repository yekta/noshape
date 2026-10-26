//! P2 assemblies mate by mate against live Onshape (fs oracle_asm, lead
//! 10-26): each case of /root/onshape-research/live/fs-oracle-asm/CASES.md
//! built here as live builds it (A fixed, B moved to a start pose, then the
//! mates on implicit connectors) and compared with live's mate status and
//! text, every instance's transform and the mates' free values
//! (live/fs-oracle-asm/results-*.jsonl).

use glam::{DMat4, DQuat, DVec2, DVec3};
use noshape_model::assembly::eval::{AssemblyEvaluator, AssemblyResult, SolveOptions};
use noshape_model::assembly::{Assembly, Instance, InstanceId, InstancePattern, InstanceSource, Limits, Mate, MateConnectorRef, MateId, MateType, PatternKind, Placement};
use noshape_model::configuration::ConfigValues;
use noshape_model::document::{Document, DocumentSettings, TabContent, TabId};
use noshape_model::params::ParamValue;
use noshape_model::partstudio::PartStudio;
use noshape_model::reference::{BodyId, DatumPlane, Inference, Reference};
use noshape_model::regen::{RegenOptions, RegenResult, Regenerator};
use noshape_model::sketch_feature::SketchData;
use noshape_units::{ExprValue, UnitPrefs};
use std::sync::Arc;

fn mm2(x: f64, y: f64) -> DVec2 {
    DVec2::new(x * 1e-3, y * 1e-3)
}

/// A Part Studio of one sketch on Top extruded `depth`: the region `pick`
/// chooses (largest when true).
fn studio(draw: impl FnOnce(&mut noshape_sketch::Sketch), depth: &str, largest: bool) -> PartStudio {
    let units = UnitPrefs::default();
    let mut sk = noshape_sketch::Sketch::new();
    draw(&mut sk);
    let mut ps = PartStudio::new();
    let mut sketch = ps.new_feature("sketch", &units).unwrap();
    sketch.params.set("sketchPlane", ParamValue::Query(vec![Reference::Plane(DatumPlane::Top)]));
    sketch.params.set("sketch", ParamValue::Sketch(Box::new(SketchData { sketch: sk, ..Default::default() })));
    let sid = sketch.id;
    ps.features.push(sketch);
    let r = Regenerator::new().regenerate(&ps, &ConfigValues::default(), &units, &RegenOptions::default());
    let regions = &r.display.sketches[0].regions;
    let pick = if largest { regions.iter().max_by(|a, b| a.area.total_cmp(&b.area)) } else { regions.iter().min_by(|a, b| a.area.total_cmp(&b.area)) }.unwrap();
    let mut ex = ps.new_feature("extrude", &units).unwrap();
    ex.params.set("entities", ParamValue::Query(vec![Reference::SketchRegion { sketch: sid, key: pick.key.clone() }]));
    ex.params.set("depth", ParamValue::Expr(ExprValue::new(depth)));
    ps.features.push(ex);
    ps
}

fn rect(sk: &mut noshape_sketch::Sketch, a: (f64, f64), b: (f64, f64)) {
    let p = [mm2(a.0, a.1), mm2(b.0, a.1), mm2(b.0, b.1), mm2(a.0, b.1)];
    let pts: Vec<_> = p.iter().map(|q| sk.add_point(*q)).collect();
    for i in 0..4 {
        sk.add_line_between(pts[i], pts[(i + 1) % 4]).unwrap();
    }
}

/// BLOCK x 0..40, y 0..20, z 0..10; PLATE x 100..160, y 0..40, z 0..10 with
/// a Ø10 hole at (130, 20); PIN Ø10 at (200, 0), z 0..30 (CASES.md).
struct Parts {
    doc: Document,
    ev: AssemblyEvaluator,
    tabs: [TabId; 3],
    bodies: [BodyId; 3],
    results: [Arc<RegenResult>; 3],
}

const BLOCK: usize = 0;
const PLATE: usize = 1;
const PIN: usize = 2;

fn parts() -> Parts {
    let block = studio(|sk| rect(sk, (0.0, 0.0), (40.0, 20.0)), "10 mm", true);
    let plate = studio(
        |sk| {
            rect(sk, (100.0, 0.0), (160.0, 40.0));
            sk.add_circle(mm2(130.0, 20.0), 0.005);
        },
        "10 mm",
        true,
    );
    let pin = studio(
        |sk| {
            sk.add_circle(mm2(200.0, 0.0), 0.005);
        },
        "30 mm",
        true,
    );
    let mut doc = Document { settings: DocumentSettings::default(), tabs: Vec::new() };
    let tabs = [doc.add_tab("BLOCK", TabContent::PartStudio(block)), doc.add_tab("PLATE", TabContent::PartStudio(plate)), doc.add_tab("PIN", TabContent::PartStudio(pin))];
    let mut ev = AssemblyEvaluator::new();
    let results = tabs.map(|t| ev.studio(&doc, t, &ConfigValues::default()).unwrap());
    let bodies = [0, 1, 2].map(|i| results[i].context.bodies[0].id.clone());
    Parts { doc, ev, tabs, bodies, results }
}

/// The face / edge / vertex of part `p` through the point (mm).
fn entity(parts: &Parts, p: usize, at: DVec3, kind: noshape_kernel::ShapeType) -> Reference {
    let b = &parts.results[p].context.bodies[0];
    let v = noshape_kernel::make_vertex(at * 1e-3).unwrap();
    let subs = match kind {
        noshape_kernel::ShapeType::Face => b.shape.faces(),
        noshape_kernel::ShapeType::Edge => b.shape.edges(),
        _ => b.shape.vertices(),
    };
    for (i, s) in subs.iter().enumerate() {
        if noshape_kernel::distance(&v, s).is_ok_and(|d| d.distance < 1e-9) {
            return match kind {
                noshape_kernel::ShapeType::Face => Reference::Face((*b.faces[i]).clone()),
                noshape_kernel::ShapeType::Edge => Reference::Edge((*b.edges[i]).clone()),
                _ => Reference::Vertex((*b.vertices[i]).clone()),
            };
        }
    }
    panic!("no {kind:?} at {at:?} on part {p}");
}

fn face(parts: &Parts, p: usize, x: f64, y: f64, z: f64) -> Reference {
    entity(parts, p, DVec3::new(x, y, z), noshape_kernel::ShapeType::Face)
}
fn edge(parts: &Parts, p: usize, x: f64, y: f64, z: f64) -> Reference {
    entity(parts, p, DVec3::new(x, y, z), noshape_kernel::ShapeType::Edge)
}
fn vertex(parts: &Parts, p: usize, x: f64, y: f64, z: f64) -> Reference {
    entity(parts, p, DVec3::new(x, y, z), noshape_kernel::ShapeType::Vertex)
}

fn conn(instance: u64, on: Reference, inference: Inference) -> MateConnectorRef {
    MateConnectorRef::on(InstanceId(instance), vec![], Reference::Implicit { on: Box::new(on), inference })
}

/// Rotate `deg` about +z, then translate (mm): live's occurrence transform.
fn pose(deg: f64, t: (f64, f64, f64)) -> Placement {
    let q = DQuat::from_rotation_z(deg.to_radians());
    Placement { translation: [t.0 * 1e-3, t.1 * 1e-3, t.2 * 1e-3], rotation: [q.x, q.y, q.z, q.w] }
}

fn instance(parts: &Parts, id: u64, name: &str, p: usize, at: Placement, fixed: bool) -> Instance {
    let mut i = Instance::new(InstanceId(id), name.into(), InstanceSource::Part { studio: parts.tabs[p], body: parts.bodies[p].clone(), config: ConfigValues::default() }, at);
    i.fixed = fixed;
    i
}

fn mate(id: u64, t: MateType, a: MateConnectorRef, b: MateConnectorRef) -> Mate {
    Mate::new(MateId(id), format!("{} {}", t.name(), id - 9), t, a, b)
}

fn limit(key: &str, min: Option<&str>, max: Option<&str>) -> (String, Limits) {
    (key.into(), Limits { min: min.map(ExprValue::new), max: max.map(ExprValue::new) })
}

/// The two blocks of the F, S, P, B and M cases: A fixed at identity, B at
/// `start`.
fn two(parts: &Parts, a: usize, b: usize, start: Placement) -> Assembly {
    Assembly { instances: vec![instance(parts, 1, "A", a, Placement::default(), true), instance(parts, 2, "B", b, start, false)], ..Default::default() }
}

/// B's bottom face (z 0) on A's top face (z 10), both CENTROID.
fn stack(parts: &Parts) -> (MateConnectorRef, MateConnectorRef) {
    (conn(2, face(parts, BLOCK, 20.0, 10.0, 0.0), Inference::Centroid), conn(1, face(parts, BLOCK, 20.0, 10.0, 10.0), Inference::Centroid))
}

fn cases(parts: &Parts) -> Vec<(&'static str, Assembly)> {
    let mut out = Vec::new();
    let f1 = pose(30.0, (100.0, 50.0, 30.0));
    let fastened = |set: &dyn Fn(&mut Mate)| {
        let mut asm = two(parts, BLOCK, BLOCK, f1);
        let (a, b) = stack(parts);
        let mut m = mate(10, MateType::Fastened, a, b);
        set(&mut m);
        asm.mates.push(m);
        asm
    };
    out.push(("F1", fastened(&|_| {})));
    out.push((
        "F2",
        fastened(&|m| {
            m.offset_z = Some(ExprValue::new("5 mm"));
            m.rotation = Some(ExprValue::new("90 deg"));
            m.rotation_axis = 2;
        }),
    ));
    out.push(("F3", fastened(&|m| m.flip_primary = true)));
    let mut asm = two(parts, BLOCK, BLOCK, f1);
    asm.mates.push(mate(10, MateType::Fastened, conn(2, vertex(parts, BLOCK, 0.0, 0.0, 0.0), Inference::Vertex), conn(1, vertex(parts, BLOCK, 40.0, 20.0, 10.0), Inference::Vertex)));
    out.push(("F4", asm));
    let mut asm = two(parts, BLOCK, BLOCK, f1);
    asm.mates.push(mate(10, MateType::Fastened, conn(2, edge(parts, BLOCK, 20.0, 0.0, 0.0), Inference::Midpoint), conn(1, edge(parts, BLOCK, 20.0, 20.0, 10.0), Inference::Midpoint)));
    out.push(("F5", asm));
    // Pin in the plate's hole.
    let r1 = pose(0.0, (-50.0, 60.0, 40.0));
    let pin_edges = |r: Placement, t: MateType, set: &dyn Fn(&mut Mate)| {
        let mut asm = two(parts, PLATE, PIN, r);
        let mut m = mate(10, t, conn(2, edge(parts, PIN, 205.0, 0.0, 0.0), Inference::Center), conn(1, edge(parts, PLATE, 135.0, 20.0, 10.0), Inference::Center));
        set(&mut m);
        asm.mates.push(m);
        asm
    };
    out.push(("R1", pin_edges(r1, MateType::Revolute, &|_| {})));
    out.push(("R2", pin_edges(pose(90.0, (-50.0, 60.0, 40.0)), MateType::Revolute, &|m| m.limits = vec![limit("rz", Some("0 deg"), Some("45 deg"))])));
    let axes = |t: MateType| {
        let mut asm = two(parts, PLATE, PIN, r1);
        asm.mates.push(mate(10, t, conn(2, face(parts, PIN, 205.0, 0.0, 15.0), Inference::MidAxis), conn(1, face(parts, PLATE, 135.0, 20.0, 5.0), Inference::MidAxis)));
        asm
    };
    out.push(("R3", axes(MateType::Revolute)));
    let stacked = |t: MateType, start: Placement, set: &dyn Fn(&mut Mate)| {
        let mut asm = two(parts, BLOCK, BLOCK, start);
        let (a, b) = stack(parts);
        let mut m = mate(10, t, a, b);
        set(&mut m);
        asm.mates.push(m);
        asm
    };
    let s1 = pose(0.0, (5.0, 30.0, 25.0));
    out.push(("S1", stacked(MateType::Slider, s1, &|_| {})));
    out.push(("S2", stacked(MateType::Slider, s1, &|m| m.limits = vec![limit("z", Some("0 mm"), Some("10 mm"))])));
    out.push(("C1", axes(MateType::Cylindrical)));
    out.push(("P1", stacked(MateType::Planar, pose(20.0, (5.0, 7.0, 30.0)), &|_| {})));
    let ball = |set: &dyn Fn(&mut Mate)| {
        let mut asm = two(parts, BLOCK, BLOCK, pose(0.0, (60.0, 30.0, 20.0)));
        let mut m = mate(10, MateType::Ball, conn(2, vertex(parts, BLOCK, 0.0, 0.0, 0.0), Inference::Vertex), conn(1, vertex(parts, BLOCK, 40.0, 20.0, 10.0), Inference::Vertex));
        set(&mut m);
        asm.mates.push(m);
        asm
    };
    out.push(("B1", ball(&|_| {})));
    out.push(("B2", ball(&|m| m.limits = vec![limit("cone", None, Some("30 deg"))])));
    // Revolute + Slider on the same pair.
    let mut asm = two(parts, BLOCK, BLOCK, f1);
    let (a, b) = stack(parts);
    asm.mates.push(mate(10, MateType::Revolute, a, b));
    asm.mates.push(mate(11, MateType::Slider, conn(2, face(parts, BLOCK, 20.0, 0.0, 5.0), Inference::Centroid), conn(1, face(parts, BLOCK, 20.0, 0.0, 5.0), Inference::Centroid)));
    out.push(("M1", asm));
    // A chain: B fastened on A, C turning on B.
    let mut asm = two(parts, BLOCK, BLOCK, f1);
    asm.instances.push(instance(parts, 3, "C", BLOCK, pose(0.0, (0.0, 0.0, 40.0)), false));
    let (a, b) = stack(parts);
    asm.mates.push(mate(10, MateType::Fastened, a, b));
    asm.mates.push(mate(11, MateType::Revolute, conn(3, face(parts, BLOCK, 20.0, 10.0, 0.0), Inference::Centroid), conn(2, face(parts, BLOCK, 20.0, 10.0, 10.0), Inference::Centroid)));
    out.push(("M2", asm));
    // Patterns.
    let mut asm = pin_edges(r1, MateType::Fastened, &|_| {});
    asm.patterns.push(InstancePattern {
        id: MateId(20),
        name: "Linear pattern 1".into(),
        kind: PatternKind::Linear { direction: conn(1, edge(parts, PLATE, 130.0, 0.0, 10.0), Inference::Midpoint).linear_direction(), distance: ExprValue::new("15 mm"), count: 3, opposite: false, equal_spacing: false, second: None, centered: false },
        seeds: vec![InstanceId(2)],
        generated: vec![],
        suppressed: false,
    });
    asm.sync_pattern(MateId(20));
    out.push(("LP1", asm));
    let mut asm = Assembly { instances: vec![instance(parts, 1, "A", PIN, Placement::default(), true), instance(parts, 2, "B", BLOCK, pose(0.0, (215.0, -10.0, 0.0)), false)], ..Default::default() };
    asm.patterns.push(InstancePattern {
        id: MateId(20),
        name: "Circular pattern 1".into(),
        kind: PatternKind::Circular { axis: conn(1, face(parts, PIN, 205.0, 0.0, 15.0), Inference::MidAxis), angle: ExprValue::new("360 deg"), count: 4, opposite: false, equal_spacing: true, centered: false },
        seeds: vec![InstanceId(2)],
        generated: vec![],
        suppressed: false,
    });
    asm.sync_pattern(MateId(20));
    out.push(("CP1", asm));
    out
}

/// What a case gives: per mate (name, ok, message, free values), per
/// instance (name, transform in mm, DOF).
#[derive(Debug)]
struct Outcome {
    mates: Vec<(String, bool, String, [f64; 4])>,
    instances: Vec<(String, [f64; 16], Option<usize>)>,
}

fn outcome(asm: &Assembly, res: &AssemblyResult) -> Outcome {
    let mates = res.mates.iter().map(|m| (asm.mates.iter().find(|x| x.id == m.id).map_or_else(|| format!("{:?}", m.id), |x| x.name.clone()), m.ok, m.message.clone().unwrap_or_default(), m.values)).collect();
    let instances = asm
        .instances
        .iter()
        .zip(&res.nodes)
        .map(|(i, n)| {
            let m = res.solved.placement(i.id).map_or(DMat4::IDENTITY, |p| p.to_dmat4());
            let mut a = m.transpose().to_cols_array();
            for k in [3, 7, 11] {
                a[k] *= 1e3;
            }
            (i.name.clone(), a, n.dof)
        })
        .collect();
    Outcome { mates, instances }
}

/// Live's transforms per case and instance (row-major, translation in mm),
/// from results-*.jsonl; empty until live answers (REQUESTS fs 10-26 c).
const LIVE: &[(&str, &str, [f64; 16])] = &[];

/// Live's mate status per case and mate: (case, mate, ok, tooltip text).
const LIVE_MATES: &[(&str, &str, bool, &str)] = &[];

#[test]
fn assemblies_match_live() {
    let mut parts = parts();
    let mut report = String::new();
    for (name, asm) in cases(&parts) {
        let res = parts.ev.evaluate(&parts.doc, &asm, &SolveOptions::default(), None);
        let got = outcome(&asm, &res);
        eprintln!("{name}: {got:?}");
        for (case, mate, ok, text) in LIVE_MATES.iter().filter(|l| l.0 == name) {
            match got.mates.iter().find(|m| m.0 == *mate) {
                Some((_, o, t, _)) if o == ok && t == text => {}
                ours => report += &format!("{case} {mate}: ours {ours:?}, live ({ok}, {text:?})\n"),
            }
        }
        for (case, inst, t) in LIVE.iter().filter(|l| l.0 == name) {
            let Some((_, ours, _)) = got.instances.iter().find(|i| i.0 == *inst) else {
                report += &format!("{case}: no instance {inst}\n");
                continue;
            };
            if ours.iter().zip(t).any(|(a, b)| (a - b).abs() > 1e-6) {
                report += &format!("{case} {inst}: ours {ours:?}\n      live {t:?}\n");
            }
        }
    }
    assert!(report.is_empty(), "differences from live:\n{report}");
}
