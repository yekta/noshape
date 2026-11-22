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

/// An STL of several bodies as Onshape imports it (live exchange suite 2147): a part per connected mesh, a mesh inside
/// another that one's hollow (its volume taken out), a mesh through another's face a part of its own (cf-spur-gear's
/// construction pin, half in its gear: live reads 5 parts).
#[test]
fn stl_pieces_are_parts_and_a_piece_inside_is_a_hollow() {
    let mm = 1e-3;
    let boxed = |a: f64, b: f64| make_box_corners(DVec3::splat(a * mm), DVec3::splat(b * mm)).unwrap();
    let corners = |a: DVec3, b: DVec3| make_box_corners(a * mm, b * mm).unwrap();
    let pin = corners(DVec3::new(2.0, 2.0, 25.0), DVec3::new(4.0, 4.0, 31.0));
    let shapes = Shape::compound(&[boxed(0.0, 30.0), boxed(10.0, 20.0), corners(DVec3::new(40.0, 0.0, 0.0), DVec3::new(50.0, 10.0, 10.0)), pin]);
    let dir = std::env::temp_dir().join(format!("noshape-meshparts-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("three.stl");
    write_stl(&shapes, &path, 1e-5, StlFormat::Binary).unwrap();
    let s = read_mesh_bodies(&path, LengthUnit::Meter).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    let solids = s.solids();
    assert_eq!(solids.len(), 3, "the outer box (with its hollow), the box beside it and the pin");
    let mut vols: Vec<f64> = solids.iter().map(|x| volume_properties(x).unwrap().mass / (mm * mm * mm)).collect();
    vols.sort_by(f64::total_cmp);
    // (An STL's coordinates are float32.)
    let near = |v: f64, want: f64| (v - want).abs() < 1e-6 * 27000.0;
    assert!(near(vols[0], 24.0) && near(vols[1], 1000.0) && near(vols[2], 27000.0 - 1000.0), "{vols:?}");
}

/// Display meshing leaves no triangulation on a B-rep shape (OCCT's booleans
/// on a meshed shape are several times slower; mesh.cpp `mesh_fill`), and
/// meshing it again gives the same mesh; a mesh body keeps its triangles.
#[test]
fn tessellating_leaves_no_triangulation_behind() {
    let b = make_box_corners(DVec3::ZERO, DVec3::new(0.02, 0.01, 0.005)).unwrap();
    let pin = make_cylinder(&Plane::from_normal(DVec3::new(0.01, 0.005, -0.001), DVec3::Z), 0.002, 0.01, std::f64::consts::TAU).unwrap();
    let s = cut(&b, &pin).unwrap().shape;
    assert!(!s.has_triangulation());
    let first = tessellate(&s, 1e-4, 0.1, false).unwrap();
    assert!(!first.indices.is_empty());
    assert!(!s.has_triangulation(), "triangulation left on the shape");
    let again = tessellate(&s, 1e-4, 0.1, false).unwrap();
    assert_eq!((again.positions.len(), again.indices.len(), again.edge_points.len()), (first.positions.len(), first.indices.len(), first.edge_points.len()));

    let dir = std::env::temp_dir().join(format!("noshape-meshclean-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("box.stl");
    write_stl(&b, &path, 1e-4, StlFormat::Binary).unwrap();
    let m = read_mesh_bodies(&path, LengthUnit::Meter).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    for _ in 0..2 {
        assert_eq!(tessellate(&m, 1e-4, 0.5, false).unwrap().positions.len(), 36);
        assert!(m.is_mesh() && m.is_valid());
    }
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
    std::fs::write(&path3, stored_zip("3D/3dmodel.model", xml.as_bytes(), false)).unwrap();
    // Zip64, as live writes its 3MF (the exchange suite: every live.3mf read as no parts).
    let path64 = dir.join("cube64.3mf");
    std::fs::write(&path64, stored_zip("3D/3dmodel.model", xml.as_bytes(), true)).unwrap();
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
    for path in [&path3, &path64, &pathg] {
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

/// A zip with one stored member; `zip64`: as live writes its 3MF, every size and offset 0xFFFFFFFF with the values in
/// Zip64 extra fields and end records.
fn stored_zip(name: &str, data: &[u8], zip64: bool) -> Vec<u8> {
    let (crc, n, len) = (crc32(data), name.len() as u16, data.len() as u32);
    let big = |v: u32| if zip64 { u32::MAX } else { v };
    let mut z = Vec::new();
    z.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
    z.extend_from_slice(&[45, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    z.extend_from_slice(&crc.to_le_bytes());
    z.extend_from_slice(&big(len).to_le_bytes());
    z.extend_from_slice(&big(len).to_le_bytes());
    z.extend_from_slice(&n.to_le_bytes());
    z.extend_from_slice(&(if zip64 { 20u16 } else { 0 }).to_le_bytes());
    z.extend_from_slice(name.as_bytes());
    if zip64 {
        z.extend_from_slice(&[1, 0, 16, 0]);
        z.extend_from_slice(&(len as u64).to_le_bytes());
        z.extend_from_slice(&(len as u64).to_le_bytes());
    }
    z.extend_from_slice(data);
    let central = z.len() as u32;
    z.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
    z.extend_from_slice(&[45, 0, 45, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    z.extend_from_slice(&crc.to_le_bytes());
    z.extend_from_slice(&big(len).to_le_bytes());
    z.extend_from_slice(&big(len).to_le_bytes());
    z.extend_from_slice(&n.to_le_bytes());
    z.extend_from_slice(&(if zip64 { 28u16 } else { 0 }).to_le_bytes());
    z.extend_from_slice(&[0; 10]);
    z.extend_from_slice(&big(0).to_le_bytes());
    z.extend_from_slice(name.as_bytes());
    if zip64 {
        z.extend_from_slice(&[1, 0, 24, 0]);
        z.extend_from_slice(&(len as u64).to_le_bytes());
        z.extend_from_slice(&(len as u64).to_le_bytes());
        z.extend_from_slice(&0u64.to_le_bytes());
    }
    let size = z.len() as u32 - central;
    if zip64 {
        let end64 = z.len() as u64;
        z.extend_from_slice(&0x0606_4b50u32.to_le_bytes());
        z.extend_from_slice(&44u64.to_le_bytes());
        z.extend_from_slice(&[45, 0, 45, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        z.extend_from_slice(&1u64.to_le_bytes());
        z.extend_from_slice(&1u64.to_le_bytes());
        z.extend_from_slice(&(size as u64).to_le_bytes());
        z.extend_from_slice(&(central as u64).to_le_bytes());
        z.extend_from_slice(&0x0706_4b50u32.to_le_bytes());
        z.extend_from_slice(&0u32.to_le_bytes());
        z.extend_from_slice(&end64.to_le_bytes());
        z.extend_from_slice(&1u32.to_le_bytes());
    }
    z.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    z.extend_from_slice(&[0, 0, 0, 0, 1, 0, 1, 0]);
    z.extend_from_slice(&size.to_le_bytes());
    z.extend_from_slice(&big(central).to_le_bytes());
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

/// A mesh whose edges are each used an even number of times is closed, as live reads it, though some are used
/// twice the same way (a mesher's fold: the exchange suite's cf-sheet-metal-model-plus STL; here a bottom triangle
/// written three times, in the plane through the origin so the volume stays the cube's).
#[test]
fn stl_with_a_fold_is_a_solid() {
    let p = |x: f64, y: f64, z: f64| [x * 0.01, y * 0.01, z * 0.01];
    let [a, b, c, d] = [p(0., 0., 0.), p(1., 0., 0.), p(1., 1., 0.), p(0., 1., 0.)];
    let [e, f, g, h] = [p(0., 0., 1.), p(1., 0., 1.), p(1., 1., 1.), p(0., 1., 1.)];
    let mut tris = vec![[a, c, b], [a, d, c], [e, f, g], [e, g, h], [a, b, f], [a, f, e], [b, c, g], [b, g, f], [c, d, h], [c, h, g], [d, a, e], [d, e, h]];
    tris.extend([[a, c, b], [a, c, b]]);
    let mut text = String::from("solid fold\n");
    for t in &tris {
        text += "facet normal 0 0 0\nouter loop\n";
        for v in t {
            text += &format!("vertex {} {} {}\n", v[0], v[1], v[2]);
        }
        text += "endloop\nendfacet\n";
    }
    text += "endsolid fold\n";
    let dir = std::env::temp_dir().join(format!("noshape-meshfold-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("fold.stl");
    std::fs::write(&path, text).unwrap();
    let s = read_mesh_bodies(&path, LengthUnit::Meter).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(s.count(ShapeType::Solid), 1, "closed: a solid");
    let v = volume_properties(&s).unwrap().mass;
    assert!((v - 1e-6).abs() < 1e-10, "volume {v}");
}

/// Closed meshes touching along edges or over faces are the model's solids (lead 11-22, DECISIONS: live's import makes
/// sheets of them, c2-bottle-split's two halves 148, its failure): at an edge three or more triangles use, each joins the
/// one met first turning into its inside, each once. What can't close splits as live's import does (stl-split-1121:
/// joined only across edges two triangles use, a repeated triangle read once).
#[test]
fn mesh_pieces_close_into_solids_and_split_only_where_open() {
    // A box from `lo` to `hi`, without its faces at x = `skip_x`.
    let v = |x: f64, y: f64, z: f64| [x, y, z];
    let boxed = |lo: [f64; 3], hi: [f64; 3], skip_x: Option<f64>| {
        let mut t = Vec::new();
        let c = [v(lo[0], lo[1], lo[2]), v(hi[0], lo[1], lo[2]), v(hi[0], hi[1], lo[2]), v(lo[0], hi[1], lo[2]), v(lo[0], lo[1], hi[2]), v(hi[0], lo[1], hi[2]), v(hi[0], hi[1], hi[2]), v(lo[0], hi[1], hi[2])];
        let quads = [([0, 3, 2, 1], None), ([4, 5, 6, 7], None), ([0, 1, 5, 4], None), ([2, 3, 7, 6], None), ([1, 2, 6, 5], Some(hi[0])), ([3, 0, 4, 7], Some(lo[0]))];
        for (q, at) in quads {
            if at.is_some() && at == skip_x {
                continue;
            }
            t.push([c[q[0]], c[q[1]], c[q[2]]]);
            t.push([c[q[0]], c[q[2]], c[q[3]]]);
        }
        t
    };
    let cube = |x0: f64, skip_x: Option<f64>| boxed([x0, 0.0, 0.0], [x0 + 1.0, 1.0, 1.0], skip_x);
    let read = |tris: &[[[f64; 3]; 3]], tag: &str| {
        let mut stl = String::from("solid two\n");
        for t in tris {
            stl += "facet normal 0 0 0\nouter loop\n";
            for p in t {
                stl += &format!("vertex {} {} {}\n", p[0], p[1], p[2]);
            }
            stl += "endloop\nendfacet\n";
        }
        stl += "endsolid two\n";
        let path = std::env::temp_dir().join(format!("noshape-two-cubes-{tag}-{}.stl", std::process::id()));
        std::fs::write(&path, stl).unwrap();
        let s = read_mesh_bodies(&path, LengthUnit::Meter).unwrap();
        let _ = std::fs::remove_file(&path);
        s
    };
    let solids = |s: &noshape_kernel::Shape, want: usize, volume: f64, what: &str| {
        assert_eq!(s.count(ShapeType::Solid), want, "{what}");
        let v = volume_properties(s).unwrap().mass;
        assert!((v - volume).abs() < 1e-9, "{what}: volume {v}");
    };
    // Each cube closed with its own face on x = 1 (two coincident squares, opposite ways, other diagonals).
    let mut tris = cube(0.0, None);
    tris.extend(cube(1.0, None));
    solids(&read(&tris, "a"), 2, 2.0, "two cubes over a face");
    // Without those faces: one box.
    let mut tris = cube(0.0, Some(1.0));
    tris.extend(cube(1.0, Some(1.0)));
    solids(&read(&tris, "b"), 1, 2.0, "one box of two cubes");
    // The second cube's square as the first's two triangles, turned the other way.
    let mut tris = cube(0.0, None);
    let shared: Vec<[[f64; 3]; 3]> = tris.iter().filter(|t| t.iter().all(|p| p[0] == 1.0)).map(|t| [t[0], t[2], t[1]]).collect();
    tris.extend(cube(1.0, Some(1.0)));
    tris.extend(shared);
    solids(&read(&tris, "c"), 2, 2.0, "the shared square repeated");
    // A cube inside a taller box, its bottom the box's own triangles the same way (c2-duct-multipart: parts written
    // over each other, their faces repeated).
    let mut tris = cube(0.0, None);
    tris.extend(boxed([0.0, 0.0, 0.0], [1.0, 1.0, 2.0], None));
    solids(&read(&tris, "d"), 2, 3.0, "a cube inside a box");
    // Open: three squares on one edge are three surfaces; an open box one.
    let sq = |p: [f64; 3]| vec![[v(0.0, 0.0, 0.0), v(0.0, 0.0, 1.0), p], [v(0.0, 0.0, 1.0), [p[0], p[1], 1.0], p]];
    let mut tris = sq(v(1.0, 0.0, 0.0));
    tris.extend(sq(v(0.0, 1.0, 0.0)));
    tris.extend(sq(v(-1.0, 0.0, 0.0)));
    let s = read(&tris, "e");
    assert_eq!((s.count(ShapeType::Solid), s.count(ShapeType::Face)), (0, 3), "three squares on an edge");
    let s = read(&cube(0.0, Some(1.0)), "f");
    assert_eq!((s.count(ShapeType::Solid), s.count(ShapeType::Face)), (0, 1), "an open box");
}
