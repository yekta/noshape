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
    MateConnectorRef { instance: InstanceId(instance), path: vec![], reference: Reference::Implicit { on: Box::new(on), inference }, flip: false, rotate_quarters: 0 }
}

fn mate(id: u64, t: MateType, a: MateConnectorRef, b: MateConnectorRef) -> Mate {
    Mate {
        id: MateId(id),
        name: format!("{} {id}", t.name()),
        mate_type: t,
        a,
        b,
        offset_z: None,
        offset_x: None,
        offset_y: None,
        rotation: None,
        flip_primary: false,
        reorient_secondary: 0,
        limits: vec![],
        suppressed: false,
    }
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
            MateConnectorRef { instance: InstanceId(5), path: vec![InstanceId(1)], reference: Reference::Implicit { on: Box::new(bottom), inference: Inference::Centroid }, flip: false, rotate_quarters: 0 },
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
        relations: vec![Relation { id: MateId(40), name: "Gear 1".into(), relation_type: RelationType::Gear, mates: [MateId(10), MateId(11)], ratio: ExprValue::new("2"), reverse: false, suppressed: false }],
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
}
