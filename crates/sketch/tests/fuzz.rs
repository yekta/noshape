//! Deterministic randomized robustness tests.
use noshape_sketch::*;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((self.0 >> 11) as f64) / ((1u64 << 53) as f64)
    }
    fn range(&mut self, a: f64, b: f64) -> f64 {
        a + (b - a) * self.next()
    }
}

#[test]
fn random_arrangements_have_closed_consistent_regions() {
    let mut rng = Rng(42);
    for case in 0..60 {
        let mut sk = Sketch::new();
        let n = 2 + case % 7;
        for _ in 0..n {
            match (rng.next() * 4.0) as u32 {
                0 => {
                    sk.add_circle(DVec2::new(rng.range(-2.0, 2.0), rng.range(-2.0, 2.0)), rng.range(0.3, 2.0));
                }
                1 => {
                    sk.add_line(DVec2::new(rng.range(-3.0, 3.0), rng.range(-3.0, 3.0)), DVec2::new(rng.range(-3.0, 3.0), rng.range(-3.0, 3.0)));
                }
                2 => {
                    let c = DVec2::new(rng.range(-2.0, 2.0), rng.range(-2.0, 2.0));
                    let r = rng.range(0.3, 2.0);
                    sk.add_arc_angles(c, r, rng.range(0.0, 6.0), rng.range(0.0, 6.0));
                }
                _ => {
                    sk.corner_rectangle(DVec2::new(rng.range(-3.0, 0.0), rng.range(-3.0, 0.0)), DVec2::new(rng.range(0.0, 3.0), rng.range(0.0, 3.0)));
                }
            }
        }
        let regions = sk.regions();
        for reg in &regions {
            for lp in std::iter::once(&reg.outer).chain(reg.holes.iter()) {
                let m = lp.segments.len();
                for k in 0..m {
                    let gap = (lp.segments[k].end() - lp.segments[(k + 1) % m].start()).length();
                    assert!(gap < 1e-6, "case {case}: gap {gap}");
                }
            }
            assert!(reg.outer.signed_area() > 0.0, "case {case}");
            assert!(reg.area() > -1e-9, "case {case}: {}", reg.area());
        }
        // regions are disjoint: interior points belong to exactly one region
        for reg in &regions {
            if reg.area() < 1e-6 {
                continue;
            }
            let p = reg.interior_point();
            let count = regions.iter().filter(|r| r.contains(p)).count();
            assert_eq!(count, 1, "case {case}");
        }
        let _ = sk.wires();
    }
}

#[test]
fn random_perturbations_resolve() {
    let mut rng = Rng(7);
    for case in 0..40 {
        let mut sk = Sketch::new();
        let r = sk.corner_rectangle(DVec2::new(0.0, 0.0), DVec2::new(2.0, 1.0));
        sk.add_constraint(ConstraintKind::Fix { entity: r.points[0] }).unwrap();
        sk.add_constraint(ConstraintKind::Length { curve: r.curves[0], value: 2.0 }).unwrap();
        let c = sk.add_circle(DVec2::new(1.0, 0.5), 0.3);
        sk.add_constraint(ConstraintKind::Tangent { a: r.curves[0], b: c }).unwrap();
        sk.add_constraint(ConstraintKind::Tangent { a: r.curves[2], b: c }).unwrap();
        let slot = sk.straight_slot(DVec2::new(3.0, 0.0), DVec2::new(5.0, 0.5), 0.5).unwrap();
        sk.add_constraint(ConstraintKind::Distance { a: r.points[1], b: slot.points[0], value: 1.2, mode: DistanceMode::Center }).unwrap();
        assert!(sk.solve().is_solved(), "case {case}");
        // perturb every free point a bit and re-solve
        let ids: Vec<EntityId> = sk.entities().filter(|e| e.kind() == EntityKind::Point).map(|e| e.id).collect();
        for id in ids {
            let p = sk.point(id).unwrap();
            sk.set_point(id, p + DVec2::new(rng.range(-0.05, 0.05), rng.range(-0.05, 0.05))).unwrap();
        }
        let rep = sk.solve();
        assert!(rep.is_solved(), "case {case}: {rep:?}");
        // random drags never break constraints
        for _ in 0..5 {
            let target = DVec2::new(rng.range(2.0, 6.0), rng.range(-1.0, 2.0));
            let rep = sk.drag(&[(DragHandle::Point(slot.points[4]), target)]);
            if rep.is_solved() {
                assert!(rep.max_residual < 1e-9);
            }
        }
        let rep = sk.solve();
        assert!(rep.is_solved(), "case {case}: {rep:?}");
    }
}

/// Random edit sessions: shapes of every builder (rectangles, the center
/// point rectangle's point-to-point midpoints, 3 point circles, polygons,
/// slots, arcs), constraints and dimensions between random entities, trims,
/// splits, fillets, offsets, deletes and drags, each followed by a solve
/// and the regions. Nothing may panic, a solve that reports Solved leaves
/// no residual, and every point stays finite. T1: seeds 1–3 × 60 steps
/// (~1.6 s dev build; seed 2 step 55 is SKETCH-SOLVED-WITH-RESIDUAL's drag);
/// `SKETCH_FUZZ_SEEDS=a..b` / `SKETCH_FUZZ_STEPS=n` widen it.
#[test]
fn random_sketch_edits_never_panic() {
    fuzz((1, 4), 60);
}

/// The wider run (T2): seeds 1–5 × 80 steps by default.
#[test]
#[ignore = "T2: fuzz"]
fn random_sketch_edits_never_panic_wide() {
    fuzz((1, 6), 80);
}

fn fuzz(default_seeds: (u64, u64), default_steps: usize) {
    let range = std::env::var("SKETCH_FUZZ_SEEDS").ok().and_then(|s| s.split_once("..").and_then(|(a, b)| Some((a.parse::<u64>().ok()?, b.parse::<u64>().ok()?)))).unwrap_or(default_seeds);
    let steps: usize = std::env::var("SKETCH_FUZZ_STEPS").ok().and_then(|s| s.parse().ok()).unwrap_or(default_steps);
    for seed in range.0..range.1 {
        eprintln!("seed {seed}");
        let mut rng = Rng(seed * 2 + 1);
        let mut sk = Sketch::new();
        for step in 0..steps {
            let p = |rng: &mut Rng| DVec2::new(rng.range(-10.0, 10.0), rng.range(-10.0, 10.0));
            let ents: Vec<EntityId> = sk.entities().map(|e| e.id).collect();
            let pick = |rng: &mut Rng, v: &[EntityId]| -> Option<EntityId> { if v.is_empty() { None } else { Some(v[((rng.next() * v.len() as f64) as usize).min(v.len() - 1)]) } };
            let curves: Vec<EntityId> = sk.entities().filter(|e| e.kind() != EntityKind::Point).map(|e| e.id).collect();
            let points: Vec<EntityId> = sk.entities().filter(|e| e.kind() == EntityKind::Point).map(|e| e.id).collect();
            match (rng.next() * 16.0) as usize {
                0 => {
                    let (a, b) = (p(&mut rng), p(&mut rng));
                    sk.corner_rectangle(a, b);
                }
                1 => {
                    let (a, b) = (p(&mut rng), p(&mut rng));
                    sk.center_rectangle(a, b);
                }
                2 => {
                    let _ = sk.three_point_circle_points(p(&mut rng), p(&mut rng), p(&mut rng));
                }
                3 => {
                    let n = 3 + (rng.next() * 6.0) as usize;
                    let _ = sk.polygon(p(&mut rng), p(&mut rng), n, rng.next() < 0.5);
                }
                4 => {
                    let l = sk.add_line(p(&mut rng), p(&mut rng));
                    let _ = sk.slot_around(l, rng.range(0.1, 3.0));
                }
                5 => {
                    let c = p(&mut rng);
                    let r = rng.range(0.2, 5.0);
                    let (a0, a1) = (rng.range(0.0, std::f64::consts::TAU), rng.range(0.0, std::f64::consts::TAU));
                    sk.add_arc(c, c + DVec2::from_angle(a0) * r, c + DVec2::from_angle(a1) * r);
                }
                6 => {
                    sk.add_line(p(&mut rng), p(&mut rng));
                }
                7 => {
                    if let (Some(a), Some(b)) = (pick(&mut rng, &ents), pick(&mut rng, &ents)) {
                        let kinds = [
                            ConstraintKind::Coincident { a, b },
                            ConstraintKind::Parallel { a, b },
                            ConstraintKind::Perpendicular { a, b },
                            ConstraintKind::Tangent { a, b },
                            ConstraintKind::Equal { a, b },
                            ConstraintKind::Horizontal { a, b: None },
                            ConstraintKind::Vertical { a, b: Some(b) },
                            ConstraintKind::Midpoint { point: a, curve: b },
                            ConstraintKind::Concentric { a, b },
                            ConstraintKind::Collinear { a, b },
                        ];
                        let k = kinds[((rng.next() * kinds.len() as f64) as usize).min(kinds.len() - 1)].clone();
                        let _ = sk.add_constraint(k);
                    }
                }
                8 => {
                    if let (Some(a), Some(b), Some(c)) = (pick(&mut rng, &points), pick(&mut rng, &points), pick(&mut rng, &points)) {
                        let _ = sk.add_constraint(ConstraintKind::MidpointOf { point: a, a: b, b: c });
                    }
                }
                9 => {
                    if let Some(c) = pick(&mut rng, &curves) {
                        let v = rng.range(0.1, 8.0);
                        let k = match sk.kind(c) {
                            Some(EntityKind::Line) => ConstraintKind::Length { curve: c, value: v },
                            Some(EntityKind::Circle) => ConstraintKind::Diameter { curve: c, value: v },
                            _ => ConstraintKind::Radius { curve: c, value: v },
                        };
                        let _ = sk.add_constraint(k);
                    }
                }
                10 => {
                    if let Some(c) = pick(&mut rng, &curves) {
                        let _ = sk.trim(c, p(&mut rng));
                    }
                }
                11 => {
                    if let Some(c) = pick(&mut rng, &curves) {
                        let _ = sk.split(c, p(&mut rng));
                    }
                }
                12 => {
                    if let (Some(a), Some(b)) = (pick(&mut rng, &curves), pick(&mut rng, &curves)) {
                        let _ = sk.fillet(a, b, rng.range(0.1, 2.0));
                    }
                }
                13 => {
                    if let Some(c) = pick(&mut rng, &curves) {
                        let _ = sk.offset(&[c], rng.range(-2.0, 2.0));
                    }
                }
                14 => {
                    if let Some(e) = pick(&mut rng, &ents) {
                        sk.remove_entity(e);
                    }
                }
                _ => {
                    if let Some(pt) = pick(&mut rng, &points) {
                        let rep = sk.drag(&[(DragHandle::Point(pt), p(&mut rng))]);
                        assert!(!rep.is_solved() || rep.max_residual < 1e-6, "seed {seed} step {step}: drag {rep:?}");
                    }
                }
            }
            let rep = sk.solve();
            assert!(!rep.is_solved() || rep.max_residual < 1e-6, "seed {seed} step {step}: {rep:?}");
            for e in sk.entities() {
                if let Some(q) = sk.point(e.id) {
                    assert!(q.is_finite(), "seed {seed} step {step}: point {:?} at {q:?}", e.id);
                }
            }
            let _ = sk.regions();
        }
    }
}
