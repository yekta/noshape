use noshape_sketch::*;
use std::f64::consts::PI;

fn v(x: f64, y: f64) -> DVec2 {
    DVec2::new(x, y)
}

fn solved(sk: &mut Sketch) -> SolveReport {
    let r = sk.solve();
    assert!(r.is_solved(), "{r:?}");
    assert!(r.max_residual < 1e-9, "{r:?}");
    r
}

#[test]
fn dof_per_entity_type_and_fix() {
    type Maker = Box<dyn Fn(&mut Sketch) -> EntityId>;
    let cases: Vec<(Maker, usize)> = vec![
        (Box::new(|s: &mut Sketch| s.add_point(v(1.0, 2.0))), 2),
        (Box::new(|s: &mut Sketch| s.add_line(v(0.0, 0.0), v(1.0, 2.0))), 4),
        (Box::new(|s: &mut Sketch| s.add_circle(v(0.0, 0.0), 1.0)), 3),
        (Box::new(|s: &mut Sketch| s.add_arc(v(0.0, 0.0), v(1.0, 0.0), v(0.0, 1.0))), 5),
        (Box::new(|s: &mut Sketch| s.add_ellipse(v(0.0, 0.0), v(2.0, 0.5), 1.0)), 5),
        (Box::new(|s: &mut Sketch| s.add_elliptical_arc(v(0.0, 0.0), v(2.0, 0.0), 1.0, 0.2, 2.0)), 7),
        (Box::new(|s: &mut Sketch| s.add_bspline(&[v(0.0, 0.0), v(1.0, 1.0), v(2.0, 0.0), v(3.0, 1.0)], 3).unwrap()), 8),
        (Box::new(|s: &mut Sketch| s.add_fit_spline(&[v(0.0, 0.0), v(1.0, 1.0), v(2.0, 0.0)]).unwrap()), 6),
        (Box::new(|s: &mut Sketch| s.add_conic(v(0.0, 0.0), v(1.0, 1.0), v(2.0, 0.0), 0.3).unwrap()), 6),
    ];
    for (make, dof) in cases {
        let mut sk = Sketch::new();
        let id = make(&mut sk);
        let r = solved(&mut sk);
        assert_eq!(r.dof, Some(dof), "{:?}", sk.kind(id));
        assert_eq!(r.state(id), Some(ConstraintState::UnderConstrained));
        let f = sk.add_constraint(ConstraintKind::Fix { entity: id }).unwrap();
        let r = solved(&mut sk);
        // Live (live2 sketch-state 11-17): Fix on a line holds the line, its two ends free to slide along it.
        let left = if sk.kind(id) == Some(EntityKind::Line) { 2 } else { 0 };
        assert_eq!(r.dof, Some(left), "{:?}", sk.kind(id));
        assert_eq!(r.state(id), Some(ConstraintState::FullyConstrained));
        assert!(r.redundant.is_empty(), "{:?} {:?}", sk.kind(id), r.redundant);
        let _ = f;
    }
}

#[test]
fn remaining_constraint_types_converge() {
    // concentric, coincident circle-circle, normal, equal ellipses
    let mut sk = Sketch::new();
    let c1 = sk.add_circle(v(0.0, 0.0), 1.0);
    let c2 = sk.add_circle(v(0.3, 0.2), 2.0);
    let a1 = sk.add_arc(v(0.5, -0.5), v(1.5, -0.5), v(0.5, 0.5));
    sk.add_constraint(ConstraintKind::Concentric { a: c1, b: c2 }).unwrap();
    sk.add_constraint(ConstraintKind::Coincident { a: a1, b: c1 }).unwrap();
    let l = sk.add_line(v(-3.0, 0.5), v(3.0, 0.8));
    sk.add_constraint(ConstraintKind::Normal { line: l, curve: c2 }).unwrap();
    let e1 = sk.add_ellipse(v(5.0, 0.0), v(2.0, 0.0), 1.0);
    let e2 = sk.add_ellipse(v(9.0, 0.0), v(1.5, 0.5), 0.7);
    sk.add_constraint(ConstraintKind::Equal { a: e1, b: e2 }).unwrap();
    solved(&mut sk);
    let (cc1, cc2) = (sk.curve(c1).unwrap(), sk.curve(c2).unwrap());
    if let (Curve2::Circle { center: o1, .. }, Curve2::Circle { center: o2, .. }) = (cc1, cc2) {
        assert!((o1 - o2).length() < 1e-9);
        assert!(sk.curve(l).unwrap().distance(o1) < 1e-6 || {
            let Curve2::Line { a, b } = sk.curve(l).unwrap() else { panic!() };
            ((b - a).perp_dot(o1 - a) / (b - a).length()).abs() < 1e-9
        });
    }
    if let Curve2::Arc { center, radius, .. } = sk.curve(a1).unwrap() {
        let Curve2::Circle { center: o1, radius: r1 } = sk.curve(c1).unwrap() else { panic!() };
        assert!((center - o1).length() < 1e-9 && (radius - r1).abs() < 1e-9);
    }

    // midpoint of arc, symmetric lines / circles / arcs, arc length, collinear
    let mut sk = Sketch::new();
    let arc = sk.add_arc(v(0.0, 0.0), v(1.0, 0.0), v(0.0, 1.0));
    let m = sk.add_point(v(0.5, 0.5));
    sk.add_constraint(ConstraintKind::Midpoint { point: m, curve: arc }).unwrap();
    let len = sk.add_constraint(ConstraintKind::Length { curve: arc, value: 2.0 }).unwrap();
    let axis = sk.add_line(v(5.0, -5.0), v(5.0, 5.0));
    let la = sk.add_line(v(3.0, 0.0), v(4.0, 1.0));
    let lb = sk.add_line(v(7.2, 0.1), v(6.1, 0.9));
    sk.add_constraint(ConstraintKind::Symmetric { a: la, b: lb, axis }).unwrap();
    let ca = sk.add_circle(v(3.0, 3.0), 0.5);
    let cb = sk.add_circle(v(7.1, 3.2), 0.7);
    sk.add_constraint(ConstraintKind::Symmetric { a: ca, b: cb, axis }).unwrap();
    let aa = sk.add_arc(v(3.0, -3.0), v(4.0, -3.0), v(3.0, -2.0));
    let ab = sk.add_arc(v(7.0, -3.0), v(7.0, -2.0), v(6.0, -3.0));
    sk.add_constraint(ConstraintKind::Symmetric { a: aa, b: ab, axis }).unwrap();
    solved(&mut sk);
    assert!((sk.measure(len).unwrap() - 2.0).abs() < 1e-9);
    let mp = sk.point(m).unwrap();
    let Curve2::Arc { center, start_angle, end_angle, .. } = sk.curve(arc).unwrap() else { panic!() };
    assert!(((mp - center).to_angle() - 0.5 * (start_angle + end_angle)).abs() < 1e-9);
    let (Curve2::Line { a: a0, b: a1 }, Curve2::Line { a: b0, b: b1 }) = (sk.curve(la).unwrap(), sk.curve(lb).unwrap()) else { panic!() };
    let ax = sk.curve(axis).unwrap();
    let Curve2::Line { a: x0, b: x1 } = ax else { panic!() };
    assert!((reflect(a0, x0, x1) - b0).length() < 1e-9);
    assert!((reflect(a1, x0, x1) - b1).length() < 1e-9);

    // distances between lines and circles in all modes
    let mut sk = Sketch::new();
    let l1 = sk.add_line(v(0.0, 0.0), v(4.0, 0.0));
    let l2 = sk.add_line(v(0.0, 1.0), v(4.0, 1.3));
    sk.add_constraint(ConstraintKind::Parallel { a: l1, b: l2 }).unwrap();
    let d = sk.add_constraint(ConstraintKind::Distance { a: l1, b: l2, value: 2.0, mode: DistanceMode::Center }).unwrap();
    let c1 = sk.add_circle(v(0.0, 5.0), 1.0);
    let c2 = sk.add_circle(v(4.0, 5.0), 1.0);
    let dmin = sk.add_constraint(ConstraintKind::Distance { a: c1, b: c2, value: 1.0, mode: DistanceMode::Minimum }).unwrap();
    let c3 = sk.add_circle(v(10.0, 5.0), 0.5);
    let c4 = sk.add_circle(v(12.0, 5.0), 0.5);
    let dmax = sk.add_constraint(ConstraintKind::Distance { a: c3, b: c4, value: 4.0, mode: DistanceMode::Maximum }).unwrap();
    let p = sk.add_point(v(0.0, 8.0));
    let dp = sk.add_constraint(ConstraintKind::Distance { a: p, b: c1, value: 1.5, mode: DistanceMode::Maximum }).unwrap();
    let vd = sk.add_constraint(ConstraintKind::VerticalDistance { a: l1, b: c1, value: 6.0 }).unwrap();
    solved(&mut sk);
    for (id, val) in [(d, 2.0), (dmin, 1.0), (dmax, 4.0), (dp, 1.5), (vd, 6.0)] {
        assert!((sk.measure(id).unwrap() - val).abs() < 1e-9, "{id}");
    }

    // tangent arc-arc through shared endpoint, spline-spline tangency, conic tangency
    let mut sk = Sketch::new();
    let c0 = sk.add_point(v(0.0, 0.0));
    let s0 = sk.add_point(v(1.0, 0.0));
    let e0 = sk.add_point(v(0.0, 1.0));
    let a0 = sk.add_arc_between(c0, s0, e0).unwrap();
    let c1p = sk.add_point(v(0.2, 2.1));
    let e1 = sk.add_point(v(-1.0, 2.0));
    let a1 = sk.add_arc_between(c1p, e1, e0).unwrap();
    sk.add_constraint(ConstraintKind::Tangent { a: a0, b: a1 }).unwrap();
    let sp1 = sk.add_bspline(&[v(3.0, 0.0), v(4.0, 1.0), v(5.0, 0.5)], 2).unwrap();
    let sp2 = sk.add_bspline(&[v(5.0, 0.5), v(6.0, 1.0), v(7.0, 0.0)], 2).unwrap();
    let (_, sp1e) = sk.endpoints(sp1).unwrap();
    let (sp2s, _) = sk.endpoints(sp2).unwrap();
    sk.add_constraint(ConstraintKind::Coincident { a: sp1e, b: sp2s }).unwrap();
    sk.add_constraint(ConstraintKind::Tangent { a: sp1, b: sp2 }).unwrap();
    let k = sk.add_conic(v(10.0, 0.0), v(11.0, 1.0), v(12.0, 0.0), 0.4).unwrap();
    let kl = sk.add_line(v(8.0, -1.2), v(10.0, 0.0));
    let (_, kle) = sk.endpoints(kl).unwrap();
    let (ks, _) = sk.endpoints(k).unwrap();
    sk.add_constraint(ConstraintKind::Coincident { a: kle, b: ks }).unwrap();
    sk.add_constraint(ConstraintKind::Tangent { a: kl, b: k }).unwrap();
    solved(&mut sk);
    let Curve2::Arc { center: o0, .. } = sk.curve(a0).unwrap() else { panic!() };
    let Curve2::Arc { center: o1, .. } = sk.curve(a1).unwrap() else { panic!() };
    let pe = sk.point(e0).unwrap();
    assert!((pe - o0).perp_dot(pe - o1).abs() < 1e-9);
    let t1 = sk.curve(sp1).unwrap().tangent(1.0).normalize();
    let t2 = sk.curve(sp2).unwrap().tangent(0.0).normalize();
    assert!(t1.perp_dot(t2).abs() < 1e-9);

    // horizontal / vertical point pairs and ellipse axis
    let mut sk = Sketch::new();
    let p1 = sk.add_point(v(0.0, 0.0));
    let p2 = sk.add_point(v(2.0, 0.5));
    let p3 = sk.add_point(v(0.3, 3.0));
    sk.add_constraint(ConstraintKind::Horizontal { a: p1, b: Some(p2) }).unwrap();
    sk.add_constraint(ConstraintKind::Vertical { a: p1, b: Some(p3) }).unwrap();
    let e = sk.add_ellipse(v(5.0, 5.0), v(1.0, 1.0), 0.5);
    sk.add_constraint(ConstraintKind::Vertical { a: e, b: None }).unwrap();
    sk.add_constraint(ConstraintKind::EllipseRadius { ellipse: e, major: false, value: 0.25 }).unwrap();
    solved(&mut sk);
    assert!((sk.point(p1).unwrap().y - sk.point(p2).unwrap().y).abs() < 1e-12);
    assert!((sk.point(p1).unwrap().x - sk.point(p3).unwrap().x).abs() < 1e-12);
    let Geometry::Ellipse { major, minor, .. } = sk.entity(e).unwrap().geometry else { panic!() };
    assert!(major.x.abs() < 1e-9 && (minor - 0.25).abs() < 1e-9, "{major} {minor}");
}

#[test]
fn angle_dimension_quadrant_is_stable() {
    let mut sk = Sketch::new();
    let a = sk.add_line(v(0.0, 0.0), v(1.0, 0.0));
    let b = sk.add_line(v(0.0, 0.0), v(-1.0, 0.6));
    // lines at ~149 deg: asking for 30 deg measures the supplement
    let ang = sk.add_constraint(ConstraintKind::Angle { a, b, value: 30f64.to_radians() }).unwrap();
    sk.add_constraint(ConstraintKind::Fix { entity: a }).unwrap();
    solved(&mut sk);
    let (b0, b1) = sk.endpoints(b).unwrap();
    let d = sk.point(b1).unwrap() - sk.point(b0).unwrap();
    assert!((d.to_angle() - 150f64.to_radians()).abs() < 1e-9, "{}", d.to_angle().to_degrees());
    // change the value repeatedly: never flips to the other quadrant
    for deg in [45.0, 80.0, 100.0, 135.0, 20.0] {
        sk.set_dimension_value(ang, f64::to_radians(deg)).unwrap();
        solved(&mut sk);
        assert!((sk.measure(ang).unwrap() - f64::to_radians(deg)).abs() < 1e-9);
    }
}

#[test]
fn conflicts_and_fixed_fixed() {
    // horizontal + vertical + length > 0 on one line
    let mut sk = Sketch::new();
    let l = sk.add_line(v(0.0, 0.0), v(1.0, 0.3));
    sk.add_constraint(ConstraintKind::Horizontal { a: l, b: None }).unwrap();
    sk.add_constraint(ConstraintKind::Length { curve: l, value: 1.0 }).unwrap();
    let bad = sk.add_constraint(ConstraintKind::Vertical { a: l, b: None }).unwrap();
    let r = sk.solve();
    assert!(r.conflicting().contains(&bad), "{r:?}");
    assert_eq!(r.state(l), Some(ConstraintState::OverConstrained));
    // geometry still satisfies the non-conflicting constraints
    let (a, b) = sk.endpoints(l).unwrap();
    let d = sk.point(b).unwrap() - sk.point(a).unwrap();
    assert!(d.y.abs() < 1e-9 && (d.length() - 1.0).abs() < 1e-9);
    sk.remove_constraint(bad).unwrap();
    assert!(sk.solve().is_solved());

    // two external points made coincident: conflict between fixed geometry
    let mut sk = Sketch::new();
    let p = sk.use_external_point(v(0.0, 0.0));
    let q = sk.use_external_point(v(1.0, 0.0));
    let c = sk.add_constraint(ConstraintKind::Coincident { a: p, b: q }).unwrap();
    let r = sk.solve();
    assert_eq!(r.conflicting(), &[c]);

    // inconsistent radius + diameter
    let mut sk = Sketch::new();
    let circ = sk.add_circle(v(0.0, 0.0), 1.0);
    sk.add_constraint(ConstraintKind::Radius { curve: circ, value: 1.0 }).unwrap();
    let dia = sk.add_constraint(ConstraintKind::Diameter { curve: circ, value: 3.0 }).unwrap();
    let r = sk.solve();
    assert!(r.conflicting().contains(&dia));
    // Redundant but consistent radius + diameter: a dimension the rest
    // already fixes over-defines the sketch (live fuzz s7: a radius
    // dimensioned twice, "Sketch could not be solved.").
    sk.set_dimension_value(dia, 2.0).unwrap();
    let r = sk.solve();
    assert_eq!(r.conflicting(), &[dia], "{r:?}");
    // switching it to a reference dimension removes the redundancy
    sk.set_driving(dia, false).unwrap();
    let r = sk.solve();
    assert!(r.redundant.is_empty());
    assert!((sk.constraint(dia).unwrap().value().unwrap() - 2.0).abs() < 1e-12);
}

#[test]
fn circle_drag_changes_radius_and_drag_respects_dims() {
    let mut sk = Sketch::new();
    let c = sk.add_circle(v(0.0, 0.0), 1.0);
    let (center,) = (sk.entity(c).unwrap().center().unwrap(),);
    sk.add_constraint(ConstraintKind::Fix { entity: center }).unwrap();
    sk.solve();
    let r = sk.drag(&[(DragHandle::Entity { id: c, grab: v(1.0, 0.0) }, v(2.0, 0.0))]);
    assert!(r.is_solved());
    assert!((sk.radius_of_test(c) - 2.0).abs() < 1e-9);
    assert_eq!(r.dof, Some(1));
    sk.add_constraint(ConstraintKind::Radius { curve: c, value: 1.5 }).unwrap();
    sk.solve();
    sk.drag(&[(DragHandle::Entity { id: c, grab: v(1.5, 0.0) }, v(3.0, 0.0))]);
    assert!((sk.radius_of_test(c) - 1.5).abs() < 1e-9);
    // dragging a line translates it
    let mut sk = Sketch::new();
    let l = sk.add_line(v(0.0, 0.0), v(1.0, 0.0));
    sk.add_constraint(ConstraintKind::Horizontal { a: l, b: None }).unwrap();
    sk.drag(&[(DragHandle::Entity { id: l, grab: v(0.5, 0.0) }, v(0.5, 1.0))]);
    let Curve2::Line { a, b } = sk.curve(l).unwrap() else { panic!() };
    assert!((a - v(0.0, 1.0)).length() < 1e-9 && (b - v(1.0, 1.0)).length() < 1e-9);
}

trait RadiusTest {
    fn radius_of_test(&self, id: EntityId) -> f64;
}
impl RadiusTest for Sketch {
    fn radius_of_test(&self, id: EntityId) -> f64 {
        match self.curve(id).unwrap() {
            Curve2::Circle { radius, .. } | Curve2::Arc { radius, .. } => radius,
            _ => panic!(),
        }
    }
}

#[test]
fn solver_is_deterministic_and_minimal() {
    let build = || {
        let mut sk = Sketch::new();
        let r = sk.corner_rectangle(v(0.0, 0.0), v(2.0, 1.0));
        sk.add_constraint(ConstraintKind::Length { curve: r.curves[0], value: 3.0 }).unwrap();
        sk.add_constraint(ConstraintKind::Length { curve: r.curves[1], value: 2.0 }).unwrap();
        let c = sk.add_circle(v(1.0, 0.5), 0.3);
        sk.add_constraint(ConstraintKind::Tangent { a: r.curves[0], b: c }).unwrap();
        sk.add_constraint(ConstraintKind::Tangent { a: r.curves[3], b: c }).unwrap();
        sk
    };
    let mut s1 = build();
    let mut s2 = build();
    s1.solve();
    s2.solve();
    let j1 = serde_json::to_string(&s1).unwrap();
    let j2 = serde_json::to_string(&s2).unwrap();
    assert_eq!(j1, j2);
    // solving an already-solved sketch changes nothing
    let before = j1.clone();
    s1.solve();
    assert_eq!(serde_json::to_string(&s1).unwrap(), before);
    // minimal change: the bottom-left corner (shared by the fixed-length edges)
    // stays near where it was since nothing pulls it
    let p0 = s1.entities().find(|e| e.kind() == EntityKind::Point).unwrap().id;
    assert!(s1.point(p0).unwrap().length() < 1.0);
    let _ = PI;
}

#[test]
fn invalid_constraints_are_rejected() {
    let mut sk = Sketch::new();
    let l = sk.add_line(v(0.0, 0.0), v(1.0, 0.0));
    let p = sk.add_point(v(0.0, 1.0));
    let c = sk.add_circle(v(0.0, 0.0), 1.0);
    assert!(sk.add_constraint(ConstraintKind::Parallel { a: l, b: p }).is_err());
    assert!(sk.add_constraint(ConstraintKind::Radius { curve: l, value: 1.0 }).is_err());
    assert!(sk.add_constraint(ConstraintKind::Parallel { a: l, b: l }).is_err());
    assert!(sk.add_constraint(ConstraintKind::Angle { a: l, b: c, value: 1.0 }).is_err());
    assert!(sk.add_constraint(ConstraintKind::Coincident { a: p, b: EntityId(999) }).is_err());
    assert!(sk.add_constraint(ConstraintKind::Length { curve: l, value: f64::NAN }).is_err());
    // removing an entity removes dependent constraints
    sk.add_constraint(ConstraintKind::Coincident { a: p, b: c }).unwrap();
    sk.remove_entity(c);
    assert_eq!(sk.constraint_count(), 0);
    // removing a line removes its now-orphaned endpoints
    let before = sk.entity_count();
    let removed = sk.remove_entity(l);
    assert_eq!(removed.len(), 3);
    assert_eq!(sk.entity_count(), before - 3);
}

#[test]
fn large_dimension_changes_converge() {
    let mut sk = Sketch::new();
    let r = sk.corner_rectangle(v(0.0, 0.0), v(2.0, 1.0));
    sk.add_constraint(ConstraintKind::Fix { entity: r.points[0] }).unwrap();
    let w = sk.add_constraint(ConstraintKind::Length { curve: r.curves[0], value: 2.0 }).unwrap();
    let h = sk.add_constraint(ConstraintKind::Length { curve: r.curves[1], value: 1.0 }).unwrap();
    let c = sk.add_circle(v(1.0, 0.5), 0.3);
    sk.add_constraint(ConstraintKind::Tangent { a: r.curves[0], b: c }).unwrap();
    sk.add_constraint(ConstraintKind::Tangent { a: r.curves[3], b: c }).unwrap();
    let rd = sk.add_constraint(ConstraintKind::Radius { curve: c, value: 0.3 }).unwrap();
    solved(&mut sk);
    for (id, val) in [(w, 200.0), (h, 150.0), (rd, 40.0), (w, 0.5), (h, 0.01), (rd, 0.001), (w, 1e3)] {
        sk.set_dimension_value(id, val).unwrap();
        let rep = sk.solve();
        assert!(rep.is_solved(), "{id} -> {val}: {rep:?}");
        assert!((sk.measure(id).unwrap() - val).abs() < 1e-9 * val.max(1.0));
    }
    let mut sk = Sketch::new();
    let a = sk.add_line(v(0.0, 0.0), v(1.0, 0.0));
    let b = sk.add_line(v(0.0, 0.0), v(1.0, 1.0));
    sk.add_constraint(ConstraintKind::Fix { entity: a }).unwrap();
    let ang = sk.add_constraint(ConstraintKind::Angle { a, b, value: PI / 4.0 }).unwrap();
    for deg in [170.0, 5.0, 179.0, 1.0, 90.0] {
        sk.set_dimension_value(ang, f64::to_radians(deg)).unwrap();
        let rep = sk.solve();
        assert!(rep.is_solved(), "{deg}: {rep:?}");
        assert!((sk.measure(ang).unwrap() - f64::to_radians(deg)).abs() < 1e-9);
    }
}


/// Live 1862 (SKETCH-REDUNDANT-SOLVES): a Ø10 circle Tangent to two
/// Horizontal lines and the lines' 10 mm Distance dimensioned too can't be
/// solved, in whatever order they were added; without the Distance it solves.
#[test]
fn a_dimension_the_tangencies_already_fix_over_defines() {
    for order in 0..4 {
        for with_distance in [true, false] {
            let mut sk = Sketch::new();
            let a = sk.add_line(v(0.0, 0.0), v(0.03, 0.0));
            let b = sk.add_line(v(0.0, 0.01), v(0.03, 0.01));
            let c = sk.add_circle(v(0.015, 0.005), 0.005);
            sk.add_constraint(ConstraintKind::Horizontal { a, b: None }).unwrap();
            sk.add_constraint(ConstraintKind::Horizontal { a: b, b: None }).unwrap();
            let mut adds = vec![
                ConstraintKind::Tangent { a, b: c },
                ConstraintKind::Tangent { a: b, b: c },
                ConstraintKind::Diameter { curve: c, value: 0.01 },
                ConstraintKind::Distance { a, b, value: 0.01, mode: DistanceMode::Minimum },
            ];
            if !with_distance {
                adds.pop();
            }
            let n = adds.len();
            adds.rotate_left(order % n);
            for k in adds {
                sk.add_constraint(k).unwrap();
            }
            let r = sk.solve();
            assert_eq!(r.is_solved(), !with_distance, "order {order}, distance {with_distance}: {:?}", r.status);
        }
    }
}

/// asm-welding-config 9c7704's Sketch 13 (live keeps it): an arc whose ends lie on two horizontal lines tangent to a
/// Ø15.875 circle, its centre level with the circle's, then Ø15.875 itself. The rest only leaves the ends' dx, which
/// the Ø takes to 0 at a singular point: not a dimension the rest already fixes, unlike live 1862's above.
#[test]
fn a_dimension_meeting_the_rest_at_a_singular_point_solves() {
    let mut sk = Sketch::new();
    let r = 0.0079375;
    let c1 = sk.add_point(v(0.0, 0.0));
    sk.add_constraint(ConstraintKind::Fix { entity: c1 }).unwrap();
    let circle = sk.add_circle_at(c1, r).unwrap();
    sk.add_constraint(ConstraintKind::Diameter { curve: circle, value: 2.0 * r }).unwrap();
    let (top, bottom) = (sk.add_point(v(-0.055, r)), sk.add_point(v(-0.055, -r)));
    let (t1, b1) = (sk.add_point(v(0.0, r)), sk.add_point(v(0.0, -r)));
    let l1 = sk.add_line_between(top, t1).unwrap();
    let l2 = sk.add_line_between(bottom, b1).unwrap();
    for l in [l1, l2] {
        sk.add_constraint(ConstraintKind::Horizontal { a: l, b: None }).unwrap();
        sk.add_constraint(ConstraintKind::Tangent { a: l, b: circle }).unwrap();
    }
    let c2 = sk.add_point(v(-0.055, 0.0));
    let arc = sk.add_arc_between(c2, top, bottom).unwrap();
    sk.add_constraint(ConstraintKind::Horizontal { a: c2, b: Some(c1) }).unwrap();
    solved(&mut sk);
    sk.add_constraint(ConstraintKind::Diameter { curve: arc, value: 2.0 * r }).unwrap();
    solved(&mut sk);
}

/// asm-suspension 995636's Sketch 4 (live): a hexagon side 8 mm inside an r22.25 rim, a minimum DISTANCE between them:
/// from the line out to the rim, r - d (ours took d - r, -8, and pulled the side 18.5 mm out).
#[test]
fn a_minimum_distance_from_a_line_inside_a_circle_is_to_its_rim() {
    let mut sk = Sketch::new();
    let c = sk.add_point(v(0.0, 0.0));
    sk.add_constraint(ConstraintKind::Fix { entity: c }).unwrap();
    let rim = sk.add_circle_at(c, 0.02225).unwrap();
    sk.add_constraint(ConstraintKind::Radius { curve: rim, value: 0.02225 }).unwrap();
    let side = sk.add_line(v(-0.0082, -0.01425), v(0.0082, -0.01425));
    sk.add_constraint(ConstraintKind::Horizontal { a: side, b: None }).unwrap();
    let d = sk.add_constraint(ConstraintKind::Distance { a: side, b: rim, value: 0.008, mode: DistanceMode::Minimum }).unwrap();
    solved(&mut sk);
    let Some(Curve2::Line { a, .. }) = sk.curve(side) else { panic!("a line") };
    assert!((a.y + 0.01425).abs() < 1e-9, "the side stays: {a}");
    assert!((sk.measure(d).unwrap() - 0.008).abs() < 1e-12);
}

#[test]
fn a_minimum_distance_between_overlapping_circles_is_their_overlap() {
    // c2-hunting-knife's Sketch 8 (live): two mirrored R60.65 arcs 108.8 mm apart, a 12.5 mm Distance between them
    // measured on the centre line where they overlap.
    let mut sk = Sketch::new();
    let (r, x) = (0.0606532, 0.0544032);
    let a = sk.add_circle(v(x, 0.0), r);
    let b = sk.add_circle(v(-x, 0.0), r);
    sk.add_constraint(ConstraintKind::Radius { curve: a, value: r }).unwrap();
    sk.add_constraint(ConstraintKind::Radius { curve: b, value: r }).unwrap();
    let d = sk.add_constraint(ConstraintKind::Distance { a, b, value: 0.0125, mode: DistanceMode::Minimum }).unwrap();
    solved(&mut sk);
    assert!((sk.measure(d).unwrap() - 0.0125).abs() < 1e-9);
    sk.set_dimension_value(d, 0.014).unwrap();
    solved(&mut sk);
    let ca = sk.measure_new(&ConstraintKind::Distance { a, b, value: 0.0, mode: DistanceMode::Center }).unwrap();
    assert!((ca - (2.0 * r - 0.014)).abs() < 1e-9, "{ca}");
    // Apart, the gap as before.
    let mut sk = Sketch::new();
    let a = sk.add_circle(v(0.0, 0.0), 1.0);
    let b = sk.add_circle(v(3.0, 0.0), 1.0);
    let d = sk.add_constraint(ConstraintKind::Distance { a, b, value: 1.0, mode: DistanceMode::Minimum }).unwrap();
    solved(&mut sk);
    assert!((sk.measure(d).unwrap() - 1.0).abs() < 1e-9);
}

/// Live's MIRROR of two lines (asm-pinslot's washer, Sketch 1): the lines mirrored about the axis, their ends free:
/// bottom x 2.65–4.75 at y 0.5 and top 2.65–5.0 at y −0.5 hold it as they are; a turn of one turns the other.
#[test]
fn mirrored_lines_hold_as_lines_with_their_ends_free() {
    let mut sk = Sketch::new();
    let axis = sk.add_line(v(0.0, 0.0), v(1.0, 0.0));
    sk.add_constraint(ConstraintKind::Fix { entity: axis }).unwrap();
    let a = sk.add_line(v(2.65, 0.5), v(4.75, 0.5));
    let b = sk.add_line(v(2.65, -0.5), v(5.0, -0.5));
    let c = sk.add_constraint(ConstraintKind::Symmetric { a, b, axis }).unwrap();
    sk.mirror_lines_only(c).unwrap();
    solved(&mut sk);
    let ends = |sk: &Sketch, l: EntityId| match sk.curve(l) {
        Some(Curve2::Line { a, b }) => (a, b),
        _ => panic!("a line"),
    };
    let (b0, b1) = ends(&sk, b);
    assert!((b0 - v(2.65, -0.5)).length() < 1e-9 && (b1 - v(5.0, -0.5)).length() < 1e-9, "the trimmed end stays: {b0} {b1}");
    let (_, a1) = ends(&sk, a);
    let a_end = sk.entity(a).and_then(|e| e.point_refs().get(1).copied()).unwrap();
    assert!(sk.drag(&[(DragHandle::Point(a_end), a1 + v(0.0, 0.5))]).is_solved());
    let ((a0, a1), (b0, b1)) = (ends(&sk, a), ends(&sk, b));
    let mirror = |p: DVec2| v(p.x, -p.y);
    let (da, db) = ((a1 - a0).normalize(), (b1 - b0).normalize());
    assert!(mirror(da).perp_dot(db).abs() < 1e-9, "directions mirrored: {da} {db}");
    assert!((mirror(b0) - a0).perp_dot(da).abs() < 1e-9, "b's line the mirror of a's: {a0} {a1} {b0} {b1}");
}

/// An ellipse held only at its major axis ends (two points on it there, its centre and axis vertical) keeps its minor
/// radius: nothing constrains it (live c2-surf-workshop-knifehandle P4, two Pierces at helpParameters π and 2π:
/// centre −7.1249, major 20.6713 mm, minor 16.6299 kept).
#[test]
fn ellipse_held_at_its_major_ends_keeps_its_minor_radius() {
    let mut sk = Sketch::new();
    let e = sk.add_ellipse(v(0.0, -13.0092), v(0.0, 23.6416), 16.6299);
    let Some(Geometry::Ellipse { center, .. }) = sk.entity(e).map(|x| x.geometry.clone()) else { panic!() };
    let origin = sk.add_point(v(0.0, 0.0));
    sk.add_constraint(ConstraintKind::Fix { entity: origin }).unwrap();
    sk.add_constraint(ConstraintKind::Vertical { a: center, b: Some(origin) }).unwrap();
    sk.add_constraint(ConstraintKind::Vertical { a: e, b: None }).unwrap();
    for y in [13.5464, -27.7962] {
        let p = sk.add_point(v(0.0, y));
        sk.add_constraint(ConstraintKind::Fix { entity: p }).unwrap();
        sk.add_constraint(ConstraintKind::Coincident { a: p, b: e }).unwrap();
    }
    solved(&mut sk);
    let Some(Curve2::Ellipse { center, major, minor }) = sk.curve(e) else { panic!() };
    assert!((minor - 16.6299).abs() < 1e-9, "minor {minor}");
    assert!((center - v(0.0, -7.1249)).length() < 1e-9 && (major.length() - 20.6713).abs() < 1e-9, "{center} {major}");
}

/// Live (skasm spline-image-1125 (2), (5)): a spline's body drag bends it toward the pointer, it doesn't move whole
/// (nor stay put with its start held): every free point and both handles move, the held start stays; a Bezier's
/// control points all move, by their weight at the grab, across the drag only.
#[test]
fn a_splines_body_drag_bends_it_as_live() {
    let mm = |x: f64, y: f64| v(x / 1000.0, y / 1000.0);
    let mut sk = Sketch::new();
    let s = sk.add_fit_spline_with_handles(&[mm(0.0, 0.0), mm(30.0, 20.0), mm(60.0, 0.0), mm(90.0, 20.0)]).unwrap();
    let refs = sk.entity(s).unwrap().point_refs().to_vec();
    sk.add_constraint(ConstraintKind::Fix { entity: refs[0] }).unwrap();
    solved(&mut sk);
    let before: Vec<DVec2> = refs.iter().map(|p| sk.point(*p).unwrap()).collect();
    let (_, grab, _) = sk.curve(s).unwrap().closest(mm(46.0, 16.0));
    let to = grab + mm(0.0, 10.0);
    let r = sk.drag(&[(DragHandle::Entity { id: s, grab }, to)]);
    assert!(r.is_solved(), "{r:?}");
    let after: Vec<DVec2> = refs.iter().map(|p| sk.point(*p).unwrap()).collect();
    assert!((after[0] - before[0]).length() < 1e-12, "the held start stays");
    assert!(after.iter().zip(&before).skip(1).all(|(a, b)| (*a - *b).length() > 1e-5), "every free point and handle moves: {before:?} -> {after:?}");
    assert!(sk.curve(s).unwrap().closest(to).2 < 5e-4, "the curve follows the pointer");
    // A Bezier: the control points' moves follow their Bernstein weights at the grab, x untouched.
    let mut sk = Sketch::new();
    let b = sk.add_bspline(&[mm(0.0, -30.0), mm(20.0, 0.0), mm(40.0, -30.0), mm(60.0, -10.0)], 3).unwrap();
    let ctrl = sk.entity(b).unwrap().point_refs().to_vec();
    solved(&mut sk);
    let before: Vec<DVec2> = ctrl.iter().map(|p| sk.point(*p).unwrap()).collect();
    let (u, grab, _) = sk.curve(b).unwrap().closest(mm(28.5, -16.0));
    sk.drag(&[(DragHandle::Entity { id: b, grab }, grab + mm(0.0, 10.0))]);
    let d: Vec<DVec2> = ctrl.iter().zip(&before).map(|(p, q)| sk.point(*p).unwrap() - *q).collect();
    assert!(d.iter().all(|d| d.x.abs() < 1e-9 && d.y > 1e-4), "{d:?}");
    let w = [(1.0 - u).powi(3), 3.0 * u * (1.0 - u).powi(2), 3.0 * u * u * (1.0 - u), u.powi(3)];
    let near = |i: usize| ((d[i].y / d[3].y) / (w[i] / w[3]) - 1.0).abs() < 0.02;
    assert!(near(0) && near(1) && near(2), "{d:?} {w:?}");
}
