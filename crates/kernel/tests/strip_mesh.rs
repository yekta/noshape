//! Cylinders and cones are meshed as strips between their rim rings
//! (mesh.cpp `StripAlgo`) instead of BRepMesh's Delaunay: the same edges
//! (silhouette ends, drawn edges), the same angular sampling (every strip
//! node is one of BRepMesh's), no more deviation from the surface than
//! BRepMesh's mesh and no more triangles. The display's own post-processing
//! (no deflection estimate) gives the same mesh as BRepMesh's. One test in
//! this file: it switches NOSHAPE_MESH_NO_STRIPS and NOSHAPE_MESH_BREPMESH_POST.

use glam::DVec3;
use noshape_kernel::*;

/// A 20 × 20 × 5 mm plate with a Ø8 mm hole and a 90° countersink.
fn plate() -> Shape {
    let b = make_box_corners(DVec3::ZERO, DVec3::new(0.02, 0.02, 0.005)).unwrap();
    let axis = Plane::from_normal(DVec3::new(0.01, 0.01, -0.001), DVec3::Z);
    let hole = cut(&b, &make_cylinder(&axis, 0.004, 0.01, std::f64::consts::TAU).unwrap()).unwrap().shape;
    let sink = Plane::from_normal(DVec3::new(0.01, 0.01, 0.004), DVec3::Z);
    cut(&hole, &make_cone(&sink, 0.004, 0.0055, 0.0015, std::f64::consts::TAU).unwrap()).unwrap().shape
}

fn mesh(s: &Shape, strips: bool) -> Tessellation {
    // SAFETY: the only test in this binary; nothing else reads the environment meanwhile.
    unsafe {
        if strips {
            std::env::remove_var("NOSHAPE_MESH_NO_STRIPS");
        } else {
            std::env::set_var("NOSHAPE_MESH_NO_STRIPS", "1");
        }
    }
    tessellate(s, 1e-3, 2.0 * std::f64::consts::TAU / 144.0, false).unwrap()
}

fn face_points(t: &Tessellation, f: usize) -> Vec<DVec3> {
    let r = t.faces[f];
    t.positions[r.first_vertex as usize..(r.first_vertex + r.vertex_count) as usize].iter().map(|p| DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64)).collect()
}

fn face_triangles(t: &Tessellation, f: usize) -> Vec<[DVec3; 3]> {
    let r = t.faces[f];
    let p = |i: u32| {
        let q = t.positions[i as usize];
        DVec3::new(q[0] as f64, q[1] as f64, q[2] as f64)
    };
    t.indices[r.first_index as usize..(r.first_index + r.index_count) as usize].chunks(3).map(|c| [p(c[0]), p(c[1]), p(c[2])]).collect()
}

/// The largest gap between a triangle's centroid and the surface (radius
/// about the hole's axis, the cone's growing with height).
fn worst_gap(tris: &[[DVec3; 3]], radius: impl Fn(f64) -> f64) -> f64 {
    tris.iter()
        .map(|t| {
            let c = (t[0] + t[1] + t[2]) / 3.0;
            (radius(c.z) - DVec3::new(c.x - 0.01, c.y - 0.01, 0.0).length()).abs()
        })
        .fold(0.0, f64::max)
}

#[test]
fn strips_keep_brepmeshs_edges_sampling_and_tolerance() {
    let s = plate();
    let faces = s.faces();
    let (strip, delaunay) = (mesh(&s, true), mesh(&s, false));
    assert_eq!(strip.faces.len(), faces.len());
    assert_eq!(strip.edges.len(), delaunay.edges.len());
    for i in 0..strip.edges.len() {
        let (a, b) = (strip.edge_polyline(i), delaunay.edge_polyline(i));
        assert_eq!(a.len(), b.len(), "edge {i}");
        for (p, q) in a.iter().zip(b) {
            assert!((0..3).all(|k| (p[k] - q[k]).abs() < 1e-7), "edge {i}: {p:?} vs {q:?}");
        }
    }
    let mut checked = 0;
    for (f, face) in faces.iter().enumerate() {
        let radius: Box<dyn Fn(f64) -> f64> = match face.face_surface().unwrap() {
            Surface::Cylinder { .. } => Box::new(|_| 0.004),
            Surface::Cone { .. } => Box::new(|z| 0.004 + (z - 0.004)),
            _ => continue,
        };
        checked += 1;
        let (sp, dp) = (face_points(&strip, f), face_points(&delaunay, f));
        for p in &sp {
            assert!(dp.iter().any(|q| q.distance(*p) < 1e-7), "face {f}: strip node {p} isn't one of BRepMesh's");
        }
        let (st, dt) = (face_triangles(&strip, f), face_triangles(&delaunay, f));
        assert!(st.len() <= dt.len(), "face {f}: {} strip triangles vs {}", st.len(), dt.len());
        let (sg, dg) = (worst_gap(&st, &radius), worst_gap(&dt, &radius));
        assert!(sg <= dg * 1.0001 + 1e-9, "face {f}: strip deviation {sg:e} vs BRepMesh {dg:e}");
    }
    assert_eq!(checked, 2, "the hole and the countersink");

    // BRepMesh's own post-processing: the same triangles and edge polygons.
    // SAFETY: as in `mesh`.
    unsafe { std::env::set_var("NOSHAPE_MESH_BREPMESH_POST", "1") };
    let theirs = mesh(&s, true);
    unsafe { std::env::remove_var("NOSHAPE_MESH_BREPMESH_POST") };
    assert_eq!((theirs.positions.len(), &theirs.indices), (strip.positions.len(), &strip.indices));
    assert_eq!((theirs.edges.len(), &theirs.edge_points), (strip.edges.len(), &strip.edge_points));
}
