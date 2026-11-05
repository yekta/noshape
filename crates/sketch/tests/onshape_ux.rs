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
    // Live 1775: a length typed in the editor (10 -> 20) grows a lone line
    // about its midpoint, drawn either way; a regeneration with the new
    // value (live's posted probes p1 / q1–q3) moves the end with the larger
    // x instead.
    for (a, b) in [(v(0.0, 0.0), v(10.0, 0.0)), (v(10.0, 0.0), v(0.0, 0.0))] {
        for regen in [false, true] {
            let mut sk = Sketch::new();
            let l = sk.add_line(a, b);
            sk.add_constraint(ConstraintKind::Horizontal { a: l, b: None }).unwrap();
            let d = sk.add_constraint(ConstraintKind::Length { curve: l, value: 10.0 }).unwrap();
            assert!(sk.solve().is_solved());
            sk.set_dimension_value(d, 20.0).unwrap();
            if !regen {
                sk.scale_for_edit(d);
            }
            let opts = SolveOptions { constructive: regen, ..sk.dimension_edit_options(d) };
            assert!(sk.solve_with(&opts).is_solved());
            let c = sk.curve(l).unwrap();
            let (lo, hi) = (c.start_point().x.min(c.end_point().x), c.start_point().x.max(c.end_point().x));
            let want = if regen { (0.0, 20.0) } else { (-5.0, 15.0) };
            assert!((lo - want.0).abs() < 1e-6 && (hi - want.1).abs() < 1e-6, "regen {regen}: {c:?}");
        }
    }
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

/// Live's slot (REST, qa5 sketch tools): 6 COINCIDENT, 4 TANGENT, 3 OFFSET
/// (+ the tool's DIAMETER on the end cap = 14). The width follows the one
/// dimension: both sides and the start cap move with it.
#[test]
fn slot_has_live_constraint_set_and_follows_its_width() {
    let mut sk = Sketch::new();
    let l = sk.add_line(v(0.0, 0.0), v(10.0, 0.0));
    let s = sk.slot_around(l, 2.0).unwrap();
    let count = |sk: &Sketch, name: &str| sk.constraints().filter(|c| c.kind.name() == name).count();
    assert_eq!((count(&sk, "Coincident"), count(&sk, "Tangent"), count(&sk, "Offset")), (6, 4, 3));
    let end_cap = s.curves[1];
    let dim = sk.add_constraint(ConstraintKind::Diameter { curve: end_cap, value: 2.0 }).unwrap();
    assert_eq!(sk.constraints().count(), 14);
    let rep = sk.solve();
    assert!(rep.is_solved() && rep.conflicting().is_empty(), "{rep:?}");
    assert_eq!(rep.dof, Some(4), "only the centerline is free");
    sk.set_dimension_value(dim, 4.0).unwrap();
    assert!(sk.solve().is_solved());
    for side in [s.curves[0], s.curves[2]] {
        let c = sk.curve(side).unwrap();
        assert!((c.start_point().y.abs() - 2.0).abs() < 1e-9 && (c.end_point().y.abs() - 2.0).abs() < 1e-9, "{c:?}");
    }
    let start_cap = sk.curve(s.curves[3]).unwrap();
    assert!(matches!(start_cap, Curve2::Arc { radius, center, .. } if (radius - 2.0).abs() < 1e-9 && center.length() < 1e-9), "{start_cap:?}");
    assert_eq!(sk.regions().len(), 1);
}

#[test]
fn corner_rectangle_width_edit_keeps_its_height() {
    // The corner rectangle's own constraints (Perpendicular top / left,
    // Parallel pairs, Horizontal top), as ops' e2e configurations drives it.
    // Regenerated with the new width (live probes 3, r1–r10, posted): it
    // keeps its lower-left corner and its height. Typed in the editor (live
    // 1785): the whole rectangle scales ×1.25 about its centre, or about its
    // corner Coincident with the Origin.
    for (constructive, origin) in [(true, false), (false, false), (false, true)] {
        let mut sk = Sketch::new();
        let r = sk.corner_rectangle(v(0.0, 0.0), v(40.0, 20.0));
        if origin {
            let o = sk.add_point(v(0.0, 0.0));
            sk.set_external(o, true).unwrap();
            sk.add_constraint(ConstraintKind::Coincident { a: r.points[0], b: o }).unwrap();
        }
        let bottom = r.curves[0];
        let d = sk.add_constraint(ConstraintKind::Length { curve: bottom, value: 40.0 }).unwrap();
        assert!(sk.solve().is_solved());
        sk.set_dimension_value(d, 50.0).unwrap();
        let opts = if constructive {
            SolveOptions { constructive: true, ..Default::default() }
        } else {
            assert!(sk.scale_for_edit(d));
            sk.dimension_edit_options(d)
        };
        assert!(sk.solve_with(&opts).is_solved());
        let at: Vec<DVec2> = r.points.iter().map(|p| sk.point(*p).unwrap()).collect();
        let want = match (constructive, origin) {
            (true, _) => [v(0.0, 0.0), v(50.0, 0.0), v(50.0, 20.0), v(0.0, 20.0)],
            (false, false) => [v(-5.0, -2.5), v(45.0, -2.5), v(45.0, 22.5), v(-5.0, 22.5)],
            (false, true) => [v(0.0, 0.0), v(50.0, 0.0), v(50.0, 25.0), v(0.0, 25.0)],
        };
        for (a, b) in at.iter().zip(want) {
            assert!((*a - b).length() < 1e-6, "constructive {constructive} origin {origin}: {at:?}");
        }
    }
}

#[test]
fn width_edit_with_other_dimensions_moves_the_far_side() {
    // Live probes 8 e1: a rectangle with width and height dimensioned isn't
    // scaled; the width typed anew in the editor moves its right side only.
    let mut sk = Sketch::new();
    let p = [v(10.0, 10.0), v(50.0, 10.0), v(50.0, 30.0), v(10.0, 30.0)];
    let pts: Vec<EntityId> = p.iter().map(|x| sk.add_point(*x)).collect();
    let l: Vec<EntityId> = (0..4).map(|i| sk.add_line_between(pts[i], pts[(i + 1) % 4]).unwrap()).collect();
    for (i, x) in l.iter().enumerate() {
        let k = if i % 2 == 0 { ConstraintKind::Horizontal { a: *x, b: None } } else { ConstraintKind::Vertical { a: *x, b: None } };
        sk.add_constraint(k).unwrap();
    }
    sk.add_constraint(ConstraintKind::Length { curve: l[1], value: 20.0 }).unwrap();
    let d = sk.add_constraint(ConstraintKind::Length { curve: l[0], value: 40.0 }).unwrap();
    assert!(sk.solve().is_solved());
    sk.set_dimension_value(d, 50.0).unwrap();
    assert!(!sk.scale_for_edit(d), "another dimension: no scaling");
    assert!(sk.solve_with(&sk.dimension_edit_options(d)).is_solved());
    let at: Vec<DVec2> = pts.iter().map(|x| sk.point(*x).unwrap()).collect();
    let want = [v(10.0, 10.0), v(60.0, 10.0), v(60.0, 30.0), v(10.0, 30.0)];
    for (a, b) in at.iter().zip(want) {
        assert!((*a - b).length() < 1e-6, "{at:?}");
    }
}

#[test]
fn slot_with_new_radius_and_centre_distance_keeps_its_top_line() {
    // Live fuzz s303 / s354 (regenerated with both values new): a slot with
    // a Horizontal bottom, its arcs Equal, the right arc's radius and the
    // centres' distance dimensioned. The free top line stays where it was
    // and grows from its left end; the arcs grow down from it.
    use std::f64::consts::FRAC_PI_2;
    let mut sk = Sketch::new();
    let bottom = sk.add_line(v(0.0, -5.0), v(30.0, -5.0));
    let right = sk.add_arc_angles(v(30.0, 0.0), 5.0, -FRAC_PI_2, FRAC_PI_2);
    let top = sk.add_line(v(30.0, 5.0), v(0.0, 5.0));
    let left = sk.add_arc_angles(v(0.0, 0.0), 5.0, FRAC_PI_2, 3.0 * FRAC_PI_2);
    let ends = |sk: &Sketch, e| sk.endpoints(e).unwrap();
    let centre = |sk: &Sketch, e| match sk.entity(e).map(|x| x.geometry.clone()) {
        Some(Geometry::Arc { center, .. }) => center,
        _ => unreachable!(),
    };
    let chain = [bottom, right, top, left];
    for i in 0..4 {
        let (a, b) = (chain[i], chain[(i + 1) % 4]);
        let (_, a_end) = ends(&sk, a);
        let (b_start, _) = ends(&sk, b);
        sk.add_constraint(ConstraintKind::Coincident { a: a_end, b: b_start }).unwrap();
        sk.add_constraint(ConstraintKind::Tangent { a, b }).unwrap();
    }
    sk.add_constraint(ConstraintKind::Equal { a: right, b: left }).unwrap();
    sk.add_constraint(ConstraintKind::Horizontal { a: bottom, b: None }).unwrap();
    let (lc, rc) = (centre(&sk, left), centre(&sk, right));
    sk.add_constraint(ConstraintKind::Distance { a: lc, b: rc, value: 36.0, mode: DistanceMode::Center }).unwrap();
    sk.add_constraint(ConstraintKind::Radius { curve: right, value: 6.0 }).unwrap();
    assert!(sk.solve_with(&SolveOptions { constructive: true, ..Default::default() }).is_solved());
    let (s, e) = ends(&sk, top);
    let at = [sk.point(s).unwrap(), sk.point(e).unwrap(), sk.point(lc).unwrap(), sk.point(rc).unwrap()];
    let want = [v(36.0, 5.0), v(0.0, 5.0), v(0.0, -1.0), v(36.0, -1.0)];
    for (a, b) in at.iter().zip(want) {
        assert!((*a - b).length() < 1e-6, "{at:?}");
    }
}

#[test]
fn a_tilted_rectangle_comes_onto_the_kept_one() {
    // Live probes 10 r6-k29 (fuzz s6 without its length): rectangle A
    // (its left side Vertical, its top Parallel to a separate line) stays;
    // rectangle B, drawn tilted, has its
    // sides Vertical / Horizontal, its top through A's top-left corner, its
    // right side Equal to A's left, its bottom Equal to A's top, and its
    // top-right 74.636 from A's top-left. B comes whole onto A.
    let mut sk = Sketch::new();
    let quad = |sk: &mut Sketch, p: [DVec2; 4]| -> Vec<EntityId> {
        let l: Vec<EntityId> = (0..4).map(|i| sk.add_line(p[i], p[(i + 1) % 4])).collect();
        for i in 0..4 {
            let (_, e) = sk.endpoints(l[i]).unwrap();
            let (s, _) = sk.endpoints(l[(i + 1) % 4]).unwrap();
            sk.add_constraint(ConstraintKind::Coincident { a: e, b: s }).unwrap();
        }
        l
    };
    // A: bottom, right, top, left (counter-clockwise).
    let a = quad(&mut sk, [v(59.551, 3.576), v(88.306, 3.576), v(88.306, 23.759), v(59.551, 23.759)]);
    let b = quad(&mut sk, [v(116.444, 1.658), v(150.007, -0.773), v(151.219, 15.965), v(117.656, 18.395)]);
    let level = sk.add_line(v(-0.91, -11.645), v(24.057, -11.645));
    sk.add_constraint(ConstraintKind::Parallel { a: a[2], b: level }).unwrap();
    sk.add_constraint(ConstraintKind::Vertical { a: a[3], b: None }).unwrap();
    sk.add_constraint(ConstraintKind::Vertical { a: b[1], b: None }).unwrap();
    sk.add_constraint(ConstraintKind::Horizontal { a: b[2], b: None }).unwrap();
    sk.add_constraint(ConstraintKind::Vertical { a: b[3], b: None }).unwrap();
    sk.add_constraint(ConstraintKind::Equal { a: a[3], b: b[1] }).unwrap();
    sk.add_constraint(ConstraintKind::Parallel { a: b[0], b: b[2] }).unwrap();
    sk.add_constraint(ConstraintKind::Equal { a: b[0], b: a[2] }).unwrap();
    let (a_top_left, _) = sk.endpoints(a[3]).unwrap();
    sk.add_constraint(ConstraintKind::Coincident { a: a_top_left, b: b[2] }).unwrap();
    sk.add_constraint(ConstraintKind::Parallel { a: a[3], b: b[1] }).unwrap();
    let (_, a_corner) = sk.endpoints(a[2]).unwrap();
    let (_, b_top_right) = sk.endpoints(b[1]).unwrap();
    sk.add_constraint(ConstraintKind::Distance { a: a_corner, b: b_top_right, value: 74.636, mode: DistanceMode::Center }).unwrap();
    let before: Vec<DVec2> = a.iter().map(|l| sk.point(sk.endpoints(*l).unwrap().0).unwrap()).collect();
    assert!(sk.solve_with(&SolveOptions { constructive: true, ..Default::default() }).is_solved());
    let starts = |sk: &Sketch, q: &[EntityId]| -> Vec<DVec2> { q.iter().map(|l| sk.point(sk.endpoints(*l).unwrap().0).unwrap()).collect() };
    let want_b = [v(105.432, 3.576), v(134.187, 3.576), v(134.187, 23.759), v(105.432, 23.759)];
    for (p, q) in starts(&sk, &a).iter().zip(&before).chain(starts(&sk, &b).iter().zip(&want_b)) {
        assert!((*p - *q).length() < 2e-3, "A {:?} B {:?}", starts(&sk, &a), starts(&sk, &b));
    }
}

#[test]
fn a_dragged_bolt_circle_hole_swings_the_pattern_about_the_next_hole() {
    // Live drag probes 9 g6: a Ø40 circle, three Ø6 holes 14 from its centre
    // (Equal, the first hole's and the big circle's radii dimensioned). The
    // top hole dragged to (3.0755, 16.1871): the next hole stays, the centre
    // swings about it and the last hole comes radially onto its circle.
    use std::f64::consts::{FRAC_PI_2, TAU};
    let mut sk = Sketch::new();
    let centre_of = |sk: &Sketch, c: EntityId| match sk.entity(c).unwrap().geometry {
        Geometry::Circle { center, .. } => center,
        _ => unreachable!(),
    };
    let big = sk.add_circle(v(0.0, 0.0), 20.0);
    let holes: Vec<EntityId> = (0..3).map(|i| sk.add_circle(DVec2::from_angle(FRAC_PI_2 + i as f64 * TAU / 3.0) * 14.0, 3.0)).collect();
    for h in &holes[1..] {
        sk.add_constraint(ConstraintKind::Equal { a: holes[0], b: *h }).unwrap();
    }
    sk.add_constraint(ConstraintKind::Radius { curve: big, value: 20.0 }).unwrap();
    sk.add_constraint(ConstraintKind::Radius { curve: holes[0], value: 3.0 }).unwrap();
    let c = centre_of(&sk, big);
    let hc: Vec<EntityId> = holes.iter().map(|h| centre_of(&sk, *h)).collect();
    for h in &hc {
        sk.add_constraint(ConstraintKind::Distance { a: c, b: *h, value: 14.0, mode: DistanceMode::Center }).unwrap();
    }
    assert!(sk.solve().is_solved());
    let rep = sk.drag(&[(DragHandle::Point(hc[0]), v(3.0755, 16.1871))]);
    assert!(rep.is_solved(), "{rep:?}");
    let got: Vec<DVec2> = [c, hc[0], hc[1], hc[2]].iter().map(|p| sk.point(*p).unwrap()).collect();
    let live = [v(-2.8875, 3.5205), v(3.0755, 16.1871), v(-12.1244, -7.0), v(8.5774, -4.5142)];
    for (p, q) in got.iter().zip(&live) {
        assert!((*p - *q).length() < 1e-3, "ours {got:?} live {live:?}");
    }
}

#[test]
fn a_dragged_slot_keeps_its_horizontal_bottom_level() {
    // Live drag probes 9 g4: a slot without dimensions (its bottom
    // Horizontal, its arcs Equal), the top line's start dragged up and out
    // in 25 steps: the bottom stays at its height (its end slides out with
    // the right arc), the arcs grow, the top follows the pointer.
    use std::f64::consts::FRAC_PI_2;
    let mut sk = Sketch::new();
    let bottom = sk.add_line(v(0.0, -6.0), v(30.0, -6.0));
    let right = sk.add_arc_angles(v(30.0, 0.0), 6.0, -FRAC_PI_2, FRAC_PI_2);
    let top = sk.add_line(v(30.0, 6.0), v(0.0, 6.0));
    let left = sk.add_arc_angles(v(0.0, 0.0), 6.0, FRAC_PI_2, 3.0 * FRAC_PI_2);
    let chain = [bottom, right, top, left];
    for i in 0..4 {
        let (a, b) = (chain[i], chain[(i + 1) % 4]);
        let (_, a_end) = sk.endpoints(a).unwrap();
        let (b_start, _) = sk.endpoints(b).unwrap();
        sk.add_constraint(ConstraintKind::Coincident { a: a_end, b: b_start }).unwrap();
        sk.add_constraint(ConstraintKind::Tangent { a, b }).unwrap();
    }
    sk.add_constraint(ConstraintKind::Equal { a: right, b: left }).unwrap();
    sk.add_constraint(ConstraintKind::Horizontal { a: bottom, b: None }).unwrap();
    assert!(sk.solve().is_solved());
    let (grab, _) = sk.endpoints(top).unwrap();
    for k in 1..=25 {
        let f = k as f64 / 25.0;
        assert!(sk.drag(&[(DragHandle::Point(grab), v(30.0 + 4.0 * f, 6.0 + 3.0 * f))]).is_solved());
    }
    let (s, e) = sk.endpoints(bottom).unwrap();
    for p in [s, e] {
        assert!((sk.point(p).unwrap().y + 6.0).abs() < 1e-6, "bottom {:?} {:?}", sk.point(s), sk.point(e));
    }
    assert!((sk.point(grab).unwrap() - v(34.0, 9.0)).length() < 1e-6);
}

#[test]
fn a_quad_side_equal_to_a_triangle_side_follows_its_corner_whole() {
    // Live fuzz s11, posted unsolved: an equilateral triangle (l1 20.621,
    // l2 = l1 = l3) and a quad whose l5 = l4 and l7 = l2. Live keeps l5, l4
    // grows along itself from their corner, l7 follows l4's moved end whole
    // and grows at its free start, l6 takes up the rest.
    let mut sk = Sketch::new();
    let pts = [
        (v(15.5648, 4.2244), v(-6.1147, 17.6617)),
        (v(-6.1147, 17.6617), v(-6.9141, -7.6995)),
        (v(-6.9141, -7.6995), v(15.5648, 4.2244)),
        (v(78.2501, 5.4463), v(66.0524, 13.3535)),
        (v(66.0524, 13.3535), v(52.3390, 3.5232)),
        (v(52.3390, 3.5232), v(63.7027, -9.1384)),
        (v(63.7027, -9.1384), v(78.2501, 5.4463)),
    ];
    let l: Vec<EntityId> = pts.iter().map(|(a, b)| sk.add_line(*a, *b)).collect();
    let ends = |sk: &Sketch, i: usize| sk.endpoints(l[i]).unwrap();
    for (a, b) in [(0, 1), (1, 2), (2, 0), (3, 4), (4, 5), (5, 6), (6, 3)] {
        let (e, s) = (ends(&sk, a).1, ends(&sk, b).0);
        sk.add_constraint(ConstraintKind::Coincident { a: e, b: s }).unwrap();
    }
    for (a, b) in [(0, 1), (2, 0), (1, 6), (4, 3)] {
        sk.add_constraint(ConstraintKind::Equal { a: l[a], b: l[b] }).unwrap();
    }
    let l3_start = ends(&sk, 2).0;
    sk.add_constraint(ConstraintKind::Coincident { a: l3_start, b: l[1] }).unwrap();
    sk.add_constraint(ConstraintKind::Length { curve: l[0], value: 20.621 }).unwrap();
    assert!(sk.solve_with(&SolveOptions { constructive: true, ..Default::default() }).is_solved());
    let got = [ends(&sk, 3).0, ends(&sk, 3).1, ends(&sk, 4).1, ends(&sk, 6).0].map(|p| sk.point(p).unwrap());
    let live = [v(80.2106, 4.1755), v(66.0524, 13.3535), v(52.3390, 3.5232), v(65.6481, -10.4245)];
    for (p, q) in got.iter().zip(&live) {
        assert!((*p - *q).length() < 2e-3, "ours {got:?} live {live:?}");
    }
}

#[test]
fn a_rectangle_with_a_circle_on_its_diagonal_grows_up_from_its_bottom() {
    // Live fuzz s485: a rectangle (H / V sides, width and height
    // dimensioned), a construction diagonal and a circle at its midpoint.
    // The height 20.266 -> 20.444: the bottom stays, the top goes up by the
    // whole change and the circle by half (the diagonal's end reads moved
    // through the top's corner; it isn't kept as it was).
    let mut sk = Sketch::new();
    let (x0, x1, y0, y1) = (-2.5976, 28.005, 5.8271, 26.0934);
    let c = [v(x0, y0), v(x1, y0), v(x1, y1), v(x0, y1)];
    let l: Vec<EntityId> = (0..4).map(|i| sk.add_line(c[i], c[(i + 1) % 4])).collect();
    let ends = |sk: &Sketch, i: EntityId| sk.endpoints(i).unwrap();
    for i in 0..4 {
        let (e, s) = (ends(&sk, l[i]).1, ends(&sk, l[(i + 1) % 4]).0);
        sk.add_constraint(ConstraintKind::Coincident { a: e, b: s }).unwrap();
        let kind = if i % 2 == 0 { ConstraintKind::Horizontal { a: l[i], b: None } } else { ConstraintKind::Vertical { a: l[i], b: None } };
        sk.add_constraint(kind).unwrap();
    }
    let diagonal = sk.add_line(c[0], c[2]);
    sk.entity_mut(diagonal).unwrap().construction = true;
    let (ds, de) = ends(&sk, diagonal);
    sk.add_constraint(ConstraintKind::Coincident { a: ds, b: ends(&sk, l[0]).0 }).unwrap();
    sk.add_constraint(ConstraintKind::Coincident { a: de, b: ends(&sk, l[2]).0 }).unwrap();
    let circle = sk.add_circle((c[0] + c[2]) * 0.5, 4.9698);
    let centre = match sk.entity(circle).unwrap().geometry {
        Geometry::Circle { center, .. } => center,
        _ => unreachable!(),
    };
    sk.add_constraint(ConstraintKind::Midpoint { point: centre, curve: diagonal }).unwrap();
    sk.add_constraint(ConstraintKind::Length { curve: l[0], value: 30.602602 }).unwrap();
    sk.add_constraint(ConstraintKind::Length { curve: l[1], value: 20.443754 }).unwrap();
    sk.add_constraint(ConstraintKind::Diameter { curve: circle, value: 9.300012 }).unwrap();
    assert!(sk.solve_with(&SolveOptions { constructive: true, ..Default::default() }).is_solved());
    let got = [ends(&sk, l[0]).0, ends(&sk, l[1]).1, centre].map(|p| sk.point(p).unwrap());
    let live = [v(-2.597643, 5.827106), v(28.004959, 26.27086), v(12.703658, 16.048983)];
    for (p, q) in got.iter().zip(&live) {
        assert!((*p - *q).length() < 2e-3, "ours {got:?} live {live:?}");
    }
}

#[test]
fn a_longer_chamfer_slides_its_ends_along_the_sides() {
    // Live fuzz s465: a rectangle with a 45° chamfer at its bottom-right
    // corner, the chamfer's length 6.757 -> 7.51: only the chamfer's ends
    // move, along the bottom and the right side; the outer box stays.
    let mut sk = Sketch::new();
    let (x0, x1, y0, y1) = (9.484914772801268, 39.92615074312081, -8.092778338134226, 14.262781702091216);
    let c = [v(x0, y0), v(35.14840455662541, y0), v(x1, -3.315032151638828), v(x1, y1), v(x0, y1)];
    let l: Vec<EntityId> = (0..5).map(|i| sk.add_line(c[i], c[(i + 1) % 5])).collect();
    let ends = |sk: &Sketch, i: EntityId| sk.endpoints(i).unwrap();
    for i in 0..5 {
        let (e, s) = (ends(&sk, l[i]).1, ends(&sk, l[(i + 1) % 5]).0);
        sk.add_constraint(ConstraintKind::Coincident { a: e, b: s }).unwrap();
    }
    sk.add_constraint(ConstraintKind::Horizontal { a: l[0], b: None }).unwrap();
    sk.add_constraint(ConstraintKind::Vertical { a: l[2], b: None }).unwrap();
    sk.add_constraint(ConstraintKind::Horizontal { a: l[3], b: None }).unwrap();
    sk.add_constraint(ConstraintKind::Vertical { a: l[4], b: None }).unwrap();
    sk.add_constraint(ConstraintKind::Angle { a: l[0], b: l[1], value: std::f64::consts::FRAC_PI_4 }).unwrap();
    sk.add_constraint(ConstraintKind::Length { curve: l[4], value: y1 - y0 }).unwrap();
    sk.add_constraint(ConstraintKind::Length { curve: l[3], value: x1 - x0 }).unwrap();
    sk.add_constraint(ConstraintKind::Length { curve: l[1], value: 7.509996366076509 }).unwrap();
    assert!(sk.solve_with(&SolveOptions { constructive: true, ..Default::default() }).is_solved());
    let got = [ends(&sk, l[0]).0, ends(&sk, l[0]).1, ends(&sk, l[1]).1, ends(&sk, l[2]).1].map(|p| sk.point(p).unwrap());
    let live = [v(9.484915, -8.092778), v(34.615782, -8.092778), v(39.926151, -2.782409), v(39.926151, 14.262782)];
    for (p, q) in got.iter().zip(&live) {
        assert!((*p - *q).length() < 2e-3, "ours {got:?} live {live:?}");
    }
}

#[test]
fn a_circle_tangent_to_two_splines_touches_them_along_their_length() {
    // SKETCH-TANGENT-SPLINE-CIRCLE (Onshape's public Spur Gear, its tooth
    // sketch as the FeatureScript host builds it): a root fillet circle
    // Tangent to the fixed root circle and to the two fixed involute fit
    // splines it isn't joined to touches the splines along their length
    // (r 0.5683 mm, on the gap's axis at x 11.818 mm), not at their ends'
    // normals (r 0.2864 mm before).
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/solve/spur-gear-fillet.json");
    let mut sk: Sketch = serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap();
    // (The constraints as `add_constraint` makes them, hints and all.)
    let ids: Vec<ConstraintId> = sk.constraints().map(|c| c.id).collect();
    for id in ids {
        sk.recompute_hints(id).unwrap();
    }
    assert!(sk.solve().is_solved());
    let (centre, r) = sk
        .entities()
        .filter_map(|e| match sk.curve(e.id) {
            Some(Curve2::Circle { center, radius }) if radius < 0.005 => Some((center, radius)),
            _ => None,
        })
        .next()
        .unwrap();
    assert!((r * 1e3 - 0.5683).abs() < 1e-3 && (centre * 1e3 - v(11.8183, 0.0)).length() < 1e-3, "centre {:?} mm, r {} mm", centre * 1e3, r * 1e3);
}

/// The public Spur Gear's tooth sketch (createTooth: two involute fit
/// splines, the root circle, a fillet circle Tangent to all three) for
/// `teeth`, `module` and pitch diameter `pcd` (mm), solved: the fillet
/// circle's centre and radius (mm).
fn spur_gear_root_circle(teeth: f64, module: f64, pcd: f64) -> (DVec2, f64) {
    let mm = 1e-3;
    let pa = 20f64.to_radians();
    let (pcd, ded, add) = (pcd * mm, 1.25 * module * mm, module * mm);
    let base = pcd * pa.cos();
    let alpha = (pcd * pcd - base * base).sqrt() / base - pa;
    let beta = (360.0 / (4.0 * teeth)).to_radians() - alpha;
    let (mut inv1, mut inv2) = (Vec::new(), Vec::new());
    let mut t = 0.0;
    while t <= 2.0 {
        let (ca, sa) = ((t + beta).cos(), (t + beta).sin());
        let (cab, sab) = ((-beta - t).cos(), (-beta - t).sin());
        let (p1, p2) = if base >= pcd - 2.0 * ded && t == 0.0 {
            let r = pcd / 2.0 - ded;
            (v(r * ca, r * sa), v(r * cab, r * sab))
        } else {
            (v(base * 0.5 * (ca + t * sa), base * 0.5 * (sa - t * ca)), v(base * 0.5 * (cab - t * sab), base * 0.5 * (sab + t * cab)))
        };
        inv1.push(p1);
        inv2.push(p2);
        if p1.length() >= pcd / 2.0 + add {
            break;
        }
        t += 1.0 / 50.0;
    }
    let mut sk = Sketch::new();
    let s1 = sk.add_fit_spline(&inv1).unwrap();
    let s2 = sk.add_fit_spline(&inv2).unwrap();
    sk.add_circle(DVec2::ZERO, pcd / 2.0 + add);
    let dedendum = sk.add_circle(DVec2::ZERO, pcd / 2.0 - ded);
    let fillet = sk.add_circle(v(pcd / 2.0 - ded / 2.0, 0.0), 0.1 * mm);
    sk.set_construction(fillet, true).unwrap();
    for e in [dedendum, s1, s2] {
        sk.add_constraint(ConstraintKind::Fix { entity: e }).unwrap();
    }
    for e in [dedendum, s1, s2] {
        sk.add_constraint(ConstraintKind::Tangent { a: fillet, b: e }).unwrap();
    }
    assert!(sk.solve().is_solved());
    match sk.curve(fillet) {
        Some(Curve2::Circle { center, radius }) => (center / mm, radius / mm),
        c => panic!("{c:?}"),
    }
}

/// SKETCH-TANGENT-SPLINE-CIRCLE: the gear's root circles as live's: the
/// defaults, live 1830's REST-made 40 teeth / module 2 that kept the 25 mm
/// pitch (live's root fillet r 0.166393 = 2/3 of 0.2495895; the contact near
/// the spline's start used to step past it and stick: r 1.3237 unsolved),
/// and live 1861's at an 80 mm pitch.
#[test]
fn spur_gear_root_circles_as_live() {
    let (c, r) = spur_gear_root_circle(25.0, 1.0, 25.0);
    assert!((r - 0.5683).abs() < 1e-3, "25 teeth, module 1: centre {c} mm, r {r} mm");
    let (c, r) = spur_gear_root_circle(40.0, 2.0, 25.0);
    assert!((r - 0.2495895).abs() < 1e-6 && c.y.abs() < 1e-9, "40 teeth, module 2, pitch Ø25: centre {c} mm, r {r} mm");
    let (c, r) = spur_gear_root_circle(40.0, 2.0, 80.0);
    assert!((r - 1.06204944).abs() < 1e-5 && (c - v(38.56204944, 0.0)).length() < 1e-5, "40 teeth, module 2: centre {c} mm, r {r} mm");
}

/// A fit spline takes Onshape's stored end derivatives (per unit of its 0–1
/// centripetal parameter, live 1777): the curve's ends have exactly them, with
/// or without handles before (SKETCH-SPLINE-END-DERIVATIVES; dx's
/// cf-curve-pattern-normal Sketch 2: (-0.304, 0) and (-0.242, 0) m).
#[test]
fn a_fit_spline_takes_onshapes_end_derivatives() {
    let pts = [v(0.0, 0.0), v(0.04, 0.03), v(0.09, 0.01)];
    let (d0, d1) = (v(-0.304, 0.0), v(-0.242, 0.0));
    for with_handles in [false, true] {
        let mut sk = Sketch::new();
        let s = if with_handles { sk.add_fit_spline_with_handles(&pts) } else { sk.add_fit_spline(&pts) }.unwrap();
        sk.set_fit_spline_end_derivatives(s, d0, d1).unwrap();
        let Some(Curve2::Nurbs(nb)) = sk.curve(s) else { panic!("not a NURBS") };
        let (a, b) = nb.domain();
        let ends = (nb.eval_ders(a)[1] * (b - a), nb.eval_ders(b)[1] * (b - a));
        assert!((ends.0 - d0).length() < 1e-9 && (ends.1 - d1).length() < 1e-9, "{ends:?}");
        assert!((nb.eval_ders(a)[0] - pts[0]).length() < 1e-12 && (nb.eval_ders(b)[0] - pts[2]).length() < 1e-12);
    }
    let mut sk = Sketch::new();
    let line = sk.add_line(v(0.0, 0.0), v(0.01, 0.0));
    assert!(sk.set_fit_spline_end_derivatives(line, d0, d1).is_err());
}

/// Live 2022: a free line's length set anew moves its start (the first click) and keeps its end, in all four
/// directions, by its dimension once the sketch has another one (its lines B2, C2, D2; and A in the typed box).
#[test]
fn a_free_lines_new_length_moves_its_start() {
    let cases = [
        ((20.13, 20.27), (39.968, 24.36), (25.276, 21.335)),
        ((39.968, 30.195), (20.13, 34.279), (34.822, 31.255)),
        ((-39.968, -19.692), (-20.13, -15.608), (-34.822, -18.633)),
        ((-20.13, -29.612), (-39.968, -25.819), (-25.235, -28.636)),
    ];
    for (a, b, want) in cases {
        let mut sk = Sketch::new();
        let q0 = sk.add_point(v(100.0, 100.0));
        let q1 = sk.add_point(v(120.0, 100.0));
        let other = sk.add_line_between(q0, q1).unwrap();
        sk.add_constraint(ConstraintKind::Length { curve: other, value: 20.0 }).unwrap();
        let (p0, p1) = (sk.add_point(v(a.0, a.1)), sk.add_point(v(b.0, b.1)));
        let l = sk.add_line_between(p0, p1).unwrap();
        let d = sk.add_constraint(ConstraintKind::Length { curve: l, value: (v(b.0, b.1) - v(a.0, a.1)).length() }).unwrap();
        assert!(sk.solve().is_solved());
        sk.set_dimension_value(d, 15.0).unwrap();
        assert!(!sk.scale_for_edit(d), "the sketch has a dimension: no scaling");
        assert!(sk.solve_with(&sk.free_line_edit_options(d).expect("a free line")).is_solved());
        assert!((sk.point(p1).unwrap() - v(b.0, b.1)).length() < 1e-9, "the end stays");
        // (Live's clicks to 0.001-0.006 mm: line A reads 20.27 in its typed-box table, 20.276 in its Dimension run.)
        assert!((sk.point(p0).unwrap() - v(want.0, want.1)).length() < 5e-3, "start {:?}, live {want:?}", sk.point(p0).unwrap());
    }
}

/// Live 2022 (8): two free lines of 18.543 and 20.254 mm, the first dimensioned to 15: the sketch's first dimension
/// scales the whole sketch, the other line too (16.384 mm), about the centre of the box of all its geometry.
#[test]
fn a_sketchs_first_dimension_scales_all_of_it() {
    let mut sk = Sketch::new();
    let pts = [v(20.13, 20.268), v(38.4, 23.4), v(21.0, 34.271), v(39.968, 30.0)];
    let ids: Vec<EntityId> = pts.iter().map(|p| sk.add_point(*p)).collect();
    let first = sk.add_line_between(ids[0], ids[1]).unwrap();
    sk.add_line_between(ids[2], ids[3]).unwrap();
    let len = (pts[1] - pts[0]).length();
    let d = sk.add_constraint(ConstraintKind::Length { curve: first, value: len }).unwrap();
    assert!(sk.solve().is_solved());
    sk.set_dimension_value(d, 15.0).unwrap();
    assert!(sk.scale_for_edit(d));
    assert!(sk.solve_with(&sk.dimension_edit_options(d)).is_solved());
    let (k, c) = (15.0 / len, v((20.13 + 39.968) / 2.0, (20.268 + 34.271) / 2.0));
    for (id, p) in ids.iter().zip(pts) {
        assert!((sk.point(*id).unwrap() - (c + (p - c) * k)).length() < 1e-6, "{:?}", sk.point(*id).unwrap());
    }
}
