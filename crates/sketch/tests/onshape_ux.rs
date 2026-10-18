//! Solver-side behaviour behind Onshape sketch interactions: chamfer,
//! centerline dimensions, suggested (measured) dimension values and the
//! "both sides move" dimension edit.

use noshape_sketch::*;

fn v(x: f64, y: f64) -> DVec2 {
    DVec2::new(x, y)
}

#[test]
fn chamfer_corner_cuts_both_lines_and_keeps_virtual_sharp() {
    let mut sk = Sketch::new();
    let a = sk.add_point(v(0.0, 0.0));
    let b = sk.add_point(v(10.0, 0.0));
    let c = sk.add_point(v(10.0, 10.0));
    let l1 = sk.add_line_between(a, b).unwrap();
    let l2 = sk.add_line_between(b, c).unwrap();
    let r = sk.chamfer_corner(b, 2.0, 3.0).unwrap();
    assert!(sk.solve().is_solved());
    let new_line = r.created.iter().copied().find(|e| sk.kind(*e) == Some(EntityKind::Line)).unwrap();
    let (p, q) = sk.endpoints(new_line).unwrap();
    assert!((sk.point(p).unwrap() - v(8.0, 0.0)).length() < 1e-9);
    assert!((sk.point(q).unwrap() - v(10.0, 3.0)).length() < 1e-9);
    // the original lines now end at the chamfer
    assert!((sk.curve(l1).unwrap().end_point() - v(8.0, 0.0)).length() < 1e-9);
    assert!((sk.curve(l2).unwrap().start_point() - v(10.0, 3.0)).length() < 1e-9);
    // the corner survives as a construction point with two distance dimensions
    assert!(sk.entity(b).unwrap().construction);
    let dims: Vec<f64> = sk.constraints().filter_map(|c| match c.kind {
        ConstraintKind::Distance { value, .. } => Some(value),
        _ => None,
    }).collect();
    assert_eq!(dims.len(), 2);
    assert!(sk.chamfer_corner(c, 1.0, 1.0).is_err());
}

#[test]
fn centerline_dimension_is_twice_the_distance() {
    let mut sk = Sketch::new();
    let axis = sk.add_line(v(-5.0, 0.0), v(5.0, 0.0));
    sk.set_construction(axis, true).unwrap();
    sk.add_constraint(ConstraintKind::Fix { entity: axis }).unwrap();
    let p = sk.add_point(v(1.0, 2.0));
    let kind = ConstraintKind::Distance { a: p, b: axis, value: 0.0, mode: DistanceMode::Centerline };
    assert!((sk.measure_new(&kind).unwrap() - 4.0).abs() < 1e-12);
    sk.add_constraint(ConstraintKind::Distance { a: p, b: axis, value: 10.0, mode: DistanceMode::Centerline }).unwrap();
    assert!(sk.solve().is_solved());
    assert!((sk.point(p).unwrap().y - 5.0).abs() < 1e-9);
}

#[test]
fn length_edit_moves_both_ends_equally() {
    // Onshape: a new dimension value moves both dimensioned sides by half.
    let mut sk = Sketch::new();
    let l = sk.add_line(v(0.0, 0.0), v(10.0, 0.0));
    sk.add_constraint(ConstraintKind::Horizontal { a: l, b: None }).unwrap();
    let d = sk.add_constraint(ConstraintKind::Length { curve: l, value: 10.0 }).unwrap();
    assert!(sk.solve().is_solved());
    sk.set_dimension_value(d, 20.0).unwrap();
    assert!(sk.solve().is_solved());
    let c = sk.curve(l).unwrap();
    assert!((c.start_point().x + 5.0).abs() < 1e-6, "{c:?}");
    assert!((c.end_point().x - 15.0).abs() < 1e-6, "{c:?}");
}

#[test]
fn slot_around_line_and_arc() {
    let mut sk = Sketch::new();
    let l = sk.add_line(v(0.0, 0.0), v(10.0, 0.0));
    let s = sk.slot_around(l, 2.0).unwrap();
    assert!(sk.solve().is_solved());
    assert!(!sk.entity(l).unwrap().construction, "centerline stays solid (live sketch/308)");
    assert_eq!(s.curves.len(), 4);
    let regions = sk.regions();
    assert_eq!(regions.len(), 1);
    let area = regions[0].area();
    assert!((area - (20.0 + std::f64::consts::PI)).abs() < 1e-3, "{area}");

    let mut sk = Sketch::new();
    let a = sk.add_arc(v(0.0, 0.0), v(10.0, 0.0), v(0.0, 10.0));
    sk.slot_around(a, 2.0).unwrap();
    assert!(sk.solve().is_solved());
    let regions = sk.regions();
    assert_eq!(regions.len(), 1);
    // quarter annulus (r 9..11) plus one full circle of radius 1 from the two caps
    let expect = std::f64::consts::PI / 4.0 * (121.0 - 81.0) + std::f64::consts::PI;
    assert!((regions[0].area() - expect).abs() < 1e-2, "{} vs {expect}", regions[0].area());
}

#[test]
fn spline_control_points_can_be_inserted() {
    let mut sk = Sketch::new();
    let s = sk.add_fit_spline(&[v(0.0, 0.0), v(5.0, 3.0), v(10.0, 0.0)]).unwrap();
    let c = sk.curve(s).unwrap();
    let (t0, t1) = c.domain();
    let mid = c.point(t0 + (t1 - t0) * 0.25);
    let p = sk.insert_spline_point(s, mid).unwrap();
    match &sk.entity(s).unwrap().geometry {
        Geometry::FitSpline { points, .. } => {
            assert_eq!(points.len(), 4);
            assert_eq!(points[1], p);
        }
        g => panic!("{g:?}"),
    }
    let b = sk.add_bspline(&[v(0.0, 0.0), v(5.0, 5.0), v(10.0, 0.0)], 2).unwrap();
    sk.insert_spline_point(b, v(7.5, 2.6)).unwrap();
    match &sk.entity(b).unwrap().geometry {
        Geometry::BSpline { control, degree, .. } => {
            assert_eq!(control.len(), 4);
            assert_eq!(*degree, 3);
        }
        g => panic!("{g:?}"),
    }
}

#[test]
fn copy_and_paste_sketch_entities() {
    let mut sk = Sketch::new();
    let r = sk.corner_rectangle(v(0.0, 0.0), v(4.0, 2.0));
    sk.add_constraint(ConstraintKind::Length { curve: r.curves[0], value: 4.0 }).unwrap();
    let other = sk.add_circle(v(10.0, 0.0), 1.0);
    let clip = sk.extract(&r.curves);
    assert!(clip.entity(other).is_none());
    assert_eq!(clip.entities().filter(|e| e.kind() == EntityKind::Line).count(), 4);
    // the 4 orientation constraints and the length dimension come along
    assert_eq!(clip.constraint_count(), 5);
    let before = sk.entity_count();
    let pasted = sk.paste(&clip, v(0.0, 5.0));
    assert_eq!(pasted.len(), 4);
    assert_eq!(sk.entity_count(), before + 8);
    assert!(sk.solve().is_solved());
    let c = sk.curve(pasted[0]).unwrap();
    assert!((c.start_point().y - 5.0).abs() < 1e-9 || (c.end_point().y - 5.0).abs() < 1e-9);
    // round trip through JSON (the clipboard format)
    let json = serde_json::to_string(&clip).unwrap();
    let back: Sketch = serde_json::from_str(&json).unwrap();
    assert_eq!(back.entity_count(), clip.entity_count());
}

#[test]
fn trimming_a_rectangle_side_keeps_the_other_sides() {
    let mut sk = Sketch::new();
    let r = sk.corner_rectangle(v(0.0, 0.0), v(4.0, 2.0));
    sk.trim(r.curves[2], v(1.0, 2.0)).unwrap();
    for (k, c) in r.curves.iter().enumerate() {
        assert_eq!(sk.entity(*c).is_some(), k != 2, "side {k}");
    }
    sk.trim(r.curves[0], v(1.0, 0.0)).unwrap();
    for (k, c) in r.curves.iter().enumerate() {
        assert_eq!(sk.entity(*c).is_some(), k % 2 == 1, "side {k} after the second trim");
    }
}

#[test]
fn trimming_rectangle_sides_off_the_line() {
    let mut sk = Sketch::new();
    let r = sk.corner_rectangle(v(-0.092, 0.0348), v(-0.0354, -0.00765));
    sk.solve();
    sk.trim(r.curves[2], v(-0.0779, 0.03593)).unwrap();
    sk.solve();
    sk.trim(r.curves[0], v(-0.0779, -0.006447)).unwrap();
    sk.solve();
    let alive: Vec<bool> = r.curves.iter().map(|c| sk.entity(*c).is_some()).collect();
    assert_eq!(alive, vec![false, true, false, true]);
}

#[test]
fn spline_tangent_handles_set_the_end_directions() {
    let mut sk = Sketch::new();
    let s = sk.add_fit_spline_with_handles(&[v(0.0, 0.0), v(5.0, 2.0), v(10.0, 0.0)]).unwrap();
    let (h0, h1) = sk.entity(s).unwrap().spline_handles().expect("handles");
    let c = sk.curve(s).unwrap();
    // the curve still interpolates its points
    assert!((c.start_point() - v(0.0, 0.0)).length() < 1e-9 && (c.end_point() - v(10.0, 0.0)).length() < 1e-9);
    // moving a handle turns the end tangent toward it
    sk.set_point(h0, v(0.0, 3.0)).unwrap();
    let c = sk.curve(s).unwrap();
    let (a, _) = c.domain();
    let t = c.tangent(a).normalize();
    assert!(t.x.abs() < 1e-6 && t.y > 0.99, "{t:?}");
    // a tangent constraint at the start uses the handle direction
    sk.set_point(h0, v(1.0, 1.5)).unwrap();
    let l = sk.add_line(v(-5.0, 0.0), v(0.0, 0.0));
    let (_, le) = sk.endpoints(l).unwrap();
    let (s0, _) = sk.endpoints(s).unwrap();
    sk.merge_points(le, s0).unwrap();
    sk.add_constraint(ConstraintKind::Tangent { a: l, b: s }).unwrap();
    let rep = sk.solve();
    assert!(rep.is_solved(), "{:?}", rep.status);
    let hp = sk.point(h0).unwrap();
    let sp = sk.point(s0).unwrap();
    let lc = sk.curve(l).unwrap();
    let ld = (lc.end_point() - lc.start_point()).normalize();
    assert!(ld.perp_dot((hp - sp).normalize()).abs() < 1e-6, "handle aligned with the line: {hp:?} {sp:?}");
    let _ = h1;
    // a point constrained on the spline follows the handled curve
    let p = sk.add_point(v(5.0, 3.0));
    sk.add_constraint(ConstraintKind::Coincident { a: p, b: s }).unwrap();
    assert!(sk.solve().is_solved());
    let c = sk.curve(s).unwrap();
    assert!(c.closest(sk.point(p).unwrap()).2 < 1e-6);
}


#[test]
fn bezier_degree_elevation_keeps_the_shape() {
    let mut sk = Sketch::new();
    let b = sk.add_bspline(&[v(0.0, 0.0), v(2.0, 4.0), v(6.0, 4.0), v(8.0, 0.0)], 3).unwrap();
    assert_eq!(sk.bezier_degree(b), Some(3));
    let before: Vec<DVec2> = (0..=10).map(|k| sk.curve(b).unwrap().point(k as f64 / 10.0)).collect();
    sk.set_bezier_degree(b, 5).unwrap();
    assert_eq!(sk.bezier_degree(b), Some(5));
    for (k, p) in before.iter().enumerate() {
        assert!((sk.curve(b).unwrap().point(k as f64 / 10.0) - *p).length() < 1e-9);
    }
    // back down: approximation, ends kept
    sk.set_bezier_degree(b, 3).unwrap();
    let c = sk.curve(b).unwrap();
    assert!((c.start_point() - v(0.0, 0.0)).length() < 1e-12 && (c.end_point() - v(8.0, 0.0)).length() < 1e-12);
    assert!((c.point(0.5) - before[5]).length() < 1e-6, "exact for an elevated cubic");
    assert!(sk.set_bezier_degree(b, 16).is_err());
}

#[test]
fn rho_dimension_sets_the_conic() {
    let mut sk = Sketch::new();
    let c = sk.add_conic(v(0.0, 0.0), v(5.0, 5.0), v(10.0, 0.0), 0.5).unwrap();
    let kind = ConstraintKind::Rho { conic: c, value: 0.0 };
    assert!((sk.measure_new(&kind).unwrap() - 0.5).abs() < 1e-12);
    let d = sk.add_constraint(ConstraintKind::Rho { conic: c, value: 0.3 }).unwrap();
    assert!(sk.solve().is_solved());
    assert!(matches!(sk.entity(c).unwrap().geometry, Geometry::Conic { rho, .. } if (rho - 0.3).abs() < 1e-12));
    sk.set_dimension_value(d, 0.8).unwrap();
    assert!(matches!(sk.entity(c).unwrap().geometry, Geometry::Conic { rho, .. } if (rho - 0.8).abs() < 1e-12));
    assert!(sk.set_dimension_value(d, 1.5).is_err());
}

#[test]
fn ellipse_quadrant_inference_and_constraint() {
    let mut sk = Sketch::new();
    let e = sk.add_ellipse(v(0.0, 0.0), v(4.0, 0.0), 2.0);
    let ctx = InferenceContext { snap_radius: 0.2, ..Default::default() };
    let r = infer(&sk, v(0.05, 1.95), &ctx);
    assert_eq!(r[0].kind, InferenceKind::Quadrant);
    assert_eq!(r[0].constraints, vec![AutoConstraint::QuadrantOf(e, false)]);
    sk.add_constraint(ConstraintKind::Fix { entity: e }).unwrap();
    let p = sk.add_point(v(0.3, 1.8));
    sk.add_constraint(ConstraintKind::Quadrant { point: p, ellipse: e, major: false }).unwrap();
    assert!(sk.solve().is_solved());
    let q = sk.point(p).unwrap();
    assert!(q.x.abs() < 1e-6 && (q.y.abs() - 2.0).abs() < 1e-6, "{q:?}");
}

#[test]
fn profile_inspector_finds_loose_ends_and_overlaps() {
    let mut sk = Sketch::new();
    let r = sk.corner_rectangle(v(0.0, 0.0), v(4.0, 2.0));
    assert!(sk.profile_issues().is_empty());
    sk.trim(r.curves[2], v(2.0, 2.0)).unwrap();
    let issues = sk.profile_issues();
    assert_eq!(issues.iter().filter(|i| i.message == "Unclosed loop").count(), 2);
    // a line lying on another
    sk.add_line(v(1.0, 0.0), v(3.0, 0.0));
    assert!(sk.profile_issues().iter().any(|i| i.message == "Overlapping"));
    // construction geometry is ignored
    let c = sk.add_line(v(10.0, 10.0), v(12.0, 10.0));
    sk.set_construction(c, true).unwrap();
    assert!(!sk.profile_issues().iter().any(|i| i.entities.contains(&c)));
}

#[test]
fn slot_around_a_chain_follows_it() {
    let mut sk = Sketch::new();
    let a = sk.add_point(v(0.0, 0.0));
    let b = sk.add_point(v(10.0, 0.0));
    let c = sk.add_point(v(10.0, 10.0));
    let l1 = sk.add_line_between(a, b).unwrap();
    let l2 = sk.add_line_between(b, c).unwrap();
    let (curves, caps) = sk.slot_chains(&[l1, l2], 2.0).unwrap();
    assert_eq!(caps.len(), 2);
    assert!(sk.solve().is_solved());
    let regions = sk.regions();
    assert_eq!(regions.len(), 1, "one closed slot profile (the solid centerline dangles inside)");
    assert!(!sk.entity(l1).unwrap().construction && !sk.entity(l2).unwrap().construction, "centerline stays solid (live)");
    assert!(curves.len() >= 6);
}

#[test]
fn linear_pattern_instances_follow_their_seed() {
    let mut sk = Sketch::new();
    let l = sk.add_line(v(0.0, 0.0), v(1.0, 0.0));
    let (s0, s1) = sk.endpoints(l).unwrap();
    let groups = sk.linear_pattern_groups(&[l], v(0.0, 2.0), 3).unwrap();
    let pts = |g: &Vec<EntityId>, sk: &Sketch| -> Vec<EntityId> { g.iter().copied().filter(|e| sk.kind(*e) == Some(EntityKind::Point)).collect() };
    let first = pts(&groups[0], &sk);
    for (k, g) in groups.iter().enumerate() {
        let ip = pts(g, &sk);
        for (j, seed) in [s0, s1].iter().enumerate() {
            if k == 0 && j == 0 {
                continue;
            }
            sk.add_constraint(ConstraintKind::PatternLink { seed: *seed, inst: ip[j], ref_seed: s0, ref_inst: first[0], k: (k + 1) as f64 }).unwrap();
        }
    }
    assert!(sk.solve().is_solved());
    // moving the seed end moves every instance end
    let rep = sk.drag(&[(DragHandle::Point(s1), v(1.5, 0.3))]);
    assert!(rep.is_solved(), "{:?}", rep.status);
    let top = pts(&groups[1], &sk);
    let p = sk.point(top[1]).unwrap();
    let q = sk.point(s1).unwrap();
    let step = sk.point(first[0]).unwrap() - sk.point(s0).unwrap();
    assert!(((p - q) - step * 2.0).length() < 1e-9, "{p:?} {q:?} {step:?}");
    assert!((q - v(1.5, 0.3)).length() < 1e-3, "the seed end followed the drag");
}

#[test]
fn circular_pattern_follows_seed_edits_and_center_drag() {
    let mut sk = Sketch::new();
    let line = sk.add_line(v(2.0, 1.0), v(3.0, 1.0));
    let (a, b) = sk.endpoints(line).unwrap();
    let groups = sk.circular_pattern_linked_groups(&[line], v(1.0, 1.0), 4, std::f64::consts::TAU).unwrap();
    let center = sk.constraints().find_map(|c| match c.kind {
        ConstraintKind::CircularPatternLink { center, .. } => Some(center),
        _ => None,
    }).unwrap();
    sk.add_constraint(ConstraintKind::Fix { entity: a }).unwrap();
    sk.add_constraint(ConstraintKind::Fix { entity: center }).unwrap();
    assert!(sk.solve().is_solved());
    let report = sk.drag(&[(DragHandle::Point(b), v(3.4, 1.6))]);
    assert!(report.is_solved(), "{report:?}");
    assert!((sk.point(b).unwrap() - v(3.4, 1.6)).length() < 1e-3);
    let check = |sk: &Sketch| {
        for (k, group) in groups.iter().enumerate() {
            let copy = group.iter().copied().find(|id| sk.kind(*id) == Some(EntityKind::Line)).unwrap();
            let (ca, cb) = sk.endpoints(copy).unwrap();
            let t = Transform::Rotate { center: sk.point(center).unwrap(), angle: std::f64::consts::FRAC_PI_2 * (k + 1) as f64 };
            assert!((sk.point(ca).unwrap() - t.apply(sk.point(a).unwrap())).length() < 1e-8);
            assert!((sk.point(cb).unwrap() - t.apply(sk.point(b).unwrap())).length() < 1e-8);
        }
    };
    check(&sk);
    let fix = sk.constraints().find(|c| matches!(c.kind, ConstraintKind::Fix { entity } if entity == center)).unwrap().id;
    sk.remove_constraint(fix).unwrap();
    sk.add_constraint(ConstraintKind::Fix { entity: b }).unwrap();
    let report = sk.drag(&[(DragHandle::Point(center), v(0.4, 0.7))]);
    assert!(report.is_solved(), "{report:?}");
    assert!((sk.point(center).unwrap() - v(0.4, 0.7)).length() < 1e-3);
    check(&sk);
}

#[test]
fn circular_pattern_keeps_circle_and_ellipse_sizes_linked() {
    let mut sk = Sketch::new();
    let circle = sk.add_circle(v(2.0, 0.0), 0.5);
    let ellipse = sk.add_ellipse(v(0.0, 3.0), v(1.0, 0.2), 0.4);
    let groups = sk.circular_pattern_linked_groups(&[circle, ellipse], DVec2::ZERO, 3, std::f64::consts::PI).unwrap();
    let radius = sk.add_constraint(ConstraintKind::Radius { curve: circle, value: 0.5 }).unwrap();
    let major = sk.add_constraint(ConstraintKind::EllipseRadius { ellipse, major: true, value: v(1.0, 0.2).length() }).unwrap();
    assert!(sk.solve().is_solved());
    sk.set_dimension_value(radius, 0.8).unwrap();
    sk.set_dimension_value(major, 1.8).unwrap();
    assert!(sk.solve().is_solved());
    for group in &groups {
        for id in group {
            match sk.curve(*id) {
                Some(Curve2::Circle { radius, .. }) => assert!((radius - 0.8).abs() < 1e-8),
                Some(Curve2::Ellipse { major, minor, .. }) => {
                    assert!((major.length() - 1.8).abs() < 1e-8);
                    let Some(Curve2::Ellipse { minor: seed_minor, .. }) = sk.curve(ellipse) else { unreachable!() };
                    assert!((minor - seed_minor).abs() < 1e-8);
                }
                _ => {}
            }
        }
    }
}

#[test]
fn removing_a_circular_pattern_drops_its_links_and_center() {
    let mut sk = Sketch::new();
    let p = sk.add_point(v(2.0, 0.0));
    let groups = sk.circular_pattern_linked_groups(&[p], DVec2::ZERO, 4, std::f64::consts::TAU).unwrap();
    assert!(sk.solve().is_solved());
    sk.remove_entities(&groups.into_iter().flatten().collect::<Vec<_>>());
    assert_eq!(sk.entity_count(), 1);
    assert_eq!(sk.constraint_count(), 0);
    assert_eq!(sk.point(p), Some(v(2.0, 0.0)));
}

#[test]
fn sketch_picking_prefers_a_movable_point_over_a_nearer_fixed_point() {
    let mut sk = Sketch::new();
    let fixed = sk.add_point(v(1.0, 1.0));
    let movable = sk.add_point(v(1.003, 1.0));
    sk.add_line(v(0.0, 1.0), v(2.0, 1.0));
    sk.add_constraint(ConstraintKind::Fix { entity: fixed }).unwrap();
    let report = sk.solve();
    assert_eq!(sk.pick(v(1.0, 1.0), 0.005), Some(fixed));
    assert_eq!(sk.pick_with_report(v(1.0, 1.0), 0.005, &report), Some(movable));
    assert_eq!(sk.pick_with_report(v(1.0, 1.0), 0.001, &report), Some(fixed));
    assert_eq!(sk.pick_with_report(v(4.0, 4.0), 0.005, &report), None);
}

#[test]
fn select_related_grows_tangent_chains_equal_sizes_and_parallels() {
    let mut sk = Sketch::new();
    let l1 = sk.add_line(v(0.0, 0.0), v(1.0, 0.0));
    let (_, e1) = sk.endpoints(l1).unwrap();
    let arc = sk.tangent_arc(e1, v(2.0, 1.0)).unwrap();
    let arc_end = *arc.points.last().unwrap();
    let up = sk.add_point(v(2.0, 2.0));
    let l2 = sk.add_line_between(arc_end, up).unwrap();
    let corner = sk.add_point(v(3.0, 2.0));
    let l3 = sk.add_line_between(up, corner).unwrap();
    let far = sk.add_line(v(5.0, 5.0), v(6.0, 5.0));
    let circle = sk.add_circle(v(8.0, 8.0), 1.0);
    let chain = sk.select_related(&[l1], RelatedSelection::TangentConnected);
    assert_eq!(chain.len(), 3, "{chain:?}");
    assert!(chain.contains(&arc.curves[0]) && chain.contains(&l2));
    assert!(!chain.contains(&l3), "the corner is not smooth");
    let equal = sk.select_related(&[l1, arc.curves[0]], RelatedSelection::EqualSize);
    assert!(equal.contains(&l3) && equal.contains(&far) && equal.contains(&l2), "unit-length lines: {equal:?}");
    assert!(equal.contains(&circle), "radius 1 like the arc");
    let parallel = sk.select_related(&[l1], RelatedSelection::Parallel);
    assert_eq!(parallel, vec![l1, l3, far]);
}
