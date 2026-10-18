//! STL/OBJ import: round-trip a box through mesh files.

use glam::DVec3;
use noshape_kernel::*;

fn roundtrip(ext: &str) {
    let b = make_box_corners(DVec3::ZERO, DVec3::new(0.02, 0.01, 0.005)).unwrap();
    let dir = std::env::temp_dir().join(format!("noshape-mesh-{}-{ext}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("box.{ext}"));
    match ext {
        "stl" => write_stl(&b, &path, 1e-4, StlFormat::Binary).unwrap(),
        _ => write_obj(&b, &path, 1e-4).unwrap(),
    }
    // The writers keep model units (meters).
    let s = read_mesh(&path, LengthUnit::Meter, 10_000).unwrap();
    assert_eq!(s.shape_type(), ShapeType::Solid, "closed mesh becomes a solid ({ext})");
    assert_eq!(s.count(ShapeType::Face), 6, "coplanar triangles merge ({ext})");
    let v = s.volume().unwrap();
    assert!((v - 1e-6).abs() < 1e-9, "volume {v} ({ext})");
    // Without merging every triangle stays a face.
    let raw = read_mesh(&path, LengthUnit::Meter, 0).unwrap();
    assert_eq!(raw.count(ShapeType::Face), 12);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn stl_box_roundtrip() {
    roundtrip("stl");
}

#[test]
fn obj_box_roundtrip() {
    roundtrip("obj");
}
