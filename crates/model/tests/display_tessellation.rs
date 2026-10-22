//! Onshape's display mesh is angle based (live 459): a full circle is drawn
//! with 144 segments in Auto whatever its size (Ø20, Ø200, Ø2000 alike),
//! and the part's Tessellation quality (Edit appearance) picks 24 / 48 /
//! 144 / 600 for Coarse / Medium / Fine / Very fine.

use glam::DVec2;
use noshape_model::configuration::ConfigValues;
use noshape_model::params::ParamValue;
use noshape_model::partstudio::{PartProperties, PartStudio, TessQuality};
use noshape_model::reference::{DatumPlane, Reference};
use noshape_model::regen::{RegenOptions, RegenResult, Regenerator};
use noshape_model::sketch_feature::SketchData;
use noshape_units::{ExprValue, UnitPrefs};

/// A cylinder of `diameter` (m), as high as it is wide.
fn cylinder(diameter: f64) -> PartStudio {
    let units = UnitPrefs::default();
    let mut ps = PartStudio::new();
    let mut sk = noshape_sketch::Sketch::new();
    sk.add_circle(DVec2::ZERO, diameter / 2.0);
    let mut sketch = ps.new_feature("sketch", &units).unwrap();
    sketch.params.set("sketchPlane", ParamValue::Query(vec![Reference::Plane(DatumPlane::Top)]));
    sketch.params.set("sketch", ParamValue::Sketch(Box::new(SketchData { sketch: sk, ..Default::default() })));
    let sid = sketch.id;
    ps.features.push(sketch);
    let key = regen(&ps).display.sketches[0].regions[0].key.clone();
    let mut ex = ps.new_feature("extrude", &units).unwrap();
    ex.params.set("entities", ParamValue::Query(vec![Reference::SketchRegion { sketch: sid, key }]));
    ex.params.set("depth", ParamValue::Expr(ExprValue::new(format!("{} mm", diameter * 1000.0))));
    ps.features.push(ex);
    ps
}

fn regen(ps: &PartStudio) -> RegenResult {
    Regenerator::new().regenerate(ps, &ConfigValues::default(), &UnitPrefs::default(), &RegenOptions::default())
}

/// Segments of the longest edge polyline (a circle: one closed edge).
fn circle_segments(r: &RegenResult) -> usize {
    r.display.bodies[0].mesh.edges.iter().map(|e| e.len().saturating_sub(1)).max().unwrap()
}

#[test]
fn circles_get_onshapes_segments_at_every_size() {
    for diameter in [0.02, 0.2, 2.0] {
        let r = regen(&cylinder(diameter));
        assert_eq!(circle_segments(&r), 144, "Ø{} mm in Auto", diameter * 1000.0);
    }
    let mut ps = cylinder(0.02);
    let body = regen(&ps).display.bodies[0].id.clone();
    for (q, n) in [(TessQuality::Coarse, 24), (TessQuality::Medium, 48), (TessQuality::Fine, 144), (TessQuality::VeryFine, 600)] {
        ps.part_properties.insert(body.clone(), PartProperties { tessellation: Some(q), ..Default::default() });
        assert_eq!(circle_segments(&regen(&ps)), n, "{q:?}");
    }
}
