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
            Instance { id: InstanceId(2), name: "Part 1 <1>".into(), source: InstanceSource::External { key: "ext:v1".into(), body: Some(ebody), config: ConfigValues::default() }, placement: Placement { translation: [0.1, 0.0, 0.0], rotation: [0.0, 0.0, 0.0, 1.0] }, fixed: false, hidden: false, suppressed: false },
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
