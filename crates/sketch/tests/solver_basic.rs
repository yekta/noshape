use noshape_sketch::*;
use std::f64::consts::PI;

fn v(x: f64, y: f64) -> DVec2 {
    DVec2::new(x, y)
}

/// Rectangle from 4 shared corner points + H/V constraints.
fn rect(sk: &mut Sketch, w: f64, h: f64) -> ([EntityId; 4], [EntityId; 4]) {
    let p = [sk.add_point(v(0.0, 0.0)), sk.add_point(v(w, 0.1)), sk.add_point(v(w + 0.2, h)), sk.add_point(v(-0.1, h - 0.1))];
    let l = [
        sk.add_line_between(p[0], p[1]).unwrap(),
        sk.add_line_between(p[1], p[2]).unwrap(),
        sk.add_line_between(p[2], p[3]).unwrap(),
        sk.add_line_between(p[3], p[0]).unwrap(),
    ];
    sk.add_constraint(ConstraintKind::Horizontal { a: l[0], b: None }).unwrap();
    sk.add_constraint(ConstraintKind::Horizontal { a: l[2], b: None }).unwrap();
    sk.add_constraint(ConstraintKind::Vertical { a: l[1], b: None }).unwrap();
    sk.add_constraint(ConstraintKind::Vertical { a: l[3], b: None }).unwrap();
    (p, l)
}

#[test]
fn rectangle_dof() {
    let mut sk = Sketch::new();
    let (p, l) = rect(&mut sk, 2.0, 1.0);
    let r = sk.solve();
    assert!(r.is_solved(), "{r:?}");
    assert_eq!(r.dof, Some(4));
    sk.add_constraint(ConstraintKind::Fix { entity: p[0] }).unwrap();
    let r = sk.solve();
    assert_eq!(r.dof, Some(2));
    let w = sk.add_constraint(ConstraintKind::Length { curve: l[0], value: 3.0 }).unwrap();
    sk.add_constraint(ConstraintKind::Distance { a: l[0], b: l[2], value: 1.5, mode: DistanceMode::Center }).unwrap();
    let r = sk.solve();
    assert!(r.is_solved(), "{r:?}");
    assert_eq!(r.dof, Some(0));
    for id in l.iter().chain(p.iter()) {
        assert_eq!(r.state(*id), Some(ConstraintState::FullyConstrained), "{id}");
    }
    let a = sk.point(p[0]).unwrap();
    let c = sk.point(p[2]).unwrap();
    assert!(((c - a) - v(3.0, 1.5)).length() < 1e-9, "{a} {c}");
    // remove the width: DOF 1, right edge + top/bottom under-constrained
    sk.remove_constraint(w).unwrap();
    let r = sk.solve();
    assert_eq!(r.dof, Some(1));
    assert_eq!(r.state(l[3]), Some(ConstraintState::FullyConstrained));
    assert_eq!(r.state(l[1]), Some(ConstraintState::UnderConstrained));
    assert_eq!(r.state(l[0]), Some(ConstraintState::UnderConstrained));
    assert_eq!(r.state(p[1]), Some(ConstraintState::UnderConstrained));
    assert_eq!(r.state(p[3]), Some(ConstraintState::FullyConstrained));
}

#[test]
fn conflicting_dimensions_are_named() {
    let mut sk = Sketch::new();
    let (p, l) = rect(&mut sk, 2.0, 1.0);
    sk.add_constraint(ConstraintKind::Fix { entity: p[0] }).unwrap();
    sk.add_constraint(ConstraintKind::Length { curve: l[0], value: 3.0 }).unwrap();
    sk.add_constraint(ConstraintKind::Length { curve: l[1], value: 1.0 }).unwrap();
    let bad = sk.add_constraint(ConstraintKind::Distance { a: p[0], b: p[2], value: 5.0, mode: DistanceMode::Center }).unwrap();
    let r = sk.solve();
    let conf = r.conflicting().to_vec();
    assert!(!conf.is_empty(), "{r:?}");
    assert!(conf.contains(&bad), "{conf:?}");
    assert!(matches!(r.status, SolveStatus::Overconstrained { .. }));
    assert_eq!(r.state(p[0]), Some(ConstraintState::OverConstrained));
    // rest of the sketch still solved
    let a = sk.point(p[0]).unwrap();
    let c = sk.point(p[2]).unwrap();
    assert!(((c - a).abs() - v(3.0, 1.0)).length() < 1e-8, "{a} {c}");
}

#[test]
fn redundant_consistent_is_solved() {
    let mut sk = Sketch::new();
    let (_, l) = rect(&mut sk, 2.0, 1.0);
    let red = sk.add_constraint(ConstraintKind::Parallel { a: l[0], b: l[2] }).unwrap();
    let r = sk.solve();
    assert!(r.is_solved(), "{r:?}");
    assert!(r.redundant.contains(&red), "{:?}", r.redundant);
    assert_eq!(r.dof, Some(4));
}

#[test]
fn drag_keeps_rectangle() {
    let mut sk = Sketch::new();
    let (p, l) = rect(&mut sk, 2.0, 1.0);
    sk.add_constraint(ConstraintKind::Fix { entity: p[0] }).unwrap();
    sk.solve();
    let r = sk.drag(&[(DragHandle::Point(p[2]), v(4.0, 3.0))]);
    assert!(r.is_solved(), "{r:?}");
    let c = sk.point(p[2]).unwrap();
    assert!((c - v(4.0, 3.0)).length() < 1e-6, "{c}");
    for (i, li) in l.iter().enumerate() {
        let (a, b) = sk.endpoints(*li).unwrap();
        let d = sk.point(b).unwrap() - sk.point(a).unwrap();
        if i % 2 == 0 {
            assert!(d.y.abs() < 1e-9);
        } else {
            assert!(d.x.abs() < 1e-9);
        }
    }
    assert!(sk.point(p[0]).unwrap().length() < 1e-12);
    // fully constrained point does not move
    let r = sk.drag(&[(DragHandle::Point(p[0]), v(1.0, 1.0))]);
    assert!(r.is_solved());
    assert!(sk.point(p[0]).unwrap().length() < 1e-9);
}

#[test]
fn each_constraint_converges() {
    // lines
    let check = |sk: &mut Sketch| {
        let r = sk.solve();
        assert!(r.is_solved(), "{r:?}");
        assert!(r.max_residual < 1e-9);
    };
    let mut sk = Sketch::new();
    let a = sk.add_line(v(0.0, 0.0), v(1.0, 0.2));
    let b = sk.add_line(v(0.0, 1.0), v(1.0, 1.5));
    sk.add_constraint(ConstraintKind::Parallel { a, b }).unwrap();
    check(&mut sk);
    let mut sk = Sketch::new();
    let a = sk.add_line(v(0.0, 0.0), v(1.0, 0.2));
    let b = sk.add_line(v(0.0, 1.0), v(1.0, 1.5));
    sk.add_constraint(ConstraintKind::Perpendicular { a, b }).unwrap();
    sk.add_constraint(ConstraintKind::Equal { a, b }).unwrap();
    let cl = sk.add_line(v(3.0, 0.0), v(4.0, 1.0));
    sk.add_constraint(ConstraintKind::Collinear { a, b: cl }).unwrap();
    check(&mut sk);
    let la = sk.endpoints(a).unwrap();
    let lb = sk.endpoints(b).unwrap();
    let da = sk.point(la.1).unwrap() - sk.point(la.0).unwrap();
    let db = sk.point(lb.1).unwrap() - sk.point(lb.0).unwrap();
    assert!(da.dot(db).abs() < 1e-9);
    assert!((da.length() - db.length()).abs() < 1e-9);

    // angle
    let mut sk = Sketch::new();
    let a = sk.add_line(v(0.0, 0.0), v(1.0, 0.0));
    let b = sk.add_line(v(0.0, 0.0), v(1.0, 1.2));
    let c = sk.add_constraint(ConstraintKind::Angle { a, b, value: 30f64.to_radians() }).unwrap();
    check(&mut sk);
    assert!((sk.measure(c).unwrap() - 30f64.to_radians()).abs() < 1e-9);

    // tangent line-circle, circle-circle, point on circle, midpoint, symmetric
    let mut sk = Sketch::new();
    let l = sk.add_line(v(-2.0, 1.2), v(2.0, 1.1));
    let c1 = sk.add_circle(v(0.0, 0.0), 1.0);
    let c2 = sk.add_circle(v(2.5, 0.3), 1.0);
    let a1 = sk.add_arc(v(0.0, -3.0), v(1.0, -3.0), v(0.0, -2.0));
    sk.add_constraint(ConstraintKind::Tangent { a: l, b: c1 }).unwrap();
    sk.add_constraint(ConstraintKind::Tangent { a: c1, b: c2 }).unwrap();
    sk.add_constraint(ConstraintKind::Tangent { a: a1, b: c1 }).unwrap();
    sk.add_constraint(ConstraintKind::Equal { a: a1, b: c2 }).unwrap();
    let p = sk.add_point(v(0.3, 0.3));
    sk.add_constraint(ConstraintKind::Midpoint { point: p, curve: l }).unwrap();
    let q = sk.add_point(v(5.0, 5.0));
    sk.add_constraint(ConstraintKind::Coincident { a: q, b: c2 }).unwrap();
    let axis = sk.add_line(v(0.0, -5.0), v(0.0, 5.0));
    let s1 = sk.add_point(v(-1.0, 3.0));
    let s2 = sk.add_point(v(1.2, 3.1));
    sk.add_constraint(ConstraintKind::Symmetric { a: s1, b: s2, axis }).unwrap();
    let r1 = sk.add_constraint(ConstraintKind::Radius { curve: c1, value: 0.8 }).unwrap();
    let ang = sk.add_constraint(ConstraintKind::ArcAngle { arc: a1, value: 120f64.to_radians() }).unwrap();
    check(&mut sk);
    assert!((sk.measure(r1).unwrap() - 0.8).abs() < 1e-9);
    assert!((sk.measure(ang).unwrap() - 120f64.to_radians()).abs() < 1e-9);
    let cc1 = sk.curve(c1).unwrap();
    let cl = sk.curve(l).unwrap();
    if let (Curve2::Circle { center, radius }, Curve2::Line { a, b }) = (cc1, cl) {
        let d = (b - a).perp_dot(center - a).abs() / (b - a).length();
        assert!((d - radius).abs() < 1e-9);
    } else {
        panic!()
    }

    // dimensions: horizontal / vertical distance, diameter, point-line distance
    let mut sk = Sketch::new();
    let p1 = sk.add_point(v(0.0, 0.0));
    let p2 = sk.add_point(v(1.0, 1.0));
    sk.add_constraint(ConstraintKind::HorizontalDistance { a: p1, b: p2, value: 2.5 }).unwrap();
    sk.add_constraint(ConstraintKind::VerticalDistance { a: p1, b: p2, value: 0.5 }).unwrap();
    let c = sk.add_circle(v(3.0, 3.0), 1.0);
    sk.add_constraint(ConstraintKind::Diameter { curve: c, value: 3.0 }).unwrap();
    let l = sk.add_line(v(-1.0, -1.0), v(5.0, -1.2));
    sk.add_constraint(ConstraintKind::Distance { a: p1, b: l, value: 0.7, mode: DistanceMode::Center }).unwrap();
    sk.add_constraint(ConstraintKind::Distance { a: c, b: l, value: 0.25, mode: DistanceMode::Minimum }).unwrap();
    sk.add_constraint(ConstraintKind::Distance { a: p2, b: c, value: 0.4, mode: DistanceMode::Minimum }).unwrap();
    check(&mut sk);
    let d = sk.point(p2).unwrap() - sk.point(p1).unwrap();
    assert!((d - v(2.5, 0.5)).length() < 1e-9);
    let _ = PI;
}

#[test]
fn ellipse_spline_conic_constraints() {
    let mut sk = Sketch::new();
    let e = sk.add_ellipse(v(0.0, 0.0), v(2.0, 0.2), 1.0);
    sk.add_constraint(ConstraintKind::Horizontal { a: e, b: None }).unwrap();
    let l = sk.add_line(v(-3.0, 1.3), v(3.0, 1.5));
    sk.add_constraint(ConstraintKind::Tangent { a: l, b: e }).unwrap();
    let p = sk.add_point(v(2.5, 0.5));
    sk.add_constraint(ConstraintKind::Coincident { a: p, b: e }).unwrap();
    sk.add_constraint(ConstraintKind::EllipseRadius { ellipse: e, major: true, value: 2.5 }).unwrap();
    let r = sk.solve();
    assert!(r.is_solved(), "{r:?}");

    let mut sk = Sketch::new();
    let s = sk.add_bspline(&[v(0.0, 0.0), v(1.0, 1.0), v(2.0, -1.0), v(3.0, 0.0)], 3).unwrap();
    let q = sk.add_point(v(1.5, 0.5));
    sk.add_constraint(ConstraintKind::Coincident { a: q, b: s }).unwrap();
    let (s0, _) = sk.endpoints(s).unwrap();
    let l = sk.add_line(v(-2.0, 0.5), v(0.0, 0.1));
    let (_, le) = sk.endpoints(l).unwrap();
    sk.add_constraint(ConstraintKind::Coincident { a: le, b: s0 }).unwrap();
    sk.add_constraint(ConstraintKind::Tangent { a: l, b: s }).unwrap();
    let arc = sk.add_arc(v(3.0, 1.0), v(3.1, 0.1), v(4.0, 1.0));
    let (_, s_end) = sk.endpoints(s).unwrap();
    let (a_s, _) = sk.endpoints(arc).unwrap();
    sk.add_constraint(ConstraintKind::Coincident { a: a_s, b: s_end }).unwrap();
    sk.add_constraint(ConstraintKind::Curvature { a: s, b: arc }).unwrap();
    let r = sk.solve();
    assert!(r.is_solved(), "{r:?}");
    let sc = sk.curve(s).unwrap();
    let qp = sk.point(q).unwrap();
    assert!(sc.distance(qp) < 1e-8);
    // G2: spline end curvature equals arc curvature
    let (_, t1) = sc.domain();
    let ks = sc.curvature(t1);
    let ac = sk.curve(arc).unwrap();
    let ka = ac.curvature(ac.domain().0);
    assert!((ks - ka).abs() < 1e-6, "{ks} {ka}");

    let mut sk = Sketch::new();
    let k = sk.add_conic(v(0.0, 0.0), v(1.0, 1.0), v(2.0, 0.0), 0.4).unwrap();
    let q = sk.add_point(v(1.0, 0.8));
    sk.add_constraint(ConstraintKind::Coincident { a: q, b: k }).unwrap();
    sk.add_constraint(ConstraintKind::Fix { entity: k }).unwrap();
    let r = sk.solve();
    assert!(r.is_solved(), "{r:?}");
    assert_eq!(r.dof, Some(1));

    let mut sk = Sketch::new();
    let f = sk.add_fit_spline(&[v(0.0, 0.0), v(1.0, 1.0), v(2.0, 0.5), v(3.0, 1.5)]).unwrap();
    let q = sk.add_point(v(1.5, 1.2));
    sk.add_constraint(ConstraintKind::Coincident { a: q, b: f }).unwrap();
    let l = sk.add_line(v(-1.0, -0.8), v(0.0, 0.0));
    let (_, le) = sk.endpoints(l).unwrap();
    let (f0, _) = sk.endpoints(f).unwrap();
    sk.add_constraint(ConstraintKind::Coincident { a: le, b: f0 }).unwrap();
    sk.add_constraint(ConstraintKind::Tangent { a: l, b: f }).unwrap();
    let r = sk.solve();
    assert!(r.is_solved(), "{r:?}");
    assert!(sk.curve(f).unwrap().distance(sk.point(q).unwrap()) < 1e-7);
}

#[test]
fn driven_dimension_is_measured() {
    let mut sk = Sketch::new();
    let l = sk.add_line(v(0.0, 0.0), v(3.0, 4.0));
    let d = sk.add_reference(ConstraintKind::Length { curve: l, value: 0.0 }).unwrap();
    assert!((sk.constraint(d).unwrap().value().unwrap() - 5.0).abs() < 1e-12);
    let r = sk.solve();
    assert!(r.is_solved());
    assert_eq!(r.dof, Some(4));
}

#[test]
fn serde_roundtrip() {
    let mut sk = Sketch::new();
    let (p, _) = rect(&mut sk, 2.0, 1.0);
    sk.add_constraint(ConstraintKind::Fix { entity: p[0] }).unwrap();
    let s = serde_json::to_string(&sk).unwrap();
    let mut back: Sketch = serde_json::from_str(&s).unwrap();
    let r = back.solve();
    assert!(r.is_solved());
    assert_eq!(r.dof, Some(2));
}
