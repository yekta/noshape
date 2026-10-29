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
fn a_void_and_a_cut_through_it() {
    let block = cuboid((0.0, 0.0, 0.0), (4.0, 4.0, 4.0));
    let hollow = boolean(Op::Cut, &[&block, &cuboid((1.0, 1.0, 1.0), (3.0, 3.0, 3.0))], TOL).unwrap();
    // One solid: the block's shell and the void's, inside out.
    assert_eq!(hollow.body.solids.len(), 1);
    assert_eq!(hollow.body.solids[0].shells.len(), 2);
    check_closed(&hollow.body);
    assert!((volume(&hollow.body) - 56.0).abs() < 1e-12);
    // A hole from the top into the void: one shell again.
    let open = boolean(Op::Cut, &[&hollow.body, &cuboid((1.5, 1.5, 2.0), (2.5, 2.5, 5.0))], TOL).unwrap();
    assert_eq!(open.body.solids[0].shells.len(), 1);
    check_closed(&open.body);
    assert!((volume(&open.body) - 55.0).abs() < 1e-12);
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
fn a_cylinder_halved_along_its_seam() {
    let cyl = cylinder(0.0, 0.0, 1.0, 0.0, 2.0);
    let half = boolean(Op::Cut, &[&cyl, &cuboid((-2.0, -2.0, -1.0), (2.0, 0.0, 3.0))], TOL).unwrap();
    check_used_both_ways(&half.body);
    // Two half disks, the half wall and the flat side; the seam is the flat side's edge at +x.
    assert_eq!(counts(&half), (1, 4, 6, 4));
    // A round boss on the half: its circle starts in the half wall's gap.
    let boss = boolean(Op::Fuse, &[&half.body, &cylinder(0.0, 0.0, 1.0, 1.0, 3.0)], TOL).unwrap();
    check_closed(&boss.body);
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

fn segment(a: DVec3, b: DVec3) -> Body {
    Body { vertices: vec![a, b], edges: vec![Edge { v: [0, 1], curve: Curve::Line }], faces: Vec::new(), solids: Vec::new() }
}

#[test]
fn a_line_through_a_box() {
    let bx = cuboid((0.0, 0.0, 0.0), (4.0, 4.0, 4.0));
    // In through the bottom, out through the top: one piece, its ends made by the line and each face.
    let r = boolean(Op::Common, &[&segment(DVec3::new(1.0, 2.0, -1.0), DVec3::new(1.0, 2.0, 5.0)), &bx], TOL).unwrap();
    assert_eq!((r.body.edges.len(), r.body.vertices.len(), r.body.faces.len()), (1, 2, 0));
    let generated = r.history.iter().filter(|h| h.relation == Relation::Generated).count();
    assert_eq!(generated, 4);
    assert!(r.history.iter().any(|h| h.kind == Kind::Edge && h.arg == 0 && h.relation == Relation::Modified));
    // Corner to corner through two vertices: they're modified, nothing generated.
    let r = boolean(Op::Common, &[&segment(DVec3::splat(-1.0), DVec3::splat(5.0)), &bx], TOL).unwrap();
    assert_eq!(r.body.edges.len(), 1);
    assert_eq!(r.history.iter().filter(|h| h.kind == Kind::Vertex && h.arg == 1 && h.relation == Relation::Modified).count(), 2);
    assert!(r.history.iter().all(|h| h.relation != Relation::Generated));
    // Along a face: its piece on the face, its ends made by the face's edges it crosses.
    let r = boolean(Op::Common, &[&segment(DVec3::new(1.0, 0.0, -1.0), DVec3::new(1.0, 0.0, 5.0)), &bx], TOL).unwrap();
    assert_eq!(r.body.edges.len(), 1);
    assert_eq!(r.history.iter().filter(|h| h.kind == Kind::Edge && h.arg == 1 && h.relation == Relation::Generated).count(), 2);
    // Along an edge: the edge's piece is the line's too.
    let r = boolean(Op::Common, &[&segment(DVec3::new(0.0, 0.0, -1.0), DVec3::new(0.0, 0.0, 5.0)), &bx], TOL).unwrap();
    assert_eq!(r.body.edges.len(), 1);
    assert_eq!(r.history.iter().filter(|h| h.kind == Kind::Edge && h.relation == Relation::Modified).count(), 2);
    // Beside it, missing it: declined.
    assert!(boolean(Op::Common, &[&segment(DVec3::new(6.0, 0.0, -1.0), DVec3::new(6.0, 0.0, 5.0)), &bx], TOL).is_err_and(|d| d == Decline("a line apart from the solid")));
}


#[test]
fn faces_on_crossing_planes_have_nothing_in_common() {
    // Two squares, one across the other: faces have in common only where they overlap.
    let sq = |z: bool| {
        let p = |a: f64, b: f64| if z { DVec3::new(a, b, 0.5) } else { DVec3::new(a, 0.5, b) };
        let vertices = vec![p(0.0, 0.0), p(1.0, 0.0), p(1.0, 1.0), p(0.0, 1.0)];
        let edges = (0..4).map(|k| Edge { v: [k, (k + 1) % 4], curve: Curve::Line }).collect();
        let normal = if z { DVec3::Z } else { -DVec3::Y };
        let loops = vec![(0..4).map(|k| Use { edge: k, reversed: false }).collect()];
        Body { vertices, edges, faces: vec![Face { surface: Surface::Plane { origin: p(0.0, 0.0), normal }, reversed: false, loops }], solids: Vec::new() }
    };
    let r = boolean(Op::Common, &[&sq(true), &sq(false)], TOL).unwrap();
    assert!(r.body.faces.is_empty() && r.body.edges.is_empty());
    assert_eq!(r.history.iter().filter(|h| h.relation == Relation::Deleted).count(), 18);
    // The same square twice: declined (their overlap isn't worked out).
    assert!(boolean(Op::Common, &[&sq(true), &sq(true)], TOL).is_err());
}

/// `b` turned by `angle` about the x axis.
fn turned_about_x(b: &Body, angle: f64) -> Body {
    let q = glam::DQuat::from_rotation_x(angle);
    let mut out = b.clone();
    for p in &mut out.vertices {
        *p = q * *p;
    }
    for e in &mut out.edges {
        if let Curve::Circle { center, normal, xdir, .. } = &mut e.curve {
            (*center, *normal, *xdir) = (q * *center, q * *normal, q * *xdir);
        }
    }
    for f in &mut out.faces {
        match &mut f.surface {
            Surface::Plane { origin, normal } => (*origin, *normal) = (q * *origin, q * *normal),
            Surface::Cylinder { origin, axis, xdir, .. } | Surface::Cone { origin, axis, xdir, .. } => {
                (*origin, *axis, *xdir) = (q * *origin, q * *axis, q * *xdir)
            }
        }
    }
    out
}

#[test]
fn a_hole_through_a_tilted_plate() {
    // A plane at an angle to a cylinder: ellipses (OCCT's: radii r / cos and r, normal the plane's).
    let a = 30f64.to_radians();
    let plate = turned_about_x(&cuboid((-2.0, -2.0, -0.5), (2.0, 2.0, 0.5)), a);
    let hole = cylinder(0.0, 0.0, 0.5, -3.0, 3.0);
    let cut = boolean(Op::Cut, &[&plate, &hole], TOL).unwrap();
    check_used_both_ways(&cut.body);
    assert_eq!(counts(&cut), (1, 7, 15, 10));
    let ellipses: Vec<(f64, f64)> = cut.body.edges.iter().filter_map(|e| if let Curve::Ellipse { major, minor, .. } = e.curve { Some((major, minor)) } else { None }).collect();
    assert_eq!(ellipses.len(), 2);
    for (major, minor) in ellipses {
        assert!((major - 0.5 / a.cos()).abs() < 1e-12 && (minor - 0.5).abs() < 1e-12, "{major} {minor}");
    }
    // Nearly along the axis: declined.
    let steep = turned_about_x(&cuboid((-2.0, -2.0, -0.5), (2.0, 2.0, 0.5)), 80f64.to_radians());
    assert!(boolean(Op::Cut, &[&steep, &hole], TOL).is_err());
}

/// A cone between radius `r0` at height `z0` and `r1` at `z1` about the vertical through (cx, cy), capped.
fn frustum(cx: f64, cy: f64, r0: f64, r1: f64, z0: f64, z1: f64) -> Body {
    let (c0, c1) = (DVec3::new(cx, cy, z0), DVec3::new(cx, cy, z1));
    let circ = |c: DVec3, r: f64| Curve::Circle { center: c, normal: DVec3::Z, xdir: DVec3::X, radius: r };
    let edges = vec![Edge { v: [0, 0], curve: circ(c0, r0) }, Edge { v: [1, 1], curve: circ(c1, r1) }, Edge { v: [0, 1], curve: Curve::Line }];
    let u = |e: u32, reversed: bool| Use { edge: e, reversed };
    let angle = ((r1 - r0) / (z1 - z0)).atan();
    let faces = vec![
        Face { surface: Surface::Plane { origin: c0, normal: -DVec3::Z }, reversed: false, loops: vec![vec![u(0, true)]] },
        Face { surface: Surface::Plane { origin: c1, normal: DVec3::Z }, reversed: false, loops: vec![vec![u(1, false)]] },
        Face {
            surface: Surface::Cone { origin: c0, axis: DVec3::Z, xdir: DVec3::X, radius: r0, angle },
            reversed: false,
            loops: vec![vec![u(0, false), u(2, false), u(1, true), u(2, true)]],
        },
    ];
    Body { vertices: vec![c0 + DVec3::X * r0, c1 + DVec3::X * r1], edges, faces, solids: vec![Solid { shells: vec![vec![0, 1, 2]] }] }
}

#[test]
fn a_cone_halved_through_its_axis() {
    // A plane through the axis: the cone's two rulings there.
    let cone = frustum(2.0, 2.0, 0.5, 1.0, 0.0, 1.0);
    let half = boolean(Op::Common, &[&cone, &cuboid((0.0, 0.0, -1.0), (2.0, 4.0, 2.0))], TOL).unwrap();
    check_used_both_ways(&half.body);
    assert_eq!(counts(&half), (1, 4, 6, 4));
    let rulings = half.body.edges.iter().filter(|e| e.curve == Curve::Line && (half.body.vertices[e.v[0] as usize].z - half.body.vertices[e.v[1] as usize].z).abs() > 0.5).count();
    assert_eq!(rulings, 2);
    // Along the axis but off it: a hyperbola (OCCT's: centred level with the apex, radii |dd| / tan and |dd|).
    let cut = boolean(Op::Common, &[&cone, &cuboid((0.0, 0.0, -1.0), (2.2, 4.0, 2.0))], TOL).unwrap();
    check_used_both_ways(&cut.body);
    assert_eq!(counts(&cut), (1, 4, 6, 4));
    let hyps: Vec<(DVec3, f64, f64)> = cut.body.edges.iter().filter_map(|e| if let Curve::Hyperbola { center, major, minor, .. } = e.curve { Some((center, major, minor)) } else { None }).collect();
    // (Two arcs of its branch: its vertex is below the cone.)
    assert_eq!(hyps.len(), 2);
    for (c, major, minor) in hyps {
        assert!((c - DVec3::new(2.2, 2.0, -1.0)).length() < 1e-12 && (major - 0.4).abs() < 1e-12 && (minor - 0.2).abs() < 1e-12, "{c} {major} {minor}");
    }
    // Clear of the cone: nothing to cut.
    let clear = boolean(Op::Cut, &[&cone, &cuboid((3.2, 0.0, -1.0), (4.0, 4.0, 2.0))], TOL).unwrap();
    assert_eq!(counts(&clear), (1, 3, 3, 2));
}

#[test]
fn cross_drilled_holes_of_one_radius() {
    // Cylinders of one radius whose axes meet: two ellipses in the planes between them (OCCT's).
    let block = cuboid((-2.0, -2.0, -2.0), (2.0, 2.0, 2.0));
    let down = cylinder(0.0, 0.0, 0.5, -3.0, 3.0);
    let across = turned_about_x(&cylinder(0.0, 0.0, 0.5, -3.0, 3.0), 90f64.to_radians());
    let cut = boolean(Op::Cut, &[&block, &down, &across], TOL).unwrap();
    check_used_both_ways(&cut.body);
    let ellipses: Vec<(f64, f64)> = cut.body.edges.iter().filter_map(|e| if let Curve::Ellipse { major, minor, .. } = e.curve { Some((major, minor)) } else { None }).collect();
    // (Each also cut where OCCT's curve starts.)
    assert_eq!(counts(&cut), (1, 10, 26, 16));
    assert_eq!(ellipses.len(), 6);
    for (major, minor) in ellipses {
        assert!((major - 0.5 * 2f64.sqrt()).abs() < 1e-12 && (minor - 0.5).abs() < 1e-12, "{major} {minor}");
    }
    // Of other radii: declined.
    let thinner = turned_about_x(&cylinder(0.0, 0.0, 0.4, -3.0, 3.0), 90f64.to_radians());
    assert!(boolean(Op::Cut, &[&block, &down, &thinner], TOL).is_err());
}
