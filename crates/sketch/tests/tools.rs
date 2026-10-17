use noshape_sketch::*;
use std::f64::consts::PI;

fn v(x: f64, y: f64) -> DVec2 {
    DVec2::new(x, y)
}

fn close(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol
}

#[test]
fn trim_line_end_and_middle() {
    let mut sk = Sketch::new();
    let h = sk.add_line(v(0.0, 0.0), v(10.0, 0.0));
    let v1 = sk.add_line(v(3.0, -1.0), v(3.0, 1.0));
    let v2 = sk.add_line(v(7.0, -1.0), v(7.0, 1.0));
    // trim the left end
    let r = sk.trim(h, v(1.0, 0.0)).unwrap();
    assert!(r.modified.contains(&h));
    let c = sk.curve(h).unwrap();
    assert!((c.start_point() - v(3.0, 0.0)).length() < 1e-9, "{c:?}");
    assert!((c.end_point() - v(10.0, 0.0)).length() < 1e-9);
    // trim the middle piece (between the vertical lines)
    let mut sk2 = Sketch::new();
    let h2 = sk2.add_line(v(0.0, 0.0), v(10.0, 0.0));
    sk2.add_line(v(3.0, -1.0), v(3.0, 1.0));
    sk2.add_line(v(7.0, -1.0), v(7.0, 1.0));
    let r = sk2.trim(h2, v(5.0, 0.0)).unwrap();
    assert_eq!(r.created.iter().filter(|e| sk2.kind(**e) == Some(EntityKind::Line)).count(), 1);
    let a = sk2.curve(h2).unwrap();
    assert!((a.end_point() - v(3.0, 0.0)).length() < 1e-9);
    let rep = sk2.solve();
    assert!(rep.is_solved(), "{rep:?}");
    // the endpoints stay on the cutting lines when those move
    let mut sk3 = sk.clone();
    let (s, _) = sk3.endpoints(v1).unwrap();
    sk3.solve();
    let rep = sk3.drag(&[(DragHandle::Entity { id: v1, grab: v(3.0, 0.5) }, v(4.0, 0.5))]);
    assert!(rep.is_solved(), "{rep:?}");
    let hs = sk3.curve(h).unwrap().start_point();
    assert!(sk3.curve(v1).unwrap().distance(hs) < 1e-8);
    let _ = (s, v2);
}

#[test]
fn trim_circle_to_arc_and_delete_isolated() {
    let mut sk = Sketch::new();
    let c = sk.add_circle(v(0.0, 0.0), 1.0);
    sk.add_line(v(-2.0, 0.0), v(2.0, 0.0));
    sk.trim(c, v(0.0, -1.0)).unwrap();
    match sk.curve(c).unwrap() {
        Curve2::Arc { start_angle, end_angle, .. } => {
            assert!(close(end_angle - start_angle, PI, 1e-9));
            let mid = sk.curve(c).unwrap().point(0.5 * (start_angle + end_angle));
            assert!(mid.y > 0.9);
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(sk.regions().len(), 1);
    let lone = sk.add_line(v(5.0, 5.0), v(6.0, 6.0));
    let r = sk.trim(lone, v(5.5, 5.5)).unwrap();
    assert!(r.removed.contains(&lone));
    assert!(sk.entity(lone).is_none());
}

#[test]
fn split_and_extend() {
    let mut sk = Sketch::new();
    let l = sk.add_line(v(0.0, 0.0), v(4.0, 0.0));
    let r = sk.split(l, v(1.0, 0.3)).unwrap();
    let new_line = *r.created.iter().find(|e| sk.kind(**e) == Some(EntityKind::Line)).unwrap();
    assert!((sk.curve(l).unwrap().end_point() - v(1.0, 0.0)).length() < 1e-12);
    assert!((sk.curve(new_line).unwrap().start_point() - v(1.0, 0.0)).length() < 1e-12);
    assert!(sk.solve().is_solved());

    let mut sk = Sketch::new();
    let l = sk.add_line(v(0.0, 0.0), v(2.0, 0.0));
    let wall = sk.add_line(v(5.0, -1.0), v(5.0, 1.0));
    sk.extend(l, v(1.9, 0.0)).unwrap();
    assert!((sk.curve(l).unwrap().end_point() - v(5.0, 0.0)).length() < 1e-9);
    let rep = sk.solve();
    assert!(rep.is_solved());
    // extended endpoint is constrained onto the wall
    let r = sk.drag(&[(DragHandle::Entity { id: wall, grab: v(5.0, 0.5) }, v(6.0, 0.5))]);
    assert!(r.is_solved());
    assert!(sk.curve(wall).unwrap().distance(sk.curve(l).unwrap().end_point()) < 1e-8);

    // arc extend
    let mut sk = Sketch::new();
    let a = sk.add_arc(v(0.0, 0.0), v(1.0, 0.0), v(0.0, 1.0));
    sk.add_line(v(-2.0, 0.0), v(-0.5, 0.0));
    sk.add_line(v(-0.5, 0.0), v(-2.0, 0.0001));
    sk.add_line(v(-2.0, -0.2), v(2.0, -0.2));
    let _ = sk.extend(a, v(0.0, 1.0));
    let c = sk.curve(a).unwrap();
    assert!(c.end_point().x < -0.99, "{c:?}");
}

#[test]
fn fillet_lines() {
    let mut sk = Sketch::new();
    let r = sk.corner_rectangle(v(0.0, 0.0), v(4.0, 3.0));
    // corner point 2 (top right) joins line 1 (right) and line 2 (top)
    let res = sk.fillet(r.curves[1], r.curves[2], 0.5).unwrap();
    let arc = *res.created.iter().find(|e| sk.kind(**e) == Some(EntityKind::Arc)).unwrap();
    match sk.curve(arc).unwrap() {
        Curve2::Arc { center, radius, start_angle, end_angle } => {
            assert!((center - v(3.5, 2.5)).length() < 1e-9, "{center}");
            assert!(close(radius, 0.5, 1e-12));
            assert!(close(end_angle - start_angle, PI / 2.0, 1e-9));
        }
        _ => panic!(),
    }
    let rep = sk.solve();
    assert!(rep.is_solved(), "{rep:?}");
    let regions = sk.regions();
    assert_eq!(regions.len(), 1);
    assert!(close(regions[0].area(), 12.0 - (1.0 - PI / 4.0) * 0.25, 1e-9));
    // fillet survives a resize: tangency is kept
    let w = sk.add_constraint(ConstraintKind::Length { curve: r.curves[0], value: 5.0 }).unwrap();
    let rep = sk.solve();
    assert!(rep.is_solved(), "{rep:?}");
    let _ = w;
    let regions = sk.regions();
    assert!(close(regions[0].area(), 15.0 - (1.0 - PI / 4.0) * 0.25, 1e-7), "{}", regions[0].area());

    // fillet at a corner point of two lines at 60 degrees
    let mut sk = Sketch::new();
    let p = sk.add_point(v(0.0, 0.0));
    let a = sk.add_point(v(3.0, 0.0));
    let b = sk.add_point(v(1.5, 1.5 * 3f64.sqrt()));
    sk.add_line_between(p, a).unwrap();
    sk.add_line_between(p, b).unwrap();
    let res = sk.fillet_corner(p, 0.4).unwrap();
    assert!(sk.entity(p).is_none());
    let arc = *res.created.iter().find(|e| sk.kind(**e) == Some(EntityKind::Arc)).unwrap();
    if let Curve2::Arc { center, .. } = sk.curve(arc).unwrap() {
        assert!(close(center.y, 0.4, 1e-9));
        assert!(close(center.x, 0.4 * 3f64.sqrt(), 1e-9));
    }
    assert!(sk.solve().is_solved());
}

#[test]
fn offset_rectangle_outward_and_inward() {
    let mut sk = Sketch::new();
    let r = sk.corner_rectangle(v(0.0, 0.0), v(2.0, 2.0));
    // rectangle loop is CCW: left of each edge = inside -> negative = outward
    let res = sk.offset(&r.curves, -0.5).unwrap();
    assert!(!res.created.is_empty());
    let rep = sk.solve();
    assert!(rep.is_solved(), "{rep:?}");
    let regions = sk.regions();
    let mut areas: Vec<f64> = regions.iter().map(|x| x.area()).collect();
    areas.sort_by(f64::total_cmp);
    assert_eq!(areas.len(), 2, "{areas:?}");
    assert!(close(areas[0], 4.0, 1e-9));
    assert!(close(areas[1], 4.0 + 4.0 * 2.0 * 0.5 + PI * 0.25 - 4.0, 1e-9), "{areas:?}");

    let mut sk = Sketch::new();
    let r = sk.corner_rectangle(v(0.0, 0.0), v(2.0, 2.0));
    sk.offset(&r.curves, 0.5).unwrap();
    assert!(sk.solve().is_solved());
    let regions = sk.regions();
    let mut areas: Vec<f64> = regions.iter().map(|x| x.area()).collect();
    areas.sort_by(f64::total_cmp);
    assert_eq!(areas.len(), 2, "{areas:?}");
    assert!(close(areas[0], 1.0, 1e-9), "{areas:?}");

    // circle and open polyline
    let mut sk = Sketch::new();
    let c = sk.add_circle(v(0.0, 0.0), 1.0);
    sk.offset(&[c], -0.25).unwrap();
    assert_eq!(sk.regions().len(), 2);
    let mut sk = Sketch::new();
    let p0 = sk.add_point(v(0.0, 0.0));
    let p1 = sk.add_point(v(2.0, 0.0));
    let p2 = sk.add_point(v(2.0, 2.0));
    let l0 = sk.add_line_between(p0, p1).unwrap();
    let l1 = sk.add_line_between(p1, p2).unwrap();
    let res = sk.offset(&[l0, l1], 0.3).unwrap();
    let lines = res.created.iter().filter(|e| sk.kind(**e) == Some(EntityKind::Line)).count();
    assert_eq!(lines, 2);
    assert!(sk.solve().is_solved());
}

#[test]
fn mirror_with_symmetric_constraints() {
    let mut sk = Sketch::new();
    let axis = sk.add_line(v(0.0, -5.0), v(0.0, 5.0));
    sk.set_construction(axis, true).unwrap();
    sk.add_constraint(ConstraintKind::Vertical { a: axis, b: None }).unwrap();
    sk.add_constraint(ConstraintKind::Fix { entity: axis }).unwrap();
    let l = sk.add_line(v(1.0, 0.0), v(3.0, 1.0));
    let c = sk.add_circle(v(2.0, 3.0), 0.5);
    let a = sk.add_arc(v(2.0, -2.0), v(3.0, -2.0), v(2.0, -1.0));
    let res = sk.mirror(&[l, c, a], axis).unwrap();
    assert!(res.constraints_added.len() >= 7);
    let rep = sk.solve();
    assert!(rep.is_solved(), "{rep:?}");
    let (s, _) = sk.endpoints(l).unwrap();
    sk.drag(&[(DragHandle::Point(s), v(1.5, 0.5))]);
    let mirrored_line = res.created.iter().copied().find(|e| sk.kind(*e) == Some(EntityKind::Line)).unwrap();
    let (ms, _) = sk.endpoints(mirrored_line).unwrap();
    assert!((sk.point(ms).unwrap() - v(-1.5, 0.5)).length() < 1e-8);
    let ma = res.created.iter().copied().find(|e| sk.kind(*e) == Some(EntityKind::Arc)).unwrap();
    match sk.curve(ma).unwrap() {
        Curve2::Arc { center, start_angle, end_angle, .. } => {
            assert!((center - v(-2.0, -2.0)).length() < 1e-9);
            assert!(close(end_angle - start_angle, PI / 2.0, 1e-9));
        }
        _ => panic!(),
    }
}

#[test]
fn patterns_transform_and_external() {
    let mut sk = Sketch::new();
    let r = sk.corner_rectangle(v(0.0, 0.0), v(1.0, 1.0));
    let res = sk.linear_pattern(&r.curves, v(2.0, 0.0), 3).unwrap();
    assert_eq!(res.created.iter().filter(|e| sk.kind(**e) == Some(EntityKind::Line)).count(), 8);
    assert_eq!(sk.regions().len(), 3);
    let c = sk.add_circle(v(5.0, 0.0), 0.2);
    let res = sk.circular_pattern(&[c], v(0.0, 0.0), 6, 2.0 * PI).unwrap();
    assert_eq!(res.created.iter().filter(|e| sk.kind(**e) == Some(EntityKind::Circle)).count(), 5);

    let mut sk = Sketch::new();
    let r = sk.corner_rectangle(v(0.0, 0.0), v(2.0, 1.0));
    sk.add_constraint(ConstraintKind::Fix { entity: r.points[0] }).unwrap();
    sk.transform(&r.curves, Transform::Translate(v(1.0, 1.0))).unwrap();
    let rep = sk.solve();
    assert!(rep.is_solved());
    assert!((sk.point(r.points[0]).unwrap() - v(1.0, 1.0)).length() < 1e-12);
    sk.transform(&r.curves, Transform::Scale { center: v(1.0, 1.0), factor: 2.0 }).unwrap();
    assert!((sk.point(r.points[2]).unwrap() - v(5.0, 3.0)).length() < 1e-12);

    let mut sk = Sketch::new();
    let res = sk.use_external(&Curve2::Line { a: v(0.0, 0.0), b: v(4.0, 0.0) }).unwrap();
    let ext = *res.created.last().unwrap();
    let p = sk.add_point(v(1.0, 0.5));
    sk.add_constraint(ConstraintKind::Pierce { point: p, target: ext }).unwrap();
    let ep = sk.use_external_point(v(3.0, 3.0));
    let q = sk.add_point(v(2.5, 2.0));
    sk.add_constraint(ConstraintKind::Pierce { point: q, target: ep }).unwrap();
    let rep = sk.solve();
    assert!(rep.is_solved(), "{rep:?}");
    assert!(sk.point(p).unwrap().y.abs() < 1e-10);
    assert!((sk.point(q).unwrap() - v(3.0, 3.0)).length() < 1e-12);
    assert_eq!(rep.state(ext), Some(ConstraintState::FullyConstrained));
    assert_eq!(rep.state(q), Some(ConstraintState::FullyConstrained));
    assert_eq!(rep.state(p), Some(ConstraintState::UnderConstrained));
    sk.drag(&[(DragHandle::Entity { id: ext, grab: v(1.0, 0.0) }, v(1.0, 1.0))]);
    assert!(sk.curve(ext).unwrap().start_point().length() < 1e-12);
    // a non-external point cannot pierce
    assert!(sk.add_constraint(ConstraintKind::Pierce { point: q, target: p }).is_err());
}
