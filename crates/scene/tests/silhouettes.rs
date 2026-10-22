//! Onshape's edge-based silhouettes (builder.rs `silhouette_edges`,
//! shaders.wgsl `vs_silhouette`): a cylinder seen from the side shows its
//! outline as dark lines along its sides, in orthographic and perspective,
//! while the middle of its curved face stays shaded; plain Shaded mode
//! draws none. With edges, the solid also gets Onshape's backface
//! silhouette pass (`Instance::back_silhouette`, shaders.wgsl `vs_backsil`).

use glam::DVec2;
use noshape_model::configuration::ConfigValues;
use noshape_model::params::ParamValue;
use noshape_model::partstudio::PartStudio;
use noshape_model::reference::{DatumPlane, Reference};
use noshape_model::regen::{RegenOptions, Regenerator};
use noshape_model::sketch_feature::SketchData;
use noshape_render::Projection;
use noshape_render::renderer::{RenderParams, Renderer};
use noshape_scene::builder::{self, DisplayMode, GpuCache, SceneOptions};
use noshape_units::{ExprValue, UnitPrefs};

#[test]
fn cylinder_sides_show_silhouettes() {
    let mut r = match Renderer::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };
    let units = UnitPrefs::default();
    let mut ps = PartStudio::new();
    let mut sk = noshape_sketch::Sketch::new();
    sk.add_circle(DVec2::ZERO, 0.01);
    let mut sketch = ps.new_feature("sketch", &units).unwrap();
    sketch.params.set("sketchPlane", ParamValue::Query(vec![Reference::Plane(DatumPlane::Top)]));
    sketch.params.set("sketch", ParamValue::Sketch(Box::new(SketchData { sketch: sk, ..Default::default() })));
    let sid = sketch.id;
    ps.features.push(sketch);
    let regen = |ps: &PartStudio| Regenerator::new().regenerate(ps, &ConfigValues::default(), &units, &RegenOptions::default());
    let key = regen(&ps).display.sketches[0].regions[0].key.clone();
    let mut ex = ps.new_feature("extrude", &units).unwrap();
    ex.params.set("entities", ParamValue::Query(vec![Reference::SketchRegion { sketch: sid, key }]));
    ex.params.set("depth", ParamValue::Expr(ExprValue::new("20 mm")));
    ps.features.push(ex);
    let out = regen(&ps);
    assert_eq!(out.display.bodies.len(), 1);
    let candidates = builder::silhouette_edges(&out.display.bodies[0].mesh);
    assert!(!candidates.segments.is_empty(), "the curved face has silhouette candidates");

    let style = noshape_scene::png::viewport_style(&noshape_scene::GraphicsPrefs::default());
    let up = r.uploader();
    let (w, h) = (400u32, 300u32);
    let mut frames = Vec::new();
    for mode in [DisplayMode::ShadedWithEdges, DisplayMode::Shaded] {
        let mut cache = GpuCache::default();
        let built = builder::build(&out.display, &SceneOptions { display_mode: mode, show_planes: false, show_origin: false, show_sketches: false, ..Default::default() }, &mut cache, &up);
        // Onshape's backface silhouettes: one pass for the solid, with edges only.
        let back = built.scene.instances.iter().filter(|i| i.back_silhouette).count();
        assert_eq!(back, usize::from(mode == DisplayMode::ShadedWithEdges), "{mode:?}: backface silhouette instances");
        let (lo, hi) = built.scene.bounds().map(|(l, h)| (l.as_dvec3(), h.as_dvec3())).unwrap();
        for projection in [Projection::Orthographic, Projection::Perspective] {
            let mut cam = noshape_scene::png::fit_camera(lo, hi, w as f64 / h as f64, Some(noshape_render::StandardView::Front));
            cam.projection = projection;
            let f = r.render(RenderParams { scene: &built.scene, camera: &cam, width: w, height: h, device_pixel_ratio: 1.0, style: &style, pick: false }).unwrap();
            if let Ok(dir) = std::env::var("NOSHAPE_TEST_OUT") {
                let png = noshape_scene::png::encode_png(f.bgra.clone(), w, h, w, h).unwrap();
                std::fs::write(format!("{dir}/silhouettes_{mode:?}_{projection:?}.png"), png).unwrap();
            }
            let mid = cam.project(glam::DVec3::new(0.0, 0.0, 0.01), glam::DVec2::new(w as f64, h as f64), 0.05).unwrap();
            frames.push((mode, projection, mid.y as u32, f.bgra));
        }
    }
    // The row through the cylinder's middle, with and without silhouettes:
    // much darker at both sides of the outline, the same across the face.
    for projection in [Projection::Orthographic, Projection::Perspective] {
        let get = |mode| frames.iter().find(|f| f.0 == mode && f.1 == projection).unwrap();
        let (with, without) = (get(DisplayMode::ShadedWithEdges), get(DisplayMode::Shaded));
        let y = with.2;
        let luma = |bgra: &[u8], x: u32| {
            let o = ((y * w + x) * 4) as usize;
            0.11 * bgra[o] as f32 + 0.59 * bgra[o + 1] as f32 + 0.3 * bgra[o + 2] as f32
        };
        let covered: Vec<u32> = (0..w).filter(|&x| luma(&without.3, x) < 250.0).collect();
        let (left, right) = (*covered.first().unwrap(), *covered.last().unwrap());
        for (side, range) in [("left", left.saturating_sub(1)..=left + 2), ("right", right - 2..=right + 1)] {
            let darkest = |bgra: &[u8]| range.clone().map(|x| luma(bgra, x)).fold(f32::MAX, f32::min);
            let (a, b) = (darkest(&with.3), darkest(&without.3));
            assert!(a < 0.75 * b, "{projection:?}: {side} side not outlined: {a:.0} with silhouettes, {b:.0} without");
        }
        let changed = ((left + 4)..=(right - 4)).filter(|&x| (luma(&with.3, x) - luma(&without.3, x)).abs() > 2.0).count();
        assert_eq!(changed, 0, "{projection:?}: silhouettes drawn across the face");
    }
}
