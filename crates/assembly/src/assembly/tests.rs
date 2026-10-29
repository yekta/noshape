//! Assembly evaluation tests with programmatic Part Studios.

use crate::prelude::*;
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
        reference_designator: None,
        unsuppressed_if: None,
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

    // Offset of 5 mm: the first connector (the fixed block's top, Z up)
    // moves +5 mm along its Z relative to the second, so the second block
    // sinks 5 mm into it (live assembly/740–748, oracle_asm F2).
    let mut asm2 = asm.clone();
    asm2.mates[0].offset_z = Some(ExprValue::new("5 mm"));
    let res = ev.evaluate(&f.doc, &asm2, &SolveOptions::default(), None);
    let o = origin_of(&res, 2);
    assert!((o.z - 0.005).abs() < 1e-6, "offset: {o:?}");

    // Same-orientation connectors (live assembly/740–748: the top faces of
    // two plates, Z axes aligned): the instance picked first ends +offset
    // along its connector's Z, whichever of the two is fixed.
    let top2 = face_with(&r, "CapEnd");
    for (first, second, expect) in [(2u64, 1u64, 0.01), (1, 2, -0.01)] {
        let mut m = mate(11, MateType::Revolute, conn(first, top2.clone(), Inference::Centroid), conn(second, top2.clone(), Inference::Centroid));
        m.flip_primary = true;
        m.offset_z = Some(ExprValue::new("10 mm"));
        let a = Assembly { mates: vec![m], ..asm.clone() };
        let res = ev.evaluate(&f.doc, &a, &SolveOptions::default(), None);
        assert!(res.mates[0].ok, "{:?}", res.mates[0].message);
        let o = origin_of(&res, 2);
        assert!((o.z - expect).abs() < 1e-6, "{first} picked first: block 2 at {o:?}");
    }

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
fn a_mate_being_defined_is_solved_alone_until_solve() {
    let f = doc_with(vec![block_studio()]);
    let mut ev = AssemblyEvaluator::new();
    let (body, r) = body_of(&mut ev, &f.doc, f.studios[0]);
    let top = face_with(&r, "CapEnd");
    let bottom = face_with(&r, "CapStart");
    // Block 3 already sits on block 2 (mate 11); the new mate 10 puts 2 on 1.
    let at2 = DVec3::new(0.07, 0.03, -0.02);
    let asm = Assembly {
        instances: vec![
            part_instance(1, "Block <1>", f.studios[0], body.clone(), DVec3::ZERO, true),
            part_instance(2, "Block <2>", f.studios[0], body.clone(), at2, false),
            part_instance(3, "Block <3>", f.studios[0], body.clone(), at2 + DVec3::new(0.0, 0.0, 0.01), false),
        ],
        mates: vec![
            mate(11, MateType::Fastened, conn(2, top.clone(), Inference::Centroid), conn(3, bottom.clone(), Inference::Centroid)),
            mate(10, MateType::Fastened, conn(1, top, Inference::Centroid), conn(2, bottom, Inference::Centroid)),
        ],
        ..Default::default()
    };
    // Before Solve: only mate 10 moves block 2; block 3 stays, mate 11 reports ok.
    let res = ev.evaluate(&f.doc, &asm, &SolveOptions { only_mate: Some(MateId(10)), ..Default::default() }, None);
    assert!((origin_of(&res, 2) - DVec3::new(0.0, 0.0, 0.01)).length() < 1e-6, "{:?}", origin_of(&res, 2));
    assert!((origin_of(&res, 3) - (at2 + DVec3::new(0.0, 0.0, 0.01))).length() < 1e-9, "{:?}", origin_of(&res, 3));
    assert!(res.mates.iter().all(|m| m.ok));
    // The instance list's degrees of freedom are the whole assembly's with
    // the new mate: block 3 is fastened through block 2 to block 1.
    let dof = |id: u64| res.nodes.iter().find(|n| n.path == [InstanceId(id)]).and_then(|n| n.dof);
    assert_eq!((dof(2), dof(3)), (Some(0), Some(0)));
    // Solve (and accepting): every mate, so block 3 follows.
    let res = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
    assert!((origin_of(&res, 3) - DVec3::new(0.0, 0.0, 0.02)).length() < 1e-6, "{:?}", origin_of(&res, 3));
    // Dragging during the edit moves the connected group (live 587–589):
    // every mate is enforced while a drag is solved.
    let drag = SolveOptions { only_mate: Some(MateId(10)), drags: vec![(InstanceId(2), DVec3::ZERO, DVec3::new(0.0, 0.0, 0.01))], ..Default::default() };
    let res = ev.evaluate(&f.doc, &asm, &drag, None);
    assert!((origin_of(&res, 3) - DVec3::new(0.0, 0.0, 0.02)).length() < 1e-6, "{:?}", origin_of(&res, 3));
}

#[test]
fn implicit_connector_move_offsets_the_mate() {
    let f = doc_with(vec![block_studio()]);
    let mut ev = AssemblyEvaluator::new();
    let (body, r) = body_of(&mut ev, &f.doc, f.studios[0]);
    let top = face_with(&r, "CapEnd");
    let bottom = face_with(&r, "CapStart");
    let mut asm = Assembly {
        instances: vec![
            part_instance(1, "Block <1>", f.studios[0], body.clone(), DVec3::ZERO, true),
            part_instance(2, "Block <2>", f.studios[0], body, DVec3::new(0.07, 0.03, -0.02), false),
        ],
        mates: vec![mate(10, MateType::Fastened, conn(1, top, Inference::Centroid), conn(2, bottom, Inference::Centroid))],
        ..Default::default()
    };
    let z = |ev: &mut AssemblyEvaluator, asm: &Assembly| origin_of(&ev.evaluate(&f.doc, asm, &SolveOptions::default(), None), 2).z;
    let z0 = z(&mut ev, &asm);
    // The first connector moved 5 mm along its Z: the second part follows,
    // and the connector stays implicit (no assembly mate connector).
    asm.mates[0].a.moved = Some(Box::new(super::ConnectorMove { translation: [None, None, Some(noshape_units::ExprValue::new("5 mm"))], rotation_axis: 2, rotation: None, off: false }));
    let z1 = z(&mut ev, &asm);
    assert!((z1 - z0 - 0.005).abs() < 1e-6, "{z0} -> {z1}");
    assert!(asm.connectors.is_empty());
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
    // Live's sense (oracle_asm S1): B above A reads negative.
    m.limits = vec![("z".into(), Limits { min: Some(ExprValue::new("-20 mm")), max: Some(ExprValue::new("0 mm")) })];
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
    // The Interference detection dialog: only the instances to analyze, with
    // the overlap's box and mesh (live assembly/735–736).
    let rows = super::analysis::instance_interferences(&res, &[InstanceId(1), InstanceId(2)], false);
    assert_eq!(rows.len(), 1);
    assert_eq!((rows[0].a.clone(), rows[0].b.clone()), (vec![InstanceId(1)], vec![InstanceId(2)]));
    let (lo, hi) = rows[0].overlap.bounds;
    assert!(((hi - lo) - DVec3::new(0.04, 0.02, 0.005)).length() < 1e-6, "{lo:?} {hi:?}");
    assert!(!rows[0].overlap.indices.is_empty());
    assert!(super::analysis::instance_interferences(&res, &[InstanceId(1), InstanceId(3)], false).is_empty());
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
    let sub_tab = f.doc.add_tab("Sub", TabContent::Assembly(Box::new(sub)));
    // Top: a fixed block and the sub-assembly fastened onto it by its upper block's top face.
    let top_asm = Assembly {
        instances: vec![
            part_instance(1, "Base <1>", f.studios[0], body.clone(), DVec3::ZERO, true),
            Instance {
                id: InstanceId(5),
                name: "Sub <1>".into(),
                source: InstanceSource::Assembly { assembly: sub_tab, config: ConfigValues::default() },
                placement: Placement { translation: [0.2, 0.0, 0.0], rotation: [0.0, 0.0, 0.0, 1.0] },
                fixed: false,
                hidden: false,
                suppressed: false,
                lock: SubassemblyLock::Flexible,
                nested: Vec::new(),
                best_tessellation: false,
                reference_designator: None,
                unsuppressed_if: None,
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
    // Live E8: a second gear with another ratio on the same pair: no error.
    asm.relations[0].relation_type = RelationType::Gear;
    let mut g2 = asm.relations[0].clone();
    (g2.id, g2.name, g2.ratio) = (MateId(41), "Gear 2".into(), ExprValue::new("1"));
    asm.relations.push(g2);
    let res = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
    assert!(res.relations.iter().all(|r| r.ok && r.message.is_none()), "{:?}", res.relations);
    // Live E4: a suppressed mated instance makes the mate and its relations inactive.
    asm.instance_mut(InstanceId(3)).unwrap().suppressed = true;
    let res = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
    let m11 = res.mates.iter().find(|m| m.id == MateId(11)).unwrap();
    assert!(m11.inactive && m11.message.as_deref() == Some("One or more instances is suppressed"), "{m11:?}");
    assert!(res.relations.iter().all(|r| r.inactive && r.message.as_deref() == Some("One or more mates is inactive or suppressed")), "{:?}", res.relations);
    // Live E5: a suppressed mate: the relations are inactive, the mate has no reason.
    asm.instance_mut(InstanceId(3)).unwrap().suppressed = false;
    asm.mates[1].suppressed = true;
    let res = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
    assert!(res.mates.iter().find(|m| m.id == MateId(11)).unwrap().message.is_none());
    assert!(res.relations.iter().all(|r| r.inactive), "{:?}", res.relations);
    // Live E3: both instances fixed.
    asm.mates[1].suppressed = false;
    asm.instance_mut(InstanceId(3)).unwrap().fixed = true;
    let res = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
    let m11 = res.mates.iter().find(|m| m.id == MateId(11)).unwrap();
    assert!(!m11.ok && m11.message.as_deref() == Some("Mate is between fixed instances."), "{m11:?}");
    // Live E11: minimum limit 30 deg above the maximum 10 deg.
    asm.instance_mut(InstanceId(3)).unwrap().fixed = false;
    asm.mates[1].limits = vec![("rz".into(), Limits { min: Some(ExprValue::new("30 deg")), max: Some(ExprValue::new("10 deg")) })];
    let res = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
    let m11 = res.mates.iter().find(|m| m.id == MateId(11)).unwrap();
    assert_eq!(m11.message.as_deref(), Some("Minimum mate limit should be smaller than maximum limit. A parameter value is not valid."));
}

fn x_axis_connector(id: u64) -> AsmMateConnector {
    // Assembly origin connector turned so its Z axis points along +X.
    let mut c = AsmMateConnector::new(MateId(id), format!("Mate connector {id}"), MateConnectorRef::origin());
    c.rotation_axis = 1;
    c.rotation = Some(ExprValue::new("90 deg"));
    c
}

/// qa5 A9 (live "qa5 assembly" 6): a linear pattern whose direction is a
/// part edge copies along that edge, not along the face normal.
/// qa5 A4 (live "qa5 assembly" 3): a Slider's Flip primary axis turns the
/// moving block over, like Fastened's.
#[test]
fn slider_flip_primary_turns_the_part_over() {
    let f = doc_with(vec![block_studio()]);
    let mut ev = AssemblyEvaluator::new();
    let (body, r) = body_of(&mut ev, &f.doc, f.studios[0]);
    let top = face_with(&r, "CapEnd");
    let bottom = face_with(&r, "CapStart");
    let mut asm = Assembly {
        instances: vec![
            part_instance(1, "Block <1>", f.studios[0], body.clone(), DVec3::ZERO, true),
            part_instance(2, "Block <2>", f.studios[0], body.clone(), DVec3::new(0.07, 0.03, -0.02), false),
        ],
        mates: vec![mate(10, MateType::Slider, conn(2, bottom, Inference::Centroid), conn(1, top.clone(), Inference::Centroid))],
        ..Default::default()
    };
    let up = |asm: &Assembly, ev: &mut AssemblyEvaluator| {
        let res = ev.evaluate(&f.doc, asm, &SolveOptions::default(), None);
        assert!(res.mates[0].ok, "{:?}", res.mates[0].message);
        let p = res.solved.placement(InstanceId(2)).unwrap();
        DQuat::from_xyzw(p.rotation[0], p.rotation[1], p.rotation[2], p.rotation[3]) * DVec3::Z
    };
    assert!(up(&asm, &mut ev).z > 0.999, "upright before the flip");
    asm.mates[0].flip_primary = true;
    let z = up(&asm, &mut ev);
    assert!(z.z < -0.999, "turned over after the flip: {z:?}");
    // qa5 A4: top face onto top face (the Z axes agree, the block sits in
    // the base); the flip turns it over, onto the base.
    asm.mates[0] = mate(10, MateType::Slider, conn(2, top.clone(), Inference::Centroid), conn(1, top, Inference::Centroid));
    asm.mates[0].flip_primary = true;
    assert!(up(&asm, &mut ev).z > 0.999, "Z axes agree: upright, in the base");
    asm.mates[0].flip_primary = false;
    let z = up(&asm, &mut ev);
    assert!(z.z < -0.999, "turned over onto the base after the flip: {z:?}");
    // The new mate's preview holds its free value at zero: the connectors
    // meet, so the turned-over block sits on the base (not below it).
    let opts = SolveOptions { hold_values: eval::new_mate_holds(&asm.mates[0]), ..Default::default() };
    let res = ev.evaluate(&f.doc, &asm, &opts, None);
    let (fa, fb) = (res.mates[0].frame_a.unwrap(), res.mates[0].frame_b.unwrap());
    assert!((fa.origin - fb.origin).length() < 1e-6, "connectors meet: {:?} {:?}", fa.origin, fb.origin);
}

#[test]
fn linear_pattern_along_an_edge() {
    let f = doc_with(vec![block_studio()]);
    let mut ev = AssemblyEvaluator::new();
    let (body, r) = body_of(&mut ev, &f.doc, f.studios[0]);
    let top = face_with(&r, "CapEnd");
    let bottom = face_with(&r, "CapStart");
    // A straight edge of the 40 x 20 block running along X.
    let b = &r.context.bodies[0];
    let edge = b
        .edges
        .iter()
        .enumerate()
        .find_map(|(i, n)| match b.shape.sub_shape(noshape_kernel::ShapeType::Edge, i).ok()?.edge_info().ok()?.curve {
            noshape_kernel::Curve::Line { direction, .. } if direction.x.abs() > 0.99 => Some(Reference::Edge((**n).clone())),
            _ => None,
        })
        .unwrap();
    let mut asm = Assembly {
        instances: vec![part_instance(1, "Base <1>", f.studios[0], body.clone(), DVec3::ZERO, true), part_instance(2, "Block <1>", f.studios[0], body.clone(), DVec3::new(0.0, 0.1, 0.0), false)],
        mates: vec![mate(10, MateType::Fastened, conn(1, top, Inference::Centroid), conn(2, bottom, Inference::Centroid))],
        ..Default::default()
    };
    let direction = conn(1, edge, Inference::Midpoint).linear_direction();
    asm.patterns.push(InstancePattern {
        id: MateId(70),
        name: "Linear pattern 1".into(),
        kind: PatternKind::Linear { direction, distance: ExprValue::new("25 mm"), count: 3, opposite: false, equal_spacing: false, second: None, centered: false },
        seeds: vec![InstanceId(2)],
        generated: vec![],
        suppressed: false,
    });
    let created = asm.sync_pattern(MateId(70));
    // Plain solve, then while the seed's mate is being edited (qa5 P2-A4:
    // only that mate is solved, but the copies still follow their seed).
    for opts in [SolveOptions::default(), SolveOptions { only_mate: Some(MateId(10)), ..Default::default() }] {
        let res = ev.evaluate(&f.doc, &asm, &opts, None);
        assert!(res.pattern_errors.is_empty(), "{:?}", res.pattern_errors);
        let seed = origin_of(&res, 2);
        for (k, id) in created.iter().enumerate() {
            let d = DVec3::from(res.solved.placement(*id).unwrap().translation) - seed;
            assert!((d.x.abs() - 0.025 * (k + 1) as f64).abs() < 1e-6 && d.y.abs() < 1e-6 && d.z.abs() < 1e-6, "copy {k} along the edge (X), only_mate {:?}: {d:?}", opts.only_mate);
        }
    }
}

/// Help "Assembly linear pattern" 7 / live qa5 dialog: Centered puts the
/// seed in the middle, `count` instances (with the seed) each way.
#[test]
fn centered_linear_pattern_goes_both_ways() {
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
        kind: PatternKind::Linear { direction: MateConnectorRef::assembly_connector(MateId(60), InstanceId::ORIGIN), distance: ExprValue::new("25 mm"), count: 3, opposite: false, equal_spacing: false, second: None, centered: true },
        seeds: vec![InstanceId(2)],
        generated: vec![],
        suppressed: false,
    });
    let created = asm.sync_pattern(MateId(70));
    assert_eq!(created.len(), 4, "two each way");
    let res = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
    assert!(res.pattern_errors.is_empty(), "{:?}", res.pattern_errors);
    let seed = origin_of(&res, 2);
    let mut xs: Vec<f64> = created.iter().map(|id| (DVec3::from(res.solved.placement(*id).unwrap().translation) - seed).x).collect();
    xs.sort_by(f64::total_cmp);
    for (x, want) in xs.iter().zip([-0.05, -0.025, 0.025, 0.05]) {
        assert!((x - want).abs() < 1e-6, "{xs:?}");
    }
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
        kind: PatternKind::Linear { direction: MateConnectorRef::assembly_connector(MateId(60), InstanceId::ORIGIN), distance: ExprValue::new("50 mm"), count: 3, opposite: false, equal_spacing: false, second: None, centered: false },
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
        kind: PatternKind::Circular { axis: MateConnectorRef::origin(), angle: ExprValue::new("360 deg"), count: 4, opposite: false, equal_spacing: true, centered: false },
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
        kind: PatternKind::Mirror { plane: MateConnectorRef::assembly_connector(MateId(60), InstanceId::ORIGIN), handed: false, derived: vec![], placement: vec![], mirrored_studios: vec![] },
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

/// Live's Mirror strategy (behaviours "skb (10-23 15:30)" (2), (3)): a part's
/// symmetry is detected about its bounding box mid-planes; Transform with
/// Placement type Bounding box XY / Origin XY is the reflection completed by
/// the part's XY plane through its bounds' centre / its origin (a rotation
/// of 180° about Y for a mirror in X): Bounding box XY keeps the Z range,
/// Origin XY flips the copy below Z = 0.
#[test]
fn mirror_placement_types_and_symmetry() {
    use super::MirrorPlacement;
    let block = noshape_kernel::make_box_corners(DVec3::ZERO, DVec3::new(0.05, 0.02, 0.04)).unwrap();
    assert_eq!(super::eval::part_symmetry(&block), [true; 3]);
    let notch = noshape_kernel::make_box_corners(DVec3::ZERO, DVec3::new(0.01, 0.005, 0.01)).unwrap();
    let notched = noshape_kernel::cut(&block, &notch).unwrap().shape;
    assert_eq!(super::eval::part_symmetry(&notched), [false; 3]);
    // Fillets (tori, B-spline blends) don't spoil it: the top edges rounded.
    let rounded = with_feature(block_studio(), "fillet", |f, r| {
        f.params.set("entities", ParamValue::Query(edges_with(r, "CapEnd")));
        f.params.set("radius", ParamValue::Expr(ExprValue::new("2 mm")));
    });
    let fr = doc_with(vec![rounded]);
    let mut evr = AssemblyEvaluator::new();
    let (rb, rr) = body_of(&mut evr, &fr.doc, fr.studios[0]);
    assert_eq!(super::eval::part_symmetry(&rr.context.body(&rb).unwrap().shape), [true, true, false]);
    // The demo parts: the plate (off-centre holes, top fillet) has none, the
    // pin every one, the flange X and Y.
    let doc = crate::demo::demo_document();
    for (tab, want) in [("Part Studio 1", [false; 3]), ("Pin", [true; 3]), ("Flange", [true, true, false])] {
        let t = doc.tabs.iter().find(|t| t.name == tab).unwrap();
        let r = AssemblyEvaluator::new().studio(&doc, t.id, &Default::default()).unwrap();
        assert_eq!(super::eval::part_symmetry(&r.context.bodies[0].shape), want, "{tab}");
    }
    let f = doc_with(vec![block_studio()]);
    let mut ev = AssemblyEvaluator::new();
    let (body, _) = body_of(&mut ev, &f.doc, f.studios[0]);
    for (pl, z_lo, z_hi) in [(MirrorPlacement::BoxXY, 0.0, 0.01), (MirrorPlacement::OriginXY, -0.01, 0.0)] {
        let mut asm = Assembly {
            instances: vec![part_instance(1, "Block <1>", f.studios[0], body.clone(), DVec3::new(0.05, 0.0, 0.0), true)],
            connectors: vec![x_axis_connector(60)],
            ..Default::default()
        };
        asm.patterns.push(InstancePattern {
            id: MateId(70),
            name: "Mirror 1".into(),
            kind: PatternKind::Mirror { plane: MateConnectorRef::assembly_connector(MateId(60), InstanceId::ORIGIN), handed: false, derived: vec![], placement: vec![(InstanceId(1), pl)], mirrored_studios: vec![] },
            seeds: vec![InstanceId(1)],
            generated: vec![],
            suppressed: false,
        });
        let created = asm.sync_pattern(MateId(70));
        let res = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
        assert_eq!(res.prepared.mirror_symmetric.get(&InstanceId(1)), Some(&true));
        let occ = res.occurrences.iter().find(|o| o.top() == created[0]).unwrap();
        let (lo, hi) = occ.world_bounds().unwrap();
        assert!((lo - DVec3::new(-0.09, 0.0, z_lo)).length() < 1e-6 && (hi - DVec3::new(-0.05, 0.02, z_hi)).length() < 1e-6, "{pl:?}: {lo:?} {hi:?}");
        // 180° about Y.
        let q = quat_of(&res.solved.placement(created[0]).unwrap());
        assert!((q * DVec3::X + DVec3::X).length() < 1e-6 && (q * DVec3::Y - DVec3::Y).length() < 1e-6, "{pl:?}: {q:?}");
    }
}

/// Live's Mirror strategy › Update with Derived (behaviours "skb (10-23
/// 15:30)" (1), (4)): a "<Part Studio>-Mirrored" Part Studio with one
/// "Derived mirror" feature, inside the folder "Mirrored parts and
/// assemblies" at the end of the tab bar; a second Derived mirror of the same
/// source reuses it.
#[test]
fn derived_mirror_makes_one_mirrored_studio() {
    let mut f = doc_with(vec![block_studio()]);
    let src = f.studios[0];
    let name = f.doc.tab(src).unwrap().name.clone();
    let m = f.doc.ensure_mirrored_studio(src).unwrap();
    let tab = f.doc.tab(m).unwrap();
    assert_eq!(tab.name, format!("{name}-Mirrored"));
    let crate::document::TabContent::PartStudio(ps) = &tab.content else { panic!("a Part Studio") };
    assert_eq!(ps.features.len(), 1);
    assert_eq!((ps.features[0].type_id.as_str(), ps.features[0].name.as_str()), ("derivedMirror", "Derived mirror"));
    let folder = f.doc.parent_of(m).unwrap();
    assert_eq!(f.doc.folder(folder).unwrap().name, crate::document::MIRRORED_FOLDER);
    assert_eq!(f.doc.settings.tab_order.iter().rev().take(2).copied().collect::<Vec<_>>(), vec![m, folder]);
    assert_eq!(f.doc.ensure_mirrored_studio(src), Some(m));
    assert_eq!(f.doc.tabs.len(), 2);
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

/// Live (behaviours "qa5 assembly" 5, qa5 A7): a seed mated on a hole's
/// circular top edge replicates once per other matching hole (through the
/// hole's face), on that hole's top edge; never onto the seed's own hole
/// (its other end) nor twice into one hole.
#[test]
fn replicate_from_a_hole_edge_matches_holes_once() {
    let f = doc_with(vec![plate_with_holes(), pin_studio()]);
    let mut ev = AssemblyEvaluator::new();
    let (plate, rp) = body_of(&mut ev, &f.doc, f.studios[0]);
    let (pin, rq) = body_of(&mut ev, &f.doc, f.studios[1]);
    // The top circular edge (the higher of the two) of the 5 mm hole at x = -20 mm.
    let b = &rp.context.bodies[0];
    let circles: Vec<(Reference, DVec3)> = b
        .edges
        .iter()
        .enumerate()
        .filter_map(|(i, n)| match b.shape.sub_shape(noshape_kernel::ShapeType::Edge, i).ok()?.edge_info().ok()?.curve {
            noshape_kernel::Curve::Circle { center, radius, .. } if (radius - 0.005).abs() < 1e-9 => Some((Reference::Edge((**n).clone()), center)),
            _ => None,
        })
        .collect();
    assert_eq!(circles.len(), 4, "two holes, two circles each");
    let top = circles.iter().map(|(_, c)| c.z).fold(f64::MIN, f64::max);
    let seed_edge = circles.iter().find(|(_, c)| c.x < 0.0 && (c.z - top).abs() < 1e-9).unwrap().0.clone();
    let other_top = circles.iter().find(|(_, c)| c.x > 0.0 && (c.z - top).abs() < 1e-9).unwrap().0.clone();
    let pf = cylinder_face(&rq);
    let mut asm = Assembly {
        instances: vec![part_instance(1, "Plate <1>", f.studios[0], plate, DVec3::ZERO, true), part_instance(2, "Pin <1>", f.studios[1], pin, DVec3::new(0.0, 0.05, 0.0), false)],
        mates: vec![mate(10, MateType::Cylindrical, conn(1, seed_edge, Inference::Center), conn(2, pf, Inference::MidAxis))],
        ..Default::default()
    };
    let res = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
    assert!(res.mates[0].ok);
    asm.patterns.push(InstancePattern { id: MateId(80), name: "Replicate 1".into(), kind: PatternKind::Replicate { mate: MateId(10), all_instances: false, scope: None, scope_refs: vec![] }, seeds: vec![InstanceId(2)], generated: vec![], suppressed: false });
    let targets = analysis::matched_targets(&res, &asm, &asm.patterns[0], &UnitPrefs::default()).unwrap();
    assert_eq!(targets.len(), 1, "only the other 5 mm hole: {:?}", targets.iter().map(|t| &t.1.reference).collect::<Vec<_>>());
    assert_eq!(targets[0].1.reference, Reference::Implicit { on: Box::new(other_top), inference: Inference::Center }, "on the other hole's top edge");
    let created = asm.sync_matched_pattern(MateId(80), targets);
    let res2 = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
    let o = DVec3::from(res2.solved.placement(created[0]).unwrap().translation);
    let s = DVec3::from(res2.solved.placement(InstanceId(2)).unwrap().translation);
    assert!((o - s - DVec3::new(0.04, 0.0, 0.0)).length() < 1e-6, "same relative height in the other hole: {o:?} vs seed {s:?}");

    // Live's dialog (help: Replicate 6–7): the edge seed with "Match edges on
    // plane" = the plate's top face finds the other hole's top edge only.
    let plate_top = face_with(&rp, "CapEnd");
    let scoped = |scope: ReplicateScope, entity: Option<Reference>| InstancePattern {
        id: MateId(81),
        name: "Replicate 2".into(),
        kind: PatternKind::Replicate { mate: MateId(10), all_instances: false, scope: Some(scope), scope_refs: vec![ScopeEntity { instance: InstanceId(1), path: vec![], entity }] },
        seeds: vec![InstanceId(2)],
        generated: vec![],
        suppressed: false,
    };
    let a0 = Assembly { patterns: vec![], ..asm.clone() };
    let a0 = { let mut a = a0; a.instances.retain(|i| i.id.0 <= 2); a.mates.retain(|m| m.id == MateId(10)); a };
    let r0 = ev.evaluate(&f.doc, &a0, &SolveOptions::default(), None);
    let t = analysis::matched_targets(&r0, &a0, &scoped(ReplicateScope::EdgesOnPlane, Some(plate_top.clone())), &UnitPrefs::default()).unwrap();
    assert_eq!(t.len(), 1, "edges on plane: {:?}", t.iter().map(|t| &t.1.reference).collect::<Vec<_>>());
    assert_eq!(t[0].1.reference, Reference::Implicit { on: Box::new(circles.iter().find(|(_, c)| c.x > 0.0 && (c.z - top).abs() < 1e-9).unwrap().0.clone()), inference: Inference::Center });
    // "Match edges on face" (live's default for an edge seed, behaviours "qa
    // (2026-10-27, P2 batch 2)" (4)) with the same face finds the same edge.
    let t = analysis::matched_targets(&r0, &a0, &scoped(ReplicateScope::EdgesOnFace, Some(plate_top.clone())), &UnitPrefs::default()).unwrap();
    assert_eq!(t.len(), 1, "edges on face: {:?}", t.iter().map(|t| &t.1.reference).collect::<Vec<_>>());
    // No selection yet: the dialog's "Parts to find match in" is required.
    let mut none = scoped(ReplicateScope::EdgesOnPlane, None);
    if let PatternKind::Replicate { scope_refs, .. } = &mut none.kind {
        scope_refs.clear();
    }
    assert!(analysis::matched_targets(&r0, &a0, &none, &UnitPrefs::default()).is_err());
    // A face seed (the hole's cylinder) with "Match faces in parts" = the plate.
    let hole_face = {
        let b = &rp.context.bodies[0];
        b.faces
            .iter()
            .enumerate()
            .find_map(|(i, n)| match b.shape.sub_shape(noshape_kernel::ShapeType::Face, i).ok()?.face_surface().ok()? {
                noshape_kernel::Surface::Cylinder { origin, radius, .. } if (radius - 0.005).abs() < 1e-9 && origin.x < 0.0 => Some(Reference::Face((**n).clone())),
                _ => None,
            })
            .unwrap()
    };
    let mut a1 = a0.clone();
    a1.mates[0] = mate(10, MateType::Cylindrical, conn(1, hole_face, Inference::MidAxis), conn(2, cylinder_face(&rq), Inference::MidAxis));
    let r1 = ev.evaluate(&f.doc, &a1, &SolveOptions::default(), None);
    let t = analysis::matched_targets(&r1, &a1, &scoped(ReplicateScope::FacesInParts, None), &UnitPrefs::default()).unwrap();
    assert_eq!(t.len(), 1, "faces in parts: the other 5 mm hole");
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
    asm.patterns.push(InstancePattern { id: MateId(80), name: "Replicate 1".into(), kind: PatternKind::Replicate { mate: MateId(10), all_instances: false, scope: None, scope_refs: vec![] }, seeds: vec![InstanceId(2)], generated: vec![], suppressed: false });
    let targets = analysis::matched_targets(&res, &asm, &asm.patterns[0], &UnitPrefs::default()).unwrap();
    assert_eq!(targets.len(), 1, "only the other 5 mm hole matches");
    let created = asm.sync_matched_pattern(MateId(80), targets);
    assert_eq!(created.len(), 1);
    assert_eq!(asm.mates.len(), 2);
    // qa5 A12: the copy's mate is listed under "Replicate 1", not at the top.
    assert_eq!(asm.pattern_mates(&asm.patterns[0]).count(), 1);
    assert_eq!(asm.listed_mate_count(), 1);
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
    a3.patterns.push(InstancePattern { id: MateId(81), name: "Replicate 2".into(), kind: PatternKind::Replicate { mate: MateId(10), all_instances: true, scope: None, scope_refs: vec![] }, seeds: vec![InstanceId(2)], generated: vec![], suppressed: false });
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
    // Replaced by a part of the same name: the instance keeps its name (live 843).
    let keep = asm.instances[1].source.clone();
    asm.replace_instance(InstanceId(2), keep, "Part 2", false);
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
    let (body, regen) = body_of(&mut ev, &f.doc, f.studios[0]);
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
    // ☐ Explode lines: none for that step; lines from a picked face start
    // at the face (its centroid), not the instance's center.
    let mut no_lines = view.clone();
    no_lines.steps[1].lines = false;
    assert_eq!(eval::explode_trails(&res, &no_lines, f64::INFINITY).len(), 1);
    let mut from_face = view.clone();
    from_face.steps[0].line_refs = vec![conn(2, face_with(&regen, "CapEnd"), Inference::Centroid)];
    let l = eval::explode_trails(&res, &from_face, f64::INFINITY);
    assert!((l[0].0.z - 0.02).abs() < 1e-9 && (l[0].1.z - 0.07).abs() < 1e-9, "from the top face: {l:?}");
    assert_eq!(view.steps[0].display_name(0), "Explode step 1");
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
    asm.patterns.push(InstancePattern { id: MateId(80), name: "Replicate 1".into(), kind: PatternKind::Replicate { mate: MateId(10), all_instances: false, scope: None, scope_refs: vec![] }, seeds: vec![InstanceId(2)], generated: vec![], suppressed: false });
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
        kind: PatternKind::Mirror { plane: MateConnectorRef::assembly_connector(MateId(60), InstanceId::ORIGIN), handed: true, derived: vec![], placement: vec![], mirrored_studios: vec![] },
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
    // Turning the option off makes the copy a plain (rigid) part again. The
    // plate is symmetric (about its XY mid-plane), so Auto position's rigid
    // copy still lands as the mirror image (the small hole at y = -110 mm).
    if let PatternKind::Mirror { handed, .. } = &mut asm.patterns[0].kind {
        *handed = false;
    }
    asm.sync_pattern(MateId(70));
    assert!(matches!(asm.instance(created[0]).unwrap().source, InstanceSource::Part { .. }));
    let res = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
    assert!(!res.occurrences.iter().find(|o| o.top() == created[0]).unwrap().mirrored);
    let w = res.connector_world(&conn(created[0].0, small, Inference::Centroid), &units).unwrap();
    assert!((w.origin.y + 0.11).abs() < 1e-6, "rigid copy: {:?}", w.origin);
    // Live's Mirror strategy: Mirror action Derived for this seed alone.
    if let PatternKind::Mirror { derived, .. } = &mut asm.patterns[0].kind {
        derived.push(InstanceId(1));
    }
    asm.sync_pattern(MateId(70));
    assert!(matches!(asm.instance(created[0]).unwrap().source, InstanceSource::MirroredPart { .. }));
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
    // Without remapping the mate loses its geometry; Replace instance keeps
    // it that way (live qa5 MT9: red, with live's message).
    let broken = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
    assert!(!broken.mates[0].ok);
    assert_eq!(broken.mates[0].message.as_deref(), Some("Mate cannot resolve mate connectors."));
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
    /// Keys `<version>/<tab>`: the same version, another tab.
    fn sibling(&self, key: &str, tab: TabId) -> Option<String> {
        Some(format!("{}/{tab}", key.split('/').next()?))
    }
}

#[test]
fn assemblies_from_other_documents() {
    // Other document: Block <2> fastened on top of the fixed Block <1>.
    let mut o = doc_with(vec![block_studio()]);
    let mut ev = AssemblyEvaluator::new();
    let (obody, or) = body_of(&mut ev, &o.doc, o.studios[0]);
    let sub = Assembly {
        instances: vec![part_instance(1, "Block <1>", o.studios[0], obody.clone(), DVec3::ZERO, true), part_instance(2, "Block <2>", o.studios[0], obody, DVec3::new(0.07, 0.03, -0.02), false)],
        mates: vec![mate(10, MateType::Fastened, conn(1, face_with(&or, "CapEnd"), Inference::Centroid), conn(2, face_with(&or, "CapStart"), Inference::Centroid))],
        ..Default::default()
    };
    let sub_tab = o.doc.add_tab("Assembly 1", TabContent::Assembly(Box::new(sub)));
    let odoc = std::sync::Arc::new(o.doc.clone());
    let ext = TestExternal { docs: vec![(format!("ext:v1/{sub_tab}"), odoc.clone(), sub_tab), (format!("ext:v1/{}", o.studios[0]), odoc, o.studios[0])] };
    ev.set_external(std::sync::Arc::new(ext));
    // This document: a fixed block and the other document's assembly 0.2 m along X.
    let mut f = doc_with(vec![block_studio()]);
    let (body, _) = body_of(&mut ev, &f.doc, f.studios[0]);
    let key = format!("ext:v1/{sub_tab}");
    let mut asm = Assembly { instances: vec![part_instance(1, "Block <1>", f.studios[0], body, DVec3::ZERO, true)], ..Default::default() };
    let at = Placement { translation: [0.2, 0.0, 0.0], rotation: [0.0, 0.0, 0.0, 1.0] };
    asm.instances.push(Instance::new(InstanceId(2), "Assembly 1 <1>".into(), InstanceSource::External { key: key.clone(), body: None, config: ConfigValues::default() }, at));
    let top_tab = f.doc.add_tab("Assembly 1", TabContent::Assembly(Box::new(asm.clone())));
    let res = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
    let node = &res.nodes[1];
    assert!(node.error.is_none(), "{:?}", node.error);
    assert_eq!(node.kind, eval::NodeKind::Assembly);
    assert_eq!(node.children.len(), 2);
    // Its parts sit where its own mates put them, carried by the instance.
    let top = res.occurrences.iter().find(|o| o.path == [InstanceId(2), InstanceId(2)]).expect("nested part");
    let (lo, _) = top.world_bounds().unwrap();
    assert!((lo.z - 0.01).abs() < 1e-6 && (lo.x - 0.2).abs() < 1e-6, "{lo:?}");
    assert!(top.external.as_deref().is_some_and(|k| k.starts_with("ext:v1/")), "{:?}", top.external);
    // Structured BOM: a subassembly row (its name there) with its parts.
    let settings = super::bom::BomSettings { bom_type: super::bom::BomType::Structured, ..Default::default() };
    let st = super::bom::lines(&f.doc, top_tab, &res, &settings);
    let summary: Vec<(String, usize, usize)> = st.iter().map(|l| (l.item.clone().unwrap_or_default(), l.depth, l.quantity)).collect();
    assert_eq!(summary, vec![("1".into(), 0, 1), ("2".into(), 0, 1), ("2.1".into(), 1, 2)]);
    assert_eq!(st[1].name, "Assembly 1");
    assert!(matches!(st[1].source, super::bom::BomSource::ExternalAssembly { .. }));
    // Reference updates re-point the whole subassembly; a missing version is reported.
    assert_eq!(asm.update_external(&key, "ext:gone"), 1);
    let res = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
    assert!(res.nodes[1].error.is_some());
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
            Instance { id: InstanceId(2), name: "Part 1 <1>".into(), source: InstanceSource::External { key: "ext:v1".into(), body: Some(ebody), config: ConfigValues::default() }, placement: Placement { translation: [0.1, 0.0, 0.0], rotation: [0.0, 0.0, 0.0, 1.0] }, fixed: false, hidden: false, suppressed: false, lock: SubassemblyLock::Flexible, nested: Vec::new(), best_tessellation: false, reference_designator: None, unsuppressed_if: None },
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
fn inertia_moves_to_a_reference_frame() {
    use super::solver::Frame;
    // A 2 kg point-like body at (1, 2, 0) with no inertia of its own: about
    // the origin Ixx = m·y² = 8, Iyy = m·x² = 2, Izz = m·(x²+y²) = 10, Ixy = −m·x·y = −4.
    let i = analysis::inertia_about_frame(glam::DMat3::ZERO, 2.0, DVec3::new(1.0, 2.0, 0.0), &Frame { origin: DVec3::ZERO, rotation: DQuat::IDENTITY });
    assert!((i.x_axis.x - 8.0).abs() < 1e-12 && (i.y_axis.y - 2.0).abs() < 1e-12 && (i.z_axis.z - 10.0).abs() < 1e-12 && (i.y_axis.x + 4.0).abs() < 1e-12, "{i:?}");
    // A frame turned 90° about Z swaps the x and y moments.
    let j = analysis::inertia_about_frame(glam::DMat3::ZERO, 2.0, DVec3::new(1.0, 2.0, 0.0), &Frame { origin: DVec3::ZERO, rotation: DQuat::from_rotation_z(std::f64::consts::FRAC_PI_2) });
    assert!((j.x_axis.x - 2.0).abs() < 1e-9 && (j.y_axis.y - 8.0).abs() < 1e-9, "{j:?}");
}

#[test]
fn face_section_properties_like_live() {
    // Live (behaviours "Face tab"): the 40×20 face of a 40×20×10 block gives
    // 800 mm², 120 mm, Lxx 26666.667, Lyy 106666.667, Lzz 133333.333 mm⁴.
    let f = doc_with(vec![block_studio()]);
    let mut ev = AssemblyEvaluator::new();
    let (body, r) = body_of(&mut ev, &f.doc, f.studios[0]);
    let asm = Assembly { instances: vec![part_instance(1, "Block <1>", f.studios[0], body.clone(), DVec3::ZERO, true), part_instance(2, "Block <2>", f.studios[0], body, DVec3::new(0.0, 0.0, 0.05), false)], ..Default::default() };
    let res = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
    let top = face_with(&r, "CapEnd");
    let s = analysis::face_section_properties(&res, &[(vec![InstanceId(1)], top.clone())]).unwrap();
    let mm4 = 1e-12;
    assert!((s.area - 8e-4).abs() < 1e-10 && (s.perimeter - 0.12).abs() < 1e-9, "{s:?}");
    let mut m = [s.moments[0][0] / mm4, s.moments[1][1] / mm4];
    m.sort_by(f64::total_cmp);
    // The mesh is f32: compare to 1e-5 relative.
    let near = |x: f64, want: f64| (x - want).abs() < want * 1e-5;
    assert!(near(m[0], 26666.667) && near(m[1], 106666.667), "{m:?}");
    assert!(near(s.polar / mm4, 133333.333) && (s.moments[0][1] / mm4).abs() < 1e-2, "{} {}", s.polar / mm4, s.moments[0][1] / mm4);
    assert!((s.centroid - DVec3::new(0.02, 0.01, 0.01)).length() < 1e-9);
    // The same face of the raised block is parallel but not coplanar: no values.
    assert!(analysis::face_section_properties(&res, &[(vec![InstanceId(1)], top.clone()), (vec![InstanceId(2)], top)]).is_none());
}

#[test]
fn torus_face_inferences_lie_on_the_axis() {
    use super::eval::inference_candidates;
    // A fillet ring (pin with a 2 mm fillet on its top edge): the centers of
    // its two circular edges, Z along the pin's axis, never sideways.
    let filleted = with_feature(pin_studio(), "fillet", |f, r| {
        f.params.set("entities", ParamValue::Query(edges_with(r, "CapEnd")));
        f.params.set("radius", ParamValue::Expr(ExprValue::new("2 mm")));
    });
    let f = doc_with(vec![filleted]);
    let mut ev = AssemblyEvaluator::new();
    let (pin, r) = body_of(&mut ev, &f.doc, f.studios[0]);
    let torus = face_of_kind(&r, |s| matches!(s, noshape_kernel::Surface::Torus { .. }));
    let pts = inference_candidates(&r.context, Some(&pin), &torus, &UnitPrefs::default());
    assert_eq!(pts.len(), 2, "{:#?}", pts.iter().map(|p| p.frame.origin).collect::<Vec<_>>());
    assert!(pts.iter().all(|p| p.frame.z().cross(DVec3::Z).length() < 1e-9 && p.frame.origin.truncate().length() < 1e-9), "{:#?}", pts.iter().map(|p| (p.frame.origin, p.frame.z())).collect::<Vec<_>>());
}

#[test]
fn mate_reset_and_apply_limit_position() {
    let f = doc_with(vec![block_studio()]);
    let mut ev = AssemblyEvaluator::new();
    let (body, r) = body_of(&mut ev, &f.doc, f.studios[0]);
    let top = face_with(&r, "CapEnd");
    let bottom = face_with(&r, "CapStart");
    let mut m = mate(10, MateType::Slider, conn(1, top, Inference::Centroid), conn(2, bottom, Inference::Centroid));
    // Live's sense (oracle_asm S1): B above A reads negative.
    m.limits = vec![("z".into(), Limits { min: Some(ExprValue::new("-20 mm")), max: Some(ExprValue::new("0 mm")) })];
    let asm = Assembly {
        instances: vec![part_instance(1, "Block <1>", f.studios[0], body.clone(), DVec3::ZERO, true), part_instance(2, "Block <2>", f.studios[0], body, DVec3::new(0.0, 0.0, 0.025), false)],
        mates: vec![m],
        ..Default::default()
    };
    let res = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
    // Apply limit position > Min Z: the slider value goes to -20 mm (20 mm up).
    let mut v = res.solved.mate_values[&MateId(10)];
    v[3] = -0.02;
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
    // A new Fastened mate rotates about X (live, qa5 MT6); about Z the block
    // turns in the face plane, its Z stays up.
    assert_eq!(m.rotation_axis, 0);
    m.rotation_axis = 2;
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
    let sub_tab = f.doc.add_tab("Assembly 2", TabContent::Assembly(Box::default()));
    let (sub, sub_id) = parent.move_to_new_subassembly(&[InstanceId(2), InstanceId(3)], sub_tab, "Assembly 2");
    assert_eq!(parent.instances.len(), 2);
    assert_eq!(parent.instance(sub_id).unwrap().name, "Assembly 2 <1>");
    assert_eq!(sub.instances.len(), 2);
    assert_eq!(sub.mates.len(), 1, "the revolute between the moved blocks moves along");
    let m = parent.mate(MateId(10)).unwrap();
    assert_eq!((m.b.instance, m.b.path.clone()), (sub_id, vec![InstanceId(2)]), "parent mate reaches into the subassembly");
    if let Some(t) = f.doc.tabs.iter_mut().find(|t| t.id == sub_tab) {
        t.content = TabContent::Assembly(Box::new(sub.clone()));
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

/// ASM-PATTERN-SUBASSEMBLY (live request 1564): a pattern whose seed and
/// direction move to a new subassembly goes along with its copies and stays
/// healthy; Dissolve brings it back to the top level, still healthy. A
/// pattern whose direction stays behind isn't dependent.
#[test]
fn move_to_new_subassembly_takes_a_dependent_pattern() {
    let mut f = doc_with(vec![block_studio()]);
    let mut ev = AssemblyEvaluator::new();
    let (body, r) = body_of(&mut ev, &f.doc, f.studios[0]);
    let top = face_with(&r, "CapEnd");
    let bottom = face_with(&r, "CapStart");
    let b = &r.context.bodies[0];
    let edge = b
        .edges
        .iter()
        .enumerate()
        .find_map(|(i, n)| match b.shape.sub_shape(noshape_kernel::ShapeType::Edge, i).ok()?.edge_info().ok()?.curve {
            noshape_kernel::Curve::Line { direction, .. } if direction.x.abs() > 0.99 => Some(Reference::Edge((**n).clone())),
            _ => None,
        })
        .unwrap();
    let mut asm = Assembly {
        instances: vec![part_instance(1, "Base <1>", f.studios[0], body.clone(), DVec3::ZERO, true), part_instance(2, "Block <1>", f.studios[0], body.clone(), DVec3::new(0.0, 0.1, 0.0), false)],
        mates: vec![mate(10, MateType::Fastened, conn(1, top, Inference::Centroid), conn(2, bottom, Inference::Centroid))],
        ..Default::default()
    };
    asm.patterns.push(InstancePattern {
        id: MateId(70),
        name: "Linear pattern 1".into(),
        kind: PatternKind::Linear { direction: conn(1, edge, Inference::Midpoint).linear_direction(), distance: ExprValue::new("25 mm"), count: 3, opposite: false, equal_spacing: false, second: None, centered: false },
        seeds: vec![InstanceId(2)],
        generated: vec![],
        suppressed: false,
    });
    asm.sync_pattern(MateId(70));
    assert!(asm.dependent_patterns(&[InstanceId(2)]).is_empty(), "the direction's part stays: not dependent");
    assert_eq!(asm.dependent_patterns(&[InstanceId(1), InstanceId(2)]), vec![MateId(70)]);
    let world = |res: &eval::AssemblyResult| -> Vec<DVec3> {
        let mut v: Vec<DVec3> = res.occurrences.iter().map(|o| o.transform.transform_point3(DVec3::new(0.01, 0.005, 0.0))).collect();
        v.sort_by(|a, b| (a.x, a.y, a.z).partial_cmp(&(b.x, b.y, b.z)).unwrap());
        v
    };
    let before = world(&ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None));
    assert_eq!(before.len(), 4);
    let mut parent = asm.clone();
    let sub_tab = f.doc.add_tab("Assembly 2", TabContent::Assembly(Box::default()));
    let (sub, sub_id) = parent.move_to_new_subassembly(&[InstanceId(1), InstanceId(2)], sub_tab, "Assembly 2");
    assert!(parent.patterns.is_empty() && parent.mates.is_empty(), "pattern and mate moved");
    assert_eq!(parent.instances.len(), 1);
    assert_eq!((sub.instances.len(), sub.patterns.len(), sub.mates.len()), (4, 1, 1), "seed, base and both copies inside");
    if let Some(t) = f.doc.tabs.iter_mut().find(|t| t.id == sub_tab) {
        t.content = TabContent::Assembly(Box::new(sub.clone()));
    }
    let inner = ev.evaluate(&f.doc, &sub, &SolveOptions::default(), None);
    assert!(inner.pattern_errors.is_empty(), "{:?}", inner.pattern_errors);
    let res = ev.evaluate(&f.doc, &parent, &SolveOptions::default(), None);
    assert_eq!(world(&res).len(), 4);
    for (a, b) in before.iter().zip(&world(&res)) {
        assert!(a.distance(*b) < 1e-6, "moved: {a:?} -> {b:?}");
    }
    let ids = parent.dissolve_subassembly(sub_id, &sub);
    assert_eq!((ids.len(), parent.patterns.len(), parent.mates.len()), (4, 1, 1));
    let p = &parent.patterns[0];
    assert!(ids.contains(&p.seeds[0]) && p.generated.iter().all(|g| ids.contains(&g.instance) && g.seed == p.seeds[0]));
    let res = ev.evaluate(&f.doc, &parent, &SolveOptions::default(), None);
    assert!(res.pattern_errors.is_empty(), "{:?}", res.pattern_errors);
    assert_eq!(world(&res).len(), 4);
    for (a, b) in before.iter().zip(&world(&res)) {
        assert!(a.distance(*b) < 1e-6, "dissolve moved: {a:?} -> {b:?}");
    }
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
    assert_eq!(asm.folders[0].name, "New folder");
    assert_eq!(asm.folders[0].instances, vec![InstanceId(2), InstanceId(3)]);
    assert_eq!(asm.folder_of_instance(InstanceId(3)).map(|f| f.id), Some(fid));
    assert!(asm.next_id() > fid.0, "folder ids are taken");
    // Moving an instance into another folder takes it out of the first.
    let f2 = asm.add_folder(&[InstanceId(3)], &[]);
    assert_eq!(asm.folders.iter().find(|f| f.id == f2).map(|f| f.name.as_str()), Some("New folder"));
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
        // Mate values in live's sense: 30 mm up reads -30 mm.
        named_positions: vec![NamedPosition { name: "Up".into(), placements: vec![], mate_values: vec![(MateId(10), [0.0, 0.0, 0.0, -0.03])] }],
        ..Default::default()
    };
    let sub_tab = f.doc.add_tab("Sub", TabContent::Assembly(Box::new(sub)));
    let mut sub_inst = Instance::new(InstanceId(5), "Sub <1>".into(), InstanceSource::Assembly { assembly: sub_tab, config: ConfigValues::default() }, Placement { translation: [0.0, 0.0, 0.01], rotation: [0.0, 0.0, 0.0, 1.0] });
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
    let sub_tab = f.doc.add_tab("Sub", TabContent::Assembly(Box::new(sub_content.clone())));
    let mut asm = Assembly {
        instances: vec![
            part_instance(1, "Base <1>", f.studios[0], body.clone(), DVec3::ZERO, true),
            Instance::new(InstanceId(5), "Sub <1>".into(), InstanceSource::Assembly { assembly: sub_tab, config: ConfigValues::default() }, Placement { translation: [0.1, 0.0, 0.0], rotation: [0.0, 0.0, 0.0, 1.0] }),
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
        t.content = TabContent::Assembly(Box::new(sub));
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
    let m = analysis::measure_entities(&res, &[(vec![InstanceId(2)], top.clone())]);
    assert!(m.iter().any(|(l, m)| l == "Area" && matches!(m, Measurement::Area(a) if (a - 0.0008).abs() < 1e-9)));
    // A pick on an instance that is gone after re-evaluation no longer exists
    // (qa4 round 5 s4: the readout kept "Area" after the part was undone).
    assert!(analysis::pick_exists(&res, &(vec![InstanceId(2)], top.clone())));
    let one = Assembly { instances: asm.instances[..1].to_vec(), ..Default::default() };
    let res = ev.evaluate(&f.doc, &one, &SolveOptions::default(), None);
    assert!(!analysis::pick_exists(&res, &(vec![InstanceId(2)], top.clone())));
    assert!(analysis::pick_exists(&res, &(vec![InstanceId(1)], top)));
}

#[test]
#[ignore = "T2: standard content vs live (P3 long tail; slow)"]
fn standard_content_parts_regenerate_with_mate_connectors() {
    use super::standard::{self, CATALOGUE, StandardSpec};
    // Every component, reporting all that fail at once.
    let mut failures: Vec<String> = Vec::new();
    for c in CATALOGUE.iter() {
        let spec = StandardSpec::new(c);
        let name = format!("{} › {}", c.standard, spec.part_name());
        let r = match standard::regenerate(&spec) {
            Ok(r) => r,
            Err(e) => {
                let params = standard::part_studio(&spec).map(|ps| format!("{:?}", ps.features[0].params)).unwrap_or_default();
                failures.push(format!("{name}: {e}\n    {params}"));
                continue;
            }
        };
        let failed: Vec<_> = r.display.features.iter().filter_map(|f| f.message.clone()).collect();
        if !failed.is_empty() {
            failures.push(format!("{name}: {failed:?}"));
            continue;
        }
        let body = r.context.body(&standard::part_body(&r).unwrap()).unwrap();
        let v = match body.shape.volume() {
            Ok(v) => v,
            Err(e) => {
                failures.push(format!("{name}: volume: {e:?}"));
                continue;
            }
        };
        let d = c.diameter_m(c.size(&spec.size).unwrap());
        // (Retaining rings are thin: live's DIN 472 #28 is 0.01 d³.)
        let floor = if c.shape == standard::Shape::RetainingRing { 0.005 } else { 0.05 };
        if v <= d * d * d * floor {
            failures.push(format!("{name}: volume {v}"));
        }
        // Bolts: one connector under the head; nuts and washers: both faces;
        // retaining rings: live's one mid-plane connector.
        let want = if c.shape.has_length() || c.shape == standard::Shape::RetainingRing || c.near_only.is_some() { 1 } else { 2 };
        if r.context.connectors.len() != want {
            failures.push(format!("{name}: {} connectors", r.context.connectors.len()));
            continue;
        }
        // At the origin, the primary axis out of the contact face (mates oppose Z axes).
        let z = if c.near_only == Some(false) { DVec3::Z } else { -DVec3::Z };
        if r.context.connectors[0].origin.length() > 1e-9 || (r.context.connectors[0].z - z).length() > 1e-9 {
            failures.push(format!("{name}: first connector misplaced"));
        }
    }
    assert!(failures.is_empty(), "{} of {} components fail:\n{}", failures.len(), CATALOGUE.len(), failures.join("\n"));
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
    // Live: a component opens on its first size and that size's first length
    // (ISO 4762: M1.6 x 2.5), metric lengths written bare.
    assert_eq!((spec.size.as_str(), spec.length.as_deref()), ("M1.6", Some("2.5")));
    spec.set_size("M6");
    assert_eq!(spec.length.as_deref(), Some("6"), "live's M6 list starts at 6");
    spec.length = Some("25".into());
    assert_eq!(spec.part_name(), "Hex socket head cap screw M6x1.00 x 25");
    // Live's washer: 3/8, Series Narrow / Wide in the name and Description.
    let washer_a = CATALOGUE.iter().find(|c| c.name == "Type A flat washer").unwrap();
    let mut w = StandardSpec::new(washer_a);
    assert_eq!(w.description(), "Type A flat washer 3/8 Narrow Stainless Steel ");
    w.params.insert("Series".into(), "Wide".into());
    assert_eq!(w.part_name(), "Type A flat washer 3/8 Wide");
    assert!(standard::regenerate(&w).is_ok());
    // Live's ANSI default: "Countersunk Bolt #10-24 x 0.25", the Description adds the material.
    let bolt = StandardSpec::new(standard::default_component());
    assert_eq!(bolt.part_name(), "Countersunk Bolt #10-24 x 0.25");
    assert_eq!(bolt.description(), "Countersunk Bolt #10-24 x 0.25 Stainless Steel ");
    let mut long = spec.clone();
    long.length = Some("40".into());
    // Documents saved with "40 mm" load as live's "40".
    let old: StandardSpec = serde_json::from_str(&serde_json::to_string(&StandardSpec { length: Some("40 mm".into()), ..long.clone() }).unwrap()).unwrap();
    assert_eq!(old.length.as_deref(), Some("40"));
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

/// Typed triad values (Transform) on a Planar-mated part are kept exactly
/// (live, flip asm: a pin Planar-mated under a fixed plate, X typed
/// 23.456789 mm → x 0.023456789000000016 m, z −30 mm and the rotation
/// untouched; 5 mm typed along the mated Z → nothing moves).
#[test]
fn typed_transform_on_a_planar_mate_is_exact() {
    let f = doc_with(vec![block_studio()]);
    let mut ev = AssemblyEvaluator::new();
    let (body, r) = body_of(&mut ev, &f.doc, f.studios[0]);
    let top = face_with(&r, "CapEnd");
    let bottom = face_with(&r, "CapStart");
    let mut asm = Assembly { instances: vec![part_instance(1, "Block <1>", f.studios[0], body.clone(), DVec3::ZERO, true), part_instance(2, "Block <2>", f.studios[0], body, DVec3::new(0.08, 0.0, 0.0), false)], ..Default::default() };
    let on = |i: u64, r: &Reference| MateConnectorRef::on(InstanceId(i), vec![], r.clone());
    asm.mates.push(Mate::new(MateId(10), "Planar 1".into(), MateType::Planar, on(2, &bottom), on(1, &top)));
    let res = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
    let p0 = res.solved.placement(InstanceId(2)).unwrap();
    assert!((p0.translation[2] - 0.01).abs() < 1e-12, "{p0:?}");
    let start: std::collections::HashMap<InstanceId, Placement> = res.solved.placements.iter().cloned().collect();
    for (along, want) in [(DVec3::X, [p0.translation[0] + 0.023456789, p0.translation[1], p0.translation[2]]), (DVec3::Z, p0.translation)] {
        let moved = Placement { translation: (DVec3::from(p0.translation) + along * 0.023456789).into(), rotation: p0.rotation };
        let s = res.prepared.solve(&SolveOptions { targets: vec![(InstanceId(2), moved)], start: Some(start.clone()), ..Default::default() });
        let p = s.placement(InstanceId(2)).unwrap();
        for k in 0..3 {
            assert!((p.translation[k] - want[k]).abs() < 1e-15, "{along}: {:?} want {want:?}", p.translation);
        }
        assert!(quat_of(&p).angle_between(quat_of(&p0)) < 1e-14, "{along}: turned");
    }
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

#[test]
fn sketch_instances_are_wire_occurrences_with_edges() {
    let f = doc_with(vec![block_studio()]);
    let mut ev = AssemblyEvaluator::new();
    let r = ev.studio(&f.doc, f.studios[0], &ConfigValues::default()).unwrap();
    let sketch = r.context.sketches[0].feature;
    let asm = Assembly {
        instances: vec![Instance::new(InstanceId(1), "Sketch 1 <1>".into(), InstanceSource::Sketch { studio: f.studios[0], sketch, config: ConfigValues::default() }, Placement { translation: [0.1, 0.0, 0.0], rotation: [0.0, 0.0, 0.0, 1.0] })],
        ..Default::default()
    };
    let res = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
    assert!(res.nodes[0].error.is_none(), "{:?}", res.nodes[0].error);
    let occ = &res.occurrences[0];
    assert_eq!(occ.kind, crate::display::BodyKind::Wire);
    assert_eq!(occ.edge_refs.len(), 4, "the rectangle's four lines");
    assert_eq!(occ.mesh.edges.len(), 4, "drawable edge polylines");
    // The region fill (live assembly/123), one pickable face per region
    // (live 601).
    assert!(!occ.mesh.indices.is_empty(), "the sketch region is filled");
    assert_eq!(occ.mesh.face_ranges.len(), 1);
    assert!(matches!(occ.face_refs.as_slice(), [Reference::SketchRegion { .. }]), "{:?}", occ.face_refs);
    // Hovering inside the loop: the region centroid plus the rectangle's
    // 4 corners and 4 edge midpoints, all with Z along the sketch normal.
    let units = UnitPrefs::default();
    let pts = eval::inference_candidates(&occ.context, Some(&occ.body), &occ.face_refs[0], &units);
    assert_eq!(pts.len(), 9, "{pts:?}");
    let normal = r.context.sketches[0].frame.normal;
    assert!(pts.iter().all(|p| (p.frame.rotation * DVec3::Z).dot(normal) > 1.0 - 1e-6));
    // The centroid connector resolves for a mate.
    let mut asm3 = asm.clone();
    asm3.instances[0].fixed = true;
    asm3.connectors.push(crate::assembly::AsmMateConnector::new(MateId(6), "Mate connector 2".into(), pts[0].connector(&[InstanceId(1)])));
    let res3 = ev.evaluate(&f.doc, &asm3, &SolveOptions::default(), None);
    assert!(res3.connectors.iter().find(|c| c.id == MateId(6)).is_some_and(|c| c.local.is_some()), "the region centroid resolves");
    // Placed with its instance.
    let p = occ.transform.transform_point3(DVec3::ZERO);
    assert!((p.x - 0.1).abs() < 1e-9);
    // A mate connector on one of its edges resolves.
    let c = conn(1, occ.edge_refs[0].clone(), Inference::Midpoint);
    let mut asm2 = asm.clone();
    asm2.instances[0].fixed = true;
    asm2.connectors.push(crate::assembly::AsmMateConnector::new(MateId(5), "Mate connector 1".into(), c));
    let res = ev.evaluate(&f.doc, &asm2, &SolveOptions::default(), None);
    let mc = res.connectors.iter().find(|c| c.id == MateId(5)).expect("connector status");
    assert!(mc.local.is_some(), "the connector on the sketch edge resolves");
}

#[test]
fn dynamic_suppression_suppresses_while_the_expression_is_false() {
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
    let count = |ev: &mut AssemblyEvaluator, asm: &Assembly| ev.evaluate(&f.doc, asm, &SolveOptions::default(), None).occurrences.len();
    assert_eq!(count(&mut ev, &asm), 2);
    asm.instances[1].unsuppressed_if = Some("1 > 2".into());
    assert_eq!(count(&mut ev, &asm), 1, "false expression suppresses the instance");
    asm.instances[1].unsuppressed_if = Some("3 > 2".into());
    assert_eq!(count(&mut ev, &asm), 2);
    // An expression that doesn't evaluate leaves the instance as it is.
    asm.instances[1].unsuppressed_if = Some("#nope".into());
    assert_eq!(count(&mut ev, &asm), 2);
}

#[test]
fn dynamic_suppression_of_a_folder_suppresses_its_instances() {
    let f = doc_with(vec![pin_studio()]);
    let mut ev = AssemblyEvaluator::new();
    let (pin, _) = body_of(&mut ev, &f.doc, f.studios[0]);
    let mut asm = Assembly {
        instances: vec![
            part_instance(1, "Pin <1>", f.studios[0], pin.clone(), DVec3::ZERO, true),
            part_instance(2, "Pin <2>", f.studios[0], pin.clone(), DVec3::new(0.05, 0.0, 0.0), false),
            part_instance(3, "Pin <3>", f.studios[0], pin, DVec3::new(0.1, 0.0, 0.0), false),
        ],
        folders: vec![super::ListFolder { id: MateId(20), name: "New folder".into(), instances: vec![InstanceId(2), InstanceId(3)], features: Vec::new(), unsuppressed_if: None }],
        ..Default::default()
    };
    let count = |ev: &mut AssemblyEvaluator, asm: &Assembly| ev.evaluate(&f.doc, asm, &SolveOptions::default(), None).occurrences.len();
    assert_eq!(count(&mut ev, &asm), 3);
    asm.folders[0].unsuppressed_if = Some("1 > 2".into());
    assert_eq!(count(&mut ev, &asm), 1, "a false expression suppresses the folder's instances");
    asm.folders[0].unsuppressed_if = Some("3 > 2".into());
    assert_eq!(count(&mut ev, &asm), 3);
}

#[test]
fn dynamic_suppression_applies_to_mates() {
    let f = doc_with(vec![block_studio()]);
    let mut ev = AssemblyEvaluator::new();
    let (body, r) = body_of(&mut ev, &f.doc, f.studios[0]);
    let top = face_with(&r, "CapEnd");
    let bottom = face_with(&r, "CapStart");
    let mut asm = Assembly {
        instances: vec![
            part_instance(1, "Block <1>", f.studios[0], body.clone(), DVec3::ZERO, true),
            part_instance(2, "Block <2>", f.studios[0], body.clone(), DVec3::new(0.07, 0.03, -0.02), false),
        ],
        mates: vec![mate(10, MateType::Fastened, conn(1, top, Inference::Centroid), conn(2, bottom, Inference::Centroid))],
        ..Default::default()
    };
    let inactive = |ev: &mut AssemblyEvaluator, asm: &Assembly| ev.evaluate(&f.doc, asm, &SolveOptions::default(), None).mates[0].inactive;
    assert!(!inactive(&mut ev, &asm));
    asm.mates[0].unsuppressed_if = Some("0".into());
    assert!(inactive(&mut ev, &asm), "a false expression suppresses the mate");
    asm.mates[0].unsuppressed_if = Some("1".into());
    assert!(!inactive(&mut ev, &asm));
}

#[test]
fn bom_flattened_structured_exclusion_and_cell_edits() {
    use super::bom::{self, BomColumn, BomSettings, BomType};
    let mut f = doc_with(vec![block_studio(), pin_studio()]);
    let mut ev = AssemblyEvaluator::new();
    let (block, _) = body_of(&mut ev, &f.doc, f.studios[0]);
    let (pin, _) = body_of(&mut ev, &f.doc, f.studios[1]);
    // Subassembly: two pins.
    let sub = Assembly {
        instances: vec![
            part_instance(1, "Pin <1>", f.studios[1], pin.clone(), DVec3::ZERO, true),
            part_instance(2, "Pin <2>", f.studios[1], pin.clone(), DVec3::new(0.05, 0.0, 0.0), true),
        ],
        ..Default::default()
    };
    let sub_tab = f.doc.add_tab("Assembly 2", TabContent::Assembly(Box::new(sub)));
    let mut sub_inst = part_instance(5, "Assembly 2 <1>", f.studios[0], block.clone(), DVec3::new(0.2, 0.0, 0.0), true);
    sub_inst.source = InstanceSource::Assembly { assembly: sub_tab, config: ConfigValues::default() };
    let top = Assembly {
        instances: vec![
            part_instance(1, "Block <1>", f.studios[0], block.clone(), DVec3::ZERO, true),
            part_instance(2, "Block <2>", f.studios[0], block.clone(), DVec3::new(0.1, 0.0, 0.0), true),
            sub_inst,
        ],
        ..Default::default()
    };
    let top_tab = f.doc.add_tab("Assembly 1", TabContent::Assembly(Box::new(top.clone())));
    let res = ev.evaluate(&f.doc, &top, &SolveOptions::default(), None);
    let mut s = BomSettings::default();
    assert_eq!(s.columns, BomColumn::DEFAULT.to_vec(), "Item | Quantity | Part number | Description");

    // Flattened: block x2, pin x2 (the subassembly's parts).
    let flat = bom::lines(&f.doc, top_tab, &res, &s);
    let summary: Vec<(Option<String>, usize)> = flat.iter().map(|l| (l.item.clone(), l.quantity)).collect();
    assert_eq!(summary, vec![(Some("1".into()), 2), (Some("2".into()), 2)]);
    assert!(matches!(flat[1].source, bom::BomSource::Part { studio, .. } if studio == f.studios[1]));

    // Structured: block x2, Assembly 2 x1 with 2.1 pins x2.
    s.bom_type = BomType::Structured;
    let st = bom::lines(&f.doc, top_tab, &res, &s);
    let summary: Vec<(String, usize, usize)> = st.iter().map(|l| (l.item.clone().unwrap(), l.depth, l.quantity)).collect();
    assert_eq!(summary, vec![("1".into(), 0, 2), ("2".into(), 0, 1), ("2.1".into(), 1, 2)]);
    assert_eq!(st[1].name, "Assembly 2");
    assert!(st[1].has_children);

    // Subassembly BOM behavior "Show Assembly only": one item in both types.
    if let Some(TabContent::Assembly(a)) = f.doc.tab_mut(sub_tab).map(|t| &mut t.content) {
        a.properties.bom_behavior = BomBehavior::AssemblyOnly;
    }
    assert_eq!(bom::lines(&f.doc, top_tab, &res, &s).len(), 2);
    s.bom_type = BomType::Flattened;
    let flat = bom::lines(&f.doc, top_tab, &res, &s);
    assert!(matches!(flat[1].source, bom::BomSource::Assembly(t) if t == sub_tab));
    if let Some(TabContent::Assembly(a)) = f.doc.tab_mut(sub_tab).map(|t| &mut t.content) {
        a.properties.bom_behavior = BomBehavior::AssemblyAndComponents;
    }

    // Editing a cell writes the part's Properties; the block row shows it.
    let flat = bom::lines(&f.doc, top_tab, &res, &s);
    assert!(bom::set_cell(&mut f.doc, &flat[0], BomColumn::PartNumber, "PRT-001"));
    assert!(!bom::set_cell(&mut f.doc, &flat[0], BomColumn::PartNumber, "PRT-001"), "unchanged");
    assert!(!bom::set_cell(&mut f.doc, &flat[0], BomColumn::Quantity, "5"), "read-only");
    assert_eq!(bom::cell(&f.doc, &res, &flat[0], BomColumn::PartNumber), bom::BomCell::Text("PRT-001".into()));
    assert_eq!(bom::cell(&f.doc, &res, &flat[0], BomColumn::UnitOfMeasure).text(), "Each");
    assert_eq!(bom::cell(&f.doc, &res, &flat[0], BomColumn::Mass).text(), "No value");
    assert_eq!(bom::cell(&f.doc, &res, &flat[0], BomColumn::Material).text(), "- None -");
    assert_eq!(bom::cell(&f.doc, &res, &flat[0], BomColumn::Appearance).text(), "Default");

    // Exclude from all BOMs: gone, listed without an item number with
    // Show excluded/suppressed.
    if let Some(TabContent::PartStudio(ps)) = f.doc.tab_mut(f.studios[1]).map(|t| &mut t.content) {
        ps.part_properties.entry(pin.clone()).or_default().meta.exclude_from_bom = true;
    }
    assert_eq!(bom::lines(&f.doc, top_tab, &res, &s).len(), 1);
    s.show_excluded = true;
    let with = bom::lines(&f.doc, top_tab, &res, &s);
    assert_eq!(with.len(), 2);
    assert!(with[1].excluded && with[1].item.as_deref() == Some("--"), "excluded rows: Item -- at the end");
    s.show_excluded = false;

    // Top level assembly row at the bottom, its name as the Item.
    s.show_top_level = true;
    let with = bom::lines(&f.doc, top_tab, &res, &s);
    let last = with.last().unwrap();
    assert!(last.top_level && last.item.as_deref() == Some("Assembly 1"));
    assert!(bom::table_text(&f.doc, &res, &with, &s, true).lines().last().unwrap().starts_with("Assembly1,,"));
    s.show_top_level = false;

    // Columns: add, move, remove; Item stays first.
    s.add_column(BomColumn::Name);
    assert_eq!(s.columns.last(), Some(&BomColumn::Name));
    s.move_column(BomColumn::Name, -1);
    assert_eq!(s.columns[3], BomColumn::Name);
    s.move_column(BomColumn::Quantity, -1);
    assert_eq!(s.columns[1], BomColumn::Quantity, "nothing moves before Item");
    s.remove_column(BomColumn::Item);
    assert_eq!(s.columns[0], BomColumn::Item);
    s.remove_column(BomColumn::Name);

    // CSV export: header plus one row per line, quoted when needed.
    let lines = bom::lines(&f.doc, top_tab, &res, &s);
    assert!(bom::set_cell(&mut f.doc, &lines[0], BomColumn::Description, "Block, 40 mm"));
    let csv = bom::table_text(&f.doc, &res, &lines, &s, true);
    let mut rows = csv.lines();
    assert_eq!(rows.next(), Some("Item,Quantity,Part number,Description"));
    assert_eq!(rows.next(), Some("1,2,PRT-001,\"Block, 40 mm\""));
    let tsv = bom::table_text(&f.doc, &res, &lines, &s, false);
    assert!(tsv.starts_with("Item\tQuantity\tPart number\tDescription\n1\t2\tPRT-001\tBlock, 40 mm\n"));
}

#[test]
fn assembly_configurations_drive_suppression_and_mate_offsets() {
    use super::config::{param, ConfigTarget};
    use crate::configuration::{ConfigInputKind, ConfigValues};
    let mut f = doc_with(vec![block_studio()]);
    let mut ev = AssemblyEvaluator::new();
    let (body, r) = body_of(&mut ev, &f.doc, f.studios[0]);
    let top = face_with(&r, "CapEnd");
    let bottom = face_with(&r, "CapStart");
    let mut asm = Assembly {
        instances: vec![
            part_instance(1, "Block <1>", f.studios[0], body.clone(), DVec3::ZERO, true),
            part_instance(2, "Block <2>", f.studios[0], body.clone(), DVec3::new(0.07, 0.03, -0.02), false),
            part_instance(3, "Block <3>", f.studios[0], body.clone(), DVec3::new(0.2, 0.0, 0.0), true),
        ],
        mates: vec![mate(10, MateType::Fastened, conn(1, top, Inference::Centroid), conn(2, bottom, Inference::Centroid))],
        ..Default::default()
    };
    // Configure Assembly: one List input "Configuration" with "Default".
    asm.configuration.inputs.push(super::config::new_list_input("in1".into(), 1));
    assert_eq!(asm.configuration.inputs[0].name, "Configuration");
    asm.add_config_row("in1", "Long", "long".into());
    let ConfigInputKind::List { options, .. } = &asm.configuration.inputs[0].kind else { panic!() };
    assert_eq!(options.iter().map(|o| o.name.as_str()).collect::<Vec<_>>(), ["Default", "Long"]);
    // Configure Revolute/Fastened 1 Unsuppressed + Offset, Block <3> suppression.
    assert!(asm.configure_param("in1", ConfigTarget::Mate(MateId(10)), param::UNSUPPRESSED));
    assert!(!asm.configure_param("in1", ConfigTarget::Mate(MateId(10)), param::UNSUPPRESSED), "once");
    assert!(asm.configure_param("in1", ConfigTarget::Mate(MateId(10)), param::OFFSET));
    assert!(asm.configure_param("in1", ConfigTarget::Mate(MateId(10)), param::OFFSET_Z));
    assert!(asm.configure_param("in1", ConfigTarget::Instance(InstanceId(3)), param::UNSUPPRESSED));
    // Every row starts at the current value.
    assert_eq!(asm.configuration.configured[0].values.get("long"), Some(&ParamValue::Bool(true)));
    // Long: the mate offset 10 mm, Block <3> suppressed.
    asm.set_config_cell(1, "long", ParamValue::Bool(true));
    asm.set_config_cell(2, "long", ParamValue::Expr(ExprValue::new("10 mm")));
    asm.set_config_cell(3, "long", ParamValue::Bool(false));
    let default = ConfigValues::default();
    let mut long = ConfigValues::default();
    long.0.insert("in1".into(), "long".into());
    let d = ev.evaluate(&f.doc, &asm.configured(&default), &SolveOptions::default(), None);
    let l = ev.evaluate(&f.doc, &asm.configured(&long), &SolveOptions::default(), None);
    assert_eq!(d.occurrences.len(), 3);
    assert_eq!(l.occurrences.len(), 2, "Block <3> suppressed in Long");
    let z = |res: &super::eval::AssemblyResult| res.occurrences.iter().find(|o| o.path == vec![InstanceId(2)]).unwrap().transform.w_axis.z;
    // The first connector is the fixed block's top: the second moves −10 mm.
    assert!((z(&d) - z(&l) - 0.01).abs() < 1e-6, "Long offsets the mate by 10 mm: {} vs {}", z(&l), z(&d));
    // Suppressing a configured feature changes only the current configuration.
    assert!(asm.set_configured_value(ConfigTarget::Mate(MateId(10)), param::UNSUPPRESSED, &long, ParamValue::Bool(false)));
    assert!(asm.configured(&long).mate(MateId(10)).unwrap().suppressed);
    assert!(!asm.configured(&default).mate(MateId(10)).unwrap().suppressed);
    // Editing a configured mate stores into the active row.
    let mut edited = asm.configured(&long).mate(MateId(10)).unwrap().clone();
    edited.offset_z = Some(ExprValue::new("12 mm"));
    asm.store_mate_edit(&mut edited, &long);
    assert_eq!(asm.configuration.configured[2].values.get("long"), Some(&ParamValue::Expr(ExprValue::new("12 mm"))));
    assert_eq!(edited.offset_z, None, "the mate itself keeps its own value");
    // A subassembly instance in a configuration.
    let sub_tab = f.doc.add_tab("Sub", TabContent::Assembly(Box::new(asm.clone())));
    let mut inst = part_instance(5, "Sub <1>", f.studios[0], body.clone(), DVec3::ZERO, true);
    inst.source = InstanceSource::Assembly { assembly: sub_tab, config: long.clone() };
    let parent = Assembly { instances: vec![inst], ..Default::default() };
    let res = ev.evaluate(&f.doc, &parent, &SolveOptions::default(), None);
    assert_eq!(res.occurrences.len(), 2, "the subassembly instance uses its Long configuration");
    // Delete the input: its columns go too.
    asm.delete_config_input("in1");
    assert!(asm.configuration.is_empty() && asm.configuration.configured.is_empty());
}

#[test]
fn edit_in_context_snapshot_is_referenced_by_part_studio_features() {
    use crate::display::FeatureStatus;
    use crate::in_context;
    let mut f = doc_with(vec![block_studio()]);
    let mut ev = AssemblyEvaluator::new();
    let (body, _) = body_of(&mut ev, &f.doc, f.studios[0]);
    let asm = Assembly {
        instances: vec![
            part_instance(1, "Block <1>", f.studios[0], body.clone(), DVec3::ZERO, true),
            part_instance(2, "Block <2>", f.studios[0], body.clone(), DVec3::new(0.1, 0.0, 0.0), true),
        ],
        ..Default::default()
    };
    let asm_tab = f.doc.add_tab("Assembly 1", TabContent::Assembly(Box::new(asm.clone())));
    let res = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
    let name = |p: &super::eval::InstancePath| asm.instance(p[0]).map(|i| i.name.clone()).unwrap_or_default();
    let c = in_context::snapshot(&res, asm_tab, 1, "Assembly 1 context 1", &vec![InstanceId(1)], &name).expect("snapshot");
    assert_eq!(c.bodies.len(), 1, "the other instance");
    assert_eq!(c.bodies[0].instance, "Block <2>");
    let bodies = c.regen_bodies();
    assert_eq!(bodies.len(), 1);
    let bb = bodies[0].shape.bounding_box(false).unwrap();
    assert!((bb.min.x - 0.1).abs() < 1e-6, "placed relative to the primary instance: {:?}", bb.min);
    // A plane offset from a face of the context body.
    let face = Reference::Face((*bodies[0].faces[0]).clone());
    let units = UnitPrefs::default();
    let Some(TabContent::PartStudio(ps)) = f.doc.tab_mut(f.studios[0]).map(|t| &mut t.content) else { panic!() };
    ps.contexts.push(c);
    ps.active_context = Some(1);
    let mut plane = ps.new_feature("cPlane", &units).unwrap();
    plane.params.set("entities", ParamValue::Query(vec![face]));
    plane.params.set("offset", ParamValue::Expr(ExprValue::new("0 mm")));
    let pid = plane.id;
    ps.features.push(plane);
    assert_eq!(in_context::referenced_contexts(&ps.features), vec![1]);
    let mut reg = Regenerator::new();
    let r = reg.regenerate(ps, &ConfigValues::default(), &units, &RegenOptions::default());
    let st = r.display.features.iter().find(|s| s.id == pid).unwrap();
    assert!(matches!(st.status, FeatureStatus::Ok), "{:?}", st.message);
    assert_eq!(r.context.bodies.len(), 1, "context bodies never stay in the Part Studio");
    assert!(r.context.bodies.iter().all(|b| !in_context::is_context_feature(b.id.feature)));
    // The shown context: its body joins the display (pickable, translucent).
    assert!(r.display.bodies.iter().all(|b| !in_context::is_context_feature(b.id.feature)), "not parts of the Part Studio");
    let shown = &r.display.context_bodies;
    assert_eq!(shown.len(), 1);
    assert!(shown[0].color[3] < 0.5 && !shown[0].face_refs.is_empty());
    ps.active_context = None;
    let r = reg.regenerate(ps, &ConfigValues::default(), &units, &RegenOptions::default());
    assert!(r.display.context_bodies.is_empty());
    let st = r.display.features.iter().find(|s| s.id == pid).unwrap();
    assert!(matches!(st.status, FeatureStatus::Ok), "the reference holds without the context shown");
}

/// A full revolve (about the sketch's Y axis, Front plane) of the region
/// built by `profile`, plus the axis line.
fn revolved_studio(profile: impl FnOnce(&mut noshape_sketch::Sketch)) -> PartStudio {
    let units = UnitPrefs::default();
    let mut ps = PartStudio::new();
    let mut sk = noshape_sketch::Sketch::new();
    profile(&mut sk);
    let axis = sk.add_line(DVec2::new(0.0, -0.05), DVec2::new(0.0, 0.05));
    let mut sketch = ps.new_feature("sketch", &units).unwrap();
    sketch.params.set("sketchPlane", ParamValue::Query(vec![Reference::Plane(DatumPlane::Front)]));
    sketch.params.set("sketch", ParamValue::Sketch(Box::new(SketchData { sketch: sk, ..Default::default() })));
    let sid = sketch.id;
    ps.features.push(sketch);
    let r = Regenerator::new().regenerate(&ps, &ConfigValues::default(), &units, &RegenOptions::default());
    let key = r.display.sketches[0].regions.iter().max_by(|a, b| a.area.partial_cmp(&b.area).unwrap()).unwrap().key.clone();
    let mut f = ps.new_feature("revolve", &units).unwrap();
    f.params.set("entities", ParamValue::Query(vec![Reference::SketchRegion { sketch: sid, key }]));
    f.params.set("axis", ParamValue::Query(vec![Reference::SketchEntity { sketch: sid, entity: axis.0 }]));
    ps.features.push(f);
    ps
}

#[test]
fn inference_points_on_cones_spheres_arcs_and_vertices() {
    use super::eval::inference_candidates;
    use noshape_kernel::{Surface, ShapeType};
    // Truncated cone: radius 10 mm at the bottom, 5 mm at 20 mm up, about
    // Y; the virtual sharp (apex) is 40 mm up, outside the part.
    let cone = revolved_studio(|sk| {
        let p = [DVec2::new(0.0, 0.0), DVec2::new(0.01, 0.0), DVec2::new(0.005, 0.02), DVec2::new(0.0, 0.02)];
        let ids: Vec<_> = p.iter().map(|q| sk.add_point(*q)).collect();
        for i in 0..4 {
            sk.add_line_between(ids[i], ids[(i + 1) % 4]).unwrap();
        }
    });
    // Sphere: half disc of radius 10 mm about Y.
    let sphere = revolved_studio(|sk| {
        sk.add_arc(DVec2::ZERO, DVec2::new(0.0, -0.01), DVec2::new(0.0, 0.01));
    });
    let f = doc_with(vec![cone, sphere, crate::demo_fab::revolve_studio(), block_studio()]);
    let mut ev = AssemblyEvaluator::new();
    let units = UnitPrefs::default();
    let face_of = |r: &crate::regen::RegenResult, want: fn(&Surface) -> bool| -> (BodyId, Reference) {
        for b in &r.context.bodies {
            for (i, n) in b.faces.iter().enumerate() {
                if b.shape.sub_shape(ShapeType::Face, i).ok().and_then(|s| s.face_surface().ok()).is_some_and(|k| want(&k)) {
                    return (b.id.clone(), Reference::Face((**n).clone()));
                }
            }
        }
        panic!("no such face")
    };
    // N4: conical face — bottom, middle, top of the axis and the virtual sharp (apex).
    let (cb, rc) = body_of(&mut ev, &f.doc, f.studios[0]);
    let (_, cone_face) = face_of(&rc, |k| matches!(k, Surface::Cone { .. }));
    let pts = inference_candidates(&rc.context, Some(&cb), &cone_face, &units);
    assert_eq!(pts.len(), 4, "{:?}", pts.iter().map(|p| p.frame.origin).collect::<Vec<_>>());
    assert!(pts.iter().all(|p| p.frame.z().cross(DVec3::Z).length() < 1e-6), "Z along the cone axis");
    assert!(pts.iter().any(|p| (p.frame.origin - DVec3::new(0.0, 0.0, 0.04)).length() < 1e-6), "virtual sharp at the apex: {:?}", pts.iter().map(|p| p.frame.origin).collect::<Vec<_>>());
    // N5: spherical face — its center.
    let (sb, rs) = body_of(&mut ev, &f.doc, f.studios[1]);
    let (_, sphere_face) = face_of(&rs, |k| matches!(k, Surface::Sphere { .. }));
    let pts = inference_candidates(&rs.context, Some(&sb), &sphere_face, &units);
    assert_eq!(pts.len(), 1);
    assert!(pts[0].frame.origin.length() < 1e-6, "sphere center at the origin");
    // N8: an open arc edge — center, midpoint and both ends.
    let (ab, ra) = body_of(&mut ev, &f.doc, f.studios[2]);
    let b = ra.context.bodies.iter().find(|b| b.id == ab).unwrap();
    let arc = (0..b.edges.len())
        .find(|i| b.shape.sub_shape(ShapeType::Edge, *i).ok().and_then(|e| e.edge_info().ok()).is_some_and(|inf| !inf.closed && matches!(inf.curve, noshape_kernel::Curve::Circle { .. })))
        .map(|i| Reference::Edge((*b.edges[i]).clone()))
        .expect("an arc edge");
    let pts = inference_candidates(&ra.context, Some(&ab), &arc, &units);
    assert_eq!(pts.len(), 4, "center, midpoint, two ends: {:?}", pts.iter().map(|p| p.frame.origin).collect::<Vec<_>>());
    // N9: a block vertex — the point, Z normal to a planar face around it.
    let (bb, rb) = body_of(&mut ev, &f.doc, f.studios[3]);
    let blk = rb.context.bodies.iter().find(|b| b.id == bb).unwrap();
    let v = Reference::Vertex((*blk.vertices[0]).clone());
    let pts = inference_candidates(&rb.context, Some(&bb), &v, &units);
    assert_eq!(pts.len(), 1);
    let z = pts[0].frame.z();
    assert!([DVec3::X, DVec3::Y, DVec3::Z].iter().any(|a| z.cross(*a).length() < 1e-6), "Z normal to one of the corner's faces: {z:?}");
}

#[test]
fn assembly_mass_properties_combine_parts_where_they_sit() {
    let f = doc_with(vec![block_studio()]);
    let mut ev = AssemblyEvaluator::new();
    let (body, _) = body_of(&mut ev, &f.doc, f.studios[0]);
    let mut b = part_instance(2, "Block <2>", f.studios[0], body.clone(), DVec3::new(0.1, 0.0, 0.0), true);
    let q = DQuat::from_rotation_z(std::f64::consts::FRAC_PI_2);
    b.placement.rotation = [q.x, q.y, q.z, q.w];
    let asm = Assembly { instances: vec![part_instance(1, "Block <1>", f.studios[0], body.clone(), DVec3::ZERO, true), b], ..Default::default() };
    let res = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
    let m = super::analysis::assembly_mass_properties(&res, &[], &|_| Some(1000.0)).expect("parts");
    let p = &m.props;
    assert!((p.volume - 2.0 * 8e-6).abs() < 1e-12);
    let mass = 0.008;
    assert!((p.mass.unwrap() - 2.0 * mass).abs() < 1e-9);
    assert!((p.center_of_mass - DVec3::new(0.055, 0.015, 0.005)).length() < 1e-9, "{:?}", p.center_of_mass);
    let izz = 2.0 * (mass * (0.0016 + 0.0004) / 12.0 + mass * 0.00125);
    let ixx = mass * 0.0005 / 12.0 + mass * 0.0017 / 12.0 + 2.0 * mass * 0.000025;
    assert!((p.inertia.z_axis.z - izz).abs() < 1e-12, "Izz {}", p.inertia.z_axis.z);
    assert!((p.inertia.x_axis.x - ixx).abs() < 1e-12, "Ixx {}", p.inertia.x_axis.x);
    // One instance only, and a part without a material.
    let one = super::analysis::assembly_mass_properties(&res, &[InstanceId(2)], &|_| Some(1000.0)).unwrap();
    assert!((one.props.mass.unwrap() - mass).abs() < 1e-9);
    let missing = super::analysis::assembly_mass_properties(&res, &[], &|o| (o.top() == InstanceId(1)).then_some(1000.0)).unwrap();
    assert!(missing.props.mass.is_none());
    assert_eq!(missing.without_material.len(), 1);
}

#[test]
fn propagated_inputs_set_the_instances_configuration() {
    use super::config::{param, AsmConfiguredParam, ConfigTarget};
    use crate::configuration::ConfigValues;
    let f = doc_with(vec![block_studio()]);
    let mut ev = AssemblyEvaluator::new();
    let (body, _) = body_of(&mut ev, &f.doc, f.studios[0]);
    let mut asm = Assembly { instances: vec![part_instance(1, "Block <1>", f.studios[0], body, DVec3::ZERO, true)], ..Default::default() };
    asm.configuration.inputs.push(super::config::new_list_input("size".into(), 1));
    asm.add_config_row("size", "Large", "large".into());
    // Input propagation: Block <1>'s own "size" follows the assembly's.
    let values = ["default", "large"].iter().map(|k| (k.to_string(), ParamValue::Text(k.to_string()))).collect();
    asm.configuration.configured.push(AsmConfiguredParam { input: "size".into(), target: ConfigTarget::Instance(InstanceId(1)), param: format!("{}size", param::INPUT_PREFIX), values });
    let key = |a: &Assembly| a.instances[0].source.clone().config_mut().and_then(|c| c.0.get("size").cloned());
    let mut cfg = ConfigValues::default();
    cfg.0.insert("size".into(), "large".into());
    assert_eq!(key(&asm.configured(&cfg)).as_deref(), Some("large"));
    cfg.0.insert("size".into(), "default".into());
    assert_eq!(key(&asm.configured(&cfg)).as_deref(), Some("default"));
}

/// The document's Part Studios as a Derived feature's sources (as the app's
/// `DocStudios`, without its caching).
#[derive(Debug)]
struct DocSources(Document);

impl crate::regen::derived::DerivedProvider for DocSources {
    fn context(&self, tab: &str) -> Option<std::sync::Arc<crate::regen::context::Context>> {
        let t = self.0.tabs.iter().find(|t| t.id.to_string() == tab)?;
        let TabContent::PartStudio(ps) = &t.content else { return None };
        Some(Regenerator::new().regenerate(ps, &ConfigValues::default(), &UnitPrefs::default(), &RegenOptions::default()).context)
    }
    fn version(&self, _tab: &str) -> u64 {
        0
    }
}

/// qa5 DER8: a Part Studio holding only derived parts lists them in Insert
/// (and they can be inserted): its regeneration finds the source studio.
#[test]
fn derived_parts_are_insertable() {
    let mut f = doc_with(vec![block_studio()]);
    let units = UnitPrefs::default();
    let mut ps = PartStudio::new();
    let mut d = ps.new_feature("importDerived", &units).unwrap();
    d.params.set("partStudio", ParamValue::Text(f.studios[0].to_string()));
    ps.features.push(d);
    let derived_tab = f.doc.add_tab("Part Studio 2", TabContent::PartStudio(ps));
    // Without sources the derived feature can't make its part.
    let mut plain = AssemblyEvaluator::new();
    assert!(plain.studio(&f.doc, derived_tab, &ConfigValues::default()).unwrap().context.bodies.is_empty());
    let mut ev = AssemblyEvaluator::new();
    ev.set_studio_options(std::sync::Arc::new(|d: &Document| RegenOptions { derived: Some(std::sync::Arc::new(DocSources(d.clone()))), ..Default::default() }));
    let r = ev.studio(&f.doc, derived_tab, &ConfigValues::default()).unwrap();
    assert_eq!(r.context.bodies.len(), 1, "{:?}", r.display.features.iter().map(|x| (&x.status, &x.message)).collect::<Vec<_>>());
    assert!(!r.display.bodies.is_empty(), "listed in the Insert dialog");
}

/// qa5 MT6: a new Fastened mate's Offset rotation defaults to "Rotate about X".
#[test]
fn fastened_offset_rotates_about_x_by_default() {
    let c = |i: u64| MateConnectorRef::on(InstanceId(i), vec![], Reference::Origin);
    assert_eq!(Mate::new(MateId(1), "Fastened 1".into(), MateType::Fastened, c(1), c(2)).rotation_axis, 0);
    assert_eq!(Mate::new(MateId(2), "Revolute 1".into(), MateType::Revolute, c(1), c(2)).rotation_axis, 2);
}


/// ANSI tapping screws (ASME B18.6.4): every tip type regenerates (live's
/// component geometry: cones, blunt points, the D / G slots, the F flutes
/// and the BT / T quarter cut), and the BT cut's sphere has live's radius
/// (#8: c = (3L² − D²)/4L, R = L − c = 0.0883 in for L = 0.111, D = 0.164).
#[test]
#[ignore = "T2: standard content vs live (P3 long tail; slow)"]
fn tapping_screw_tips_regenerate() {
    use super::standard::{self, CATALOGUE, StandardSpec};
    let c = CATALOGUE.iter().find(|c| c.name == "Flat countersunk trim head tapping screw").expect("tapping screws in the catalogue");
    let mut spec = StandardSpec::new(c);
    spec.set_size("#8-#6");
    let tips: Vec<String> = c.params.iter().find(|p| p.label == "Tip type").unwrap().options.clone();
    assert_eq!(tips, ["A", "AB", "B", "BF", "BP", "BT", "C", "D", "F", "G", "T"]);
    for tip in &tips {
        spec.params.insert("Tip type".into(), tip.clone());
        spec.params.remove("Threads/inch");
        spec.fix_length();
        let r = standard::regenerate(&spec).unwrap_or_else(|e| panic!("tip {tip}: {e}"));
        let failed: Vec<_> = r.display.features.iter().filter_map(|f| f.message.clone()).collect();
        assert!(failed.is_empty(), "tip {tip}: {failed:?}");
        let body = r.context.body(&standard::part_body(&r).unwrap()).unwrap();
        assert!(body.shape.volume().unwrap() > 0.0, "tip {tip}");
        if tip == "BT" {
            let radii: Vec<f64> = (0..body.faces.len())
                .filter_map(|i| match body.shape.sub_shape(noshape_kernel::ShapeType::Face, i).ok()?.face_surface().ok()? {
                    noshape_kernel::Surface::Sphere { radius, .. } => Some(radius / 0.0254),
                    _ => None,
                })
                .collect();
            assert!(radii.iter().any(|r| (r - 0.0883).abs() < 5e-4), "BT sphere radius (in): {radii:?}");
        }
    }
}

/// Tapping-screw Type III / Type VI recesses (live, behaviours.md "skb: Type
/// III / Type VI recess features"): a square / hexalobular wall with a 140°
/// cone bottom at the FSRules depth (the 0.002 in mouth fillet: see
/// `pan_head_tapping_screw_mouth_fillets`). Fillister #8: III 0.112 /
/// 0.075 in, VI Ø0.155 / Ø0.1094 / 0.055 in.
#[test]
#[ignore = "T2: standard content vs live (P3 long tail; slow)"]
fn tapping_screw_recesses_iii_and_vi() {
    use super::standard::{self, CATALOGUE, StandardSpec};
    for name in ["Fillister head tapping screw", "Oval countersunk head tapping screw"] {
        let c = CATALOGUE.iter().find(|c| c.name == name).expect("in the catalogue");
        let mut spec = StandardSpec::new(c);
        spec.set_size("#8");
        let mut volumes = Vec::new();
        for head in ["Type I", "Type III", "Type VI"] {
            spec.params.insert("Head type".into(), head.into());
            spec.fix_length();
            let r = standard::regenerate(&spec).unwrap_or_else(|e| panic!("{name} {head}: {e}"));
            let failed: Vec<_> = r.display.features.iter().filter_map(|f| f.message.clone()).collect();
            assert!(failed.is_empty(), "{name} {head}: {failed:?}");
            let body = r.context.body(&standard::part_body(&r).unwrap()).unwrap();
            volumes.push(body.shape.volume().unwrap());
            if head == "Type I" {
                continue;
            }
            let surfaces: Vec<noshape_kernel::Surface> = (0..body.faces.len()).filter_map(|i| body.shape.sub_shape(noshape_kernel::ShapeType::Face, i).ok()?.face_surface().ok()).collect();
            // The 140° cone bottom (70° half angle) and the 0.002 in fillet.
            assert!(surfaces.iter().any(|s| matches!(s, noshape_kernel::Surface::Cone { half_angle, .. } if (half_angle.abs().to_degrees() - 70.0).abs() < 0.01)), "{name} {head}: cone bottom");
        }
        assert!(volumes.iter().all(|v| *v > 0.0), "{name}: {volumes:?}");
    }
}

/// Pan head tapping screw #8, tip A (live skb-sc-geom): crown sphere
/// R 1.65·D = 0.2706 in, head Ø0.322 × 0.115 with an R0.015 edge fillet;
/// faces: Slot 10, Type III 19, Type VI 15.
#[test]
fn pan_head_tapping_screw_matches_live() {
    use super::standard::{self, CATALOGUE, StandardSpec};
    let c = CATALOGUE.iter().find(|c| c.name == "Pan head tapping screw").expect("in the catalogue");
    let mut spec = StandardSpec::new(c);
    spec.set_size("#8");
    spec.params.insert("Tip type".into(), "A".into());
    // Live: Type VI 15, Type III 19 with the 0.002 in mouth fillet (1 / 4
    // faces).
    for (head, faces) in [("Slot", 10), ("Type VI", 15), ("Type III", 19)] {
        spec.params.insert("Head type".into(), head.into());
        spec.params.remove("Threads/inch");
        spec.fix_length();
        let r = standard::regenerate(&spec).unwrap_or_else(|e| panic!("{head}: {e}"));
        let failed: Vec<_> = r.display.features.iter().filter_map(|f| f.message.clone()).collect();
        assert!(failed.is_empty(), "{head}: {failed:?}");
        let body = r.context.body(&standard::part_body(&r).unwrap()).unwrap();
        let surfaces: Vec<noshape_kernel::Surface> = (0..body.faces.len()).filter_map(|i| body.shape.sub_shape(noshape_kernel::ShapeType::Face, i).ok()?.face_surface().ok()).collect();
        assert!(surfaces.iter().any(|s| matches!(s, noshape_kernel::Surface::Sphere { radius, .. } if (radius / 0.0254 - 0.2706).abs() < 1e-6)), "{head}: crown");
        assert!(surfaces.iter().any(|s| matches!(s, noshape_kernel::Surface::Torus { minor_radius, .. } if (minor_radius / 0.0254 - 0.015).abs() < 1e-6)), "{head}: edge fillet");
        let mut kinds: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
        for s in &surfaces {
            *kinds.entry(format!("{s:?}").split([' ', '(']).next().unwrap_or_default().to_string()).or_default() += 1;
        }
        assert_eq!(body.faces.len(), faces, "{head}: {kinds:?}");
    }
}

/// Live's Pan #8 Type VI 15 faces / Type III 19 faces include the 0.002 in
/// mouth fillet (1 / 4 TORUS-like faces).
#[test]
fn pan_head_tapping_screw_mouth_fillets() {
    use super::standard::{self, CATALOGUE, StandardSpec};
    let c = CATALOGUE.iter().find(|c| c.name == "Pan head tapping screw").expect("in the catalogue");
    let mut spec = StandardSpec::new(c);
    spec.set_size("#8");
    spec.params.insert("Tip type".into(), "A".into());
    for (head, faces) in [("Type VI", 15), ("Type III", 19)] {
        spec.params.insert("Head type".into(), head.into());
        spec.params.remove("Threads/inch");
        spec.fix_length();
        let r = standard::regenerate(&spec).unwrap();
        let body = r.context.body(&standard::part_body(&r).unwrap()).unwrap();
        assert_eq!(body.faces.len(), faces, "{head}");
    }
}

/// The remaining ANSI tapping screws (live, behaviours.md "skb (10-22 07:45)"):
/// #8 tip A face counts as live's geometry files (the 0.002 in mouth fillet
/// of Type III / VI included: FeatureScript's opFillet is the checked kernel
/// fillet). Flat countersunk: live's Type VI file (14 faces, its under-head
/// arc a torus); Slot / Type III counted the same way. Undercut heads stop the 41° cone at the head height with a flat
/// step; hex heads with a 30° top chamfer, the indent a Ø0.9 s dish; the hex
/// washer's disc under the hex.
#[test]
#[ignore = "T2: standard content vs live (P3 long tail; slow)"]
fn remaining_tapping_screws_match_live() {
    use super::standard::{self, CATALOGUE, StandardSpec};
    // (head type, live's faces).
    type Heads = &'static [(&'static str, usize)];
    let cases: [(&str, Heads); 5] = [
        ("Flat countersunk head tapping screw", &[("Slot", 9), ("Type III", 18), ("Type VI", 14)]),
        ("Flat undercut countersunk head tapping screw", &[("Slot", 9), ("Type III", 18)]),
        ("Oval undercut countersunk head tapping screw", &[("Slot", 9), ("Type III", 18)]),
        ("Hex head tapping screw", &[("Plain", 16), ("Indent", 18), ("Slot", 20), ("Slot and indent", 24), ("Type VI", 27)]),
        ("Hex washer head tapping screw", &[("Plain", 18), ("Indent", 20), ("Slot", 22), ("Slot and indent", 26), ("Type VI", 29)]),
    ];
    let mut wrong = Vec::new();
    for (name, heads) in cases {
        let c = CATALOGUE.iter().find(|c| c.name == name).unwrap_or_else(|| panic!("{name} in the catalogue"));
        let mut spec = StandardSpec::new(c);
        spec.set_size("#8");
        spec.params.insert("Tip type".into(), "A".into());
        for (head, faces) in heads {
            spec.params.insert("Head type".into(), (*head).into());
            spec.params.remove("Threads/inch");
            spec.fix_length();
            let r = standard::regenerate(&spec).unwrap_or_else(|e| panic!("{name} {head}: {e}"));
            let failed: Vec<_> = r.display.features.iter().filter_map(|f| f.message.clone()).collect();
            assert!(failed.is_empty(), "{name} {head}: {failed:?}");
            let body = r.context.body(&standard::part_body(&r).unwrap()).unwrap();
            if body.faces.len() != *faces {
                wrong.push(format!("{name} {head}: {} faces, live {faces}", body.faces.len()));
            }
            // Flat countersunk Type VI: live's 14 (behaviours "skb (10-23
            // 03:30)") include the 0.002 in recess-mouth fillet, one OTHER
            // (B-spline) face of 0.00134 in².
            if name == "Flat countersunk head tapping screw" && *head == "Type VI" {
                let mouth: Vec<f64> = (0..body.faces.len())
                    .filter_map(|i| body.shape.sub_shape(noshape_kernel::ShapeType::Face, i).ok())
                    .filter(|f| matches!(f.face_surface(), Ok(noshape_kernel::Surface::BSpline { .. })))
                    .filter_map(|f| f.area().ok().map(|a| a / 0.0254 / 0.0254))
                    .collect();
                if mouth.len() != 1 || (mouth[0] - 0.00134).abs() > 0.000005 {
                    wrong.push(format!("{name} {head}: B-spline faces {mouth:?} in², live one of 0.00134"));
                }
            }
            // Overall height as live's geometry files (in; a slot can take the apex).
            let live_h = match (name, *head) {
                ("Flat undercut countersunk head tapping screw", _) => Some(1.25),
                ("Oval undercut countersunk head tapping screw", "Slot") => Some(1.3016),
                ("Oval undercut countersunk head tapping screw", "Type III") => Some(1.2963),
                ("Hex head tapping screw", _) => Some(1.36),
                ("Hex washer head tapping screw", _) => Some(1.391),
                _ => None,
            };
            if let Some(h) = live_h {
                let b = body.shape.bounding_box(true).unwrap();
                let got = (b.max.z - b.min.z) / 0.0254;
                if (got - h).abs() > 8e-4 {
                    wrong.push(format!("{name} {head}: height {got:.4} in, live {h}"));
                }
            }
        }
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
}



/// Live's Type I (Phillips) recess (behaviours.md "skb (10-22 19:40)"): Hex #8
/// Type I = 63 faces, Hex washer 65; every face's area as live's face lists
/// (dom/skb-sc-typeI-facelists.json, in², sorted).
#[test]
#[ignore = "T2: standard content vs live (P3 long tail; slow)"]
fn hex_type_i_recess_matches_live() {
    use super::standard::{self, CATALOGUE, StandardSpec};
    const HEX: [f64; 63] = [4.09e-05, 4.09e-05, 4.09e-05, 4.09e-05, 4.09e-05, 4.09e-05, 4.09e-05, 4.09e-05, 9.63e-05, 9.63e-05, 9.63e-05, 9.63e-05, 0.0001328, 0.0001328, 0.0001328, 0.0001328, 0.0001328, 0.0001328, 0.0001328, 0.0001328, 0.0002074, 0.0002074, 0.0002074, 0.0002074, 0.0009698, 0.0009698, 0.0009698, 0.0009698, 0.0009698, 0.0009698, 0.00117, 0.00117, 0.00117, 0.00117, 0.00117, 0.00117, 0.00117, 0.00117, 0.0018665, 0.0018665, 0.0018665, 0.0018665, 0.0018665, 0.0018665, 0.0018665, 0.0018665, 0.0025543, 0.0025543, 0.0025543, 0.0025543, 0.0053973, 0.0093266, 0.0129591, 0.0153244, 0.0153244, 0.0153244, 0.0153244, 0.0153244, 0.0153244, 0.026083, 0.0330025, 0.0551999, 0.5420305];
    const HEX_WASHER: [f64; 65] = [4.09e-05, 4.09e-05, 4.09e-05, 4.09e-05, 4.09e-05, 4.09e-05, 4.09e-05, 4.09e-05, 9.63e-05, 9.63e-05, 9.63e-05, 9.63e-05, 0.0001328, 0.0001328, 0.0001328, 0.0001328, 0.0001328, 0.0001328, 0.0001328, 0.0001328, 0.0002074, 0.0002074, 0.0002074, 0.0002074, 0.0009698, 0.0009698, 0.0009698, 0.0009698, 0.0009698, 0.0009698, 0.00117, 0.00117, 0.00117, 0.00117, 0.00117, 0.00117, 0.00117, 0.00117, 0.0018665, 0.0018665, 0.0018665, 0.0018665, 0.0018665, 0.0018665, 0.0018665, 0.0018665, 0.0025543, 0.0025543, 0.0025543, 0.0025543, 0.0053973, 0.0093266, 0.0129591, 0.0153244, 0.0153244, 0.0153244, 0.0153244, 0.0153244, 0.0153244, 0.026083, 0.0338915, 0.0409883, 0.0551999, 0.0739908, 0.5420305];
    for (name, live) in [("Hex head tapping screw", &HEX[..]), ("Hex washer head tapping screw", &HEX_WASHER[..])] {
        let c = CATALOGUE.iter().find(|c| c.name == name).unwrap();
        let mut spec = StandardSpec::new(c);
        spec.set_size("#8");
        spec.params.insert("Tip type".into(), "A".into());
        spec.params.insert("Head type".into(), "Type I".into());
        spec.params.remove("Threads/inch");
        spec.fix_length();
        let r = standard::regenerate(&spec).unwrap_or_else(|e| panic!("{name}: {e}"));
        let body = r.context.body(&standard::part_body(&r).unwrap()).unwrap();
        let mut areas: Vec<f64> = (0..body.faces.len()).map(|i| body.shape.sub_shape(noshape_kernel::ShapeType::Face, i).unwrap().area().unwrap() / 0.0254 / 0.0254).collect();
        areas.sort_by(f64::total_cmp);
        assert_eq!(areas.len(), live.len(), "{name}: {areas:?}");
        // Walls, floors, cone and head within 0.5 %; the 0.002 in fillets and the
        // arm ends they trim within 1.5 % (OCCT ends the fillets at the corners a
        // little differently from Parasolid).
        let off: Vec<(f64, f64)> = areas.iter().zip(live).filter(|(a, l)| (*a - *l).abs() > (l.abs() * 1.5e-2).max(2e-7)).map(|(a, l)| (*a, *l)).collect();
        assert!(off.is_empty(), "{name}: (ours, live) {off:?}");
    }
}

/// Live's Type I recipe on every other head (cross + kites cut from the floor
/// plane up through the head): every #8 regenerates, with the 20-edge mouth
/// fillet where OCCT makes it (flat tops; crowns need kern's checked fillet).
#[test]
#[ignore = "T2: standard content vs live (P3 long tail; slow)"]
fn type_i_recipe_regenerates_on_every_head() {
    use super::standard::{self, CATALOGUE, StandardSpec};
    for (name, size) in [
        ("Pan head tapping screw", "#8"),
        ("Fillister head tapping screw", "#8"),
        ("Flat countersunk head tapping screw", "#8"),
        ("Flat undercut countersunk head tapping screw", "#8"),
        ("Oval countersunk head tapping screw", "#8"),
        ("Oval undercut countersunk head tapping screw", "#8"),
        ("Flat countersunk trim head tapping screw", "#8-#5"),
        ("Oval countersunk trim head tapping screw", "#8-#5"),
    ] {
        let c = CATALOGUE.iter().find(|c| c.name == name).unwrap();
        let mut spec = StandardSpec::new(c);
        spec.set_size(size);
        spec.params.insert("Tip type".into(), "A".into());
        spec.params.insert("Head type".into(), "Type I".into());
        spec.params.remove("Threads/inch");
        spec.fix_length();
        let r = standard::regenerate(&spec).unwrap_or_else(|e| panic!("{name}: {e}"));
        let failed: Vec<_> = r.display.features.iter().filter_map(|f| f.message.clone()).collect();
        assert!(failed.is_empty(), "{name}: {failed:?}");
        let body = r.context.body(&standard::part_body(&r).unwrap()).unwrap();
        // Recess 45 faces (25 while the crown's mouth fillet fails) + the head's own.
        assert!(body.faces.len() >= 29, "{name}: {} faces", body.faces.len());
        eprintln!("{name} Type I: {} faces", body.faces.len());
    }
}


/// Live's Type IA recess (behaviours.md "skb (10-22 19:40)", Pan #8): one arm
/// patterned 90°, the slot-angle core with its 28° cone, four 10° V-groove
/// wings, the 0.0059 in arm/core fillets: all 43 faces have live's areas.
#[test]
fn pan_type_ia_recess_matches_live() {
    use super::standard::{self, CATALOGUE, StandardSpec};
    const LIVE: [f64; 43] = [0.0003561, 0.0003561, 0.0003561, 0.0003561, 0.0003561, 0.0003561, 0.0003561, 0.0003561, 0.0003973, 0.0003973, 0.0003973, 0.0003973, 0.0003973, 0.0003973, 0.0003973, 0.0003973, 0.0005825, 0.0005825, 0.0005825, 0.0005825, 0.0014395, 0.0014395, 0.0014395, 0.0014395, 0.0014395, 0.0014395, 0.0014395, 0.0014395, 0.001689, 0.001689, 0.001689, 0.001689, 0.0024152, 0.0024152, 0.0024152, 0.0024152, 0.0075453, 0.014409, 0.0548267, 0.0551999, 0.0603092, 0.0675773, 0.5420305];
    let c = CATALOGUE.iter().find(|c| c.name == "Pan head tapping screw").unwrap();
    let mut spec = StandardSpec::new(c);
    spec.set_size("#8");
    spec.params.insert("Tip type".into(), "A".into());
    spec.params.insert("Head type".into(), "Type IA".into());
    spec.params.remove("Threads/inch");
    spec.fix_length();
    let r = standard::regenerate(&spec).unwrap_or_else(|e| panic!("{e}"));
    let body = r.context.body(&standard::part_body(&r).unwrap()).unwrap();
    let mut areas: Vec<f64> = (0..body.faces.len()).map(|i| body.shape.sub_shape(noshape_kernel::ShapeType::Face, i).unwrap().area().unwrap() / 0.0254 / 0.0254).collect();
    areas.sort_by(f64::total_cmp);
    let near = |a: f64, l: f64| (a - l).abs() <= (l * 5e-3).max(2e-7);
    assert_eq!(areas.len(), LIVE.len(), "{areas:?}");
    let off: Vec<(f64, f64)> = areas.iter().zip(LIVE).filter(|(a, l)| !near(**a, *l)).map(|(a, l)| (*a, l)).collect();
    assert!(off.is_empty(), "(ours, live) {off:?}");
}

/// Live's Type IA recipe on every head that offers it (variants: wing length
/// L·1.25 / 1.125 / 1, V-groove or flat-floored wings, Fillister's one arm).
#[test]
#[ignore = "T2: standard content vs live (P3 long tail; slow)"]
fn type_ia_recipe_regenerates_on_every_head() {
    use super::standard::{self, CATALOGUE, StandardSpec};
    for (name, size) in [
        ("Pan head tapping screw", "#8"),
        ("Fillister head tapping screw", "#8"),
        ("Flat countersunk head tapping screw", "#8"),
        ("Flat undercut countersunk head tapping screw", "#8"),
        ("Oval countersunk head tapping screw", "#8"),
        ("Oval undercut countersunk head tapping screw", "#8"),
        ("Flat countersunk trim head tapping screw", "#8-#5"),
        ("Oval countersunk trim head tapping screw", "#8-#5"),
    ] {
        let c = CATALOGUE.iter().find(|c| c.name == name).unwrap();
        let mut spec = StandardSpec::new(c);
        spec.set_size(size);
        spec.params.insert("Tip type".into(), "A".into());
        spec.params.insert("Head type".into(), "Type IA".into());
        spec.params.remove("Threads/inch");
        spec.fix_length();
        let r = standard::regenerate(&spec).unwrap_or_else(|e| panic!("{name}: {e}"));
        let failed: Vec<_> = r.display.features.iter().filter_map(|f| f.message.clone()).collect();
        assert!(failed.is_empty(), "{name}: {failed:?}");
        let body = r.context.body(&standard::part_body(&r).unwrap()).unwrap();
        assert!(body.faces.len() >= 20, "{name}: {} faces", body.faces.len());
        eprintln!("{name} Type IA: {} faces", body.faces.len());
    }
}



/// ANSI nuts and spring lock washers built from Onshape's own tables
/// (dom/skb-sc-comp) as live's B-rep (dom/skb-sc-geom, default size): face
/// types and total area (m²); lock nuts at live's smallest and largest size.
#[test]
#[ignore = "T2: standard content vs live (P3 long tail; slow)"]
fn ansi_nuts_and_lock_washers_match_live() {
    use super::standard::{self, CATALOGUE, StandardSpec};
    type Case = (&'static str, &'static [(&'static str, usize)], f64);
    let cases: [Case; 7] = [
        ("Hex flat nut", &[("Cone", 8), ("Cylinder", 1), ("Plane", 8)], 7.898129e-3),
        ("Hex machine screw nut", &[("Cone", 14), ("Cylinder", 1), ("Plane", 8)], 4.520420e-5),
        ("Square nut", &[("Cone", 10), ("Cylinder", 1), ("Plane", 6)], 5.276197e-4),
        ("Hex slotted nut", &[("Cone", 19), ("Cylinder", 7), ("Plane", 25)], 4.755728e-4),
        ("Heavy hex slotted nut", &[("Cone", 13), ("Cylinder", 8), ("Plane", 31)], 6.322624e-4),
        ("Regular spring lock washer", &[("Cone", 2), ("Cylinder", 2), ("Plane", 2), ("Torus", 4)], 2.515125e-5),
        ("High collar spring lock washer", &[("Cone", 2), ("Cylinder", 2), ("Plane", 2), ("Torus", 4)], 2.258067e-5),
    ];
    // Lock nuts at the sizes live captured (their default bodies are placeholders).
    let sized: [(&str, &str, Case); 8] = [
        ("T-slot nut", "5/16", ("", &[("Cone", 2), ("Cylinder", 3), ("Plane", 14)], 8.159743e-4)),
        ("T-slot nut", "1-1/2", ("", &[("Cone", 2), ("Cylinder", 3), ("Plane", 14)], 1.994554e-2)),
        ("Hex lock nut", "#4-40", ("", &[("Cone", 8), ("Cylinder", 1), ("Plane", 8)], 1.082215e-4)),
        ("Hex lock nut", "4-4", ("", &[("Cone", 8), ("Cylinder", 1), ("Plane", 8)], 9.182168e-2)),
        ("Hex lock nut with nylon insert", "#2-56", ("", &[("Cone", 14), ("Cylinder", 3), ("Plane", 9), ("Torus", 1)], 1.379852e-4)),
        ("Hex lock nut with nylon insert", "1-1/2-6", ("", &[("Cone", 14), ("Cylinder", 3), ("Plane", 8), ("Torus", 1)], 1.237012e-2)),
        ("Hex thin lock nut with nylon insert", "#4-40", ("", &[("Cone", 14), ("Cylinder", 3), ("Plane", 9), ("Torus", 1)], 1.659259e-4)),
        ("Hex thin lock nut with nylon insert", "1-1/2-12", ("", &[("Cone", 14), ("Cylinder", 3), ("Plane", 9), ("Torus", 1)], 1.464824e-2)),
    ];
    let all = cases.iter().map(|(n, k, a)| (*n, None, *k, *a)).chain(sized.iter().map(|(n, size, (_, k, a))| (*n, Some(*size), *k, *a)));
    for (name, size, kinds, live_area) in all {
        let c = CATALOGUE.iter().find(|c| c.name == name).unwrap_or_else(|| panic!("{name} in the catalogue"));
        let mut spec = StandardSpec::new(c);
        if let Some(size) = size {
            spec.set_size(size);
        }
        let name = format!("{name} {}", spec.size);
        let r = standard::regenerate(&spec).unwrap_or_else(|e| panic!("{name}: {e}"));
        let b = r.context.body(&standard::part_body(&r).unwrap()).unwrap();
        let mut got: std::collections::BTreeMap<String, usize> = Default::default();
        let mut area = 0.0;
        for i in 0..b.faces.len() {
            let f = b.shape.sub_shape(noshape_kernel::ShapeType::Face, i).unwrap();
            area += f.area().unwrap();
            *got.entry(format!("{:?}", f.face_surface().unwrap()).split([' ', '(']).next().unwrap().to_string()).or_default() += 1;
        }
        let want: std::collections::BTreeMap<String, usize> = kinds.iter().map(|(k, n)| (k.to_string(), *n)).collect();
        assert_eq!(got, want, "{name}");
        assert!((area / live_area - 1.0).abs() < 1e-3, "{name}: area {area} live {live_area}");
    }
}

/// DIN keys as live's B-rep (dom/skb-sc-geom, smallest and largest sizes):
/// along Y on the XY plane, round ends (Type A), tapered 1:100 from the -Y end.
#[test]
fn din_keys_match_live() {
    use super::standard::{self, CATALOGUE, StandardSpec};
    // (DIN 6885-1's largest size, 440mm - 550mm, has no length list in live's
    // tables: its LENGTHS rows say "440mm - 500mm"; it is left out.)
    let cases: [(&str, &str, &str, usize, usize, f64); 5] = [
        ("DIN 6885-1 Parallel keys", "6mm - 8mm", "6", 2, 4, 5.084956e-5),
        ("DIN 6883 Tapered key", "22mm - 30mm", "20", 0, 6, 5.944080e-4),
        ("DIN 6883 Tapered key", "200mm - 230mm", "400", 0, 6, 5.440100e-2),
        ("DIN 6886 Stressed tapered key", "6mm - 8mm", "6", 2, 4, 5.042161e-5),
        ("DIN 6886 Stressed tapered key", "75mm - 85mm", "250", 2, 4, 1.748775e-2),
    ];
    for (name, size, length, cylinders, planes, live_area) in cases {
        let c = CATALOGUE.iter().find(|c| c.name == name).unwrap_or_else(|| panic!("{name} in the catalogue"));
        let mut spec = StandardSpec::new(c);
        spec.set_size(size);
        spec.length = Some(length.into());
        let r = standard::regenerate(&spec).unwrap_or_else(|e| panic!("{name} {size}: {e}"));
        let b = r.context.body(&standard::part_body(&r).unwrap()).unwrap();
        let (mut cyl, mut pl, mut area) = (0, 0, 0.0);
        for i in 0..b.faces.len() {
            let f = b.shape.sub_shape(noshape_kernel::ShapeType::Face, i).unwrap();
            area += f.area().unwrap();
            match f.face_surface().unwrap() {
                noshape_kernel::Surface::Cylinder { .. } => cyl += 1,
                noshape_kernel::Surface::Plane(_) => pl += 1,
                s => panic!("{name} {size}: {s:?}"),
            }
        }
        assert_eq!((cyl, pl), (cylinders, planes), "{name} {size}");
        assert!((area / live_area - 1.0).abs() < 1e-4, "{name} {size}: area {area} live {live_area}");
        // The thick end at -Y (live's taper falls towards +Y).
        let bb = b.shape.bounding_box(true).unwrap();
        assert!(bb.min.z.abs() < 1e-9 && (bb.max.y + bb.min.y).abs() < 1e-9, "{name} {size}: {bb:?}");
    }
    // Type AB (live's Extrude 1 regions): round at -Y, square at +Y.
    let c = CATALOGUE.iter().find(|c| c.name == "DIN 6885-1 Parallel keys").unwrap();
    let mut spec = StandardSpec::new(c);
    spec.length = Some("6".into());
    spec.params.insert("Type".into(), "AB".into());
    let r = standard::regenerate(&spec).unwrap();
    let b = r.context.body(&standard::part_body(&r).unwrap()).unwrap();
    let round: Vec<f64> = (0..b.faces.len())
        .filter_map(|i| match b.shape.sub_shape(noshape_kernel::ShapeType::Face, i).unwrap().face_surface().unwrap() {
            noshape_kernel::Surface::Cylinder { origin, .. } => Some(origin.y),
            _ => None,
        })
        .collect();
    assert!(round.len() == 1 && round[0] < 0.0, "AB round end: {round:?}");
    // The chosen Type names the part as live ("Parallel key B x 2 x 2 x 6").
    let c = CATALOGUE.iter().find(|c| c.name == "DIN 6885-1 Parallel keys").unwrap();
    let mut spec = StandardSpec::new(c);
    spec.length = Some("6".into());
    spec.params.insert("Type".into(), "B".into());
    assert_eq!(spec.part_name(), "Parallel key B x 2 x 2 x 6");
}

/// AS568 O-rings from live's table (inch: −001 is 0.029 in inside, 0.037 in
/// section) with live's four materials, Butyl first.
#[test]
fn as568_o_rings_match_live() {
    use super::standard::{self, CATALOGUE, StandardSpec};
    let c = CATALOGUE.iter().find(|c| c.name == "AS568 D O-rings").unwrap();
    assert_eq!(c.materials(), ["Butyl", "Nitrile", "Silicone Rubber", "Teflon"]);
    let spec = StandardSpec::new(c);
    assert_eq!(spec.part_name(), "O-ring -001 (0.029 x 0.04)");
    assert!(spec.description().starts_with("O-ring -001 (0.029 x 0.04) Butyl"), "{}", spec.description());
    let r = standard::regenerate(&spec).unwrap();
    let b = r.context.body(&standard::part_body(&r).unwrap()).unwrap();
    let bb = b.shape.bounding_box(true).unwrap();
    // Outside diameter = inside + 2 sections; height = the section.
    assert!(((bb.max.x - bb.min.x) / 0.0254 - (0.029 + 2.0 * 0.037)).abs() < 1e-4, "{bb:?}");
    assert!(((bb.max.z - bb.min.z) / 0.0254 - 0.037).abs() < 1e-4, "{bb:?}");
    // ISO 3601-1: live's size names carry inside diameter x width (mm), shown
    // per size in the read-only Inside diameter / Width.
    let c = CATALOGUE.iter().find(|c| c.name == "ISO 3601-1:2012 O-rings for aerospace applications").unwrap();
    assert_eq!(c.materials(), ["Butyl", "Nitrile", "Silicone Rubber", "Teflon"]);
    let mut spec = StandardSpec::new(c);
    spec.set_size("-A0020 (2 x 1.8)");
    assert_eq!((spec.param("Inside diameter"), spec.param("Width")), ("2", "1.8"));
    let r = standard::regenerate(&spec).unwrap();
    let bb = r.context.body(&standard::part_body(&r).unwrap()).unwrap().shape.bounding_box(true).unwrap();
    assert!(((bb.max.x - bb.min.x) * 1e3 - (2.0 + 2.0 * 1.8)).abs() < 1e-3, "{bb:?}");
}

/// Sizes live lists but can't build (behaviours "skb (10-23 14:00)"): listed
/// in live's order with the server's message; no geometry.
#[test]
fn failing_standard_sizes_are_listed_with_live_errors() {
    use super::standard::CATALOGUE;
    let c = CATALOGUE.iter().find(|c| c.name == "Half dog point hex socket set screw").unwrap();
    let names = c.size_names();
    assert_eq!(&names[names.len() - 2..], ["1-8", "1-12"]);
    assert_eq!(c.failure("1-8"), Some("Not found."));
    assert!(c.size("1-8").is_none() && c.failure("7/8-9").is_none());
    let k = CATALOGUE.iter().find(|c| c.name == "DIN 6885-1 Parallel keys").unwrap();
    assert_eq!(k.size_names().last(), Some(&"440mm - 550mm"));
    assert_eq!(k.failure("440mm - 550mm"), Some("com.belmonttech.service.exception.BTBadRequestException: Feature script error: @stringToNumber: String is not a number"));
}

/// NAS clinch nuts (live's raw /features): NASM 45938/2 (hex on a pilot hex,
/// the bore's pilot end coned to the flats) and NASM 3214 (hex on a stepped
/// shank, Nut thickness rows); faces and total area (m²) of live's B-rep.
#[test]
#[ignore = "T2: standard content vs live (P3 long tail; slow)"]
fn nas_clinch_nuts_match_live() {
    use super::standard::{self, CATALOGUE, StandardSpec};
    let cases: [(&str, &str, usize, f64); 5] = [
        ("Self-locking clinch nut without self-locking threads NASM 45938", "1/4-20", 37, 5.396039629105201e-4),
        ("Self-locking clinch nut without self-locking threads NASM 45938", "3/8-16", 37, 9.938981948384477e-4),
        ("Hex head self-locking clinch nut NASM 45938/2", "#6-32", 29, 2.3303731762714102e-4),
        ("Hex head self-locking clinch nut NASM 45938/2", "1/2-20", 29, 1.99182546955828e-3),
        ("Plain clinch flush nut NASM 3214", "#2-56", 15, 6.52275552151411e-5),
    ];
    for (name, size, faces, live_area) in cases {
        let c = CATALOGUE.iter().find(|c| c.name == name).unwrap_or_else(|| panic!("{name} in the catalogue"));
        let mut spec = StandardSpec::new(c);
        spec.set_size(size);
        assert_eq!(spec.size, size, "{name}: size {size}");
        let r = standard::regenerate(&spec).unwrap_or_else(|e| panic!("{name} {size}: {e}"));
        let b = r.context.body(&standard::part_body(&r).unwrap()).unwrap();
        let area: f64 = (0..b.faces.len()).map(|i| b.shape.sub_shape(noshape_kernel::ShapeType::Face, i).unwrap().area().unwrap()).sum();
        assert_eq!(b.faces.len(), faces, "{name} {size}");
        assert!((area / live_area - 1.0).abs() < 1e-4, "{name} {size}: area {area} live {live_area}");
    }
    let c = CATALOGUE.iter().find(|c| c.name == "Plain clinch flush nut NASM 3214").unwrap();
    let mut spec = StandardSpec::new(c);
    spec.set_size("#6-32");
    spec.params.insert("Nut thickness".into(), "0.147".into());
    assert_eq!(spec.part_name(), "Plain clinch flush nut #6-32 x 0.147");
}

/// ANSI askew head bolt (live's raw /features): the 9.45° skewed square
/// head with its 25° corner cone, the shank filleted into it, the end
/// chamfer, the shank face split at the Thread length. Live's B-rep of 3/8-16
/// x 1 (thread 1): 15 faces, total area (m²).
#[test]
fn askew_head_bolt_matches_live() {
    use super::standard::{self, CATALOGUE, StandardSpec};
    let c = CATALOGUE.iter().find(|c| c.name == "Askew head bolt").unwrap();
    let mut spec = StandardSpec::new(c);
    spec.set_size("3/8-16");
    spec.length = Some("1".into());
    assert_eq!(c.thread_lengths(spec.size_row().unwrap(), "1.25"), ["1.25", "1"]);
    let r = standard::regenerate(&spec).unwrap_or_else(|e| panic!("askew: {e}"));
    let b = r.context.body(&standard::part_body(&r).unwrap()).unwrap();
    let area: f64 = (0..b.faces.len()).map(|i| b.shape.sub_shape(noshape_kernel::ShapeType::Face, i).unwrap().area().unwrap()).sum();
    assert_eq!(b.faces.len(), 15);
    assert!((area / 1.5118592461752917e-3 - 1.0).abs() < 1e-3, "askew area {area}");
}

/// B17.1 keys (live's raw /features + FSRules): parallel = the W x L x H box,
/// tapered = the side profile flat for W at +Y then 1/8 in per foot down to
/// -Y, extruded symmetric W; Type picks HEIGHT_SQR / HEIGHT_REC (only one
/// of them for narrow / wide keys). Faces and areas of live's B-rep at the
/// captures' lengths (8 in / 5 in).
#[test]
fn b17_keys_match_live() {
    use super::standard::{self, CATALOGUE, StandardSpec};
    let cases: [(&str, &str, &str, &str, usize, f64); 3] = [
        ("B17.1 Standard parallel key", "5/16 - 7/16", "Square", "8", 6, 1.9468207031250015e-3),
        ("B17.1 Standard parallel key", "26 - 30", "Rectangle", "8", 6, 1.6903192000000006e-1),
        ("B17.1 Tapered key", "5/16 - 7/16", "Square", "5", 7, 1.0561715809651172e-3),
    ];
    for (name, size, kind, length, faces, live_area) in cases {
        let c = CATALOGUE.iter().find(|c| c.name == name).unwrap_or_else(|| panic!("{name} in the catalogue"));
        let mut spec = StandardSpec::new(c);
        spec.set_size(size);
        assert_eq!(spec.param("Type"), kind, "{name} {size}");
        spec.length = Some(length.into());
        let r = standard::regenerate(&spec).unwrap_or_else(|e| panic!("{name} {size}: {e}"));
        let b = r.context.body(&standard::part_body(&r).unwrap()).unwrap();
        let area: f64 = (0..b.faces.len()).map(|i| b.shape.sub_shape(noshape_kernel::ShapeType::Face, i).unwrap().area().unwrap()).sum();
        assert_eq!(b.faces.len(), faces, "{name} {size}");
        assert!((area / live_area - 1.0).abs() < 1e-6, "{name} {size}: area {area} live {live_area}");
    }
    // Live's Length is a free text box: a typed length stays through size
    // changes and names the part ("… x 2.5").
    let c = CATALOGUE.iter().find(|c| c.name == "B17.1 Standard parallel key").unwrap();
    assert!(c.length_edit);
    let mut spec = StandardSpec::new(c);
    assert_eq!(spec.length.as_deref(), Some("1"));
    spec.length = Some("2.5".into());
    spec.set_size("26 - 30");
    assert_eq!(spec.part_name(), "Parallel key B17.1 7 x 5 x 2.5");
    let c = CATALOGUE.iter().find(|c| c.name == "B17.1 Tapered key").unwrap();
    let mut spec = StandardSpec::new(c);
    spec.set_size("7/16 - 9/16");
    spec.params.insert("Type".into(), "Rectangle".into());
    assert_eq!(spec.part_name(), "Tapered key B17.1 0.125 x 0.09375 x 1");
    assert_eq!(c.failure("26 - 30"), Some("Not found."));
}

/// Retaining rings from live's raw /features (one sketch extruded symmetric):
/// faces and total area (m²) of live's B-rep at live's sizes, one mid-plane
/// connector.
#[test]
#[ignore = "T2: standard content vs live (P3 long tail; slow)"]
fn retaining_rings_match_live() {
    use super::standard::{self, CATALOGUE, StandardSpec};
    let cases: [(&str, &str, usize, f64); 19] = [
        ("External retaining ring DIN 471", "3", 16, 2.9818272413563026e-5),
        ("External retaining ring DIN 471", "20", 16, 4.335286134113179e-4),
        ("External retaining ring DIN 471", "300", 16, 2.9904218621879402e-2),
        ("External heavy retaining ring DIN 471", "15", 16, 3.8767709584080676e-4),
        ("External retaining ring ANSI", "1/8", 16, 1.7601329908219825e-5),
        ("External retaining ring ANSI", "10", 16, 2.9542221674196612e-2),
        // Internal rings by live's configuration (behaviours "skb (10-24)
        // rings / washers", dom/skb-sc-geom/*-cfg.json): A 12 faces, B 16, C 8.
        ("Internal retaining ring DIN 472", "8", 12, 9.06039056661354e-5),
        ("Internal retaining ring DIN 472", "20", 12, 3.530270066208348e-4),
        ("Internal retaining ring DIN 472", "50", 16, 1.736065747516791e-3),
        ("Internal retaining ring DIN 472", "165", 16, 1.2673866128864023e-2),
        ("Internal retaining ring DIN 472", "170", 8, 1.6487842814705427e-2),
        ("Internal retaining ring DIN 472", "300", 8, 3.598308846165846e-2),
        ("Internal heavy retaining ring DIN 472", "20", 12, 4.3285329774319357e-4),
        ("Internal heavy retaining ring DIN 472", "24", 12, 5.705149742541508e-4),
        ("Internal heavy retaining ring DIN 472", "25", 16, 6.296372798030354e-4),
        ("Internal heavy retaining ring DIN 472", "50", 16, 1.9563687460152457e-3),
        ("Internal heavy retaining ring DIN 472", "100", 16, 6.267662448717445e-3),
        ("Internal retaining ring ANSI", "1/4", 12, 4.179622202617048e-5),
        ("Internal retaining ring ANSI", "10", 16, 2.6929616519916714e-2),
    ];
    for (name, size, faces, live_area) in cases {
        let c = CATALOGUE.iter().find(|c| c.name == name).unwrap_or_else(|| panic!("{name} in the catalogue"));
        let mut spec = StandardSpec::new(c);
        spec.set_size(size);
        assert_eq!(spec.size, size, "{name}: size {size}");
        let ps = standard::part_studio(&spec).unwrap();
        assert_eq!(ps.features.iter().filter(|f| f.type_id == "mateConnector").count(), 1, "{name}: one connector");
        let r = standard::regenerate(&spec).unwrap_or_else(|e| panic!("{name} {size}: {e}"));
        let b = r.context.body(&standard::part_body(&r).unwrap()).unwrap();
        let area: f64 = (0..b.faces.len()).map(|i| b.shape.sub_shape(noshape_kernel::ShapeType::Face, i).unwrap().area().unwrap()).sum();
        assert_eq!(b.faces.len(), faces, "{name} {size}");
        assert!((area / live_area - 1.0).abs() < 1e-4, "{name} {size}: area {area} live {live_area}");
    }
}

/// ANSI internal/external tooth lock washers (live's raw /features): 8
/// inner slots and 16 (Type A) / 12 (Type B) outer teeth; the Outside
/// diameter select (FSRules DATA_AI_OUTSIDE_DIAMETER) sets OD and thickness.
/// Faces and volume (m³) of live's B-rep (bodydetails).
#[test]
#[ignore = "T2: standard content vs live (P3 long tail; slow)"]
fn internal_external_tooth_lock_washers_match_live() {
    use super::standard::{self, CATALOGUE, StandardSpec};
    type Case<'a> = (&'a str, &'a str, Option<&'a str>, usize, Option<f64>);
    let cases: [Case; 3] = [
        ("Type A internal/external tooth lock washer", "#4", None, 98, Some(3.793605138737692e-8)),
        ("Type A internal/external tooth lock washer", "5/8", Some("1.975"), 98, Some(1.990116153548997e-6)),
        ("Type B internal/external tooth lock washer", "#4", None, 82, None),
    ];
    for (name, size, od, faces, live_volume) in cases {
        let c = CATALOGUE.iter().find(|c| c.name == name).unwrap_or_else(|| panic!("{name} in the catalogue"));
        let mut spec = StandardSpec::new(c);
        spec.set_size(size);
        if let Some(od) = od {
            spec.params.insert("Outside diameter".into(), od.into());
        }
        assert_eq!(spec.param("Outside diameter"), od.unwrap_or("0.475"));
        let r = standard::regenerate(&spec).unwrap_or_else(|e| panic!("{name} {size}: {e}"));
        let b = r.context.body(&standard::part_body(&r).unwrap()).unwrap();
        assert_eq!(b.faces.len(), faces, "{name} {size}");
        if let Some(v) = live_volume {
            let vol = b.shape.volume().unwrap();
            assert!((vol / v - 1.0).abs() < 1e-4, "{name} {size}: volume {vol} live {v}");
        }
    }
}

/// ANSI tooth lock washers from live's recipes (raw /features): straight
/// slots on radial lines, external from 1.05 (ID + OD) / 2, internal from the
/// bore to (ID + OD) / 2; faces and total area (m²) at live's sizes.
#[test]
#[ignore = "T2: standard content vs live (P3 long tail; slow)"]
fn tooth_lock_washers_match_live() {
    use super::standard::{self, CATALOGUE, StandardSpec};
    let cases: [(&str, &str, usize, f64); 10] = [
        ("Type A external tooth lock washer", "#3", 51, 4.713456e-5),
        ("Type A external tooth lock washer", "1", 51, 1.617386e-3),
        ("Type B external tooth lock washer", "#3", 43, 4.601603e-5),
        ("Type B external tooth lock washer", "1", 43, 1.599059e-3),
        ("Type A internal tooth lock washer", "#2", 43, 4.136907e-5),
        ("Type A internal tooth lock washer", "1-1/4", 43, 2.327614e-3),
        ("Type A heavy internal tooth lock washer", "1/4", 43, 2.951435e-4),
        ("Type A heavy internal tooth lock washer", "7/8", 43, 1.553333e-3),
        ("Type B internal tooth lock washer", "#2", 39, 4.083287e-5),
        ("Type B internal tooth lock washer", "1-1/4", 39, 2.313777e-3),
    ];
    for (name, size, faces, live_area) in cases {
        let c = CATALOGUE.iter().find(|c| c.name == name).unwrap_or_else(|| panic!("{name} in the catalogue"));
        let mut spec = StandardSpec::new(c);
        spec.set_size(size);
        assert_eq!(spec.size, size, "{name}: size {size}");
        let r = standard::regenerate(&spec).unwrap_or_else(|e| panic!("{name} {size}: {e}"));
        let b = r.context.body(&standard::part_body(&r).unwrap()).unwrap();
        let area: f64 = (0..b.faces.len()).map(|i| b.shape.sub_shape(noshape_kernel::ShapeType::Face, i).unwrap().area().unwrap()).sum();
        assert_eq!(b.faces.len(), faces, "{name} {size}");
        assert!((area / live_area - 1.0).abs() < 1e-3, "{name} {size}: area {area} live {live_area}");
    }
}

/// Acorn high / low crown nuts 1/4-20 (live dom/skb-sc-geom, default): 26 faces
/// (CONE 14, PLANE 8, CYLINDER 2, TORUS 1, SPHERE 1); the crown's arc of
/// BODY_RADIUS is a spindle torus whose lemon part Parasolid reports with a
/// negative major radius (high: -3.429 / 8.636 mm, low: -1.143 / 6.35 mm), the
/// nose a sphere (1.524 / 2.794 mm).
#[test]
#[ignore = "T2: standard content vs live (P3 long tail; slow)"]
fn acorn_nut_crown_is_a_torus_like_live() {
    use super::standard::{self, CATALOGUE, StandardSpec};
    for (name, major, minor, nose) in [("Acorn high crown nut", -3.429, 8.636, 1.524), ("Acorn low crown nut", -1.143, 6.35, 2.794)] {
        let c = CATALOGUE.iter().find(|c| c.name == name).unwrap_or_else(|| panic!("{name} in the catalogue"));
        let mut spec = StandardSpec::new(c);
        spec.set_size("1/4-20");
        let r = standard::regenerate(&spec).unwrap_or_else(|e| panic!("{name}: {e}"));
        let body = r.context.body(&standard::part_body(&r).unwrap()).unwrap();
        let surfaces: Vec<noshape_kernel::Surface> = (0..body.faces.len()).filter_map(|i| body.shape.sub_shape(noshape_kernel::ShapeType::Face, i).ok()?.face_surface().ok()).collect();
        let mut kinds: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
        for s in &surfaces {
            *kinds.entry(format!("{s:?}").split([' ', '(']).next().unwrap_or_default().to_string()).or_default() += 1;
        }
        let want: std::collections::BTreeMap<String, usize> = [("Cone", 14), ("Plane", 8), ("Cylinder", 2), ("Torus", 1), ("Sphere", 1)].iter().map(|(k, n)| (k.to_string(), *n)).collect();
        assert_eq!(kinds, want, "{name}");
        let torus = surfaces.iter().find_map(|s| match s {
            noshape_kernel::Surface::Torus { major_radius, minor_radius, .. } => Some((*major_radius * 1e3, *minor_radius * 1e3)),
            _ => None,
        });
        let (a, b) = torus.unwrap();
        assert!((a - major).abs() < 1e-3 && (b - minor).abs() < 1e-3, "{name}: torus {a} / {b} mm, live {major} / {minor}");
        let sphere = surfaces.iter().find_map(|s| if let noshape_kernel::Surface::Sphere { radius, .. } = s { Some(*radius * 1e3) } else { None }).unwrap();
        assert!((sphere - nose).abs() < 1e-3, "{name}: nose {sphere} mm, live {nose}");
    }
}

/// DIN 985 nylon-insert thin nuts (qa: missing from the catalogue): live names
/// its sizes with the pitch ("M3x0.5", fine "M8x1", "M48X3"); each takes the
/// published table's row of its diameter (DIN 985:1987). Every size builds;
/// M8 is s 13 across the flats and 8 high (h max) as the table says.
#[test]
#[ignore = "T2: standard content vs live (P3 long tail; slow)"]
fn din_985_nuts_build_every_live_size() {
    use super::standard::{self, CATALOGUE, StandardSpec};
    let c = CATALOGUE.iter().find(|c| c.name == "Prevailing torque hex thin nut grade A & B DIN 985").expect("DIN 985 in the catalogue");
    let names: Vec<&str> = c.sizes.iter().map(|s| s.name.as_str()).collect();
    for want in ["M2x0.4", "M2.5x0.45", "M3x0.5", "M8x1.25", "M8x1", "M48X3"] {
        assert!(names.contains(&want), "{want} in {names:?}");
    }
    // Live's B-rep (behaviours "skb (10-25) DIN 985"): 27 faces, their total
    // area and the volume (M48: the kernel's own integration once skipped
    // the internal edge the tangent collar leaves in each flat, -16 mm³).
    for (size, area, volume) in [("M2x0.4", 71.7929, Some(27.4414)), ("M8x1.25", 713.9754, Some(819.7291)), ("M8x1", 716.1122, Some(801.2191)), ("M48X3", 24805.5791, Some(149094.5953))] {
        let mut spec = StandardSpec::new(c);
        spec.set_size(size);
        let r = standard::regenerate(&spec).unwrap();
        let b = r.context.body(&standard::part_body(&r).unwrap()).unwrap();
        assert_eq!(b.faces.len(), 27, "{size}");
        let total: f64 = (0..b.faces.len()).map(|i| b.shape.sub_shape(noshape_kernel::ShapeType::Face, i).unwrap().area().unwrap() * 1e6).sum();
        assert!((total - area).abs() < area * 1e-6 + 1e-3, "{size}: {total} mm², live {area}");
        if let Some(volume) = volume {
            let v = b.shape.volume().unwrap() * 1e9;
            assert!((v - volume).abs() < f64::max(1e-3, volume * 1e-7), "{size}: {v} mm³, live {volume}");
        }
    }
    let mut failures = Vec::new();
    for size in &names {
        let mut spec = StandardSpec::new(c);
        spec.set_size(size);
        match standard::regenerate(&spec) {
            Ok(r) if r.display.features.iter().all(|f| f.message.is_none()) => {}
            Ok(r) => failures.push(format!("{size}: {:?}", r.display.features.iter().filter_map(|f| f.message.clone()).collect::<Vec<_>>())),
            Err(e) => failures.push(format!("{size}: {e}")),
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
    let mut spec = StandardSpec::new(c);
    spec.set_size("M8x1.25");
    let r = standard::regenerate(&spec).unwrap();
    let bb = r.context.body(&standard::part_body(&r).unwrap()).unwrap().shape.bounding_box(true).unwrap();
    let (dx, dz) = ((bb.max.x - bb.min.x).min(bb.max.y - bb.min.y), bb.max.z - bb.min.z);
    assert!((dx - 0.013).abs() < 1e-6, "across flats {dx}");
    assert!((dz - 0.008).abs() < 1e-6, "height {dz}");
}

/// ISO 4762 socket head cap screw M6 x 20 against live's B-rep (dom/skb-sc-geom
/// Hex_socket_head_cap_screw_ISO_4762_M6, behaviours "skb (10-25) ISO 4762 /
/// ISO 10642 tables"): 23 faces, 755.5717 mm²; the head's top edge a 0.6 mm
/// torus, and under the head a cone from Ø6.8 over 0.68 mm (30.47°) with a
/// 0.25 mm torus at each end (majors 3.54299 and 3.25).
#[test]
#[ignore = "T2: standard content vs live (P3 long tail; slow)"]
fn iso_4762_cap_screw_matches_live() {
    use super::standard::{self, CATALOGUE, StandardSpec};
    use noshape_kernel::Surface;
    let c = CATALOGUE.iter().find(|c| c.name == "Hex socket head cap screw ISO 4762").unwrap();
    let mut spec = StandardSpec::new(c);
    spec.set_size("M6");
    spec.length = Some("20".into());
    let r = standard::regenerate(&spec).unwrap();
    let b = r.context.body(&standard::part_body(&r).unwrap()).unwrap();
    let (mut total, mut tori, mut cones) = (0.0, 0, 0);
    for i in 0..b.faces.len() {
        let f = b.shape.sub_shape(noshape_kernel::ShapeType::Face, i).unwrap();
        total += f.area().unwrap() * 1e6;
        match f.face_surface().unwrap() {
            Surface::Torus { .. } => tori += 1,
            Surface::Cone { .. } => cones += 1,
            _ => {}
        }
    }
    assert_eq!((b.faces.len(), tori, cones), (23, 3, 3));
    assert!((total - 755.5717).abs() < 1e-3, "{total} mm²");
    // Every size of live's table builds one solid.
    let mut failures = Vec::new();
    for size in c.sizes.iter().map(|s| s.name.clone()) {
        let mut spec = StandardSpec::new(c);
        spec.set_size(&size);
        match standard::regenerate(&spec) {
            Ok(r) if standard::part_body(&r).is_some() => {}
            Ok(r) => failures.push(format!("{size}: {:?}", r.display.features.iter().filter_map(|f| f.message.clone()).collect::<Vec<_>>())),
            Err(e) => failures.push(format!("{size}: {e}")),
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

/// ISO 10642 socket countersunk head M6 x 20 against live's B-rep
/// (dom/skb-sc-geom Hex_socket_countersunk_head_screw_ISO_10642_M6): 20
/// faces, 601.1356 mm²; a Ø11.34 head with a 1.05 mm cylindrical rim over
/// the 90 degree cone, which a 0.25 mm torus (major 3.25) rounds into the
/// shank.
#[test]
#[ignore = "T2: standard content vs live (P3 long tail; slow)"]
fn iso_10642_countersunk_screw_matches_live() {
    use super::standard::{self, CATALOGUE, StandardSpec};
    let c = CATALOGUE.iter().find(|c| c.name == "Hex socket countersunk head screw ISO 10642").unwrap();
    let mut spec = StandardSpec::new(c);
    spec.set_size("M6");
    spec.length = Some("20".into());
    let r = standard::regenerate(&spec).unwrap();
    let b = r.context.body(&standard::part_body(&r).unwrap()).unwrap();
    let total: f64 = (0..b.faces.len()).map(|i| b.shape.sub_shape(noshape_kernel::ShapeType::Face, i).unwrap().area().unwrap() * 1e6).sum();
    assert_eq!(b.faces.len(), 20);
    assert!((total - 601.1356).abs() < 1e-3, "{total} mm²");
    let mut failures = Vec::new();
    for size in c.sizes.iter().map(|s| s.name.clone()) {
        let mut spec = StandardSpec::new(c);
        spec.set_size(&size);
        match standard::regenerate(&spec) {
            Ok(r) if standard::part_body(&r).is_some() => {}
            Ok(r) => failures.push(format!("{size}: {:?}", r.display.features.iter().filter_map(|f| f.message.clone()).collect::<Vec<_>>())),
            Err(e) => failures.push(format!("{size}: {e}")),
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

/// Metric hex socket set screws against live's B-rep (behaviours "skb
/// (10-25) set screw sockets"): 12 faces each, a flat-bottomed socket
/// (shallow A on short screws), a 30 degree chamfer from the minor diameter
/// on top, and the flat or 120 degree cup point; live's volumes.
#[test]
fn iso_set_screws_match_live() {
    use super::standard::{self, CATALOGUE, StandardSpec};
    for (name, size, len, volume) in [
        ("Flat point hex socket set screw ISO 4026", "M3", "4", 24.7018),
        ("Flat point hex socket set screw ISO 4026", "M6", "6", 144.1804),
        ("Flat point hex socket set screw ISO 4026", "M6", "30", 811.0731),
        ("Flat point hex socket set screw ISO 4026", "M12", "40", 4213.1173),
        ("Cup point hex socket set screw ISO 4029", "M6", "30", 799.7387),
    ] {
        let c = CATALOGUE.iter().find(|c| c.name == name).unwrap();
        let mut spec = StandardSpec::new(c);
        spec.set_size(size);
        spec.length = Some(len.into());
        let r = standard::regenerate(&spec).unwrap();
        let b = r.context.body(&standard::part_body(&r).unwrap()).unwrap();
        let v = b.shape.volume().unwrap() * 1e9;
        assert_eq!(b.faces.len(), 12, "{name} {size}x{len}");
        assert!((v - volume).abs() < 1e-3, "{name} {size}x{len}: {v} mm³, live {volume}");
    }
}

/// ASME socket head cap screw 1/4-20 x 1 against live's B-rep (dom/skb-sc-geom
/// Socket_head_cap_screw_1-4-20, captured with the drilled(6) default: 27
/// faces, 909.9164 mm²): the 0.025 in 45 degree head chamfer, the 0.007 in
/// fillet torus under the head, the socket with its 120 degree drill point;
/// the plain Hex screw 21 faces.
#[test]
#[ignore = "T2: standard content vs live (P3 long tail; slow)"]
fn asme_socket_head_cap_screw_matches_live() {
    use super::standard::{self, CATALOGUE, StandardSpec};
    let c = CATALOGUE.iter().find(|c| c.name == "Socket head cap screw" && c.inch).unwrap();
    let mut spec = StandardSpec::new(c);
    spec.set_size("1/4-20");
    spec.length = Some("1".into());
    let r = standard::regenerate(&spec).unwrap();
    let b = r.context.body(&standard::part_body(&r).unwrap()).unwrap();
    assert_eq!(b.faces.len(), 21);
    // Live's Drive type (REST FeatureScript on the component, DrillHole2/4/6):
    // Hex 21 faces 1178.4515 mm³; drilled(2) one hole, drilled(6) three.
    for (drive, faces, volume) in [("Hex", 21, 1178.4515), ("Hex drilled(2)", 23, 1173.2688), ("Hex drilled(6)", 27, 1162.9034)] {
        let mut spec = StandardSpec::new(c);
        spec.set_size("1/4-20");
        spec.length = Some("1".into());
        spec.params.insert("Drive type".into(), drive.into());
        let r = standard::regenerate(&spec).unwrap();
        let b = r.context.body(&standard::part_body(&r).unwrap()).unwrap();
        let v = b.shape.volume().unwrap() * 1e9;
        assert_eq!(b.faces.len(), faces, "{drive}");
        // The capture is the drilled(6) screw: all 27 faces, 909.9164 mm².
        if drive == "Hex drilled(6)" {
            let total: f64 = (0..b.faces.len()).map(|i| b.shape.sub_shape(noshape_kernel::ShapeType::Face, i).unwrap().area().unwrap() * 1e6).sum();
            assert!((total - 909.9164).abs() < 1e-3, "{total} mm²");
        }
        // (Mass properties agree to a few 1e-6 of the volume with the holes'
        // cylinder intersections.)
        assert!((v - volume).abs() < volume * 5e-6, "{drive}: {v} mm³");
    }
    // A size without drilled heads offers only Hex.
    assert_eq!(c.size("#0-80").unwrap().values.get("Drive type").map(Vec::as_slice), Some(&["Hex".to_string()][..]));
}

/// ASME socket button and countersunk head cap screws against live's B-rep
/// and mass properties (dom/skb-sc-geom, behaviours "skb (10-26) ASME socket
/// button / countersunk heads"). Button: a sphere through the rim on top of
/// the HEAD_SIDE_HEIGHT cylinder, cut flat HEAD_DIAMETER / 2 across at the
/// head height; countersunk: an 82 degree cone tangent to the under-head
/// torus, its socket ending in a 140 degree drill point (the button's: 120).
/// Both end the bearing face at FILLET_TRANSITION_DIAMETER in a torus of
/// radius (FILLET_TRANSITION_DIAMETER - d) / 2.
#[test]
#[ignore = "T2: standard content vs live (P3 long tail; slow)"]
fn asme_socket_button_and_countersunk_screws_match_live() {
    use super::standard::{self, CATALOGUE, StandardSpec};
    for (name, faces, cases) in [
        ("Socket button head cap screw", 21, [("#4-40", "0.5", 174.1654, 103.7091), ("1/4-20", "1", 777.9013, 1006.6053), ("1/2-13", "1.5", 2611.3755, 6423.2130)]),
        ("Socket countersunk head cap screw", 20, [("#4-40", "0.5", 166.5982, 97.2179), ("1/4-20", "1", 759.1454, 936.6143), ("1/2-13", "1.5", 2328.2045, 5392.5206)]),
    ] {
        let c = CATALOGUE.iter().find(|c| c.name == name && c.inch).unwrap();
        for (size, length, area, volume) in cases {
            let mut spec = StandardSpec::new(c);
            spec.set_size(size);
            spec.length = Some(length.into());
            let r = standard::regenerate(&spec).unwrap();
            let b = r.context.body(&standard::part_body(&r).unwrap()).unwrap();
            assert_eq!(b.faces.len(), faces, "{name} {size}");
            let total: f64 = (0..b.faces.len()).map(|i| b.shape.sub_shape(noshape_kernel::ShapeType::Face, i).unwrap().area().unwrap() * 1e6).sum();
            let v = b.shape.volume().unwrap() * 1e9;
            assert!((total - area).abs() < area * 1e-6, "{name} {size}: {total} mm²");
            assert!((v - volume).abs() < volume * 1e-6, "{name} {size}: {v} mm³");
        }
    }
}

/// ISO 7380 socket button head M6 x 20 against live's B-rep (dom/skb-sc-geom
/// M6x20, behaviours "skb (10-25) ISO 7380 dome"): 20 faces; the dome is a
/// sphere through the rim, cut flat at the head height (top plane out to
/// HEAD_TOP_DIA / 2 = 3 mm): sphere 111.4929, top 14.4179, hex walls 4.8036
/// each; 624.6237 mm² in all and 739.6215 mm³.
#[test]
fn iso_7380_button_head_matches_live() {
    use super::standard::{self, CATALOGUE, StandardSpec};
    use noshape_kernel::Surface;
    let c = CATALOGUE.iter().find(|c| c.name == "Socket button head screw ISO 7380").unwrap();
    // Live's table brings M2, M2.5 and M18 too.
    for size in ["M2", "M2.5", "M18"] {
        assert!(c.size(size).is_some(), "{size}");
    }
    let mut spec = StandardSpec::new(c);
    spec.set_size("M6");
    spec.length = Some("20".into());
    let r = standard::regenerate(&spec).unwrap();
    let b = r.context.body(&standard::part_body(&r).unwrap()).unwrap();
    assert_eq!(b.faces.len(), 20);
    let (mut total, mut sphere) = (0.0, 0.0);
    for i in 0..b.faces.len() {
        let f = b.shape.sub_shape(noshape_kernel::ShapeType::Face, i).unwrap();
        let a = f.area().unwrap() * 1e6;
        total += a;
        if matches!(f.face_surface().unwrap(), Surface::Sphere { .. }) {
            sphere += a;
        }
    }
    assert!((sphere - 111.4929).abs() < 1e-3, "sphere {sphere} mm²");
    assert!((total - 624.6237).abs() < 1e-3, "{total} mm²");
    let v = b.shape.volume().unwrap() * 1e9;
    assert!((v - 739.6215).abs() < 1e-3, "{v} mm³");
}




/// Live's texts (docs/parity/workflow-assembly-sweep.md rows 1–5, 7, 8, 10, 12, 13, 28):
/// relations on mates of the wrong kind, a relation whose mate is gone or
/// failed, and a mate with both connectors on one instance.
#[test]
fn relation_and_mate_errors_use_live_texts() {
    use super::eval::relation_values;
    let err = |k, a, b| relation_values(k, a, b).unwrap_err();
    assert_eq!(err(RelationType::Gear, MateType::Slider, MateType::Revolute), "Gear relations require two rotational degrees of freedom.");
    assert_eq!(err(RelationType::RackAndPinion, MateType::Revolute, MateType::Revolute), "Rack and pinion relations require one translational degree of freedom and one rotational degree of freedom.");
    assert_eq!(err(RelationType::Screw, MateType::Revolute, MateType::Revolute), "Screw relations require one cylindrical mate.");
    assert_eq!(err(RelationType::Linear, MateType::Revolute, MateType::Revolute), "Linear relations require two translational degrees of freedom.");
    assert!(relation_values(RelationType::Gear, MateType::Revolute, MateType::Cylindrical).is_ok());

    let f = doc_with(vec![plate_with_hole(), pin_studio()]);
    let mut ev = AssemblyEvaluator::new();
    let (plate, rp) = body_of(&mut ev, &f.doc, f.studios[0]);
    let (pin, rq) = body_of(&mut ev, &f.doc, f.studios[1]);
    let hole = cylinder_face(&rp);
    let pf = cylinder_face(&rq);
    let gear = |mates: [MateId; 2]| Relation { id: MateId(40), name: "Gear 1".into(), relation_type: RelationType::Gear, mates, ratio: ExprValue::new("1"), reverse: false, suppressed: false, base: None };
    let mut asm = Assembly {
        instances: vec![
            part_instance(1, "Plate <1>", f.studios[0], plate, DVec3::ZERO, true),
            part_instance(2, "Pin <1>", f.studios[1], pin.clone(), DVec3::ZERO, false),
            part_instance(3, "Pin <2>", f.studios[1], pin, DVec3::ZERO, false),
        ],
        mates: vec![
            mate(10, MateType::Revolute, conn(1, hole.clone(), Inference::MidAxis), conn(2, pf.clone(), Inference::MidAxis)),
            mate(11, MateType::Revolute, conn(1, hole, Inference::MidAxis), conn(3, pf.clone(), Inference::MidAxis)),
        ],
        relations: vec![gear([MateId(10), MateId(99)])],
        ..Default::default()
    };
    let res = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
    assert_eq!(res.relations[0].message.as_deref(), Some("One or more mates in this relation do not exist."));
    // A mated instance fixed too: the mate fails, so does the relation.
    asm.relations = vec![gear([MateId(10), MateId(11)])];
    asm.instance_mut(InstanceId(3)).unwrap().fixed = true;
    let res = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
    assert_eq!(res.relations[0].message.as_deref(), Some("One or more mates in this relation are invalid."));
    // Both connectors on one instance.
    asm.instance_mut(InstanceId(3)).unwrap().fixed = false;
    asm.relations.clear();
    asm.mates.push(mate(12, MateType::Fastened, conn(2, pf.clone(), Inference::MidAxis), conn(2, pf.clone(), Inference::Centroid)));
    let res = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
    let m12 = res.mates.iter().find(|m| m.id == MateId(12)).unwrap();
    assert_eq!(m12.message.as_deref(), Some("Mate connectors are on same instance."));
    // Two members of one group (row 13).
    asm.mates.pop();
    asm.groups.push(Group { id: MateId(50), name: "Group 1".into(), instances: vec![InstanceId(2), InstanceId(3)], suppressed: false });
    asm.mates.push(mate(13, MateType::Fastened, conn(2, pf.clone(), Inference::Centroid), conn(3, pf.clone(), Inference::Centroid)));
    let res = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
    let m13 = res.mates.iter().find(|m| m.id == MateId(13)).unwrap();
    assert_eq!(m13.message.as_deref(), Some("Mate cannot be added between member of same group."));
    // Tangent on the assembly Origin (row 10).
    asm.mates.pop();
    asm.groups.clear();
    asm.mates.push(mate(14, MateType::Tangent, conn(2, pf.clone(), Inference::Centroid), MateConnectorRef::on(InstanceId::ORIGIN, vec![], Reference::Origin)));
    let res = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
    let m14 = res.mates.iter().find(|m| m.id == MateId(14)).unwrap();
    assert_eq!(m14.message.as_deref(), Some("Geometry is not supported for tangent mate."));
    // A Width mate with one width connector (row 8).
    asm.mates.pop();
    asm.mates.push(mate(15, MateType::Width, conn(2, pf.clone(), Inference::Centroid), conn(1, cylinder_face(&rp), Inference::Centroid)));
    let res = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
    let m15 = res.mates.iter().find(|m| m.id == MateId(15)).unwrap();
    assert_eq!(m15.message.as_deref(), Some("Two width mate connectors are required."));
    // The pin's part gone from its Part Studio (row 28, live E15): the
    // instance and the mate on its face.
    asm.mates.pop();
    let lost = BodyId { feature: crate::reference::FeatureId(9999), index: 0 };
    if let InstanceSource::Part { body, .. } = &mut asm.instance_mut(InstanceId(3)).unwrap().source {
        *body = lost;
    }
    let res = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
    let node = res.nodes.iter().find(|n| n.path == vec![InstanceId(3)]).expect("the instance row");
    assert_eq!(node.error.as_deref(), Some("Failed to resolve instance."));
    let m11 = res.mates.iter().find(|m| m.id == MateId(11)).unwrap();
    assert_eq!(m11.message.as_deref(), Some("Failed to resolve the instance for mate."));
}

/// Random assembly edits (seeded): mates of every type between random faces
/// and edges with random inferences, deleted mates, fixes, suppressions,
/// moves, drags and the mate-being-defined solve, each evaluated. Nothing
/// may panic and every solved placement stays finite. `ASM_FUZZ_SEEDS=a..b`
/// / `ASM_FUZZ_STEPS=n` widen it.
#[test]
#[ignore = "T2: fuzz"]
fn random_assembly_edits_never_panic() {
    let f = doc_with(vec![block_studio(), plate_with_hole(), pin_studio()]);
    let mut ev = AssemblyEvaluator::new();
    let parts: Vec<(TabId, BodyId, Vec<Reference>)> = f
        .studios
        .iter()
        .map(|&s| {
            let (b, r) = body_of(&mut ev, &f.doc, s);
            let body = &r.context.bodies[0];
            let refs = body.faces.iter().map(|n| Reference::Face((**n).clone())).chain(body.edges.iter().map(|n| Reference::Edge((**n).clone()))).collect();
            (s, b, refs)
        })
        .collect();
    let range = std::env::var("ASM_FUZZ_SEEDS").ok().and_then(|s| s.split_once("..").and_then(|(a, b)| Some((a.parse::<u64>().ok()?, b.parse::<u64>().ok()?)))).unwrap_or((1, 4));
    let steps: usize = std::env::var("ASM_FUZZ_STEPS").ok().and_then(|s| s.parse().ok()).unwrap_or(60);
    const TYPES: [MateType; 11] = [MateType::Fastened, MateType::Revolute, MateType::Slider, MateType::Planar, MateType::Cylindrical, MateType::PinSlot, MateType::Ball, MateType::Parallel, MateType::Tangent, MateType::Width, MateType::Fastened];
    const INFS: [Inference; 7] = [Inference::Centroid, Inference::Center, Inference::Midpoint, Inference::Vertex, Inference::TopAxis, Inference::MidAxis, Inference::BottomAxis];
    for seed in range.0..range.1 {
        eprintln!("seed {seed}");
        let mut state = seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1;
        let mut rnd = |n: usize| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state % n as u64) as usize
        };
        let mut asm = Assembly::default();
        let mut next = 1u64;
        let mut log = Vec::new();
        for step in 0..steps {
            let t_step = std::time::Instant::now();
            let n = asm.instances.len();
            match if n < 2 { 0 } else { rnd(10) } {
                0 | 1 => {
                    let (s, b, _) = &parts[rnd(parts.len())];
                    let at = DVec3::new(rnd(200) as f64 - 100.0, rnd(200) as f64 - 100.0, rnd(100) as f64) * 1e-3;
                    asm.instances.push(part_instance(next, &format!("P <{next}>"), *s, b.clone(), at, n == 0));
                    log.push(format!("insert {next}"));
                    next += 1;
                }
                2..=4 => {
                    let t = TYPES[rnd(TYPES.len())];
                    let mut pick = |asm: &Assembly| {
                        let i = &asm.instances[rnd(asm.instances.len())];
                        let refs = &parts.iter().find(|p| matches!(&i.source, InstanceSource::Part { studio, .. } if *studio == p.0)).unwrap().2;
                        conn(i.id.0, refs[rnd(refs.len())].clone(), INFS[rnd(INFS.len())])
                    };
                    let (a, b) = (pick(&asm), pick(&asm));
                    let mut m = mate(next, t, a, b);
                    if t == MateType::Width {
                        m.extra = vec![pick(&asm), pick(&asm)];
                    }
                    m.flip_primary = rnd(2) == 0;
                    log.push(format!("mate {next} {t:?}"));
                    asm.mates.push(m);
                    next += 1;
                }
                5 if !asm.mates.is_empty() => {
                    let k = rnd(asm.mates.len());
                    log.push(format!("delete mate {}", asm.mates[k].id.0));
                    asm.mates.remove(k);
                }
                6 => {
                    let k = rnd(n);
                    asm.instances[k].fixed = !asm.instances[k].fixed;
                    log.push(format!("fix {}", asm.instances[k].id.0));
                }
                7 => {
                    let k = rnd(n);
                    asm.instances[k].suppressed = rnd(3) == 0;
                    log.push(format!("suppress {}", asm.instances[k].id.0));
                }
                _ => {}
            }
            let mut opts = SolveOptions::default();
            match rnd(4) {
                0 if !asm.instances.is_empty() => {
                    let i = asm.instances[rnd(asm.instances.len())].id;
                    opts.drags.push((i, DVec3::ZERO, DVec3::new(rnd(100) as f64, rnd(100) as f64, rnd(100) as f64) * 1e-3));
                }
                1 if !asm.mates.is_empty() => opts.only_mate = Some(asm.mates[rnd(asm.mates.len())].id),
                _ => {}
            }
            let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| ev.evaluate(&f.doc, &asm, &opts, None)));
            let Ok(res) = res else { panic!("seed {seed} step {step} panicked; log: {log:?}") };
            if t_step.elapsed().as_secs_f64() > 2.0 {
                eprintln!("seed {seed} step {step}: evaluation took {:.1} s; log: {log:?}", t_step.elapsed().as_secs_f64());
            }
            for (id, p) in &res.solved.placements {
                assert!(p.translation.iter().chain(p.rotation.iter()).all(|v| v.is_finite()), "seed {seed} step {step}: {id:?} at {p:?}; log: {log:?}");
            }
            // Keep the solve, as the app does after an edit.
            for (id, p) in res.solved.placements.clone() {
                if let Some(i) = asm.instance_mut(id) {
                    i.placement = p;
                }
            }
        }
    }
}

/// Live (docs/parity/workflow-assembly-sweep.md row 9, E16): a mate
/// connector whose secondary axis is parallel to its primary axis fails with
/// Onshape's text.
#[test]
fn secondary_axis_parallel_to_the_primary_fails_as_live() {
    let f = doc_with(vec![block_studio()]);
    let mut ev = AssemblyEvaluator::new();
    let (body, r) = body_of(&mut ev, &f.doc, f.studios[0]);
    let top = face_with(&r, "CapEnd");
    let mut mc = AsmMateConnector::new(MateId(20), "Mate connector 1".into(), conn(1, top.clone(), Inference::Centroid));
    mc.secondary_axis = Some(conn(1, top, Inference::Centroid));
    let asm = Assembly { instances: vec![part_instance(1, "Block <1>", f.studios[0], body, DVec3::ZERO, true)], connectors: vec![mc], ..Default::default() };
    let res = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
    assert_eq!(res.connectors[0].message.as_deref(), Some("Failed to resolve mate connector. Secondary axis conflicts with primary axis."));
}

/// Live (docs/parity/workflow-assembly-sweep.md row 11): Replicate on a
/// plate whose only hole is the seed's says so in Onshape's words.
#[test]
fn replicate_with_nothing_to_match_says_so_as_live() {
    let f = doc_with(vec![plate_with_hole(), pin_studio()]);
    let mut ev = AssemblyEvaluator::new();
    let (plate, rp) = body_of(&mut ev, &f.doc, f.studios[0]);
    let (pin, rq) = body_of(&mut ev, &f.doc, f.studios[1]);
    let mut asm = Assembly {
        instances: vec![part_instance(1, "Plate <1>", f.studios[0], plate, DVec3::ZERO, true), part_instance(2, "Pin <1>", f.studios[1], pin, DVec3::new(0.0, 0.05, 0.0), false)],
        mates: vec![mate(10, MateType::Revolute, conn(1, cylinder_face(&rp), Inference::MidAxis), conn(2, cylinder_face(&rq), Inference::MidAxis))],
        ..Default::default()
    };
    let res = ev.evaluate(&f.doc, &asm, &SolveOptions::default(), None);
    asm.patterns.push(InstancePattern { id: MateId(80), name: "Replicate 1".into(), kind: PatternKind::Replicate { mate: MateId(10), all_instances: false, scope: None, scope_refs: vec![] }, seeds: vec![InstanceId(2)], generated: vec![], suppressed: false });
    let err = analysis::matched_targets(&res, &asm, &asm.patterns[0], &UnitPrefs::default()).unwrap_err();
    assert_eq!(err, "Selected target did not contain identical geometry.");
}

/// ISO 10512 fine-pitch prevailing torque nuts (missing from the catalogue,
/// skasm 10-27): live's table (dom/skb-sc-comp) through DIN 985's
/// prevailing-torque nut with live's bore (0.74 s), 90 degree countersinks and
/// 30 degree corner chamfers. Live's B-rep of M8 x 1 (behaviours "skasm
/// (2026-10-27) standard content gaps"): 27 faces, volume 935.290 –
/// 935.619 mm³ (/massproperties LOW bounds).
#[test]
#[ignore = "T2: standard content vs live (P3 long tail; slow)"]
fn iso_10512_nut_matches_live() {
    use super::standard::{self, CATALOGUE, StandardSpec};
    let c = CATALOGUE.iter().find(|c| c.name == "Prevailing torque nut fine pitch ISO 10512").expect("ISO 10512 in the catalogue");
    let names: Vec<&str> = c.sizes.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, ["M8 x 1", "M10 x 1", "M10 x 1.25", "M12 x 1.25", "M12 x 1.5", "M16 x 1.5", "M20 x 1.5", "M24 x 2", "M30 x 2", "M36 x 2"]);
    let mut spec = StandardSpec::new(c);
    spec.set_size("M8 x 1");
    let r = standard::regenerate(&spec).unwrap();
    let b = r.context.body(&standard::part_body(&r).unwrap()).unwrap();
    let v = b.shape.volume().unwrap() * 1e9;
    eprintln!("ISO 10512 M8 x 1: {} faces, {v} mm³", b.faces.len());
    assert_eq!(b.faces.len(), 27);
    assert!((935.290..=935.619).contains(&v), "{v} mm³, live 935.290 – 935.619");
    let area: f64 = (0..b.faces.len()).map(|i| b.shape.sub_shape(noshape_kernel::ShapeType::Face, i).unwrap().area().unwrap() * 1e6).sum();
    assert!((area - 804.0370).abs() < 1e-2, "{area} mm², live 804.0370");
    for size in &names {
        let mut spec = StandardSpec::new(c);
        spec.set_size(size);
        let r = standard::regenerate(&spec).unwrap();
        assert!(r.display.features.iter().all(|f| f.message.is_none()), "{size}");
    }
}

/// DIN 6908 conical spring lock washers (missing from the catalogue, skasm
/// 10-27): live's table; live builds a flat ring (M2.25: 2 planes, 2
/// cylinders, 12.149 mm³, behaviours "skasm (2026-10-27) standard content
/// gaps").
#[test]
#[ignore = "T2: standard content vs live (P3 long tail; slow)"]
fn din_6908_washer_matches_live() {
    use super::standard::{self, CATALOGUE, StandardSpec};
    let c = CATALOGUE.iter().find(|c| c.name == "Conical spring lock washer DIN 6908").expect("DIN 6908 in the catalogue");
    let names: Vec<&str> = c.sizes.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, ["M2.25", "M2.75", "M3.2", "M3.6", "M4.55", "M5.5", "M7.4", "M9.3", "M11"]);
    let mut spec = StandardSpec::new(c);
    spec.set_size("M2.25");
    let r = standard::regenerate(&spec).unwrap();
    let b = r.context.body(&standard::part_body(&r).unwrap()).unwrap();
    let v = b.shape.volume().unwrap() * 1e9;
    assert_eq!(b.faces.len(), 4);
    assert!((12.144..=12.154).contains(&v), "{v} mm³, live 12.144 – 12.154");
}

/// ISO 10511 thin and ISO 7040 prevailing torque nuts on live's template
/// (behaviours "skasm (2026-10-27) standard content gaps", addendum 1488):
/// ISO 10512's (bore at the thread's minor diameter, 90 degree countersinks,
/// 30 degree corner chamfers on both faces, collar of diameter s with its
/// top rounded (h − m) 5/16 and its floor (h − m) 10/29 down) but the collar
/// bored 0.7 s as DIN 985. Live at the studios' defaults: 27 faces each;
/// M3 × 0.5 thin 69.461 – 69.504 mm³, 136.3028 mm²; M5 × 0.8 257.282 –
/// 257.396 mm³, 338.7503 mm².
#[test]
#[ignore = "T2: standard content vs live (P3 long tail; slow)"]
fn iso_10511_and_7040_nuts_match_live() {
    use super::standard::{self, CATALOGUE, StandardSpec};
    for (name, size, vol, live_area) in [("Prevailing torque nut thin ISO 10511", "M3", 69.461..=69.504, 136.3028), ("Prevailing torque nut ISO 7040", "M5", 257.282..=257.396, 338.7503)] {
        let c = CATALOGUE.iter().find(|c| c.name == name).expect(name);
        let mut spec = StandardSpec::new(c);
        spec.set_size(size);
        let r = standard::regenerate(&spec).unwrap();
        let b = r.context.body(&standard::part_body(&r).unwrap()).unwrap();
        let v = b.shape.volume().unwrap() * 1e9;
        let area: f64 = (0..b.faces.len()).map(|i| b.shape.sub_shape(noshape_kernel::ShapeType::Face, i).unwrap().area().unwrap() * 1e6).sum();
        eprintln!("{name} {size}: {} faces, {v} mm³, {area} mm²", b.faces.len());
        assert_eq!(b.faces.len(), 27, "{name}");
        assert!(vol.contains(&v), "{name}: {v} mm³, live {vol:?}");
        assert!((area - live_area).abs() < 1e-2, "{name}: {area} mm², live {live_area}");
        for s in &c.sizes {
            let mut spec = StandardSpec::new(c);
            spec.set_size(&s.name);
            let r = standard::regenerate(&spec).unwrap();
            assert!(r.display.features.iter().all(|f| f.message.is_none()), "{name} {}", s.name);
        }
    }
}

/// Live's ANSI commercial taper pin B-rep (behaviours "skasm (2026-10-27)
/// standard content gaps (addendum): ANSI Commercial class taper pin B-rep",
/// request 1504): crowned ends (spheres of END_CROWN_RADIUS) and the 1:48
/// cone, three faces, centred on the origin along Z; volume and face areas.
#[test]
#[ignore = "T2: standard content vs live (P3 long tail; slow)"]
fn ansi_taper_pin_has_live_crowned_ends() {
    use super::standard::{self, CATALOGUE, StandardSpec};
    let c = CATALOGUE.iter().find(|c| c.name == "Commercial class taper pin").unwrap();
    for (size, length, vol, areas, zmax) in [("#7/0", "0.25", 11.855162..=11.865563, [1.73483, 2.08252, 30.35146], 3.34111), ("#4", "1", 761.465751..=761.632538, [28.04676, 33.74725, 485.6233], 13.44461)] {
        let mut spec = StandardSpec::new(c);
        spec.set_size(size);
        spec.length = Some(length.into());
        assert_eq!(spec.part_name(), format!("Taper pin {size} x {length}"));
        let r = standard::regenerate(&spec).unwrap();
        let b = r.context.body(&standard::part_body(&r).unwrap()).unwrap();
        let v = b.shape.volume().unwrap() * 1e9;
        let mut a: Vec<f64> = (0..b.faces.len()).map(|i| b.shape.sub_shape(noshape_kernel::ShapeType::Face, i).unwrap().area().unwrap() * 1e6).collect();
        a.sort_by(f64::total_cmp);
        let bb = b.shape.bounding_box(true).unwrap();
        eprintln!("{size} x {length}: {} faces {a:?}, {v} mm³, z {} .. {}", b.faces.len(), bb.min.z * 1e3, bb.max.z * 1e3);
        assert_eq!(b.faces.len(), 3, "{size}");
        assert!(vol.contains(&v), "{size}: {v} mm³, live {vol:?}");
        for (x, live) in a.iter().zip(areas) {
            assert!((x - live).abs() < 1e-3 * live, "{size}: face areas {a:?}, live {areas:?}");
        }
        assert!((bb.max.z * 1e3 - zmax).abs() < 1e-3 && (bb.min.z * 1e3 + zmax).abs() < 1e-3, "{size}: z {} .. {}, live +-{zmax}", bb.min.z * 1e3, bb.max.z * 1e3);
    }
}

/// DIN 923 slotted pan head screw with shoulder as live's component (behaviours
/// "skasm (2026-10-27) standard content gaps": M1.4 x 0.5 has 19 faces,
/// PLANE 8, CONE 4, CYLINDER 4, TORUS 3, 11.8897749 mm³ in 11.885 – 11.895);
/// the shoulder end at z = 0, the head above, the thread below; every size
/// regenerates.
#[test]
fn din_923_shoulder_screw_matches_live() {
    use super::standard::{self, CATALOGUE, StandardSpec};
    use noshape_kernel::{ShapeType, Surface};
    let c = CATALOGUE.iter().find(|c| c.name == "Slotted pan head screw with shoulder DIN 923").unwrap();
    assert_eq!(c.length_label(), "Shoulder length");
    let mut spec = StandardSpec::new(c);
    spec.set_size("M1.4");
    spec.length = Some("0.5".into());
    assert_eq!(spec.part_name(), "Slotted pan head screw with shoulder M1.4x0.30 x 0.5");
    let r = standard::regenerate(&spec).unwrap();
    let b = r.context.body(&standard::part_body(&r).unwrap()).unwrap();
    let v = b.shape.volume().unwrap() * 1e9;
    let mut kinds = std::collections::BTreeMap::new();
    for i in 0..b.faces.len() {
        let k = match b.shape.sub_shape(ShapeType::Face, i).unwrap().face_surface().unwrap() {
            Surface::Plane(_) => "plane",
            Surface::Cone { .. } => "cone",
            Surface::Cylinder { .. } => "cylinder",
            Surface::Torus { .. } => "torus",
            _ => "other",
        };
        *kinds.entry(k).or_insert(0) += 1;
    }
    let bb = b.shape.bounding_box(true).unwrap();
    assert_eq!(kinds, [("cone", 4), ("cylinder", 4), ("plane", 8), ("torus", 3)].into_iter().collect(), "{v} mm³");
    assert!((11.885..=11.895).contains(&v), "{v} mm³, live 11.8897749");
    assert!((bb.max.z * 1e3 - 1.4).abs() < 1e-6 && (bb.min.z * 1e3 + 2.2).abs() < 1e-6, "z {} .. {}", bb.min.z * 1e3, bb.max.z * 1e3);
    for s in &c.sizes {
        let mut spec = StandardSpec::new(c);
        spec.set_size(&s.name);
        let r = standard::regenerate(&spec).unwrap();
        assert!(standard::part_body(&r).is_some(), "{}", s.name);
    }
}

/// ANSI commercial taper pins and slotted spring pins with live's whole size
/// lists (its FSRules tables, behaviours "skasm (2026-10-27) standard content
/// gaps": taper pins from #7/0, spring pins with 9/64, 5/8, 3/4); every size
/// regenerates.
#[test]
#[ignore = "T2: standard content vs live (P3 long tail; slow)"]
fn ansi_taper_and_spring_pin_sizes_match_live() {
    use super::standard::{self, CATALOGUE, StandardSpec};
    for (name, live) in [
        ("Commercial class taper pin", &["#7/0", "#6/0", "#5/0", "#4/0", "#3/0", "#2/0", "#0", "#1", "#2", "#3", "#4", "#5", "#6", "#7", "#8", "#9", "#10", "#11", "#12", "#13", "#14"][..]),
        ("Slotted spring pin", &["1/16", "5/64", "3/32", "1/8", "9/64", "5/32", "3/16", "7/32", "1/4", "5/16", "3/8", "7/16", "1/2", "5/8", "3/4"][..]),
    ] {
        let c = CATALOGUE.iter().find(|c| c.name == name).expect(name);
        let names: Vec<&str> = c.sizes.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, live, "{name}");
        for s in &c.sizes {
            let mut spec = StandardSpec::new(c);
            spec.set_size(&s.name);
            let r = standard::regenerate(&spec).unwrap();
            assert!(r.display.features.iter().all(|f| f.message.is_none()), "{name} {}", s.name);
            assert!(standard::part_body(&r).is_some(), "{name} {}", s.name);
        }
    }
}

/// INSTANCE-NAME-STALE (live behaviours "Replace instance": the instance
/// name follows the part name + <n>): a renamed part renames its instances.
#[test]
fn instances_follow_their_part_name() {
    let mut f = doc_with(vec![block_studio()]);
    let mut ev = AssemblyEvaluator::new();
    let (body, _) = body_of(&mut ev, &f.doc, f.studios[0]);
    let asm = Assembly {
        instances: vec![part_instance(1, "Part 1 <1>", f.studios[0], body.clone(), DVec3::ZERO, true), part_instance(2, "Part 1 <2>", f.studios[0], body.clone(), DVec3::X, false)],
        ..Default::default()
    };
    let tab = f.doc.add_tab("Assembly 1", TabContent::Assembly(Box::new(asm)));
    assert!(crate::assembly::follow_part_names(&mut f.doc, f.studios[0]).is_empty(), "no own names yet");
    if let Some(TabContent::PartStudio(ps)) = f.doc.tab_mut(f.studios[0]).map(|t| &mut t.content) {
        ps.part_properties.entry(body).or_default().name = Some("Pin".into());
    }
    assert_eq!(crate::assembly::follow_part_names(&mut f.doc, f.studios[0]), vec![tab]);
    let Some(TabContent::Assembly(a)) = f.doc.tab(tab).map(|t| &t.content) else { panic!() };
    assert_eq!(a.instances.iter().map(|i| i.name.as_str()).collect::<Vec<_>>(), ["Pin <1>", "Pin <2>"]);
}
