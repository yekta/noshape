//! noshape's own display mesher (crates/brep `mesh`) against BRepMesh as the viewport shows them (lead 10-28:
//! "pixel checks on a few offscreen renders"): live's 30-feature part (planes, cylinders, cones, tori) regenerated
//! with each, rendered shaded with edges from three views; the images match but for a few pixels of shading
//! (the two triangulate a face's inside differently; edges, silhouettes and normals are the same).
//! NOSHAPE_TEST_OUT=<dir> writes the images.

use glam::DVec3;
use noshape_model::configuration::ConfigValues;
use noshape_model::document::TabContent;
use noshape_model::regen::{RegenOptions, Regenerator};
use noshape_ops::Ctx;
use noshape_render::renderer::RenderParams;
use noshape_render::{Camera, Projection};
use noshape_scene::builder::{self, DisplayMode, SceneOptions};

#[test]
fn live_30_features_look_the_same_meshed_by_noshape_and_by_brepmesh() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/perf/live-30-features.noshape");
    let tmp = std::env::temp_dir().join(format!("noshape-mesh-pixels-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    let copy = tmp.join("live-30-features.noshape");
    std::fs::copy(&path, &copy).unwrap();
    let s = noshape_ops::session::Session::open(&copy).unwrap();
    let ps = s.doc.tabs.iter().find_map(|t| if let TabContent::PartStudio(ps) = &t.content { Some(ps.clone()) } else { None }).unwrap();
    // Each way from scratch (new shapes: no mesh cached from the other).
    let regen = |rust: bool| {
        noshape_kernel::set_rust_mesh_everywhere(rust);
        let r = Regenerator::new().regenerate(&ps, &ConfigValues::default(), &s.doc.settings.units, &RegenOptions::default());
        noshape_kernel::set_rust_mesh_everywhere(true);
        r
    };
    let (ours, occt) = (regen(true), regen(false));
    let _ = std::fs::remove_dir_all(&tmp);
    let mut ctx = Ctx::new();
    // Rendering needs a GPU adapter; CI's Linux runner has none.
    let off = match ctx.offscreen() {
        Ok(off) => off,
        Err(e) if e.code == "render_failed" => return eprintln!("render skipped: {}", e.message),
        Err(e) => panic!("offscreen: {}", e.to_json()),
    };
    let up = off.renderer.uploader();
    let opts = SceneOptions { display_mode: DisplayMode::ShadedWithEdges, ..Default::default() };
    let (lo, hi) = ours.display.bodies.iter().filter_map(|b| b.bounds).fold((DVec3::MAX, DVec3::MIN), |(a, b), (l, h)| (a.min(l), b.max(h)));
    assert!(lo.x < hi.x, "a part");
    let style = noshape_scene::png::viewport_style(&noshape_scene::GraphicsPrefs::default());
    let out = std::env::var("NOSHAPE_TEST_OUT").ok();
    let (w, h) = (800u32, 600u32);
    let mut report = Vec::new();
    for (name, look) in [("iso", DVec3::new(-1.0, 1.0, -1.0)), ("top", DVec3::new(0.0, 0.01, -1.0)), ("low", DVec3::new(1.0, 0.6, -0.3))] {
        let mut camera = Camera { rotation: Camera::look_rotation(look.normalize(), DVec3::Z), projection: Projection::Perspective, ..Default::default() };
        camera.fit_box(lo, hi, w as f64 / h as f64);
        let mut frames = Vec::new();
        let empty = noshape_model::display::StudioDisplay::default();
        for (way, d) in [("ours", &ours.display), ("brepmesh", &occt.display), ("background", &empty)] {
            let built = builder::build(d, &opts, &mut off.cache, &up);
            let f = off.renderer.render(RenderParams { scene: &built.scene, camera: &camera, width: w, height: h, device_pixel_ratio: 1.0, style: &style, pick: false }).unwrap();
            if let Some(dir) = &out {
                std::fs::write(format!("{dir}/live30_{name}_{way}.png"), noshape_scene::png::encode_png(f.bgra.clone(), w, h, w, h).unwrap()).unwrap();
            }
            frames.push(f.bgra);
        }
        // Pixels whose colour differs by more than a shading step; the part's, where BRepMesh's isn't background.
        let apart = |a: &[u8], b: &[u8]| a.iter().zip(b).take(3).any(|(x, y)| x.abs_diff(*y) > 24);
        let differ = frames[0].chunks(4).zip(frames[1].chunks(4)).filter(|(a, b)| apart(a, b)).count();
        let part = frames[1].chunks(4).zip(frames[2].chunks(4)).filter(|(a, b)| apart(a, b)).count();
        report.push((name, differ, part));
    }
    eprintln!("mesh pixels (view, differing, part pixels): {report:?}");
    for (name, differ, part) in report {
        assert!(part > 10_000, "{name}: the part fills {part} pixels");
        assert!(differ * 200 <= part, "{name}: {differ} of the part's {part} pixels differ");
    }
}
