use noshape_kernel::*;

#[test]
fn face_with_circle_hole_has_reduced_area() {
    let pl = Plane::xy();
    let outer = vec![
        Segment2d::Line { a: DVec2::new(0.0, 0.0), b: DVec2::new(0.1, 0.0) },
        Segment2d::Line { a: DVec2::new(0.1, 0.0), b: DVec2::new(0.1, 0.05) },
        Segment2d::Line { a: DVec2::new(0.1, 0.05), b: DVec2::new(0.0, 0.05) },
        Segment2d::Line { a: DVec2::new(0.0, 0.05), b: DVec2::new(0.0, 0.0) },
    ];
    let hole = vec![Segment2d::Circle { center: DVec2::new(0.05, 0.025), radius: 0.01 }];
    let f = make_face_2d_tracked(&pl, &outer, &[hole]).unwrap();
    let a = f.shape.area().unwrap();
    let expected = 0.005 - std::f64::consts::PI * 1e-4;
    assert!((a - expected).abs() < 1e-9, "area {a} expected {expected}");
    let r = extrude(&f.shape, DVec3::new(0.0, 0.0, 0.025)).unwrap();
    let v = r.shape.volume().unwrap();
    assert!((v - expected * 0.025).abs() < 1e-10, "volume {v}");
}
