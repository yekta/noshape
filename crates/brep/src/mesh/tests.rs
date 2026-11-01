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
    let out = mesh_body(&v, &e, &[face], DEFL);
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
fn a_hole_listed_before_the_outer_loop_is_still_a_hole() {
    let (mut v, mut e) = (Vec::new(), Vec::new());
    let outer = square(&mut v, &mut e, DVec3::ZERO, 4.0);
    let inner: Vec<Use> = square(&mut v, &mut e, DVec3::new(0.5, 0.0, 0.0), 1.0).into_iter().rev().map(|u| Use { edge: u.edge, reversed: true }).collect();
    let face = MeshFace { surface: MeshSurface::Plane { origin: DVec3::ZERO, normal: DVec3::Z, xdir: DVec3::X }, loops: vec![inner, outer] };
    let m = mesh_body(&v, &e, &[face], DEFL).faces.remove(0).expect("meshed");
    assert!((area(&m, DVec3::Z) - 15.0).abs() < 1e-12);
}

#[test]
fn a_loop_whose_uses_dont_join_is_declined() {
    let (mut v, mut e) = (Vec::new(), Vec::new());
    let mut outer = square(&mut v, &mut e, DVec3::ZERO, 4.0);
    outer.remove(1);
    let face = MeshFace { surface: MeshSurface::Plane { origin: DVec3::ZERO, normal: DVec3::Z, xdir: DVec3::X }, loops: vec![outer] };
    assert!(mesh_body(&v, &e, &[face], DEFL).faces[0].is_none());
}

#[test]
fn uses_listed_out_of_order_are_chained() {
    let (mut v, mut e) = (Vec::new(), Vec::new());
    let mut outer = square(&mut v, &mut e, DVec3::ZERO, 4.0);
    outer.swap(1, 2);
    let face = MeshFace { surface: MeshSurface::Plane { origin: DVec3::ZERO, normal: DVec3::Z, xdir: DVec3::X }, loops: vec![outer.clone()] };
    let m = mesh_body(&v, &e, &[face], DEFL).faces.remove(0).expect("meshed");
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
    let m = mesh_body(&v, &e, &[face], DEFL).faces.remove(0).expect("meshed");
    assert!((area(&m, -DVec3::Z) - 4.0).abs() < 1e-12);
    for (k, u) in cw.iter().enumerate() {
        let edge = &e[u.edge as usize];
        let nodes = &m.edge_nodes[0][k];
        assert_eq!(m.points[nodes[0] as usize], v[edge.v[0] as usize]);
        assert_eq!(m.points[nodes[1] as usize], v[edge.v[1] as usize]);
    }
}

/// A cylinder of radius `r` and height `h` on the z axis: the bottom and top circles (closed at their seam
/// vertices) and the seam, and its face loop as OCCT has it (bottom, seam up, top back, seam down).
fn cylinder(r: f64, h: f64) -> (Vec<DVec3>, Vec<MeshEdge>, MeshFace) {
    let v = vec![DVec3::new(r, 0.0, 0.0), DVec3::new(r, 0.0, h)];
    let circle = |z: f64| Some(Curve::Circle { center: DVec3::new(0.0, 0.0, z), normal: DVec3::Z, xdir: DVec3::X, radius: r });
    let e = vec![
        MeshEdge { curve: circle(0.0), v: [0, 0], t0: 0.0, t1: std::f64::consts::TAU },
        MeshEdge { curve: circle(h), v: [1, 1], t0: 0.0, t1: std::f64::consts::TAU },
        MeshEdge { curve: Some(Curve::Line), v: [0, 1], t0: 0.0, t1: h },
    ];
    let face = MeshFace {
        surface: MeshSurface::Cylinder { origin: DVec3::ZERO, axis: DVec3::Z, xdir: DVec3::X, direct: true, radius: r },
        loops: vec![vec![Use { edge: 0, reversed: false }, Use { edge: 2, reversed: false }, Use { edge: 1, reversed: true }, Use { edge: 2, reversed: true }]],
    };
    (v, e, face)
}

#[test]
fn a_circle_is_split_as_brepmesh_splits_it() {
    let (v, e, _) = cylinder(2.0, 1.0);
    // The display's Auto: half of 2·2π/144 a step, the chord limit far looser: 144 segments.
    let n = edge_nodes(&v, &e[0], Deflection { linear: 10.0, angular: 2.0 * std::f64::consts::TAU / 144.0 }).unwrap();
    assert_eq!(n.points.len(), 145);
    assert_eq!((n.points[0], n.points[144]), (v[0], v[0]));
    // A tight chord deflection: 2 acos(1 − d/2r) a step.
    let n = edge_nodes(&v, &e[0], Deflection { linear: 0.02, angular: 1.0 }).unwrap();
    let step = 2.0 * (1.0f64 - 0.01 / 2.0).acos();
    assert_eq!(n.points.len() - 1, (std::f64::consts::TAU / step).ceil() as usize);
    // At least three segments.
    assert_eq!(edge_nodes(&v, &e[0], Deflection { linear: 100.0, angular: 10.0 }).unwrap().points.len(), 4);
}

#[test]
fn a_cylinder_between_two_circles_is_its_rims_zipped() {
    let (r, h) = (2.0, 1.0);
    let (v, e, face) = cylinder(r, h);
    let m = mesh_body(&v, &e, std::slice::from_ref(&face), DEFL).faces.remove(0).expect("meshed");
    // DEFL's 0.1 rad: 0.05 a step, 126 segments a rim, both rims with their seam node twice.
    let n = (std::f64::consts::TAU / 0.05).ceil() as usize;
    assert_eq!(m.points.len(), 2 * (n + 1));
    assert_eq!(m.triangles.len(), 2 * n);
    // The inscribed prism's side, normals outward, triangles turned like them.
    let chord = 2.0 * r * (std::f64::consts::PI / n as f64).sin();
    let area: f64 = m.triangles.iter().map(|t| (m.points[t[1] as usize] - m.points[t[0] as usize]).cross(m.points[t[2] as usize] - m.points[t[0] as usize]).length() / 2.0).sum();
    assert!((area - n as f64 * chord * h).abs() < 1e-9);
    for t in &m.triangles {
        let (a, b, c) = (m.points[t[0] as usize], m.points[t[1] as usize], m.points[t[2] as usize]);
        let tn = (b - a).cross(c - a);
        assert!(tn.dot(m.normals[t[0] as usize]) > 0.0, "a triangle against its normal");
        let mid = (a + b + c) / 3.0;
        assert!(tn.dot(DVec3::new(mid.x, mid.y, 0.0)) > 0.0, "the normal points inward");
    }
    // The seam's two uses: the same points, different nodes (u = 2π and u = 0).
    let (up, down) = (&m.edge_nodes[0][1], &m.edge_nodes[0][3]);
    assert_eq!(up.len(), 2);
    assert_ne!(up, down);
    for k in 0..2 {
        assert!((m.points[up[k] as usize] - m.points[down[k] as usize]).length() < 1e-12);
    }
    // A left-handed frame: the normal points inward, the triangles turn the other way.
    let mut inward = face;
    inward.surface = MeshSurface::Cylinder { origin: DVec3::ZERO, axis: DVec3::Z, xdir: DVec3::X, direct: false, radius: r };
    inward.loops = vec![inward.loops[0].iter().rev().map(|u| Use { edge: u.edge, reversed: !u.reversed }).collect()];
    let m = mesh_body(&v, &e, &[inward], DEFL).faces.remove(0).expect("meshed");
    let t = m.triangles[0];
    let (a, b, c) = (m.points[t[0] as usize], m.points[t[1] as usize], m.points[t[2] as usize]);
    assert!((b - a).cross(c - a).dot(m.normals[t[0] as usize]) > 0.0);
    assert!(m.normals[0].dot(DVec3::new(m.points[0].x, m.points[0].y, 0.0)) < 0.0);
}

#[test]
fn a_plane_with_a_round_hole_shares_the_circles_nodes() {
    let (mut v, mut e) = (Vec::new(), Vec::new());
    let outer = square(&mut v, &mut e, DVec3::ZERO, 10.0);
    let c = v.len() as u32;
    v.push(DVec3::new(1.0, 0.0, 0.0));
    let ce = e.len() as u32;
    e.push(MeshEdge { curve: Some(Curve::Circle { center: DVec3::ZERO, normal: DVec3::Z, xdir: DVec3::X, radius: 1.0 }), v: [c, c], t0: 0.0, t1: std::f64::consts::TAU });
    let face = MeshFace { surface: MeshSurface::Plane { origin: DVec3::ZERO, normal: DVec3::Z, xdir: DVec3::X }, loops: vec![outer, vec![Use { edge: ce, reversed: true }]] };
    let out = mesh_body(&v, &e, &[face], DEFL);
    let m = out.faces[0].as_ref().expect("meshed");
    let circle = out.edges[ce as usize].as_ref().unwrap();
    assert_eq!(m.edge_nodes[1][0].len(), circle.points.len());
    for (k, &i) in m.edge_nodes[1][0].iter().enumerate() {
        assert_eq!(m.points[i as usize], circle.points[k]);
    }
    let n = circle.points.len() - 1;
    let hole = n as f64 * (std::f64::consts::TAU / n as f64).sin() / 2.0;
    assert!((area(m, DVec3::Z) - (100.0 - hole)).abs() < 1e-9);
}

#[test]
fn a_face_on_a_circle_a_declined_face_shares_is_declined() {
    let (v, e, cyl) = cylinder(2.0, 1.0);
    let disc = |surface| MeshFace { surface, loops: vec![vec![Use { edge: 0, reversed: true }]] };
    // The bottom disc meshed here too: both taken.
    let both = mesh_body(&v, &e, &[cyl.clone(), disc(MeshSurface::Plane { origin: DVec3::ZERO, normal: -DVec3::Z, xdir: DVec3::X })], DEFL);
    assert!(both.faces.iter().all(|f| f.is_some()));
    // The disc on a surface BRepMesh meshes: the cylinder on its circle is declined too (BRepMesh would split
    // the circle for the disc, not sharing our nodes).
    let out = mesh_body(&v, &e, &[cyl, disc(MeshSurface::Other)], DEFL);
    assert!(out.faces.iter().all(|f| f.is_none()));
}

/// A plate face with ten round holes of 144 nodes each (live-30's top face has ~1450 boundary nodes): the
/// triangulation alone, 200 times. `cargo test --release -p noshape-brep bench_cdt -- --ignored --nocapture`
#[test]
#[ignore = "bench"]
fn bench_cdt_plate_with_holes() {
    use std::f64::consts::TAU;
    let mut pts = vec![cdt::Pt { x: 0.0, y: 0.0 }, cdt::Pt { x: 100.0, y: 0.0 }, cdt::Pt { x: 100.0, y: 40.0 }, cdt::Pt { x: 0.0, y: 40.0 }];
    let mut rings = vec![vec![0, 1, 2, 3]];
    for h in 0..10 {
        let (cx, cy) = (10.0 + 20.0 * (h % 5) as f64, if h < 5 { 10.0 } else { 30.0 });
        let base = pts.len();
        pts.extend((0..144).map(|k| {
            let a = TAU * k as f64 / 144.0;
            cdt::Pt { x: cx + 6.0 * a.cos(), y: cy + 6.0 * a.sin() }
        }));
        rings.push((base..base + 144).collect());
    }
    let t = std::time::Instant::now();
    for _ in 0..200 {
        assert!(cdt::triangulate(&pts, &rings).is_some());
    }
    eprintln!("cdt: {} points, {:.3} ms each", pts.len(), t.elapsed().as_secs_f64() * 1e3 / 200.0);
}

#[test]
fn a_big_face_met_again_takes_its_last_triangulation() {
    use std::f64::consts::TAU;
    // A plate round ten holes of 40 nodes (≥ 256 nodes: cached), then the same plate moved (an edit moved the face
    // along its normal: the same outline, its coordinates a little off in the last bits).
    let plate = |dx: f64, dy: f64| {
        let mut pts = vec![cdt::Pt { x: dx, y: dy }, cdt::Pt { x: 100.0 + dx, y: dy }, cdt::Pt { x: 100.0 + dx, y: 40.0 + dy }, cdt::Pt { x: dx, y: 40.0 + dy }];
        let mut rings = vec![vec![0, 1, 2, 3]];
        for h in 0..10 {
            let (cx, cy) = (10.0 + 20.0 * (h % 5) as f64 + dx, if h < 5 { 10.0 } else { 30.0 } + dy);
            let base = pts.len();
            pts.extend((0..40).map(|k| {
                let a = TAU * k as f64 / 40.0;
                cdt::Pt { x: cx + 6.0 * a.cos(), y: cy + 6.0 * a.sin() }
            }));
            rings.push((base..base + 40).rev().collect());
        }
        (pts, rings)
    };
    let (pts, rings) = plate(0.0, 0.0);
    let first = cdt::triangulate(&pts, &rings).expect("clean");
    let hits = cdt::cache_hits();
    let (moved, rings) = plate(1e-9, 0.0);
    let again = cdt::triangulate(&moved, &rings).expect("clean");
    assert!(cdt::cache_hits() > hits, "the second one came from the cache");
    assert_eq!(first, again);
}

/// MESH-EMPTY-LOOP-PANIC: a face with a loop of no edges (CLI fuzz seed 355's mirror) is declined, not a panic.
#[test]
fn a_loop_with_no_edges_is_declined() {
    let (mut v, mut e) = (Vec::new(), Vec::new());
    let outer = square(&mut v, &mut e, DVec3::ZERO, 4.0);
    let plane = MeshSurface::Plane { origin: DVec3::ZERO, normal: DVec3::Z, xdir: DVec3::X };
    let faces = [MeshFace { surface: plane, loops: vec![outer, Vec::new()] }, MeshFace { surface: plane, loops: vec![Vec::new()] }];
    let out = mesh_body(&v, &e, &faces, DEFL);
    assert!(out.faces.iter().all(Option::is_none));
}

/// A B-spline face (lead 11-01: the imports' and the knurl's faces in noshape's mesher): a 100 × 50 mm patch arched
/// 20 mm high along x (quadratic in u, linear in v), its long sides B-spline edges, its ends lines. It is meshed with
/// interior nodes, every triangle's centre within the linear deflection (plus the chord's own share) of the
/// surface, every normal up, each edge's nodes its loop's.
#[test]
fn a_bspline_face_is_meshed_within_the_deflection() {
    use crate::nurbs::{NurbsCurve, NurbsSurface};
    // A dome over 100 x 50 mm: quadratic both ways, its four sides the surface's boundary curves.
    let (x, y, zu, zv) = ([0.0, 0.05, 0.1], [0.0, 0.025, 0.05], [0.0, 0.02, 0.0], [0.0, 0.01, 0.0]);
    let pole = |i: usize, j: usize| DVec3::new(x[i], y[j], zu[i] + zv[j]);
    let q = vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0];
    let surface = NurbsSurface { udegree: 2, vdegree: 2, uknots: q.clone(), vknots: q.clone(), nu: 3, nv: 3, poles: (0..3).flat_map(|i| (0..3).map(move |j| pole(i, j))).collect(), weights: Vec::new() };
    let curve = |poles: Vec<DVec3>| NurbsCurve { degree: 2, knots: q.clone(), poles, weights: Vec::new(), spans: Default::default() };
    let along_u = |j: usize| curve((0..3).map(|i| pole(i, j)).collect());
    let along_v = |i: usize| curve((0..3).map(|j| pole(i, j)).collect());
    let curves = [along_u(0), along_v(2), along_u(2), along_v(0)];
    let surfaces = [surface.clone()];
    let v = vec![pole(0, 0), pole(2, 0), pole(2, 2), pole(0, 2)];
    let nurbs = |index: u32, a: u32, b: u32| MeshEdge { curve: Some(Curve::Nurbs { index, t0: 0.0, t1: 1.0 }), v: [a, b], t0: 0.0, t1: 1.0 };
    let e = vec![nurbs(0, 0, 1), nurbs(1, 1, 2), nurbs(2, 3, 2), nurbs(3, 0, 3)];
    let face = MeshFace { surface: MeshSurface::Nurbs { index: 0 }, loops: vec![vec![Use { edge: 0, reversed: false }, Use { edge: 1, reversed: false }, Use { edge: 2, reversed: true }, Use { edge: 3, reversed: true }]] };
    let defl = Deflection { linear: 1e-4, angular: 0.5 };
    let out = mesh_body_with(&v, &e, std::slice::from_ref(&face), defl, MeshNurbs { curves: &curves, surfaces: &surfaces });
    let m = out.faces[0].as_ref().expect("meshed");
    let boundary: usize = out.edges.iter().flatten().map(|n| n.points.len()).sum::<usize>() - 4;
    assert!(m.points.len() > boundary, "interior nodes: {} of {}", m.points.len(), boundary);
    let mut worst: f64 = 0.0;
    for t in &m.triangles {
        let c = (m.points[t[0] as usize] + m.points[t[1] as usize] + m.points[t[2] as usize]) / 3.0;
        worst = worst.max(surface.param_of(c, None).1);
    }
    assert!(worst <= 2.0 * defl.linear, "a triangle {worst} m off the surface");
    assert!(m.normals.iter().all(|n| n.z > 0.0), "normals up");
    for t in &m.triangles {
        let (a, b, c) = (m.points[t[0] as usize], m.points[t[1] as usize], m.points[t[2] as usize]);
        assert!((b - a).cross(c - a).z > 0.0, "counter-clockwise about the surface normal");
    }
    let (n0, n2) = (out.edges[0].as_ref().unwrap(), out.edges[2].as_ref().unwrap());
    assert_eq!(m.edge_nodes[0][0].len(), n0.points.len());
    assert_eq!(m.edge_nodes[0][2].len(), n2.points.len());
}

#[test]
fn a_cylinder_piece_under_a_slanted_cut_is_meshed_as_a_grid() {
    use crate::nurbs::NurbsCurve;
    // A cylinder's piece from u 0 to 1.2 above z 0, under the plane z = 0.005 + 0.3 x: its top an ellipse's arc
    // (the circle's arc as a rational quadratic, lifted onto the plane), not a rim, so not a strip.
    let (r, th) = (0.01f64, 1.2f64);
    let lift = |p: DVec3| DVec3::new(p.x, p.y, 0.005 + 0.3 * p.x);
    let arc = [DVec3::new(r, 0.0, 0.0), DVec3::new(r, r * (th / 2.0).tan(), 0.0), DVec3::new(r * th.cos(), r * th.sin(), 0.0)];
    let top = NurbsCurve { degree: 2, knots: vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0], poles: arc.iter().map(|&p| lift(p)).collect(), weights: vec![1.0, (th / 2.0).cos(), 1.0], spans: Default::default() };
    let v = vec![arc[0], arc[2], lift(arc[2]), lift(arc[0])];
    let e = vec![
        MeshEdge { curve: Some(Curve::Circle { center: DVec3::ZERO, normal: DVec3::Z, xdir: DVec3::X, radius: r }), v: [0, 1], t0: 0.0, t1: th },
        MeshEdge { curve: Some(Curve::Line), v: [1, 2], t0: 0.0, t1: v[2].z },
        MeshEdge { curve: Some(Curve::Nurbs { index: 0, t0: 0.0, t1: 1.0 }), v: [3, 2], t0: 0.0, t1: 1.0 },
        MeshEdge { curve: Some(Curve::Line), v: [3, 0], t0: 0.0, t1: v[3].z },
    ];
    let face = MeshFace {
        surface: MeshSurface::Cylinder { origin: DVec3::ZERO, axis: DVec3::Z, xdir: DVec3::X, direct: true, radius: r },
        loops: vec![vec![Use { edge: 0, reversed: false }, Use { edge: 1, reversed: false }, Use { edge: 2, reversed: true }, Use { edge: 3, reversed: false }]],
    };
    let defl = Deflection { linear: 1e-5, angular: 0.5 };
    let curves = [top];
    let out = mesh_body_with(&v, &e, std::slice::from_ref(&face), defl, MeshNurbs { curves: &curves, surfaces: &[] });
    let m = out.faces[0].as_ref().expect("meshed");
    for t in &m.triangles {
        let (a, b, c) = (m.points[t[0] as usize], m.points[t[1] as usize], m.points[t[2] as usize]);
        let mid = (a + b + c) / 3.0;
        assert!(r - mid.truncate().length() <= 2.0 * defl.linear, "a triangle {} m inside the surface", r - mid.truncate().length());
        let tn = (b - a).cross(c - a);
        assert!(tn.dot(DVec3::new(mid.x, mid.y, 0.0)) > 0.0, "a triangle turned inward");
        assert!(tn.dot(m.normals[t[0] as usize]) > 0.0, "a triangle against its normal");
    }
    for (p, n) in m.points.iter().zip(&m.normals) {
        assert!((r - p.truncate().length()).abs() < 1e-9, "a node off the cylinder");
        assert!((n.dot(DVec3::new(p.x, p.y, 0.0).normalize()) - 1.0).abs() < 1e-9, "a normal not the cylinder's");
    }
    assert_eq!(m.edge_nodes[0][2].len(), out.edges[2].as_ref().unwrap().points.len());
}

#[test]
fn a_whole_cylinder_under_a_slanted_cut_is_meshed_across_its_seam() {
    use crate::nurbs::NurbsCurve;
    // The whole turn under z = 0.005 + 0.3 x: its top a whole ellipse (the circle as nine rational poles,
    // lifted), its seam at u 0 used both ways.
    let r = 0.01f64;
    let lift = |x: f64, y: f64| DVec3::new(x, y, 0.005 + 0.3 * x);
    let corners = [(1.0, 0.0), (1.0, 1.0), (0.0, 1.0), (-1.0, 1.0), (-1.0, 0.0), (-1.0, -1.0), (0.0, -1.0), (1.0, -1.0), (1.0, 0.0)];
    let w = std::f64::consts::FRAC_1_SQRT_2;
    let top = NurbsCurve {
        degree: 2,
        knots: vec![0.0, 0.0, 0.0, 0.25, 0.25, 0.5, 0.5, 0.75, 0.75, 1.0, 1.0, 1.0],
        poles: corners.iter().map(|&(x, y)| lift(r * x, r * y)).collect(),
        weights: (0..9).map(|k| if k % 2 == 1 { w } else { 1.0 }).collect(),
        spans: Default::default(),
    };
    let v = vec![DVec3::new(r, 0.0, 0.0), lift(r, 0.0)];
    let e = vec![
        MeshEdge { curve: Some(Curve::Circle { center: DVec3::ZERO, normal: DVec3::Z, xdir: DVec3::X, radius: r }), v: [0, 0], t0: 0.0, t1: std::f64::consts::TAU },
        MeshEdge { curve: Some(Curve::Nurbs { index: 0, t0: 0.0, t1: 1.0 }), v: [1, 1], t0: 0.0, t1: 1.0 },
        MeshEdge { curve: Some(Curve::Line), v: [0, 1], t0: 0.0, t1: v[1].z },
    ];
    let face = MeshFace {
        surface: MeshSurface::Cylinder { origin: DVec3::ZERO, axis: DVec3::Z, xdir: DVec3::X, direct: true, radius: r },
        loops: vec![vec![Use { edge: 0, reversed: false }, Use { edge: 2, reversed: false }, Use { edge: 1, reversed: true }, Use { edge: 2, reversed: true }]],
    };
    let defl = Deflection { linear: 1e-5, angular: 0.5 };
    let curves = [top];
    let out = mesh_body_with(&v, &e, std::slice::from_ref(&face), defl, MeshNurbs { curves: &curves, surfaces: &[] });
    let m = out.faces[0].as_ref().expect("meshed");
    let mut area = 0.0;
    for t in &m.triangles {
        let (a, b, c) = (m.points[t[0] as usize], m.points[t[1] as usize], m.points[t[2] as usize]);
        let mid = (a + b + c) / 3.0;
        assert!(r - mid.truncate().length() <= 2.0 * defl.linear, "a triangle {} m inside the surface", r - mid.truncate().length());
        let tn = (b - a).cross(c - a);
        assert!(tn.dot(DVec3::new(mid.x, mid.y, 0.0)) > 0.0, "a triangle turned inward");
        area += tn.length() / 2.0;
    }
    // The side's area: the mean height (0.005) round the whole turn, less the chords' share.
    let whole = std::f64::consts::TAU * r * 0.005;
    assert!(area < whole && area > whole * (1.0 - 1e-2), "area {area} of {whole}");
    // The seam's two uses: the same points, different nodes.
    let (up, down) = (&m.edge_nodes[0][1], &m.edge_nodes[0][3]);
    assert_eq!(up.len(), 2);
    assert_ne!(up, down);
}
