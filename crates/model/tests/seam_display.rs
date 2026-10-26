//! A part shows Onshape's topology (lead 10-25 SEAM): a cylinder is 3 faces,
//! 2 circle edges and no vertex in the display (drawn and pickable edges and
//! vertices), whatever OCCT's seam edge and its vertices.

use glam::DVec2;
use noshape_model::configuration::ConfigValues;
use noshape_model::params::ParamValue;
use noshape_model::partstudio::PartStudio;
use noshape_model::reference::{DatumPlane, Reference};
use noshape_model::regen::{RegenOptions, RegenResult, Regenerator};
use noshape_model::sketch_feature::SketchData;
use noshape_units::{ExprValue, UnitPrefs};

fn regen(ps: &PartStudio) -> RegenResult {
    Regenerator::new().regenerate(ps, &ConfigValues::default(), &UnitPrefs::default(), &RegenOptions::default())
}

#[test]
fn a_cylinder_shows_two_edges_and_no_vertex() {
    let units = UnitPrefs::default();
    let mut ps = PartStudio::new();
    let mut sk = noshape_sketch::Sketch::new();
    sk.add_circle(DVec2::ZERO, 0.025);
    let mut sketch = ps.new_feature("sketch", &units).unwrap();
    sketch.params.set("sketchPlane", ParamValue::Query(vec![Reference::Plane(DatumPlane::Top)]));
    sketch.params.set("sketch", ParamValue::Sketch(Box::new(SketchData { sketch: sk, ..Default::default() })));
    let sid = sketch.id;
    ps.features.push(sketch);
    let key = regen(&ps).display.sketches[0].regions[0].key.clone();
    let mut ex = ps.new_feature("extrude", &units).unwrap();
    ex.params.set("entities", ParamValue::Query(vec![Reference::SketchRegion { sketch: sid, key }]));
    ex.params.set("depth", ParamValue::Expr(ExprValue::new("40 mm")));
    ps.features.push(ex);
    let r = regen(&ps);
    let b = &r.display.bodies[0];
    assert_eq!(b.face_refs.len(), 3);
    assert_eq!(b.mesh.edges.iter().filter(|p| !p.is_empty()).count(), 2, "the seam isn't drawn");
    assert_eq!((b.mesh.vertices.len(), b.vertex_refs.len()), (0, 0), "no vertex on the circles");
    for m in &b.coarser {
        assert_eq!(m.vertices.len(), 0);
    }
}
