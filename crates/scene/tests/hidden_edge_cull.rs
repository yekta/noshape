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

/// A frame: its size in CSS pixels, its pixel ratio, and how many times farther than fitting the part the camera is
/// (zoomed out, the part is small on screen and its edges are drawn at a coarser line level).
#[derive(Clone, Copy, Debug)]
struct Shot {
    size: (u32, u32),
    dpr: f32,
    far: f64,
}

impl Shot {
    fn device(self) -> (u32, u32) {
        ((self.size.0 as f32 * self.dpr) as u32, (self.size.1 as f32 * self.dpr) as u32)
    }
}

/// The camera of a view.
fn camera(display: &StudioDisplay, view: DVec3, projection: Projection, shot: Shot) -> Camera {
    let (w, h) = shot.device();
    let (lo, hi) = display.bodies[0].mesh.positions.iter().fold((DVec3::INFINITY, DVec3::NEG_INFINITY), |(lo, hi), p| {
        let p = DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64);
        (lo.min(p), hi.max(p))
    });
    let mut cam = Camera::default();
    let fwd = view.normalize();
    let side = fwd.cross(DVec3::Z).normalize_or(DVec3::X);
    cam.rotation = Camera::look_rotation(fwd, side.cross(fwd));
    cam.projection = projection;
    cam.fit_box(lo, hi, w as f64 / h as f64);
    cam.distance *= shot.far;
    cam
}

/// The frame with hidden edges kept or left out.
fn render(r: &mut Renderer, display: &StudioDisplay, keep: bool, cam: &Camera, shot: Shot) -> Vec<u8> {
    let style = noshape_scene::png::viewport_style(&noshape_scene::GraphicsPrefs::default());
    let up = r.uploader();
    let mut cache = GpuCache::default();
    let opts = SceneOptions { keep_hidden_edges: keep, display_mode: DisplayMode::ShadedWithEdges, show_planes: false, show_origin: false, show_sketches: false, ..Default::default() };
    let built = builder::build(display, &opts, &mut cache, &up);
    let (w, h) = shot.device();
    let scene = builder::with_highlights(&built, display, &[], None, &[], Vec::new(), false, &up);
    r.render(RenderParams { scene: &scene, camera: cam, width: w, height: h, device_pixel_ratio: shot.dpr, style: &style, pick: false }).unwrap().bgra
}

/// How many of the finest edge segments the visible lines' pass leaves out (shaders.wgsl `behind_own_solid`, at a
/// 1 px line), as the renderer decides.
fn left_out(display: &StudioDisplay, cam: &Camera, shot: Shot) -> usize {
    let (w, h) = shot.device();
    let viewport = glam::DVec2::new(w as f64, h as f64);
    let mesh = &display.bodies[0].mesh;
    let normals = builder::edge_facet_normals(mesh);
    let px_across = |w: DVec3, t: f64| -> f64 {
        match (cam.project(w, viewport, 1.0), cam.project(w + cam.up() * t, viewport, 1.0)) {
            (Some(a), Some(b)) => a.distance(b),
            _ => 0.0,
        }
    };
    let mut n = 0;
    for (pl, ns) in mesh.edges.iter().zip(&normals) {
        for si in 0..ns.len() {
            let Some(c) = noshape_render::scene::edge_cull_cones(ns, si, si + 1) else { continue };
            if c.clearance < 2f32.powi(-16) {
                continue;
            }
            let ends = [pl[si], pl[si + 1]].map(|p| DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64));
            let toward = |w: DVec3| if cam.projection == Projection::Perspective { (cam.eye() - w).normalize() } else { -cam.forward() };
            let side = |cone: noshape_render::scene::NormalCone| {
                let s = ((cone.sin_spread as f64 + 0.006) * 31.0).ceil() / 31.0;
                ends.iter().map(|&w| -cone.axis.as_dvec3().dot(toward(w))).fold(f64::INFINITY, f64::min) * (1.0 - s * s) - s
            };
            let d = side(c.a).min(side(c.b));
            let t = 2f64.powf((c.clearance as f64).log2().floor().min(-2.0));
            let px = ends.iter().map(|&w| px_across(w, t)).fold(f64::INFINITY, f64::min);
            let width = 0.5 * shot.dpr as f64 + 1.5;
            let thick = if c.convex { px * d * d * 0.5 >= width && px.min(2.0 * px * d) >= 8.0 } else { px * d >= width && px >= 8.0 };
            if d > 0.0 && thick {
                n += 1;
            }
        }
    }
    n
}

fn compare(views: &[[f64; 3]], shots: &[Shot]) {
    let mut r = match Renderer::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };
    let mut left = 0;
    for (name, display) in models() {
        let normals = builder::edge_facet_normals(&display.bodies[0].mesh);
        let known = normals.iter().flatten().filter(|n| n.is_some()).count();
        let total = normals.iter().flatten().count();
        assert!(known * 10 >= total * 9, "{name}: facet normals for {known} of {total} edge segments");
        for v in views {
            let view = DVec3::from_array(*v);
            for projection in [Projection::Orthographic, Projection::Perspective] {
                for &shot in shots {
                    let cam = camera(&display, view, projection, shot);
                    left += left_out(&display, &cam, shot);
                    let kept = render(&mut r, &display, true, &cam, shot);
                    let culled = render(&mut r, &display, false, &cam, shot);
                    let differ: Vec<usize> = kept.chunks(4).zip(culled.chunks(4)).enumerate().filter(|(_, (a, b))| a != b).map(|(i, _)| i).collect();
                    let w = shot.device().0 as usize;
                    let first = differ.first().map(|i| (i % w, i / w));
                    assert!(differ.is_empty(), "{name}, view {v:?}, {projection:?}, {shot:?}: {} pixels differ, first at {first:?}", differ.len());
                }
            }
        }
    }
    assert!(left > 0, "some segments are left out (else the comparison proves nothing)");
}

#[test]
fn edges_behind_their_own_solid_change_no_pixel() {
    let small = |far| Shot { size: (320, 240), dpr: 1.0, far };
    compare(&VIEWS[..1], &[small(1.0), small(8.0)]);
}

#[test]
#[ignore = "T2: every view at 640 x 480, 1x and 2x, zoomed out 8x and 30x (~12 min on lavapipe)"]
fn edges_behind_their_own_solid_change_no_pixel_everywhere() {
    let shot = |dpr, far| Shot { size: (640, 480), dpr, far };
    compare(&VIEWS, &[shot(1.0, 1.0), shot(2.0, 1.0), shot(1.0, 8.0), shot(1.0, 30.0)]);
}
