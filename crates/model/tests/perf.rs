//! Performance benchmarks (run with `--release --test all perf:: -- --ignored --nocapture`).

use glam::DVec2;
use noshape_model::configuration::ConfigValues;
use noshape_model::params::ParamValue;
use noshape_model::partstudio::PartStudio;
use noshape_model::reference::{DatumPlane, Reference};
use noshape_model::regen::{RegenOptions, Regenerator};
use noshape_model::sketch_feature::SketchData;
use noshape_units::{ExprValue, UnitPrefs};
use std::time::Instant;

fn perforated_plate(n: usize) -> PartStudio {
    let units = UnitPrefs::default();
    let mut sk = noshape_sketch::Sketch::new();
    let size = 0.2;
    let p = [DVec2::new(0.0, 0.0), DVec2::new(size, 0.0), DVec2::new(size, size), DVec2::new(0.0, size)];
    let pts: Vec<_> = p.iter().map(|q| sk.add_point(*q)).collect();
    for i in 0..4 {
        sk.add_line_between(pts[i], pts[(i + 1) % 4]).unwrap();
    }
    let pitch = size / (n as f64 + 1.0);
    for i in 0..n {
        for j in 0..n {
            sk.add_circle(DVec2::new(pitch * (i as f64 + 1.0), pitch * (j as f64 + 1.0)), pitch * 0.3);
        }
    }
    let mut ps = PartStudio::new();
    let mut sketch = ps.new_feature("sketch", &units).unwrap();
    sketch.params.set("sketchPlane", ParamValue::Query(vec![Reference::Plane(DatumPlane::Top)]));
    sketch.params.set("sketch", ParamValue::Sketch(Box::new(SketchData { sketch: sk, ..Default::default() })));
    let sid = sketch.id;
    ps.features.push(sketch);
    let mut reg = Regenerator::new();
    let r = reg.regenerate(&ps, &ConfigValues::default(), &units, &RegenOptions::default());
    let outer = r.display.sketches[0].regions.iter().max_by(|a, b| a.area.partial_cmp(&b.area).unwrap()).unwrap().key.clone();
    let mut ex = ps.new_feature("extrude", &units).unwrap();
    ex.params.set("entities", ParamValue::Query(vec![Reference::SketchRegion { sketch: sid, key: outer }]));
    ex.params.set("depth", ParamValue::Expr(ExprValue::new("10 mm")));
    ps.features.push(ex);
    ps
}

#[test]
#[ignore]
fn bench_perforated_plate() {
    let _ = env_logger::builder().is_test(true).filter_level(log::LevelFilter::Debug).try_init();
    let units = UnitPrefs::default();
    let mut ps = perforated_plate(20);
    let mut reg = Regenerator::new();
    let t = Instant::now();
    let r = reg.regenerate(&ps, &ConfigValues::default(), &units, &RegenOptions::default());
    eprintln!("initial regen (sketch 404 curves + extrude 400 holes): {:?} (regen_ms {:.1})", t.elapsed(), r.display.regen_ms);
    for f in &r.display.features {
        eprintln!("  feature time {:.1} ms  {:?}", f.time_ms, f.status);
    }
    let tris: usize = r.display.bodies.iter().map(|b| b.mesh.indices.len() / 3).sum();
    eprintln!("  triangles: {tris}, faces: {}", r.context.bodies[0].faces.len());

    // Fillet all top edges (cap end).
    let body = &r.context.bodies[0];
    let top: Vec<Reference> = body.edges.iter().filter(|e| format!("{e:?}").contains("CapEnd")).map(|e| Reference::Edge((**e).clone())).collect();
    eprintln!("  filleting {} edges", top.len());
    let mut f = ps.new_feature("fillet", &units).unwrap();
    f.params.set("entities", ParamValue::Query(top));
    f.params.set("radius", ParamValue::Expr(ExprValue::new("0.5 mm")));
    f.params.set("tangentPropagation", ParamValue::Bool(false));
    ps.features.push(f);
    let t = Instant::now();
    let r = reg.regenerate(&ps, &ConfigValues::default(), &units, &RegenOptions::default());
    eprintln!("add fillet (upstream cached): {:?}", t.elapsed());
    for f in &r.display.features {
        eprintln!("  feature time {:.1} ms  {:?} {:?}", f.time_ms, f.status, f.message);
    }
    let tris: usize = r.display.bodies.iter().map(|b| b.mesh.indices.len() / 3).sum();
    eprintln!("  triangles: {tris}");

    // Edit only the fillet radius: sketch + extrude must come from cache.
    ps.features[2].params.set("radius", ParamValue::Expr(ExprValue::new("0.8 mm")));
    let t = Instant::now();
    let _ = reg.regenerate(&ps, &ConfigValues::default(), &units, &RegenOptions::default());
    eprintln!("edit fillet radius: {:?}", t.elapsed());

    let t = Instant::now();
    let _ = reg.regenerate(&ps, &ConfigValues::default(), &units, &RegenOptions::default());
    eprintln!("no-op regen (fully cached): {:?}", t.elapsed());
}
