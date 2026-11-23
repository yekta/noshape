//! A free arc (no dimension, no tangency) whose ends sit on a curve that a regeneration resizes: live keeps its radius,
//! each end goes to its nearest point on the new curve and the centre follows (behaviours "skasm free arc re-solve on
//! a resized curve", live/skasm/free-arc-resolve-1123).

use noshape_sketch::*;

fn mm(x: f64, y: f64) -> DVec2 {
    DVec2::new(x * 1e-3, y * 1e-3)
}

fn regen() -> SolveOptions {
    SolveOptions { constructive: true, ..Default::default() }
}

/// The arc from `c` through `s` to `e`, both ends on `curve`.
fn free_arc_on(sk: &mut Sketch, curve: EntityId, c: DVec2, s: DVec2, e: DVec2) -> EntityId {
    let arc = sk.add_arc(c, s, e);
    let (ps, pe) = sk.endpoints(arc).unwrap();
    sk.add_constraint(ConstraintKind::Coincident { a: ps, b: curve }).unwrap();
    sk.add_constraint(ConstraintKind::Coincident { a: pe, b: curve }).unwrap();
    arc
}

fn check_arc(sk: &Sketch, arc: EntityId, live: [DVec2; 3], r: f64) {
    let Some(Curve2::Arc { center, radius, .. }) = sk.curve(arc) else { panic!("not an arc") };
    let (ps, pe) = sk.endpoints(arc).unwrap();
    let got = [center, sk.point(ps).unwrap(), sk.point(pe).unwrap()];
    let off = got.iter().zip(&live).map(|(a, b)| (*a - *b).length()).fold(0.0, f64::max);
    assert!(off < 2e-6 && (radius - r).abs() < 2e-6, "ours r {radius} {got:?}, live r {r} {live:?}");
}

#[test]
fn a_free_arc_on_a_bigger_circle_keeps_its_radius() {
    // Live (a): Ø20 -> 21, the arc's centre slides out along its ray, r 4 kept: by a regeneration and by an edit of
    // the dimension in the open sketch.
    for edit in [false, true] {
        on_a_bigger_circle(edit);
    }
}

fn on_a_bigger_circle(edit: bool) {
    let mut sk = Sketch::new();
    let circle = sk.add_circle(mm(0.0, 0.0), 10e-3);
    let centre = sk.entity(circle).unwrap().center().unwrap();
    sk.add_constraint(ConstraintKind::Fix { entity: centre }).unwrap();
    let dia = sk.add_constraint(ConstraintKind::Diameter { curve: circle, value: 20e-3 }).unwrap();
    let arc = free_arc_on(&mut sk, circle, mm(-5.000004, 11.999991), mm(-5.870127, 8.095777), mm(-1.615089, 9.868713));
    assert!(sk.solve_with(&regen()).is_solved());
    sk.set_dimension_value(dia, 21e-3).unwrap();
    let opts = if edit { sk.dimension_edit_options(dia) } else { regen() };
    assert!(sk.solve_with(&opts).is_solved());
    check_arc(&sk, arc, [mm(-5.154687, 12.371228), mm(-6.163633, 8.500566), mm(-1.695844, 10.362148)], 4e-3);
}

#[test]
fn a_free_arc_on_a_longer_ellipse_keeps_its_radius() {
    // Live (d): major 10 -> 10.5 (vertical, minor 8), r 3 kept; the corpus' notch ("Extruded ring" of c2-rings-loft).
    let mut sk = Sketch::new();
    let e = sk.add_ellipse(mm(0.0, 0.0), mm(0.0, 10.0), 8e-3);
    let centre = sk.entity(e).unwrap().center().unwrap();
    sk.add_constraint(ConstraintKind::Fix { entity: centre }).unwrap();
    sk.add_constraint(ConstraintKind::Vertical { a: e, b: None }).unwrap();
    sk.add_constraint(ConstraintKind::EllipseRadius { ellipse: e, major: false, value: 8e-3 }).unwrap();
    let major = sk.add_constraint(ConstraintKind::EllipseRadius { ellipse: e, major: true, value: 10e-3 }).unwrap();
    let arc = free_arc_on(&mut sk, e, mm(-5.0, 9.5), mm(-5.967424, 6.660266), mm(-2.005447, 9.680698));
    assert!(sk.solve_with(&regen()).is_solved());
    sk.set_dimension_value(major, 10.5e-3).unwrap();
    assert!(sk.solve_with(&regen()).is_solved());
    check_arc(&sk, arc, [mm(-5.117386, 9.586112), mm(-6.121936, 6.759298), mm(-2.163275, 10.108827)], 3e-3);
}
