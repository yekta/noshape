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

/// Mesh bodies (Onshape's mesh parts): one face, no edges; volume, area,
/// transforms (a mirror keeps the volume positive), BREP round trip and
/// display tessellation (a facet per triangle) all work on the triangulation.
#[test]
fn stl_mesh_body() {
    let b = make_box_corners(DVec3::ZERO, DVec3::new(0.02, 0.01, 0.005)).unwrap();
    let dir = std::env::temp_dir().join(format!("noshape-meshbody-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("box.stl");
    write_stl(&b, &path, 1e-4, StlFormat::Binary).unwrap();
    let s = read_mesh_bodies(&path, LengthUnit::Meter).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(s.is_mesh() && s.is_valid());
    assert_eq!((s.count(ShapeType::Solid), s.count(ShapeType::Face), s.count(ShapeType::Edge), s.count(ShapeType::Vertex)), (1, 1, 0, 0));
    assert!(matches!(s.faces()[0].face_surface(), Ok(Surface::Other)));
    let v = volume_properties(&s).unwrap().mass;
    assert!((v - 1e-6).abs() < 1e-11, "volume {v}");
    let a = surface_properties(&s).unwrap().mass;
    assert!((a - 2.0 * (0.02 * 0.01 + 0.02 * 0.005 + 0.01 * 0.005)).abs() < 1e-9, "area {a} (float32 STL)");
    let moved = s.translated(DVec3::new(1.0, 0.0, 0.0)).unwrap();
    let bb = moved.bounding_box(false).unwrap();
    assert!((bb.min.x - 1.0).abs() < 1e-6 && moved.is_mesh(), "{bb:?}");
    let mirrored = s.transformed(&Transform::Mirror(Plane::from_normal(DVec3::ZERO, DVec3::X))).unwrap();
    let vm = volume_properties(&mirrored).unwrap().mass;
    assert!((vm - 1e-6).abs() < 1e-12 && mirrored.bounding_box(false).unwrap().max.x < 1e-6, "mirrored volume {vm}");
    let scaled = s.transformed(&Transform::Scale { center: DVec3::ZERO, factor: 2.0 }).unwrap();
    assert!((volume_properties(&scaled).unwrap().mass - 8e-6).abs() < 1e-11);
    let back = Shape::from_brep_bytes(&s.to_brep_bytes().unwrap()).unwrap();
    assert!(back.is_mesh() && (volume_properties(&back).unwrap().mass - 1e-6).abs() < 1e-12);
    let t = tessellate(&s, 1e-4, 0.5, false).unwrap();
    assert_eq!(t.positions.len(), 36, "faceted: 12 triangles x 3 corners");
}

/// 3MF and glTF as mesh bodies: a 10 mm cube placed 5 mm along x by the
/// file's own transform, read in millimetres.
#[test]
fn mesh_formats_3mf_gltf() {
    let dir = std::env::temp_dir().join(format!("noshape-meshfmt-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let v: [[f64; 3]; 8] = [[0.0, 0.0, 0.0], [10.0, 0.0, 0.0], [10.0, 10.0, 0.0], [0.0, 10.0, 0.0], [0.0, 0.0, 10.0], [10.0, 0.0, 10.0], [10.0, 10.0, 10.0], [0.0, 10.0, 10.0]];
    let t: [[u32; 3]; 12] = [[0, 2, 1], [0, 3, 2], [4, 5, 6], [4, 6, 7], [0, 1, 5], [0, 5, 4], [1, 2, 6], [1, 6, 5], [2, 3, 7], [2, 7, 6], [3, 0, 4], [3, 4, 7]];
    // 3MF: the model XML in a stored zip.
    let mut xml = String::from("<?xml version=\"1.0\"?><model unit=\"millimeter\" xmlns=\"http://schemas.microsoft.com/3dmanufacturing/core/2015/02\"><resources><object id=\"1\" type=\"model\"><mesh><vertices>");
    for p in v {
        xml += &format!("<vertex x=\"{}\" y=\"{}\" z=\"{}\"/>", p[0], p[1], p[2]);
    }
    xml += "</vertices><triangles>";
    for k in t {
        xml += &format!("<triangle v1=\"{}\" v2=\"{}\" v3=\"{}\"/>", k[0], k[1], k[2]);
    }
    xml += "</triangles></mesh></object></resources><build><item objectid=\"1\" transform=\"1 0 0 0 1 0 0 0 1 5 0 0\"/></build></model>";
    let path3 = dir.join("cube.3mf");
    std::fs::write(&path3, stored_zip("3D/3dmodel.model", xml.as_bytes())).unwrap();
    // glTF: metres, a base64 buffer, a node translation.
    let mut buf = Vec::new();
    for p in v {
        for c in p {
            buf.extend_from_slice(&((c / 1000.0) as f32).to_le_bytes());
        }
    }
    for k in t {
        for i in k {
            buf.extend_from_slice(&(i as u16).to_le_bytes());
        }
    }
    let gltf = format!(
        r#"{{"asset":{{"version":"2.0"}},"scene":0,"scenes":[{{"nodes":[0]}}],"nodes":[{{"mesh":0,"translation":[0.005,0,0]}}],"meshes":[{{"primitives":[{{"attributes":{{"POSITION":0}},"indices":1}}]}}],"buffers":[{{"byteLength":{},"uri":"data:application/octet-stream;base64,{}"}}],"bufferViews":[{{"buffer":0,"byteOffset":0,"byteLength":96}},{{"buffer":0,"byteOffset":96,"byteLength":72}}],"accessors":[{{"bufferView":0,"componentType":5126,"count":8,"type":"VEC3"}},{{"bufferView":1,"componentType":5123,"count":36,"type":"SCALAR"}}]}}"#,
        buf.len(),
        b64(&buf)
    );
    let pathg = dir.join("cube.gltf");
    std::fs::write(&pathg, gltf).unwrap();
    for path in [&path3, &pathg] {
        let s = read_mesh_bodies(path, LengthUnit::Millimeter).unwrap_or_else(|e| panic!("{path:?}: {e}"));
        assert!(s.is_mesh() && s.is_valid(), "{path:?}");
        assert_eq!(s.count(ShapeType::Solid), 1, "{path:?}");
        let vol = volume_properties(&s).unwrap().mass;
        assert!((vol - 1e-6).abs() < 1e-12, "{path:?}: volume {vol}");
        let bb = s.bounding_box(false).unwrap();
        assert!((bb.min.x - 0.005).abs() < 1e-6 && (bb.max.x - 0.015).abs() < 1e-6, "{path:?}: {bb:?}"); // the box includes the tolerance
    }
    let _ = std::fs::remove_dir_all(&dir);
}

fn crc32(data: &[u8]) -> u32 {
    let mut c = 0xFFFF_FFFFu32;
    for &b in data {
        c ^= b as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 { (c >> 1) ^ 0xEDB8_8320 } else { c >> 1 };
        }
    }
    !c
}

/// A zip with one stored member.
fn stored_zip(name: &str, data: &[u8]) -> Vec<u8> {
    let (crc, n, len) = (crc32(data), name.len() as u16, data.len() as u32);
    let mut z = Vec::new();
    z.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
    z.extend_from_slice(&[20, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    z.extend_from_slice(&crc.to_le_bytes());
    z.extend_from_slice(&len.to_le_bytes());
    z.extend_from_slice(&len.to_le_bytes());
    z.extend_from_slice(&n.to_le_bytes());
    z.extend_from_slice(&0u16.to_le_bytes());
    z.extend_from_slice(name.as_bytes());
    z.extend_from_slice(data);
    let central = z.len() as u32;
    z.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
    z.extend_from_slice(&[20, 0, 20, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    z.extend_from_slice(&crc.to_le_bytes());
    z.extend_from_slice(&len.to_le_bytes());
    z.extend_from_slice(&len.to_le_bytes());
    z.extend_from_slice(&n.to_le_bytes());
    z.extend_from_slice(&[0; 12]);
    z.extend_from_slice(&0u32.to_le_bytes());
    z.extend_from_slice(name.as_bytes());
    let size = z.len() as u32 - central;
    z.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    z.extend_from_slice(&[0, 0, 0, 0, 1, 0, 1, 0]);
    z.extend_from_slice(&size.to_le_bytes());
    z.extend_from_slice(&central.to_le_bytes());
    z.extend_from_slice(&0u16.to_le_bytes());
    z
}

fn b64(data: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::new();
    for c in data.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for i in 0..4 {
            s.push(if i <= c.len() { A[(n >> (18 - 6 * i) & 63) as usize] as char } else { '=' });
        }
    }
    s
}
