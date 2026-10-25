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

/// Live "psb (10-24 j) sketch offset constraints" on the L chain (mm:
/// (0,0)-(80,0)-(80,20)-(30,20)-(30,50)-(0,50)) and a Ø10 circle at (15,35).
fn offset_l() -> (Sketch, [EntityId; 6], EntityId) {
    let mut sk = Sketch::new();
    let m = 1e-3;
    let pts: Vec<EntityId> = [(0.0, 0.0), (80.0, 0.0), (80.0, 20.0), (30.0, 20.0), (30.0, 50.0), (0.0, 50.0)].iter().map(|(x, y)| sk.add_point(v(x * m, y * m))).collect();
    let l: Vec<EntityId> = (0..6).map(|i| sk.add_line_between(pts[i], pts[(i + 1) % 6]).unwrap()).collect();
    let c = sk.add_circle(v(15.0 * m, 35.0 * m), 5.0 * m);
    (sk, [l[0], l[1], l[2], l[3], l[4], l[5]], c)
}

fn radius(sk: &Sketch, id: EntityId) -> f64 {
    match sk.curve(id) {
        Some(Curve2::Circle { radius, .. }) => radius,
        c => panic!("{id} is not a circle: {c:?}"),
    }
}

fn line_ends(sk: &Sketch, id: EntityId) -> (DVec2, DVec2) {
    match sk.curve(id) {
        Some(Curve2::Line { a, b }) => (a, b),
        c => panic!("{id} is not a line: {c:?}"),
    }
}

/// Distance from the midpoint of line `copy` to the line through `src`, and
/// whether they are parallel.
fn offset_of(sk: &Sketch, src: EntityId, copy: EntityId) -> (f64, bool) {
    let (a, b) = line_ends(sk, src);
    let (p, q) = line_ends(sk, copy);
    let d = (b - a).normalize();
    (d.perp_dot((p + q) * 0.5 - a).abs(), d.perp_dot((q - p).normalize()).abs() < 1e-9)
}

#[test]
fn offset_tool_makes_live_constraints_on_a_chain() {
    let m = 1e-3;
    let (mut sk, l, _) = offset_l();
    // (a) the top edge (l4, picked first) and the upper leg's right edge
    // (l3), 3 mm outward (the copy of the top edge above it).
    let up = |d: f64| {
        let mut t = sk.clone();
        let r = t.offset_tool(&[l[4], l[3]], d).unwrap();
        r.created.iter().any(|e| t.kind(*e) == Some(EntityKind::Line) && line_ends(&t, *e).0.y > 0.051)
    };
    let d = if up(3.0 * m) { 3.0 * m } else { -3.0 * m };
    let res = sk.offset_tool(&[l[4], l[3]], d).unwrap();
    let lines: Vec<EntityId> = res.created.iter().copied().filter(|e| sk.kind(*e) == Some(EntityKind::Line)).collect();
    assert_eq!(lines.len(), 2);
    let copy = |src: EntityId| *lines.iter().find(|c| offset_of(&sk, src, **c).1 && (offset_of(&sk, src, **c).0 - 3.0 * m).abs() < 1e-9).unwrap();
    let (c4, c3) = (copy(l[4]), copy(l[3]));
    // Sharp corner at (33, 53); free ends at (33, 20) and (0, 53).
    let ends: Vec<DVec2> = [c3, c4].iter().flat_map(|c| [line_ends(&sk, *c).0, line_ends(&sk, *c).1]).collect();
    for p in [v(33.0 * m, 53.0 * m), v(33.0 * m, 20.0 * m), v(0.0, 53.0 * m)] {
        assert!(ends.iter().any(|e| (*e - p).length() < 1e-9), "{p:?} in {ends:?}");
    }
    // One Coincident (the corner), one OFFSET on (l4, c4) carrying
    // (l3, c3), one DISTANCE 3 on the first picked pair; no Parallel.
    let kinds: Vec<ConstraintKind> = res.constraints_added.iter().map(|id| sk.constraint(*id).unwrap().kind.clone()).collect();
    assert_eq!(kinds.iter().filter(|k| matches!(k, ConstraintKind::Coincident { .. })).count(), 1, "{kinds:?}");
    assert!(!kinds.iter().any(|k| matches!(k, ConstraintKind::Parallel { .. })), "{kinds:?}");
    let offsets: Vec<&ConstraintKind> = kinds.iter().filter(|k| matches!(k, ConstraintKind::OffsetCurve { .. })).collect();
    assert_eq!(offsets.len(), 1);
    assert!(matches!(offsets[0], ConstraintKind::OffsetCurve { master, curve, second: Some((s, sc)), first_shape: true, .. } if *master == l[4] && *curve == c4 && *s == l[3] && *sc == c3));
    let dist: Vec<(ConstraintId, ConstraintKind)> = res.constraints_added.iter().map(|id| (*id, sk.constraint(*id).unwrap().kind.clone())).filter(|(_, k)| matches!(k, ConstraintKind::Distance { .. })).collect();
    assert_eq!(dist.len(), 1);
    assert!(matches!(&dist[0].1, ConstraintKind::Distance { a, b, value, .. } if *a == l[4] && *b == c4 && (value - 3.0 * m).abs() < 1e-12));
    let rep = sk.solve();
    assert!(rep.is_solved() && rep.redundant.is_empty(), "{rep:?}");
    // (d) the source top edge 30 → 40 long: the copies follow at 3 mm.
    sk.add_constraint(ConstraintKind::Length { curve: l[4], value: 40.0 * m }).unwrap();
    assert!(sk.solve().is_solved());
    for (s, c) in [(l[4], c4), (l[3], c3)] {
        let (off, par) = offset_of(&sk, s, c);
        assert!(par && (off - 3.0 * m).abs() < 1e-9, "{off}");
    }
    // (e) the "3" set to 6 moves both copies together.
    sk.set_dimension_value(dist[0].0, 6.0 * m).unwrap();
    let rep = sk.solve();
    assert!(rep.is_solved() && rep.redundant.is_empty(), "{rep:?}");
    for (s, c) in [(l[4], c4), (l[3], c3)] {
        let (off, par) = offset_of(&sk, s, c);
        assert!(par && (off - 6.0 * m).abs() < 1e-9, "{off}");
    }
}

#[test]
fn offset_tool_on_the_closed_loop_and_a_circle() {
    let m = 1e-3;
    // (b) the whole loop: 6 copies, 6 Coincidents, 5 OFFSETs on the first
    // picked pair, 1 DISTANCE; fully solvable without redundancy.
    let (mut sk, l, c) = offset_l();
    let picks = [l[0], l[1], l[2], l[3], l[4], l[5]];
    let res = sk.offset_tool(&picks, 3.0 * m).unwrap();
    let kinds: Vec<ConstraintKind> = res.constraints_added.iter().map(|id| sk.constraint(*id).unwrap().kind.clone()).collect();
    let count = |f: fn(&ConstraintKind) -> bool| kinds.iter().filter(|k| f(k)).count();
    assert_eq!(count(|k| matches!(k, ConstraintKind::Coincident { .. })), 6, "{kinds:?}");
    assert_eq!(count(|k| matches!(k, ConstraintKind::OffsetCurve { .. })), 5);
    assert_eq!(count(|k| matches!(k, ConstraintKind::OffsetCurve { first_shape: true, .. })), 1);
    assert_eq!(kinds.iter().filter(|k| matches!(k, ConstraintKind::Distance { a, .. } if *a == l[0])).count(), 1);
    let rep = sk.solve();
    assert!(rep.is_solved() && rep.redundant.is_empty(), "{rep:?}");
    // (c) the circle 3 mm outward: its own centre, an OFFSET and a
    // DISTANCE, no Concentric; an edit keeps it concentric.
    let res = sk.offset_tool(&[c], -3.0 * m).unwrap();
    let copy = res.created.iter().copied().find(|e| sk.kind(*e) == Some(EntityKind::Circle)).unwrap();
    assert!((radius(&sk, copy) - 8.0 * m).abs() < 1e-12);
    let kinds: Vec<(ConstraintId, ConstraintKind)> = res.constraints_added.iter().map(|id| (*id, sk.constraint(*id).unwrap().kind.clone())).collect();
    assert!(!kinds.iter().any(|(_, k)| matches!(k, ConstraintKind::Concentric { .. } | ConstraintKind::Coincident { .. })), "{kinds:?}");
    assert!(kinds.iter().any(|(_, k)| matches!(k, ConstraintKind::OffsetCurve { master, curve, second: None, .. } if *master == c && *curve == copy)));
    let (dim, _) = kinds.iter().find(|(_, k)| matches!(k, ConstraintKind::Distance { .. })).unwrap();
    sk.set_dimension_value(*dim, 5.0 * m).unwrap();
    let rep = sk.solve();
    assert!(rep.is_solved() && rep.redundant.is_empty(), "{rep:?}");
    let center = |id: EntityId| match sk.curve(id) {
        Some(Curve2::Circle { center, .. }) => center,
        x => panic!("{x:?}"),
    };
    assert!((center(c) - center(copy)).length() < 1e-9);
    assert!((radius(&sk, copy) - radius(&sk, c) - 5.0 * m).abs() < 1e-9);
}

/// Live (qa4 round 5 v3 s6, CON-HV-COLLAPSE): Vertical on a Horizontal
/// line doesn't collapse it to a point: the sketch can't be solved, the
/// line stays and its constraints are the conflicting ones.
#[test]
fn vertical_on_a_horizontal_line_does_not_solve() {
    let mut sk = Sketch::new();
    let l = sk.add_line(v(0.0, 0.0), v(0.02, 0.0005));
    sk.add_constraint(ConstraintKind::Horizontal { a: l, b: None }).unwrap();
    assert!(sk.solve().is_solved());
    let vert = sk.add_constraint(ConstraintKind::Vertical { a: l, b: None }).unwrap();
    let rep = sk.solve();
    assert!(!rep.is_solved());
    assert!(rep.conflicting().contains(&vert), "{rep:?}");
    let Some(Curve2::Line { a, b }) = sk.curve(l) else { panic!() };
    assert!((b - a).length() > 0.019, "the line stays: {a:?} {b:?}");
}

/// qa4 round 5 v3 s6 (CON-CURVATURE): Curvature between two splines from
/// the Spline tool (interpolated fit splines) solves, tangent and with
/// equal curvature at the joint.
#[test]
fn curvature_between_fit_splines() {
    let mut sk = Sketch::new();
    let a = sk.add_fit_spline(&[v(0.0, 0.0), v(0.01, 0.01), v(0.02, 0.0)]).unwrap();
    let b = sk.add_fit_spline(&[v(0.02, 0.0), v(0.03, -0.015), v(0.04, 0.0)]).unwrap();
    let (_, a_end) = sk.endpoints(a).unwrap();
    let (b_start, _) = sk.endpoints(b).unwrap();
    sk.add_constraint(ConstraintKind::Coincident { a: a_end, b: b_start }).unwrap();
    sk.add_constraint(ConstraintKind::Curvature { a, b }).unwrap();
    let rep = sk.solve();
    assert!(rep.is_solved() && rep.invalid.is_empty() && rep.redundant.is_empty(), "{rep:?}");
    let (ca, cb) = (sk.curve(a).unwrap(), sk.curve(b).unwrap());
    let (ta, tb) = (ca.tangent(ca.domain().1).normalize(), cb.tangent(cb.domain().0).normalize());
    assert!(ta.perp_dot(tb).abs() < 1e-6, "{ta:?} {tb:?}");
}
