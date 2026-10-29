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
        let Surface::Plane { origin, .. } = f.surface else { panic!("planes only") };
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

/// A cylinder along z: centre (cx, cy), radius r, from z0 to z1, its seam at +x.
fn cylinder(cx: f64, cy: f64, r: f64, z0: f64, z1: f64) -> Body {
    let c0 = DVec3::new(cx, cy, z0);
    let c1 = DVec3::new(cx, cy, z1);
    let circ = |c: DVec3| Curve::Circle { center: c, normal: DVec3::Z, xdir: DVec3::X, radius: r };
    let edges = vec![Edge { v: [0, 0], curve: circ(c0) }, Edge { v: [1, 1], curve: circ(c1) }, Edge { v: [0, 1], curve: Curve::Line }];
    let u = |e: u32, reversed: bool| Use { edge: e, reversed };
    let faces = vec![
        Face { surface: Surface::Plane { origin: c0, normal: DVec3::Z }, reversed: true, loops: vec![vec![u(0, true)]] },
        Face { surface: Surface::Plane { origin: c1, normal: DVec3::Z }, reversed: false, loops: vec![vec![u(1, false)]] },
        Face {
            surface: Surface::Cylinder { origin: c0, axis: DVec3::Z, xdir: DVec3::X, radius: r },
            reversed: false,
            loops: vec![vec![u(0, false), u(2, false), u(1, true), u(2, true)]],
        },
    ];
    Body { vertices: vec![c0 + DVec3::X * r, c1 + DVec3::X * r], edges, faces, solids: vec![Solid { shells: vec![vec![0, 1, 2]] }] }
}

/// Every edge used once each way (a seam both ways by one face).
fn check_used_both_ways(b: &Body) {
    let mut uses = vec![[0, 0]; b.edges.len()];
    for f in &b.faces {
        for l in &f.loops {
            for u in l {
                uses[u.edge as usize][u.reversed as usize] += 1;
            }
        }
    }
    assert!(uses.iter().all(|u| *u == [1, 1]), "edges not used once each way: {uses:?}");
}

#[test]
fn a_slot_cut_by_two_boxes_side_by_side() {
    // The tools' faces where they meet face each other inside the tools: in neither's piece of the result.
    let plate = cuboid((0.0, 0.0, 0.0), (4.0, 4.0, 1.0));
    let (a, b) = (cuboid((1.0, -1.0, 0.5), (2.0, 5.0, 2.0)), cuboid((2.0, -1.0, 0.5), (3.0, 5.0, 2.0)));
    let cut = boolean(Op::Cut, &[&plate, &a, &b], TOL).unwrap();
    check_closed(&cut.body);
    // Bottom, four sides, the top either side of the slot, its walls and its floor (a piece of each tool's).
    assert_eq!(cut.body.faces.len(), 11);
    assert!((volume(&cut.body) - (16.0 - 4.0)).abs() < 1e-12);
}

#[test]
fn a_hole_through_a_plate_and_a_blind_one() {
    let plate = cuboid((0.0, 0.0, 0.0), (4.0, 4.0, 1.0));
    let tool = cylinder(2.0, 2.0, 0.5, -1.0, 2.0);
    let cut = boolean(Op::Cut, &[&plate, &tool], TOL).unwrap();
    check_used_both_ways(&cut.body);
    // The plate's six faces (top and bottom with a hole) and the hole's wall; its circles and seam.
    assert_eq!(counts(&cut), (1, 7, 15, 10));
    let blind = cylinder(2.0, 2.0, 0.5, 0.5, 1.0);
    let cut = boolean(Op::Cut, &[&plate, &blind], TOL).unwrap();
    check_used_both_ways(&cut.body);
    assert_eq!(counts(&cut), (1, 8, 15, 10));
}

#[test]
fn a_round_boss_on_a_plate() {
    let plate = cuboid((0.0, 0.0, 0.0), (4.0, 4.0, 1.0));
    let boss = cylinder(2.0, 2.0, 0.5, 1.0, 2.0);
    let fuse = boolean(Op::Fuse, &[&plate, &boss], TOL).unwrap();
    check_used_both_ways(&fuse.body);
    assert_eq!(counts(&fuse), (1, 8, 15, 10));
    // Through the plate's side: the cylinder's wall meets the plate's side in lines.
    let side = cylinder(4.0, 2.0, 0.5, -1.0, 2.0);
    let cut = boolean(Op::Cut, &[&plate, &side], TOL).unwrap();
    check_used_both_ways(&cut.body);
    assert_eq!(cut.body.faces.len(), 8);
}

#[test]
fn a_cylinder_halved_along_its_seam_declines() {
    // OCCT's result gives the cut line a seam's two curves on the cylinder, and the model's merging of faces
    // after (unify_same_domain) went differently on ours (LP2, CP2 in oracle_p1): declined (10-28).
    let cyl = cylinder(0.0, 0.0, 1.0, 0.0, 2.0);
    let half = boolean(Op::Cut, &[&cyl, &cuboid((-2.0, -2.0, -1.0), (2.0, 0.0, 3.0))], TOL);
    assert_eq!(half.err(), Some(Decline("a section along a cylinder's seam")));
}

/// A drill: a cylinder along z from z0 to z1 with a cone's tip below z0 (half-angle 59°, as a 118° drill point).
fn drill(cx: f64, cy: f64, r: f64, z0: f64, z1: f64) -> Body {
    let ang = 59f64.to_radians();
    let (c0, c1) = (DVec3::new(cx, cy, z0), DVec3::new(cx, cy, z1));
    let apex = DVec3::new(cx, cy, z0 - r / ang.tan());
    let circ = |c: DVec3| Curve::Circle { center: c, normal: DVec3::Z, xdir: DVec3::X, radius: r };
    let edges = vec![
        Edge { v: [0, 0], curve: circ(c0) },
        Edge { v: [1, 1], curve: circ(c1) },
        Edge { v: [0, 1], curve: Curve::Line },
        Edge { v: [2, 0], curve: Curve::Line },
        Edge { v: [2, 2], curve: Curve::Degenerate },
    ];
    let u = |e: u32, reversed: bool| Use { edge: e, reversed };
    let faces = vec![
        Face { surface: Surface::Plane { origin: c1, normal: DVec3::Z }, reversed: false, loops: vec![vec![u(1, false)]] },
        Face {
            surface: Surface::Cylinder { origin: c0, axis: DVec3::Z, xdir: DVec3::X, radius: r },
            reversed: false,
            loops: vec![vec![u(0, false), u(2, false), u(1, true), u(2, true)]],
        },
        Face {
            surface: Surface::Cone { origin: c0, axis: DVec3::Z, xdir: DVec3::X, radius: r, angle: ang },
            reversed: false,
            loops: vec![vec![u(4, false), u(3, false), u(0, true), u(3, true)]],
        },
    ];
    Body { vertices: vec![c0 + DVec3::X * r, c1 + DVec3::X * r, apex], edges, faces, solids: vec![Solid { shells: vec![vec![0, 1, 2]] }] }
}

#[test]
fn a_drilled_hole() {
    let plate = cuboid((0.0, 0.0, 0.0), (4.0, 4.0, 1.0));
    // Blind: the tip inside the plate.
    let tool = drill(2.0, 2.0, 0.5, 0.5, 2.0);
    let cut = boolean(Op::Cut, &[&plate, &tool], TOL).unwrap();
    check_used_both_ways_but_apex(&cut.body);
    assert_eq!(counts(&cut), (1, 8, 17, 11));
    // Through, the tip below the plate.
    let tool = drill(2.0, 2.0, 0.5, -0.5, 2.0);
    let cut = boolean(Op::Cut, &[&plate, &tool], TOL).unwrap();
    check_used_both_ways_but_apex(&cut.body);
    assert_eq!(counts(&cut), (1, 7, 15, 10));
    // A cone cut by the plate's top: a section circle on the cone.
    let tool = drill(2.0, 2.0, 0.5, 1.2, 2.0);
    let cut = boolean(Op::Cut, &[&plate, &tool], TOL).unwrap();
    check_used_both_ways_but_apex(&cut.body);
    assert_eq!(cut.body.faces.len(), 7);
}

/// Every edge used once each way, a cone's apex (one face, twice) aside.
fn check_used_both_ways_but_apex(b: &Body) {
    let mut uses = vec![[0, 0]; b.edges.len()];
    for f in &b.faces {
        for l in &f.loops {
            for u in l {
                uses[u.edge as usize][u.reversed as usize] += 1;
            }
        }
    }
    for (e, u) in uses.iter().enumerate() {
        if b.edges[e].curve != Curve::Degenerate {
            assert_eq!(*u, [1, 1], "edge {e}");
        }
    }
}

#[test]
fn an_arc_leaving_along_a_side_splits_the_face() {
    use crate::curve2::C2;
    use glam::DVec2;
    // A 0.02 × 0.04 rectangle and an arc from its corner to its side, tangent to the side there (a cylinder
    // touching a box's edge; the face came out with no pieces).
    let s6 = 0.024_494_897_427_831_803;
    let pts = [DVec2::ZERO, DVec2::new(-0.02, 0.0), DVec2::new(0.0, -0.04), DVec2::new(-0.02, -0.04), DVec2::new(0.0, -s6)];
    let seg = |a: usize, b: usize| ([a as u32, b as u32], C2::Seg { a: pts[a], b: pts[b] });
    let a0 = 1.369_438_406_004_566_6;
    let arc = C2::Arc { c: DVec2::new(-0.024_999_999_999_999_98, -0.024_494_897_427_831_768), r: 0.024_999_999_999_999_98, a0, sweep: -1.369_438_406_004_567_5 };
    let segs = [seg(0, 1), seg(2, 3), seg(0, 4), seg(4, 2), seg(1, 3), ([1, 4], arc)];
    assert_eq!(crate::arrange::regions(&pts, &segs).unwrap().len(), 2);
}
