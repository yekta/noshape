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
    sketch.params.set("sketch", ParamValue::Sketch(std::sync::Arc::new(SketchData { sketch: sk, ..Default::default() })));
    let sid = sketch.id;
    ps.features.push(sketch);
    let key = regen(&ps).display.sketches[0].regions[0].key.clone();
    let mut ex = ps.new_feature("extrude", &units).unwrap();
    ex.params.set("entities", ParamValue::Query(vec![Reference::SketchRegion { sketch: sid, key }]));
    ex.params.set("depth", ParamValue::Expr(ExprValue::new(format!("{} mm", diameter * 1000.0))));
    ps.features.push(ex);
    ps
}

/// View in high quality (Shift+R): the app's 4x finer relative deflection.
fn high_quality() -> RegenOptions {
    RegenOptions { relative_deflection: RegenOptions::default().relative_deflection * 0.25, ..Default::default() }
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
    // The default (regen::angle_display_mesh), whatever NOSHAPE_DISPLAY_MESH says.
    noshape_model::regen::set_angle_display_mesh(true);
    for diameter in [0.02, 0.2, 2.0] {
        let r = regen(&cylinder(diameter));
        assert_eq!(circle_segments(&r), 144, "Ø{} mm in Auto", diameter * 1000.0);
        // One level: no screen-space LOD outside high quality (live 10-23 a3).
        assert_eq!(level_segments(&r), [144], "Ø{} mm", diameter * 1000.0);
    }
    let mut ps = cylinder(0.02);
    let body = regen(&ps).display.bodies[0].id.clone();
    for (q, n) in [(TessQuality::Coarse, 24), (TessQuality::Medium, 48), (TessQuality::Fine, 144), (TessQuality::VeryFine, 600)] {
        ps.part_properties.insert(body.clone(), PartProperties { tessellation: Some(q), ..Default::default() });
        let r = regen(&ps);
        assert_eq!(circle_segments(&r), n, "{q:?}");
        assert_eq!(level_segments(&r), [n], "{q:?}");
        // View in high quality: its own level up to Very fine, the viewport
        // refining it once large on screen.
        let r = Regenerator::new().regenerate(&ps, &ConfigValues::default(), &UnitPrefs::default(), &high_quality());
        let want: Vec<usize> = [24, 48, 144, 600].into_iter().skip(q.level()).collect();
        assert_eq!(level_segments(&r), want, "{q:?} in high quality");
    }
}

#[test]
fn coarser_levels_stay_coarse_after_finer_meshing() {
    // The shape keeps the finest triangulation it was meshed with (Very
    // fine in high quality); a later regen that needs coarser levels again
    // must not get it back. (Lowering the part's own setting reuses the
    // finer one for its finest level, as before levels.)
    noshape_model::regen::set_angle_display_mesh(true);
    let mut ps = cylinder(0.02);
    let mut regen = Regenerator::new();
    let opts = high_quality();
    let mut run = |ps: &PartStudio| regen.regenerate(ps, &ConfigValues::default(), &UnitPrefs::default(), &opts);
    let body = run(&ps).display.bodies[0].id.clone();
    for (q, want) in [(TessQuality::Fine, vec![144, 600]), (TessQuality::Coarse, vec![24, 48, 144, 600]), (TessQuality::Auto, vec![144, 600])] {
        ps.part_properties.insert(body.clone(), PartProperties { tessellation: Some(q), ..Default::default() });
        assert_eq!(level_segments(&run(&ps)), want, "{q:?}");
    }
}

#[test]
fn preview_regen_of_an_unchanged_part_keeps_its_refined_mesh() {
    noshape_model::regen::set_angle_display_mesh(true);
    let ps = cylinder(0.02);
    let (config, units) = (ConfigValues::default(), UnitPrefs::default());
    let preview = RegenOptions { preview_meshes: true, ..Default::default() };
    let mut regen = Regenerator::new();
    // An edit: coarse preview first, then the refine (the app's progressive meshing).
    assert!(regen.regenerate(&ps, &config, &units, &preview).needs_refine);
    let fine = regen.regenerate(&ps, &config, &units, &RegenOptions::default());
    assert!(!fine.needs_refine);
    // Opening or cancelling a dialog regenerates the unchanged part: the
    // refined mesh is drawn again, with no coarse step and no refine.
    let again = regen.regenerate(&ps, &config, &units, &preview);
    assert!(!again.needs_refine);
    assert!(std::sync::Arc::ptr_eq(&again.display.bodies[0].mesh, &fine.display.bodies[0].mesh));
    assert_eq!(circle_segments(&again), 144);
}

/// A curved body's preview mesh is the coarse one (a refine pass follows).
#[test]
fn curved_bodies_get_a_coarse_preview_first() {
    noshape_model::regen::set_angle_display_mesh(true);
    let ps = cylinder(0.02);
    let opts = RegenOptions { preview_meshes: true, ..Default::default() };
    let preview = Regenerator::new().regenerate(&ps, &ConfigValues::default(), &UnitPrefs::default(), &opts);
    assert!(preview.needs_refine);
    assert!(circle_segments(&preview) < circle_segments(&regen(&ps)), "the preview circle is coarser");
}

/// A used (projected) B-spline is drawn with its two ends only, no points along it (live 2461: the user's used STEP
/// edge showed a row of its control points).
#[test]
fn a_used_bspline_shows_only_its_ends() {
    let units = UnitPrefs::default();
    let mut ps = PartStudio::new();
    let mut sk = noshape_sketch::Sketch::new();
    let ctrl = [DVec2::new(0.0, 0.0), DVec2::new(0.01, 0.02), DVec2::new(0.02, -0.01), DVec2::new(0.03, 0.015), DVec2::new(0.04, 0.0)];
    let id = sk.add_bspline(&ctrl, 3).unwrap();
    sk.entity_mut(id).unwrap().external = true;
    let mut sketch = ps.new_feature("sketch", &units).unwrap();
    sketch.params.set("sketchPlane", ParamValue::Query(vec![Reference::Plane(DatumPlane::Top)]));
    sketch.params.set("sketch", ParamValue::Sketch(std::sync::Arc::new(SketchData { sketch: sk, ..Default::default() })));
    ps.features.push(sketch);
    let d = regen(&ps).display;
    assert_eq!(d.sketches[0].points.len(), 2, "only the spline's two ends: {:?}", d.sketches[0].points.iter().map(|p| p.position).collect::<Vec<_>>());
}

/// A part's display box is what it draws (c2-impeller-loft: OCCT's quick box bounded the loft's B-spline faces by
/// control points 3 m out, and Zoom to fit framed that): a closed fit spline through a star's points, extruded.
#[test]
fn a_bspline_parts_display_box_is_its_drawn_box() {
    let units = UnitPrefs::default();
    let mut ps = PartStudio::new();
    let mut sk = noshape_sketch::Sketch::new();
    let star: Vec<DVec2> = (0..=10).map(|i| {
        let a = i as f64 * std::f64::consts::PI / 5.0;
        DVec2::new(a.cos(), a.sin()) * if i % 2 == 0 { 0.05 } else { 0.012 }
    }).collect();
    sk.add_closed_fit_spline(&star).unwrap();
    let mut sketch = ps.new_feature("sketch", &units).unwrap();
    sketch.params.set("sketchPlane", ParamValue::Query(vec![Reference::Plane(DatumPlane::Top)]));
    sketch.params.set("sketch", ParamValue::Sketch(std::sync::Arc::new(SketchData { sketch: sk, ..Default::default() })));
    let sid = sketch.id;
    ps.features.push(sketch);
    let key = regen(&ps).display.sketches[0].regions[0].key.clone();
    let mut ex = ps.new_feature("extrude", &units).unwrap();
    ex.params.set("entities", ParamValue::Query(vec![Reference::SketchRegion { sketch: sid, key }]));
    ex.params.set("depth", ParamValue::Expr(ExprValue::new("10 mm")));
    ps.features.push(ex);
    let r = regen(&ps);
    let b = &r.display.bodies[0];
    let (lo, hi) = b.bounds.expect("bounds");
    let drawn = b.mesh.positions.iter().fold((glam::DVec3::splat(f64::INFINITY), glam::DVec3::splat(f64::NEG_INFINITY)), |(l, h), p| {
        let p = glam::DVec3::from(p.map(f64::from));
        (l.min(p), h.max(p))
    });
    let quick = r.context.bodies[0].shape.bounding_box(false).unwrap();
    assert!((quick.max - quick.min).max_element() > (drawn.1 - drawn.0).max_element() * 1.05, "the case: OCCT's quick box is looser than the drawn part ({:?} vs {drawn:?})", (quick.min, quick.max));
    assert!((lo - drawn.0).abs().max_element() < 1e-4 && (hi - drawn.1).abs().max_element() < 1e-4, "display box {:?}, drawn {drawn:?}", (lo, hi));
}
