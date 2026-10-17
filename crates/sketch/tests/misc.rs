use noshape_sketch::*;
use std::f64::consts::PI;

fn v(x: f64, y: f64) -> DVec2 {
    DVec2::new(x, y)
}

#[test]
fn sketch_is_send_sync() {
    fn assert_send_sync<T: Send + Sync + Clone>() {}
    assert_send_sync::<Sketch>();
    assert_send_sync::<SolveReport>();
    assert_send_sync::<Region>();
}

#[test]
fn eval_helpers() {
    let mut sk = Sketch::new();
    let c = sk.add_circle(v(0.0, 0.0), 1.0);
    let l = sk.add_line(v(3.0, 0.0), v(5.0, 0.0));
    let pts = sk.tessellate(c, 1e-3);
    assert!(pts.len() > 30);
    for p in &pts {
        assert!((p.length() - 1.0).abs() < 1e-12);
    }
    let (q, d) = sk.closest_point(c, v(2.0, 0.0)).unwrap();
    assert!((q - v(1.0, 0.0)).length() < 1e-12 && (d - 1.0).abs() < 1e-12);
    assert!((sk.distance_to(l, v(4.0, 1.0)).unwrap() - 1.0).abs() < 1e-12);
    let bb = sk.entity_bbox(c).unwrap();
    assert!((bb.max - v(1.0, 1.0)).length() < 1e-12);
    // picking prefers points
    let (s, _) = sk.endpoints(l).unwrap();
    assert_eq!(sk.pick(v(3.01, 0.01), 0.05), Some(s));
    assert_eq!(sk.pick(v(4.0, 0.01), 0.05), Some(l));
    assert_eq!(sk.pick(v(10.0, 10.0), 0.05), None);
    let spl = sk.add_bspline(&[v(0.0, 3.0), v(1.0, 4.0), v(2.0, 3.0), v(3.0, 4.0)], 3).unwrap();
    let t = sk.tessellate(spl, 1e-4);
    let cv = sk.curve(spl).unwrap();
    for w in t.windows(2) {
        let m = (w[0] + w[1]) * 0.5;
        assert!(cv.distance(m) < 2e-4);
    }
}

#[test]
fn trim_and_split_curves_of_every_kind() {
    // spline crossing a line: trim one side
    let mut sk = Sketch::new();
    let s = sk.add_bspline(&[v(0.0, 0.0), v(1.0, 2.0), v(2.0, -2.0), v(3.0, 0.0)], 3).unwrap();
    sk.add_line(v(1.5, -3.0), v(1.5, 3.0));
    let before = sk.curve(s).unwrap();
    sk.trim(s, v(2.8, -0.2)).unwrap();
    let after = sk.curve(s).unwrap();
    assert!((after.start_point() - before.start_point()).length() < 1e-12);
    assert!((after.end_point().x - 1.5).abs() < 1e-9);
    // remaining part coincides with the original curve
    for i in 0..=10 {
        let p = after.point(after.domain().0 + (after.domain().1 - after.domain().0) * i as f64 / 10.0);
        assert!(before.distance(p) < 1e-9);
    }
    assert!(sk.solve().is_solved());

    // ellipse cut by a line becomes an elliptical arc
    let mut sk = Sketch::new();
    let e = sk.add_ellipse(v(0.0, 0.0), v(2.0, 0.0), 1.0);
    sk.add_line(v(-3.0, 0.0), v(3.0, 0.0));
    sk.trim(e, v(0.0, -1.0)).unwrap();
    assert_eq!(sk.kind(e), Some(EntityKind::EllipticalArc));
    let regs = sk.regions();
    assert_eq!(regs.len(), 1);
    assert!((regs[0].area() - PI).abs() < 1e-8);
    assert!(sk.solve().is_solved());

    // split an arc: the pieces share a center and have equal radius
    let mut sk = Sketch::new();
    let a = sk.add_arc(v(0.0, 0.0), v(1.0, 0.0), v(-1.0, 0.0));
    let res = sk.split(a, v(0.0, 2.0)).unwrap();
    let b = *res.created.iter().find(|x| sk.kind(**x) == Some(EntityKind::Arc)).unwrap();
    assert_eq!(sk.entity(a).unwrap().center(), sk.entity(b).unwrap().center());
    let r = sk.solve();
    assert!(r.is_solved());
    let rr = sk.add_constraint(ConstraintKind::Radius { curve: a, value: 2.0 }).unwrap();
    assert!(sk.solve().is_solved());
    let _ = rr;
    match sk.curve(b).unwrap() {
        Curve2::Arc { radius, .. } => assert!((radius - 2.0).abs() < 1e-9),
        _ => panic!(),
    }

    // conic trim keeps a conic
    let mut sk = Sketch::new();
    let k = sk.add_conic(v(0.0, 0.0), v(1.0, 2.0), v(2.0, 0.0), 0.6).unwrap();
    let before = sk.curve(k).unwrap();
    sk.add_line(v(1.0, -1.0), v(1.0, 3.0));
    sk.trim(k, v(1.8, 0.3)).unwrap();
    assert_eq!(sk.kind(k), Some(EntityKind::Conic));
    let after = sk.curve(k).unwrap();
    for i in 0..=10 {
        assert!(before.distance(after.point(i as f64 / 10.0)) < 1e-9);
    }
}

#[test]
fn fillet_line_and_arc() {
    let mut sk = Sketch::new();
    let p = sk.add_point(v(1.0, 0.0));
    let a = sk.add_point(v(-2.0, 0.0));
    let c = sk.add_point(v(0.0, 0.0));
    let e = sk.add_point(v(0.0, 1.0));
    let line = sk.add_line_between(a, p).unwrap();
    // arc from (1,0) to (0,1) around origin; line ends at the arc start
    let arc = sk.add_arc_between(c, p, e).unwrap();
    // move the line so it meets the arc at an angle
    sk.set_point(a, v(1.0, -2.0)).unwrap();
    let res = sk.fillet(line, arc, 0.2).unwrap();
    let f = *res.created.iter().find(|x| sk.kind(**x) == Some(EntityKind::Arc)).unwrap();
    let r = sk.solve();
    assert!(r.is_solved(), "{r:?}");
    let Curve2::Arc { center, radius, .. } = sk.curve(f).unwrap() else { panic!() };
    assert!((radius - 0.2).abs() < 1e-9);
    // fillet circle is tangent to the line (x = 1) and internally tangent to the unit circle
    assert!(((center.x - 1.0).abs() - 0.2).abs() < 1e-9);
    assert!((center.length() - 0.8).abs() < 1e-9 || (center.length() - 1.2).abs() < 1e-9);
}

#[test]
fn wires_with_branches() {
    let mut sk = Sketch::new();
    let a = sk.add_point(v(0.0, 0.0));
    let b = sk.add_point(v(1.0, 0.0));
    let c = sk.add_point(v(2.0, 0.0));
    let d = sk.add_point(v(1.0, 1.0));
    sk.add_line_between(a, b).unwrap();
    sk.add_line_between(b, c).unwrap();
    sk.add_line_between(b, d).unwrap();
    let w = sk.wires();
    // branch point splits into three open wires
    assert_eq!(w.len(), 3);
    assert!(w.iter().all(|x| !x.closed && x.segments.len() == 1));
    let mut sk = Sketch::new();
    let r = sk.corner_rectangle(v(0.0, 0.0), v(1.0, 1.0));
    let ws = sk.wires();
    assert_eq!(ws.len(), 1);
    assert!(ws[0].closed);
    assert_eq!(ws[0].segments.len(), 4);
    let _ = r;
}

#[test]
fn region_profile_segments_are_consistent() {
    let mut sk = Sketch::new();
    sk.straight_slot(v(0.0, 0.0), v(3.0, 1.0), 1.0).unwrap();
    sk.add_circle(v(1.5, 0.5), 0.2);
    sk.add_ellipse(v(5.0, 5.0), v(1.0, 1.0), 0.5);
    sk.add_conic(v(8.0, 0.0), v(9.0, 2.0), v(10.0, 0.0), 0.5).unwrap();
    sk.add_line(v(8.0, 0.0), v(10.0, 0.0));
    for reg in sk.regions() {
        for lp in std::iter::once(&reg.outer).chain(reg.holes.iter()) {
            let n = lp.segments.len();
            assert_eq!(lp.sources.len(), n);
            for k in 0..n {
                let e = lp.segments[k].end();
                let s = lp.segments[(k + 1) % n].start();
                assert!((e - s).length() < 1e-7, "gap {e} {s}");
            }
        }
        assert!(reg.outer.signed_area() > 0.0);
        for h in &reg.holes {
            assert!(h.signed_area() < 0.0);
        }
        assert!(reg.area() > 0.0);
        assert!(reg.contains(reg.interior_point()));
        // serde
        let s = serde_json::to_string(&reg).unwrap();
        let back: Region = serde_json::from_str(&s).unwrap();
        assert_eq!(back, reg);
    }
    // parabola segment area = 2/3 * base * height (height of the curve = 1)
    let regs = sk.regions();
    assert!(regs.iter().any(|r| (r.area() - 2.0 / 3.0 * 2.0 * 1.0).abs() < 1e-9));
}
