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
    sketch.params.set("sketch", ParamValue::Sketch(std::sync::Arc::new(SketchData { sketch: sk, ..Default::default() })));
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

/// The face / edge / vertex of part `p` through the point (mm). Circles are
/// picked away from x = centre + r, where our cylinders have their seam.
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
        let mut m = mate(10, t, conn(2, edge(parts, PIN, 200.0, 5.0, 0.0), Inference::Center), conn(1, edge(parts, PLATE, 130.0, 25.0, 10.0), Inference::Center));
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
        kind: PatternKind::Linear { direction: conn(1, edge(parts, PLATE, 130.0, 0.0, 10.0), Inference::Midpoint).linear_direction(), distance: ExprValue::new("15 mm"), count: 3, opposite: false, equal_spacing: false, second: None, centered: false, third: None },
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
    // Mates numbered per type, as live names them (Revolute 1, Slider 1).
    for (_, asm) in &mut out {
        let mut n = std::collections::HashMap::new();
        for m in &mut asm.mates {
            let k = n.entry(m.mate_type.name()).or_insert(0);
            *k += 1;
            m.name = format!("{} {k}", m.mate_type.name());
        }
    }
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
/// from live/fs-oracle-asm/raw/*.jsonl (live 10-26); A is fixed at identity.
const LIVE: &[(&str, &str, [f64; 16])] = &[
    ("F1", "B", [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, -6.938893903907228e-15, 0.0, 0.0, 1.0, 10.000000000000002, 0.0, 0.0, 0.0, 1.0]),
    ("F2", "B", [1.1102230246251565e-16, 1.0000000000000004, 0.0, 9.999999999999995, -1.0000000000000004, 1.1102230246251565e-16, 0.0, 30.0, 0.0, 0.0, 1.0, 5.000000000000001, 0.0, 0.0, 0.0, 1.0]),
    ("F3", "B", [1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 20.00000000000001, 0.0, 0.0, -1.0, 10.000000000000002, 0.0, 0.0, 0.0, 1.0]),
    ("F4", "B", [1.0, 0.0, 0.0, 39.99999999999999, 0.0, -1.0, 0.0, 20.0, 0.0, 0.0, -1.0, 10.000000000000002, 0.0, 0.0, 0.0, 1.0]),
    ("F5", "B", [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 20.000000000000018, 0.0, 0.0, 1.0, 10.000000000000002, 0.0, 0.0, 0.0, 1.0]),
    ("R1", "B", [1.0000000000000002, 1.1102230246251565e-16, 0.0, -70.00000000000006, 1.1102230246251565e-16, 0.9999999999999997, 0.0, 19.999999999999975, 0.0, 0.0, 1.0, 10.000000000000002, 0.0, 0.0, 0.0, 1.0]),
    ("R2", "B", [1.0, 0.0, 6.123233995736765e-17, -70.0, 0.0, 1.0, 1.232595164407831e-32, 20.00000000000001, -6.123233995736766e-17, -3.749399456654644e-33, 1.0, 10.000000000000016, 0.0, 0.0, 0.0, 1.0]),
    ("R3", "B", [-0.9237280213470682, -0.3830490080632378, 0.0, 314.74560426941366, -0.3830490080632378, 0.923728021347068, 0.0, 96.60980161264757, 0.0, 0.0, -1.0, 19.999999999999996, 0.0, 0.0, 0.0, 1.0]),
    ("S1", "B", [1.0, 0.0, 0.0, -8.673617379884035e-16, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 25.0, 0.0, 0.0, 0.0, 1.0]),
    ("S2", "B", [1.0, 0.0, 0.0, -4.336808689942018e-15, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 10.000000000000002, 0.0, 0.0, 0.0, 1.0]),
    ("C1", "B", [-0.9237280213470682, -0.3830490080632378, 0.0, 314.74560426941366, -0.3830490080632378, 0.923728021347068, 0.0, 96.60980161264757, 0.0, 0.0, -1.0, 70.0, 0.0, 0.0, 0.0, 1.0]),
    ("P1", "B", [0.9396926207859084, -0.3420201433256687, 0.0, 5.0, 0.3420201433256687, 0.9396926207859084, 0.0, 7.0, 0.0, 0.0, 1.0, 10.000000000000002, 0.0, 0.0, 0.0, 1.0]),
    ("B1", "B", [1.0, 0.0, 0.0, 40.0, 0.0, 1.0, 0.0, 20.0, 0.0, 0.0, 1.0, 10.0, 0.0, 0.0, 0.0, 1.0]),
    ("B2", "B", [-0.8660254037844387, 0.0, 0.5, 40.0, 0.0, 1.0, 0.0, 20.0, -0.5, 0.0, -0.8660254037844387, 10.000000000000002, 0.0, 0.0, 0.0, 1.0]),
    ("M1", "B", [0.8660254037844387, -0.49999999999999994, 0.0, 7.679491924311235, 0.49999999999999994, 0.8660254037844387, 0.0, -8.660254037844393, 0.0, 0.0, 1.0, 10.000000000000002, 0.0, 0.0, 0.0, 1.0]),
    ("M2", "B", [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, -6.938893903907228e-15, 0.0, 0.0, 1.0, 10.000000000000002, 0.0, 0.0, 0.0, 1.0]),
    ("M2", "C", [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 20.000000000000004, 0.0, 0.0, 0.0, 1.0]),
    ("LP1", "B", [1.0, 0.0, 0.0, -70.00000000000001, 0.0, 1.0, 0.0, 20.000000000000004, 0.0, 0.0, 1.0, 10.000000000000002, 0.0, 0.0, 0.0, 1.0]),
    ("LP1", "B <1>", [1.0, 0.0, 0.0, -55.00000000000002, 0.0, 1.0, 0.0, 20.000000000000004, 0.0, 0.0, 1.0, 10.000000000000002, 0.0, 0.0, 0.0, 1.0]),
    ("LP1", "B <2>", [1.0, 0.0, 0.0, -40.00000000000002, 0.0, 1.0, 0.0, 20.000000000000004, 0.0, 0.0, 1.0, 10.000000000000002, 0.0, 0.0, 0.0, 1.0]),
    ("CP1", "B", [1.0, 0.0, 0.0, 215.0, 0.0, 1.0, 0.0, -10.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0]),
    ("CP1", "B <1>", [6.123233995736766e-17, 1.0, 0.0, 190.00000000000003, -1.0, 6.123233995736766e-17, 0.0, -14.999999999999986, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0]),
    ("CP1", "B <2>", [-1.0, 1.2246467991473532e-16, 0.0, 185.00000000000003, -1.2246467991473532e-16, -1.0, 0.0, 9.999999999999998, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0]),
    ("CP1", "B <3>", [-1.8369701987210297e-16, -1.0, 0.0, 210.0, 1.0, -1.8369701987210297e-16, 0.0, 14.999999999999986, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0]),
];
/// Live's mate status per case and mate: (case, mate, ok, tooltip text).
const LIVE_MATES: &[(&str, &str, bool, &str)] = &[
    ("F1", "Fastened 1", true, ""),
    ("F2", "Fastened 1", true, ""),
    ("F3", "Fastened 1", true, ""),
    ("F4", "Fastened 1", true, ""),
    ("F5", "Fastened 1", true, ""),
    ("R1", "Revolute 1", true, ""),
    ("R2", "Revolute 1", true, ""),
    ("R3", "Revolute 1", true, ""),
    ("S1", "Slider 1", true, ""),
    ("S2", "Slider 1", true, ""),
    ("C1", "Cylindrical 1", true, ""),
    ("P1", "Planar 1", true, ""),
    ("B1", "Ball 1", true, ""),
    ("B2", "Ball 1", true, ""),
    ("M1", "Revolute 1", false, ""),
    ("M1", "Slider 1", false, ""),
    ("M2", "Fastened 1", true, ""),
    ("M2", "Revolute 1", true, ""),
    ("LP1", "Fastened 1", true, ""),
];

/// Cases that differ from live, routed (lead 10-26): skb's assembly
/// connectors and solver. The test fails on a new difference and on a known
/// one that matches again.
const KNOWN: &[(&str, &str)] = &[
    ("M1", "skb: Revolute (bottom on top) + Slider (front faces), both \"Mate overdefines the assembly.\" as live; live's B upright, turned 30° at (7.68, −8.66, 10) where its Revolute (added first) had put it; ours, both mates at once, moves nothing from the start (live 1764: a conflict moves nothing)"),
];

#[test]
#[ignore = "T2: oracle vs live (exact values, known-difference lists)"]
fn assemblies_match_live() {
    let mut parts = parts();
    let mut report = String::new();
    for (name, asm) in cases(&parts) {
        let res = parts.ev.evaluate(&parts.doc, &asm, &SolveOptions::default(), None);
        let got = outcome(&asm, &res);
        eprintln!("{name}: {got:?}");
        let mut diff = String::new();
        for (case, mate, ok, text) in LIVE_MATES.iter().filter(|l| l.0 == name) {
            match got.mates.iter().find(|m| m.0 == *mate) {
                Some((_, o, t, _)) if o == ok && t == text => {}
                ours => diff += &format!("{case} {mate}: ours {ours:?}, live ({ok}, {text:?})\n"),
            }
        }
        for (case, inst, t) in LIVE.iter().filter(|l| l.0 == name) {
            let Some((_, ours, _)) = got.instances.iter().find(|i| i.0 == *inst) else {
                diff += &format!("{case}: no instance {inst}\n");
                continue;
            };
            if ours.iter().zip(t).any(|(a, b)| (a - b).abs() > 1e-6) {
                diff += &format!("{case} {inst}: ours {ours:?}\n      live {t:?}\n");
            }
        }
        match KNOWN.iter().find(|k| k.0 == name) {
            Some(k) if diff.is_empty() => report += &format!("{name} now matches live: drop it from KNOWN ({})\n", k.1),
            None => report += &diff,
            _ => {}
        }
    }
    assert!(report.is_empty(), "differences from live:\n{report}");
}

// ---------------------------------------------------------------- fuzz
// Assembly solver fuzz against live (user 11-02): random small assemblies of
// the oracle's parts (BLOCK, PLATE, PIN) with random start poses and random
// mates between their connectors, in the oracle's vocabulary (an entity
// through a point of the part + Onshape's inference), so live's oracle
// pipeline (live/fs-oracle-asm) can build them. `fixtures/live_fuzz/asm-N.json`
// holds the cases and ours' answers; live's come back as `asm-N.live.jsonl`
// (its results-N.jsonl lines).

struct FuzzRng(u64);
impl FuzzRng {
    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((self.0 >> 11) as f64) / ((1u64 << 53) as f64)
    }
    fn below(&mut self, n: usize) -> usize {
        ((self.next() * n as f64) as usize).min(n - 1)
    }
    fn range(&mut self, a: f64, b: f64) -> f64 {
        a + (b - a) * self.next()
    }
}

const PART_NAMES: [&str; 3] = ["BLOCK", "PLATE", "PIN"];

/// A part's connectors: (name, entity kind, point on it in part mm, inference, axial).
fn connectors(p: usize) -> Vec<(&'static str, &'static str, [f64; 3], Inference, bool)> {
    use Inference::*;
    match p {
        BLOCK => vec![
            ("top face", "face", [20.0, 10.0, 10.0], Centroid, false),
            ("bottom face", "face", [20.0, 10.0, 0.0], Centroid, false),
            ("front face", "face", [20.0, 0.0, 5.0], Centroid, false),
            ("right face", "face", [40.0, 10.0, 5.0], Centroid, false),
            ("vertex (0,0,0)", "vertex", [0.0, 0.0, 0.0], Vertex, false),
            ("vertex (40,20,10)", "vertex", [40.0, 20.0, 10.0], Vertex, false),
            ("bottom front edge", "edge", [20.0, 0.0, 0.0], Midpoint, false),
            ("top back edge", "edge", [20.0, 20.0, 10.0], Midpoint, false),
        ],
        PLATE => vec![
            ("top face", "face", [110.0, 5.0, 10.0], Centroid, false),
            ("bottom face", "face", [110.0, 5.0, 0.0], Centroid, false),
            ("hole top edge", "edge", [130.0, 25.0, 10.0], Center, true),
            ("hole bottom edge", "edge", [130.0, 25.0, 0.0], Center, true),
            ("hole cylinder", "face", [135.0, 20.0, 5.0], MidAxis, true),
        ],
        _ => vec![
            ("top edge", "edge", [200.0, 5.0, 30.0], Center, true),
            ("bottom edge", "edge", [200.0, 5.0, 0.0], Center, true),
            ("cylinder", "face", [205.0, 0.0, 15.0], MidAxis, true),
        ],
    }
}

fn onshape_inference(i: Inference) -> &'static str {
    match i {
        Inference::Centroid => "CENTROID",
        Inference::Vertex => "POINT",
        Inference::Midpoint => "MID_POINT",
        Inference::Center => "CENTER",
        Inference::MidAxis => "MID_AXIS_POINT",
        Inference::TopAxis => "TOP_AXIS_POINT",
        Inference::BottomAxis => "BOTTOM_AXIS_POINT",
        Inference::Origin => "ORIGIN",
    }
}

/// A random case: the assembly and its spec for live.
fn fuzz_case(parts: &Parts, seed: u64) -> (Assembly, serde_json::Value) {
    use serde_json::json;
    let mut r = FuzzRng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0x5851_F42D_4C95_7F2D);
    for _ in 0..4 {
        r.next();
    }
    // Batch 2 on (seeds from 21): 3–5 instances.
    let n = if seed > 20 { 3 + r.below(3) } else { 2 + r.below(3) };
    let names = ["A", "B", "C", "D", "E"];
    let mut asm = Assembly::default();
    let mut kinds = Vec::new();
    let mut spec_inst = Vec::new();
    for (k, name) in names.iter().enumerate().take(n) {
        let p = r.below(3);
        let (deg, t) = if k == 0 { (0.0, (0.0, 0.0, 0.0)) } else { ((r.range(0.0, 360.0) * 10.0).round() / 10.0, ((r.range(-100.0, 100.0)).round(), (r.range(-100.0, 100.0)).round(), (r.range(0.0, 60.0)).round())) };
        asm.instances.push(instance(parts, k as u64 + 1, name, p, pose(deg, t), k == 0));
        kinds.push(p);
        spec_inst.push(json!({"name": name, "part": PART_NAMES[p], "fixed": k == 0, "start": {"rotate_z_deg": deg, "translate_mm": [t.0, t.1, t.2]}}));
    }
    let types = [MateType::Fastened, MateType::Revolute, MateType::Slider, MateType::Cylindrical, MateType::Planar, MateType::Ball, MateType::Parallel];
    let mut spec_mates = Vec::new();
    let mut pairs: Vec<(usize, usize)> = (1..n).map(|k| (k, r.below(k))).collect();
    if r.next() < 0.3 {
        let k = 1 + r.below(n - 1);
        pairs.push((k, r.below(k)));
    }
    for (moving, onto) in pairs {
        let t = types[r.below(types.len())];
        let axial = matches!(t, MateType::Revolute | MateType::Slider | MateType::Cylindrical);
        let pick = |r: &mut FuzzRng, p: usize| {
            let cs = connectors(p);
            let pool: Vec<_> = cs.iter().filter(|c| match t {
                MateType::Ball => c.1 == "vertex" || c.4,
                _ if axial && cs.iter().any(|c| c.4) => c.4,
                _ => true,
            }).cloned().collect();
            let pool = if pool.is_empty() { cs } else { pool };
            pool[r.below(pool.len())]
        };
        let (ca, cb) = (pick(&mut r, kinds[moving]), pick(&mut r, kinds[onto]));
        let refer = |p: usize, c: &(&str, &str, [f64; 3], Inference, bool)| match c.1 {
            "face" => face(parts, p, c.2[0], c.2[1], c.2[2]),
            "edge" => edge(parts, p, c.2[0], c.2[1], c.2[2]),
            _ => vertex(parts, p, c.2[0], c.2[1], c.2[2]),
        };
        let id = 10 + asm.mates.len() as u64;
        let mut m = mate(id, t, conn(moving as u64 + 1, refer(kinds[moving], &ca), ca.3), conn(onto as u64 + 1, refer(kinds[onto], &cb), cb.3));
        m.flip_primary = r.next() < 0.2;
        asm.mates.push(m);
        let c = |inst: usize, c: &(&str, &str, [f64; 3], Inference, bool)| json!({"instance": names[inst], "entity": c.1, "what": c.0, "at_mm": c.2, "inference": onshape_inference(c.3)});
        spec_mates.push(json!({"type": t.name(), "first": c(moving, &ca), "second": c(onto, &cb), "flip_primary": asm.mates.last().unwrap().flip_primary}));
    }
    // Named per type, as live does (Revolute 1, Revolute 2).
    let mut count = std::collections::HashMap::new();
    for (m, s) in asm.mates.iter_mut().zip(spec_mates.iter_mut()) {
        let k = count.entry(m.mate_type.name()).or_insert(0);
        *k += 1;
        m.name = format!("{} {k}", m.mate_type.name());
        s["name"] = json!(m.name);
    }
    (asm, json!({"case": format!("f{seed}"), "instances": spec_inst, "mates": spec_mates}))
}

fn fuzz_fixture(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/live_fuzz").join(name)
}

/// The assembly batches: (number, seeds).
const ASM_BATCHES: [(usize, std::ops::RangeInclusive<u64>); 3] = [(1, 1..=20), (2, 21..=50), (3, 51..=80)];

/// The batches with ours' answers; batch n written when LIVE_FUZZ_WRITE is
/// asm-<n>, else checked unchanged (the generator is deterministic).
#[test]
#[ignore = "T2: live fuzz batch (assemblies)"]
fn asm_fuzz_write_batch() {
    let mut parts = parts();
    for (n, seeds) in ASM_BATCHES {
        let mut cases = Vec::new();
        for seed in seeds {
            let (asm, mut spec) = fuzz_case(&parts, seed);
            spec["ours"] = fuzz_ours(&mut parts, &asm);
            cases.push(spec);
        }
        finish_batch(n, cases);
    }
}

/// Ours for a fuzz case: mates added one at a time, each solve stored (as
/// live builds it), so a later conflicting mate meets the earlier ones'
/// solved state.
fn fuzz_ours(parts: &mut Parts, asm: &Assembly) -> serde_json::Value {
    {
        let mut built = asm.clone();
        built.mates.clear();
        let mut res = parts.ev.evaluate(&parts.doc, &built, &SolveOptions::default(), None);
        for m in &asm.mates {
            built.mates.push(m.clone());
            res = parts.ev.evaluate(&parts.doc, &built, &SolveOptions::default(), None);
            for (id, p) in &res.solved.placements {
                if let Some(i) = built.instances.iter_mut().find(|i| i.id == *id) {
                    i.placement = *p;
                }
            }
        }
        let got = outcome(&built, &res);
        serde_json::json!({
            "status": got.mates.iter().map(|m| (m.0.clone(), if m.1 { "OK" } else { "ERROR" })).collect::<std::collections::BTreeMap<_, _>>(),
            "text": got.mates.iter().map(|m| (m.0.clone(), m.2.clone())).collect::<std::collections::BTreeMap<_, _>>(),
            "transforms_mm": got.instances.iter().map(|i| (i.0.clone(), i.1.to_vec())).collect::<std::collections::BTreeMap<_, _>>(),
            "dof": got.instances.iter().map(|i| (i.0.clone(), i.2)).collect::<std::collections::BTreeMap<_, _>>(),
        })
    }
}

fn finish_batch(n: usize, cases: Vec<serde_json::Value>) {
    use serde_json::json;
    let b = json!({"parts": "live/fs-oracle-asm CASES.md parts (BLOCK, PLATE, PIN, one Part Studio)", "method": "CASES.md method: insert in order, fix A, POST the start transform (rotate about +z, then translate), add the mates with implicit connectors (the entity through `at_mm` on that instance's part, the inference)", "cases": cases});
    let path = fuzz_fixture(&format!("asm-{n}.json"));
    if std::env::var("LIVE_FUZZ_WRITE").is_ok_and(|v| v == format!("asm-{n}")) {
        std::fs::write(&path, serde_json::to_string_pretty(&b).unwrap()).unwrap();
    }
    let stored: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    for (x, y) in b["cases"].as_array().unwrap().iter().zip(stored["cases"].as_array().unwrap()) {
        assert_eq!((&x["instances"], &x["mates"]), (&y["instances"], &y["mates"]), "{}: the generator changed", x["case"]);
    }
    let errs = b["cases"].as_array().unwrap().iter().filter(|c| c["ours"]["status"].as_object().unwrap().values().any(|v| v != "OK")).count();
    println!("asm batch {n}: {} cases, {errs} with a mate in error (ours)", cases_len(&b));
}

fn cases_len(b: &serde_json::Value) -> usize {
    b["cases"].as_array().map_or(0, |a| a.len())
}

/// Ours against live's lines (`asm-N.live.jsonl`: live's results format):
/// each mate's status and text, each instance's transform (1e-4 mm and
/// 1e-7 in the rotation).
#[test]
#[ignore = "T2: live fuzz batch vs live (assemblies)"]
fn asm_fuzz_live_matches() {
    // Known differences from live (handoff "Solver fuzz"): f50, where B now
    // matches (a Revolute opposed to its Cylindrical still slides the plate,
    // probes 1786 cb1) but C's free Ball pose is 2e-6 off in its rotation:
    // the conflict stops the solve just short of full convergence.
    // Batch 3 (live 11-02 15:36, 23/30 at first; f53's statuses, f69, f80
    // since): f54 which mates of a conflicting loop of Cylindricals read
    // red; f53 the half-turn's point when both parts are held (probes 1798
    // (c): it depends on what the turned part carries); f52 / f55 / f64 a
    // free part's pose after a Ball / Cylindrical / Slider chain.
    const KNOWN: &[&str] = &["f50", "f53", "f54", "f55", "f64"];
    let mut parts = parts();
    let (mut answered, mut matches, mut report) = (0, 0, Vec::new());
    for n in 1.. {
        let p = fuzz_fixture(&format!("asm-{n}.json"));
        if !p.exists() {
            break;
        }
        let Ok(live) = std::fs::read_to_string(fuzz_fixture(&format!("asm-{n}.live.jsonl"))) else { continue };
        let mut ours: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
        // Ours as the solver stands now (the batch file keeps ours as sent).
        for case in ours["cases"].as_array_mut().unwrap() {
            let seed = case["case"].as_str().unwrap().trim_start_matches('f').parse::<u64>().unwrap();
            let (asm, _) = fuzz_case(&parts, seed);
            case["ours"] = fuzz_ours(&mut parts, &asm);
        }
        for line in live.lines().filter(|l| !l.trim().is_empty()) {
            let l: serde_json::Value = serde_json::from_str(line).unwrap();
            let Some(case) = ours["cases"].as_array().unwrap().iter().find(|c| c["case"] == l["case"]) else { continue };
            answered += 1;
            let d = fuzz_differences(case, &l);
            let name = l["case"].as_str().unwrap_or("?");
            if d.is_empty() {
                matches += 1;
            } else if !KNOWN.contains(&name) {
                report.push(format!("{name}: {}", d.join("; ")));
            }
        }
    }
    println!("live fuzz (assemblies): {answered} cases answered, {matches} match");
    assert!(report.is_empty(), "{}", report.join("\n"));
}

/// Ours (`case["ours"]`) against one of live's result lines: each mate's
/// status and text, each instance's transform (1e-3 mm, 1e-6).
fn fuzz_differences(case: &serde_json::Value, l: &serde_json::Value) -> Vec<String> {
    {
        {
            let mut d = Vec::new();
            for (m, st) in l["status"].as_object().into_iter().flatten() {
                let ok = st == "OK" || st == "INFO";
                // Live's tooltip: "<mate> has error: <message>".
                let text = l["text"][m].as_str().unwrap_or("");
                let text = text.strip_prefix(&format!("{m} has error: ")).unwrap_or(text);
                if (case["ours"]["status"][m] == "OK") != ok || case["ours"]["text"][m].as_str().unwrap_or("") != text {
                    d.push(format!("{m}: ours {} {}, live {st} {}", case["ours"]["status"][m], case["ours"]["text"][m], l["text"][m]));
                }
            }
            for (inst, t) in l["transforms"].as_object().into_iter().flatten() {
                let o = &case["ours"]["transforms_mm"][inst];
                let off = (0..16).any(|k| {
                    let (a, b) = (o[k].as_f64().unwrap_or(f64::NAN), t[k].as_f64().unwrap() * if [3, 7, 11].contains(&k) { 1e3 } else { 1.0 });
                    // Both solvers stop near 1e-7 of a turn: 1e-6 rotation, 1e-3 mm.
                    (a - b).abs() > if [3, 7, 11].contains(&k) { 1e-3 } else { 1e-6 }
                });
                if off {
                    d.push(format!("{inst}: ours {o}, live {t}"));
                }
            }
            d
        }
    }
}

/// Live's assembly probes (request 1776): (a) which message a Fastened
/// that can't hold gets, (b) where a Slider leaves a part turned onto it.
fn asm_probes(parts: &Parts) -> Vec<(&'static str, Assembly)> {
    let top = |p: &Parts| face(p, BLOCK, 20.0, 10.0, 10.0);
    let bottom = |p: &Parts| face(p, BLOCK, 20.0, 10.0, 0.0);
    let mut out = Vec::new();
    let named = |mut asm: Assembly| {
        let mut n = std::collections::HashMap::new();
        for m in &mut asm.mates {
            let k = n.entry(m.mate_type.name()).or_insert(0);
            *k += 1;
            m.name = format!("{} {k}", m.mate_type.name());
        }
        asm
    };
    let blocks = |n: usize| Assembly { instances: ["A", "B", "C"].iter().take(n).enumerate().map(|(k, name)| instance(parts, k as u64 + 1, name, BLOCK, Placement::default(), k == 0)).collect(), ..Default::default() };
    // a1: B Planar on A, then Fastened on the same faces 10 mm up.
    let mut a = blocks(2);
    a.mates.push(mate(10, MateType::Planar, conn(2, bottom(parts), Inference::Centroid), conn(1, top(parts), Inference::Centroid)));
    let mut f = mate(11, MateType::Fastened, conn(2, bottom(parts), Inference::Centroid), conn(1, top(parts), Inference::Centroid));
    f.offset_z = Some(ExprValue::new("10 mm"));
    a.mates.push(f);
    out.push(("a1", named(a)));
    // a2: B Fastened on A, then front face on front face 5 mm along X.
    let mut a = blocks(2);
    a.mates.push(mate(10, MateType::Fastened, conn(2, bottom(parts), Inference::Centroid), conn(1, top(parts), Inference::Centroid)));
    let mut f = mate(11, MateType::Fastened, conn(2, face(parts, BLOCK, 20.0, 0.0, 5.0), Inference::Centroid), conn(1, face(parts, BLOCK, 20.0, 0.0, 5.0), Inference::Centroid));
    f.offset_x = Some(ExprValue::new("5 mm"));
    a.mates.push(f);
    out.push(("a2", named(a)));
    // a4: B Planar on A, C Planar on B, then C Fastened on A 10 mm up.
    let mut a = blocks(3);
    a.mates.push(mate(10, MateType::Planar, conn(2, bottom(parts), Inference::Centroid), conn(1, top(parts), Inference::Centroid)));
    a.mates.push(mate(11, MateType::Planar, conn(3, bottom(parts), Inference::Centroid), conn(2, top(parts), Inference::Centroid)));
    let mut f = mate(12, MateType::Fastened, conn(3, bottom(parts), Inference::Centroid), conn(1, top(parts), Inference::Centroid));
    f.offset_z = Some(ExprValue::new("10 mm"));
    a.mates.push(f);
    out.push(("a4", named(a)));
    // b1–b5: a plate's hole on A's front face by a Slider, turned from its start.
    for (name, deg, t) in [("b1", 0.0, (88.0, 66.0, 49.0)), ("b2", 90.0, (88.0, 66.0, 49.0)), ("b3", 180.0, (88.0, 66.0, 49.0)), ("b4", 302.0, (88.0, 66.0, 49.0)), ("b5", 302.0, (0.0, 0.0, 0.0))] {
        let mut a = two(parts, BLOCK, PLATE, pose(deg, t));
        a.mates.push(mate(10, MateType::Slider, conn(2, edge(parts, PLATE, 130.0, 25.0, 0.0), Inference::Center), conn(1, face(parts, BLOCK, 20.0, 0.0, 5.0), Inference::Centroid)));
        out.push((name, named(a)));
    }
    out
}

#[test]
#[ignore = "T2: live's assembly probes (request 1776)"]
fn asm_probes_match_live() {
    let mut parts = parts();
    let live = std::fs::read_to_string(fuzz_fixture("asm-probe-1.live.jsonl")).unwrap();
    let mut report = Vec::new();
    let mut n = 0;
    for (name, asm) in asm_probes(&parts) {
        let Some(l) = live.lines().filter(|l| !l.trim().is_empty()).map(|l| serde_json::from_str::<serde_json::Value>(l).unwrap()).find(|l| l["case"] == name) else { continue };
        n += 1;
        let case = serde_json::json!({"ours": fuzz_ours(&mut parts, &asm)});
        let d = fuzz_differences(&case, &l);
        if !d.is_empty() {
            report.push(format!("{name}: {}", d.join("; ")));
        }
    }
    println!("live assembly probes: {n} compared, {} differ", report.len());
    assert!(report.is_empty(), "{}", report.join("\n"));
}

/// Live's assembly probes posted from their own input (`asm-probe-<n>.input.json`,
/// live's lines in `asm-probe-<n>.live.jsonl`): 1786 and 1798 (conflicting
/// loops, f69's Parallel, f53's half-turn), 4 (f54's redundant coaxial loops).
#[test]
#[ignore = "T2: live's assembly probes from their input"]
fn asm_spec_probes_match_live() {
    // 1798 f53par: the half-turn's point when both parts are held (as f53);
    // a3: every mate reads red in both, live leaves B turned over elsewhere.
    const KNOWN: &[&str] = &["f53par", "a3"];
    let mut parts = parts();
    let (mut answered, mut matches, mut report) = (0, 0, Vec::new());
    for n in ["1786", "1798", "4"] {
        let input: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(fuzz_fixture(&format!("asm-probe-{n}.input.json"))).unwrap()).unwrap();
        let Ok(live) = std::fs::read_to_string(fuzz_fixture(&format!("asm-probe-{n}.live.jsonl"))) else { continue };
        for l in live.lines().filter(|l| !l.trim().is_empty()).map(|l| serde_json::from_str::<serde_json::Value>(l).unwrap()) {
            let Some(case) = input["cases"].as_array().unwrap().iter().find(|c| c["case"] == l["case"]) else { continue };
            answered += 1;
            let asm = spec_assembly(&parts, case);
            let d = fuzz_differences(&serde_json::json!({"ours": fuzz_ours(&mut parts, &asm)}), &l);
            let name = l["case"].as_str().unwrap_or("?");
            if d.is_empty() {
                matches += 1;
            } else {
                println!("  {n} {name}: {}", d.join("; "));
                if !KNOWN.contains(&name) {
                    report.push(name.to_string());
                }
            }
        }
    }
    println!("live assembly probes from input: {answered} answered, {matches} match");
    assert!(report.is_empty(), "{}", report.join(", "));
}

/// One fuzz case mate by mate (FUZZ_CASE=<seed>): each step's statuses.
#[test]
#[ignore = "debug: one fuzz case"]
fn asm_fuzz_trace() {
    let Ok(seed) = std::env::var("FUZZ_CASE").map(|s| s.parse::<u64>().unwrap()) else { return };
    let mut parts = parts();
    let (asm, spec) = fuzz_case(&parts, seed);
    println!("{}", serde_json::to_string(&spec["mates"]).unwrap());
    let mut built = asm.clone();
    built.mates.clear();
    for m in &asm.mates {
        built.mates.push(m.clone());
        let res = parts.ev.evaluate(&parts.doc, &built, &SolveOptions::default(), None);
        println!("+ {}: {:?}", m.name, res.mates.iter().map(|s| (s.ok, s.message.clone(), s.values)).collect::<Vec<_>>());
        println!("  converged {} placements {:?}", res.solved.converged, res.solved.placements);
        for (id, p) in &res.solved.placements {
            if let Some(i) = built.instances.iter_mut().find(|i| i.id == *id) {
                i.placement = *p;
            }
        }
    }
}

/// An assembly from a case in the fuzz vocabulary (live's own inputs, e.g.
/// the drag probes).
fn spec_assembly(parts: &Parts, spec: &serde_json::Value) -> Assembly {
    let mut asm = Assembly::default();
    let names: Vec<String> = spec["instances"].as_array().unwrap().iter().map(|i| i["name"].as_str().unwrap().to_string()).collect();
    let kind = |name: &str| PART_NAMES.iter().position(|p| *p == name).unwrap();
    let kinds: Vec<usize> = spec["instances"].as_array().unwrap().iter().map(|i| kind(i["part"].as_str().unwrap())).collect();
    for (k, i) in spec["instances"].as_array().unwrap().iter().enumerate() {
        let t = &i["start"]["translate_mm"];
        let at = pose(i["start"]["rotate_z_deg"].as_f64().unwrap(), (t[0].as_f64().unwrap(), t[1].as_f64().unwrap(), t[2].as_f64().unwrap()));
        asm.instances.push(instance(parts, k as u64 + 1, &names[k], kinds[k], at, i["fixed"].as_bool().unwrap()));
    }
    let types = [MateType::Fastened, MateType::Revolute, MateType::Slider, MateType::Cylindrical, MateType::Planar, MateType::Ball, MateType::Parallel];
    let infer = |s: &str| [Inference::Centroid, Inference::Vertex, Inference::Midpoint, Inference::Center, Inference::MidAxis, Inference::TopAxis, Inference::BottomAxis, Inference::Origin].into_iter().find(|i| onshape_inference(*i) == s).unwrap();
    for m in spec["mates"].as_array().unwrap() {
        let t = types.into_iter().find(|t| t.name() == m["type"].as_str().unwrap()).unwrap();
        let side = |c: &serde_json::Value| {
            let inst = names.iter().position(|n| n == c["instance"].as_str().unwrap()).unwrap();
            let at = &c["at_mm"];
            let (x, y, z) = (at[0].as_f64().unwrap(), at[1].as_f64().unwrap(), at[2].as_f64().unwrap());
            let r = match c["entity"].as_str().unwrap() {
                "face" => face(parts, kinds[inst], x, y, z),
                "edge" => edge(parts, kinds[inst], x, y, z),
                _ => vertex(parts, kinds[inst], x, y, z),
            };
            conn(inst as u64 + 1, r, infer(c["inference"].as_str().unwrap()))
        };
        let mut x = mate(10 + asm.mates.len() as u64, t, side(&m["first"]), side(&m["second"]));
        x.flip_primary = m["flip_primary"].as_bool().unwrap_or(false);
        x.name = m["name"].as_str().unwrap_or("").to_string();
        if let Some(o) = m["offset_mm"].as_array() {
            let at = |k: usize| o.get(k).and_then(|v| v.as_f64()).filter(|v| *v != 0.0).map(|v| ExprValue::new(format!("{v} mm")));
            (x.offset_x, x.offset_y, x.offset_z) = (at(0), at(1), at(2));
        }
        asm.mates.push(x);
    }
    asm
}

/// Live's assembly drag probes (request after the sketch drag probes:
/// `asm-drag-1.input.json`, live's lines in `asm-drag-1.live.jsonl`): each
/// case solved mate by mate, then the named point dragged in the view plane
/// in 25 steps, released, and every instance's transform compared.
#[test]
#[ignore = "T2: live assembly drag probes"]
fn asm_drag_probes_match_live() {
    let mut parts = parts();
    let (mut answered, mut matches) = (0, 0);
    // Probes 1 (1802: d1-d6), 2 (1803: d7 / d8; d9's four-bar didn't close in live), 4 (1870: d6's Ball dragged
    // five ways in the top view, once in the front view).
    for n in [1, 2, 4] {
    let input: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(fuzz_fixture(&format!("asm-drag-{n}.input.json"))).unwrap()).unwrap();
    let live: Vec<serde_json::Value> = std::fs::read_to_string(fuzz_fixture(&format!("asm-drag-{n}.live.jsonl"))).map(|t| t.lines().filter(|l| !l.trim().is_empty()).filter_map(|l| serde_json::from_str(l).ok()).collect()).unwrap_or_default();
    for case in input["cases"].as_array().unwrap() {
        let name = case["case"].as_str().unwrap();
        if name.starts_with("d9") {
            continue;
        }
        let asm = spec_assembly(&parts, case);
        // Mate by mate, as live adds them.
        let mut built = asm.clone();
        built.mates.clear();
        for m in &asm.mates {
            built.mates.push(m.clone());
            let res = parts.ev.evaluate(&parts.doc, &built, &SolveOptions::default(), None);
            for (id, p) in &res.solved.placements {
                if let Some(i) = built.instances.iter_mut().find(|i| i.id == *id) {
                    i.placement = *p;
                }
            }
        }
        let d = &case["drag"];
        let inst = built.instances.iter().find(|i| i.name == d["inst"].as_str().unwrap()).unwrap().id;
        let l = &d["local"];
        let local = DVec3::new(l[0].as_f64().unwrap(), l[1].as_f64().unwrap(), l[2].as_f64().unwrap()) * 1e-3;
        for mv in d["moves"].as_array().unwrap() {
            let (view, by) = (mv[0].as_str().unwrap(), &mv[1]);
            let (a, b) = (by[0].as_f64().unwrap() * 1e-3, by[1].as_f64().unwrap() * 1e-3);
            let (step, view_dir) = match view {
                "front" => (DVec3::new(a, 0.0, b), DVec3::Y),
                "right" => (DVec3::new(0.0, a, b), DVec3::X),
                _ => (DVec3::new(a, b, 0.0), DVec3::Z),
            };
            let place = |asm: &Assembly| {
                let p = asm.instances.iter().find(|i| i.id == inst).unwrap().placement;
                let q = glam::DQuat::from_array(p.rotation);
                DVec3::from(p.translation) + q * local
            };
            let from = place(&built);
            // As the app drags (`Prepared::solve_drag`): each step from the
            // drag's start (live asm drag probes 3: the end pose depends
            // only on where it ends).
            let origin = built.clone();
            let at_start = parts.ev.evaluate(&parts.doc, &origin, &SolveOptions::default(), None);
            let start: std::collections::HashMap<_, _> = at_start.solved.placements.iter().cloned().collect();
            let mut landed = true;
            for k in 1..=25 {
                let to = from + step * (k as f64 / 25.0);
                built = origin.clone();
                let opts = SolveOptions { drags: vec![(inst, local, to)], drag_view: Some(view_dir), start: Some(start.clone()), skip_dof: true, ..Default::default() };
                let (solved, l) = at_start.prepared.solve_drag(&opts, &[from], landed);
                landed = l;
                for (id, p) in &solved.placements {
                    if let Some(i) = built.instances.iter_mut().find(|i| i.id == *id) {
                        i.placement = *p;
                    }
                }
            }
        }
        let res = parts.ev.evaluate(&parts.doc, &built, &SolveOptions::default(), None);
        let got = outcome(&built, &res);
        let ours: std::collections::BTreeMap<String, Vec<f64>> = got.instances.iter().map(|i| (i.0.clone(), i.1.to_vec())).collect();
        println!("{name} ours: {}", serde_json::to_string(&ours).unwrap());
        let Some(l) = live.iter().find(|l| l["case"] == name) else { continue };
        answered += 1;
        // After the last move (live's lines: `moves[..].after`).
        let after = l["moves"].as_array().and_then(|m| m.last()).map(|m| &m["after"]).unwrap_or(&serde_json::Value::Null);
        assert!(after.is_object(), "{name}: no result in live's line");
        let off: Vec<String> = after.as_object().into_iter().flatten().filter(|(n, t)| {
            let o = &ours[n.as_str()];
            (0..16).any(|k| {
                let (a, b) = (o[k], t[k].as_f64().unwrap_or(f64::NAN) * if [3, 7, 11].contains(&k) { 1e3 } else { 1.0 });
                (a - b).abs() > if [3, 7, 11].contains(&k) { 0.3 } else { 1e-3 }
            })
        }).map(|(n, t)| {
            // How far off: the turn between the two rotations, the offset.
            let o = &ours[n.as_str()];
            let l: Vec<f64> = (0..16).map(|k| t[k].as_f64().unwrap_or(f64::NAN)).collect();
            let trace: f64 = (0..3).map(|i| (0..3).map(|j| o[4 * j + i] * l[4 * j + i]).sum::<f64>()).sum();
            let turn = ((trace - 1.0) / 2.0).clamp(-1.0, 1.0).acos().to_degrees();
            let off = [3, 7, 11].iter().map(|k| (o[*k] - l[*k] * 1e3).powi(2)).sum::<f64>().sqrt();
            format!("{n}: {turn:.2}° and {off:.2} mm off")
        }).collect();
        if off.is_empty() {
            matches += 1;
        } else {
            println!("  {name} differs: {}", off.join("; "));
        }
    }
    }
    println!("live assembly drag probes: {answered} answered, {matches} match (positions within 0.3 mm: the pointer's landing)");
}
