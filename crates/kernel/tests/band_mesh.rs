//! Curved faces bounded by long rails of edge nodes (thread bands: a tooth
//! swept along a helix) are meshed by triangulating their boundary nodes in
//! the parameters scaled to lengths (mesh.cpp `BandCdtAlgo`), instead of
//! BRepMesh's Delaunay: the same edges, no nodes but BRepMesh's edge nodes,
//! no more triangles, and every triangle within the deflection of the
//! thread's surface. One test in this file: it switches NOSHAPE_MESH_NO_BANDS.

use glam::{DVec2, DVec3};
use noshape_kernel::*;

const MM: f64 = 1e-3;
const DEFLECTION: f64 = 0.01 * MM;
const PITCH: f64 = 1.0 * MM;
/// The tooth profile in the plane y = 0 (x radial, z axial), as in thread.rs.
const PROFILE: [(f64, f64); 6] = [(4.55, 20.1), (4.55, 20.9), (4.7, 20.9), (5.0, 20.6), (5.0, 20.4), (4.7, 20.1)];

/// Y Threads' tooth (tests/thread.rs) swept 8 turns down a Ø10 helix: two
/// helix edges of 1152 nodes each on every helical face.
fn teeth() -> Shape {
    let frame = Plane::new(DVec3::new(0.0, 0.0, 20.5) * MM, -DVec3::Z, DVec3::X);
    let helix = make_helix(&frame, 5.0 * MM, PITCH, 8.0 * MM, true, 0.0).unwrap();
    let p = |(x, z): (f64, f64)| DVec3::new(x, 0.0, z) * MM;
    let edges: Vec<Shape> = (0..PROFILE.len()).map(|i| make_line(p(PROFILE[i]), p(PROFILE[(i + 1) % PROFILE.len()])).unwrap()).collect();
    let face = make_face_from_wire(&make_wire(&edges).unwrap(), true).unwrap();
    sweep(&face, &helix, &SweepOptions { solid: true, ..Default::default() }).unwrap().shape
}

fn mesh(s: &Shape, bands: bool) -> Tessellation {
    // SAFETY: the only test in this binary; nothing else reads the environment meanwhile.
    unsafe {
        if bands {
            std::env::remove_var("NOSHAPE_MESH_NO_BANDS");
        } else {
            std::env::set_var("NOSHAPE_MESH_NO_BANDS", "1");
        }
    }
    tessellate(s, DEFLECTION, std::f64::consts::TAU / 144.0, false).unwrap()
}

fn point(q: [f32; 3]) -> DVec3 {
    DVec3::new(q[0] as f64, q[1] as f64, q[2] as f64)
}

fn face_points(t: &Tessellation, f: usize) -> Vec<DVec3> {
    let r = t.faces[f];
    t.positions[r.first_vertex as usize..(r.first_vertex + r.vertex_count) as usize].iter().map(|&p| point(p)).collect()
}

fn face_triangles(t: &Tessellation, f: usize) -> Vec<[DVec3; 3]> {
    let r = t.faces[f];
    t.indices[r.first_index as usize..(r.first_index + r.index_count) as usize].chunks(3).map(|c| [point(t.positions[c[0] as usize]), point(t.positions[c[1] as usize]), point(t.positions[c[2] as usize])]).collect()
}

/// Distance from `p` to the tooth's surface: unwound by the screw motion
/// (`sign`: the helix's sense) onto the plane y = 0, the distance in that
/// plane to the profile's outline, over the turns that could hold it.
fn off_surface(p: DVec3, sign: f64) -> f64 {
    let r = DVec2::new(p.x, p.y).length();
    let turn = p.y.atan2(p.x) / std::f64::consts::TAU;
    let z0 = p.z + sign * PITCH * turn;
    let seg = |q: DVec2, a: DVec2, b: DVec2| {
        let t = ((q - a).dot(b - a) / (b - a).length_squared()).clamp(0.0, 1.0);
        q.distance(a + (b - a) * t)
    };
    let k0 = ((20.5 * MM - z0) / PITCH).round() as i32;
    (k0 - 2..=k0 + 2)
        .map(|k| {
            let q = DVec2::new(r, z0 + k as f64 * PITCH);
            (0..PROFILE.len())
                .map(|i| {
                    let (a, b) = (PROFILE[i], PROFILE[(i + 1) % PROFILE.len()]);
                    seg(q, DVec2::new(a.0, a.1) * MM, DVec2::new(b.0, b.1) * MM)
                })
                .fold(f64::MAX, f64::min)
        })
        .fold(f64::MAX, f64::min)
}

#[test]
fn bands_keep_brepmeshs_edges_and_the_deflection() {
    let s = teeth();
    let (band, delaunay) = (mesh(&s, true), mesh(&s, false));
    assert_eq!(band.faces.len(), delaunay.faces.len());
    assert_eq!(band.edges.len(), delaunay.edges.len());
    // Edge nodes by their exact coordinates (face nodes are copies of them).
    let key = |p: [f32; 3]| p.map(f32::to_bits);
    let mut edge_points = std::collections::HashSet::new();
    for i in 0..band.edges.len() {
        let (a, b) = (band.edge_polyline(i), delaunay.edge_polyline(i));
        assert_eq!(a.len(), b.len(), "edge {i}");
        for (p, q) in a.iter().zip(b) {
            assert!((0..3).all(|k| (p[k] - q[k]).abs() < 1e-8), "edge {i}: {p:?} vs {q:?}");
        }
        edge_points.extend(a.iter().map(|&p| key(p)));
    }
    // The helix's sense: the one that puts BRepMesh's nodes on the tooth.
    let nodes: Vec<DVec3> = (0..delaunay.faces.len()).flat_map(|f| face_points(&delaunay, f)).collect();
    let worst = |sign: f64| nodes.iter().map(|&p| off_surface(p, sign)).fold(0.0, f64::max);
    let sign = if worst(1.0) < worst(-1.0) { 1.0 } else { -1.0 };
    // (f32 positions: ~2e-9 m steps at this size.)
    assert!(worst(sign) < 1e-4 * MM, "BRepMesh's nodes are {:e} m off the tooth", worst(sign));
    let mut banded = 0;
    for f in 0..band.faces.len() {
        let (bp, dp) = (face_points(&band, f), face_points(&delaunay, f));
        // The helical faces: thousands of edge nodes (BandCdtAlgo takes 1000+).
        if bp.len() < 1000 {
            continue;
        }
        banded += 1;
        // Only edge nodes, all of them BRepMesh's.
        let r = band.faces[f];
        for &p in &band.positions[r.first_vertex as usize..(r.first_vertex + r.vertex_count) as usize] {
            assert!(edge_points.contains(&key(p)), "face {f}: band node {p:?} isn't an edge node");
        }
        assert!(bp.len() <= dp.len(), "face {f}: {} band nodes vs {}", bp.len(), dp.len());
        let (bt, dt) = (face_triangles(&band, f), face_triangles(&delaunay, f));
        assert!(bt.len() <= dt.len(), "face {f}: {} band triangles vs {}", bt.len(), dt.len());
        let gap = bt.iter().map(|t| off_surface((t[0] + t[1] + t[2]) / 3.0, sign)).fold(0.0, f64::max);
        assert!(gap <= DEFLECTION, "face {f}: a band triangle is {gap:e} m off the tooth");
    }
    assert!(banded >= 4, "the tooth's helical faces ({banded})");
}
