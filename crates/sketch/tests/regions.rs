use noshape_sketch::*;
use std::f64::consts::PI;

fn v(x: f64, y: f64) -> DVec2 {
    DVec2::new(x, y)
}

fn rect(sk: &mut Sketch, x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<EntityId> {
    let p = [sk.add_point(v(x0, y0)), sk.add_point(v(x1, y0)), sk.add_point(v(x1, y1)), sk.add_point(v(x0, y1))];
    (0..4).map(|k| sk.add_line_between(p[k], p[(k + 1) % 4]).unwrap()).collect()
}

fn assert_close(a: f64, b: f64, tol: f64) {
    assert!((a - b).abs() <= tol, "{a} != {b}");
}

#[test]
fn single_circle() {
    let mut sk = Sketch::new();
    sk.add_circle(v(1.0, 1.0), 2.0);
    let r = sk.regions();
    assert_eq!(r.len(), 1);
    assert_close(r[0].area(), PI * 4.0, 1e-9);
    assert!(r[0].contains(v(1.0, 1.0)));
    assert!(!r[0].contains(v(3.5, 1.0)));
}

#[test]
fn two_overlapping_circles() {
    let mut sk = Sketch::new();
    sk.add_circle(v(0.0, 0.0), 1.0);
    sk.add_circle(v(1.0, 0.0), 1.0);
    let r = sk.regions();
    assert_eq!(r.len(), 3);
    let total: f64 = r.iter().map(|x| x.area()).sum();
    // union area of two unit circles at distance 1
    let lens = 2.0 * (0.5f64).acos() - 0.5 * (3.0f64).sqrt();
    assert_close(total, 2.0 * PI - lens, 1e-9);
    let mut areas: Vec<f64> = r.iter().map(|x| x.area()).collect();
    areas.sort_by(f64::total_cmp);
    assert_close(areas[0], lens, 1e-9);
    assert_close(areas[1], PI - lens, 1e-9);
    // ids are unique
    let mut ids: Vec<_> = r.iter().map(|x| x.id.clone()).collect();
    ids.dedup();
    assert_eq!(ids.len(), 3);
    for reg in &r {
        assert!(reg.outer.signed_area() > 0.0);
        assert!(reg.contains(reg.interior_point()));
    }
}

#[test]
fn rectangle_with_circle_hole() {
    let mut sk = Sketch::new();
    rect(&mut sk, 0.0, 0.0, 4.0, 3.0);
    sk.add_circle(v(2.0, 1.5), 1.0);
    let r = sk.regions();
    assert_eq!(r.len(), 2);
    let outer = r.iter().find(|x| !x.holes.is_empty()).expect("region with hole");
    assert_eq!(outer.holes.len(), 1);
    assert!(outer.holes[0].signed_area() < 0.0);
    assert_close(outer.area(), 12.0 - PI, 1e-9);
    assert!(!outer.contains(v(2.0, 1.5)));
    assert!(outer.contains(v(0.2, 0.2)));
    let inner = r.iter().find(|x| x.holes.is_empty()).unwrap();
    assert_close(inner.area(), PI, 1e-9);
}

#[test]
fn intersecting_rectangles_and_dangling_line() {
    let mut sk = Sketch::new();
    rect(&mut sk, 0.0, 0.0, 2.0, 2.0);
    rect(&mut sk, 1.0, 1.0, 3.0, 3.0);
    // dangling line sticking into the first rectangle
    sk.add_line(v(-1.0, 0.5), v(0.5, 0.5));
    let r = sk.regions();
    assert_eq!(r.len(), 3);
    let total: f64 = r.iter().map(|x| x.area()).sum();
    assert_close(total, 7.0, 1e-9);
}

#[test]
fn line_splits_circle_and_construction_ignored() {
    let mut sk = Sketch::new();
    sk.add_circle(v(0.0, 0.0), 1.0);
    sk.add_line(v(-2.0, 0.0), v(2.0, 0.0));
    let c = sk.add_line(v(0.0, -2.0), v(0.0, 2.0));
    sk.set_construction(c, true).unwrap();
    let r = sk.regions();
    assert_eq!(r.len(), 2);
    for reg in &r {
        assert_close(reg.area(), PI / 2.0, 1e-9);
    }
}

#[test]
fn tangent_circles_and_arcs_slot() {
    // two circles tangent internally + a slot made of lines and arcs
    let mut sk = Sketch::new();
    sk.add_circle(v(0.0, 0.0), 2.0);
    sk.add_circle(v(1.0, 0.0), 1.0);
    let r = sk.regions();
    assert_eq!(r.len(), 2);
    let mut a: Vec<f64> = r.iter().map(|x| x.area()).collect();
    a.sort_by(f64::total_cmp);
    assert_close(a[0], PI, 1e-9);
    assert_close(a[1], 3.0 * PI, 1e-9);

    let mut sk = Sketch::new();
    let p = [sk.add_point(v(0.0, -1.0)), sk.add_point(v(4.0, -1.0)), sk.add_point(v(4.0, 1.0)), sk.add_point(v(0.0, 1.0))];
    let c0 = sk.add_point(v(0.0, 0.0));
    let c1 = sk.add_point(v(4.0, 0.0));
    sk.add_line_between(p[0], p[1]).unwrap();
    sk.add_arc_between(c1, p[1], p[2]).unwrap();
    sk.add_line_between(p[2], p[3]).unwrap();
    sk.add_arc_between(c0, p[3], p[0]).unwrap();
    let r = sk.regions();
    assert_eq!(r.len(), 1);
    assert_close(r[0].area(), 8.0 + PI, 1e-9);
}

#[test]
fn ellipse_and_spline_regions() {
    let mut sk = Sketch::new();
    sk.add_ellipse(v(0.0, 0.0), v(2.0, 0.0), 1.0);
    sk.add_line(v(-3.0, 0.0), v(3.0, 0.0));
    let r = sk.regions();
    assert_eq!(r.len(), 2);
    for reg in &r {
        assert_close(reg.area(), PI, 1e-8);
    }
    // closed region bounded by a spline and a line
    let mut sk = Sketch::new();
    let s = sk.add_bspline(&[v(0.0, 0.0), v(0.0, 2.0), v(3.0, 2.0), v(3.0, 0.0)], 3).unwrap();
    let (a, b) = sk.endpoints(s).unwrap();
    sk.add_line_between(b, a).unwrap();
    let r = sk.regions();
    assert_eq!(r.len(), 1);
    // area under cubic Bezier: 3/20*(...) computed via formula = 3.6
    assert_close(r[0].area(), 3.6, 1e-9);
}

#[test]
fn nested_holes_and_islands() {
    let mut sk = Sketch::new();
    rect(&mut sk, 0.0, 0.0, 10.0, 10.0);
    rect(&mut sk, 2.0, 2.0, 8.0, 8.0);
    rect(&mut sk, 4.0, 4.0, 6.0, 6.0);
    let r = sk.regions();
    assert_eq!(r.len(), 3);
    let mut a: Vec<f64> = r.iter().map(|x| x.area()).collect();
    a.sort_by(f64::total_cmp);
    assert_close(a[0], 4.0, 1e-9);
    assert_close(a[1], 32.0, 1e-9);
    assert_close(a[2], 64.0, 1e-9);
    let ring = r.iter().find(|x| (x.area() - 32.0).abs() < 1e-6).unwrap();
    assert_eq!(ring.holes.len(), 1);
    assert!(ring.contains(v(3.0, 3.0)));
    assert!(!ring.contains(v(5.0, 5.0)));
    let picked = sk.region_at(v(5.0, 5.0)).unwrap();
    assert_close(picked.area(), 4.0, 1e-9);
}

#[test]
fn region_id_is_stable_across_edits() {
    let mut sk = Sketch::new();
    sk.add_circle(v(0.0, 0.0), 1.0);
    sk.add_circle(v(1.0, 0.0), 1.0);
    let r = sk.regions();
    let lens = r.iter().min_by(|a, b| a.area().total_cmp(&b.area())).unwrap().id.clone();
    // unrelated geometry added far away
    sk.add_line(v(10.0, 10.0), v(11.0, 11.0));
    sk.add_circle(v(20.0, 0.0), 1.0);
    let found = sk.find_region(&lens).unwrap();
    assert_eq!(found.id, lens);
    assert!(found.contains(v(0.5, 0.0)));
}

#[test]
fn wires_open_and_closed() {
    let mut sk = Sketch::new();
    let a = sk.add_point(v(0.0, 0.0));
    let b = sk.add_point(v(1.0, 0.0));
    let c = sk.add_point(v(1.0, 1.0));
    let d = sk.add_point(v(2.0, 1.0));
    sk.add_line_between(a, b).unwrap();
    sk.add_line_between(b, c).unwrap();
    sk.add_line_between(c, d).unwrap();
    sk.add_circle(v(5.0, 5.0), 1.0);
    let w = sk.wires();
    assert_eq!(w.len(), 2);
    let open = w.iter().find(|x| !x.closed).unwrap();
    assert_eq!(open.segments.len(), 3);
    for k in 0..2 {
        assert!((open.segments[k].end() - open.segments[k + 1].start()).length() < 1e-12);
    }
}
