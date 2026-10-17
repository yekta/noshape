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
