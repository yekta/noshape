//! v0.4.6 macOS arm64 crash (CI run 39827462890): OCCT's Delabella
//! dereferenced null on a planar face of features_a's
//! pattern_instance_references_survive_upstream_edit at the display's
//! angle-based settings (chord tolerance = the part's size). Planar faces
//! now go through BRepMesh's own algorithm; this meshes awkward planar
//! faces at those settings: every face gets triangles, the mesh stays closed.
use noshape_kernel::*;
use std::collections::HashMap;
use std::f64::consts::TAU;

fn display_mesh(s: &Shape) -> Tessellation {
    // display.rs `display_deflection` at Auto: the diagonal and ~144 segments a circle.
    let diag = s.bounding_box(false).unwrap().diagonal();
    tessellate(s, diag, 2.0 * TAU / 144.0, true).unwrap()
}

fn assert_closed_and_complete(what: &str, s: &Shape) {
    let m = display_mesh(s);
    for (i, f) in m.faces.iter().enumerate() {
        assert!(f.index_count > 0, "{what}: face {i} has no triangles");
    }
    let mut id: HashMap<[u32; 3], u32> = HashMap::new();
    let weld: Vec<u32> = m
        .positions
        .iter()
        .map(|p| {
            let n = id.len() as u32;
            *id.entry(p.map(f32::to_bits)).or_insert(n)
        })
        .collect();
    let mut edges: HashMap<(u32, u32), i32> = HashMap::new();
    for t in m.indices.chunks(3) {
        for k in 0..3 {
            let (a, b) = (weld[t[k] as usize], weld[t[(k + 1) % 3] as usize]);
            *edges.entry((a.min(b), a.max(b))).or_insert(0) += 1;
        }
    }
    let open = edges.values().filter(|&&n| n != 2).count();
    assert_eq!(open, 0, "{what}: {open} triangle edges not shared by two triangles");
}

fn bx(a: [f64; 3], b: [f64; 3]) -> Shape {
    make_box_corners(DVec3::from(a), DVec3::from(b)).unwrap()
}

fn cylinder(x: f64, y: f64, z: f64, r: f64, h: f64) -> Shape {
    make_cylinder(&Plane::new(DVec3::new(x, y, z), DVec3::Z, DVec3::X), r, h, TAU).unwrap()
}

#[test]
fn planar_faces_mesh_at_display_settings() {
    // A 1 µm sliver, and a long thin strip.
    assert_closed_and_complete("sliver", &bx([0.0; 3], [0.1, 0.02, 1e-6]));
    assert_closed_and_complete("strip", &bx([0.0; 3], [2.0, 0.0005, 0.0005]));
    // An L-shaped prism (a concave planar face).
    let l = boolean(BooleanOp::Cut, &[bx([0.0; 3], [0.05, 0.04, 0.01])], &[bx([0.02, 0.015, -0.001], [0.06, 0.05, 0.02])], &BooleanOptions::default()).unwrap().shape;
    assert_closed_and_complete("L", &l);
    // Holes nearly touching the outer boundary and each other.
    let tools = vec![cylinder(0.004 + 1e-6, 0.01, -0.001, 0.004, 0.012), cylinder(0.0121, 0.01, -0.001, 0.004, 0.012), cylinder(0.03, 0.03, -0.001, 0.0001, 0.012)];
    let holed = boolean(BooleanOp::Cut, &[bx([0.0; 3], [0.04, 0.04, 0.01])], &tools, &BooleanOptions::default()).unwrap().shape;
    assert_closed_and_complete("near holes", &holed);
    // A prism on a polygon with collinear boundary points (a subdivided side).
    let pts: Vec<DVec3> = (0..=20).map(|k| DVec3::new(0.001 * k as f64, 0.0, 0.0)).chain([DVec3::new(0.02, 0.01, 0.0), DVec3::new(0.0, 0.01, 0.0)]).collect();
    let face = make_face_from_wire(&make_polyline(&pts, true).unwrap(), true).unwrap();
    let prism = extrude(&face, DVec3::new(0.0, 0.0, 0.005)).unwrap().shape;
    assert_closed_and_complete("collinear", &prism);
    // A Part pattern's shape: copies sharing one TShape at other locations.
    let seed = bx([0.0; 3], [0.01, 0.002, 0.004]);
    let copies: Vec<Shape> = (0..5).map(|k| transform(&seed, &Transform::Translate(DVec3::new(0.0, 0.003 * k as f64, 0.0)), false).unwrap().shape).collect();
    assert_closed_and_complete("pattern", &Shape::compound(&copies));
}
