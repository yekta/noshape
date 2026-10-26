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
    ("F2", "skb: Fastened offset z 5 and rotation 90°: live puts B at (10, 30, 5) turned −90° about z, ours at (30, −10, 15) turned +90° (offset and rotation sense)"),
    ("F5", "skb: edge MID_POINT connectors (B's bottom front edge on A's top back edge): live B unturned at (0, 20, 10), ours turned 180° about y at (40, 20, 10); LP1's pattern direction on an edge midpoint runs the other way too"),
    ("R1", "skb: circular edge CENTER connector: live keeps the pin upright on the hole (identity at (−70, 20, 10), rz 0), ours turns it upside down and 6° about z: live's Z points out of the part (the pin's bottom edge −z, the hole's top edge +z)"),
    ("R2", "skb: Revolute limits 0..45° from a start at 90°: live returns to rz 0 (pin unturned), ours stops at 45°; also R1's connector flip"),
    ("R3", "skb: Revolute on cylinder MID_AXIS_POINTs: both flip the pin, live with rz −157.5° (B at (314.75, 96.61, 20)), ours rz +6°"),
    ("S2", "skb: Slider limits 0..10 mm: live leaves B on A (z 10, translationZ 0), ours lifts it to z 20; S1's value is −15 mm in live, +15 in ours (the slider's sense)"),
    ("C1", "skb: Cylindrical on cylinder axes: as R3 (live rz −157.5°, translationZ +50 mm; ours rz +6°, −50 mm)"),
    ("B2", "skb: Ball with a 30° cone limit: live turns B 150° about y at (40, 20, 10), ours elsewhere"),
    ("M1", "skb: Revolute (bottom on top) + Slider (front faces): live keeps B upright, turned 30° at (7.68, −8.66, 10); ours turns it upside down"),
    ("M2", "skb: Revolute C on fastened B: live C unturned at (0, 0, 20), ours turned 1.3e-4 rad (the solver stops short)"),
    ("LP1", "skb: R1's connector flip and F5's edge direction: live's copies at x −55 / −40 upright, ours at −85 / −100 upside down"),
];

#[test]
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
