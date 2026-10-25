//! Pattern and mirror copies made without `copy` are located instances: the
//! rotated copies share the source's TShape, and the mirrors of those share
//! one mirrored TShape (boolean.cpp mirror_instance), so a boolean meets the
//! same pair of TShapes again at the same relative location and reuses the
//! intersection (OCCT patch 0001). The results are the same solids as deep
//! copies.

use noshape_kernel::*;
use std::f64::consts::PI;

#[test]
fn rotated_and_mirrored_instances_share_their_shapes() {
    let tool = make_cylinder(&Plane::new(DVec3::new(30.0, 0.0, -1.0), DVec3::Z, DVec3::X), 4.0, 12.0, 2.0 * PI).unwrap();
    let rotate = |deg: f64| Transform::Rotate { origin: DVec3::ZERO, axis: DVec3::Z, angle: deg.to_radians() };
    let mirror = Transform::Mirror(Plane::new(DVec3::ZERO, DVec3::Y, DVec3::X));
    let rotated: Vec<Shape> = [20.0, 110.0, 200.0, 290.0].iter().map(|&d| transform(&tool, &rotate(d), false).unwrap().shape).collect();
    let results: Vec<OpResult> = rotated.iter().map(|t| transform(t, &mirror, false).unwrap()).collect();
    // Every face and edge of an instance has its image, as for a deep copy (the names follow).
    for (t, res) in rotated.iter().zip(&results) {
        for kind in [ShapeType::Face, ShapeType::Edge] {
            let mut images: Vec<u32> = (0..t.count(kind) as u32).flat_map(|i| res.history.modified(0, kind, i)).collect();
            images.sort();
            assert_eq!(images, (0..res.shape.count(kind) as u32).collect::<Vec<_>>(), "{kind:?}");
        }
    }
    let mirrored: Vec<Shape> = results.into_iter().map(|r| r.shape).collect();
    for t in &rotated {
        assert_eq!(t.partner_hash(), tool.partner_hash());
    }
    for (t, m) in rotated.iter().zip(&mirrored) {
        assert_eq!(m.partner_hash(), mirrored[0].partner_hash());
        assert_ne!(m.partner_hash(), tool.partner_hash());
        let deep = transform(t, &mirror, true).unwrap().shape;
        assert!(m.is_valid());
        assert!((m.centroid().unwrap() - deep.centroid().unwrap()).length() < 1e-9);
        assert!((m.volume().unwrap() - deep.volume().unwrap()).abs() < 1e-9);
        assert_eq!(m.count(ShapeType::Face), 3);
    }
    let plate = make_box_corners(DVec3::new(-50.0, -50.0, 0.0), DVec3::new(50.0, 50.0, 10.0)).unwrap();
    let tools: Vec<Shape> = rotated.into_iter().chain(mirrored).collect();
    let cut = boolean(BooleanOp::Cut, &[plate], &tools, &BooleanOptions::default()).unwrap().shape;
    assert!(cut.is_valid());
    assert_eq!(cut.count(ShapeType::Solid), 1);
    let expected = 100.0 * 100.0 * 10.0 - 8.0 * PI * 16.0 * 10.0;
    assert!((cut.volume().unwrap() - expected).abs() < 1e-6 * expected, "{}", cut.volume().unwrap());
}

/// A small diamond knurl: 12 slanted bars around a cylinder and their
/// mirrors, crossing; the crossings repeat around the axis (congruent pairs,
/// computed once with the OCCT patch). The cut is the one made with deep
/// copies.
#[test]
fn crossing_instances_cut_as_deep_copies() {
    let bar = make_box_corners(DVec3::new(18.5, -0.8, -8.0), DVec3::new(21.5, 0.8, 18.0)).unwrap();
    let bar = transform(&bar, &Transform::Rotate { origin: DVec3::new(20.0, 0.0, 5.0), axis: DVec3::X, angle: 30f64.to_radians() }, true).unwrap().shape;
    let mirror = Transform::Mirror(Plane::new(DVec3::ZERO, DVec3::Y, DVec3::X));
    let tools = |copy: bool| -> Vec<Shape> {
        let left: Vec<Shape> = (0..12).map(|i| transform(&bar, &Transform::Rotate { origin: DVec3::ZERO, axis: DVec3::Z, angle: 2.0 * PI * i as f64 / 12.0 }, copy).unwrap().shape).collect();
        let right: Vec<Shape> = left.iter().map(|t| transform(t, &mirror, copy).unwrap().shape).collect();
        left.into_iter().chain(right).collect()
    };
    let cut = |copy: bool| {
        let cylinder = make_cylinder(&Plane::new(DVec3::ZERO, DVec3::Z, DVec3::X), 20.0, 10.0, 2.0 * PI).unwrap();
        boolean(BooleanOp::Cut, &[cylinder], &tools(copy), &BooleanOptions::default()).unwrap().shape
    };
    let (instances, copies) = (cut(false), cut(true));
    assert!(instances.is_valid());
    assert_eq!(instances.count(ShapeType::Solid), 1);
    assert_eq!(instances.count(ShapeType::Face), copies.count(ShapeType::Face));
    assert_eq!(instances.count(ShapeType::Edge), copies.count(ShapeType::Edge));
    let (a, b) = (instances.volume().unwrap(), copies.volume().unwrap());
    assert!((a - b).abs() < 1e-7 * b, "{a} vs {b}");
}
