//! Sketch editing latency on large sketches (perf.yml, not ci.yml):
//! `cargo test --release -p noshape-scene --test all sketch_perf:: -- --ignored --nocapture`.
//! Dragging a point of a ~300-entity dimensioned sketch and the seed of a
//! 50-instance linked pattern, step by step through the editor (what a
//! pointer move does), and adding a constraint (add + solve). Targets (the
//! medians, release build): a drag step with the regeneration within a
//! 60 Hz frame, adding a constraint (even a conflicting one) under 50 ms.

use glam::DVec2;
use noshape_model::reference::FeatureId;
use noshape_model::display::PlaneFrame;
use noshape_model::sketch_feature::SketchData;
use noshape_scene::sketch_mode::{SketchEditor, Tool};
use noshape_model::configuration::ConfigValues;
use noshape_model::params::ParamValue;
use noshape_model::partstudio::PartStudio;
use noshape_model::reference::{DatumPlane, Reference};
use noshape_model::regen::{RegenOptions, Regenerator};
use noshape_sketch::{ConstraintKind, Curve2, EntityId, Sketch, SolveOptions};
use noshape_units::UnitPrefs;
use std::time::{Duration, Instant};

/// One display frame at 60 Hz.
const FRAME: Duration = Duration::from_micros(16_667);
/// Adding a constraint shows within this.
const CONSTRAINT: Duration = Duration::from_millis(50);

fn v(x: f64, y: f64) -> DVec2 {
    DVec2::new(x, y)
}

/// `n` dimensioned rectangles with a tangent circle each (equal circles),
/// linked by dimensioned lines: 6 curves + 4 points a rectangle.
fn big_sketch(n: usize) -> (Sketch, EntityId) {
    let mut sk = Sketch::new();
    let mut last = None;
    let mut circles = Vec::new();
    let mut corner = None;
    for i in 0..n {
        let x0 = i as f64 * 0.03;
        let h = 0.01 + 0.001 * (i % 7) as f64;
        let p = [sk.add_point(v(x0, 0.0)), sk.add_point(v(x0 + 0.02, 0.0)), sk.add_point(v(x0 + 0.02, h)), sk.add_point(v(x0, h))];
        let l: Vec<EntityId> = (0..4).map(|k| sk.add_line_between(p[k], p[(k + 1) % 4]).unwrap()).collect();
        sk.add_constraint(ConstraintKind::Horizontal { a: l[0], b: None }).unwrap();
        sk.add_constraint(ConstraintKind::Horizontal { a: l[2], b: None }).unwrap();
        sk.add_constraint(ConstraintKind::Vertical { a: l[1], b: None }).unwrap();
        sk.add_constraint(ConstraintKind::Vertical { a: l[3], b: None }).unwrap();
        sk.add_constraint(ConstraintKind::Length { curve: l[0], value: 0.02 }).unwrap();
        let c = sk.add_circle(v(x0 + 0.004, 0.004), 0.004);
        sk.add_constraint(ConstraintKind::Tangent { a: l[0], b: c }).unwrap();
        sk.add_constraint(ConstraintKind::Tangent { a: l[3], b: c }).unwrap();
        if let Some(&prev) = circles.last() {
            sk.add_constraint(ConstraintKind::Equal { a: prev, b: c }).unwrap();
        }
        circles.push(c);
        if let Some(prev) = last {
            let l = sk.add_line_between(prev, p[0]).unwrap();
            let len = (sk.point(p[0]).unwrap() - sk.point(prev).unwrap()).length();
            sk.add_constraint(ConstraintKind::Length { curve: l, value: len }).unwrap();
        }
        last = Some(p[2]);
        corner = Some(p[2]);
    }
    assert!(sk.solve().is_solved());
    (sk, corner.unwrap())
}

struct Stats {
    median: Duration,
    worst: Duration,
}

fn stats(mut t: Vec<Duration>) -> Stats {
    t.sort();
    Stats { median: t[t.len() / 2], worst: *t.last().unwrap() }
}

fn drag_steps(ed: &mut SketchEditor, data: &mut SketchData, from: DVec2, steps: usize) -> Stats {
    ed.pointer_move(data, from, false, false);
    ed.tool_down(data, from, ed.hover, false, false);
    let mut times = Vec::new();
    for i in 1..=steps {
        let a = i as f64 * 0.05;
        let p = from + v(0.004 * a.cos() + 0.004 * i as f64 / steps as f64, 0.004 * a.sin());
        let t = Instant::now();
        ed.pointer_move(data, p, false, false);
        ed.tool_drag(data, p);
        times.push(t.elapsed());
    }
    ed.tool_up(data, from);
    stats(times)
}

fn editor() -> SketchEditor {
    let mut ed = SketchEditor::new(FeatureId(1), PlaneFrame::TOP);
    ed.world_per_px = 1e-4;
    ed
}

#[test]
#[ignore]
fn sketch_drag_and_constraint_latency() {
    // ~300 entities with dimensions and constraints.
    let (sk, corner) = big_sketch(30);
    let curves = sk.entities().filter(|e| e.kind() != noshape_sketch::EntityKind::Point).count();
    println!("big sketch: {} entities ({curves} curves), {} constraints", sk.entity_count(), sk.constraint_count());
    let mut data = SketchData { sketch: sk, ..Default::default() };
    let mut ed = editor();
    ed.set_tool(Tool::Select);
    let from = data.sketch.point(corner).unwrap();
    ed.hover = Some(corner);
    let s = drag_steps(&mut ed, &mut data, from, 120);
    println!("  drag a corner: median {:.2} ms, worst {:.2} ms", s.median.as_secs_f64() * 1e3, s.worst.as_secs_f64() * 1e3);
    assert!(s.median < FRAME, "corner drag: median {:?}", s.median);
    // Adding a dimension, as the Dimension tool does: add + solve. A
    // rectangle's height (free) and, the worst case, a second length on a
    // dimensioned line (conflicting: the solver finds the conflict).
    let lines: Vec<EntityId> = data.sketch.entities().filter(|e| e.kind() == noshape_sketch::EntityKind::Line).map(|e| e.id).collect();
    for (what, line, value) in [("a free height", lines[1], 0.012), ("a conflicting length", lines[0], 0.025)] {
        let mut adds = Vec::new();
        for k in 0..10 {
            let mut d = data.clone();
            let t = Instant::now();
            let c = d.sketch.add_constraint(ConstraintKind::Length { curve: line, value: value + 0.0001 * k as f64 }).unwrap();
            let r = d.sketch.solve();
            adds.push(t.elapsed());
            assert!(r.is_solved() || !r.conflicting().is_empty(), "{c:?} {r:?}");
        }
        let s = stats(adds);
        println!("  add {what} + solve: median {:.2} ms, worst {:.2} ms", s.median.as_secs_f64() * 1e3, s.worst.as_secs_f64() * 1e3);
        assert!(s.median < CONSTRAINT, "adding {what}: median {:?}", s.median);
    }
    let t = Instant::now();
    let regions = data.sketch.regions();
    println!("  regions: {} in {:.2} ms", regions.len(), t.elapsed().as_secs_f64() * 1e3);
    // Re-solving the solved sketch (what every regeneration does).
    let mut sk = data.sketch.clone();
    let t = Instant::now();
    sk.solve_with(&SolveOptions { analyze: false, ..Default::default() });
    let plain = t.elapsed();
    let t = Instant::now();
    sk.solve();
    println!("  re-solve: {:.2} ms, with the DOF analysis {:.2} ms", plain.as_secs_f64() * 1e3, t.elapsed().as_secs_f64() * 1e3);
    // The app regenerates the studio after every drag step (regen_sync).
    let units = UnitPrefs::default();
    let mut ps = PartStudio::new();
    let mut f = ps.new_feature("sketch", &units).unwrap();
    f.params.set("sketchPlane", ParamValue::Query(vec![Reference::Plane(DatumPlane::Top)]));
    f.params.set("sketch", ParamValue::Sketch(Box::new(data.clone())));
    let fid = f.id;
    ps.features.push(f);
    let mut regen = Regenerator::new();
    let opts = RegenOptions { editing_sketch: Some(fid), ..Default::default() };
    regen.regenerate(&ps, &ConfigValues::default(), &units, &opts);
    let mut ed = editor();
    ed.set_tool(Tool::Select);
    let from = data.sketch.point(corner).unwrap();
    ed.hover = Some(corner);
    ed.pointer_move(&mut data, from, false, false);
    ed.tool_down(&mut data, from, Some(corner), false, false);
    let mut frames = Vec::new();
    for i in 1..=40 {
        let p = from + v(0.0002 * i as f64, 0.0001 * i as f64);
        let t = Instant::now();
        ed.pointer_move(&mut data, p, false, false);
        let t_move = t.elapsed();
        ed.tool_drag(&mut data, p);
        let t_edit = t.elapsed();
        ps.features[0].params.set("sketch", ParamValue::Sketch(Box::new(data.clone())));
        regen.regenerate(&ps, &ConfigValues::default(), &units, &opts);
        frames.push(t.elapsed());
        if t.elapsed() > Duration::from_millis(30) {
            println!("    frame {i}: pointer move {:.1} ms, edit {:.1} ms, total {:.1} ms", t_move.as_secs_f64() * 1e3, t_edit.as_secs_f64() * 1e3, t.elapsed().as_secs_f64() * 1e3);
        }
    }
    ed.tool_up(&mut data, from);
    let s = stats(frames);
    println!("  drag frame with regeneration: median {:.2} ms, worst {:.2} ms", s.median.as_secs_f64() * 1e3, s.worst.as_secs_f64() * 1e3);
    assert!(s.median < FRAME, "drag frame with regeneration: median {:?}", s.median);

    // A circle patterned 50 times, linked to its seed.
    let mut data = SketchData::default();
    let c = data.sketch.add_circle(v(0.0, 0.0), 0.002);
    let mut ed = editor();
    ed.selection = vec![c];
    ed.set_tool(Tool::LinearPattern);
    ed.pattern_count = 50;
    ed.pattern_distance = 0.006;
    let t = Instant::now();
    assert!(ed.apply_pattern(&mut data).changed);
    println!("pattern: 50 instances in {:.2} ms; {} entities, {} constraints", t.elapsed().as_secs_f64() * 1e3, data.sketch.entity_count(), data.sketch.constraint_count());
    ed.set_tool(Tool::Select);
    let Some(Curve2::Circle { center, radius }) = data.sketch.curve(c) else { panic!() };
    let grab = center + v(radius, 0.0);
    ed.hover = Some(c);
    let s = drag_steps(&mut ed, &mut data, grab, 120);
    println!("  drag the seed: median {:.2} ms, worst {:.2} ms", s.median.as_secs_f64() * 1e3, s.worst.as_secs_f64() * 1e3);
    assert!(s.median < FRAME, "pattern seed drag: median {:?}", s.median);
}

/// The regeneration after each drag step alone (to profile it).
#[test]
#[ignore]
fn sketch_drag_regeneration_only() {
    let (sk, corner) = big_sketch(30);
    let mut data = SketchData { sketch: sk, ..Default::default() };
    let units = UnitPrefs::default();
    let mut ps = PartStudio::new();
    let mut f = ps.new_feature("sketch", &units).unwrap();
    f.params.set("sketchPlane", ParamValue::Query(vec![Reference::Plane(DatumPlane::Top)]));
    f.params.set("sketch", ParamValue::Sketch(Box::new(data.clone())));
    let fid = f.id;
    ps.features.push(f);
    let mut regen = Regenerator::new();
    let opts = RegenOptions { editing_sketch: Some(fid), ..Default::default() };
    regen.regenerate(&ps, &ConfigValues::default(), &units, &opts);
    let from = data.sketch.point(corner).unwrap();
    let mut frames = Vec::new();
    for i in 1..=200 {
        let p = from + v(0.00002 * (i % 40) as f64, 0.00001 * (i % 40) as f64);
        data.sketch.drag(&[(noshape_sketch::DragHandle::Point(corner), p)]);
        let t = Instant::now();
        ps.features[0].params.set("sketch", ParamValue::Sketch(Box::new(data.clone())));
        regen.regenerate(&ps, &ConfigValues::default(), &units, &opts);
        frames.push(t.elapsed());
    }
    let s = stats(frames);
    println!("regeneration per drag step: median {:.2} ms, worst {:.2} ms", s.median.as_secs_f64() * 1e3, s.worst.as_secs_f64() * 1e3);
}

/// qa SKETCH-DRAG-BIG: 10 × 10 corner rectangles and 100 circles (made as
/// sketch_draw shapes make them), a corner dragged: the solve, the regions
/// and the regeneration of each step, timed apart.
#[test]
#[ignore]
fn big_shape_sketch_drag_step_parts() {
    let mut sk = Sketch::new();
    let mut corner = None;
    for i in 0..10 {
        for j in 0..10 {
            let (x, y) = (i as f64 * 0.03, j as f64 * 0.03);
            let r = sk.corner_rectangle(v(x, y), v(x + 0.02, y + 0.015));
            corner.get_or_insert(r.points[2]);
            sk.add_circle(v(x + 0.025, y + 0.024), 0.003);
        }
    }
    let corner = corner.unwrap();
    sk.solve();
    let mut data = SketchData { sketch: sk, ..Default::default() };
    let units = UnitPrefs::default();
    let mut ps = PartStudio::new();
    let mut f = ps.new_feature("sketch", &units).unwrap();
    f.params.set("sketchPlane", ParamValue::Query(vec![Reference::Plane(DatumPlane::Top)]));
    f.params.set("sketch", ParamValue::Sketch(Box::new(data.clone())));
    let fid = f.id;
    ps.features.push(f);
    let mut regen = Regenerator::new();
    let opts = RegenOptions { editing_sketch: Some(fid), ..Default::default() };
    regen.regenerate(&ps, &ConfigValues::default(), &units, &opts);
    let from = data.sketch.point(corner).unwrap();
    let (mut solve, mut regions, mut regenerate) = (Vec::new(), Vec::new(), Vec::new());
    for i in 1..=60 {
        let p = from + v(0.00002 * (i % 40) as f64, 0.00001 * (i % 40) as f64);
        let t = Instant::now();
        data.sketch.drag(&[(noshape_sketch::DragHandle::Point(corner), p)]);
        solve.push(t.elapsed());
        let t = Instant::now();
        let _ = data.sketch.regions();
        regions.push(t.elapsed());
        let t = Instant::now();
        ps.features[0].params.set("sketch", ParamValue::Sketch(Box::new(data.clone())));
        regen.regenerate(&ps, &ConfigValues::default(), &units, &opts);
        regenerate.push(t.elapsed());
    }
    let ms = |s: Stats| format!("median {:.2} ms, worst {:.2} ms", s.median.as_secs_f64() * 1e3, s.worst.as_secs_f64() * 1e3);
    println!("{} entities: drag solve {}; regions {}; regeneration {}", data.sketch.entities().count(), ms(stats(solve)), ms(stats(regions)), ms(stats(regenerate)));
}
