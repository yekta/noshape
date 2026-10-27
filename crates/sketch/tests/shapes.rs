use noshape_sketch::*;
use std::f64::consts::PI;

fn v(x: f64, y: f64) -> DVec2 {
    DVec2::new(x, y)
}

#[test]
fn corner_rectangle_dims_fix_fully_constrained() {
    let mut sk = Sketch::new();
    let r = sk.corner_rectangle(v(0.0, 0.0), v(2.0, 1.0));
    sk.add_constraint(ConstraintKind::Fix { entity: r.points[0] }).unwrap();
    sk.add_constraint(ConstraintKind::Length { curve: r.curves[0], value: 3.0 }).unwrap();
    let h = sk.add_constraint(ConstraintKind::Length { curve: r.curves[1], value: 1.5 }).unwrap();
    let rep = sk.solve();
    assert!(rep.is_solved());
    assert_eq!(rep.dof, Some(0));
    for c in &r.curves {
        assert_eq!(rep.state(*c), Some(ConstraintState::FullyConstrained));
    }
    sk.remove_constraint(h).unwrap();
    let rep = sk.solve();
    assert_eq!(rep.dof, Some(1));
    // vertical edges keep their x, top edge moves: bottom fully constrained
    assert_eq!(rep.state(r.curves[0]), Some(ConstraintState::FullyConstrained));
    assert_eq!(rep.state(r.curves[2]), Some(ConstraintState::UnderConstrained));
    assert_eq!(rep.state(r.curves[1]), Some(ConstraintState::UnderConstrained));
    assert_eq!(rep.state(r.curves[3]), Some(ConstraintState::UnderConstrained));
    // regions
    assert_eq!(sk.regions().len(), 1);
}

#[test]
fn rectangle_variants_dof() {
    let mut sk = Sketch::new();
    let r = sk.center_rectangle(v(0.0, 0.0), v(2.0, 1.0));
    let rep = sk.solve();
    assert!(rep.is_solved());
    assert_eq!(rep.dof, Some(4));
    assert_eq!(sk.regions().len(), 1);
    assert!((sk.regions()[0].area() - 8.0).abs() < 1e-9);
    let _ = r;

    let mut sk = Sketch::new();
    let r = sk.three_point_rectangle(v(0.0, 0.0), v(2.0, 1.0), v(0.0, 3.0)).unwrap();
    let rep = sk.solve();
    assert!(rep.is_solved());
    assert_eq!(rep.dof, Some(5));
    // drag a corner: stays a rectangle
    let rep = sk.drag(&[(DragHandle::Point(r.points[2]), v(3.0, 4.0))]);
    assert!(rep.is_solved());
    let p: Vec<DVec2> = r.points.iter().map(|id| sk.point(*id).unwrap()).collect();
    assert!((p[1] - p[0]).dot(p[2] - p[1]).abs() < 1e-8);
    assert!(((p[1] - p[0]) - (p[2] - p[3])).length() < 1e-8);

    let mut sk = Sketch::new();
    sk.aligned_rectangle(v(1.0, 1.0), v(2.0, 2.0), v(1.0, 3.0)).unwrap();
    let rep = sk.solve();
    assert!(rep.is_solved());
    assert_eq!(rep.dof, Some(5));
}

#[test]
fn polygons_and_slot() {
    for on_vertex in [true, false] {
        for n in [3, 4, 6] {
            let mut sk = Sketch::new();
            let p = sk.polygon(v(0.0, 0.0), v(1.0, 0.0), n, on_vertex).unwrap();
            let rep = sk.solve();
            assert!(rep.is_solved(), "{rep:?}");
            assert_eq!(rep.dof, Some(4), "n={n} on_vertex={on_vertex} {rep:?}");
            let regions = sk.regions();
            assert_eq!(regions.len(), 1);
            let big_r = if on_vertex { 1.0 } else { 1.0 / (PI / n as f64).cos() };
            let area = 0.5 * n as f64 * big_r * big_r * (2.0 * PI / n as f64).sin();
            assert!((regions[0].area() - area).abs() < 1e-9);
            let _ = p;
        }
    }
    let mut sk = Sketch::new();
    let s = sk.straight_slot(v(0.0, 0.0), v(4.0, 0.0), 2.0).unwrap();
    let rep = sk.solve();
    assert!(rep.is_solved(), "{rep:?}");
    assert_eq!(rep.dof, Some(5));
    let reg = sk.regions();
    assert_eq!(reg.len(), 1);
    assert!((reg[0].area() - (8.0 + PI)).abs() < 1e-9);
    // change width through a radius dimension
    sk.add_constraint(ConstraintKind::Radius { curve: s.curves[1], value: 0.5 }).unwrap();
    let rep = sk.solve();
    assert!(rep.is_solved(), "{rep:?}");
    let reg = sk.regions();
    assert!((reg[0].area() - (4.0 + PI * 0.25)).abs() < 1e-6, "{}", reg[0].area());
}

#[test]
fn arcs_and_circles() {
    let mut sk = Sketch::new();
    let a = sk.three_point_arc(v(1.0, 0.0), v(0.0, 1.0), v(-1.0, 0.0)).unwrap();
    match sk.curve(a).unwrap() {
        Curve2::Arc { center, radius, start_angle, end_angle } => {
            assert!(center.length() < 1e-12);
            assert!((radius - 1.0).abs() < 1e-12);
            assert!((end_angle - start_angle - PI).abs() < 1e-12);
        }
        _ => panic!(),
    }
    // clockwise 3-point arc gets reversed into a CCW arc through the mid point
    let b = sk.three_point_arc(v(-1.0, 0.0), v(0.0, 1.0), v(1.0, 0.0)).unwrap();
    let cb = sk.curve(b).unwrap();
    assert!(cb.distance(v(0.0, 1.0)) < 1e-12);
    let c = sk.three_point_circle(v(1.0, 0.0), v(0.0, 1.0), v(-1.0, 0.0)).unwrap();
    assert!(matches!(sk.curve(c).unwrap(), Curve2::Circle { radius, .. } if (radius - 1.0).abs() < 1e-12));

    // tangent arc continuing a line
    let mut sk = Sketch::new();
    let l = sk.add_line(v(0.0, 0.0), v(2.0, 0.0));
    let (_, e) = sk.endpoints(l).unwrap();
    let t = sk.tangent_arc(e, v(2.0, 2.0)).unwrap();
    let rep = sk.solve();
    assert!(rep.is_solved());
    match sk.curve(t.curves[0]).unwrap() {
        Curve2::Arc { center, radius, .. } => {
            assert!((center - v(2.0, 1.0)).length() < 1e-9);
            assert!((radius - 1.0).abs() < 1e-9);
        }
        _ => panic!(),
    }
    let cp = sk.center_point_arc(v(0.0, 0.0), v(1.0, 0.0), v(0.0, 1.0), false);
    match sk.curve(cp).unwrap() {
        Curve2::Arc { start_angle, end_angle, .. } => assert!((end_angle - start_angle - 1.5 * PI).abs() < 1e-9),
        _ => panic!(),
    }
}

/// Live (behaviours "spec: skasm sketch tools"): the corner rectangle's
/// Horizontal is on the edge at the second click's y, its Perpendicular
/// between that edge and the one at the first click's x.
#[test]
fn corner_rectangle_constraints_follow_the_click_order() {
    let mut sk = Sketch::new();
    let r = sk.corner_rectangle(v(0.0, 1.0), v(2.0, 0.0));
    let at_y = |sk: &Sketch, y: f64| r.curves.iter().copied().find(|c| matches!(sk.curve(*c), Some(Curve2::Line { a, b }) if (a.y - y).abs() < 1e-12 && (b.y - y).abs() < 1e-12)).unwrap();
    let at_x = |sk: &Sketch, x: f64| r.curves.iter().copied().find(|c| matches!(sk.curve(*c), Some(Curve2::Line { a, b }) if (a.x - x).abs() < 1e-12 && (b.x - x).abs() < 1e-12)).unwrap();
    let (top, left) = (at_y(&sk, 0.0), at_x(&sk, 0.0));
    assert!(sk.constraints().any(|c| c.kind == ConstraintKind::Horizontal { a: top, b: None }));
    assert!(sk.constraints().any(|c| c.kind == ConstraintKind::Perpendicular { a: top, b: left }));
    assert_eq!(sk.solve().dof, Some(4));
}
