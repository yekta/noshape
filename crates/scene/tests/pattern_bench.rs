//! A 1000-instance part pattern through regeneration, scene building and
//! rendering, timed per stage (viewport-performance.md, instancing):
//! `cargo test --release -p noshape-scene --test pattern_bench -- --ignored --nocapture`.

use glam::DVec2;
use noshape_model::configuration::ConfigValues;
use noshape_model::params::ParamValue;
use noshape_model::partstudio::PartStudio;
use noshape_model::reference::{DatumPlane, Reference};
use noshape_model::regen::{RegenOptions, Regenerator};
use noshape_model::sketch_feature::SketchData;
use noshape_render::renderer::{RenderParams, Renderer};
use noshape_render::{Camera, Style};
use noshape_units::{ExprValue, UnitPrefs};
use std::time::Instant;

/// A 20 x 20 x 10 mm block with a 6 mm hole, patterned 40 x 25 along X and Y.
fn pattern_studio(units: &UnitPrefs) -> PartStudio {
    let mut ps = PartStudio::new();
    let mut sk = noshape_sketch::Sketch::new();
    let p = [DVec2::new(0.0, 0.0), DVec2::new(0.02, 0.0), DVec2::new(0.02, 0.02), DVec2::new(0.0, 0.02)];
    let pts: Vec<_> = p.iter().map(|q| sk.add_point(*q)).collect();
    for i in 0..4 {
        sk.add_line_between(pts[i], pts[(i + 1) % 4]).unwrap();
    }
    sk.add_circle(DVec2::new(0.01, 0.01), 0.003);
    let mut sketch = ps.new_feature("sketch", units).unwrap();
    sketch.params.set("sketchPlane", ParamValue::Query(vec![Reference::Plane(DatumPlane::Top)]));
    sketch.params.set("sketch", ParamValue::Sketch(Box::new(SketchData { sketch: sk, ..Default::default() })));
    let sid = sketch.id;
    ps.features.push(sketch);
    let r = Regenerator::new().regenerate(&ps, &ConfigValues::default(), units, &RegenOptions::default());
    let region = r.display.sketches[0].regions.iter().max_by(|a, b| a.area.partial_cmp(&b.area).unwrap()).unwrap();
    let mut ex = ps.new_feature("extrude", units).unwrap();
    ex.params.set("entities", ParamValue::Query(vec![Reference::SketchRegion { sketch: sid, key: region.key.clone() }]));
    ex.params.set("depth", ParamValue::Expr(ExprValue::new("10 mm")));
    ps.features.push(ex);
    let r = Regenerator::new().regenerate(&ps, &ConfigValues::default(), units, &RegenOptions::default());
    let body = r.display.bodies[0].id.clone();
    let mut pat = ps.new_feature("linearPattern", units).unwrap();
    pat.params.set("patternType", ParamValue::Enum("PART".into()));
    pat.params.set("entities", ParamValue::Query(vec![Reference::Body(body)]));
    pat.params.set("directionOne", ParamValue::Query(vec![Reference::Plane(DatumPlane::Right)]));
    pat.params.set("distance", ParamValue::Expr(ExprValue::new("30 mm")));
    pat.params.set("instanceCount", ParamValue::Expr(ExprValue::new("40")));
    pat.params.set("hasSecondDir", ParamValue::Bool(true));
    pat.params.set("directionTwo", ParamValue::Query(vec![Reference::Plane(DatumPlane::Front)]));
    pat.params.set("distanceTwo", ParamValue::Expr(ExprValue::new("30 mm")));
    pat.params.set("instanceCountTwo", ParamValue::Expr(ExprValue::new("25")));
    ps.features.push(pat);
    ps
}

#[test]
#[ignore]
fn pattern_1000_instances() {
    let units = UnitPrefs::default();
    let ps = pattern_studio(&units);
    let t = Instant::now();
    let r = Regenerator::new().regenerate(&ps, &ConfigValues::default(), &units, &RegenOptions::default());
    let regen = t.elapsed();
    for f in &r.display.features {
        eprintln!("{:?} {:?}", f.status, f.message);
    }
    let bodies = r.display.bodies.len();
    let shared = {
        let mut ptrs: Vec<_> = r.display.bodies.iter().map(|b| std::sync::Arc::as_ptr(&b.mesh)).collect();
        ptrs.sort();
        ptrs.dedup();
        ptrs.len()
    };
    let tris: usize = r.display.bodies.iter().map(|b| b.mesh.indices.len() / 3).sum();
    let mut renderer = match Renderer::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };
    let up = renderer.uploader();
    let mut cache = Default::default();
    let opts = Default::default();
    let t = Instant::now();
    let cold = noshape_scene::builder::build(&r.display, &opts, &mut cache, &up);
    let build_cold = t.elapsed();
    let t = Instant::now();
    let built = noshape_scene::builder::build(&r.display, &opts, &mut cache, &up);
    let build_warm = t.elapsed();
    assert_eq!(cold.scene.instances.len(), built.scene.instances.len());
    let scene = built.scene.clone();
    let (w, h) = (1280u32, 800u32);
    let mut cam = Camera::default();
    let (lo, hi) = scene.bounds().unwrap();
    cam.fit_box(lo.as_dvec3(), hi.as_dvec3(), w as f64 / h as f64);
    let style = Style::default();
    let frame = |renderer: &mut Renderer, cam: &Camera| renderer.render(RenderParams { scene: &scene, camera: cam, width: w, height: h, device_pixel_ratio: 1.0, style: &style, pick: false }).unwrap();
    let t = Instant::now();
    frame(&mut renderer, &cam);
    let first = t.elapsed();
    let n = 20;
    let t = Instant::now();
    for i in 0..n {
        let mut c = cam;
        c.rotation = glam::DQuat::from_rotation_z(i as f64 * 0.02) * c.rotation;
        frame(&mut renderer, &c);
    }
    let per_frame = t.elapsed() / n;
    eprintln!(
        "BENCH pattern: bodies {bodies} (distinct meshes {shared}, {tris} triangles), instances {}, regen {regen:.2?}, scene build cold {build_cold:.2?} warm {build_warm:.2?}, first frame {first:.2?}, frame {per_frame:.2?} (sync render incl. readback)",
        scene.instances.len()
    );
}
