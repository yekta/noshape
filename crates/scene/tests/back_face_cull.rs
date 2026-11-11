//! A closed solid's back faces are culled in the color pass (`Instance::closed`), as live does (its solid faces are
//! one-sided): they are behind its front faces, so that changes no pixel of the drawing with edges or in hidden line,
//! and in plain Shaded only the back-face pixels our depth offsets let through steep faces and outlines, which live
//! never shows; ambient occlusion, which reads the depth those back faces wrote, moves by at most 2 levels. The same scenes are rendered with the flag
//! as the builder sets it and with it cleared, from several directions in both projections and three display modes,
//! on a thin plate, a block with a hole and a cylinder.

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
    sketch.params.set("sketch", ParamValue::Sketch(std::sync::Arc::new(SketchData { sketch: sk, ..Default::default() })));
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

/// View directions (forward), off-axis.
const VIEWS: [[f64; 3]; 4] = [[-1.0, 1.0, -1.0], [1.0, 0.4, -0.6], [0.3, -1.0, 0.8], [-0.2, 0.1, -1.0]];

/// The frame of `look` (view direction, projection, display mode), back faces culled or not, ambient occlusion or not.
fn render(r: &mut Renderer, display: &StudioDisplay, cull: bool, ao: bool, look: (DVec3, Projection, DisplayMode), size: (u32, u32)) -> (Vec<u8>, usize) {
    let (view, projection, mode) = look;
    let mut style = noshape_scene::png::viewport_style(&noshape_scene::GraphicsPrefs::default());
    if !ao {
        style.ambient_occlusion = 0.0;
    }
    let up = r.uploader();
    let mut cache = GpuCache::default();
    let opts = SceneOptions { display_mode: mode, show_planes: false, show_origin: false, show_sketches: false, ..Default::default() };
    let built = builder::build(display, &opts, &mut cache, &up);
    let (lo, hi) = built.scene.bounds().map(|(l, h)| (l.as_dvec3(), h.as_dvec3())).unwrap();
    let mut cam = Camera::default();
    let fwd = view.normalize();
    let side = fwd.cross(DVec3::Z).normalize_or(DVec3::X);
    cam.rotation = Camera::look_rotation(fwd, side.cross(fwd));
    cam.projection = projection;
    cam.fit_box(lo, hi, size.0 as f64 / size.1 as f64);
    let mut scene = builder::with_highlights(&built, display, &[], None, &[], Vec::new(), false, &up);
    let closed = scene.instances.iter().filter(|i| i.closed).count();
    if !cull {
        scene.instances.iter_mut().for_each(|i| i.closed = false);
    }
    let frame = r.render(RenderParams { scene: &scene, camera: &cam, width: size.0, height: size.1, device_pixel_ratio: 1.0, style: &style, pick: false }).unwrap().bgra;
    (frame, closed)
}

fn compare(views: &[[f64; 3]], size: (u32, u32)) {
    let mut r = match Renderer::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };
    let (mut bled, mut ao_moved) = (0, 0);
    for (name, display) in models() {
        for v in views {
            let view = DVec3::from_array(*v);
            for projection in [Projection::Orthographic, Projection::Perspective] {
                for mode in [DisplayMode::ShadedWithEdges, DisplayMode::HiddenLine, DisplayMode::Shaded] {
                    let pair = |r: &mut Renderer, ao: bool| {
                        let (culled, closed) = render(r, &display, true, ao, (view, projection, mode), size);
                        assert!(closed > 0, "{name}: its faces are a closed solid's");
                        let (kept, _) = render(r, &display, false, ao, (view, projection, mode), size);
                        let differ: Vec<(usize, i32)> = kept.chunks(4).zip(culled.chunks(4)).enumerate().filter(|(_, (a, b))| a != b).map(|(i, (a, b))| (i, a.iter().zip(b).map(|(x, y)| (*x as i32 - *y as i32).abs()).max().unwrap_or(0))).collect();
                        (differ, kept.len() / 4)
                    };
                    let at = |i: usize| (i % size.0 as usize, i / size.0 as usize);
                    let (differ, pixels) = pair(&mut r, false);
                    let first = differ.first().map(|d| at(d.0));
                    // Plain Shaded: with its back faces drawn, the depth offsets let them through where a front face
                    // is steep or at an outline (a dashed dark line along a far edge, specks along a hole's walls; edges
                    // cover them in the other modes). Live never draws them (its solid faces are one-sided,
                    // `isTwoSided: false`), and neither do we now: those few pixels change, nothing else (a front
                    // face culled by a wrong winding would change a whole face).
                    if mode == DisplayMode::Shaded {
                        assert!(differ.len() * 20 < pixels, "{name}, view {v:?}, {projection:?}, Shaded: {} pixels differ (more than back faces bleeding through), first at {first:?}", differ.len());
                        bled += differ.len();
                        continue;
                    }
                    // With edges and in hidden line: not one pixel of the drawing changes.
                    assert!(differ.is_empty(), "{name}, view {v:?}, {projection:?}, {mode:?}: {} pixels differ, first at {first:?}", differ.len());
                    // Ambient occlusion reads the depth those bled-through back faces wrote (a few samples by a thin
                    // wall's far edge): without them it shades by at most 2 levels of 255 otherwise there (Metal: one
                    // pixel of the 3 mm plate, lavapipe: some under sub-pixel camera moves). Never more.
                    let (differ, _) = pair(&mut r, true);
                    if let Some(&(i, d)) = differ.iter().max_by_key(|d| d.1) {
                        assert!(d <= 2, "{name}, view {v:?}, {projection:?}, {mode:?} with AO: a channel {d} levels apart at {:?} (more than back-face depth in the AO)", at(i));
                    }
                    ao_moved += differ.len();
                }
            }
        }
    }
    eprintln!("back-face pixels no longer bleeding through in plain Shaded: {bled}; AO within 2 levels at {ao_moved}");
}

#[test]
fn culling_a_closed_solids_back_faces_changes_no_pixel() {
    compare(&VIEWS[..2], (320, 240));
}

#[test]
#[ignore = "T2: every view at 640 x 480 (~1 min on lavapipe)"]
fn culling_a_closed_solids_back_faces_changes_no_pixel_everywhere() {
    compare(&VIEWS, (640, 480));
}

