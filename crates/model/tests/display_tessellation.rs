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
fn segments(m: &noshape_model::display::MeshBuffers) -> usize {
    m.edges.iter().map(|e| e.len().saturating_sub(1)).max().unwrap()
}

fn circle_segments(r: &RegenResult) -> usize {
    segments(&r.display.bodies[0].mesh)
}

/// Segments per circle at each display level, coarsest first; every
/// level has the finest's faces and edges (the same pick ids).
fn level_segments(r: &RegenResult) -> Vec<usize> {
    let b = &r.display.bodies[0];
    for m in &b.coarser {
        assert_eq!((m.face_ranges.len(), m.edges.len(), m.vertices.len()), (b.mesh.face_ranges.len(), b.mesh.edges.len(), b.mesh.vertices.len()));
    }
    b.coarser.iter().chain(std::iter::once(&b.mesh)).map(|m| segments(m)).collect()
}

#[test]
fn circles_get_onshapes_segments_at_every_size() {
    // Gated off by default (regen::angle_display_mesh).
    noshape_model::regen::set_angle_display_mesh(true);
    for diameter in [0.02, 0.2, 2.0] {
        let r = regen(&cylinder(diameter));
        assert_eq!(circle_segments(&r), 144, "Ø{} mm in Auto", diameter * 1000.0);
        // Onshape's levels below it (the viewport picks one by its error on
        // screen): Coarse and Medium.
        assert_eq!(level_segments(&r), [24, 48, 144], "Ø{} mm", diameter * 1000.0);
    }
    let mut ps = cylinder(0.02);
    let body = regen(&ps).display.bodies[0].id.clone();
    for (q, n) in [(TessQuality::Coarse, 24), (TessQuality::Medium, 48), (TessQuality::Fine, 144), (TessQuality::VeryFine, 600)] {
        ps.part_properties.insert(body.clone(), PartProperties { tessellation: Some(q), ..Default::default() });
        let r = regen(&ps);
        assert_eq!(circle_segments(&r), n, "{q:?}");
        // Its levels: Coarse up to its setting.
        let want: Vec<usize> = [24, 48, 144, 600].into_iter().take(q.level() + 1).collect();
        assert_eq!(level_segments(&r), want, "{q:?}");
    }
}

#[test]
fn coarser_levels_stay_coarse_after_finer_meshing() {
    // The shape keeps the finest triangulation it was meshed with; a later
    // regen that needs a coarser level again (evicted from the cache) must
    // not get the finest back. (Lowering the part's own setting, e.g. Very
    // fine to Fine, still reuses the finer one, as before levels.)
    noshape_model::regen::set_angle_display_mesh(true);
    let mut ps = cylinder(0.02);
    let mut regen = Regenerator::new();
    let mut run = |ps: &PartStudio| regen.regenerate(ps, &ConfigValues::default(), &UnitPrefs::default(), &RegenOptions::default());
    let body = run(&ps).display.bodies[0].id.clone();
    for (q, want) in [(TessQuality::Fine, vec![24, 48, 144]), (TessQuality::Coarse, vec![24]), (TessQuality::Auto, vec![24, 48, 144])] {
        ps.part_properties.insert(body.clone(), PartProperties { tessellation: Some(q), ..Default::default() });
        assert_eq!(level_segments(&run(&ps)), want, "{q:?}");
    }
}
