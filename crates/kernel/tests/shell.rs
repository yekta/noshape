//! Robust shell: the offset and the boolean (slab) constructions agree, and
//! cases OCCT's offset can't do still give the right solid.

use glam::{DVec2, DVec3};
use noshape_kernel::*;

fn bx(a: [f64; 3], b: [f64; 3]) -> Shape {
    make_box_corners(DVec3::from(a), DVec3::from(b)).unwrap()
}

fn cyl_z(x: f64, y: f64, z0: f64, r: f64, h: f64) -> Shape {
    make_cylinder(&Plane::from_normal(DVec3::new(x, y, z0), DVec3::Z), r, h, std::f64::consts::TAU).unwrap()
}

fn vol(s: &Shape) -> f64 {
    volume_properties(s).unwrap().mass
}

fn edges_where(s: &Shape, f: impl Fn(&EdgeInfo) -> bool) -> Vec<Shape> {
    s.edges().into_iter().filter(|e| e.edge_info().map(|i| !i.degenerated && f(&i)).unwrap_or(false)).collect()
}

fn faces_where(s: &Shape, f: impl Fn(DVec3) -> bool) -> Vec<Shape> {
    s.faces().into_iter().filter(|x| f(x.centroid().unwrap())).collect()
}

/// 40 x 20 x 10 block, optionally with 3 mm vertical corner fillets and a
/// 1 mm chamfer all round the top.
fn block(fillets: bool, chamfer_top: bool) -> Shape {
    let mut s = bx([0.0; 3], [40.0, 20.0, 10.0]);
    if fillets {
        let v = edges_where(&s, |i| (i.start.z - i.end.z).abs() > 1e-6);
        s = fillet(&s, &v, 3.0).unwrap().shape;
    }
    if chamfer_top {
        let t = edges_where(&s, |i| i.start.z > 9.99 && i.end.z > 9.99);
        s = chamfer(&s, &t, ChamferSpec::Equal(1.0), &[]).unwrap().shape;
    }
    s
}

fn top(s: &Shape, z: f64) -> Vec<Shape> {
    faces_where(s, |c| c.z > z - 1e-6)
}

fn shelled(s: &Shape, faces: &[Shape], t: f64, side: ShellSide, m: ShellMethod) -> Option<(Shape, f64)> {
    let r = shell_solid(s, faces, t, side, m).ok()?;
    assert!(r.shape.is_valid(), "{m:?}: invalid solid");
    let v = vol(&r.shape);
    Some((r.shape, v))
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-6 * a.abs().max(b.abs()).max(1.0)
}

/// Where OCCT's offset works, the slab construction gives the same solid.
#[test]
fn offset_and_slabs_agree() {
    let plate_hole = |y: f64| cut(&bx([0.0; 3], [100.0, 40.0, 12.0]), &cyl_z(50.0, y, -1.0, 5.0, 14.0)).unwrap().shape;
    let step = cut(&bx([0.0; 3], [40.0, 40.0, 20.0]), &bx([-1.0, -1.0, 10.0], [20.0, 41.0, 21.0])).unwrap().shape;
    let cases: Vec<(&str, Shape, Vec<Shape>)> = {
        let b = bx([0.0; 3], [40.0, 20.0, 10.0]);
        let p = plate_hole(20.0);
        let f = block(true, false);
        let c = block(false, true);
        let fc = block(true, true);
        let cy = cyl_z(0.0, 0.0, 0.0, 15.0, 40.0);
        vec![
            ("box", b.clone(), top(&b, 10.0)),
            ("box hollow", b.clone(), vec![]),
            ("plate hole", p.clone(), top(&p, 12.0)),
            ("fillets", f.clone(), top(&f, 10.0)),
            ("chamfer", c.clone(), top(&c, 10.0)),
            ("fillets+chamfer", fc.clone(), top(&fc, 10.0)),
            ("step", step.clone(), faces_where(&step, |c| c.z < 1e-6)),
            ("cylinder", cy.clone(), top(&cy, 40.0)),
        ]
    };
    for (name, s, faces) in &cases {
        for t in [1.0, 2.5] {
            for side in [ShellSide::Inside, ShellSide::Outside] {
                let a = shelled(s, faces, t, side, ShellMethod::Offset);
                let b = shelled(s, faces, t, side, ShellMethod::Slabs);
                match (a, b) {
                    (Some((_, va)), Some((_, vb))) => assert!(close(va, vb), "{name} t={t} {side:?}: offset {va} vs slabs {vb}"),
                    (a, b) => eprintln!("{name} t={t} {side:?}: offset {:?}, slabs {:?}", a.map(|x| x.1), b.map(|x| x.1)),
                }
            }
        }
    }
}

/// QA3 case A: a Ø10 hole 3 mm from the front of a 12 mm plate, shelled
/// 2.5 mm from the top: the hole's wall merges with the front wall.
#[test]
fn shell_walls_that_overlap() {
    let s = cut(&bx([0.0; 3], [100.0, 40.0, 12.0]), &cyl_z(50.0, 8.0, -1.0, 5.0, 14.0)).unwrap().shape;
    let faces = top(&s, 12.0);
    // Cavity: 95 x 35 x 9.5 box minus the hole grown by 2.5 mm (r = 7.5),
    // which reaches 0.5 mm past the inner front wall (y = 2.5).
    let r: f64 = 7.5;
    let d = 8.0 - 2.5;
    let segment = r * r * (d / r).acos() - d * (r * r - d * d).sqrt();
    let cavity = 95.0 * 35.0 * 9.5 - (std::f64::consts::PI * r * r - segment) * 9.5;
    let want = vol(&s) - cavity;
    for m in [ShellMethod::Auto, ShellMethod::Slabs] {
        let (_, v) = shelled(&s, &faces, 2.5, ShellSide::Inside, m).expect("shell");
        assert!(close(v, want), "{m:?}: {v} vs {want}");
    }
}

/// QA3 case B: 3 mm corner fillets and a 1 mm chamfer round the top,
/// shelled 2.5 mm from the top. The chamfer's corner cones vanish in the
/// offset (OCCT's offset fails); the exact solid has volume 4125.9988 mm³
/// (built from half-spaces in the kernel scratch study).
#[test]
fn shell_where_offset_faces_vanish() {
    let s = block(true, true);
    let faces = top(&s, 10.0);
    let (r, v) = shelled(&s, &faces, 2.5, ShellSide::Inside, ShellMethod::Auto).expect("shell");
    assert!((v - 4125.9988).abs() < 1e-3, "{v}");
    assert!(r.faces().len() < 40, "{} faces", r.faces().len());
    // 1 mm (no vanishing): offset and slabs agree with the exact 1846.8729.
    let (_, v) = shelled(&s, &faces, 1.0, ShellSide::Inside, ShellMethod::Auto).expect("shell");
    assert!((v - 1846.8729).abs() < 1e-3, "{v}");
}

/// History: original faces Modified, offset faces Generated by their face,
/// the rim Modified from the removed face; every output face has a parent.
#[test]
fn shell_history_names_every_face() {
    let s = block(true, true);
    let faces = top(&s, 10.0);
    let top_index = s.index_of(&faces[0], ShapeType::Face).unwrap() as u32;
    for (t, m) in [(1.0, ShellMethod::Offset), (1.0, ShellMethod::Slabs), (2.5, ShellMethod::Auto)] {
        let r = shell_solid(&s, &faces, t, ShellSide::Inside, m).unwrap();
        let n = r.shape.faces().len();
        for i in 0..n as u32 {
            let parents: Vec<_> = r.history.of_output(ShapeType::Face, i).collect();
            assert!(!parents.is_empty(), "t={t} {m:?}: face {i} has no parent");
        }
        let rim: Vec<_> = r.history.of_input(0, ShapeType::Face, top_index).filter(|h| h.relation == Relation::Modified).collect();
        assert_eq!(rim.len(), 1, "t={t} {m:?}: rim");
        let generated = r.history.records.iter().filter(|h| h.relation == Relation::Generated && h.output_kind == ShapeType::Face).count();
        assert!(generated > 0, "t={t} {m:?}: no offset faces");
    }
}

/// Live G50: a 50 × 30 × 30 block, a 5 mm chamfer on the vertical edge at
/// (50, 0), then R3 fillets on the two top edges meeting there, shelled with
/// the top removed: Onshape's wall closes the rim corner with one flat
/// triangle (20 faces / 50 edges / 32 vertices at 1, 1.5, 2 and 2.05 mm;
/// 5990.51958739 mm³ at 1 mm, Onshape's x_t read back 5990.5231). In metres.
#[test]
fn chamfer_then_fillets_shell_like_onshape() {
    let s = bx([0.0; 3], [0.05, 0.03, 0.03]);
    let corner = edges_where(&s, |i| (i.start.z - i.end.z).abs() > 1e-9 && (i.start.x - 0.05).abs() < 1e-9 && i.start.y.abs() < 1e-9);
    let s = chamfer_checked(&s, &corner, ChamferSpec::Equal(0.005), &[]).unwrap().shape;
    let tops = edges_where(&s, |i| (i.start.z - 0.03).abs() < 1e-9 && (i.end.z - 0.03).abs() < 1e-9 && ((i.start.y.abs() < 1e-9 && i.end.y.abs() < 1e-9) || ((i.start.x - 0.05).abs() < 1e-9 && (i.end.x - 0.05).abs() < 1e-9)));
    assert_eq!(tops.len(), 2);
    let s = fillet_checked(&s, &tops, &[0.003]).unwrap().shape;
    assert_eq!(s.count(ShapeType::Face), 9);
    let top = faces_where(&s, |c| c.z > 0.029999);
    assert_eq!(top.len(), 1);
    for (t, want) in [(0.001, 5990.51958739), (0.0015, 8746.15438317), (0.002, 11348.9952635), (0.00205, 11601.0764818)] {
        let r = shell_solid(&s, &top, t, ShellSide::Inside, ShellMethod::Auto).unwrap().shape;
        let v = vol(&r) * 1e9;
        assert_eq!((r.count(ShapeType::Face), r.count(ShapeType::Edge), r.count(ShapeType::Vertex)), (20, 50, 32), "{t}");
        assert!((v - want).abs() < 0.01, "{t}: {v} vs {want}");
    }
    // Live G51: Onshape fails from 2.053 mm up to the fillet radius, then
    // works again.
    for t in [0.00206, 0.0025, 0.00299] {
        assert!(shell_solid(&s, &top, t, ShellSide::Inside, ShellMethod::Auto).is_err(), "{t} should fail like Onshape");
    }
    let r = shell_solid(&s, &top, 0.003, ShellSide::Inside, ShellMethod::Auto).unwrap().shape;
    assert!(r.is_valid() && vol(&r) > 0.0);
}

/// fs's std_reference shellTop (live 2653.5 mm³): a 20 mm cube at x = 900
/// with its top removed, shelled 1.5 mm inward. Its caps lie on planes whose
/// origin stayed at the sketch's (the world origin), 900 mm from the faces:
/// the region construction centred each offset half-space there and lost the
/// floor (2220 mm³).
#[test]
fn inward_shell_of_a_part_far_from_its_planes_origin() {
    let _ = set_working_scale(1000.0);
    let plane = Plane::new(DVec3::ZERO, DVec3::Z, DVec3::X);
    let sq = |a: (f64, f64), b: (f64, f64)| Segment2d::Line { a: DVec2::new(a.0, a.1), b: DVec2::new(b.0, b.1) };
    let (x0, x1) = (0.9, 0.92);
    let face = make_face_2d(&plane, &[sq((x0, 0.0), (x1, 0.0)), sq((x1, 0.0), (x1, 0.02)), sq((x1, 0.02), (x0, 0.02)), sq((x0, 0.02), (x0, 0.0))], &[]).unwrap();
    let cube = extrude(&face, DVec3::new(0.0, 0.0, 0.02)).unwrap().shape;
    let top = cube.faces().into_iter().find(|f| f.centroid().is_ok_and(|c| (c.z - 0.02).abs() < 1e-9)).unwrap();
    let r = shell_solid(&cube, &[top], 0.0015, ShellSide::Inside, ShellMethod::Auto).unwrap();
    assert!((r.shape.volume().unwrap() * 1e9 - 2653.5).abs() < 1e-6, "{}", r.shape.volume().unwrap() * 1e9);
    assert_eq!(r.shape.count(ShapeType::Face), 11);
}

/// Live "qa5 (10-25 p2)" (6) (qa5 P1-16), in metres as the model regenerates
/// it: block 75.554 x 20.245 x 10 mm, R1 on three vertical edges, a 1 mm
/// chamfer round the top (the tangent chain), four Ø6.35 through holes at
/// x 15 / 40, y 6 / 14.245; the top face removed. Onshape: 1 mm → 3998.627
/// mm³ (33 faces, the hole walls stand as tubes), 2.5 mm → 8503.961 mm³
/// (47 faces, the tubes merge with each other and the side walls).
#[test]
fn shell_block_with_close_holes_like_onshape() {
    let mm = 1e-3;
    let (a, b, h) = (75.554 * mm, 20.245 * mm, 10.0 * mm);
    let mut s = bx([0.0; 3], [a, b, h]);
    let v = edges_where(&s, |i| (i.start.z - i.end.z).abs() > 1e-9 && !(i.start.x.abs() < 1e-9 && (i.start.y - b).abs() < 1e-9));
    assert_eq!(v.len(), 3);
    s = fillet_checked(&s, &v, &[1.0 * mm]).unwrap().shape;
    let t = edges_where(&s, |i| (i.start.z - h).abs() < 1e-9 && (i.end.z - h).abs() < 1e-9);
    s = chamfer_checked(&s, &t, ChamferSpec::Equal(1.0 * mm), &[]).unwrap().shape;
    for (x, y) in [(15.0, 6.0), (15.0, 14.245), (40.0, 6.0), (40.0, 14.245)] {
        let c = make_cylinder(&Plane::from_normal(DVec3::new(x * mm, y * mm, -mm), DVec3::Z), 3.175 * mm, 12.0 * mm, std::f64::consts::TAU).unwrap();
        s = cut(&s, &c).unwrap().shape;
    }
    eprintln!("solid {:.3} mm³, {} faces", vol(&s) * 1e9, s.count(ShapeType::Face));
    let faces = faces_where(&s, |c| c.z > h - 1e-9);
    assert_eq!(faces.len(), 1);
    for th in [1.0, 2.5] {
        for m in [ShellMethod::Offset, ShellMethod::Slabs] {
            match shell_solid(&s, &faces, th * mm, ShellSide::Inside, m) {
                Ok(r) => eprintln!("{th} {m:?}: {:.3} mm³ {} faces valid {}", vol(&r.shape) * 1e9, r.shape.count(ShapeType::Face), r.shape.is_valid()),
                Err(e) => eprintln!("{th} {m:?}: {e}"),
            }
        }
    }
    for (th, want, nf) in [(1.0, 3998.627, 33), (2.5, 8503.961, 47)] {
        let r = shell_solid(&s, &faces, th * mm, ShellSide::Inside, ShellMethod::Auto).unwrap_or_else(|e| panic!("{th} mm: {e}")).shape;
        let v = vol(&r) * 1e9;
        assert!(r.is_valid());
        assert!((v - want).abs() < 0.01, "{th} mm: {v} vs {want}");
        assert_eq!(r.count(ShapeType::Face), nf, "{th} mm");
    }
}
