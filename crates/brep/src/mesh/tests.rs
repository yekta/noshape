use super::*;

/// A square of side `s` at height `z` (vertices `base..base+4`, edges `ebase..ebase+4`), counter-clockwise
/// seen from +z.
fn square(vertices: &mut Vec<DVec3>, edges: &mut Vec<MeshEdge>, c: DVec3, s: f64) -> Vec<Use> {
    let base = vertices.len() as u32;
    let h = s / 2.0;
    vertices.extend([c + DVec3::new(-h, -h, 0.0), c + DVec3::new(h, -h, 0.0), c + DVec3::new(h, h, 0.0), c + DVec3::new(-h, h, 0.0)]);
    (0..4)
        .map(|k| {
            let e = edges.len() as u32;
            edges.push(MeshEdge { curve: Some(Curve::Line), v: [base + k, base + (k + 1) % 4], t0: 0.0, t1: s });
            Use { edge: e, reversed: false }
        })
        .collect()
}

fn area(m: &FaceMesh, normal: DVec3) -> f64 {
    m.triangles.iter().map(|t| (m.points[t[1] as usize] - m.points[t[0] as usize]).cross(m.points[t[2] as usize] - m.points[t[0] as usize]).dot(normal) / 2.0).sum()
}

const DEFL: Deflection = Deflection { linear: 1.0, angular: 0.1 };

#[test]
fn a_plane_with_a_hole_is_its_boundary_nodes_triangulated() {
    let (mut v, mut e) = (Vec::new(), Vec::new());
    let outer = square(&mut v, &mut e, DVec3::ZERO, 4.0);
    // The hole runs clockwise seen from +z: its uses reversed, in reverse order.
    let inner: Vec<Use> = square(&mut v, &mut e, DVec3::new(0.5, 0.0, 0.0), 1.0).into_iter().rev().map(|u| Use { edge: u.edge, reversed: true }).collect();
    let normal = DVec3::Z;
    let loops = vec![outer, inner];
    let face = MeshFace { surface: MeshSurface::Plane { origin: DVec3::ZERO, normal, xdir: DVec3::X }, loops: loops.clone() };
    let out = mesh_body(&v, &e, &[Some(face)], DEFL);
    let m = out.faces[0].as_ref().expect("meshed");
    // V + 2H − 2 triangles, all counter-clockwise about the normal, covering the face.
    assert_eq!(m.points.len(), 8);
    assert_eq!(m.triangles.len(), 8);
    assert!(m.triangles.iter().all(|t| (m.points[t[1] as usize] - m.points[t[0] as usize]).cross(m.points[t[2] as usize] - m.points[t[0] as usize]).dot(normal) > 0.0));
    assert!((area(m, normal) - 15.0).abs() < 1e-12);
    assert!(m.normals.iter().all(|n| *n == normal));
    // Each edge's nodes, from v[0] to v[1], are mesh nodes at the edge's vertices.
    for (l, uses) in m.edge_nodes.iter().enumerate() {
        for (k, nodes) in uses.iter().enumerate() {
            let edge = &e[loops[l][k].edge as usize];
            assert_eq!(m.points[nodes[0] as usize], v[edge.v[0] as usize]);
            assert_eq!(m.points[*nodes.last().unwrap() as usize], v[edge.v[1] as usize]);
        }
    }
    assert!(out.edges.iter().all(|n| n.as_ref().is_some_and(|n| n.points.len() == 2)));
}

#[test]
fn a_loop_whose_uses_dont_join_is_declined() {
    let (mut v, mut e) = (Vec::new(), Vec::new());
    let mut outer = square(&mut v, &mut e, DVec3::ZERO, 4.0);
    outer.remove(1);
    let face = MeshFace { surface: MeshSurface::Plane { origin: DVec3::ZERO, normal: DVec3::Z, xdir: DVec3::X }, loops: vec![outer] };
    assert!(mesh_body(&v, &e, &[Some(face)], DEFL).faces[0].is_none());
}

#[test]
fn uses_listed_out_of_order_are_chained() {
    let (mut v, mut e) = (Vec::new(), Vec::new());
    let mut outer = square(&mut v, &mut e, DVec3::ZERO, 4.0);
    outer.swap(1, 2);
    let face = MeshFace { surface: MeshSurface::Plane { origin: DVec3::ZERO, normal: DVec3::Z, xdir: DVec3::X }, loops: vec![outer.clone()] };
    let m = mesh_body(&v, &e, &[Some(face)], DEFL).faces.remove(0).expect("meshed");
    assert!((area(&m, DVec3::Z) - 16.0).abs() < 1e-12);
    // Edge nodes listed per use as given.
    for (k, u) in outer.iter().enumerate() {
        assert_eq!(m.points[m.edge_nodes[0][k][0] as usize], v[e[u.edge as usize].v[0] as usize]);
    }
}

#[test]
fn a_reversed_use_lists_its_edge_nodes_from_the_edge_start() {
    let (mut v, mut e) = (Vec::new(), Vec::new());
    // The square's loop run clockwise (a face seen from below).
    let cw: Vec<Use> = square(&mut v, &mut e, DVec3::ZERO, 2.0).into_iter().rev().map(|u| Use { edge: u.edge, reversed: true }).collect();
    let face = MeshFace { surface: MeshSurface::Plane { origin: DVec3::ZERO, normal: -DVec3::Z, xdir: DVec3::X }, loops: vec![cw.clone()] };
    let m = mesh_body(&v, &e, &[Some(face)], DEFL).faces.remove(0).expect("meshed");
    assert!((area(&m, -DVec3::Z) - 4.0).abs() < 1e-12);
    for (k, u) in cw.iter().enumerate() {
        let edge = &e[u.edge as usize];
        let nodes = &m.edge_nodes[0][k];
        assert_eq!(m.points[nodes[0] as usize], v[edge.v[0] as usize]);
        assert_eq!(m.points[nodes[1] as usize], v[edge.v[1] as usize]);
    }
}
