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
    // Live: the pieces are Coincident curve to curve.
    let piece = r.created.iter().copied().find(|e| sk2.kind(*e) == Some(EntityKind::Line)).unwrap();
    assert!(sk2.constraints().any(|c| c.kind == ConstraintKind::Coincident { a: h2, b: piece }));
    // an arc trimmed in its middle: Coincident pieces on one centre, nothing redundant
    let mut sk4 = Sketch::new();
    let arc = sk4.add_arc(v(0.0, 0.0), v(5.0, 0.0), v(-5.0, 0.0));
    sk4.add_line(v(2.0, 0.0), v(2.0, 6.0));
    sk4.add_line(v(-2.0, 0.0), v(-2.0, 6.0));
    let before = sk4.solve().dof;
    let r = sk4.trim(arc, v(0.0, 5.0)).unwrap();
    let piece = r.created.iter().copied().find(|e| sk4.kind(*e) == Some(EntityKind::Arc)).unwrap();
    assert!(sk4.constraints().any(|c| c.kind == ConstraintKind::Coincident { a: arc, b: piece }));
    let rep = sk4.solve();
    assert!(rep.is_solved(), "{rep:?}");
    assert_eq!(rep.dof, before, "the pieces keep the arc's freedom");
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

/// Live sketch-edit-feel-1122 sk3: a circle at (14, 3) R 6 on the line (0, 0) → (40, 0) crossing it at 8.8 and 19.2;
/// Trim's hover lights the piece under the pointer between those crossings (the line's ends past them).
#[test]
fn trim_hover_pieces_of_a_circle_on_a_line_end_at_the_crossings() {
    let mut sk = Sketch::new();
    let l = sk.add_line(v(0.0, 0.0), v(40.0, 0.0));
    let c = sk.add_circle(v(14.0, 3.0), 6.0);
    let x = 27.0_f64.sqrt();
    let ends = |id, at| {
        let (t0, t1) = sk.trim_span(id, at).unwrap();
        let curve = sk.curve(id).unwrap();
        let (a, b) = curve.domain();
        let p = |t: f64| curve.point(if curve.is_periodic() && t > b { t - (b - a) } else { t });
        let (p0, p1) = (p(t0), p(t1));
        if p0.x <= p1.x { (p0, p1) } else { (p1, p0) }
    };
    let near = |(p, q): (DVec2, DVec2), (a, b): (DVec2, DVec2)| (p - a).length() < 1e-6 && (q - b).length() < 1e-6;
    let (left, right) = (v(14.0 - x, 0.0), v(14.0 + x, 0.0));
    assert!(near(ends(c, v(14.0, -2.9)), (left, right)), "the lower arc");
    assert!(near(ends(c, v(14.0, 9.0)), (left, right)), "the upper arc");
    assert!(near(ends(l, v(14.0, 0.0)), (left, right)), "the chord");
    assert!(near(ends(l, v(4.0, 0.0)), (v(0.0, 0.0), left)), "left of the circle");
    assert!(near(ends(l, v(30.0, 0.0)), (right, v(40.0, 0.0))), "right of it");
    // A click on the upper arc (live S4 retry, trim-upper-arc-reach-and-click.png): it goes, the lower arc stays between
    // the crossings, each end coincident with the line; the line whole.
    sk.trim(c, v(14.0, 9.0)).unwrap();
    let (a, b) = sk.endpoints(c).expect("an arc");
    let mut got = [sk.point(a).unwrap(), sk.point(b).unwrap()];
    got.sort_by(|p, q| p.x.total_cmp(&q.x));
    assert!(near((got[0], got[1]), (left, right)), "{got:?}");
    let arc = sk.curve(c).unwrap();
    assert!(arc.point(0.5 * (arc.domain().0 + arc.domain().1)).y < 0.0, "the lower arc kept");
    let line = sk.curve(l).unwrap();
    assert!((line.point(line.domain().0) - v(0.0, 0.0)).length() < 1e-9 && (line.point(line.domain().1) - v(40.0, 0.0)).length() < 1e-9, "the line whole");
    for p in [a, b] {
        assert!(sk.constraints().any(|k| matches!(k.kind, ConstraintKind::Coincident { a: x, b: y } if (x == p && y == l) || (x == l && y == p))), "end {p:?} coincident with the line");
    }
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
    sk.extend(l, wall).unwrap();
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
    let to = sk.add_line(v(-2.0, 0.0), v(-1.5, 0.0));
    sk.add_line(v(-2.0, -0.2), v(2.0, -0.2));
    sk.extend(a, to).unwrap();
    let c = sk.curve(a).unwrap();
    assert!((c.end_point() - v(-1.0, 0.0)).length() < 1e-9, "{c:?}");

    // Live (audit 1969): the boundary counts as its infinite line, and the
    // end that reaches it moves.
    let mut sk = Sketch::new();
    let l = sk.add_line(v(0.0, 0.06), v(0.02, 0.06));
    let short = sk.add_line(v(0.05, 0.03), v(0.05, 0.05));
    sk.extend(l, short).unwrap();
    let c = sk.curve(l).unwrap();
    assert!((c.start_point() - v(0.0, 0.06)).length() < 1e-12);
    assert!((c.end_point() - v(0.05, 0.06)).length() < 1e-9, "{c:?}");
    assert!(sk.solve().is_solved());
    let left = sk.add_line(v(-0.01, -0.01), v(-0.01, 0.01));
    sk.extend(l, left).unwrap();
    let c = sk.curve(l).unwrap();
    assert!((c.start_point() - v(-0.01, 0.06)).length() < 1e-9, "the start reaches the left boundary: {c:?}");
}

/// A side of twice the fillet radius, filleted at both ends: live keeps it at zero length with its direction, which its
/// Tangents to both arcs still use (c2-fan-blade-sweep's Sketch 2), through a resize too.
#[test]
fn a_side_two_fillets_consume_keeps_its_direction() {
    let mut sk = Sketch::new();
    let r = sk.corner_rectangle(v(0.0, 0.0), v(4.0, 2.0));
    sk.fillet(r.curves[1], r.curves[2], 1.0).unwrap();
    sk.fillet(r.curves[0], r.curves[1], 1.0).unwrap();
    let held = sk.entity(r.curves[1]).and_then(|e| e.held_direction);
    assert!(held.is_some_and(|d| d.x.abs() < 1e-9 && (d.y.abs() - 1.0).abs() < 1e-9), "{held:?}");
    let rep = sk.solve();
    assert!(rep.is_solved(), "{rep:?}");
    let cut = 2.0 * (1.0 - PI / 4.0);
    let area = |sk: &Sketch| sk.regions().first().map_or(0.0, |r| r.area());
    assert!(close(area(&sk), 8.0 - cut, 1e-9), "{}", area(&sk));
    sk.add_constraint(ConstraintKind::Length { curve: r.curves[3], value: 3.0 }).unwrap();
    let rep = sk.solve();
    assert!(rep.is_solved(), "{rep:?}");
    // Taller: the side grows back along its direction between the two arcs.
    assert!(close(area(&sk), 12.0 - cut, 1e-6), "{}", area(&sk));
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
    assert!(sk.entity(p).is_some(), "the corner stays as the visual sharp (live)");
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
    // Live (qa4 OFF-CORNER): the copies' lines meet at sharp corners.
    assert!(close(areas[1], 9.0 - 4.0, 1e-9), "{areas:?}");

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
    // Live stores a mirror copy of a circle or an arc clockwise (core q-arc-sense-1120), and a copy of that back.
    let mc = res.created.iter().copied().find(|e| sk.kind(*e) == Some(EntityKind::Circle)).unwrap();
    assert!(sk.entity(ma).unwrap().clockwise && sk.entity(mc).unwrap().clockwise && !sk.entity(a).unwrap().clockwise);
    let axis2 = sk.add_line(v(10.0, -5.0), v(10.0, 5.0));
    let back = sk.mirror(&[ma], axis2).unwrap();
    let again = back.created.iter().copied().find(|e| sk.kind(*e) == Some(EntityKind::Arc)).unwrap();
    assert!(!sk.entity(again).unwrap().clockwise);
}

/// Live's centre-point arc swept clockwise is stored clockwise (core q-arc-sense-1120: its edges run start to end
/// clockwise); swept counter-clockwise, or a three-point arc whatever the click order, not.
#[test]
fn a_centre_point_arc_swept_clockwise_is_stored_clockwise() {
    let mut sk = Sketch::new();
    let cw = sk.center_point_arc(v(0.0, 0.0), v(0.0, 1.0), v(1.0, 0.0), false);
    let ccw = sk.center_point_arc(v(5.0, 0.0), v(6.0, 0.0), v(5.0, 1.0), true);
    let three = sk.three_point_arc(v(0.0, -10.0), v(2.0, -11.0), v(0.0, -12.0)).unwrap();
    assert!(sk.entity(cw).unwrap().clockwise);
    assert!(!sk.entity(ccw).unwrap().clockwise && !sk.entity(three).unwrap().clockwise);
    let json = serde_json::to_string(sk.entity(ccw).unwrap()).unwrap();
    assert!(!json.contains("clockwise"), "{json}");
}

/// Live's sketch Mirror (live/feel/mirror.json): one mirror constraint per copy (a line's, an arc's, a circle's,
/// a point's), a segment's ends one each, a circle's or an arc's centre none of its own; the source's own
/// constraints not copied; mirroring again makes a second copy (sketch-tools-1123). The sketch solves.
#[test]
fn mirror_constrains_each_copy_as_live() {
    let mut sk = Sketch::new();
    let axis = sk.add_line(v(-5.0, -10.0), v(-5.0, 10.0));
    let l = sk.add_line(v(0.0, 0.0), v(4.0, 0.0));
    sk.add_constraint(ConstraintKind::Horizontal { a: l, b: None }).unwrap();
    let a = sk.add_arc(v(2.0, -3.0), v(3.0, -3.0), v(2.0, -2.0));
    let p = sk.add_point(v(1.0, 5.0));
    let c = sk.add_circle(v(3.0, 6.0), 0.5);
    let before = sk.constraints().count();
    let res = sk.mirror(&[l, a, p, c], axis).unwrap();
    let sym = |x: EntityId| sk.constraints().filter(|k| matches!(k.kind, ConstraintKind::Symmetric { a, b, .. } if a == x || b == x)).count();
    let copy = |k: EntityKind| res.created.iter().copied().find(|e| sk.kind(*e) == Some(k)).unwrap();
    let (line, arc, circle) = (copy(EntityKind::Line), copy(EntityKind::Arc), copy(EntityKind::Circle));
    assert_eq!((sym(line), sym(arc), sym(circle)), (1, 1, 1), "the curve pairs");
    let (s, e) = sk.endpoints(line).unwrap();
    assert_eq!((sym(s), sym(e)), (1, 1), "a line's ends");
    let arc_centre = sk.entity(arc).unwrap().center().unwrap();
    let circle_centre = sk.entity(circle).unwrap().center().unwrap();
    assert_eq!((sym(arc_centre), sym(circle_centre)), (0, 0), "centres through their curves");
    let copied = |k: &ConstraintKind| match k {
        ConstraintKind::Equal { .. } => true,
        ConstraintKind::Horizontal { a, .. } => *a == line,
        _ => false,
    };
    assert!(!sk.constraints().any(|k| copied(&k.kind)), "no EQUAL, the source's HORIZONTAL not copied");
    // line 1 + 2 ends, arc 1 + 2 ends, point 1, circle 1.
    assert_eq!(sk.constraints().count() - before, 8);
    let rep = sk.solve();
    assert!(rep.is_solved(), "{rep:?}");
    // Again: a second, coincident copy of each with its own mirror constraints (live sketch-tools-1123).
    let again = sk.mirror(&[l, a, p, c], axis).unwrap();
    assert_eq!((again.created.iter().filter(|e| sk.kind(**e) != Some(EntityKind::Point)).count(), again.constraints_added.len()), (3, 8), "{again:?}");
    assert!(sk.solve().is_solved());
}

/// Live workflows-1122 22 s35: a line ending on the mirror line gets a copy with its own end there, three SYMMETRIC
/// (the line and both ends) and no COINCIDENT; the sketch solves and the copy follows its source.
#[test]
fn mirror_copies_an_end_on_the_axis_too() {
    let mut sk = Sketch::new();
    let axis = sk.add_line(v(0.0, -10.0), v(0.0, 30.0));
    sk.add_constraint(ConstraintKind::Vertical { a: axis, b: None }).unwrap();
    let l = sk.add_line(v(0.0, 5.0), v(25.0, 5.0));
    let (s, e) = sk.endpoints(l).unwrap();
    sk.add_constraint(ConstraintKind::Coincident { a: s, b: axis }).unwrap();
    sk.add_constraint(ConstraintKind::Horizontal { a: l, b: None }).unwrap();
    let before = sk.constraints().count();
    let res = sk.mirror(&[l], axis).unwrap();
    let copy = res.created.iter().copied().find(|x| sk.kind(*x) == Some(EntityKind::Line)).unwrap();
    let (cs, ce) = sk.endpoints(copy).unwrap();
    assert!(cs != s && ce != e, "the copy has its own ends");
    let added: Vec<&ConstraintKind> = sk.constraints().skip(before).map(|c| &c.kind).collect();
    assert_eq!(added.len(), 3, "{added:?}");
    assert!(added.iter().all(|k| matches!(k, ConstraintKind::Symmetric { .. })), "{added:?}");
    let rep = sk.solve();
    assert!(rep.is_solved(), "{rep:?}");
    sk.drag(&[(DragHandle::Point(e), v(20.0, 5.0))]);
    let pe = sk.point(e).unwrap();
    assert!(pe.x < 24.0, "the source end moved: {pe:?}");
    // The axis is only vertical: the drag may slide it, so mirror about where it is.
    let (a0, _) = sk.endpoints(axis).unwrap();
    let ax = sk.point(a0).unwrap().x;
    assert!((sk.point(ce).unwrap() - v(2.0 * ax - pe.x, pe.y)).length() < 1e-6, "{:?} for {pe:?}, axis x {ax}", sk.point(ce));
    assert!((sk.point(cs).unwrap() - sk.point(s).unwrap()).length() < 1e-6, "{:?}", sk.point(cs));
}

/// Live's mirrored spline (live/feel/mirror.json, 17-18): one mirror constraint for the curve and one for each end;
/// its inner fit points and handles held through the curve's. Moving a source fit point moves the copy's.
#[test]
fn mirror_spline_as_one_curve_and_its_ends() {
    let mut sk = Sketch::new();
    let axis = sk.add_line(v(-5.0, -10.0), v(-5.0, 10.0));
    sk.add_constraint(ConstraintKind::Fix { entity: axis }).unwrap();
    let spline = sk.add_fit_spline_with_handles(&[v(0.0, 0.0), v(2.0, 3.0), v(4.0, 1.0), v(6.0, 4.0)]).unwrap();
    let before = sk.constraints().count();
    let res = sk.mirror(&[spline], axis).unwrap();
    let copy = res.created.iter().copied().find(|e| sk.kind(*e) == Some(EntityKind::FitSpline)).unwrap();
    let sym = |x: EntityId| sk.constraints().filter(|k| matches!(k.kind, ConstraintKind::Symmetric { a, b, .. } if a == x || b == x)).count();
    let Some(Geometry::FitSpline { points, handles: Some((h0, h1)) }) = sk.entity(copy).map(|e| e.geometry.clone()) else { panic!() };
    assert_eq!((sym(copy), sym(points[0]), sym(points[3])), (1, 1, 1), "the curve and its ends");
    assert_eq!((sym(points[1]), sym(points[2]), sym(h0), sym(h1)), (0, 0, 0, 0), "inner points through the curve");
    assert_eq!(sk.constraints().count() - before, 3);
    let Some(Geometry::FitSpline { points: src, .. }) = sk.entity(spline).map(|e| e.geometry.clone()) else { panic!() };
    sk.set_point(src[1], v(1.0, 5.0)).unwrap();
    sk.add_constraint(ConstraintKind::Fix { entity: src[1] }).unwrap();
    let rep = sk.solve();
    assert!(rep.is_solved(), "{rep:?}");
    assert!((sk.point(points[1]).unwrap() - v(-11.0, 5.0)).length() < 1e-9, "{:?}", sk.point(points[1]));
    for (p, q) in src.iter().zip(&points) {
        let (p, q) = (sk.point(*p).unwrap(), sk.point(*q).unwrap());
        assert!((v(-10.0 - p.x, p.y) - q).length() < 1e-9, "{p} {q}");
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

/// SK5 (live, qa5 sketch tools): 30.089 × 22.065 rectangle, top-right
/// fillet R5, then a new radius. The top line keeps its R5-trimmed length
/// (25.089) and the right edge follows the arc: width = 25.089 + R. R3:
/// bottom 28.089, right 19.065; R8: bottom 33.089, right 14.065.
#[test]
fn fillet_radius_change_moves_like_onshape() {
    for (radius, bottom_len, right_len) in [(0.003, 28.089, 19.065), (0.008, 33.089, 14.065)] {
        let mut sk = Sketch::new();
        let r = sk.corner_rectangle(v(0.0, 0.0), v(0.030089, 0.022065));
        let res = sk.fillet_corner(r.points[2], 0.005).unwrap();
        let arc = res.created.iter().copied().find(|e| sk.kind(*e) == Some(EntityKind::Arc)).unwrap();
        assert!(sk.solve().is_solved());
        let dim = sk.constraints().find(|c| matches!(c.kind, ConstraintKind::Radius { curve, .. } if curve == arc)).unwrap().id;
        sk.set_dimension_value(dim, radius).unwrap();
        let opts = sk.dimension_edit_options(dim);
        assert!(sk.solve_with(&opts).is_solved());
        let len = |l: EntityId| {
            let c = sk.curve(l).unwrap();
            (c.end_point() - c.start_point()).length() * 1000.0
        };
        let [bottom, right, top, left] = [r.curves[0], r.curves[1], r.curves[2], r.curves[3]];
        for (l, want) in [(top, 25.089), (bottom, bottom_len), (left, 22.065), (right, right_len)] {
            assert!(close(len(l), want, 1e-6), "R{radius}: {l:?}: {} vs {want}", len(l));
        }
        assert!((sk.point(r.points[0]).unwrap()).length() < 1e-12, "the far corner stays");
    }
}

/// A closed fit spline (last point = first) is periodic: smooth at the seam.
#[test]
fn closed_fit_spline_is_smooth_at_the_seam() {
    let mut sk = Sketch::new();
    let mut pts: Vec<DVec2> = (0..12)
        .map(|k| {
            let a = (60.0 + 30.0 * k as f64).to_radians();
            let r = if k % 2 == 0 { 0.0547 } else { 0.0775 };
            v(r * a.cos(), r * a.sin())
        })
        .collect();
    pts.push(pts[0]);
    let id = sk.add_closed_fit_spline(&pts).unwrap();
    let c = sk.curve(id).unwrap();
    let (a, b) = c.domain();
    assert!((c.point(a) - c.point(b)).length() < 1e-12);
    let (ta, tb) = (c.tangent(a).normalize(), c.tangent(b).normalize());
    assert!((ta - tb).length() < 1e-9, "seam tangents {ta} {tb}");
    // Six-fold symmetry: the lobe tip at 90 degrees has a horizontal tangent.
    let near_tip = (0..=200).map(|i| a + (b - a) * i as f64 / 200.0).map(|t| c.point(t)).fold(DVec2::ZERO, |m, p| if p.y > m.y { p } else { m });
    assert!(near_tip.x.abs() < 2e-3 && (near_tip.y - 0.0775).abs() < 1e-3, "{near_tip}");
}

/// Live 1777 / 1836 (SKETCH-SPLINE-CENTRIPETAL): a fit spline's knots are
/// centripetal (cumulative square roots of the chords: s202's p7 has
/// interior knots 0.3425, 0.6367), and a closed one is periodic (live's
/// closed spline extruded 10 mm: 19782.85 mm³, a 1978.285 mm² profile);
/// with no end derivatives the ends are natural (live 1861).
#[test]
fn fit_splines_are_centripetal_and_closed_ones_periodic() {
    let mm = |x: f64, y: f64| v(x * 1e-3, y * 1e-3);
    let pts = [mm(118.591988, -2.29809), mm(128.129138, 7.889517), mm(137.666288, 4.004219), mm(152.043988, -2.29809)];
    let nb = Nurbs::interpolate_with_ends(&pts, v(0.0146, 0.0615), v(0.0245, -0.0123)).unwrap();
    let inner: Vec<f64> = nb.knots.iter().copied().filter(|k| *k > 1e-9 && *k < 1.0 - 1e-9).collect();
    assert!(inner.len() == 2 && (inner[0] - 0.3425).abs() < 1e-4 && (inner[1] - 0.6367).abs() < 1e-4, "{inner:?}");
    // Live 1861: skFitSpline through 4 points, no derivatives: centripetal
    // knots, natural ends, these control points.
    let nb = Nurbs::interpolate(&[mm(0.0, 0.0), mm(10.0, 2.0), mm(12.0, 10.0), mm(25.0, 12.0)]).unwrap();
    assert_eq!(nb.degree, 3);
    let inner: Vec<f64> = nb.knots.iter().copied().filter(|k| *k > 1e-9 && *k < 1.0 - 1e-9).collect();
    assert!(inner.len() == 2 && (inner[0] - 0.32950028).abs() < 1e-8 && (inner[1] - 0.625795524).abs() < 1e-8, "{inner:?}");
    let live = [(0.0, 0.0), (4.24206415, -0.117932998), (12.29870301, -0.341914423), (9.459277054, 12.86737159), (19.433440481, 12.310685391), (25.0, 12.0)];
    assert_eq!(nb.ctrl.len(), live.len());
    for (c, (x, y)) in nb.ctrl.iter().zip(live) {
        assert!((*c - mm(x, y)).length() < 1e-9, "{c} vs ({x}, {y}) mm");
    }
    let mut sk = Sketch::new();
    let ring = [mm(0.0, 0.0), mm(30.0, -10.0), mm(50.0, 15.0), mm(25.0, 35.0), mm(-5.0, 20.0), mm(0.0, 0.0)];
    let id = sk.add_closed_fit_spline(&ring).unwrap();
    let c = sk.curve(id).unwrap();
    let (a, b) = c.domain();
    let q: Vec<DVec2> = (0..=20000).map(|i| c.point(a + (b - a) * i as f64 / 20000.0)).collect();
    let area = q.windows(2).map(|w| w[0].perp_dot(w[1])).sum::<f64>().abs() * 0.5 * 1e6;
    assert!((area - 1978.285).abs() < 0.01, "{area} mm²");
    assert!((c.tangent(a).normalize() - c.tangent(b).normalize()).length() < 1e-9);
}

/// A closed 3D fit (Onshape's opFitSpline through points whose last is the first) is the closed sketch spline's
/// periodic cubic at the centripetal parameters (live 1836 above; cf-3d-spline's Curve 1 then has live's box): on a
/// plane its control points are the sketch's, and tilting the points tilts the curve.
#[test]
fn a_closed_3d_fit_is_the_closed_sketch_splines() {
    let mm = |x: f64, y: f64| v(x * 1e-3, y * 1e-3);
    let ring = [mm(0.0, 0.0), mm(30.0, -10.0), mm(50.0, 15.0), mm(25.0, 35.0), mm(-5.0, 20.0), mm(0.0, 0.0)];
    let flat = Nurbs::interpolate_periodic(&ring).unwrap();
    let (knots, poles) = noshape_sketch::nurbs::fit_3d_closed(&ring.map(|p| glam::DVec3::new(p.x, p.y, 0.0))).unwrap();
    assert_eq!(poles.len(), flat.ctrl.len());
    assert!(knots.iter().zip(&flat.knots).all(|(a, b)| (a - b).abs() < 1e-12), "{knots:?} vs {:?}", flat.knots);
    for (p, c) in poles.iter().zip(&flat.ctrl) {
        assert!((p.truncate() - *c).length() < 1e-12 && p.z.abs() < 1e-15, "{p} vs {c}");
    }
    let tilt = glam::DQuat::from_axis_angle(glam::DVec3::new(1.0, 2.0, 0.5).normalize(), 0.7);
    let (_, tilted) = noshape_sketch::nurbs::fit_3d_closed(&ring.map(|p| tilt * glam::DVec3::new(p.x, p.y, 0.0))).unwrap();
    for (p, c) in tilted.iter().zip(&flat.ctrl) {
        assert!((*p - tilt * glam::DVec3::new(c.x, c.y, 0.0)).length() < 1e-12, "{p}");
    }
}

/// Live (behaviours "psb (10-24 j) sketch offset constraints", qa4
/// OFF-CONSTR): the first pair gets the offset DISTANCE, every other pair an
/// OFFSET tied to it, so the copies follow their sources when those move and
/// the one distance drives them all.
#[test]
fn offset_copies_follow_their_sources() {
    let mut sk = Sketch::new();
    let r = sk.corner_rectangle(v(0.0, 0.0), v(4.0, 2.0));
    let res = sk.offset(&r.curves, 0.5).unwrap();
    let kinds: Vec<&ConstraintKind> = res.constraints_added.iter().filter_map(|c| sk.constraint(*c)).map(|c| &c.kind).collect();
    let dist: Vec<ConstraintId> = res.constraints_added.iter().copied().filter(|c| matches!(sk.constraint(*c).map(|c| &c.kind), Some(ConstraintKind::Distance { .. }))).collect();
    assert_eq!(dist.len(), 1, "{kinds:?}");
    assert_eq!(kinds.iter().filter(|k| matches!(k, ConstraintKind::OffsetPair { .. })).count(), 3, "{kinds:?}");
    assert!(!kinds.iter().any(|k| matches!(k, ConstraintKind::Parallel { .. })), "{kinds:?}");
    assert!(sk.solve().is_solved());
    let copies: Vec<EntityId> = res.created.iter().copied().filter(|e| sk.kind(*e) == Some(EntityKind::Line)).collect();
    // Each copy with its source: the parallel side nearest to it.
    let ends = |sk: &Sketch, id: EntityId| {
        let (a, b) = sk.endpoints(id).unwrap();
        (sk.point(a).unwrap(), sk.point(b).unwrap())
    };
    let pairs: Vec<(EntityId, EntityId)> = copies
        .iter()
        .map(|&c| {
            let (c0, c1) = ends(&sk, c);
            let src = *r.curves.iter().filter(|&&s| { let (a, b) = ends(&sk, s); (b - a).perp_dot(c1 - c0).abs() < 1e-9 }).min_by(|&&x, &&y| {
                let m = (c0 + c1) / 2.0;
                let dx = { let (a, b) = ends(&sk, x); ((a + b) / 2.0 - m).length() };
                let dy = { let (a, b) = ends(&sk, y); ((a + b) / 2.0 - m).length() };
                dx.total_cmp(&dy)
            }).unwrap();
            (src, c)
        })
        .collect();
    let gap = |sk: &Sketch, want: f64| {
        for &(src, copy) in &pairs {
            let (s0, s1) = sk.endpoints(src).unwrap();
            let (a, b) = (sk.point(s0).unwrap(), sk.point(s1).unwrap());
            let (c0, c1) = sk.endpoints(copy).unwrap();
            for p in [sk.point(c0).unwrap(), sk.point(c1).unwrap()] {
                let d = (b - a).perp_dot(p - a).abs() / (b - a).length();
                assert!(close(d, want, 1e-7), "copy {copy:?} of {src:?} at {d}, not {want}");
            }
        }
    };
    gap(&sk, 0.5);
    // The source's right side moves out: its copy follows, still 0.5 inside.
    let rep = sk.drag(&[(DragHandle::Entity { id: r.curves[1], grab: v(4.0, 1.0) }, v(6.0, 1.0))]);
    assert!(rep.is_solved(), "{rep:?}");
    gap(&sk, 0.5);
    // The one distance drives every copy.
    sk.set_dimension_value(dist[0], 0.3).unwrap();
    assert!(sk.solve().is_solved());
    gap(&sk, 0.3);
}

/// Live (behaviours "psb (10-24 j)" (c), "qa4 round 5" (4)): a lone line or
/// circle gets one DISTANCE and one OFFSET on the pair alone; the line's copy
/// stays parallel when its source turns, the circle's grows with the value.
#[test]
fn offset_lone_line_and_circle_stay_offsets() {
    let mut sk = Sketch::new();
    let l = sk.add_line(v(0.0, 0.0), v(4.0, 0.0));
    let res = sk.offset(&[l], 1.0).unwrap();
    let kinds: Vec<ConstraintKind> = res.constraints_added.iter().filter_map(|c| sk.constraint(*c)).map(|c| c.kind.clone()).collect();
    assert_eq!(kinds.len(), 2, "{kinds:?}");
    assert!(matches!(kinds[0], ConstraintKind::Distance { .. }) && matches!(kinds[1], ConstraintKind::OffsetPair { .. }), "{kinds:?}");
    let copy = res.created.iter().copied().find(|e| sk.kind(*e) == Some(EntityKind::Line)).unwrap();
    let rep = sk.drag(&[(DragHandle::Entity { id: l, grab: v(4.0, 0.0) }, v(4.0, 2.0))]);
    assert!(rep.is_solved(), "{rep:?}");
    let (s0, s1) = sk.endpoints(l).unwrap();
    let (a, b) = (sk.point(s0).unwrap(), sk.point(s1).unwrap());
    let (c0, c1) = sk.endpoints(copy).unwrap();
    for p in [sk.point(c0).unwrap(), sk.point(c1).unwrap()] {
        assert!(close((b - a).perp_dot(p - a).abs() / (b - a).length(), 1.0, 1e-7), "{p}");
    }

    let mut sk = Sketch::new();
    let c = sk.add_circle(v(0.0, 0.0), 5.0);
    let res = sk.offset(&[c], -3.0).unwrap();
    let dist = res.constraints_added[0];
    assert!(matches!(sk.constraint(dist).map(|c| &c.kind), Some(ConstraintKind::Distance { .. })));
    assert!(matches!(sk.constraint(res.constraints_added[1]).map(|c| &c.kind), Some(ConstraintKind::OffsetPair { .. })));
    let copy = *res.created.last().unwrap();
    sk.set_dimension_value(dist, 2.0).unwrap();
    assert!(sk.solve().is_solved());
    let r = |sk: &Sketch, id| match sk.curve(id) {
        Some(Curve2::Circle { radius, .. }) => radius,
        other => panic!("{other:?}"),
    };
    assert!(close(r(&sk, copy) - r(&sk, c), 2.0, 1e-7), "{} {}", r(&sk, copy), r(&sk, c));
}

/// Live "skb (10-25) sketch Use on a face sketch": a used circle's centre
/// point is construction; a used line's end points are not.
#[test]
fn used_circle_centre_is_construction() {
    let mut sk = Sketch::new();
    let res = sk.use_external(&Curve2::Circle { center: v(10.0, 10.0), radius: 3.0 }).unwrap();
    let circle = *res.created.last().unwrap();
    let centre = sk.entity(circle).and_then(|e| e.center()).unwrap();
    assert!(sk.entity(centre).unwrap().construction && sk.entity(centre).unwrap().external);
    let res = sk.use_external(&Curve2::Line { a: v(40.0, 20.0), b: v(0.0, 20.0) }).unwrap();
    let line = *res.created.last().unwrap();
    let (a, b) = sk.endpoints(line).unwrap();
    assert!(!sk.entity(a).unwrap().construction && !sk.entity(b).unwrap().construction);
}

/// SKETCH-DEGENERATE-SOLVE (live behaviours "skasm (2026-10-31) hole row
/// name, unsolvable dimension colours" (b)): a 20 × 10 corner rectangle from
/// the origin (bottom 20, left 10), the top-right corner filleted R6, then
/// R20: live solves it with the arc's centre at (0, −10), the right edge
/// running down to y −10 and the top line shrunk to a point.
#[test]
fn fillet_radius_past_the_rectangle_collapses_the_top_line() {
    let mut sk = Sketch::new();
    let r = sk.corner_rectangle(v(0.0, 0.0), v(0.020, 0.010));
    // The corner on the origin (live: Coincident with it).
    sk.add_constraint(ConstraintKind::Fix { entity: r.points[0] }).unwrap();
    sk.add_constraint(ConstraintKind::Length { curve: r.curves[0], value: 0.020 }).unwrap();
    sk.add_constraint(ConstraintKind::Length { curve: r.curves[3], value: 0.010 }).unwrap();
    let res = sk.fillet_corner(r.points[2], 0.006).unwrap();
    let arc = res.created.iter().copied().find(|e| sk.kind(*e) == Some(EntityKind::Arc)).unwrap();
    assert!(sk.solve().is_solved());
    let dim = sk.constraints().find(|c| matches!(c.kind, ConstraintKind::Radius { curve, .. } if curve == arc)).unwrap().id;
    sk.set_dimension_value(dim, 0.020).unwrap();
    let opts = sk.dimension_edit_options(dim);
    let rep = sk.solve_with(&opts);
    let Curve2::Arc { center, .. } = sk.curve(arc).unwrap() else { panic!() };
    let right = sk.curve(r.curves[1]).unwrap();
    let top = sk.curve(r.curves[2]).unwrap();
    eprintln!("{rep:?}\ncentre {center:?} right {:?}–{:?} top {:?}–{:?}", right.start_point(), right.end_point(), top.start_point(), top.end_point());
    assert!(rep.is_solved(), "{rep:?}");
    assert!((center - v(0.0, -0.010)).length() < 1e-9, "{center:?}");
    assert!((top.end_point() - top.start_point()).length() < 1e-9);
    assert!(right.start_point().y.min(right.end_point().y) < -0.010 + 1e-9);
}

/// Live (the same request, (b)): a distance dimension set to 0 between a
/// rectangle's top and bottom solves, the lines coincident and the sides
/// shrunk to points (a dimension edit may collapse lines; CON-HV-COLLAPSE
/// only refuses constraints doing it).
#[test]
fn distance_set_to_zero_collapses_the_sides() {
    let mut sk = Sketch::new();
    let r = sk.corner_rectangle(v(0.0, 0.0), v(0.020, 0.010));
    sk.add_constraint(ConstraintKind::Fix { entity: r.points[0] }).unwrap();
    sk.add_constraint(ConstraintKind::Length { curve: r.curves[0], value: 0.020 }).unwrap();
    let d = sk.add_constraint(ConstraintKind::Distance { a: r.curves[0], b: r.curves[2], value: 0.010, mode: DistanceMode::default() }).unwrap();
    assert!(sk.solve().is_solved());
    sk.set_dimension_value(d, 0.0).unwrap();
    let opts = sk.dimension_edit_options(d);
    let rep = sk.solve_with(&opts);
    assert!(rep.is_solved(), "{rep:?}");
    let side = sk.curve(r.curves[1]).unwrap();
    assert!((side.end_point() - side.start_point()).length() < 1e-9, "{side:?}");
}

/// Live sketch-tools-1123 fillet.md / chamfer.md: two crossing lines picked left of and below their crossing: each is
/// cut there keeping the picked side, the fillet (no visualSharp point) or chamfer joins them in that quadrant.
#[test]
fn picked_crossing_lines_fillet_and_chamfer_in_the_picks_quadrant() {
    let mm = |x: f64, y: f64| v(x * 1e-3, y * 1e-3);
    let lines = |sk: &mut Sketch| (sk.add_line(mm(-110.0, -25.0), mm(-75.0, -25.0)), sk.add_line(mm(-92.0, -10.0), mm(-92.0, -45.0)));
    let ends = |sk: &Sketch, id: EntityId| {
        let c = sk.curve(id).unwrap();
        (c.start_point() * 1e3, c.end_point() * 1e3)
    };
    let near = |p: DVec2, x: f64, y: f64| (p - v(x, y)).length() < 1e-6;

    let mut sk = Sketch::new();
    let (h, w) = lines(&mut sk);
    let r = sk.fillet_picked((h, mm(-100.0, -25.0)), (w, mm(-92.0, -35.0)), 3e-3).unwrap();
    assert!(sk.solve().is_solved());
    let (hs, he) = ends(&sk, h);
    let (ws, we) = ends(&sk, w);
    assert!(near(hs, -110.0, -25.0) && near(he, -95.0, -25.0), "horizontal {hs} {he}");
    assert!(near(ws, -92.0, -28.0) && near(we, -92.0, -45.0), "vertical {ws} {we}");
    let arc = r.created.iter().copied().find(|e| sk.kind(*e) == Some(EntityKind::Arc)).expect("the fillet arc");
    let Some(Curve2::Arc { center, radius, .. }) = sk.curve(arc) else { panic!("an arc") };
    assert!(near(center * 1e3, -95.0, -28.0) && close(radius, 3e-3, 1e-12), "arc {center} r {radius}");
    let lone = sk.entities().filter(|e| e.kind() == EntityKind::Point && sk.users_of_point(e.id).is_empty()).count();
    assert_eq!(lone, 0, "no visualSharp point");

    let mut sk = Sketch::new();
    let (h, w) = lines(&mut sk);
    sk.chamfer_picked((h, mm(-100.0, -25.0)), (w, mm(-92.0, -35.0)), 4e-3, 6e-3).unwrap();
    assert!(sk.solve().is_solved());
    let (hs, he) = ends(&sk, h);
    let (ws, we) = ends(&sk, w);
    assert!(near(hs, -110.0, -25.0) && near(he, -96.0, -25.0), "horizontal {hs} {he}");
    assert!(near(ws, -92.0, -31.0) && near(we, -92.0, -45.0), "vertical {ws} {we}");

    let mut sk = Sketch::new();
    let a = sk.add_line(mm(0.0, 0.0), mm(10.0, 0.0));
    let b = sk.add_line(mm(0.0, 5.0), mm(10.0, 5.0));
    let n = sk.entities().count();
    assert!(sk.fillet_picked((a, mm(5.0, 0.0)), (b, mm(5.0, 5.0)), 1e-3).is_err(), "parallel lines");
    assert_eq!(sk.entities().count(), n, "left as it was");
}

#[test]
fn a_circles_centre_on_a_line_is_a_trim_boundary_as_live() {
    // Live (skasm circle-trim-cons-1125): a click between the centre and the right crossing removes only that span; the
    // left piece ends on the centre point.
    let mut sk = Sketch::new();
    let g = sk.add_line(v(0.0, 0.0), v(40.0, 0.0));
    let c = sk.add_circle(v(20.0, 0.0), 5.0);
    let centre = sk.entity(c).and_then(|e| e.center()).unwrap();
    let (t0, t1) = sk.trim_span(g, v(22.0, 0.0)).unwrap();
    let curve = sk.curve(g).unwrap();
    let (a, b) = (curve.point(t0.min(t1)).x, curve.point(t0.max(t1)).x);
    assert!((a - 20.0).abs() < 1e-6 && (b - 25.0).abs() < 1e-6, "hover span {a}..{b}");
    sk.trim(g, v(22.0, 0.0)).unwrap();
    let (_, end) = sk.endpoints(g).unwrap();
    assert_eq!(end, centre, "the left piece ends on the circle's centre");
    let rest: Vec<_> = sk.entities().filter(|e| e.kind() == EntityKind::Line && e.id != g).map(|e| e.id).collect();
    let (s, _) = sk.endpoints(rest[0]).unwrap();
    assert!((sk.point(s).unwrap().x - 25.0).abs() < 1e-6, "the right piece starts at the crossing");
}
