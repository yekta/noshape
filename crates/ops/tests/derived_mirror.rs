//! Live's assembly Mirror › Derived (behaviours "skb (10-23 15:30)"): the
//! "<Part Studio>-Mirrored" Part Studio's "Derived mirror" feature brings
//! every source part mirrored, named "<part>-Mirrored", under the source
//! part's id (the assembly's Derived copy references it).

use noshape_model::reference::BodyId;

#[test]
fn derived_mirror_studio_mirrors_the_source_parts() {
    let mut doc = noshape_model::demo::demo_document();
    let src = doc.tabs.iter().find(|t| t.name == "Part Studio 1").unwrap().id;
    let mut ev = noshape_ops::derived::assembly_evaluator(Default::default());
    let source = ev.studio(&doc, src, &Default::default()).unwrap();
    let ids: Vec<BodyId> = source.context.bodies.iter().map(|b| b.id.clone()).collect();
    let m = doc.ensure_mirrored_studio(src).unwrap();
    let mut ev = noshape_ops::derived::assembly_evaluator(Default::default());
    let r = ev.studio(&doc, m, &Default::default()).unwrap();
    assert!(r.display.features.iter().all(|f| f.message.is_none()), "{:?}", r.display.features.iter().filter_map(|f| f.message.clone()).collect::<Vec<_>>());
    assert_eq!(r.context.bodies.iter().map(|b| b.id.clone()).collect::<Vec<_>>(), ids);
    let (a, b) = (&source.context.bodies[0], &r.context.bodies[0]);
    assert_eq!(b.name, format!("{}-Mirrored", a.name));
    // Mirrored in the source's XY plane (live: z 0..40 -> -40..0): the same
    // volume, z bounds negated, x and y kept.
    assert!((a.shape.volume().unwrap() - b.shape.volume().unwrap()).abs() < 1e-12);
    let (ba, bb) = (a.shape.bounding_box(false).unwrap(), b.shape.bounding_box(false).unwrap());
    assert!((ba.min.z + bb.max.z).abs() < 1e-7 && (ba.max.z + bb.min.z).abs() < 1e-7, "{ba:?} {bb:?}");
    assert!((ba.min.x - bb.min.x).abs() < 1e-7 && (ba.max.y - bb.max.y).abs() < 1e-7, "{ba:?} {bb:?}");
}

/// The assembly's Derived copy (the "-Mirrored" part, its reflection
/// completed in the source's XY plane) is the true mirror image of the seed:
/// its world centre of mass is the seed's reflected (the plate's holes are
/// off-centre, so a rigid copy would differ), with live's transform: the
/// seed's turned 180 degrees about Y and mirrored in the plane (live: plane
/// x = 50, seed at identity -> diag(-1, 1, -1), (100, 0, 0) mm).
#[test]
fn derived_copy_is_the_mirror_image() {
    use noshape_model::assembly::eval::SolveOptions;
    use noshape_model::assembly::*;
    use noshape_units::ExprValue;
    let mut doc = noshape_model::demo::demo_document();
    let src = doc.tabs.iter().find(|t| t.name == "Part Studio 1").unwrap().id;
    let mut ev = noshape_ops::derived::assembly_evaluator(Default::default());
    let body = ev.studio(&doc, src, &Default::default()).unwrap().context.bodies[0].id.clone();
    let m = doc.ensure_mirrored_studio(src).unwrap();
    let mut plane = AsmMateConnector::new(MateId(60), "Mate connector 60".into(), MateConnectorRef::origin());
    plane.rotation_axis = 1;
    plane.rotation = Some(ExprValue::new("90 deg"));
    let seed = Instance {
        id: InstanceId(1),
        name: "Part 1 <1>".into(),
        source: InstanceSource::Part { studio: src, body, config: Default::default() },
        placement: Placement { translation: [0.1, 0.02, 0.0], rotation: [0.0, 0.0, 0.0, 1.0] },
        fixed: true,
        hidden: false,
        suppressed: false,
        lock: SubassemblyLock::Flexible,
        nested: Vec::new(),
        best_tessellation: false,
        reference_designator: None,
        unsuppressed_if: None,
    };
    let mut asm = Assembly { instances: vec![seed], connectors: vec![plane], ..Default::default() };
    asm.patterns.push(InstancePattern {
        id: MateId(70),
        name: "Mirror 1".into(),
        kind: PatternKind::Mirror { plane: MateConnectorRef::assembly_connector(MateId(60), InstanceId::ORIGIN), handed: false, derived: vec![InstanceId(1)], placement: vec![], mirrored_studios: vec![(src, m)] },
        seeds: vec![InstanceId(1)],
        generated: vec![],
        suppressed: false,
    });
    let created = asm.sync_pattern(MateId(70));
    assert!(matches!(&asm.instance(created[0]).unwrap().source, InstanceSource::Part { studio, .. } if *studio == m));
    let res = ev.evaluate(&doc, &asm, &SolveOptions::default(), None);
    assert!(res.nodes.iter().all(|n| n.error.is_none()), "{:?}", res.nodes.iter().filter_map(|n| n.error.clone()).collect::<Vec<_>>());
    let center = |id: InstanceId| {
        let o = res.occurrences.iter().find(|o| o.top() == id).unwrap();
        let c = noshape_kernel::volume_properties(&o.context.body(&o.body).unwrap().shape).unwrap().center;
        o.transform.transform_point3(c)
    };
    let (a, b) = (center(InstanceId(1)), center(created[0]));
    assert!((b - glam::DVec3::new(-a.x, a.y, a.z)).length() < 1e-7, "seed {a:?} copy {b:?}");
    let p = res.solved.placement(created[0]).unwrap();
    let t = glam::DVec3::from(p.translation);
    let r = glam::DQuat::from_array(p.rotation);
    assert!((t - glam::DVec3::new(-0.1, 0.02, 0.0)).length() < 1e-9, "{t:?}");
    assert!(r.angle_between(glam::DQuat::from_rotation_y(std::f64::consts::PI)) < 1e-9, "{r:?}");
}
