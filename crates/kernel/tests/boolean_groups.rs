//! A cut with many tools gathers the tools that don't touch each other into
//! one argument each group (boolean.cpp group_disjoint); tools that cross
//! must stay apart, and the result is the same solid.

use noshape_kernel::*;
use std::f64::consts::PI;

#[test]
fn cut_with_many_tools_some_crossing() {
    let plate = make_box_corners(DVec3::ZERO, DVec3::new(100.0, 100.0, 10.0)).unwrap();
    // 12 holes Ø6 through the plate, apart from each other and from the bars.
    let r = 3.0;
    let mut tools: Vec<Shape> = Vec::new();
    for i in 0..4 {
        for j in 0..3 {
            let c = DVec3::new(15.0 + 22.0 * i as f64, 15.0 + 70.0 * (j as f64 / 2.0), -1.0);
            if (c.y - 50.0).abs() < 1.0 {
                continue;
            }
            tools.push(make_cylinder(&Plane::new(c, DVec3::Z, DVec3::X), r, 12.0, 2.0 * PI).unwrap());
        }
    }
    for i in 0..4 {
        tools.push(make_cylinder(&Plane::new(DVec3::new(15.0 + 22.0 * i as f64, 95.0, -1.0), DVec3::Z, DVec3::X), r, 12.0, 2.0 * PI).unwrap());
    }
    let holes = tools.len();
    assert_eq!(holes, 12);
    // Two through slots crossing each other in the middle (a plus).
    tools.push(make_box_corners(DVec3::new(20.0, 48.0, -1.0), DVec3::new(80.0, 52.0, 11.0)).unwrap());
    tools.push(make_box_corners(DVec3::new(48.0, 30.0, -1.0), DVec3::new(52.0, 70.0, 11.0)).unwrap());
    let res = boolean(BooleanOp::Cut, &[plate], &tools, &BooleanOptions::default()).unwrap().shape;
    let want = 100.0 * 100.0 * 10.0 - holes as f64 * PI * r * r * 10.0 - (60.0 * 4.0 + 40.0 * 4.0 - 4.0 * 4.0) * 10.0;
    let v = res.volume().unwrap();
    assert!((v - want).abs() < 1e-6 * want, "volume {v}, want {want}");
    assert_eq!(res.count(ShapeType::Solid), 1);
    assert!(res.is_valid());
}

/// Holes straight through a plate are built directly (boolean.cpp
/// cut_through_holes; big-plate's 400 cost ~1.7 s of CPU in OCCT): the same
/// solid as a cut, each tool's side the hole's wall, its caps gone.
#[test]
fn holes_through_a_plate_built_directly() {
    let _ = set_working_scale(1000.0);
    let mm = 1e-3;
    let plate = make_box_corners(DVec3::ZERO, DVec3::new(160.0, 160.0, 10.0) * mm).unwrap();
    let r = 4.0 * mm;
    let mut tools: Vec<Shape> = Vec::new();
    for i in 0..8 {
        for j in 0..8 {
            let c = DVec3::new(10.0 + 20.0 * i as f64, 10.0 + 20.0 * j as f64, -5.0) * mm;
            tools.push(make_cylinder(&Plane::new(c, DVec3::Z, DVec3::X), r, 20.0 * mm, 2.0 * PI).unwrap());
        }
    }
    let t = std::time::Instant::now();
    let res = boolean(BooleanOp::Cut, &[plate], &tools, &BooleanOptions::default()).unwrap();
    eprintln!("64 holes in {:.1} ms", t.elapsed().as_secs_f64() * 1e3);
    let s = &res.shape;
    assert!(s.is_valid());
    let want = (160.0 * 160.0 - 64.0 * PI * 16.0) * 10.0 * 1e-9;
    assert!((s.volume().unwrap() - want).abs() < 1e-12, "{} vs {want}", s.volume().unwrap());
    assert_eq!(s.faces().len(), 6 + 64);
    for (k, tool) in tools.iter().enumerate() {
        let side = tool.faces().iter().position(|f| matches!(f.face_surface(), Ok(Surface::Cylinder { .. }))).unwrap() as u32;
        let walls = res.history.modified(k as u32 + 1, ShapeType::Face, side);
        assert_eq!(walls.len(), 1, "tool {k}: {:?}", res.history.of_input(k as u32 + 1, ShapeType::Face, side).collect::<Vec<_>>());
    }
}

/// A boss standing on a face is fused directly (boolean.cpp fuse_on_face):
/// the face gets the boss's outline as an inner loop; the same solid as the
/// boolean's. Off the face's edge it is fused as usual.
#[test]
fn a_boss_on_a_face_fused_directly() {
    let _ = set_working_scale(1000.0);
    let mm = 1e-3;
    let plate = make_box_corners(DVec3::ZERO, DVec3::new(100.0, 60.0, 10.0) * mm).unwrap();
    let boss = make_cylinder(&Plane::new(DVec3::new(30.0, 30.0, 10.0) * mm, DVec3::Z, DVec3::X), 5.0 * mm, 8.0 * mm, 2.0 * PI).unwrap();
    let t = std::time::Instant::now();
    let r = boolean(BooleanOp::Fuse, std::slice::from_ref(&plate), std::slice::from_ref(&boss), &BooleanOptions::default()).unwrap();
    eprintln!("boss in {:.2} ms", t.elapsed().as_secs_f64() * 1e3);
    assert!(r.shape.is_valid());
    let want = (100.0 * 60.0 * 10.0 + PI * 25.0 * 8.0) * 1e-9;
    assert!((r.shape.volume().unwrap() - want).abs() < 1e-13, "{}", r.shape.volume().unwrap());
    assert_eq!(r.shape.faces().len(), 6 + 2);
    let top = plate.faces().iter().position(|f| f.centroid().unwrap().z > 9.9 * mm).unwrap() as u32;
    assert_eq!(r.history.modified(0, ShapeType::Face, top).len(), 1);
    let bottom_cap = boss.faces().iter().position(|f| f.centroid().unwrap().z < 10.1 * mm && matches!(f.face_surface(), Ok(Surface::Plane(_)))).unwrap() as u32;
    assert!(r.history.is_deleted(1, ShapeType::Face, bottom_cap));
    // Over the plate's edge: the boolean's (the boss's foot is split).
    let edge = make_cylinder(&Plane::new(DVec3::new(98.0, 30.0, 10.0) * mm, DVec3::Z, DVec3::X), 5.0 * mm, 8.0 * mm, 2.0 * PI).unwrap();
    let r = boolean(BooleanOp::Fuse, &[plate], &[edge], &BooleanOptions::default()).unwrap();
    assert!(r.shape.is_valid());
    let v = r.shape.volume().unwrap();
    assert!((v - (100.0 * 60.0 * 10.0 + PI * 25.0 * 8.0) * 1e-9).abs() < 1e-12, "{v}");
}
