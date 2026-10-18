//! Assembly evaluation tests with programmatic Part Studios.

use super::analysis::{auto_exploded_view, bom, interference};
use super::eval::{AssemblyEvaluator, SolveOptions};
use super::*;
use crate::configuration::ConfigValues;
use crate::document::{Document, DocumentSettings, TabContent, TabId};
use crate::params::ParamValue;
use crate::partstudio::PartStudio;
use crate::reference::{BodyId, DatumPlane, Inference, Reference};
use crate::regen::{RegenOptions, Regenerator};
use crate::sketch_feature::SketchData;
use glam::{DQuat, DVec2, DVec3};
use noshape_units::{ExprValue, UnitPrefs};

fn rect(sk: &mut noshape_sketch::Sketch, x0: f64, y0: f64, w: f64, h: f64) {
    let p = [DVec2::new(x0, y0), DVec2::new(x0 + w, y0), DVec2::new(x0 + w, y0 + h), DVec2::new(x0, y0 + h)];
    let pts: Vec<_> = p.iter().map(|q| sk.add_point(*q)).collect();
    for i in 0..4 {
        sk.add_line_between(pts[i], pts[(i + 1) % 4]).unwrap();
    }
}

/// Studio with one extruded sketch; `region` picks the extruded region
/// (largest when true, smallest otherwise).
fn extruded_studio(sk: noshape_sketch::Sketch, depth: &str, largest: bool) -> PartStudio {
    let units = UnitPrefs::default();
    let mut ps = PartStudio::new();
    let mut sketch = ps.new_feature("sketch", &units).unwrap();
    sketch.params.set("sketchPlane", ParamValue::Query(vec![Reference::Plane(DatumPlane::Top)]));
    sketch.params.set("sketch", ParamValue::Sketch(Box::new(SketchData { sketch: sk, ..Default::default() })));
    let sid = sketch.id;
    ps.features.push(sketch);
    let mut reg = Regenerator::new();
    let r = reg.regenerate(&ps, &ConfigValues::default(), &units, &RegenOptions::default());
    let regions = &r.display.sketches[0].regions;
    let pick = if largest {
        regions.iter().max_by(|a, b| a.area.partial_cmp(&b.area).unwrap())
    } else {
        regions.iter().min_by(|a, b| a.area.partial_cmp(&b.area).unwrap())
    }
    .unwrap();
    let mut ex = ps.new_feature("extrude", &units).unwrap();
    ex.params.set("entities", ParamValue::Query(vec![Reference::SketchRegion { sketch: sid, key: pick.key.clone() }]));
    ex.params.set("depth", ParamValue::Expr(ExprValue::new(depth)));
    ps.features.push(ex);
    ps
}

/// 40 x 20 x 10 mm block with its corner at the origin.
fn block_studio() -> PartStudio {
    let mut sk = noshape_sketch::Sketch::new();
    rect(&mut sk, 0.0, 0.0, 0.04, 0.02);
    extruded_studio(sk, "10 mm", true)
}

/// 60 x 40 x 10 mm plate centered on the origin with a 5 mm radius hole at the center.
fn plate_with_hole() -> PartStudio {
    let mut sk = noshape_sketch::Sketch::new();
    rect(&mut sk, -0.03, -0.02, 0.06, 0.04);
    sk.add_circle(DVec2::ZERO, 0.005);
    extruded_studio(sk, "10 mm", true)
}

/// Pin: radius 5 mm, 30 mm long, centered on the origin.
fn pin_studio() -> PartStudio {
    let mut sk = noshape_sketch::Sketch::new();
    sk.add_circle(DVec2::ZERO, 0.005);
    extruded_studio(sk, "30 mm", true)
}

struct Fixture {
    doc: Document,
    studios: Vec<TabId>,
}

fn doc_with(studios: Vec<PartStudio>) -> Fixture {
    let mut doc = Document { settings: DocumentSettings::default(), tabs: Vec::new() };
    let ids = studios.into_iter().enumerate().map(|(i, s)| doc.add_tab(&format!("Part Studio {}", i + 1), TabContent::PartStudio(s))).collect();
    Fixture { doc, studios: ids }
}

fn body_of(ev: &mut AssemblyEvaluator, doc: &Document, studio: TabId) -> (BodyId, std::sync::Arc<crate::regen::RegenResult>) {
    let r = ev.studio(doc, studio, &ConfigValues::default()).unwrap();
    assert!(!r.context.bodies.is_empty(), "studio produced no bodies: {:?}", r.display.features);
    (r.context.bodies[0].id.clone(), r)
}

fn face_with(r: &crate::regen::RegenResult, needle: &str) -> Reference {
    let b = &r.context.bodies[0];
    let n = b.faces.iter().find(|f| format!("{f:?}").contains(needle)).unwrap_or_else(|| panic!("no face containing {needle}: {:#?}", b.faces));
    Reference::Face((**n).clone())
}

/// Cylindrical face: side face of the circle (entity id of the circle is the last entity).
fn cylinder_face(r: &crate::regen::RegenResult) -> Reference {
    let b = &r.context.bodies[0];
    for (i, f) in b.faces.iter().enumerate() {
        let face = b.shape.sub_shape(noshape_kernel::ShapeType::Face, i).unwrap();
        if matches!(face.face_surface(), Ok(noshape_kernel::Surface::Cylinder { .. })) {
            return Reference::Face((**f).clone());
        }
    }
    panic!("no cylindrical face");
}

fn part_instance(id: u64, name: &str, studio: TabId, body: BodyId, at: DVec3, fixed: bool) -> Instance {
    Instance {
        id: InstanceId(id),
        name: name.into(),
        source: InstanceSource::Part { studio, body, config: ConfigValues::default() },
        placement: Placement { translation: at.into(), rotation: [0.0, 0.0, 0.0, 1.0] },
        fixed,
        hidden: false,
        suppressed: false,
        lock: SubassemblyLock::Flexible,
        nested: Vec::new(),
        best_tessellation: false,
    }
}

fn conn(instance: u64, on: Reference, inference: Inference) -> MateConnectorRef {
    MateConnectorRef::on(InstanceId(instance), vec![], Reference::Implicit { on: Box::new(on), inference })
}

fn mate(id: u64, t: MateType, a: MateConnectorRef, b: MateConnectorRef) -> Mate {
    Mate::new(MateId(id), format!("{} {id}", t.name()), t, a, b)
}

fn origin_of(r: &eval::AssemblyResult, id: u64) -> DVec3 {
    DVec3::from(r.solved.placement(InstanceId(id)).unwrap().translation)
}

#[test]
fn two_blocks_fastened_face_to_face() {
    let f = doc_with(vec![block_studio()]);
    let mut ev = AssemblyEvaluator::new();
    let (body, r) = body_of(&mut ev, &f.doc, f.studios[0]);
    let top = face_with(&r, "CapEnd");
    let bottom = face_with(&r, "CapStart");
    let asm = Assembly {
        instances: vec![
            part_instance(1, "Block <1>", f.studios[0], body.clone(), DVec3::ZERO, true),
            part_instance(2, "Block <2>", f.studios[0], body.clone(), DVec3::new(0.07, 0.03, -0.02), false),
        ],
        mates: vec![mate(10, MateType::Fastened, conn(1, top, Inference::Centroid), conn(2, bottom, Inference::Centroid))],
        ..Default::default()
    };
    let res = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
    assert!(res.mates[0].ok, "{:?}", res.mates[0].message);
    let o = origin_of(&res, 2);
    assert!((o - DVec3::new(0.0, 0.0, 0.01)).length() < 1e-6, "B origin {o:?}");
    assert_eq!(res.solved.dof, 0);
    assert_eq!(res.nodes[1].dof, Some(0));
    assert_eq!(res.occurrences.len(), 2);
    // Display transform matches the solved placement.
    let t = res.occurrences[1].transform.transform_point3(DVec3::ZERO);
    assert!((t - o).length() < 1e-9);
    // Stacked blocks touch but do not interfere.
    assert!(interference(&res, 1e-12).is_empty());
    // BOM: one row, quantity 2.
    let b = bom(&res);
    assert_eq!(b.len(), 1);
    assert_eq!(b[0].quantity, 2);

    // Offset of 5 mm along Z separates them.
    let mut asm2 = asm.clone();
    asm2.mates[0].offset_z = Some(ExprValue::new("5 mm"));
    let res = ev.evaluate(&f.doc, &asm2, &SolveOptions::default(), None);
    let o = origin_of(&res, 2);
    assert!((o.z - 0.015).abs() < 1e-6, "offset: {o:?}");

    // Flip primary: Z axes agree, so B is turned upside down and sits below A's top face.
    let mut asm3 = asm.clone();
    asm3.mates[0].flip_primary = true;
    let res = ev.evaluate(&f.doc, &asm3, &SolveOptions::default(), None);
    assert!(res.mates[0].ok);
    let p = res.solved.placement(InstanceId(2)).unwrap();
    let q = DQuat::from_xyzw(p.rotation[0], p.rotation[1], p.rotation[2], p.rotation[3]);
    assert!((q * DVec3::Z).z < -0.999, "flipped orientation {:?}", q * DVec3::Z);
}

#[test]
fn revolute_pin_in_plate_hole() {
    let f = doc_with(vec![plate_with_hole(), pin_studio()]);
    let mut ev = AssemblyEvaluator::new();
    let (plate, rp) = body_of(&mut ev, &f.doc, f.studios[0]);
    let (pin, rq) = body_of(&mut ev, &f.doc, f.studios[1]);
    let hole = cylinder_face(&rp);
    let pin_face = cylinder_face(&rq);
    let asm = Assembly {
        instances: vec![
            part_instance(1, "Plate <1>", f.studios[0], plate, DVec3::ZERO, true),
            part_instance(2, "Pin <1>", f.studios[1], pin, DVec3::new(0.05, 0.04, 0.02), false),
        ],
        mates: vec![mate(10, MateType::Revolute, conn(1, hole, Inference::MidAxis), conn(2, pin_face, Inference::MidAxis))],
        ..Default::default()
    };
    let res = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
    assert!(res.mates[0].ok, "{:?}", res.mates[0].message);
    assert_eq!(res.nodes[1].dof, Some(1), "revolute leaves one DOF");
    // Pin axis passes through the hole axis (x = y = 0), mid-planes coincide (z = 5 mm).
    let pin_mid = res.occurrences[1].transform.transform_point3(DVec3::new(0.0, 0.0, 0.015));
    assert!(pin_mid.truncate().length() < 1e-6 && (pin_mid.z - 0.005).abs() < 1e-6, "pin mid {pin_mid:?}");
    // Pin and hole have equal radii: no interference.
    assert!(interference(&res, 1e-10).is_empty());

    // Drive the revolute to 90 degrees: the pin's local X axis turns about the hole axis.
    let solved = res.prepared.solve(&SolveOptions { drive: Some((MateId(10), std::f64::consts::FRAC_PI_2)), ..Default::default() });
    assert!(solved.converged);
    let v0 = solved.mate_values[&MateId(10)][0];
    assert!((v0 - std::f64::consts::FRAC_PI_2).abs() < 1e-5, "driven angle {v0}");
    let p = solved.placement(InstanceId(2)).unwrap();
    assert!(DVec3::from(p.translation).truncate().length() < 1e-6, "pin stays on axis: {:?}", p.translation);

    // A revolute leaves the pin free to spin: dragging a point on its rim rotates it.
    let s = res.prepared.solve(&SolveOptions { drags: vec![(InstanceId(2), DVec3::new(0.005, 0.0, 0.015), DVec3::new(0.0, 0.01, 0.005))], ..Default::default() });
    assert!(s.converged);
    let p = s.placement(InstanceId(2)).unwrap();
    assert!(DVec3::from(p.translation).truncate().length() < 1e-6, "drag keeps pin on axis: {:?}", p.translation);
}

#[test]
fn slider_respects_limits() {
    let f = doc_with(vec![block_studio()]);
    let mut ev = AssemblyEvaluator::new();
    let (body, r) = body_of(&mut ev, &f.doc, f.studios[0]);
    let top = face_with(&r, "CapEnd");
    let bottom = face_with(&r, "CapStart");
    let mut m = mate(10, MateType::Slider, conn(1, top, Inference::Centroid), conn(2, bottom, Inference::Centroid));
    m.limits = vec![("z".into(), Limits { min: Some(ExprValue::new("0 mm")), max: Some(ExprValue::new("20 mm")) })];
    let asm = Assembly {
        instances: vec![
            part_instance(1, "Block <1>", f.studios[0], body.clone(), DVec3::ZERO, true),
            part_instance(2, "Block <2>", f.studios[0], body, DVec3::new(0.0, 0.0, 0.02), false),
        ],
        mates: vec![m],
        ..Default::default()
    };
    let res = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
    assert!(res.mates[0].ok, "{:?}", res.mates[0].message);
    assert_eq!(res.nodes[1].dof, Some(1));
    // Drag far beyond the max limit: stops at 20 mm above A's top face.
    let s = res.prepared.solve(&SolveOptions { drags: vec![(InstanceId(2), DVec3::new(0.02, 0.01, 0.0), DVec3::new(0.03, 0.0, 0.08))], ..Default::default() });
    let o = DVec3::from(s.placement(InstanceId(2)).unwrap().translation);
    assert!(o.x.abs() < 1e-6 && o.y.abs() < 1e-6, "slides along Z only: {o:?}");
    assert!((o.z - 0.03).abs() < 1e-4, "clamped to max limit (10 mm top + 20 mm): {o:?}");
    // Drag inside the range: follows.
    let s = res.prepared.solve(&SolveOptions { drags: vec![(InstanceId(2), DVec3::new(0.02, 0.01, 0.0), DVec3::new(0.02, 0.01, 0.025))], ..Default::default() });
    let o = DVec3::from(s.placement(InstanceId(2)).unwrap().translation);
    assert!((o.z - 0.025).abs() < 1e-3, "inside range: {o:?}");
}

#[test]
fn interference_bom_and_explode() {
    let f = doc_with(vec![block_studio()]);
    let mut ev = AssemblyEvaluator::new();
    let (body, _) = body_of(&mut ev, &f.doc, f.studios[0]);
    let asm = Assembly {
        instances: vec![
            part_instance(1, "Block <1>", f.studios[0], body.clone(), DVec3::ZERO, true),
            part_instance(2, "Block <2>", f.studios[0], body.clone(), DVec3::new(0.0, 0.0, 0.005), false),
            part_instance(3, "Block <3>", f.studios[0], body, DVec3::new(0.1, 0.0, 0.0), false),
        ],
        ..Default::default()
    };
    let res = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
    let hits = interference(&res, 1e-12);
    assert_eq!(hits.len(), 1, "{hits:?}");
    let expected = 0.04 * 0.02 * 0.005;
    assert!((hits[0].volume - expected).abs() < 1e-10, "{} vs {expected}", hits[0].volume);
    let b = bom(&res);
    assert_eq!(b.len(), 1);
    assert_eq!(b[0].quantity, 3);
    // Unconstrained instances keep 6 DOF; fixed keeps 0.
    assert_eq!(res.nodes[0].dof, Some(0));
    assert_eq!(res.nodes[2].dof, Some(6));

    // Exploded view moves instances apart (display only).
    let view = auto_exploded_view(&res, "Exploded view 1", 1.0);
    let mut asm2 = asm.clone();
    asm2.exploded_views.push(view);
    let ex = ev.evaluate(&f.doc, &asm2, &SolveOptions::default(), Some(0));
    let moved = ex.occurrences.iter().zip(res.occurrences.iter()).filter(|(a, b)| !a.transform.abs_diff_eq(b.transform, 1e-9)).count();
    assert!(moved >= 2, "exploded view should move instances");
    assert_eq!(ex.solved.placements, res.solved.placements, "explode does not change placements");
}

#[test]
fn sub_assembly_and_nested_mate() {
    let mut f = doc_with(vec![block_studio()]);
    let mut ev = AssemblyEvaluator::new();
    let (body, r) = body_of(&mut ev, &f.doc, f.studios[0]);
    let top = face_with(&r, "CapEnd");
    let bottom = face_with(&r, "CapStart");
    // Sub-assembly: two blocks stacked.
    let sub = Assembly {
        instances: vec![
            part_instance(1, "Block <1>", f.studios[0], body.clone(), DVec3::ZERO, true),
            part_instance(2, "Block <2>", f.studios[0], body.clone(), DVec3::new(0.1, 0.0, 0.0), false),
        ],
        mates: vec![mate(10, MateType::Fastened, conn(1, top.clone(), Inference::Centroid), conn(2, bottom.clone(), Inference::Centroid))],
        ..Default::default()
    };
    let sub_tab = f.doc.add_tab("Sub", TabContent::Assembly(sub));
    // Top: a fixed block and the sub-assembly fastened onto it by its upper block's top face.
    let top_asm = Assembly {
        instances: vec![
            part_instance(1, "Base <1>", f.studios[0], body.clone(), DVec3::ZERO, true),
            Instance {
                id: InstanceId(5),
                name: "Sub <1>".into(),
                source: InstanceSource::Assembly { assembly: sub_tab },
                placement: Placement { translation: [0.2, 0.0, 0.0], rotation: [0.0, 0.0, 0.0, 1.0] },
                fixed: false,
                hidden: false,
                suppressed: false,
                lock: SubassemblyLock::Flexible,
                nested: Vec::new(),
                best_tessellation: false,
            },
        ],
        mates: vec![mate(
            20,
            MateType::Fastened,
            conn(1, top, Inference::Centroid),
            MateConnectorRef::on(InstanceId(5), vec![InstanceId(1)], Reference::Implicit { on: Box::new(bottom), inference: Inference::Centroid }),
        )],
        ..Default::default()
    };
    let res = ev.evaluate(&f.doc, &top_asm, &SolveOptions::default(), None);
    assert_eq!(res.occurrences.len(), 3);
    assert!(res.mates[0].ok, "{:?}", res.mates[0].message);
    assert_eq!(res.nodes[1].children.len(), 2);
    assert_eq!(res.nodes[1].children[1].path, vec![InstanceId(5), InstanceId(2)]);
    // Sub's lower block sits on the base (z = 10 mm), its upper block at z = 20 mm.
    let lower = res.occurrences.iter().find(|o| o.path == vec![InstanceId(5), InstanceId(1)]).unwrap();
    let upper = res.occurrences.iter().find(|o| o.path == vec![InstanceId(5), InstanceId(2)]).unwrap();
    assert!((lower.transform.transform_point3(DVec3::ZERO) - DVec3::new(0.0, 0.0, 0.01)).length() < 1e-6);
    assert!((upper.transform.transform_point3(DVec3::ZERO) - DVec3::new(0.0, 0.0, 0.02)).length() < 1e-6);
    assert_eq!(bom(&res)[0].quantity, 3);
}

#[test]
fn group_moves_rigidly_and_gear_relation() {
    let f = doc_with(vec![block_studio(), plate_with_hole(), pin_studio()]);
    let mut ev = AssemblyEvaluator::new();
    let (body, r) = body_of(&mut ev, &f.doc, f.studios[0]);
    let top = face_with(&r, "CapEnd");
    let bottom = face_with(&r, "CapStart");
    // Block 2 and 3 grouped; mating 2 onto 1 carries 3 along.
    let asm = Assembly {
        instances: vec![
            part_instance(1, "Block <1>", f.studios[0], body.clone(), DVec3::ZERO, true),
            part_instance(2, "Block <2>", f.studios[0], body.clone(), DVec3::new(0.1, 0.0, 0.0), false),
            part_instance(3, "Block <3>", f.studios[0], body.clone(), DVec3::new(0.1, 0.05, 0.0), false),
        ],
        mates: vec![mate(10, MateType::Fastened, conn(1, top, Inference::Centroid), conn(2, bottom, Inference::Centroid))],
        groups: vec![Group { id: MateId(30), name: "Group 1".into(), instances: vec![InstanceId(2), InstanceId(3)], suppressed: false }],
        ..Default::default()
    };
    let res = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
    assert!(res.mates[0].ok);
    let o2 = origin_of(&res, 2);
    let o3 = origin_of(&res, 3);
    assert!((o2 - DVec3::new(0.0, 0.0, 0.01)).length() < 1e-6, "{o2:?}");
    assert!((o3 - o2 - DVec3::new(0.0, 0.05, 0.0)).length() < 1e-6, "group offset kept: {o3:?}");

    // Gear: two pins revolute in the fixed plate's hole (same axis), 2:1.
    let (plate, rp) = body_of(&mut ev, &f.doc, f.studios[1]);
    let (pin, rq) = body_of(&mut ev, &f.doc, f.studios[2]);
    let hole = cylinder_face(&rp);
    let pf = cylinder_face(&rq);
    let asm = Assembly {
        instances: vec![
            part_instance(1, "Plate <1>", f.studios[1], plate, DVec3::ZERO, true),
            part_instance(2, "Pin <1>", f.studios[2], pin.clone(), DVec3::ZERO, false),
            part_instance(3, "Pin <2>", f.studios[2], pin, DVec3::ZERO, false),
        ],
        mates: vec![
            mate(10, MateType::Revolute, conn(1, hole.clone(), Inference::MidAxis), conn(2, pf.clone(), Inference::MidAxis)),
            mate(11, MateType::Revolute, conn(1, hole, Inference::MidAxis), conn(3, pf, Inference::MidAxis)),
        ],
        relations: vec![Relation { id: MateId(40), name: "Gear 1".into(), relation_type: RelationType::Gear, mates: [MateId(10), MateId(11)], ratio: ExprValue::new("2"), reverse: false, suppressed: false, base: None }],
        ..Default::default()
    };
    let res = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
    assert!(res.mates.iter().all(|m| m.ok));
    assert!(res.relations[0].ok);
    let a0 = res.solved.mate_values[&MateId(10)][0];
    let b0 = res.solved.mate_values[&MateId(11)][0];
    let start: std::collections::HashMap<_, _> = res.solved.placements.iter().cloned().collect();
    let s = res.prepared.solve(&SolveOptions { drive: Some((MateId(10), a0 + 0.3)), start: Some(start), ..Default::default() });
    let a1 = s.mate_values[&MateId(10)][0];
    let b1 = s.mate_values[&MateId(11)][0];
    assert!((a1 - a0 - 0.3).abs() < 1e-4, "driver moved {a0} -> {a1}");
    assert!(((b1 - b0) + 2.0 * (a1 - a0)).abs() < 1e-3, "gear 2:1 opposite: da={} db={}", a1 - a0, b1 - b0);
}

#[test]
fn demo_assembly_is_mated() {
    let doc = crate::demo::demo_document();
    let asm = doc.tabs.iter().find_map(|t| match &t.content {
        TabContent::Assembly(a) => Some(a.clone()),
        _ => None,
    });
    let asm = asm.expect("demo assembly");
    assert_eq!(asm.instances.len(), 3);
    let mut ev = AssemblyEvaluator::new();
    let res = ev.evaluate(&doc, &asm, &SolveOptions::default(), None);
    assert!(res.mates.iter().all(|m| m.ok), "{:?}", res.mates.iter().map(|m| &m.message).collect::<Vec<_>>());
    assert_eq!(res.nodes[2].dof, Some(1));
    assert_eq!(bom(&res).len(), 2);
    // The pin's Part Studio is configured (Change configuration… is offered):
    // the default is 40 mm long, "Long" 60 mm, and the mate keeps resolving.
    let pin_len = |res: &eval::AssemblyResult| {
        let (lo, hi) = res.occurrences.iter().find(|o| o.instance_name == "Pin <1>").unwrap().world_bounds().unwrap();
        (hi - lo).max_element()
    };
    assert!((pin_len(&res) - 0.04).abs() < 1e-6);
    let pin_studio = asm.instances[2].source.studio().unwrap();
    let TabContent::PartStudio(ps) = &doc.tab(pin_studio).unwrap().content else { panic!() };
    assert_eq!(ps.configuration.inputs.len(), 1);
    let mut asm2 = asm.clone();
    let mut long = ConfigValues::default();
    long.0.insert("Length".into(), "Long".into());
    assert!(asm2.set_instance_config(InstanceId(3), long));
    let res2 = ev.evaluate(&doc, &asm2, &SolveOptions::default(), None);
    assert!(res2.mates.iter().all(|m| m.ok), "{:?}", res2.mates.iter().map(|m| &m.message).collect::<Vec<_>>());
    assert!((pin_len(&res2) - 0.06).abs() < 1e-6, "{}", pin_len(&res2));
    // The second plate sits on top of the first (12 mm plates), turned about Z only.
    let p = res.solved.placement(InstanceId(2)).unwrap();
    assert!((p.translation[2] - 0.012).abs() < 1e-6, "{p:?}");
    let q = DQuat::from_xyzw(p.rotation[0], p.rotation[1], p.rotation[2], p.rotation[3]);
    assert!((q * DVec3::Z).z > 0.999, "plate 2 stays horizontal");
}

// ------------------------------------------------------------ new behaviors

fn quat_of(p: &Placement) -> DQuat {
    DQuat::from_xyzw(p.rotation[0], p.rotation[1], p.rotation[2], p.rotation[3])
}

#[test]
fn origin_mate_and_assembly_connectors() {
    let f = doc_with(vec![block_studio()]);
    let mut ev = AssemblyEvaluator::new();
    let (body, r) = body_of(&mut ev, &f.doc, f.studios[0]);
    let top = face_with(&r, "CapEnd");
    let bottom = face_with(&r, "CapStart");
    // Assembly connector on the origin moved by (10, 0, 5) mm, and one on block 1's top face.
    let mut on_origin = AsmMateConnector::new(MateId(50), "Mate connector 1".into(), MateConnectorRef::origin());
    on_origin.translation = [Some(ExprValue::new("10 mm")), None, Some(ExprValue::new("5 mm"))];
    let on_block = AsmMateConnector::new(MateId(51), "Mate connector 2".into(), conn(1, top, Inference::Centroid));
    let asm = Assembly {
        instances: vec![
            part_instance(1, "Block <1>", f.studios[0], body.clone(), DVec3::new(0.3, 0.2, 0.1), false),
            part_instance(2, "Block <2>", f.studios[0], body.clone(), DVec3::new(-0.2, 0.0, 0.0), false),
        ],
        mates: vec![
            // Block 1's bottom-face centroid fastened face-to-face onto the moved origin connector.
            mate(10, MateType::Fastened, MateConnectorRef::assembly_connector(MateId(50), InstanceId::ORIGIN), conn(1, bottom.clone(), Inference::Centroid)),
            mate(11, MateType::Fastened, MateConnectorRef::assembly_connector(MateId(51), InstanceId(1)), conn(2, bottom, Inference::Centroid)),
        ],
        connectors: vec![on_origin, on_block],
        ..Default::default()
    };
    let res = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
    assert!(res.mates.iter().all(|m| m.ok), "{:?}", res.mates.iter().map(|m| &m.message).collect::<Vec<_>>());
    // Bottom centroid (20, 10, 0) mm lands on (10, 0, 5) mm.
    let o1 = origin_of(&res, 1);
    assert!((o1 - DVec3::new(-0.01, -0.01, 0.005)).length() < 1e-6, "block 1 {o1:?}");
    assert!((quat_of(&res.solved.placement(InstanceId(1)).unwrap()) * DVec3::Z).z > 0.999999, "stands upright");
    // Block 2 stacks on block 1 through the connector on block 1's top face.
    let o2 = origin_of(&res, 2);
    assert!((o2 - (o1 + DVec3::new(0.0, 0.0, 0.01))).length() < 1e-6, "block 2 {o2:?}");
    assert_eq!(res.solved.dof, 0);
    assert_eq!(res.nodes[0].dof, Some(0), "mated to the origin: fully constrained");
    // Connector world frames are reported.
    let f50 = res.connector_frame(MateId(50)).unwrap();
    assert!((f50.origin - DVec3::new(0.01, 0.0, 0.005)).length() < 1e-9);
    let f51 = res.connector_frame(MateId(51)).unwrap();
    assert!((f51.origin - (o1 + DVec3::new(0.02, 0.01, 0.01))).length() < 1e-6, "{:?}", f51.origin);
    // Deleting the connector deletes the mate that uses it.
    let mut a2 = asm.clone();
    a2.remove_mate(MateId(51));
    assert_eq!(a2.mates.len(), 1);
    // Deleting block 1 removes its connector and both mates.
    let mut a3 = asm.clone();
    a3.remove_instance(InstanceId(1));
    assert!(a3.connectors.iter().all(|c| c.id != MateId(51)));
    assert!(a3.mates.is_empty());
}

#[test]
fn tangent_mates_keep_contact() {
    let f = doc_with(vec![block_studio(), pin_studio()]);
    let mut ev = AssemblyEvaluator::new();
    let (block, rb) = body_of(&mut ev, &f.doc, f.studios[0]);
    let (pin, rp) = body_of(&mut ev, &f.doc, f.studios[1]);
    let top = face_with(&rb, "CapEnd");
    let bottom = face_with(&rb, "CapStart");
    let cyl = cylinder_face(&rp);
    let ent = |i: u64, r: Reference| MateConnectorRef::on(InstanceId(i), vec![], r);
    // Pin lying roughly on its side above the block.
    let lying = Placement { translation: [0.02, 0.0, 0.03], rotation: { let q = DQuat::from_rotation_x(1.3); [q.x, q.y, q.z, q.w] } };
    let mut pin_inst = part_instance(2, "Pin <1>", f.studios[1], pin.clone(), DVec3::ZERO, false);
    pin_inst.placement = lying;
    let asm = Assembly {
        instances: vec![part_instance(1, "Block <1>", f.studios[0], block.clone(), DVec3::ZERO, true), pin_inst],
        mates: vec![mate(10, MateType::Tangent, ent(1, top.clone()), ent(2, cyl.clone()))],
        ..Default::default()
    };
    let res = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
    assert!(res.mates[0].ok, "{:?}", res.mates[0].message);
    let p = res.solved.placement(InstanceId(2)).unwrap();
    let axis = quat_of(&p) * DVec3::Z;
    assert!(axis.z.abs() < 1e-6, "cylinder axis parallel to the plane: {axis:?}");
    // Axis distance to the top face (z = 10 mm) equals the 5 mm radius.
    let axis_pt = DVec3::from(p.translation);
    assert!((axis_pt.z - 0.015).abs() < 1e-6, "rests on the face: {axis_pt:?}");
    assert_eq!(res.nodes[1].dof, Some(4), "plane-cylinder tangency removes 2 DOF");

    // Plane-plane: coplanar, opposed normals (3 DOF removed).
    let asm = Assembly {
        instances: vec![part_instance(1, "Block <1>", f.studios[0], block.clone(), DVec3::ZERO, true), part_instance(2, "Block <2>", f.studios[0], block.clone(), DVec3::new(0.01, 0.02, 0.03), false)],
        mates: vec![mate(10, MateType::Tangent, ent(1, top.clone()), ent(2, bottom.clone()))],
        ..Default::default()
    };
    let res = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
    assert!(res.mates[0].ok, "{:?}", res.mates[0].message);
    let o = origin_of(&res, 2);
    assert!((o.z - 0.01).abs() < 1e-6, "{o:?}");
    assert_eq!(res.nodes[1].dof, Some(3));

    // Cylinder-cylinder: two parallel pins touching externally (axis distance 10 mm).
    let asm = Assembly {
        instances: vec![part_instance(1, "Pin <1>", f.studios[1], pin.clone(), DVec3::ZERO, true), part_instance(2, "Pin <2>", f.studios[1], pin.clone(), DVec3::new(0.03, 0.01, 0.0), false)],
        mates: vec![mate(10, MateType::Tangent, ent(1, cyl.clone()), ent(2, cyl.clone()))],
        ..Default::default()
    };
    let res = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
    assert!(res.mates[0].ok, "{:?}", res.mates[0].message);
    let o = origin_of(&res, 2);
    assert!((o.truncate().length() - 0.01).abs() < 1e-6, "axis distance r1 + r2: {o:?}");
    // Mate connectors are rejected for tangent mates.
    let asm2 = Assembly { mates: vec![mate(10, MateType::Tangent, conn(1, cyl.clone(), Inference::MidAxis), MateConnectorRef::origin())], ..asm.clone() };
    let res = ev.evaluate(&f.doc, &asm2, &SolveOptions::default(), None);
    assert!(!res.mates[0].ok);
}

#[test]
fn width_mate_centers_tab() {
    let f = doc_with(vec![block_studio()]);
    let mut ev = AssemblyEvaluator::new();
    let (body, r) = body_of(&mut ev, &f.doc, f.studios[0]);
    let top = face_with(&r, "CapEnd");
    let bottom = face_with(&r, "CapStart");
    // Walls: block 1 (z 0..10 mm) and block 2 (z 30..40 mm), both fixed; the gap is z 10..30.
    let walls = vec![
        part_instance(1, "Wall <1>", f.studios[0], body.clone(), DVec3::ZERO, true),
        part_instance(2, "Wall <2>", f.studios[0], body.clone(), DVec3::new(0.0, 0.0, 0.03), true),
        part_instance(3, "Tab <1>", f.studios[0], body.clone(), DVec3::new(0.05, 0.0, 0.002), false),
    ];
    // Two tab connectors (tab's bottom and top faces): its mid-plane centers at z = 20 mm.
    let mut m = mate(10, MateType::Width, conn(3, bottom.clone(), Inference::Centroid), conn(1, top.clone(), Inference::Centroid));
    m.extra = vec![conn(2, bottom.clone(), Inference::Centroid), conn(3, top.clone(), Inference::Centroid)];
    let asm = Assembly { instances: walls.clone(), mates: vec![m.clone()], ..Default::default() };
    let res = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
    assert!(res.mates[0].ok, "{:?}", res.mates[0].message);
    let o = origin_of(&res, 3);
    assert!((o.z - 0.015).abs() < 1e-6, "tab centered: {o:?}");
    let p = res.solved.placement(InstanceId(3)).unwrap();
    assert!((quat_of(&p) * DVec3::Z).z.abs() > 1.0 - 1e-6, "tab stays parallel to the walls");
    // Tab may slide in the center plane and spin about its normal: 3 DOF left.
    assert_eq!(res.nodes[2].dof, Some(3));
    // Single tab connector (its top face) sits on the center plane.
    let mut m1 = m.clone();
    m1.a = conn(3, top.clone(), Inference::Centroid);
    m1.extra.truncate(1);
    let asm = Assembly { instances: walls, mates: vec![m1], ..Default::default() };
    let res = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
    assert!(res.mates[0].ok, "{:?}", res.mates[0].message);
    assert!((origin_of(&res, 3).z - 0.01).abs() < 1e-6, "{:?}", origin_of(&res, 3));
}

#[test]
fn relation_measured_from_stored_reference() {
    let f = doc_with(vec![plate_with_hole(), pin_studio()]);
    let mut ev = AssemblyEvaluator::new();
    let (plate, rp) = body_of(&mut ev, &f.doc, f.studios[0]);
    let (pin, rq) = body_of(&mut ev, &f.doc, f.studios[1]);
    let hole = cylinder_face(&rp);
    let pf = cylinder_face(&rq);
    let mut asm = Assembly {
        instances: vec![
            part_instance(1, "Plate <1>", f.studios[0], plate, DVec3::ZERO, true),
            part_instance(2, "Pin <1>", f.studios[1], pin.clone(), DVec3::ZERO, false),
            part_instance(3, "Pin <2>", f.studios[1], pin, DVec3::ZERO, false),
        ],
        mates: vec![
            mate(10, MateType::Revolute, conn(1, hole.clone(), Inference::MidAxis), conn(2, pf.clone(), Inference::MidAxis)),
            mate(11, MateType::Revolute, conn(1, hole, Inference::MidAxis), conn(3, pf, Inference::MidAxis)),
        ],
        ..Default::default()
    };
    let res = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
    // Store the reference when the relation is created.
    let base = res.relation_base(&asm, RelationType::Gear, [MateId(10), MateId(11)]).expect("base");
    assert!((base[0] - res.solved.mate_values[&MateId(10)][0]).abs() < 1e-12);
    for (id, p) in &res.solved.placements {
        asm.instance_mut(*id).unwrap().placement = *p;
    }
    asm.relations.push(Relation { id: MateId(40), name: "Gear 1".into(), relation_type: RelationType::Gear, mates: [MateId(10), MateId(11)], ratio: ExprValue::new("2"), reverse: false, suppressed: false, base: Some(base) });
    // Turn pin 1 by 0.2 rad in the stored placements (as an outside edit would):
    // the relation is enforced against the stored reference, not the new start.
    let p2 = asm.instance(InstanceId(2)).unwrap().placement;
    let q = DQuat::from_rotation_z(0.2) * quat_of(&p2);
    asm.instance_mut(InstanceId(2)).unwrap().placement.rotation = [q.x, q.y, q.z, q.w];
    let res = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
    assert!(res.relations[0].ok, "{:?}", res.relations[0].message);
    let wrap = |a: f64| (a + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI;
    let da = wrap(res.solved.mate_values[&MateId(10)][0] - base[0]);
    let db = wrap(res.solved.mate_values[&MateId(11)][0] - base[1]);
    assert!(da.abs() > 0.01, "the start offset is kept: {da}");
    assert!(wrap(db + 2.0 * da).abs() < 1e-4, "absolute coupling: da={da} db={db}");
    // Incompatible mate types are reported.
    asm.relations[0].relation_type = RelationType::Linear;
    let res = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
    assert!(!res.relations[0].ok);
    assert!(res.relations[0].message.as_deref().unwrap_or("").contains("translational"));
}

fn x_axis_connector(id: u64) -> AsmMateConnector {
    // Assembly origin connector turned so its Z axis points along +X.
    let mut c = AsmMateConnector::new(MateId(id), format!("Mate connector {id}"), MateConnectorRef::origin());
    c.rotation_axis = 1;
    c.rotation = Some(ExprValue::new("90 deg"));
    c
}

#[test]
fn linear_and_circular_instance_patterns() {
    let f = doc_with(vec![block_studio()]);
    let mut ev = AssemblyEvaluator::new();
    let (body, r) = body_of(&mut ev, &f.doc, f.studios[0]);
    let top = face_with(&r, "CapEnd");
    let bottom = face_with(&r, "CapStart");
    let mut asm = Assembly {
        instances: vec![part_instance(1, "Base <1>", f.studios[0], body.clone(), DVec3::ZERO, true), part_instance(2, "Block <1>", f.studios[0], body.clone(), DVec3::new(0.0, 0.1, 0.0), false)],
        mates: vec![mate(10, MateType::Fastened, conn(1, top, Inference::Centroid), conn(2, bottom, Inference::Centroid))],
        connectors: vec![x_axis_connector(60)],
        ..Default::default()
    };
    asm.patterns.push(InstancePattern {
        id: MateId(70),
        name: "Linear pattern 1".into(),
        kind: PatternKind::Linear { direction: MateConnectorRef::assembly_connector(MateId(60), InstanceId::ORIGIN), distance: ExprValue::new("50 mm"), count: 3, opposite: false, equal_spacing: false, second: None },
        seeds: vec![InstanceId(2)],
        generated: vec![],
        suppressed: false,
    });
    let created = asm.sync_pattern(MateId(70));
    assert_eq!(created.len(), 2);
    let res = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
    assert!(res.pattern_errors.is_empty(), "{:?}", res.pattern_errors);
    let seed = origin_of(&res, 2);
    assert!((seed - DVec3::new(0.0, 0.0, 0.01)).length() < 1e-6);
    for (k, id) in created.iter().enumerate() {
        let o = DVec3::from(res.solved.placement(*id).unwrap().translation);
        assert!((o - seed - DVec3::new(0.05 * (k + 1) as f64, 0.0, 0.0)).length() < 1e-6, "copy {k}: {o:?}");
        assert_eq!(res.solved.instance_dof[id], 0, "copies follow the seed");
    }
    assert_eq!(bom(&res)[0].quantity, 4);
    // Fewer instances: one copy removed.
    if let PatternKind::Linear { count, .. } = &mut asm.patterns[0].kind {
        *count = 2;
    }
    asm.sync_pattern(MateId(70));
    assert_eq!(asm.instances.len(), 3);
    assert_eq!(asm.patterns[0].generated.len(), 1);
    // Suppressing the pattern deactivates its copies.
    asm.patterns[0].suppressed = true;
    let res = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
    assert_eq!(res.occurrences.len(), 2);
    asm.patterns[0].suppressed = false;

    // Circular: 4 copies of a free block about the origin Z axis, equally spaced over 360°.
    let mut asm = Assembly {
        instances: vec![part_instance(1, "Block <1>", f.studios[0], body.clone(), DVec3::new(0.1, 0.0, 0.0), false)],
        ..Default::default()
    };
    asm.patterns.push(InstancePattern {
        id: MateId(71),
        name: "Circular pattern 1".into(),
        kind: PatternKind::Circular { axis: MateConnectorRef::origin(), angle: ExprValue::new("360 deg"), count: 4, opposite: false, equal_spacing: true },
        seeds: vec![InstanceId(1)],
        generated: vec![],
        suppressed: false,
    });
    let created = asm.sync_pattern(MateId(71));
    assert_eq!(created.len(), 3);
    let res = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
    let p = res.solved.placement(created[0]).unwrap();
    assert!((DVec3::from(p.translation) - DVec3::new(0.0, 0.1, 0.0)).length() < 1e-6, "{p:?}");
    assert!(((quat_of(&p) * DVec3::X) - DVec3::Y).length() < 1e-6);
    let p = res.solved.placement(created[1]).unwrap();
    assert!((DVec3::from(p.translation) - DVec3::new(-0.1, 0.0, 0.0)).length() < 1e-6, "{p:?}");
    // Dragging the seed carries the copies around.
    let s = res.prepared.solve(&SolveOptions { targets: vec![(InstanceId(1), Placement { translation: [0.2, 0.0, 0.0], rotation: [0.0, 0.0, 0.0, 1.0] })], ..Default::default() });
    let p = s.placement(created[0]).unwrap();
    assert!((DVec3::from(p.translation) - DVec3::new(0.0, 0.2, 0.0)).length() < 1e-5, "{p:?}");
    // Deleting the pattern deletes the copies.
    asm.remove_mate(MateId(71));
    assert_eq!(asm.instances.len(), 1);
}

#[test]
fn mirror_instances_across_connector_plane() {
    let f = doc_with(vec![block_studio()]);
    let mut ev = AssemblyEvaluator::new();
    let (body, _) = body_of(&mut ev, &f.doc, f.studios[0]);
    let mut asm = Assembly {
        instances: vec![part_instance(1, "Block <1>", f.studios[0], body, DVec3::new(0.05, 0.0, 0.0), true)],
        connectors: vec![x_axis_connector(60)],
        ..Default::default()
    };
    asm.patterns.push(InstancePattern {
        id: MateId(70),
        name: "Mirror 1".into(),
        kind: PatternKind::Mirror { plane: MateConnectorRef::assembly_connector(MateId(60), InstanceId::ORIGIN), handed: false },
        seeds: vec![InstanceId(1)],
        generated: vec![],
        suppressed: false,
    });
    let created = asm.sync_pattern(MateId(70));
    assert_eq!(created.len(), 1);
    let res = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
    let occ = res.occurrences.iter().find(|o| o.top() == created[0]).unwrap();
    let (lo, hi) = occ.world_bounds().unwrap();
    // Seed spans x 50..90 mm, y 0..20, z 0..10; the mirror image spans x -90..-50.
    assert!((lo - DVec3::new(-0.09, 0.0, 0.0)).length() < 1e-6 && (hi - DVec3::new(-0.05, 0.02, 0.01)).length() < 1e-6, "{lo:?} {hi:?}");
    // A proper rigid motion (no reflection in the stored placement).
    let p = res.solved.placement(created[0]).unwrap();
    assert!((quat_of(&p).length() - 1.0).abs() < 1e-9);
}

/// 80 x 40 x 10 mm plate with two 5 mm holes at x = ±20 mm and a 3 mm hole at (0, 10).
fn plate_with_holes() -> PartStudio {
    let mut sk = noshape_sketch::Sketch::new();
    rect(&mut sk, -0.04, -0.02, 0.08, 0.04);
    sk.add_circle(DVec2::new(-0.02, 0.0), 0.005);
    sk.add_circle(DVec2::new(0.02, 0.0), 0.005);
    sk.add_circle(DVec2::new(0.0, 0.01), 0.003);
    extruded_studio(sk, "10 mm", true)
}

#[test]
fn replicate_on_matching_holes() {
    let f = doc_with(vec![plate_with_holes(), pin_studio()]);
    let mut ev = AssemblyEvaluator::new();
    let (plate, rp) = body_of(&mut ev, &f.doc, f.studios[0]);
    let (pin, rq) = body_of(&mut ev, &f.doc, f.studios[1]);
    // The 5 mm hole at x = -20 mm.
    let b = &rp.context.bodies[0];
    let hole = b
        .faces
        .iter()
        .enumerate()
        .find_map(|(i, n)| match b.shape.sub_shape(noshape_kernel::ShapeType::Face, i).ok()?.face_surface().ok()? {
            noshape_kernel::Surface::Cylinder { origin, radius, .. } if (radius - 0.005).abs() < 1e-9 && origin.x < 0.0 => Some(Reference::Face((**n).clone())),
            _ => None,
        })
        .unwrap();
    let pf = cylinder_face(&rq);
    let mut asm = Assembly {
        instances: vec![part_instance(1, "Plate <1>", f.studios[0], plate, DVec3::ZERO, true), part_instance(2, "Pin <1>", f.studios[1], pin, DVec3::new(0.0, 0.05, 0.0), false)],
        mates: vec![mate(10, MateType::Revolute, conn(1, hole, Inference::MidAxis), conn(2, pf, Inference::MidAxis))],
        ..Default::default()
    };
    let res = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
    assert!(res.mates[0].ok);
    asm.patterns.push(InstancePattern { id: MateId(80), name: "Replicate 1".into(), kind: PatternKind::Replicate { mate: MateId(10), all_instances: false }, seeds: vec![InstanceId(2)], generated: vec![], suppressed: false });
    let targets = analysis::matched_targets(&res, &asm, &asm.patterns[0], &UnitPrefs::default()).unwrap();
    assert_eq!(targets.len(), 1, "only the other 5 mm hole matches");
    let created = asm.sync_matched_pattern(MateId(80), targets);
    assert_eq!(created.len(), 1);
    assert_eq!(asm.mates.len(), 2);
    let res = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
    assert!(res.mates.iter().all(|m| m.ok), "{:?}", res.mates.iter().map(|m| &m.message).collect::<Vec<_>>());
    let o = DVec3::from(res.solved.placement(created[0]).unwrap().translation);
    assert!((o.truncate() - glam::DVec2::new(0.02, 0.0)).length() < 1e-6, "copy in the matching hole: {o:?}");
    assert_eq!(res.solved.instance_dof[&created[0]], 1, "copy is revolute like the seed");
    // Including other instances of the plate: a second plate adds its two 5 mm holes.
    let mut a3 = asm.clone();
    let plate2 = Instance { id: InstanceId(50), name: "Plate <2>".into(), placement: Placement { translation: [0.0, 0.1, 0.0], rotation: [0.0, 0.0, 0.0, 1.0] }, ..a3.instances[0].clone() };
    a3.instances.push(plate2);
    a3.remove_mate(MateId(80));
    a3.patterns.clear();
    a3.patterns.push(InstancePattern { id: MateId(81), name: "Replicate 2".into(), kind: PatternKind::Replicate { mate: MateId(10), all_instances: true }, seeds: vec![InstanceId(2)], generated: vec![], suppressed: false });
    let r3 = ev.evaluate(&f.doc, &a3, &SolveOptions::default(), None);
    let t3 = analysis::matched_targets(&r3, &a3, &a3.patterns[0], &UnitPrefs::default()).unwrap();
    assert_eq!(t3.len(), 3, "other hole on plate 1 + both holes on plate 2");
    assert_eq!(t3.iter().filter(|(_, c, _)| c.instance == InstanceId(50)).count(), 2);
    // Dissolving keeps the instance and mate; deleting the seed mate removes the replicate.
    let mut a2 = asm.clone();
    a2.dissolve_pattern(MateId(80));
    assert_eq!((a2.instances.len(), a2.mates.len(), a2.patterns.len()), (3, 2, 0));
    asm.remove_mate(MateId(10));
    assert_eq!((asm.instances.len(), asm.mates.len(), asm.patterns.len()), (2, 0, 0));
}

#[test]
fn part_pattern_follows_studio_pattern() {
    // Plate with one hole, the hole face-patterned twice along X (30 mm) in the Part Studio.
    let units = UnitPrefs::default();
    let mut ps = plate_with_holes();
    let mut reg = Regenerator::new();
    let r = reg.regenerate(&ps, &ConfigValues::default(), &units, &RegenOptions::default());
    let b = &r.context.bodies[0];
    let small = b
        .faces
        .iter()
        .enumerate()
        .find_map(|(i, n)| match b.shape.sub_shape(noshape_kernel::ShapeType::Face, i).ok()?.face_surface().ok()? {
            noshape_kernel::Surface::Cylinder { radius, .. } if (radius - 0.003).abs() < 1e-9 => Some(Reference::Face((**n).clone())),
            _ => None,
        })
        .unwrap();
    let mut pat = ps.new_feature("linearPattern", &units).unwrap();
    pat.params.set("patternType", ParamValue::Enum("FACE".into()));
    pat.params.set("faces", ParamValue::Query(vec![small.clone()]));
    pat.params.set("directionOne", ParamValue::Query(vec![Reference::Plane(DatumPlane::Right)]));
    pat.params.set("distance", ParamValue::Expr(ExprValue::new("10 mm")));
    pat.params.set("instanceCount", ParamValue::Expr(ExprValue::new("3")));
    ps.features.push(pat);
    let f = doc_with(vec![ps, pin_studio()]);
    let mut ev = AssemblyEvaluator::new();
    let (plate, rp) = body_of(&mut ev, &f.doc, f.studios[0]);
    eprintln!("{:?}", rp.display.features.iter().map(|x| (&x.status, &x.message)).collect::<Vec<_>>());
    let (pin, rq) = body_of(&mut ev, &f.doc, f.studios[1]);
    let pf = cylinder_face(&rq);
    let mut asm = Assembly {
        instances: vec![part_instance(1, "Plate <1>", f.studios[0], plate, DVec3::ZERO, true), part_instance(2, "Pin <1>", f.studios[1], pin, DVec3::new(0.0, 0.05, 0.0), false)],
        mates: vec![mate(10, MateType::Revolute, conn(1, small, Inference::MidAxis), conn(2, pf, Inference::MidAxis))],
        ..Default::default()
    };
    let res = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
    assert!(res.mates[0].ok);
    asm.patterns.push(InstancePattern { id: MateId(80), name: "Part pattern 1".into(), kind: PatternKind::PartPattern { mate: MateId(10) }, seeds: vec![InstanceId(2)], generated: vec![], suppressed: false });
    let targets = analysis::matched_targets(&res, &asm, &asm.patterns[0], &units).unwrap();
    assert_eq!(targets.len(), 2, "one copy per patterned hole");
    let created = asm.sync_matched_pattern(MateId(80), targets);
    let res = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
    assert!(res.mates.iter().all(|m| m.ok), "{:?}", res.mates.iter().map(|m| &m.message).collect::<Vec<_>>());
    let mut xs: Vec<f64> = created.iter().map(|id| res.solved.placement(*id).unwrap().translation[0]).collect();
    xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    assert!((xs[0] - 0.01).abs() < 1e-6 && (xs[1] - 0.02).abs() < 1e-6, "{xs:?}");
}

#[test]
fn snap_explode_progress_replace_and_config() {
    // The second studio is a copy of the first (same feature ids, so same face names).
    let s = block_studio();
    let f = doc_with(vec![s.clone(), s]);
    let mut ev = AssemblyEvaluator::new();
    let (body, r) = body_of(&mut ev, &f.doc, f.studios[0]);
    let top = face_with(&r, "CapEnd");
    let bottom = face_with(&r, "CapStart");
    let mut asm = Assembly {
        instances: vec![part_instance(1, "Block <1>", f.studios[0], body.clone(), DVec3::ZERO, true), part_instance(2, "Block <2>", f.studios[0], body.clone(), DVec3::new(0.1, 0.05, 0.02), false)],
        ..Default::default()
    };
    let res = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
    let units = UnitPrefs::default();
    // Snap block 2's bottom face onto block 1's top face.
    let src = res.connector_world(&conn(2, bottom.clone(), Inference::Centroid), &units).unwrap();
    let dst = res.connector_world(&conn(1, top.clone(), Inference::Centroid), &units).unwrap();
    let np = eval::snap_placement(&asm.instances[1].placement, &src, &dst);
    assert!((DVec3::from(np.translation) - DVec3::new(0.0, 0.0, 0.01)).length() < 1e-9, "{np:?}");
    asm.instances[1].placement = np;
    asm.mates.push(mate(10, MateType::Fastened, conn(2, bottom.clone(), Inference::Centroid), conn(1, top.clone(), Inference::Centroid)));
    let res = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
    assert!(res.mates[0].ok);
    assert!((origin_of(&res, 2) - DVec3::new(0.0, 0.0, 0.01)).length() < 1e-9, "already in place");

    // Exploded view played halfway through its second step.
    let view = ExplodedView { name: "Explode 1".into(), steps: vec![ExplodeStep::translate(vec![InstanceId(2)], [0.0, 0.0, 0.04]), ExplodeStep::rotate(vec![InstanceId(2)], [0.0, 0.0, 1.0], std::f64::consts::PI, [0.0, 0.0, 0.0])] };
    let mut r2 = res.clone();
    r2.apply_placements_progress(res.solved.clone(), Some((&view, 1.5)));
    let o = r2.occurrences[1].transform.transform_point3(DVec3::ZERO);
    assert!((o - DVec3::new(0.0, 0.0, 0.05)).length() < 1e-9, "after the first step, rotated 90° about the origin: {o:?}");
    let x = r2.occurrences[1].transform.transform_vector3(DVec3::X);
    assert!((x - DVec3::Y).length() < 1e-9, "{x:?}");
    r2.apply_placements_progress(res.solved.clone(), Some((&view, 0.5)));
    assert!((r2.occurrences[1].transform.transform_point3(DVec3::ZERO) - DVec3::new(0.0, 0.0, 0.03)).length() < 1e-9);

    // Replace block 2 by the second studio's block: same face names, so the mate keeps resolving.
    let (body2, _) = body_of(&mut ev, &f.doc, f.studios[1]);
    let replaced = asm.replace_instance(InstanceId(2), InstanceSource::Part { studio: f.studios[1], body: body2, config: ConfigValues::default() }, "Part 2", false);
    assert_eq!(replaced, vec![InstanceId(2)]);
    assert_eq!(asm.instances[1].name, "Part 2 <1>");
    let res = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
    assert!(res.mates[0].ok, "{:?}", res.mates[0].message);
    assert_eq!(res.occurrences[1].studio, f.studios[1]);
    // Per-instance configuration is stored on the source.
    let mut cfg = ConfigValues::default();
    cfg.0.insert("Size".into(), "Large".into());
    assert!(asm.set_instance_config(InstanceId(2), cfg.clone()));
    assert!(matches!(&asm.instances[1].source, InstanceSource::Part { config, .. } if *config == cfg));
}

// ------------------------------------------------------------ remaining gaps

fn with_feature(mut ps: PartStudio, kind: &str, set: impl FnOnce(&mut crate::partstudio::Feature, &crate::regen::RegenResult)) -> PartStudio {
    let units = UnitPrefs::default();
    let mut reg = Regenerator::new();
    let r = reg.regenerate(&ps, &ConfigValues::default(), &units, &RegenOptions::default());
    let mut f = ps.new_feature(kind, &units).unwrap();
    set(&mut f, &r);
    ps.features.push(f);
    ps
}

/// Edges of the first body whose name mentions `needle`.
fn edges_with(r: &crate::regen::RegenResult, needle: &str) -> Vec<Reference> {
    r.context.bodies[0].edges.iter().filter(|e| format!("{e:?}").contains(needle)).map(|e| Reference::Edge((**e).clone())).collect()
}

/// Face of the first body with a given surface kind.
fn face_of_kind(r: &crate::regen::RegenResult, pred: impl Fn(&noshape_kernel::Surface) -> bool) -> Reference {
    let b = &r.context.bodies[0];
    for (i, f) in b.faces.iter().enumerate() {
        let face = b.shape.sub_shape(noshape_kernel::ShapeType::Face, i).unwrap();
        if face.face_surface().is_ok_and(|s| pred(&s)) {
            return Reference::Face((**f).clone());
        }
    }
    panic!("no face of the requested kind");
}

#[test]
fn explode_lines_and_step_reorder() {
    let f = doc_with(vec![block_studio()]);
    let mut ev = AssemblyEvaluator::new();
    let (body, _) = body_of(&mut ev, &f.doc, f.studios[0]);
    let asm = Assembly {
        instances: vec![part_instance(1, "Block <1>", f.studios[0], body.clone(), DVec3::ZERO, true), part_instance(2, "Block <2>", f.studios[0], body, DVec3::new(0.0, 0.0, 0.01), false)],
        ..Default::default()
    };
    let res = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
    let mut view = ExplodedView { name: "Explode 1".into(), steps: vec![ExplodeStep::translate(vec![InstanceId(2)], [0.0, 0.0, 0.05]), ExplodeStep::translate(vec![InstanceId(2)], [0.03, 0.0, 0.0])] };
    // One line per step and instance, from the start to the exploded position.
    let lines = eval::explode_trails(&res, &view, f64::INFINITY);
    assert_eq!(lines.len(), 2);
    let c0 = DVec3::new(0.02, 0.01, 0.015);
    assert!((lines[0].0 - c0).length() < 1e-9 && (lines[0].1 - (c0 + DVec3::new(0.0, 0.0, 0.05))).length() < 1e-9, "{lines:?}");
    assert!((lines[1].0 - lines[0].1).length() < 1e-9, "the second line starts where the first ends");
    // Played halfway: only the first line, half long; the display result carries them.
    let mut r2 = res.clone();
    r2.apply_placements_progress(res.solved.clone(), Some((&view, 0.5)));
    assert_eq!(r2.explode_trails.len(), 1);
    assert!((r2.explode_trails[0].1.z - (c0.z + 0.025)).abs() < 1e-9);
    // Reordering steps.
    assert!(view.move_step(1, 0));
    assert_eq!(view.steps[0].translation, [0.03, 0.0, 0.0]);
    assert!(!view.move_step(0, 5));
    let lines = eval::explode_trails(&res, &view, f64::INFINITY);
    assert!((lines[0].1 - (c0 + DVec3::new(0.03, 0.0, 0.0))).length() < 1e-9);
}

#[test]
fn named_position_restores_driving_mate_values() {
    let f = doc_with(vec![plate_with_hole(), pin_studio(), block_studio()]);
    let mut ev = AssemblyEvaluator::new();
    let (plate, rp) = body_of(&mut ev, &f.doc, f.studios[0]);
    let (pin, rq) = body_of(&mut ev, &f.doc, f.studios[1]);
    let (block, rb) = body_of(&mut ev, &f.doc, f.studios[2]);
    let mut asm = Assembly {
        instances: vec![
            part_instance(1, "Plate <1>", f.studios[0], plate, DVec3::ZERO, true),
            part_instance(2, "Pin <1>", f.studios[1], pin, DVec3::new(0.05, 0.04, 0.02), false),
            part_instance(3, "Block <1>", f.studios[2], block, DVec3::new(0.0, 0.0, 0.05), false),
        ],
        mates: vec![
            mate(10, MateType::Revolute, conn(1, cylinder_face(&rp), Inference::MidAxis), conn(2, cylinder_face(&rq), Inference::MidAxis)),
            mate(11, MateType::Slider, conn(1, face_with(&rp, "CapEnd"), Inference::Centroid), conn(3, face_with(&rb, "CapStart"), Inference::Centroid)),
        ],
        ..Default::default()
    };
    let res = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
    assert!(res.mates.iter().all(|m| m.ok), "{:?}", res.mates.iter().map(|m| &m.message).collect::<Vec<_>>());
    // Turn the pin to 0.7 rad and slide the block 30 mm up, then save the position.
    let start: std::collections::HashMap<_, _> = res.solved.placements.iter().cloned().collect();
    let s = res.prepared.solve(&SolveOptions { drive: Some((MateId(10), 0.7)), start: Some(start), ..Default::default() });
    let start: std::collections::HashMap<_, _> = s.placements.iter().cloned().collect();
    let s = res.prepared.solve(&SolveOptions { drive: Some((MateId(11), 0.03)), start: Some(start), ..Default::default() });
    let mut saved = res.clone();
    saved.apply_placements(s.clone(), None);
    let values = saved.named_position_values(&asm);
    assert_eq!(values.len(), 2, "revolute and slider values are stored");
    let np = NamedPosition { name: "Open".into(), placements: s.placements.clone(), mate_values: values };
    // Move away (both mates back to 0), then restore: the stored values come back
    // even from placements that do not match them.
    for (id, p) in &res.solved.placements {
        asm.instance_mut(*id).unwrap().placement = *p;
    }
    let mut np_stale = np.clone();
    np_stale.placements = res.solved.placements.clone();
    let res = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
    let r = res.prepared.solve_named_position(&np_stale);
    assert!(r.converged);
    let wrap = |a: f64| (a + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI;
    assert!(wrap(r.mate_values[&MateId(10)][0] - 0.7).abs() < 1e-5, "pin angle restored: {:?}", r.mate_values[&MateId(10)]);
    assert!((r.mate_values[&MateId(11)][3] - 0.03).abs() < 1e-6, "slider distance restored: {:?}", r.mate_values[&MateId(11)]);
    // The restored positions still satisfy the mates (with their own free values).
    assert_eq!(r.mate_ok.values().filter(|ok| !**ok).count(), 0);
    // Old documents without stored values just use the placements.
    let old: NamedPosition = serde_json::from_str(r#"{"name":"Old","placements":[]}"#).unwrap();
    assert!(old.mate_values.is_empty());
}

#[test]
fn replicate_updates_when_target_geometry_changes() {
    let mut f = doc_with(vec![plate_with_holes(), pin_studio()]);
    let mut ev = AssemblyEvaluator::new();
    let (plate, rp) = body_of(&mut ev, &f.doc, f.studios[0]);
    let (pin, rq) = body_of(&mut ev, &f.doc, f.studios[1]);
    let b = &rp.context.bodies[0];
    let hole = b
        .faces
        .iter()
        .enumerate()
        .find_map(|(i, n)| match b.shape.sub_shape(noshape_kernel::ShapeType::Face, i).ok()?.face_surface().ok()? {
            noshape_kernel::Surface::Cylinder { origin, radius, .. } if (radius - 0.005).abs() < 1e-9 && origin.x < 0.0 => Some(Reference::Face((**n).clone())),
            _ => None,
        })
        .unwrap();
    let mut asm = Assembly {
        instances: vec![part_instance(1, "Plate <1>", f.studios[0], plate, DVec3::ZERO, true), part_instance(2, "Pin <1>", f.studios[1], pin, DVec3::new(0.0, 0.05, 0.0), false)],
        mates: vec![mate(10, MateType::Revolute, conn(1, hole, Inference::MidAxis), conn(2, cylinder_face(&rq), Inference::MidAxis))],
        ..Default::default()
    };
    asm.patterns.push(InstancePattern { id: MateId(80), name: "Replicate 1".into(), kind: PatternKind::Replicate { mate: MateId(10), all_instances: false }, seeds: vec![InstanceId(2)], generated: vec![], suppressed: false });
    let units = UnitPrefs::default();
    let res = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
    // A new pattern is stale until synced; afterwards it is up to date.
    let stale = analysis::stale_matched_patterns(&res, &asm, &units);
    assert_eq!(stale.len(), 1);
    for (id, t) in stale {
        asm.sync_matched_pattern(id, t);
    }
    let res = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
    assert!(analysis::stale_matched_patterns(&res, &asm, &units).is_empty());
    assert_eq!(asm.patterns[0].generated.len(), 1);
    // The Part Studio gains two more 5 mm holes (sketch edited in place, same
    // features): the replicate is stale and re-syncs to three copies.
    if let TabContent::PartStudio(ps) = &mut f.doc.tabs[0].content
        && let Some(ParamValue::Sketch(sd)) = ps.features[0].params.get("sketch").cloned()
    {
        let mut sd = sd;
        sd.sketch.add_circle(DVec2::new(-0.02, 0.012), 0.005);
        sd.sketch.add_circle(DVec2::new(0.02, 0.012), 0.005);
        ps.features[0].params.set("sketch", ParamValue::Sketch(sd));
    }
    let res = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
    let stale = analysis::stale_matched_patterns(&res, &asm, &units);
    assert_eq!(stale.len(), 1, "geometry changed: pattern is stale");
    assert_eq!(stale[0].1.len(), 3);
    for (id, t) in stale {
        asm.sync_matched_pattern(id, t);
    }
    assert_eq!(asm.patterns[0].generated.len(), 3);
    let res = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
    assert!(res.mates.iter().all(|m| m.ok), "{:?}", res.mates.iter().map(|m| &m.message).collect::<Vec<_>>());
    assert!(analysis::stale_matched_patterns(&res, &asm, &units).is_empty());
}

#[test]
fn handed_mirror_creates_mirrored_part() {
    let f = doc_with(vec![plate_with_holes()]);
    let mut ev = AssemblyEvaluator::new();
    let (body, r) = body_of(&mut ev, &f.doc, f.studios[0]);
    let small = face_of_kind(&r, |s| matches!(s, noshape_kernel::Surface::Cylinder { radius, .. } if (radius - 0.003).abs() < 1e-9));
    // Mirror plane: the assembly XZ plane (connector Z along +Y).
    let mut plane = AsmMateConnector::new(MateId(60), "Mate connector 1".into(), MateConnectorRef::origin());
    plane.rotation_axis = 0;
    plane.rotation = Some(ExprValue::new("-90 deg"));
    let mut asm = Assembly { instances: vec![part_instance(1, "Plate <1>", f.studios[0], body.clone(), DVec3::new(0.0, 0.1, 0.0), true)], connectors: vec![plane], ..Default::default() };
    asm.patterns.push(InstancePattern {
        id: MateId(70),
        name: "Mirror 1".into(),
        kind: PatternKind::Mirror { plane: MateConnectorRef::assembly_connector(MateId(60), InstanceId::ORIGIN), handed: true },
        seeds: vec![InstanceId(1)],
        generated: vec![],
        suppressed: false,
    });
    let created = asm.sync_pattern(MateId(70));
    assert!(matches!(asm.instance(created[0]).unwrap().source, InstanceSource::MirroredPart { .. }));
    let res = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
    assert!(res.pattern_errors.is_empty(), "{:?}", res.pattern_errors);
    let occ = res.occurrences.iter().find(|o| o.top() == created[0]).unwrap();
    assert!(occ.mirrored);
    // The small hole (y = +10 mm on the seed, so world y = 110 mm) lands at
    // y = -110 mm: a true mirror image (a rigid copy would put it at -90 mm).
    let units = UnitPrefs::default();
    let w = res.connector_world(&conn(created[0].0, small.clone(), Inference::Centroid), &units).expect("mate connectors resolve on the mirrored part");
    assert!((w.origin.y + 0.11).abs() < 1e-6 && w.origin.x.abs() < 1e-6, "{:?}", w.origin);
    let (lo, hi) = occ.world_bounds().unwrap();
    assert!((lo.y + 0.12).abs() < 1e-6 && (hi.y + 0.08).abs() < 1e-6, "{lo:?} {hi:?}");
    // Kernel shape mirrored too (interference, measurements): same volume, same face names.
    let mb = occ.context.body(&occ.body).unwrap();
    assert_eq!(mb.faces.len(), r.context.bodies[0].faces.len());
    assert!((mb.shape.volume().unwrap() - r.context.bodies[0].shape.volume().unwrap()).abs() < 1e-12);
    // The BOM lists the mirrored part separately.
    let rows = bom(&res);
    assert_eq!(rows.len(), 2);
    assert!(rows[1].mirrored && rows[1].name.ends_with("(mirrored)"));
    // Turning the option off makes the copy a plain (rigid) part again.
    if let PatternKind::Mirror { handed, .. } = &mut asm.patterns[0].kind {
        *handed = false;
    }
    asm.sync_pattern(MateId(70));
    assert!(matches!(asm.instance(created[0]).unwrap().source, InstanceSource::Part { .. }));
    let res = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
    let w = res.connector_world(&conn(created[0].0, small, Inference::Centroid), &units).unwrap();
    assert!((w.origin.y + 0.09).abs() < 1e-6, "rigid copy: {:?}", w.origin);
}

#[test]
fn replace_instance_matches_geometry_when_names_differ() {
    // A block whose features have other ids (a dummy sketch first), so no face names match.
    let units = UnitPrefs::default();
    let mut other = PartStudio::new();
    let mut dummy = other.new_feature("sketch", &units).unwrap();
    dummy.params.set("sketchPlane", ParamValue::Query(vec![Reference::Plane(DatumPlane::Front)]));
    other.features.push(dummy);
    let base = block_studio();
    for mut ft in base.features.clone() {
        let fresh = other.new_feature(&ft.type_id, &units).unwrap();
        ft.id = fresh.id;
        // Re-point the extrude's region query at the renumbered sketch.
        if let Some(ParamValue::Query(q)) = ft.params.get("entities").cloned() {
            let q: Vec<Reference> = q
                .into_iter()
                .map(|r| match r {
                    Reference::SketchRegion { sketch, key } if sketch == base.features[0].id => Reference::SketchRegion { sketch: other.features[1].id, key },
                    r => r,
                })
                .collect();
            ft.params.set("entities", ParamValue::Query(q));
        }
        other.features.push(ft);
    }
    let f = doc_with(vec![block_studio(), other]);
    let mut ev = AssemblyEvaluator::new();
    let (body, r) = body_of(&mut ev, &f.doc, f.studios[0]);
    let (body2, r2) = body_of(&mut ev, &f.doc, f.studios[1]);
    let top = face_with(&r, "CapEnd");
    let bottom = face_with(&r, "CapStart");
    assert!(r2.context.bodies[0].find_faces(match &bottom {
        Reference::Face(n) => n,
        _ => unreachable!(),
    })
    .is_empty(), "names must differ for this test");
    let mut asm = Assembly {
        instances: vec![part_instance(1, "Block <1>", f.studios[0], body.clone(), DVec3::ZERO, true), part_instance(2, "Block <2>", f.studios[0], body, DVec3::new(0.0, 0.0, 0.01), false)],
        mates: vec![mate(10, MateType::Fastened, conn(1, top, Inference::Centroid), conn(2, bottom, Inference::Centroid))],
        ..Default::default()
    };
    let old = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
    assert!(old.mates[0].ok);
    let ids = asm.replace_instance(InstanceId(2), InstanceSource::Part { studio: f.studios[1], body: body2.clone(), config: ConfigValues::default() }, "Block B", false);
    // Without remapping the mate loses its geometry.
    let broken = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
    assert!(!broken.mates[0].ok);
    let n = analysis::remap_replaced(&mut asm, &ids, &old, &r2.context, Some(&body2));
    assert_eq!(n, 1);
    let res = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
    assert!(res.mates[0].ok, "{:?}", res.mates[0].message);
    assert!((origin_of(&res, 2) - DVec3::new(0.0, 0.0, 0.01)).length() < 1e-6, "{:?}", origin_of(&res, 2));
    // The bottom face was matched (same type, nearest position and normal), not the top one.
    let Reference::Implicit { on, .. } = &asm.mates[0].b.reference else { panic!() };
    let bf = r2.context.bodies[0].find_faces(match &**on {
        Reference::Face(n) => n,
        _ => panic!(),
    })[0];
    let face = r2.context.bodies[0].shape.sub_shape(noshape_kernel::ShapeType::Face, bf).unwrap();
    assert!(face.centroid().unwrap().z.abs() < 1e-9);
}

#[test]
fn tangent_cones_tori_and_propagation() {
    // Pin with a 2 mm chamfer (cone) on its top edge, pin with a 2 mm fillet (torus).
    let chamfered = with_feature(pin_studio(), "chamfer", |f, r| {
        f.params.set("entities", ParamValue::Query(edges_with(r, "CapEnd")));
        f.params.set("width", ParamValue::Expr(ExprValue::new("2 mm")));
    });
    let filleted = with_feature(pin_studio(), "fillet", |f, r| {
        f.params.set("entities", ParamValue::Query(edges_with(r, "CapEnd")));
        f.params.set("radius", ParamValue::Expr(ExprValue::new("2 mm")));
    });
    let rounded_block = with_feature(block_studio(), "fillet", |f, r| {
        f.params.set("entities", ParamValue::Query(edges_with(r, "CapEnd")));
        f.params.set("radius", ParamValue::Expr(ExprValue::new("2 mm")));
    });
    let f = doc_with(vec![block_studio(), chamfered, filleted, rounded_block]);
    let mut ev = AssemblyEvaluator::new();
    let (block, rb) = body_of(&mut ev, &f.doc, f.studios[0]);
    let (cpin, rc) = body_of(&mut ev, &f.doc, f.studios[1]);
    let (tpin, rt) = body_of(&mut ev, &f.doc, f.studios[2]);
    let (rblock, rr) = body_of(&mut ev, &f.doc, f.studios[3]);
    let top = face_with(&rb, "CapEnd");
    let cone = face_of_kind(&rc, |s| matches!(s, noshape_kernel::Surface::Cone { .. }));
    let torus = face_of_kind(&rt, |s| matches!(s, noshape_kernel::Surface::Torus { .. }));
    let ent = |i: u64, r: Reference| MateConnectorRef::on(InstanceId(i), vec![], r);
    let tilted = |at: DVec3, ang: f64| Placement { translation: at.into(), rotation: { let q = DQuat::from_rotation_x(ang); [q.x, q.y, q.z, q.w] } };

    // Cone lying on the block's top face: apex on the plane, 2 DOF removed.
    let mut p2 = part_instance(2, "Pin <1>", f.studios[1], cpin, DVec3::ZERO, false);
    p2.placement = tilted(DVec3::new(0.02, 0.01, 0.05), 2.0);
    let asm = Assembly { instances: vec![part_instance(1, "Block <1>", f.studios[0], block.clone(), DVec3::ZERO, true), p2], mates: vec![mate(10, MateType::Tangent, ent(1, top.clone()), ent(2, cone.clone()))], ..Default::default() };
    let res = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
    assert!(res.mates[0].ok, "cone: {:?}", res.mates[0].message);
    assert_eq!(res.nodes[1].dof, Some(4));

    // Torus (upside-down filleted pin) resting on the top face: the rounded tip touches it.
    let mut p3 = part_instance(2, "Pin <2>", f.studios[2], tpin, DVec3::ZERO, false);
    p3.placement = tilted(DVec3::new(0.02, 0.01, 0.06), std::f64::consts::PI);
    let asm = Assembly { instances: vec![part_instance(1, "Block <1>", f.studios[0], block.clone(), DVec3::ZERO, true), p3], mates: vec![mate(10, MateType::Tangent, ent(1, top.clone()), ent(2, torus.clone()))], ..Default::default() };
    let res = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
    assert!(res.mates[0].ok, "torus: {:?}", res.mates[0].message);
    let units = UnitPrefs::default();
    let tg = eval::tangent_geometry(&rt.context, None, &torus, &units).unwrap().transformed(&eval::matrix_frame(&res.occurrences[1].transform));
    let crate::assembly::solver::TangentGeom::Torus { center, axis, major, minor } = tg else { panic!("{tg:?}") };
    let lowest = center.z - (major * (1.0 - axis.z * axis.z).sqrt() + minor);
    assert!((lowest - 0.01).abs() < 1e-6, "the torus touches the face: {lowest}");
    assert!(axis.z.abs() > 0.99, "still nearly upright: {axis:?}");
    assert_eq!(res.nodes[1].dof, Some(5));

    // A vertex (point) on the cone surface.
    let corner = {
        let b = &rb.context.bodies[0];
        let i = (0..b.vertices.len()).find(|&i| b.shape.sub_shape(noshape_kernel::ShapeType::Vertex, i).unwrap().vertex_point().unwrap().length() < 1e-9).unwrap();
        Reference::Vertex((*b.vertices[i]).clone())
    };
    let asm = Assembly {
        instances: vec![part_instance(1, "Pin <1>", f.studios[1], rc.context.bodies[0].id.clone(), DVec3::ZERO, true), part_instance(2, "Block <1>", f.studios[0], block.clone(), DVec3::new(0.01, 0.0, 0.035), false)],
        mates: vec![mate(10, MateType::Tangent, ent(1, cone.clone()), ent(2, corner.clone()))],
        ..Default::default()
    };
    let res = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
    assert!(res.mates[0].ok, "vertex on cone: {:?}", res.mates[0].message);
    let g = eval::tangent_geometry(&rc.context, None, &cone, &units).unwrap();
    let v = DVec3::from(res.solved.placement(InstanceId(2)).unwrap().translation);
    assert!(g.signed_distance(v).abs() < 1e-6, "vertex lies on the cone: {}", g.signed_distance(v));

    // Unsupported pairs are reported.
    let asm2 = Assembly { mates: vec![mate(10, MateType::Tangent, ent(1, cone.clone()), ent(2, face_with(&rb, "CapStart")))], ..asm.clone() };
    let mut asm2 = asm2;
    asm2.instances[1] = part_instance(2, "Pin <2>", f.studios[2], rt.context.bodies[0].id.clone(), DVec3::new(0.05, 0.0, 0.0), false);
    asm2.mates[0].b = ent(2, torus.clone());
    let res = ev.evaluate(&f.doc, &asm2, &SolveOptions::default(), None);
    assert!(!res.mates[0].ok && res.mates[0].message.as_deref().unwrap_or("").contains("not supported"));

    // Tangent propagation: a block corner held on the rounded block's top face
    // follows over the fillet onto the side face when dragged past the edge.
    let rtop = {
        let b = &rr.context.bodies[0];
        let i = b.faces.iter().position(|n| matches!(&**n, crate::reference::TopoName::Face { role: crate::reference::Role::CapEnd(_), .. })).unwrap();
        Reference::Face((*b.faces[i]).clone())
    };
    let chain = eval::tangent_chain(&rr.context, None, &rtop, &units);
    assert_eq!(chain.len(), 9, "top + 4 fillets + 4 sides");
    let mk = |prop: bool| {
        let mut m = mate(10, MateType::Tangent, ent(1, rtop.clone()), ent(2, corner.clone()));
        m.tangent_propagation = prop;
        Assembly { instances: vec![part_instance(1, "Rounded <1>", f.studios[3], rblock.clone(), DVec3::ZERO, true), part_instance(2, "Block <1>", f.studios[0], block.clone(), DVec3::new(0.02, 0.01, 0.01), false)], mates: vec![m], ..Default::default() }
    };
    for prop in [true, false] {
        let asm = mk(prop);
        let res = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
        assert!(res.mates[0].ok, "{:?}", res.mates[0].message);
        // Pull the corner outward and down in steps (like a drag).
        let mut start: std::collections::HashMap<_, _> = res.solved.placements.iter().cloned().collect();
        let mut v = DVec3::ZERO;
        for k in 1..=12 {
            let t = k as f64 / 12.0;
            let target = DVec3::new(0.02 + 0.04 * t, 0.01, 0.01 - 0.006 * t);
            let s = res.prepared.solve(&SolveOptions { drags: vec![(InstanceId(2), DVec3::ZERO, target)], start: Some(start.clone()), ..Default::default() });
            start = s.placements.iter().cloned().collect();
            v = DVec3::from(s.placement(InstanceId(2)).unwrap().translation);
        }
        if prop {
            assert!((v.x - 0.04).abs() < 1e-5 && v.z < 0.0081, "on the side face: {v:?}");
        } else {
            assert!((v.z - 0.01).abs() < 1e-6 && v.x > 0.045, "stays on the (infinite) top plane: {v:?}");
        }
    }
}

struct TestExternal {
    docs: Vec<(String, std::sync::Arc<Document>, TabId)>,
}

impl eval::ExternalStudios for TestExternal {
    fn studio(&self, key: &str) -> Option<(std::sync::Arc<Document>, TabId)> {
        self.docs.iter().find(|(k, _, _)| k == key).map(|(_, d, t)| (d.clone(), *t))
    }
}

#[test]
fn instances_from_other_documents() {
    // "Other document": two versions of a Part Studio, the block 10 mm then 25 mm thick.
    let v1 = doc_with(vec![block_studio()]);
    let mut v2 = v1.doc.clone();
    if let TabContent::PartStudio(ps) = &mut v2.tabs[0].content {
        ps.features[1].params.set("depth", ParamValue::Expr(ExprValue::new("25 mm")));
    }
    let v2 = Fixture { doc: v2, studios: v1.studios.clone() };
    let ext = TestExternal { docs: vec![("ext:v1".into(), std::sync::Arc::new(v1.doc.clone()), v1.studios[0]), ("ext:v2".into(), std::sync::Arc::new(v2.doc.clone()), v2.studios[0])] };
    let f = doc_with(vec![block_studio()]);
    let mut ev = AssemblyEvaluator::new();
    ev.set_external(std::sync::Arc::new(ext));
    let (body, r) = body_of(&mut ev, &f.doc, f.studios[0]);
    let (_, er) = ev.external_studio("ext:v1", &ConfigValues::default()).expect("resolved");
    let ebody = er.context.bodies[0].id.clone();
    let top = face_with(&r, "CapEnd");
    let bottom = face_with(&er, "CapStart");
    let mut asm = Assembly {
        instances: vec![
            part_instance(1, "Block <1>", f.studios[0], body, DVec3::ZERO, true),
            Instance { id: InstanceId(2), name: "Part 1 <1>".into(), source: InstanceSource::External { key: "ext:v1".into(), body: Some(ebody), config: ConfigValues::default() }, placement: Placement { translation: [0.1, 0.0, 0.0], rotation: [0.0, 0.0, 0.0, 1.0] }, fixed: false, hidden: false, suppressed: false, lock: SubassemblyLock::Flexible, nested: Vec::new(), best_tessellation: false },
        ],
        mates: vec![mate(10, MateType::Fastened, conn(1, top, Inference::Centroid), conn(2, bottom, Inference::Centroid))],
        ..Default::default()
    };
    let res = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
    assert!(res.nodes[1].error.is_none(), "{:?}", res.nodes[1].error);
    assert!(res.mates[0].ok, "{:?}", res.mates[0].message);
    assert_eq!(res.occurrences[1].external.as_deref(), Some("ext:v1"));
    assert_eq!(bom(&res).len(), 2, "external part is its own BOM row");
    // Update to the newer version: same feature ids, so the mate keeps resolving.
    assert_eq!(asm.update_external("ext:v1", "ext:v2"), 1);
    let res = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
    assert!(res.nodes[1].error.is_none(), "{:?}", res.nodes[1].error);
    assert!(res.mates[0].ok, "{:?}", res.mates[0].message);
    let (lo, hi) = res.occurrences[1].world_bounds().unwrap();
    assert!((hi.z - lo.z - 0.025).abs() < 1e-6, "the new version's geometry: {lo:?} {hi:?}");
    // A missing document is reported on the instance.
    asm.update_external("ext:v2", "ext:gone");
    let res = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
    assert!(res.nodes[1].error.is_some());
    // Serialized form round-trips.
    let json = serde_json::to_string(&asm.instances[1].source).unwrap();
    assert_eq!(serde_json::from_str::<InstanceSource>(&json).unwrap(), asm.instances[1].source);
}

// ---------------------------------------------------------------- Onshape behaviours

#[test]
fn list_filter_colon_commands() {
    use super::ops::{FilterCommand, ListFilter, RowInfo, RowKind};
    let row = |kind, name, hidden, suppressed, error, has_limits| RowInfo { kind, name, error, warning: false, suppressed, hidden, has_limits };
    let f = ListFilter::parse(":part :hidden Bolt");
    assert_eq!(f.commands, vec![FilterCommand::Part, FilterCommand::Hidden]);
    assert_eq!(f.text, "bolt");
    assert!(f.matches(&row(RowKind::Part, "Hex bolt <2>", true, false, false, false)));
    assert!(!f.matches(&row(RowKind::Part, "Hex bolt <2>", false, false, false, false)), "shown parts do not pass :hidden");
    assert!(!f.matches(&row(RowKind::Assembly, "Bolt kit <1>", true, false, false, false)), ":part excludes subassemblies");
    assert!(!f.wants_mate_features());
    // :mate :errors (combinable), :matelimits, relations count as mates.
    let f = ListFilter::parse(":mate :errors");
    assert!(f.matches(&row(RowKind::Mate, "Revolute 1", false, false, true, false)));
    assert!(!f.matches(&row(RowKind::Mate, "Revolute 2", false, false, false, false)));
    assert!(f.matches(&row(RowKind::Relation, "Gear 1", false, false, true, false)));
    assert!(!f.wants_instances());
    let f = ListFilter::parse(":matelimits");
    assert!(f.matches(&row(RowKind::Mate, "Slider 1", false, false, false, true)));
    assert!(!f.matches(&row(RowKind::Mate, "Slider 2", false, false, false, false)));
    // Plain text matches names case-insensitively; unknown colon words are text.
    let f = ListFilter::parse("PIN");
    assert!(f.matches(&row(RowKind::Part, "Pin <1>", false, false, false, false)));
    assert!(ListFilter::parse(":shown").matches(&row(RowKind::Part, "A", false, false, false, false)));
    assert!(ListFilter::parse(":suppressed").matches(&row(RowKind::Mate, "M", false, true, false, false)));
    assert_eq!(ListFilter::parse(":nonsense").text, ":nonsense");
}

#[test]
fn copy_paste_duplicates_instances_with_their_mates() {
    use super::ops::InstanceClipboard;
    let f = doc_with(vec![block_studio()]);
    let mut ev = AssemblyEvaluator::new();
    let (body, r) = body_of(&mut ev, &f.doc, f.studios[0]);
    let top = face_with(&r, "CapEnd");
    let bottom = face_with(&r, "CapStart");
    let mut asm = Assembly {
        instances: vec![
            part_instance(1, "Block <1>", f.studios[0], body.clone(), DVec3::ZERO, true),
            part_instance(2, "Block <2>", f.studios[0], body.clone(), DVec3::new(0.0, 0.0, 0.01), false),
            part_instance(3, "Block <3>", f.studios[0], body, DVec3::new(0.0, 0.0, 0.02), true),
        ],
        mates: vec![
            mate(10, MateType::Fastened, conn(1, top.clone(), Inference::Centroid), conn(2, bottom.clone(), Inference::Centroid)),
            mate(11, MateType::Fastened, conn(2, top, Inference::Centroid), conn(3, bottom, Inference::Centroid)),
        ],
        ..Default::default()
    };
    let clip = InstanceClipboard::copy(&asm, &[InstanceId(2), InstanceId(3)]);
    assert_eq!(clip.instances.len(), 2);
    assert_eq!(clip.mates.len(), 1, "only the mate between copied instances comes along");
    let ids = clip.paste(&mut asm, [0.1, 0.0, 0.0]);
    assert_eq!(ids.len(), 2);
    assert_eq!(asm.instances.len(), 5);
    let a = asm.instance(ids[0]).unwrap();
    let b = asm.instance(ids[1]).unwrap();
    assert_eq!(a.name, "Block <4>");
    assert_eq!(b.name, "Block <5>");
    assert!(!b.fixed, "pasted instances are free");
    assert!((a.placement.translation[0] - 0.1).abs() < 1e-12);
    let m = asm.mates.last().unwrap();
    assert_eq!(m.name, "Fastened 1");
    assert_eq!((m.a.instance, m.b.instance), (ids[0], ids[1]), "mate remapped to the copies");
    // The pasted pair solves like the original.
    let res = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
    assert!(res.mates.iter().all(|m| m.ok));
}

#[test]
fn rename_hide_others_and_unfix_all() {
    let f = doc_with(vec![block_studio()]);
    let mut ev = AssemblyEvaluator::new();
    let (body, r) = body_of(&mut ev, &f.doc, f.studios[0]);
    let top = face_with(&r, "CapEnd");
    let bottom = face_with(&r, "CapStart");
    let mut asm = Assembly {
        instances: vec![
            part_instance(1, "Block <1>", f.studios[0], body.clone(), DVec3::ZERO, true),
            part_instance(2, "Block <2>", f.studios[0], body.clone(), DVec3::new(0.0, 0.0, 0.01), true),
            part_instance(3, "Block <3>", f.studios[0], body, DVec3::new(0.0, 0.0, 0.02), false),
        ],
        mates: vec![mate(10, MateType::Fastened, conn(1, top, Inference::Centroid), conn(2, bottom, Inference::Centroid))],
        ..Default::default()
    };
    assert!(asm.rename(Some(InstanceId(2)), None, "  Base  "));
    assert_eq!(asm.instance(InstanceId(2)).unwrap().name, "Base");
    assert!(!asm.rename(Some(InstanceId(2)), None, "   "), "empty names are ignored");
    assert!(asm.rename(None, Some(MateId(10)), "Stack"));
    assert_eq!(asm.mate(MateId(10)).unwrap().name, "Stack");
    asm.hide_other_instances(&[InstanceId(3)]);
    assert_eq!(asm.instances.iter().map(|i| i.hidden).collect::<Vec<_>>(), vec![true, true, false]);
    asm.set_all_hidden(false);
    assert!(asm.instances.iter().all(|i| !i.hidden));
    assert_eq!(asm.fixed_instances().len(), 2);
    assert_eq!(asm.unfix_all(), 2);
    assert!(asm.instances.iter().all(|i| !i.fixed));
    asm.set_mates_hidden(&[MateId(10)], true);
    assert!(asm.mate(MateId(10)).unwrap().hidden);
    // Deleting a group member keeps the group until its last member goes.
    asm.groups.push(Group { id: MateId(20), name: "Group 1".into(), instances: vec![InstanceId(2), InstanceId(3)], suppressed: false });
    asm.remove_instance(InstanceId(3));
    assert_eq!(asm.groups.len(), 1);
    assert_eq!(asm.groups[0].instances, vec![InstanceId(2)]);
    asm.remove_instance(InstanceId(2));
    assert!(asm.groups.is_empty());
}

#[test]
fn inference_points_match_onshape() {
    use super::eval::inference_candidates;
    let f = doc_with(vec![block_studio(), plate_with_hole()]);
    let mut ev = AssemblyEvaluator::new();
    let units = UnitPrefs::default();
    let (bb, rb) = body_of(&mut ev, &f.doc, f.studios[0]);
    let (pb, rp) = body_of(&mut ev, &f.doc, f.studios[1]);
    // Planar face of a block: centroid, 4 corners and 4 edge midpoints, all normal to the face.
    let top = face_with(&rb, "CapEnd");
    let pts = inference_candidates(&rb.context, Some(&bb), &top, &units);
    assert_eq!(pts.len(), 9, "{:#?}", pts.iter().map(|p| p.frame.origin).collect::<Vec<_>>());
    assert!(pts.iter().all(|p| (p.frame.z() - DVec3::Z).length() < 1e-9), "Z normal to the top face");
    assert!(pts.iter().any(|p| p.frame.origin.distance(DVec3::new(0.02, 0.01, 0.01)) < 1e-9), "centroid");
    assert!(pts.iter().any(|p| p.frame.origin.distance(DVec3::new(0.04, 0.02, 0.01)) < 1e-9), "corner");
    assert!(pts.iter().any(|p| p.frame.origin.distance(DVec3::new(0.02, 0.0, 0.01)) < 1e-9), "edge midpoint");
    // The bottom face points down.
    let bottom = face_with(&rb, "CapStart");
    let pts = inference_candidates(&rb.context, Some(&bb), &bottom, &units);
    assert!(pts.iter().all(|p| (p.frame.z() + DVec3::Z).length() < 1e-9));
    // Plate with a centered hole: the hole center (here also the centroid) is
    // offered once; the circle's seam vertex is not a point.
    let ptop = face_with(&rp, "CapEnd");
    let pts = inference_candidates(&rp.context, Some(&pb), &ptop, &units);
    assert_eq!(pts.len(), 9, "{:#?}", pts.iter().map(|p| p.frame.origin).collect::<Vec<_>>());
    assert!(pts.iter().filter(|p| p.frame.origin.truncate().length() < 1e-9).count() == 1, "hole center coincides with the centroid");
    // Cylindrical face: axis points at both ends and the middle, Z along the axis.
    let hole = cylinder_face(&rp);
    let pts = inference_candidates(&rp.context, Some(&pb), &hole, &units);
    let mut zs: Vec<f64> = pts.iter().map(|p| p.frame.origin.z).collect();
    zs.sort_by(f64::total_cmp);
    assert_eq!(zs.len(), 3);
    assert!((zs[0]).abs() < 1e-9 && (zs[1] - 0.005).abs() < 1e-9 && (zs[2] - 0.01).abs() < 1e-9, "{zs:?}");
    assert!(pts.iter().all(|p| p.frame.origin.truncate().length() < 1e-9 && p.frame.z().cross(DVec3::Z).length() < 1e-9));
    // Circular edge: its center only.
    let b = &rp.context.bodies[0];
    let circle = b
        .edges
        .iter()
        .enumerate()
        .find(|(i, _)| b.shape.sub_shape(noshape_kernel::ShapeType::Edge, *i).ok().and_then(|e| e.edge_info().ok()).is_some_and(|inf| matches!(inf.curve, noshape_kernel::Curve::Circle { .. })))
        .map(|(_, n)| Reference::Edge((**n).clone()))
        .unwrap();
    assert_eq!(inference_candidates(&rp.context, Some(&pb), &circle, &units).len(), 1);
    // Linear edge: midpoint and both ends, Z along the edge.
    let line = b
        .edges
        .iter()
        .enumerate()
        .find(|(i, _)| b.shape.sub_shape(noshape_kernel::ShapeType::Edge, *i).ok().and_then(|e| e.edge_info().ok()).is_some_and(|inf| matches!(inf.curve, noshape_kernel::Curve::Line { .. })))
        .map(|(_, n)| Reference::Edge((**n).clone()))
        .unwrap();
    let pts = inference_candidates(&rp.context, Some(&pb), &line, &units);
    assert_eq!(pts.len(), 3);
    let dir = (pts[1].frame.origin - pts[2].frame.origin).normalize();
    assert!(pts.iter().all(|p| p.frame.z().cross(dir).length() < 1e-9), "Z along the edge");
    // A connector picked from these points resolves to the same frame in the assembly.
    let asm = Assembly { instances: vec![part_instance(1, "Plate <1>", f.studios[1], pb, DVec3::new(0.1, 0.0, 0.0), true)], ..Default::default() };
    let res = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
    let p = &inference_candidates(&rp.context, Some(&rp.context.bodies[0].id), &ptop, &units)[3];
    let w = res.connector_world(&p.connector(&[InstanceId(1)]), &units).unwrap();
    assert!(w.origin.distance(p.frame.origin + DVec3::new(0.1, 0.0, 0.0)) < 1e-9 && (w.z() - p.frame.z()).length() < 1e-9);
}

#[test]
fn mate_reset_and_apply_limit_position() {
    let f = doc_with(vec![block_studio()]);
    let mut ev = AssemblyEvaluator::new();
    let (body, r) = body_of(&mut ev, &f.doc, f.studios[0]);
    let top = face_with(&r, "CapEnd");
    let bottom = face_with(&r, "CapStart");
    let mut m = mate(10, MateType::Slider, conn(1, top, Inference::Centroid), conn(2, bottom, Inference::Centroid));
    m.limits = vec![("z".into(), Limits { min: Some(ExprValue::new("0 mm")), max: Some(ExprValue::new("20 mm")) })];
    let asm = Assembly {
        instances: vec![part_instance(1, "Block <1>", f.studios[0], body.clone(), DVec3::ZERO, true), part_instance(2, "Block <2>", f.studios[0], body, DVec3::new(0.0, 0.0, 0.025), false)],
        mates: vec![m],
        ..Default::default()
    };
    let res = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
    // Apply limit position > Max Z: the slider value goes to 20 mm.
    let mut v = res.solved.mate_values[&MateId(10)];
    v[3] = 0.02;
    let s = res.prepared.solve(&SolveOptions { locks: vec![(MateId(10), v)], ..Default::default() });
    assert!((s.placement(InstanceId(2)).unwrap().translation[2] - 0.03).abs() < 1e-6);
    // Reset: back to the home position (value 0, faces touching).
    let s = res.prepared.solve(&SolveOptions { locks: vec![(MateId(10), [0.0; 4])], ..Default::default() });
    assert!((s.placement(InstanceId(2)).unwrap().translation[2] - 0.01).abs() < 1e-6);
}

#[test]
fn mate_rotation_offset_about_any_axis() {
    let f = doc_with(vec![block_studio()]);
    let mut ev = AssemblyEvaluator::new();
    let (body, r) = body_of(&mut ev, &f.doc, f.studios[0]);
    let top = face_with(&r, "CapEnd");
    let bottom = face_with(&r, "CapStart");
    let mut m = mate(10, MateType::Fastened, conn(1, top, Inference::Centroid), conn(2, bottom, Inference::Centroid));
    m.rotation = Some(ExprValue::new("90 deg"));
    let mk = |m: Mate| Assembly {
        instances: vec![part_instance(1, "Block <1>", f.studios[0], body.clone(), DVec3::ZERO, true), part_instance(2, "Block <2>", f.studios[0], body.clone(), DVec3::new(0.0, 0.0, 0.03), false)],
        mates: vec![m],
        ..Default::default()
    };
    let rot = |res: &eval::AssemblyResult| {
        let [x, y, z, w] = res.solved.placement(InstanceId(2)).unwrap().rotation;
        DQuat::from_xyzw(x, y, z, w)
    };
    // About Z (default): the block turns in the face plane, its Z stays up.
    let res = ev.evaluate(&f.doc, &mk(m.clone()), &SolveOptions::default(), None);
    assert!(res.mates[0].ok);
    assert!((rot(&res) * DVec3::Z - DVec3::Z).length() < 1e-6);
    // About X: the block's Z axis tips into the face plane.
    m.rotation_axis = 0;
    let res = ev.evaluate(&f.doc, &mk(m), &SolveOptions::default(), None);
    assert!(res.mates[0].ok, "{:?}", res.mates[0].message);
    assert!((rot(&res) * DVec3::Z).z.abs() < 1e-6, "{:?}", rot(&res) * DVec3::Z);
}

#[test]
fn move_to_new_subassembly_and_dissolve_keep_positions() {
    let mut f = doc_with(vec![block_studio()]);
    let mut ev = AssemblyEvaluator::new();
    let (body, r) = body_of(&mut ev, &f.doc, f.studios[0]);
    let top = face_with(&r, "CapEnd");
    let bottom = face_with(&r, "CapStart");
    let asm = Assembly {
        instances: vec![
            part_instance(1, "Block <1>", f.studios[0], body.clone(), DVec3::ZERO, true),
            part_instance(2, "Block <2>", f.studios[0], body.clone(), DVec3::new(0.0, 0.0, 0.01), false),
            part_instance(3, "Block <3>", f.studios[0], body, DVec3::new(0.0, 0.0, 0.02), false),
        ],
        mates: vec![
            mate(10, MateType::Fastened, conn(1, top.clone(), Inference::Centroid), conn(2, bottom.clone(), Inference::Centroid)),
            mate(11, MateType::Revolute, conn(2, top, Inference::Centroid), conn(3, bottom, Inference::Centroid)),
        ],
        ..Default::default()
    };
    let world = |res: &eval::AssemblyResult| -> Vec<DVec3> {
        let mut v: Vec<DVec3> = res.occurrences.iter().map(|o| o.transform.transform_point3(DVec3::new(0.01, 0.005, 0.0))).collect();
        v.sort_by(|a, b| a.z.total_cmp(&b.z));
        v
    };
    let before = world(&ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None));
    let mut parent = asm.clone();
    let sub_tab = f.doc.add_tab("Assembly 2", TabContent::Assembly(Assembly::default()));
    let (sub, sub_id) = parent.move_to_new_subassembly(&[InstanceId(2), InstanceId(3)], sub_tab, "Assembly 2");
    assert_eq!(parent.instances.len(), 2);
    assert_eq!(parent.instance(sub_id).unwrap().name, "Assembly 2 <1>");
    assert_eq!(sub.instances.len(), 2);
    assert_eq!(sub.mates.len(), 1, "the revolute between the moved blocks moves along");
    let m = parent.mate(MateId(10)).unwrap();
    assert_eq!((m.b.instance, m.b.path.clone()), (sub_id, vec![InstanceId(2)]), "parent mate reaches into the subassembly");
    if let Some(t) = f.doc.tabs.iter_mut().find(|t| t.id == sub_tab) {
        t.content = TabContent::Assembly(sub.clone());
    }
    let res = ev.evaluate(&f.doc, &parent, &SolveOptions::default(), None);
    assert!(res.mates.iter().all(|m| m.ok), "{:?}", res.mates.iter().map(|m| &m.message).collect::<Vec<_>>());
    let after = world(&res);
    for (a, b) in before.iter().zip(&after) {
        assert!(a.distance(*b) < 1e-6, "moved: {a:?} -> {b:?}");
    }
    // Dissolve: back to three top-level instances at the same places.
    let ids = parent.dissolve_subassembly(sub_id, &sub);
    assert_eq!(ids.len(), 2);
    assert_eq!(parent.instances.len(), 3);
    assert!(parent.instance(sub_id).is_none());
    assert_eq!(parent.mates.len(), 2);
    let m = parent.mate(MateId(10)).unwrap();
    assert!(m.b.path.is_empty() && ids.contains(&m.b.instance));
    let res = ev.evaluate(&f.doc, &parent, &SolveOptions::default(), None);
    assert!(res.mates.iter().all(|m| m.ok));
    for (a, b) in before.iter().zip(&world(&res)) {
        assert!(a.distance(*b) < 1e-6, "dissolve moved: {a:?} -> {b:?}");
    }
    assert_eq!(super::ops::next_assembly_tab_name(&f.doc), "Assembly 1");
}

#[test]
fn hold_one_degree_of_freedom() {
    let f = doc_with(vec![plate_with_hole(), pin_studio()]);
    let mut ev = AssemblyEvaluator::new();
    let (plate, rp) = body_of(&mut ev, &f.doc, f.studios[0]);
    let (pin, rq) = body_of(&mut ev, &f.doc, f.studios[1]);
    let asm = Assembly {
        instances: vec![part_instance(1, "Plate <1>", f.studios[0], plate, DVec3::ZERO, true), part_instance(2, "Pin <1>", f.studios[1], pin, DVec3::new(0.0, 0.0, -0.01), false)],
        mates: vec![mate(10, MateType::Cylindrical, conn(1, cylinder_face(&rp), Inference::MidAxis), conn(2, cylinder_face(&rq), Inference::MidAxis))],
        ..Default::default()
    };
    let res = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
    let z0 = res.solved.placement(InstanceId(2)).unwrap().translation[2];
    // Animate the rotation only: the translation along the axis stays free (unchanged).
    let s = res.prepared.solve(&SolveOptions { hold_values: vec![(MateId(10), 0, 1.0)], ..Default::default() });
    assert!((s.mate_values[&MateId(10)][0] - 1.0).abs() < 1e-6);
    assert!((s.placement(InstanceId(2)).unwrap().translation[2] - z0).abs() < 1e-6);
    // Animate the translation only.
    let s = res.prepared.solve(&SolveOptions { hold_values: vec![(MateId(10), 3, 0.004)], ..Default::default() });
    assert!((s.mate_values[&MateId(10)][3] - 0.004).abs() < 1e-6, "{:?}", s.mate_values[&MateId(10)]);
}

#[test]
fn folders_and_display_states() {
    let f = doc_with(vec![block_studio()]);
    let mut ev = AssemblyEvaluator::new();
    let (body, r) = body_of(&mut ev, &f.doc, f.studios[0]);
    let top = face_with(&r, "CapEnd");
    let bottom = face_with(&r, "CapStart");
    let mut asm = Assembly {
        instances: vec![
            part_instance(1, "Block <1>", f.studios[0], body.clone(), DVec3::ZERO, true),
            part_instance(2, "Block <2>", f.studios[0], body.clone(), DVec3::new(0.0, 0.0, 0.01), false),
            part_instance(3, "Block <3>", f.studios[0], body, DVec3::new(0.0, 0.0, 0.02), false),
        ],
        mates: vec![mate(10, MateType::Fastened, conn(1, top, Inference::Centroid), conn(2, bottom, Inference::Centroid))],
        ..Default::default()
    };
    // Folder around instances 3 and 2 (members keep the list order).
    let fid = asm.add_folder(&[InstanceId(3), InstanceId(2)], &[]);
    assert_eq!(asm.folders[0].name, "Folder 1");
    assert_eq!(asm.folders[0].instances, vec![InstanceId(2), InstanceId(3)]);
    assert_eq!(asm.folder_of_instance(InstanceId(3)).map(|f| f.id), Some(fid));
    assert!(asm.next_id() > fid.0, "folder ids are taken");
    // Moving an instance into another folder takes it out of the first.
    let f2 = asm.add_folder(&[InstanceId(3)], &[]);
    assert_eq!(asm.folders[0].instances, vec![InstanceId(2)]);
    assert!(asm.rename_folder(f2, "Fasteners"));
    // Mate folder; deleting the mate empties it.
    let mf = asm.add_folder(&[], &[MateId(10)]);
    assert!(asm.folders.iter().find(|f| f.id == mf).unwrap().is_mate_folder());
    // Display state: capture, change, apply.
    asm.instances[1].hidden = true;
    assert!(asm.save_display_state("Without block 2"));
    asm.set_all_hidden(false);
    assert!(asm.apply_display_state(0));
    assert!(asm.instances[1].hidden && !asm.instances[0].hidden);
    // Drag and drop: reorder, joining the target's folder.
    asm.move_instance(InstanceId(1), Some(InstanceId(3)));
    assert_eq!(asm.instances.iter().map(|i| i.id.0).collect::<Vec<_>>(), vec![2, 1, 3]);
    assert_eq!(asm.folder_of_instance(InstanceId(1)).map(|f| f.id), Some(f2));
    asm.move_instance(InstanceId(1), None);
    assert_eq!(asm.instances.last().unwrap().id, InstanceId(1));
    assert!(asm.folder_of_instance(InstanceId(1)).is_none(), "dropped among unfoldered instances");
    asm.move_into_folder(InstanceId(1), fid);
    assert_eq!(asm.folder_of_instance(InstanceId(1)).map(|f| f.id), Some(fid));
    asm.move_into_folder(InstanceId(1), f2);
    // Unpack keeps contents, delete removes them.
    asm.unpack_folder(fid);
    assert_eq!(asm.instances.len(), 3);
    asm.delete_folder(f2);
    assert_eq!(asm.instances.len(), 1);
    assert!(asm.instance(InstanceId(3)).is_none());
    asm.remove_mate(MateId(10));
    assert!(asm.folders.iter().find(|f| f.id == mf).unwrap().features.is_empty());
}

#[test]
fn flexible_subassembly_moves_in_parent_and_locks() {
    let mut f = doc_with(vec![block_studio()]);
    let mut ev = AssemblyEvaluator::new();
    let (body, r) = body_of(&mut ev, &f.doc, f.studios[0]);
    let top = face_with(&r, "CapEnd");
    let bottom = face_with(&r, "CapStart");
    // Subassembly: block 2 slides up and down on block 1 (fixed in the subassembly).
    let sub = Assembly {
        instances: vec![part_instance(1, "Block <1>", f.studios[0], body.clone(), DVec3::ZERO, true), part_instance(2, "Block <2>", f.studios[0], body.clone(), DVec3::new(0.0, 0.0, 0.015), false)],
        mates: vec![mate(10, MateType::Slider, conn(1, top.clone(), Inference::Centroid), conn(2, bottom.clone(), Inference::Centroid))],
        named_positions: vec![NamedPosition { name: "Up".into(), placements: vec![], mate_values: vec![(MateId(10), [0.0, 0.0, 0.0, 0.03])] }],
        ..Default::default()
    };
    let sub_tab = f.doc.add_tab("Sub", TabContent::Assembly(sub));
    let mut sub_inst = Instance::new(InstanceId(5), "Sub <1>".into(), InstanceSource::Assembly { assembly: sub_tab }, Placement { translation: [0.0, 0.0, 0.01], rotation: [0.0, 0.0, 0.0, 1.0] });
    sub_inst.lock = SubassemblyLock::Flexible;
    let mut asm = Assembly {
        instances: vec![part_instance(1, "Base <1>", f.studios[0], body.clone(), DVec3::ZERO, true), sub_inst],
        mates: vec![mate(20, MateType::Fastened, conn(1, top, Inference::Centroid), MateConnectorRef::on(InstanceId(5), vec![InstanceId(1)], Reference::Implicit { on: Box::new(bottom), inference: Inference::Centroid }))],
        ..Default::default()
    };
    let res = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
    assert!(res.mates[0].ok, "{:?}", res.mates[0].message);
    assert!(res.prepared.flexible.contains(&InstanceId(5)));
    assert_eq!(res.solved.instance_dof.get(&InstanceId(5)).copied(), Some(1), "the slider inside stays free in the parent");
    let z_of = |res: &eval::AssemblyResult, s: &eval::Solved| {
        let _ = res;
        s.nested_world(InstanceId(5), InstanceId(2)).unwrap().origin.z
    };
    // Starts where the subassembly has it (5 mm above block 1's top, block 1 on the base).
    assert!((z_of(&res, &res.solved) - 0.025).abs() < 1e-6, "{}", z_of(&res, &res.solved));
    // Drag block 2 of the subassembly upward: it slides, nothing else moves.
    let s = res.prepared.solve(&SolveOptions { nested_drags: vec![((InstanceId(5), InstanceId(2)), DVec3::new(0.02, 0.01, 0.0), DVec3::new(0.0, 0.0, 0.05))], ..Default::default() });
    assert!((z_of(&res, &s) - 0.05).abs() < 1e-3, "{}", z_of(&res, &s));
    let w = s.nested_world(InstanceId(5), InstanceId(2)).unwrap().origin;
    assert!(w.x.abs() < 1e-6 && w.y.abs() < 1e-6, "slides along Z only: {w:?}");
    assert!((DVec3::from(s.placement(InstanceId(5)).unwrap().translation) - DVec3::new(0.0, 0.0, 0.01)).length() < 1e-6, "subassembly frame stays on the base");
    // The display follows the nested placement.
    let mut r2 = res.clone();
    r2.apply_placements(s.clone(), None);
    let upper = r2.occurrences.iter().find(|o| o.path == vec![InstanceId(5), InstanceId(2)]).unwrap();
    assert!((upper.transform.transform_point3(DVec3::ZERO).z - 0.05).abs() < 1e-3);
    // Stored nested positions are used next time.
    asm.instances[1].nested = s.nested.iter().filter(|((sid, _), _)| *sid == InstanceId(5)).map(|((_, c), p)| (*c, *p)).collect();
    let res = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
    assert!((z_of(&res, &res.solved) - 0.05).abs() < 1e-3);
    // Locked at the current position: one rigid body, no DOF.
    asm.instances[1].lock = SubassemblyLock::Current;
    let res = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
    assert!(res.prepared.flexible.is_empty());
    assert_eq!(res.solved.instance_dof.get(&InstanceId(5)).copied(), Some(0));
    let upper = res.occurrences.iter().find(|o| o.path == vec![InstanceId(5), InstanceId(2)]).unwrap();
    assert!((upper.transform.transform_point3(DVec3::ZERO).z - 0.05).abs() < 1e-3, "locked where it was");
    // Following the named position "Up": block 2 is 30 mm above block 1's top.
    asm.instances[1].lock = SubassemblyLock::Named("Up".into());
    let res = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
    let upper = res.occurrences.iter().find(|o| o.path == vec![InstanceId(5), InstanceId(2)]).unwrap();
    assert!((upper.transform.transform_point3(DVec3::ZERO).z - 0.05).abs() < 1e-3, "{}", upper.transform.transform_point3(DVec3::ZERO).z);
}

#[test]
fn ball_mate_cone_limit() {
    let f = doc_with(vec![block_studio()]);
    let mut ev = AssemblyEvaluator::new();
    let (body, r) = body_of(&mut ev, &f.doc, f.studios[0]);
    let top = face_with(&r, "CapEnd");
    let bottom = face_with(&r, "CapStart");
    let mut m = mate(10, MateType::Ball, conn(1, top, Inference::Centroid), conn(2, bottom, Inference::Centroid));
    m.limits = vec![("cone".into(), Limits { min: None, max: Some(ExprValue::new("20 deg")) })];
    let asm = Assembly {
        instances: vec![part_instance(1, "Block <1>", f.studios[0], body.clone(), DVec3::ZERO, true), part_instance(2, "Block <2>", f.studios[0], body, DVec3::new(0.0, 0.0, 0.01), false)],
        mates: vec![m],
        ..Default::default()
    };
    let res = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
    assert!(res.mates[0].ok, "{:?}", res.mates[0].message);
    // Drag a far corner sideways: the block tips but no more than 20 degrees.
    let s = res.prepared.solve(&SolveOptions { drags: vec![(InstanceId(2), DVec3::new(0.0, 0.0, 0.01), DVec3::new(0.2, 0.0, 0.0))], ..Default::default() });
    let [x, y, z, w] = s.placement(InstanceId(2)).unwrap().rotation;
    let tilt = (DQuat::from_xyzw(x, y, z, w) * DVec3::Z).z.clamp(-1.0, 1.0).acos().to_degrees();
    assert!(tilt > 5.0 && tilt < 20.5, "tilt {tilt}");
}

#[test]
fn drop_instances_into_a_subassembly() {
    let mut f = doc_with(vec![block_studio()]);
    let mut ev = AssemblyEvaluator::new();
    let (body, r) = body_of(&mut ev, &f.doc, f.studios[0]);
    let top = face_with(&r, "CapEnd");
    let bottom = face_with(&r, "CapStart");
    let sub_content = Assembly { instances: vec![part_instance(1, "Block <1>", f.studios[0], body.clone(), DVec3::ZERO, true)], ..Default::default() };
    let sub_tab = f.doc.add_tab("Sub", TabContent::Assembly(sub_content.clone()));
    let mut asm = Assembly {
        instances: vec![
            part_instance(1, "Base <1>", f.studios[0], body.clone(), DVec3::ZERO, true),
            Instance::new(InstanceId(5), "Sub <1>".into(), InstanceSource::Assembly { assembly: sub_tab }, Placement { translation: [0.1, 0.0, 0.0], rotation: [0.0, 0.0, 0.0, 1.0] }),
            part_instance(2, "Block <2>", f.studios[0], body.clone(), DVec3::new(0.0, 0.0, 0.01), false),
            part_instance(3, "Block <3>", f.studios[0], body, DVec3::new(0.0, 0.0, 0.02), false),
        ],
        mates: vec![
            mate(10, MateType::Fastened, conn(1, top.clone(), Inference::Centroid), conn(2, bottom.clone(), Inference::Centroid)),
            mate(11, MateType::Fastened, conn(2, top, Inference::Centroid), conn(3, bottom, Inference::Centroid)),
        ],
        ..Default::default()
    };
    let world = |res: &eval::AssemblyResult, path: Vec<InstanceId>| res.occurrences.iter().find(|o| o.path == path).unwrap().transform.transform_point3(DVec3::ZERO);
    let before = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
    let (w2, w3) = (world(&before, vec![InstanceId(2)]), world(&before, vec![InstanceId(3)]));
    let mut sub = sub_content;
    let new = asm.move_into_subassembly(&[InstanceId(2), InstanceId(3)], InstanceId(5), &mut sub);
    assert_eq!(new.len(), 2);
    assert_eq!(sub.instances.len(), 3);
    assert_eq!(sub.mates.len(), 1, "the mate between the dropped blocks moves along");
    assert_eq!(asm.instances.len(), 2);
    let m = asm.mate(MateId(10)).unwrap();
    assert_eq!((m.b.instance, m.b.path.clone()), (InstanceId(5), vec![new[0]]));
    if let Some(t) = f.doc.tabs.iter_mut().find(|t| t.id == sub_tab) {
        t.content = TabContent::Assembly(sub);
    }
    let res = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
    assert!(res.mates.iter().all(|m| m.ok), "{:?}", res.mates.iter().map(|m| &m.message).collect::<Vec<_>>());
    assert!(world(&res, vec![InstanceId(5), new[0]]).distance(w2) < 1e-6);
    assert!(world(&res, vec![InstanceId(5), new[1]]).distance(w3) < 1e-6);
}

#[test]
fn measure_between_instances() {
    use crate::regen::measure::Measurement;
    let f = doc_with(vec![block_studio()]);
    let mut ev = AssemblyEvaluator::new();
    let (body, r) = body_of(&mut ev, &f.doc, f.studios[0]);
    let top = face_with(&r, "CapEnd");
    let bottom = face_with(&r, "CapStart");
    let asm = Assembly {
        instances: vec![part_instance(1, "Block <1>", f.studios[0], body.clone(), DVec3::ZERO, true), part_instance(2, "Block <2>", f.studios[0], body, DVec3::new(0.0, 0.0, 0.03), false)],
        ..Default::default()
    };
    let res = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
    // Top of block 1 (z = 10 mm) to the bottom of block 2 (z = 30 mm): 20 mm, parallel.
    let m = analysis::measure_entities(&res, &[(vec![InstanceId(1)], top.clone()), (vec![InstanceId(2)], bottom)]);
    let d = m.iter().find_map(|(_, m)| if let Measurement::Distance { distance, .. } = m { Some(*distance) } else { None }).unwrap();
    assert!((d - 0.02).abs() < 1e-9, "{d}");
    let m = analysis::measure_entities(&res, &[(vec![InstanceId(2)], top)]);
    assert!(m.iter().any(|(l, m)| l == "Area" && matches!(m, Measurement::Area(a) if (a - 0.0008).abs() < 1e-9)));
}

#[test]
fn standard_content_parts_regenerate_with_mate_connectors() {
    use super::standard::{self, CATALOGUE, StandardSpec};
    for c in CATALOGUE {
        let spec = StandardSpec::new(c);
        let r = standard::regenerate(&spec).unwrap_or_else(|e| panic!("{e}"));
        let failed: Vec<_> = r.display.features.iter().filter_map(|f| f.message.clone()).collect();
        assert!(failed.is_empty(), "{}: {failed:?}", spec.part_name());
        let body = r.context.body(&standard::part_body(&r).unwrap()).unwrap();
        let v = body.shape.volume().unwrap();
        let d = c.diameter_m(c.size(&spec.size).unwrap());
        assert!(v > d * d * d * 0.05, "{}: volume {v}", spec.part_name());
        // Bolts: one connector under the head; nuts and washers: both faces.
        let want = if c.shape.has_length() { 1 } else { 2 };
        assert_eq!(r.context.connectors.len(), want, "{}", spec.part_name());
        assert!(r.context.connectors[0].origin.length() < 1e-9);
        // Primary axis out of the contact face (mates oppose Z axes).
        assert!((r.context.connectors[0].z + DVec3::Z).length() < 1e-9);
    }
}

#[test]
fn standard_content_auto_size_and_bom() {
    use super::standard::{self, CATALOGUE, StandardSpec, auto_size};
    let bolt = CATALOGUE.iter().find(|c| c.name == "Hex socket head cap screw ISO 4762").unwrap();
    let nut = CATALOGUE.iter().find(|c| c.name == "Hex nut grade A & B ISO 4032").unwrap();
    let washer = CATALOGUE.iter().find(|c| c.name == "Plain washer normal grade A ISO 7089").unwrap();
    // A 6.4 mm clearance hole: bolts round down, nuts round up; a 6 mm shaft takes an M6 washer.
    assert_eq!(auto_size(bolt, 0.0064), Some("M6"));
    assert_eq!(auto_size(nut, 0.0064), Some("M8"));
    assert_eq!(auto_size(nut, 0.006), Some("M6"));
    assert_eq!(auto_size(washer, 0.006), Some("M6"));
    assert_eq!(auto_size(bolt, 0.001), None);
    assert_eq!(standard::length_value("1 1/4 in"), Some(1.25 * 0.0254));
    let mut spec = StandardSpec::new(bolt);
    assert_eq!(spec.size, "M8");
    spec.set_size("M6");
    assert_eq!(spec.part_name(), "Hex socket head cap screw ISO 4762 - M6 x 25 mm");
    let mut long = spec.clone();
    long.length = Some("40 mm".into());
    let inst = |id: u64, spec: &StandardSpec, x: f64| Instance::new(InstanceId(id), format!("{} <{id}>", spec.part_name()), InstanceSource::Standard { spec: spec.clone() }, Placement { translation: [x, 0.0, 0.0], rotation: [0.0, 0.0, 0.0, 1.0] });
    let asm = Assembly { instances: vec![inst(1, &spec, 0.0), inst(2, &spec, 0.02), inst(3, &long, 0.04)], ..Default::default() };
    let doc = Document::new();
    let mut ev = AssemblyEvaluator::new();
    let res = ev.evaluate(&doc, &asm, &SolveOptions::default(), None);
    assert_eq!(res.occurrences.len(), 3);
    let rows = bom(&res);
    assert_eq!(rows.iter().map(|r| r.quantity).collect::<Vec<_>>(), vec![2, 1]);
    // The 40 mm screw reaches 15 mm further down than the 25 mm one.
    let low = |i: usize| res.occurrences[i].world_bounds().unwrap().0.z;
    assert!((low(0) - low(2) - 0.015).abs() < 1e-6, "{} {}", low(0), low(2));
}

#[test]
fn isolate_expands_by_distance_and_connectivity() {
    let f = doc_with(vec![block_studio()]);
    let mut ev = AssemblyEvaluator::new();
    let (body, _) = body_of(&mut ev, &f.doc, f.studios[0]);
    let at = |id: u64, x: f64| part_instance(id, &format!("Block <{id}>"), f.studios[0], body.clone(), DVec3::new(x, 0.0, 0.0), id == 1);
    // 1 - 2 - 3 mated in a chain; 4 sits right next to 1 but is not mated.
    let mut asm = Assembly { instances: vec![at(1, 0.0), at(2, 0.2), at(3, 0.4), at(4, 0.05)], ..Default::default() };
    let mc = |i: u64| MateConnectorRef::on(InstanceId(i), vec![], Reference::Origin);
    asm.mates.push(Mate::new(MateId(10), "Planar 1".into(), MateType::Planar, mc(1), mc(2)));
    asm.mates.push(Mate::new(MateId(11), "Planar 2".into(), MateType::Planar, mc(2), mc(3)));
    let res = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
    let by_distance = analysis::isolate_expansion(&res, &asm, &[InstanceId(1)], false);
    assert_eq!(by_distance.first(), Some(&vec![InstanceId(4)]));
    assert_eq!(by_distance.len(), 3);
    let by_mates = analysis::isolate_expansion(&res, &asm, &[InstanceId(1)], true);
    assert_eq!(by_mates, vec![vec![InstanceId(2)], vec![InstanceId(3)]]);
}

#[test]
fn mate_connector_between_entities_and_realign() {
    let f = doc_with(vec![block_studio()]);
    let mut ev = AssemblyEvaluator::new();
    let (body, r) = body_of(&mut ev, &f.doc, f.studios[0]);
    let top = face_with(&r, "CapEnd");
    let bottom = face_with(&r, "CapStart");
    let side = side_face(&r);
    let asm_with = |c: AsmMateConnector| Assembly { instances: vec![part_instance(1, "Block <1>", f.studios[0], body.clone(), DVec3::ZERO, true)], connectors: vec![c], ..Default::default() };
    let on = |r: &Reference| MateConnectorRef::on(InstanceId(1), vec![], r.clone());
    // Between the top and bottom faces: halfway up the block (z = 5 mm).
    let mut c = AsmMateConnector::new(MateId(5), "Mate connector 1".into(), on(&top));
    c.between = Some(on(&bottom));
    let res = ev.evaluate(&f.doc, &asm_with(c.clone()), &SolveOptions::default(), None);
    let fr = res.connector_frame(MateId(5)).unwrap();
    assert!((fr.origin - DVec3::new(0.02, 0.01, 0.005)).length() < 1e-9, "{:?}", fr.origin);
    // Realign: Z along a side face's normal (horizontal).
    c.primary_axis = Some(on(&side));
    let res = ev.evaluate(&f.doc, &asm_with(c.clone()), &SolveOptions::default(), None);
    let fr = res.connector_frame(MateId(5)).unwrap();
    assert!(fr.z().z.abs() < 1e-9 && (fr.z().length() - 1.0).abs() < 1e-9, "{:?}", fr.z());
    // A secondary axis parallel to the primary one is an error.
    c.secondary_axis = Some(on(&side));
    let res = ev.evaluate(&f.doc, &asm_with(c), &SolveOptions::default(), None);
    assert!(res.connector_frame(MateId(5)).is_none());
}

/// A planar side face of the block (normal horizontal).
fn side_face(r: &crate::regen::RegenResult) -> Reference {
    let b = &r.context.bodies[0];
    for (i, f) in b.faces.iter().enumerate() {
        let face = b.shape.sub_shape(noshape_kernel::ShapeType::Face, i).unwrap();
        if let Ok(Some(p)) = face.face_plane()
            && p.normal.z.abs() < 1e-9
        {
            return Reference::Face((**f).clone());
        }
    }
    panic!("no side face");
}

#[test]
fn reorder_mate_features() {
    let mc = |i: u64| MateConnectorRef::on(InstanceId(i), vec![], Reference::Origin);
    let mut asm = Assembly::default();
    for (id, n) in [(1, "Fastened 1"), (2, "Revolute 1"), (3, "Slider 1")] {
        asm.mates.push(Mate::new(MateId(id), n.into(), MateType::Fastened, mc(10), mc(11)));
    }
    asm.connectors.push(AsmMateConnector::new(MateId(4), "Mate connector 1".into(), MateConnectorRef::origin()));
    assert!(asm.move_feature_before(MateId(3), MateId(1)));
    assert_eq!(asm.mates.iter().map(|m| m.id.0).collect::<Vec<_>>(), vec![3, 1, 2]);
    assert!(asm.move_feature_before(MateId(3), MateId(2)));
    assert_eq!(asm.mates.iter().map(|m| m.id.0).collect::<Vec<_>>(), vec![1, 3, 2]);
    // Different kinds don't mix.
    assert!(!asm.move_feature_before(MateId(4), MateId(1)));
}

#[test]
fn hole_connectors_through_a_filleted_rim() {
    use super::standard::hole_connectors;
    // The demo plate: holes with a filleted top edge.
    let mut doc = Document::new();
    if let TabContent::PartStudio(ps) = &mut doc.tabs[0].content {
        *ps = crate::demo::plate_studio();
    }
    let tab = doc.tabs[0].id;
    let mut ev = AssemblyEvaluator::new();
    let r = ev.studio(&doc, tab, &ConfigValues::default()).unwrap();
    let b = &r.context.bodies[0];
    let units = UnitPrefs::default();
    // Small holes (radius 4.8 mm): their cylinder and the fillet around them.
    let mut cylinders = 0;
    for (i, f) in b.faces.iter().enumerate() {
        let face = b.shape.sub_shape(noshape_kernel::ShapeType::Face, i).unwrap();
        let surf = face.face_surface().unwrap();
        let is_small_cyl = matches!(surf, noshape_kernel::Surface::Cylinder { radius, .. } if (radius - 0.0048).abs() < 1e-6);
        if !is_small_cyl {
            continue;
        }
        cylinders += 1;
        let holes = hole_connectors(&r.context, None, &Reference::Face((**f).clone()), DVec3::Z, &units);
        assert_eq!(holes.len(), 1);
        let (p, d) = &holes[0];
        assert!((d - 0.0096).abs() < 1e-9, "{d}");
        // The top end: on the top face (z = 12 mm), primary axis up.
        assert!((p.frame.origin.z - 0.012).abs() < 1e-9 && p.frame.z().z > 0.999, "{:?} {:?}", p.frame.origin, p.frame.z());
    }
    assert_eq!(cylinders, 2);
}

#[test]
fn best_available_tessellation_refines_only_flagged_instances() {
    let f = doc_with(vec![pin_studio()]);
    let mut ev = AssemblyEvaluator::new();
    let (pin, _) = body_of(&mut ev, &f.doc, f.studios[0]);
    let mut asm = Assembly {
        instances: vec![
            part_instance(1, "Pin <1>", f.studios[0], pin.clone(), DVec3::ZERO, true),
            part_instance(2, "Pin <2>", f.studios[0], pin, DVec3::new(0.05, 0.0, 0.0), false),
        ],
        ..Default::default()
    };
    let tris = |r: &eval::AssemblyResult, i: usize| r.occurrences[i].mesh.indices.len() / 3;
    let res = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
    let normal = tris(&res, 0);
    assert_eq!(normal, tris(&res, 1));
    asm.instances[1].best_tessellation = true;
    let res = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
    assert_eq!(tris(&res, 0), normal, "other instances keep the regular mesh");
    assert!(tris(&res, 1) > normal, "best available tessellation is finer: {} vs {normal}", tris(&res, 1));
    // The flag round-trips and is omitted when off.
    let json = serde_json::to_string(&asm.instances[0]).unwrap();
    assert!(!json.contains("best_tessellation"));
    let back: Instance = serde_json::from_str(&serde_json::to_string(&asm.instances[1]).unwrap()).unwrap();
    assert!(back.best_tessellation);
}
