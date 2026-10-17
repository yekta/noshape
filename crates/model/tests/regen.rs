use glam::DVec2;
use noshape_model::configuration::ConfigValues;
use noshape_model::display::FeatureStatus;
use noshape_model::params::ParamValue;
use noshape_model::partstudio::PartStudio;
use noshape_model::reference::{DatumPlane, Reference};
use noshape_model::regen::{RegenOptions, Regenerator};
use noshape_model::sketch_feature::SketchData;
use noshape_units::{ExprValue, UnitPrefs};

fn rect_with_hole(w: f64, h: f64, r: f64) -> SketchData {
    let mut sk = noshape_sketch::Sketch::new();
    let p = [DVec2::new(0.0, 0.0), DVec2::new(w, 0.0), DVec2::new(w, h), DVec2::new(0.0, h)];
    let pts: Vec<_> = p.iter().map(|q| sk.add_point(*q)).collect();
    for i in 0..4 {
        sk.add_line_between(pts[i], pts[(i + 1) % 4]).unwrap();
    }
    sk.add_circle(DVec2::new(w / 2.0, h / 2.0), r);
    SketchData { sketch: sk, ..Default::default() }
}

fn studio_with_block() -> (PartStudio, UnitPrefs) {
    let units = UnitPrefs::default();
    let mut ps = PartStudio::new();
    let mut sketch = ps.new_feature("sketch", &units).unwrap();
    sketch.params.set("sketchPlane", ParamValue::Query(vec![Reference::Plane(DatumPlane::Top)]));
    sketch.params.set("sketch", ParamValue::Sketch(Box::new(rect_with_hole(0.1, 0.05, 0.01))));
    let sketch_id = sketch.id;
    ps.features.push(sketch);
    (ps, units).tap(|(ps, units)| {
        // Extrude the outer region (the one with the hole).
        let mut reg = Regenerator::new();
        let r = reg.regenerate(ps, &ConfigValues::default(), units, &RegenOptions::default());
        let sk = &r.display.sketches[0];
        assert_eq!(sk.regions.len(), 2, "rectangle with circle = 2 regions");
        let outer = sk.regions.iter().max_by(|a, b| a.area.partial_cmp(&b.area).unwrap()).unwrap();
        let mut ex = ps.new_feature("extrude", units).unwrap();
        ex.params.set("entities", ParamValue::Query(vec![Reference::SketchRegion { sketch: sketch_id, key: outer.key.clone() }]));
        ex.params.set("depth", ParamValue::Expr(ExprValue::new("25 mm")));
        ps.features.push(ex);
    })
}

trait Tap: Sized {
    fn tap(mut self, f: impl FnOnce(&mut Self)) -> Self {
        f(&mut self);
        self
    }
}
impl<T> Tap for T {}

#[test]
fn sketch_extrude_fillet_and_edit_upstream() {
    let (mut ps, units) = studio_with_block();
    let mut reg = Regenerator::new();
    let r = reg.regenerate(&ps, &ConfigValues::default(), &units, &RegenOptions::default());
    for s in &r.display.features {
        assert_eq!(s.status, FeatureStatus::Ok, "{:?}", s.message);
    }
    assert_eq!(r.display.bodies.len(), 1);
    let body = &r.context.bodies[0];
    let vol = body.shape.volume().unwrap();
    let expected = 0.1 * 0.05 * 0.025 - std::f64::consts::PI * 0.01f64.powi(2) * 0.025;
    assert!((vol - expected).abs() < 1e-9, "volume {vol} vs {expected}");
    assert_eq!(body.faces.len(), 7, "4 sides + 2 caps + hole");
    assert!(!r.display.bodies[0].mesh.indices.is_empty());

    // Fillet the 4 top outer edges: edges between the end cap and the side faces.
    let top_edges: Vec<Reference> = body
        .edges
        .iter()
        .filter(|e| {
            let s = format!("{e:?}");
            s.contains("CapEnd") && s.contains("Side") && s.contains("entity: ") && !s.contains("entity: 5")
        })
        .map(|e| Reference::Edge((**e).clone()))
        .collect();
    assert_eq!(top_edges.len(), 4, "top outer edges: {:#?}", body.edges);
    let mut fillet = ps.new_feature("fillet", &units).unwrap();
    fillet.params.set("entities", ParamValue::Query(top_edges));
    fillet.params.set("radius", ParamValue::Expr(ExprValue::new("3 mm")));
    ps.features.push(fillet);
    let r = reg.regenerate(&ps, &ConfigValues::default(), &units, &RegenOptions::default());
    assert_eq!(r.display.features[2].status, FeatureStatus::Ok, "{:?}", r.display.features[2].message);
    let filleted = r.context.bodies[0].faces.len();
    assert_eq!(filleted, 11, "7 faces + 4 blends");

    // Edit upstream: make the block wider. The fillet must still find its edges.
    if let Some(ParamValue::Sketch(d)) = ps.features[0].params.0.get_mut("sketch") {
        let ids: Vec<_> = d.sketch.entities().filter(|e| e.kind() == noshape_sketch::EntityKind::Point).map(|e| e.id).collect();
        // Move the two right-hand corners.
        let p1 = d.sketch.point(ids[1]).unwrap();
        let p2 = d.sketch.point(ids[2]).unwrap();
        d.sketch.set_point(ids[1], p1 + DVec2::new(0.05, 0.0)).unwrap();
        d.sketch.set_point(ids[2], p2 + DVec2::new(0.05, 0.0)).unwrap();
    }
    let t = std::time::Instant::now();
    let r = reg.regenerate(&ps, &ConfigValues::default(), &units, &RegenOptions::default());
    eprintln!("full regen after upstream edit: {:?}", t.elapsed());
    for s in &r.display.features {
        assert_eq!(s.status, FeatureStatus::Ok, "after edit: {:?}", s.message);
    }
    assert_eq!(r.context.bodies[0].faces.len(), 11);

    // Re-regenerating with no change hits the cache entirely.
    let t = std::time::Instant::now();
    let _ = reg.regenerate(&ps, &ConfigValues::default(), &units, &RegenOptions::default());
    eprintln!("cached regen: {:?}", t.elapsed());
}

#[test]
fn failed_feature_does_not_break_downstream() {
    let (mut ps, units) = studio_with_block();
    let mut bad = ps.new_feature("fillet", &units).unwrap();
    bad.params.set("entities", ParamValue::Query(vec![]));
    ps.features.push(bad);
    let mut shell = ps.new_feature("shell", &units).unwrap();
    shell.params.set("isHollow", ParamValue::Bool(true));
    let body = noshape_model::reference::BodyId { feature: ps.features[1].id, index: 0 };
    shell.params.set("parts", ParamValue::Query(vec![Reference::Body(body)]));
    shell.params.set("thickness", ParamValue::Expr(ExprValue::new("2 mm")));
    ps.features.push(shell);
    let mut reg = Regenerator::new();
    let r = reg.regenerate(&ps, &ConfigValues::default(), &units, &RegenOptions::default());
    assert_eq!(r.display.features[2].status, FeatureStatus::Error);
    assert_eq!(r.display.features[3].status, FeatureStatus::Ok, "{:?}", r.display.features[3].message);
}

#[test]
fn rollback_bar_limits_evaluation() {
    let (mut ps, units) = studio_with_block();
    ps.rollback = Some(1);
    let mut reg = Regenerator::new();
    let r = reg.regenerate(&ps, &ConfigValues::default(), &units, &RegenOptions::default());
    assert_eq!(r.display.bodies.len(), 0);
    assert_eq!(r.display.features[1].status, FeatureStatus::RolledBack);
}
