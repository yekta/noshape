//! A solid's edge segments both of whose facets face away from the viewer lie behind the solid itself: the visible
//! lines' pass leaves them out (`Instance::cull_hidden`, shaders.wgsl `behind_own_solid`). That must change no pixel:
//! the same scenes are rendered with them kept (`SceneOptions::keep_hidden_edges`) and left out, byte for byte, from
//! several directions in both projections, on a thin plate (where a back edge is closest to the front faces), a
//! block with a hole and a cylinder (curved edges, segments turning away along them).

use glam::{DVec2, DVec3};
use noshape_model::configuration::ConfigValues;
use noshape_model::display::StudioDisplay;
use noshape_model::params::ParamValue;
use noshape_model::partstudio::PartStudio;
use noshape_model::reference::{DatumPlane, Reference};
use noshape_model::regen::{RegenOptions, Regenerator};
use noshape_model::sketch_feature::SketchData;
use noshape_render::renderer::{RenderParams, Renderer};
use noshape_render::{Camera, Projection};
use noshape_scene::builder::{self, DisplayMode, GpuCache, SceneOptions};
use noshape_units::{ExprValue, UnitPrefs};

/// A sketch on Top (a w × h rectangle at the origin and/or a circle) extruded `depth`; its biggest region.
fn extruded(w: f64, h: f64, circle: Option<(DVec2, f64)>, depth: &str) -> StudioDisplay {
    let units = UnitPrefs::default();
    let mut ps = PartStudio::new();
    let mut sk = noshape_sketch::Sketch::new();
    if w > 0.0 {
        let p = [DVec2::new(0.0, 0.0), DVec2::new(w, 0.0), DVec2::new(w, h), DVec2::new(0.0, h)];
        let pts: Vec<_> = p.iter().map(|q| sk.add_point(*q)).collect();
        for i in 0..4 {
            sk.add_line_between(pts[i], pts[(i + 1) % 4]).unwrap();
        }
    }
    if let Some((c, r)) = circle {
        sk.add_circle(c, r);
    }
    let mut sketch = ps.new_feature("sketch", &units).unwrap();
    sketch.params.set("sketchPlane", ParamValue::Query(vec![Reference::Plane(DatumPlane::Top)]));
    sketch.params.set("sketch", ParamValue::Sketch(Box::new(SketchData { sketch: sk, ..Default::default() })));
    let sid = sketch.id;
    ps.features.push(sketch);
    let regen = |ps: &PartStudio| Regenerator::new().regenerate(ps, &ConfigValues::default(), &units, &RegenOptions::default());
    let r = regen(&ps);
    let region = r.display.sketches[0].regions.iter().max_by(|a, b| a.area.partial_cmp(&b.area).unwrap()).unwrap().key.clone();
    let mut ex = ps.new_feature("extrude", &units).unwrap();
    ex.params.set("entities", ParamValue::Query(vec![Reference::SketchRegion { sketch: sid, key: region }]));
    ex.params.set("depth", ParamValue::Expr(ExprValue::new(depth)));
    ps.features.push(ex);
    let out = regen(&ps);
    assert_eq!(out.display.bodies.len(), 1);
    out.display
}

fn models() -> Vec<(&'static str, StudioDisplay)> {
    vec![
        ("3 mm plate with a hole", extruded(0.3, 0.2, Some((DVec2::new(0.15, 0.1), 0.02)), "3 mm")),
        ("block with a hole", extruded(0.02, 0.02, Some((DVec2::new(0.01, 0.01), 0.003)), "10 mm")),
        ("cylinder", extruded(0.0, 0.0, Some((DVec2::new(0.0, 0.0), 0.01)), "30 mm")),
    ]
}

/// View directions (forward), off-axis so edges turn away at many angles.
const VIEWS: [[f64; 3]; 4] = [[-1.0, 1.0, -1.0], [1.0, 0.4, -0.6], [0.3, -1.0, 0.8], [-0.2, 0.1, -1.0]];

/// The frame with hidden edges kept or left out, and how many edge segments face away on both sides.
fn render(r: &mut Renderer, display: &StudioDisplay, keep: bool, view: DVec3, projection: Projection, dpr: f32, far: f64) -> Vec<u8> {
    let style = noshape_scene::png::viewport_style(&noshape_scene::GraphicsPrefs::default());
    let up = r.uploader();
    let mut cache = GpuCache::default();
    let opts = SceneOptions { keep_hidden_edges: keep, display_mode: DisplayMode::ShadedWithEdges, show_planes: false, show_origin: false, show_sketches: false, ..Default::default() };
    let built = builder::build(display, &opts, &mut cache, &up);
    let (lo, hi) = built.scene.bounds().map(|(l, h)| (l.as_dvec3(), h.as_dvec3())).unwrap();
    let (w, h) = ((640.0 * dpr) as u32, (480.0 * dpr) as u32);
    let mut cam = Camera::default();
    let fwd = view.normalize();
    let side = fwd.cross(DVec3::Z).normalize_or(DVec3::X);
    cam.rotation = Camera::look_rotation(fwd, side.cross(fwd));
    cam.projection = projection;
    cam.fit_box(lo, hi, w as f64 / h as f64);
    // Zoomed out, the part is small on screen and its edges are drawn at a coarser line level.
    cam.distance *= far;
    let scene = builder::with_highlights(&built, display, &[], None, &[], Vec::new(), false, &up);
    r.render(RenderParams { scene: &scene, camera: &cam, width: w, height: h, device_pixel_ratio: dpr, style: &style, pick: false }).unwrap().bgra
}

/// The finest edge segments left out for `view` (orthographic), as the shader decides (their cones past
/// perpendicular by the spread and the margin).
fn turned_away(display: &StudioDisplay, view: DVec3) -> usize {
    let v = -view.normalize().as_vec3();
    let normals = builder::edge_facet_normals(&display.bodies[0].mesh);
    let away = |c: noshape_render::scene::NormalCone| c.axis.dot(v) < -(c.sin_spread + 0.006 + 1.0 / 63.0 + 0.02);
    normals
        .iter()
        .map(|ns| (0..ns.len()).filter(|&si| noshape_render::scene::edge_cull_cones(ns, si, si + 1).is_some_and(|(a, b)| away(a) && away(b))).count())
        .sum()
}

fn compare(views: &[[f64; 3]], dprs: &[f32]) {
    let mut r = match Renderer::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };
    let mut away = 0;
    for (name, display) in models() {
        let normals = builder::edge_facet_normals(&display.bodies[0].mesh);
        let known = normals.iter().flatten().filter(|n| n.is_some()).count();
        let total = normals.iter().flatten().count();
        assert!(known * 10 >= total * 9, "{name}: facet normals for {known} of {total} edge segments");
        for v in views {
            let view = DVec3::from_array(*v);
            away += turned_away(&display, view);
            for projection in [Projection::Orthographic, Projection::Perspective] {
                for (dpr, far) in dprs.iter().map(|&d| (d, 1.0)).chain([(1.0, 8.0), (1.0, 30.0)]) {
                    let kept = render(&mut r, &display, true, view, projection, dpr, far);
                    let culled = render(&mut r, &display, false, view, projection, dpr, far);
                    let differ = kept.chunks(4).zip(culled.chunks(4)).filter(|(a, b)| a != b).count();
                    if differ > 0 {
                        let w = (640.0 * dpr) as usize;
                        let px: Vec<String> = kept.chunks(4).zip(culled.chunks(4)).enumerate().filter(|(_, (a, b))| a != b).map(|(i, (a, b))| format!("({},{}) kept {:?} culled {:?}", i % w, i / w, a, b)).take(8).collect();
                        eprintln!("DIFF {name} {v:?} {projection:?} {dpr}: {px:?}");
                    }
                    assert_eq!(differ, 0, "{name}, view {v:?}, {projection:?}, {dpr}x, zoomed out {far}x: {differ} pixels differ");
                }
            }
        }
    }
    eprintln!("segments left out over the views: {away}");
    assert!(away > 0, "some segments are left out (else the comparison proves nothing)");
}

#[test]
fn edges_behind_their_own_solid_change_no_pixel() {
    compare(&VIEWS[..3], &[1.0]);
}

#[test]
#[ignore = "T2: every view at 1× and 2× (~1 min on lavapipe)"]
fn edges_behind_their_own_solid_change_no_pixel_everywhere() {
    compare(&VIEWS, &[1.0, 2.0]);
}
