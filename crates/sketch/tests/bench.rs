//! Simple timing benchmarks (run with `cargo test --release -p noshape-sketch --test bench -- --ignored --nocapture`).

use noshape_sketch::*;
use std::time::Instant;

fn v(x: f64, y: f64) -> DVec2 {
    DVec2::new(x, y)
}

/// Builds a connected sketch with `n_rect` rectangles, one circle per
/// rectangle, link lines and a few arcs. For n_rect = 16 this is
/// 100 curve entities and 150 constraints.
fn build(n_rect: usize, n_arcs: usize) -> (Sketch, EntityId) {
    let mut sk = Sketch::new();
    let mut rects = Vec::new();
    let mut circles = Vec::new();
    for i in 0..n_rect {
        let x0 = i as f64 * 3.0;
        let h = 1.0 + 0.1 * i as f64;
        let p = [sk.add_point(v(x0, 0.0)), sk.add_point(v(x0 + 2.0, 0.0)), sk.add_point(v(x0 + 2.0, h)), sk.add_point(v(x0, h))];
        let l: Vec<EntityId> = (0..4).map(|k| sk.add_line_between(p[k], p[(k + 1) % 4]).unwrap()).collect();
        sk.add_constraint(ConstraintKind::Horizontal { a: l[0], b: None }).unwrap();
        sk.add_constraint(ConstraintKind::Horizontal { a: l[2], b: None }).unwrap();
        sk.add_constraint(ConstraintKind::Vertical { a: l[1], b: None }).unwrap();
        sk.add_constraint(ConstraintKind::Vertical { a: l[3], b: None }).unwrap();
        sk.add_constraint(ConstraintKind::Length { curve: l[0], value: 2.0 }).unwrap();
        let c = sk.add_circle(v(x0 + 0.4, 0.4), 0.4);
        sk.add_constraint(ConstraintKind::Tangent { a: l[0], b: c }).unwrap();
        sk.add_constraint(ConstraintKind::Tangent { a: l[3], b: c }).unwrap();
        if let Some(&prev) = circles.last() {
            sk.add_constraint(ConstraintKind::Equal { a: prev, b: c }).unwrap();
        }
        circles.push(c);
        rects.push((p, l));
    }
    for i in 0..n_rect - 1 {
        let a = rects[i].0[2];
        let b = rects[i + 1].0[0];
        let l = sk.add_line_between(a, b).unwrap();
        let len = (sk.point(b).unwrap() - sk.point(a).unwrap()).length();
        sk.add_constraint(ConstraintKind::Length { curve: l, value: len }).unwrap();
    }
    for k in 0..n_arcs {
        let c = circles[k % circles.len()];
        let cc = match sk.curve(c).unwrap() {
            Curve2::Circle { center, radius } => (center, radius),
            _ => unreachable!(),
        };
        let center = cc.0 + v(0.0, 2.0 * cc.1 + 0.5);
        let r = 0.5;
        let arc = sk.add_arc(center, center + v(r, 0.0), center + v(-r, 0.0));
        sk.add_constraint(ConstraintKind::Tangent { a: arc, b: c }).unwrap();
        if k % 2 == 0 {
            sk.add_constraint(ConstraintKind::Radius { curve: arc, value: r }).unwrap();
        }
    }
    let r = sk.solve();
    assert!(r.is_solved(), "{r:?}");
    let drag_pt = rects[n_rect - 1].0[2];
    (sk, drag_pt)
}

fn curve_count(sk: &Sketch) -> usize {
    sk.entities().filter(|e| e.kind() != EntityKind::Point).count()
}

fn bench(n_rect: usize, n_arcs: usize, iters: usize) {
    let (mut sk, p) = build(n_rect, n_arcs);
    let start = sk.point(p).unwrap();
    let t = Instant::now();
    let mut worst = 0.0f64;
    for i in 0..iters {
        let a = i as f64 * 0.05;
        let target = start + v(0.3 * a.cos(), 0.3 * a.sin());
        let t1 = Instant::now();
        let r = sk.drag(&[(DragHandle::Point(p), target)]);
        worst = worst.max(t1.elapsed().as_secs_f64());
        assert!(r.is_solved(), "{r:?}");
    }
    let per = t.elapsed().as_secs_f64() / iters as f64;
    let t2 = Instant::now();
    let r = sk.solve();
    let full = t2.elapsed().as_secs_f64();
    let t3 = Instant::now();
    let regions = sk.regions();
    let treg = t3.elapsed().as_secs_f64();
    let t4 = Instant::now();
    let ctx = InferenceContext { tool: SketchTool::Line, snap_radius: 0.05, ..Default::default() };
    let n_inf = 100;
    for i in 0..n_inf {
        let _ = infer(&sk, start + v(0.01 * i as f64, 0.003 * i as f64), &ctx);
    }
    let tinf = t4.elapsed().as_secs_f64() / n_inf as f64;
    println!(
        "{} curves / {} entities / {} constraints: drag {:.3} ms avg ({:.3} ms worst), full solve + DOF analysis {:.3} ms (dof {:?}), regions {:.2} ms ({} regions), inference {:.3} ms",
        curve_count(&sk),
        sk.entity_count(),
        sk.constraint_count(),
        per * 1e3,
        worst * 1e3,
        full * 1e3,
        r.dof,
        treg * 1e3,
        regions.len(),
        tinf * 1e3
    );
}

#[test]
#[ignore]
fn bench_drag() {
    bench(16, 5, 300);
    bench(64, 20, 100);
    bench(250, 50, 20);
}

#[test]
fn bench_sketch_is_consistent() {
    let (sk, _) = build(16, 5);
    assert_eq!(curve_count(&sk), 100);
    assert_eq!(sk.constraint_count(), 150);
}
