use crate::*;
use glam::DVec3;

const TOL: f64 = 1e-7;

/// A prism: the polygon `base` (counter-clockwise seen from +z) at z0, extruded to z1.
fn prism(base: &[(f64, f64)], z0: f64, z1: f64) -> Body {
    let n = base.len();
    let mut b = Body::default();
    for &z in &[z0, z1] {
        for &(x, y) in base {
            b.vertices.push(DVec3::new(x, y, z));
        }
    }
    let n32 = n as u32;
    // Bottom edges 0..n, top edges n..2n, vertical edges 2n..3n.
    for k in 0..n32 {
        b.edges.push(Edge { v: [k, (k + 1) % n32], curve: Curve::Line });
    }
    for k in 0..n32 {
        b.edges.push(Edge { v: [n32 + k, n32 + (k + 1) % n32], curve: Curve::Line });
    }
    for k in 0..n32 {
        b.edges.push(Edge { v: [k, n32 + k], curve: Curve::Line });
    }
    let fw = |e: u32| Use { edge: e, reversed: false };
    let rv = |e: u32| Use { edge: e, reversed: true };
    b.faces.push(Face {
        surface: Surface::Plane { origin: DVec3::new(0.0, 0.0, z0), normal: DVec3::Z },
        reversed: true,
        loops: vec![(0..n32).rev().map(rv).collect()],
    });
    b.faces.push(Face { surface: Surface::Plane { origin: DVec3::new(0.0, 0.0, z1), normal: DVec3::Z }, reversed: false, loops: vec![(0..n32).map(|k| fw(n32 + k)).collect()] });
    for k in 0..n {
        let (a, c) = (base[k], base[(k + 1) % n]);
        let normal = DVec3::new(c.1 - a.1, a.0 - c.0, 0.0).normalize();
        let k32 = k as u32;
        b.faces.push(Face {
            surface: Surface::Plane { origin: DVec3::new(a.0, a.1, z0), normal },
            reversed: false,
            loops: vec![vec![fw(k32), fw(2 * n32 + (k32 + 1) % n32), rv(n32 + k32), rv(2 * n32 + k32)]],
        });
    }
    b.solids.push(Solid { shells: vec![(0..b.faces.len() as u32).collect()] });
    b
}

fn cuboid(lo: (f64, f64, f64), hi: (f64, f64, f64)) -> Body {
    prism(&[(lo.0, lo.1), (hi.0, lo.1), (hi.0, hi.1), (lo.0, hi.1)], lo.2, hi.2)
}

fn volume(b: &Body) -> f64 {
    let mut v = 0.0;
    for f in &b.faces {
        let Surface::Plane { origin, .. } = f.surface;
        for l in &f.loops {
            for u in l {
                let e = b.edges[u.edge as usize];
                let (p, q) = if u.reversed { (e.v[1], e.v[0]) } else { (e.v[0], e.v[1]) };
                v += origin.dot(b.vertices[p as usize].cross(b.vertices[q as usize]));
            }
        }
    }
    v / 6.0
}

/// Every edge used once each way, loops closed.
fn check_closed(b: &Body) {
    let mut uses = vec![[0, 0]; b.edges.len()];
    for f in &b.faces {
        for l in &f.loops {
            for (k, u) in l.iter().enumerate() {
                uses[u.edge as usize][u.reversed as usize] += 1;
                let end = |u: &Use| {
                    let e = b.edges[u.edge as usize];
                    if u.reversed { e.v[0] } else { e.v[1] }
                };
                let start = |u: &Use| {
                    let e = b.edges[u.edge as usize];
                    if u.reversed { e.v[1] } else { e.v[0] }
                };
                assert_eq!(end(u), start(&l[(k + 1) % l.len()]), "loop not closed");
            }
        }
    }
    assert!(uses.iter().all(|u| *u == [1, 1]), "edges not used once each way: {uses:?}");
}

fn counts(o: &Output) -> (usize, usize, usize, usize) {
    (o.body.solids.len(), o.body.faces.len(), o.body.edges.len(), o.body.vertices.len())
}

#[test]
fn a_prism_is_closed_and_has_its_volume() {
    let b = cuboid((0.0, 0.0, 0.0), (2.0, 3.0, 4.0));
    check_closed(&b);
    assert!((volume(&b) - 24.0).abs() < 1e-12);
}

#[test]
fn overlapping_boxes() {
    let a = cuboid((0.0, 0.0, 0.0), (2.0, 2.0, 2.0));
    let b = cuboid((1.0, 1.0, 1.0), (3.0, 3.0, 3.0));
    let fuse = boolean(Op::Fuse, &[&a, &b], TOL).unwrap();
    check_closed(&fuse.body);
    assert!((volume(&fuse.body) - 15.0).abs() < 1e-9);
    assert_eq!(counts(&fuse), (1, 12, 30, 20));
    let cut = boolean(Op::Cut, &[&a, &b], TOL).unwrap();
    check_closed(&cut.body);
    assert!((volume(&cut.body) - 7.0).abs() < 1e-9);
    assert_eq!(counts(&cut), (1, 9, 21, 14));
    let common = boolean(Op::Common, &[&a, &b], TOL).unwrap();
    check_closed(&common.body);
    assert!((volume(&common.body) - 1.0).abs() < 1e-9);
    assert_eq!(counts(&common), (1, 6, 12, 8));
}

#[test]
fn a_boss_on_a_face_and_a_pocket_in_it() {
    let base = cuboid((0.0, 0.0, 0.0), (4.0, 4.0, 1.0));
    let boss = cuboid((1.0, 1.0, 1.0), (2.0, 2.0, 2.0));
    let fuse = boolean(Op::Fuse, &[&base, &boss], TOL).unwrap();
    check_closed(&fuse.body);
    assert!((volume(&fuse.body) - 17.0).abs() < 1e-9);
    // The base's top gets a hole where the boss stands; the boss's bottom goes.
    assert_eq!(counts(&fuse), (1, 11, 24, 16));
    let pocket = cuboid((1.0, 1.0, 0.5), (2.0, 2.0, 1.5));
    let cut = boolean(Op::Cut, &[&base, &pocket], TOL).unwrap();
    check_closed(&cut.body);
    assert!((volume(&cut.body) - 15.5).abs() < 1e-9);
    assert_eq!(counts(&cut), (1, 11, 24, 16));
}

#[test]
fn a_hole_through_and_a_step() {
    let plate = cuboid((0.0, 0.0, 0.0), (4.0, 4.0, 1.0));
    let tool = prism(&[(1.0, 1.0), (2.0, 1.0), (1.5, 2.0)], -1.0, 2.0);
    let cut = boolean(Op::Cut, &[&plate, &tool], TOL).unwrap();
    check_closed(&cut.body);
    assert!((volume(&cut.body) - (16.0 - 0.5)).abs() < 1e-9);
    assert_eq!(counts(&cut), (1, 9, 21, 14));
    // A step off one edge: flush with three faces.
    let step = cuboid((3.0, 0.0, 0.5), (4.0, 4.0, 1.0));
    let cut = boolean(Op::Cut, &[&plate, &step], TOL).unwrap();
    check_closed(&cut.body);
    assert!((volume(&cut.body) - 14.0).abs() < 1e-9);
    assert_eq!(counts(&cut).1, 8);
}

#[test]
fn boxes_side_by_side_fuse_into_one() {
    let a = cuboid((0.0, 0.0, 0.0), (1.0, 1.0, 1.0));
    let b = cuboid((1.0, 0.0, 0.0), (2.0, 1.0, 1.0));
    let fuse = boolean(Op::Fuse, &[&a, &b], TOL).unwrap();
    check_closed(&fuse.body);
    assert!((volume(&fuse.body) - 2.0).abs() < 1e-9);
    // No unify: the coplanar faces stay apart.
    assert_eq!(counts(&fuse), (1, 10, 20, 12));
}

#[test]
fn apart_tools_cut_two_holes() {
    let plate = cuboid((0.0, 0.0, 0.0), (6.0, 2.0, 1.0));
    let h1 = cuboid((1.0, 0.5, -1.0), (2.0, 1.5, 2.0));
    let h2 = cuboid((4.0, 0.5, -1.0), (5.0, 1.5, 2.0));
    let cut = boolean(Op::Cut, &[&plate, &h1, &h2], TOL).unwrap();
    check_closed(&cut.body);
    assert!((volume(&cut.body) - 10.0).abs() < 1e-9);
    assert_eq!(counts(&cut).1, 14);
}

#[test]
fn near_coincidences_decline() {
    let a = cuboid((0.0, 0.0, 0.0), (1.0, 1.0, 1.0));
    let b = cuboid((0.5, 0.5, 1.0 + 1e-6), (1.5, 1.5, 2.0));
    assert!(boolean(Op::Fuse, &[&a, &b], TOL).is_err());
}

#[test]
fn the_result_is_the_same_every_time() {
    let a = cuboid((0.0, 0.0, 0.0), (2.0, 2.0, 2.0));
    let b = prism(&[(1.0, -1.0), (3.0, 1.0), (1.0, 3.0), (-1.0, 1.0)], 0.5, 1.5);
    let r1 = boolean(Op::Cut, &[&a, &b], TOL).unwrap();
    let r2 = boolean(Op::Cut, &[&a, &b], TOL).unwrap();
    check_closed(&r1.body);
    assert_eq!(r1.body, r2.body);
    assert_eq!(r1.history, r2.history);
}
