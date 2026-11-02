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

/// A tub: a block with R3 fillets on its vertical and bottom edges (spheres at the bottom corners), the top sharp.
fn tub() -> Shape {
    let p = bx([0.0; 3], [40.0, 20.0, 10.0]);
    let es = edges_where(&p, |i| !(i.start.z > 9.99 && i.end.z > 9.99));
    fillet(&p, &es, 3.0).unwrap().shape
}

/// A block whose three edges at the origin are filleted R2 / R3 / R4 in one feature: a free-form (B-spline) corner.
fn mixed_corner() -> Shape {
    let p = bx([0.0; 3], [40.0, 20.0, 10.0]);
    // The three edges at the origin, R2 / R3 / R4 in one fillet: a free-form corner patch.
    let at0 = |i: &EdgeInfo| i.start.length() < 1e-9 || i.end.length() < 1e-9;
    let es: Vec<Shape> = edges_where(&p, at0);
    assert_eq!(es.len(), 3);
    let rs: Vec<f64> = es.iter().map(|e| { let i = e.edge_info().unwrap(); let d = (i.end - i.start).abs(); if d.x > 1e-9 { 2.0 } else if d.y > 1e-9 { 3.0 } else { 4.0 } }).collect();
    fillet_checked(&p, &es, &rs).unwrap().shape
}

/// A pocket with R3 vertical corners and an R2 floor fillet (tori where the floor fillet turns the corners).
fn round_pocket() -> Shape {
    let p = unify_same_domain(&cut(&bx([0.0; 3], [60.0, 40.0, 20.0]), &bx([20.0, 10.0, 10.0], [40.0, 30.0, 21.0])).unwrap().shape).unwrap().shape;
    let v = edges_where(&p, |i| (i.start.z - i.end.z).abs() > 1e-6 && i.start.x > 19.0 && i.start.x < 41.0 && i.start.y > 9.0 && i.start.y < 31.0);
    let p = fillet(&p, &v, 3.0).unwrap().shape;
    let floor = edges_where(&p, |i| (i.start.z - 10.0).abs() < 1e-9 && (i.end.z - 10.0).abs() < 1e-9);
    fillet(&p, &floor, 2.0).unwrap().shape
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

/// Live G46: R3 on the two top edges at (50, 0, 30) of a 50 x 30 x 30 block, then a 2 mm chamfer of the vertical
/// edge between them (OCCT's patch across the fillets, a B-spline), shelled with the top removed. Live HIGH (request
/// 1674): 6052.6808 / 13946.7022 mm³, 21 faces; ours is the exact offset (live's inner patch isn't symmetric, so
/// this pins ours). Slabs, ~3 s a shell (SHELL-SLOW: three of the nine slabs are free-form).
#[test]
#[ignore = "T2: ~7 s, our exact offset vs live's +1.2 mm³ (docs/handoff/core.md G46)"]
fn fillets_then_chamfer_shell_like_onshape() {
    let _ = set_working_scale(1000.0);
    let s = bx([0.0; 3], [0.05, 0.03, 0.03]);
    let tops = edges_where(&s, |i| (i.start.z - 0.03).abs() < 1e-9 && (i.end.z - 0.03).abs() < 1e-9 && ((i.start.y.abs() < 1e-9 && i.end.y.abs() < 1e-9) || ((i.start.x - 0.05).abs() < 1e-9 && (i.end.x - 0.05).abs() < 1e-9)));
    assert_eq!(tops.len(), 2);
    let s = fillet_checked(&s, &tops, &[0.003]).unwrap().shape;
    let corner = edges_where(&s, |i| (i.start.z - i.end.z).abs() > 1e-9 && (i.start.x - 0.05).abs() < 1e-9 && i.start.y.abs() < 1e-9 && (i.end.x - 0.05).abs() < 1e-9 && i.end.y.abs() < 1e-9);
    assert_eq!(corner.len(), 1);
    let s = chamfer_checked(&s, &corner, ChamferSpec::Equal(0.002), &[]).unwrap().shape;
    assert_eq!(s.count(ShapeType::Face), 10);
    assert!((vol(&s) * 1e9 - 44790.075177).abs() < 1e-4, "{}", vol(&s) * 1e9);
    let top = faces_where(&s, |c| c.z > 0.029999);
    assert_eq!(top.len(), 1);
    for (t, want) in [(0.001, 6053.881842), (0.0025, 13946.756013)] {
        let t0 = std::time::Instant::now();
        let r = shell_solid(&s, &top, t, ShellSide::Inside, ShellMethod::Auto).unwrap().shape;
        let took = t0.elapsed();
        let v = vol(&r) * 1e9;
        assert_eq!(r.count(ShapeType::Face), 21, "{t}");
        assert!((v - want).abs() < 1e-3, "{t}: {v} vs {want}");
        eprintln!("shell {t}: {v:.6} mm³ in {took:?} ({})", last_shell_path());
    }
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

/// qa's SHELL-WALLS (live "qa (2026-10-26, P0 wrong geometry check)"), in
/// metres as the model regenerates it: plate 80 x 50 x 10 mm with a Ø20
/// through hole at its centre, R2 fillets round the top face (its four
/// outer edges and the hole's rim), the top face removed. Onshape: 2.5 mm
/// → 14951.9619 mm³ / 19 faces, 1.5 mm → 9609.9491 mm³ / 27 faces (an
/// open box with a tube round the hole). The fillets' inward offset
/// collapses past their axis (t > R): the walls must stay.
#[test]
fn shell_of_a_filleted_plate_with_a_hole_keeps_its_walls() {
    let _ = set_working_scale(1000.0);
    let mm = 1e-3;
    let mut s = cut(&bx([0.0; 3], [80.0 * mm, 50.0 * mm, 10.0 * mm]), &cyl_z(40.0 * mm, 25.0 * mm, -mm, 10.0 * mm, 12.0 * mm)).unwrap().shape;
    let t = edges_where(&s, |i| (i.start.z - 10.0 * mm).abs() < 1e-9 && (i.end.z - 10.0 * mm).abs() < 1e-9);
    assert_eq!(t.len(), 5);
    s = fillet_checked(&s, &t, &[2.0 * mm]).unwrap().shape;
    let faces = faces_where(&s, |c| c.z > 10.0 * mm - 1e-9);
    assert_eq!(faces.len(), 1);
    for th in [1.5, 2.5] {
        for m in [ShellMethod::Offset, ShellMethod::Slabs] {
            match shell_solid(&s, &faces, th * mm, ShellSide::Inside, m) {
                Ok(r) => eprintln!("{th} {m:?}: {:.4} mm³ {} faces valid {}", vol(&r.shape) * 1e9, r.shape.count(ShapeType::Face), r.shape.is_valid()),
                Err(e) => eprintln!("{th} {m:?}: {e}"),
            }
        }
    }
    // Within 1 mm³: live's fillets meeting at the plate's sharp vertical
    // corners keep ~1 mm³ more than OCCT's mitre (live "core (2026-10-26
    // b)"; the same offset at t ≤ R, before any collapse). At 1.5 mm ours
    // has 28 faces, live 27 (open).
    for (th, want, nf) in [(2.5, 14951.9619, Some(19)), (1.5, 9609.9491, None)] {
        let r = shell_solid(&s, &faces, th * mm, ShellSide::Inside, ShellMethod::Auto).unwrap_or_else(|e| panic!("{th} mm: {e}")).shape;
        let v = vol(&r) * 1e9;
        assert!(r.is_valid());
        assert!((v - want).abs() < 1.0, "{th} mm: {v} vs {want}");
        if let Some(nf) = nf {
            assert_eq!(r.count(ShapeType::Face), nf, "{th} mm");
        }
    }
}

/// Live "qa5 (10-25 p2)" (6) (qa5 P1-16), in metres as the model regenerates
/// it: block 75.554 x 20.245 x 10 mm, R1 on three vertical edges, a 1 mm
/// chamfer round the top (the tangent chain), four Ø6.35 through holes at
/// x 15 / 40, y 6 / 14.245; the top face removed. Onshape: 1 mm → 3998.627
/// mm³ (33 faces, the hole walls stand as tubes), 2.5 mm → 8503.961 mm³
/// (47 faces, the tubes merge with each other and the side walls).
#[test]
fn shell_block_with_close_holes_like_onshape() {
    let _ = set_working_scale(1000.0);
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
        // Regions since the holes' tubes are added to the wall (1.2 / 2.3 s in slabs).
        assert_eq!(last_shell_path(), "regions", "{th} mm");
        let v = vol(&r) * 1e9;
        assert!(r.is_valid());
        assert!((v - want).abs() < 0.01, "{th} mm: {v} vs {want}");
        assert_eq!(r.count(ShapeType::Face), nf, "{th} mm");
    }
}

/// qa's SHELL-FILLETED-BOX (live "qa (2026-10-26, P0 shell check 2)"): a
/// 40 x 30 x 20 mm box with its 12 edges filleted R1, shelled thicker than
/// the radius: the fillets' and corners' inner offsets collapse and the
/// cavity is a sharp box. Live: hollow 2 mm 8948.0826 mm³ / 32 faces, open
/// top 2 mm 7076.0826 / 31, hollow 4 mm 15476.0826 / 32 (the hand math).
#[test]
fn shell_of_a_box_filleted_all_round_has_a_sharp_cavity() {
    let _ = set_working_scale(1000.0);
    let mm = 1e-3;
    let b = bx([0.0; 3], [40.0 * mm, 30.0 * mm, 20.0 * mm]);
    let all = edges_where(&b, |_| true);
    assert_eq!(all.len(), 12);
    let s = fillet_checked(&b, &all, &[1.0 * mm]).unwrap().shape;
    let top = faces_where(&s, |c| c.z > 20.0 * mm - 1e-9);
    assert_eq!(top.len(), 1);
    for (t, open, want, nf) in [(2.0, false, 8948.0826, 32), (2.0, true, 7076.0826, 31), (4.0, false, 15476.0826, 32)] {
        let faces: &[Shape] = if open { &top } else { &[] };
        let r = shell_solid(&s, faces, t * mm, ShellSide::Inside, ShellMethod::Auto).unwrap_or_else(|e| panic!("{t} mm open {open}: {e}")).shape;
        let v = vol(&r) * 1e9;
        eprintln!("{t} mm open {open}: {v:.4} mm³ {} faces", r.count(ShapeType::Face));
        assert!(r.is_valid());
        assert!((v - want).abs() < 0.01, "{t} mm open {open}: {v} vs {want}");
        assert_eq!(r.count(ShapeType::Face), nf, "{t} mm open {open}");
    }
}

/// qa's shell matrix against live's oracle (live "spec: core SHELL-WALLS /
/// SHELL-FILLETED-BOX", cases 22–24, 27, 28): 80 x 50 x 10 mm plates with a
/// centred Ø20 hole and the top face filleted R, shelled t > R with the top
/// removed; v0.4.8 lost walls in every one (#22 6305.5 vs 9559.7). Within
/// 0.55·R mm³ (R1 −0.47, R2 −0.96 in every case): live's volumes here are
/// REST mass properties at LOW precision, off by the same order (+0.28 /
/// +0.61 mm³ on a 40 × 30 × 20 block with R2 / R4 top fillets, behaviours
/// "core (2026-10-27)"); its corners are the exact mitre OCCT builds
/// (blend `top_edge_fillets_mitre_at_sharp_corners_as_live`).
#[test]
fn shell_matrix_cases_with_a_hole_keep_their_walls() {
    let _ = set_working_scale(1000.0);
    let mm = 1e-3;
    for (r, t, want) in [(1.0, 1.5, 9559.7209), (1.0, 2.5, 15158.7320), (1.0, 4.0, 22340.1910), (2.0, 4.0, 22133.4210)] {
        let mut s = cut(&bx([0.0; 3], [80.0 * mm, 50.0 * mm, 10.0 * mm]), &cyl_z(40.0 * mm, 25.0 * mm, -mm, 10.0 * mm, 12.0 * mm)).unwrap().shape;
        let top = edges_where(&s, |i| (i.start.z - 10.0 * mm).abs() < 1e-9 && (i.end.z - 10.0 * mm).abs() < 1e-9);
        s = fillet_checked(&s, &top, &[r * mm]).unwrap().shape;
        let faces = faces_where(&s, |c| c.z > 10.0 * mm - 1e-9);
        let out = shell_solid(&s, &faces, t * mm, ShellSide::Inside, ShellMethod::Auto).unwrap_or_else(|e| panic!("R{r} t{t}: {e}")).shape;
        let v = vol(&out) * 1e9;
        eprintln!("R{r} t{t}: {v:.4} mm³ (live {want}) {} faces", out.count(ShapeType::Face));
        assert!(out.is_valid(), "R{r} t{t}");
        assert!((v - want).abs() < 0.55 * r, "R{r} t{t}: {v} vs {want}");
    }
}

/// Live's rule for blind holes (behaviours "core (2026-10-27 b)", qa5 P1-5's
/// 75.554 × 20.245 × 15 block, Ø6.35 holes from the top, top removed): the
/// shell fails as soon as the hole's lowest flat level (a drilled tip's
/// shoulder or a flat bottom) is less than 2t above the floor.
#[test]
fn shell_fails_when_a_blind_holes_bottom_is_within_2t_of_the_floor() {
    let _ = set_working_scale(1000.0);
    let mm = 1e-3;
    let r = 3.175 * mm;
    let tip = r / 59f64.to_radians().tan();
    let part = |depth: f64, drilled: bool| {
        let mut s = bx([0.0; 3], [75.554 * mm, 20.245 * mm, 15.0 * mm]);
        for y in [6.0, 14.245] {
            let z = (15.0 - depth) * mm;
            let mut tool = cyl_z(15.0 * mm, y * mm, z, r, 16.0 * mm - z);
            if drilled {
                let cone = make_cone(&Plane::from_normal(DVec3::new(15.0 * mm, y * mm, z - tip), DVec3::Z), 0.0, r, tip, std::f64::consts::TAU).unwrap();
                tool = fuse(&tool, &cone).unwrap().shape;
            }
            s = cut(&s, &tool).unwrap().shape;
        }
        s
    };
    for (depth, drilled, t, ok) in [(10.0, false, 2.5, true), (10.5, false, 2.5, false), (12.0, false, 1.0, true), (10.0, true, 2.5, true), (10.5, true, 2.5, false), (12.0, true, 2.5, false)] {
        let s = part(depth, drilled);
        let top = faces_where(&s, |c| c.z > 15.0 * mm - 1e-9);
        let out = shell_solid(&s, &top, t * mm, ShellSide::Inside, ShellMethod::Auto);
        assert_eq!(out.is_ok(), ok, "depth {depth} drilled {drilled} t {t}: {:?}", out.as_ref().map(|o| vol(&o.shape) * 1e9).map_err(|e| e.to_string()));
        if let Ok(o) = out {
            assert!(o.shape.is_valid(), "depth {depth} drilled {drilled} t {t}");
        }
    }
}

/// SHELL-SLOW: a chamfer round the top over filleted corners makes cones,
/// which sent the whole part to slabs (here ~2.3 s, regions ~0.2 s; wf-block
/// keeps slabs for its hole, see by_regions). The regions path builds them
/// with the slabs' volumes; t below and above the fillets' radius.
#[test]
fn shell_over_chamfered_fillet_corners_by_regions() {
    let _ = set_working_scale(1000.0);
    let mm = 1e-3;
    let (a, b, h) = (40.0 * mm, 20.0 * mm, 10.0 * mm);
    let mut s = bx([0.0; 3], [a, b, h]);
    let v = edges_where(&s, |i| (i.start.z - i.end.z).abs() > 1e-9);
    s = fillet_checked(&s, &v, &[2.0 * mm]).unwrap().shape;
    let t = edges_where(&s, |i| (i.start.z - h).abs() < 1e-9 && (i.end.z - h).abs() < 1e-9);
    s = chamfer_checked(&s, &t, ChamferSpec::Equal(1.0 * mm), &[]).unwrap().shape;
    let faces = faces_where(&s, |c| c.z > h - 1e-9);
    assert_eq!(faces.len(), 1);
    // Faces: the slabs split each inner offset cone in two at 1 mm (39); one
    // offset face per face here.
    for (th, nf) in [(1.0, 35), (2.5, 27)] {
        let auto = shell_solid(&s, &faces, th * mm, ShellSide::Inside, ShellMethod::Auto).unwrap().shape;
        assert_eq!(last_shell_path(), "regions", "{th} mm");
        let slabs = shell_solid(&s, &faces, th * mm, ShellSide::Inside, ShellMethod::Slabs).unwrap().shape;
        assert!(auto.is_valid());
        let (va, vs) = (vol(&auto) * 1e9, vol(&slabs) * 1e9);
        assert!((va - vs).abs() < 1e-6, "{th} mm: {va} vs slabs {vs}");
        assert_eq!(auto.count(ShapeType::Face), nf, "{th} mm");
    }
}

/// SHELL-SLOW: a hole whose offset tube crosses a top chamfer's offset plane
/// (not the side wall's): the regions path's wall is exact, 3479.07658 mm³
/// (by sections: the offset box less the tube, the chamfer's offset y = z −
/// 8.586 above z 8.586). Cutting the tube from the planes' polytope came out
/// 3479.08807, so slabs took such parts (wf-block: ~3 s).
#[test]
fn shell_of_a_hole_by_a_chamfer_is_exact() {
    let _ = set_working_scale(1000.0);
    let mm = 1e-3;
    let (a, b, h) = (75.554 * mm, 20.245 * mm, 10.0 * mm);
    let mut s = bx([0.0; 3], [a, b, h]);
    let t = edges_where(&s, |i| (i.start.z - h).abs() < 1e-9 && (i.end.z - h).abs() < 1e-9);
    s = chamfer_checked(&s, &t, ChamferSpec::Equal(1.0 * mm), &[]).unwrap().shape;
    let c = make_cylinder(&Plane::from_normal(DVec3::new(15.0 * mm, 6.0 * mm, -mm), DVec3::Z), 3.175 * mm, 12.0 * mm, std::f64::consts::TAU).unwrap();
    s = cut(&s, &c).unwrap().shape;
    let faces = faces_where(&s, |c| c.z > h - 1e-9);
    let r = shell_solid(&s, &faces, mm, ShellSide::Inside, ShellMethod::Auto).unwrap().shape;
    assert_eq!(last_shell_path(), "regions");
    assert!(r.is_valid());
    assert!((vol(&r) * 1e9 - 3479.07658).abs() < 1e-3, "{}", vol(&r) * 1e9);
    assert_eq!(r.count(ShapeType::Face), 21);
}

/// qa GUARD-SEGV-MALLOC-DEADLOCK: Shell and Draft leave their input as it was
/// (OCCT's offset, Booleans and draft change their input in place: added
/// curves on faces, raised tolerances; on a body the model keeps, and a budget
/// or guard jump halfway through left it damaged for every later op). The
/// input's BRep, curves and tolerances included, is the same afterwards.
#[test]
fn shell_and_draft_leave_their_input_as_it_was() {
    let _ = set_working_scale(1000.0);
    let mm = 1e-3;
    let mut s = cut(&bx([0.0; 3], [80.0 * mm, 50.0 * mm, 10.0 * mm]), &cyl_z(40.0 * mm, 25.0 * mm, -mm, 10.0 * mm, 12.0 * mm)).unwrap().shape;
    let t = edges_where(&s, |i| (i.start.z - 10.0 * mm).abs() < 1e-9 && (i.end.z - 10.0 * mm).abs() < 1e-9);
    s = fillet_checked(&s, &t, &[2.0 * mm]).unwrap().shape;
    let before = s.to_brep_bytes().unwrap();
    let top = faces_where(&s, |c| c.z > 10.0 * mm - 1e-9);
    for th in [1.5, 2.5] {
        for m in [ShellMethod::Auto, ShellMethod::Offset, ShellMethod::Slabs] {
            let _ = shell_solid(&s, &top, th * mm, ShellSide::Inside, m);
            assert!(s.to_brep_bytes().unwrap() == before, "shell {th} mm {m:?} changed its input");
        }
    }
    let sides = faces_where(&s, |c| (c.x.abs() < 1e-9 || (c.x - 80.0 * mm).abs() < 1e-9) && c.z < 9.0 * mm);
    assert_eq!(sides.len(), 2);
    let _ = draft(&s, &sides, DVec3::Z, &Plane::xy(), 3f64.to_radians());
    assert!(s.to_brep_bytes().unwrap() == before, "draft changed its input");
}

/// The same for every op that takes a body the model keeps: none may change
/// it (a jump halfway through such an edit corrupts the body for good). Each
/// op on a fresh part.
#[test]
fn ops_leave_their_input_as_it_was() {
    let _ = set_working_scale(1000.0);
    let mm = 1e-3;
    let part = || {
        let s = cut(&bx([0.0; 3], [80.0 * mm, 50.0 * mm, 10.0 * mm]), &cyl_z(40.0 * mm, 25.0 * mm, -mm, 10.0 * mm, 12.0 * mm)).unwrap().shape;
        let t = edges_where(&s, |i| (i.start.z - 10.0 * mm).abs() < 1e-9 && (i.end.z - 10.0 * mm).abs() < 1e-9);
        fillet_checked(&s, &t, &[2.0 * mm]).unwrap().shape
    };
    let tool = bx([60.0 * mm, 10.0 * mm, 5.0 * mm], [100.0 * mm, 40.0 * mm, 20.0 * mm]);
    let circle = |z: f64| make_circle(DVec3::new(20.0 * mm, 25.0 * mm, z), DVec3::Z, 3.0 * mm).unwrap();
    let profile = make_face_from_wire(&make_wire(&[circle(30.0 * mm)]).unwrap(), true).unwrap();
    let plane = Plane::from_normal(DVec3::new(0.0, 0.0, 5.0 * mm), DVec3::Z);
    type Op<'a> = (&'a str, Box<dyn Fn(&Shape) + 'a>);
    let ops: Vec<Op> = vec![
        ("fuse", Box::new(|s| drop(fuse(s, &tool)))),
        ("cut", Box::new(|s| drop(cut(s, &tool)))),
        ("common", Box::new(|s| drop(common(s, &tool)))),
        ("boolean", Box::new(|s| drop(boolean(BooleanOp::Fuse, std::slice::from_ref(s), std::slice::from_ref(&tool), &BooleanOptions::default())))),
        ("split_by_plane", Box::new(|s| drop(split_by_plane(s, &plane)))),
        ("section_plane", Box::new(|s| drop(section_plane(s, &plane)))),
        ("unify_same_domain", Box::new(|s| drop(unify_same_domain(s)))),
        ("fillet_checked", Box::new(|s| drop(fillet_checked(s, &edges_where(s, |i| i.start.z.abs() < 1e-9 && i.end.z.abs() < 1e-9), &[1.0 * mm])))),
        ("chamfer_checked", Box::new(|s| drop(chamfer_checked(s, &edges_where(s, |i| i.start.z.abs() < 1e-9 && i.end.z.abs() < 1e-9), ChamferSpec::Equal(1.0 * mm), &[])))),
        ("draft", Box::new(|s| drop(draft(s, &faces_where(s, |c| (c.x.abs() < 1e-9 || (c.x - 80.0 * mm).abs() < 1e-9) && c.z < 9.0 * mm), DVec3::Z, &Plane::xy(), 3f64.to_radians())))),
        ("draft_parting_curve", Box::new(|s| {
            let sides = faces_where(s, |c| (c.x.abs() < 1e-9 || (c.x - 80.0 * mm).abs() < 1e-9) && c.z < 9.0 * mm);
            let parting = edges_where(s, |i| (i.start.z - 5.0 * mm).abs() < 1e-9 && (i.end.z - 5.0 * mm).abs() < 1e-9);
            drop(draft_parting_curve(s, &sides, &parting, DVec3::Z, &vec![3f64.to_radians(); sides.len()]))
        })),
        ("draft_wedges", Box::new(|s| drop(draft_wedges(s, &faces_where(s, |c| (c.x.abs() < 1e-9 || (c.x - 80.0 * mm).abs() < 1e-9) && c.z < 9.0 * mm), DVec3::Z, &Plane::xy(), 3f64.to_radians())))),
        ("delete_faces", Box::new(|s| drop(delete_faces(s, &faces_where(s, |c| c.z > 8.0 * mm && c.z < 10.0 * mm - 1e-9))))),
        ("offset_faces", Box::new(|s| drop(offset_faces(s, &faces_where(s, |c| c.z > 10.0 * mm - 1e-9), 1.0 * mm)))),
        ("offset_shape", Box::new(|s| drop(offset_shape(s, 0.5 * mm, JoinType::Arc)))),
        ("shell", Box::new(|s| drop(shell(s, &faces_where(s, |c| c.z > 10.0 * mm - 1e-9), 1.0 * mm, ShellSide::Inside, JoinType::Arc)))),
        ("shell_solid", Box::new(|s| drop(shell_solid(s, &faces_where(s, |c| c.z > 10.0 * mm - 1e-9), 1.0 * mm, ShellSide::Inside, ShellMethod::Auto)))),
        ("extrude_up_to", Box::new(|s| drop(extrude_up_to(&profile, -DVec3::Z, s, 0.0)))),
        ("project_curves", Box::new(|s| drop(project_curves(&circle(30.0 * mm), s, Some(-DVec3::Z))))),
        ("imprint", Box::new(|s| drop(imprint(s, &[circle(10.0 * mm)])))),
    ];
    let mut changed = Vec::new();
    for (what, op) in &ops {
        let s = part();
        let before = s.to_brep_bytes().unwrap();
        op(&s);
        if s.to_brep_bytes().unwrap() != before {
            changed.push(*what);
        }
    }
    assert!(changed.is_empty(), "changed their input: {changed:?}");
}

/// Solids bounded by planes: the direct wall (ShellMethod::Planes) is the slabs' wall: volume, counts and each input
/// face's images; where offsets would meet (a 3 mm wall between pockets, shelled 2 mm) it declines.
#[test]
fn planes_and_slabs_agree() {
    let unified = |s: Shape| unify_same_domain(&s).unwrap().shape;
    let l_block = unified(fuse(&bx([0.0; 3], [40.0, 20.0, 10.0]), &bx([0.0, 0.0, 0.0], [10.0, 40.0, 10.0])).unwrap().shape);
    let pocket = unified(cut(&bx([0.0; 3], [60.0, 40.0, 20.0]), &bx([20.0, 10.0, 10.0], [40.0, 30.0, 21.0])).unwrap().shape);
    let stepped = unified(fuse(&bx([0.0; 3], [60.0, 40.0, 10.0]), &bx([0.0, 0.0, 0.0], [30.0, 40.0, 20.0])).unwrap().shape);
    let pockets = unified(cut(&cut(&bx([0.0; 3], [60.0, 40.0, 20.0]), &bx([5.0, 5.0, 5.0], [28.5, 35.0, 21.0])).unwrap().shape, &bx([31.5, 5.0, 5.0], [55.0, 35.0, 21.0])).unwrap().shape);
    let bottom = |s: &Shape| faces_where(s, |c| c.z.abs() < 1e-9);
    let cases: Vec<(&str, Shape, Vec<Shape>, f64)> = vec![
        ("L, top removed", l_block.clone(), top(&l_block, 10.0), 2.0),
        ("pocket, bottom removed", pocket.clone(), bottom(&pocket), 2.0),
        ("stepped, bottom removed", stepped.clone(), bottom(&stepped), 1.5),
    ];
    let images = |r: &OpResult, s: &Shape| -> Vec<(usize, usize)> {
        (0..s.faces().len()).map(|i| (r.history.modified(0, ShapeType::Face, i as u32).len(), r.history.generated(0, ShapeType::Face, i as u32).len())).collect()
    };
    for (name, s, rm, t) in cases {
        let p = shell_solid(&s, &rm, t, ShellSide::Inside, ShellMethod::Planes).unwrap_or_else(|e| panic!("{name}: planes declined: {e}"));
        let q = shell_solid(&s, &rm, t, ShellSide::Inside, ShellMethod::Slabs).unwrap();
        assert!(p.shape.is_valid(), "{name}");
        assert!(close(vol(&p.shape), vol(&q.shape)), "{name}: {} vs slabs {}", vol(&p.shape), vol(&q.shape));
        for k in [ShapeType::Face, ShapeType::Edge, ShapeType::Vertex] {
            assert_eq!(p.shape.count(k), q.shape.count(k), "{name}: {k:?}");
        }
        assert_eq!(images(&p, &s), images(&q, &s), "{name}: each face's images (modified, generated)");
    }
    // A pocket's floor corners (all three edges concave) mitred, as live 1747: 13472 mm³, 21 faces, all planes.
    let w = shell_solid(&pocket, &bottom(&pocket), 2.0, ShellSide::Inside, ShellMethod::Planes).unwrap().shape;
    assert!((vol(&w) - 13472.0).abs() < 1e-6 && w.count(ShapeType::Face) == 21, "{} mm³, {} faces", vol(&w), w.count(ShapeType::Face));
    let rm = top(&pockets, 20.0);
    assert!(shell_solid(&pockets, &rm, 2.0, ShellSide::Inside, ShellMethod::Planes).is_err(), "offsets meeting across the 3 mm wall");
    // Both tops of the step removed: the step's riser hangs over the cavity, its foot past the removed step face.
    let rm = faces_where(&stepped, |c| ((c.z - 10.0).abs() < 1e-9 && c.x > 30.0) || (c.z - 20.0).abs() < 1e-9);
    assert!(shell_solid(&stepped, &rm, 1.5, ShellSide::Inside, ShellMethod::Planes).is_err(), "a rim past its removed face");
    assert!(shell_solid(&stepped, &rm, 1.5, ShellSide::Inside, ShellMethod::Quadrics).is_err(), "a rim past its removed face (quadrics)");
    let rm = top(&pockets, 20.0);
    assert!(shell_solid(&pockets, &rm, 2.0, ShellSide::Inside, ShellMethod::Quadrics).is_err(), "offsets meeting across the 3 mm wall (quadrics)");
}

/// A shell case: name, part, faces removed, thickness.
type Case = (&'static str, Shape, Vec<Shape>, f64);

/// Solids of planes, cylinders and cones: the direct wall (ShellMethod::Quadrics) against the slabs'.
#[test]
fn quadrics_and_slabs_agree() {
    let unified = |s: Shape| unify_same_domain(&s).unwrap().shape;
    let plate_hole = unified(cut(&bx([0.0; 3], [100.0, 40.0, 12.0]), &cyl_z(50.0, 20.0, -1.0, 5.0, 14.0)).unwrap().shape);
    let filleted = block(true, false);
    let l_block = unified(fuse(&bx([0.0; 3], [40.0, 20.0, 10.0]), &bx([0.0, 0.0, 0.0], [10.0, 40.0, 10.0])).unwrap().shape);
    let inner = edges_where(&l_block, |i| (i.start.z - i.end.z).abs() > 1e-6 && (i.start.x - 10.0).abs() < 1e-9 && (i.start.y - 20.0).abs() < 1e-9);
    let l_fillet = fillet(&l_block, &inner, 4.0).unwrap().shape;
    let sunk = {
        let p = unified(cut(&bx([0.0; 3], [60.0, 40.0, 12.0]), &cyl_z(30.0, 20.0, -1.0, 4.0, 14.0)).unwrap().shape);
        let rim = edges_where(&p, |i| i.start.z > 11.99 && i.end.z > 11.99 && i.start.distance(DVec3::new(30.0, 20.0, 12.0)) < 4.1);
        chamfer(&p, &rim, ChamferSpec::Equal(1.5), &[]).unwrap().shape
    };
    let bottom = |s: &Shape| faces_where(s, |c| c.z.abs() < 1e-9);
    // SHELL-SLOW's plate: 80 × 50 × 10, a Ø20 hole, R2 vertical outer fillets (t 1.5: the fillets stay; t 3: they collapse).
    let qa_plate = {
        let p = unified(cut(&bx([0.0; 3], [80.0, 50.0, 10.0]), &cyl_z(40.0, 25.0, -1.0, 10.0, 12.0)).unwrap().shape);
        let v = edges_where(&p, |i| (i.start.z - i.end.z).abs() > 1e-6 && matches!(i.curve, Curve::Line { .. }) && (i.start.x.abs() < 1e-9 || (i.start.x - 80.0).abs() < 1e-9));
        fillet(&p, &v, 2.0).unwrap().shape
    };
    // Chamfers on two top edges meeting at a corner: below their mitre a vertex of four faces (both chamfers, both
    // sides), whose offsets meet in a point. (The slabs' corner filler imprints an edge on each side face there, at
    // (0, 0.707, 7.293) and (0.707, 0, 7.293): two edges and two vertices more than ours, whose outside is the part's.)
    let chamfered = {
        let p = bx([0.0; 3], [40.0, 20.0, 10.0]);
        let es = edges_where(&p, |i| i.start.z > 9.99 && i.end.z > 9.99 && (i.start.y.abs() < 1e-9 && i.end.y.abs() < 1e-9 || i.start.x.abs() < 1e-9 && i.end.x.abs() < 1e-9));
        let p = chamfer(&p, &es, ChamferSpec::Equal(2.0), &[]).unwrap().shape;
        unified(cut(&p, &bx([15.0, 5.0, 4.0], [25.0, 15.0, 11.0])).unwrap().shape)
    };
    // A block with a blind Ø10 hole 10 deep from the top (its floor edge concave).
    let blind = unified(cut(&bx([0.0; 3], [60.0, 40.0, 20.0]), &cyl_z(30.0, 20.0, 10.0, 5.0, 11.0)).unwrap().shape);
    let (tub, round_pocket, mixed) = (tub(), round_pocket(), mixed_corner());
    // A block whose top is a vault (an R50 cylinder along x, 15 mm high in the middle); a slot cut in its floor.
    let vault = {
        let c = make_cylinder(&Plane::from_normal(DVec3::new(-1.0, 10.0, -35.0), DVec3::X), 50.0, 42.0, std::f64::consts::TAU).unwrap();
        let p = common(&bx([0.0; 3], [40.0, 20.0, 20.0]), &c).unwrap().shape;
        unified(cut(&p, &bx([10.0, 8.0, -1.0], [30.0, 12.0, 4.0])).unwrap().shape)
    };
    // (The countersunk plate: the slabs' offset cone has its seam opposite the offset hole's, splitting the circle
    // they share in two (one more shown edge, two more vertices); ours keep the faces' frames, the circle whole, its
    // corners only shown, as live's.)
    let cases: Vec<Case> = vec![
        ("plate with a hole, top removed", plate_hole.clone(), top(&plate_hole, 12.0), 2.0),
        ("plate with a hole, bottom removed", plate_hole.clone(), bottom(&plate_hole), 2.0),
        ("filleted block, top removed", filleted.clone(), top(&filleted, 10.0), 1.0),
        ("L with a filleted inner corner, top removed", l_fillet.clone(), top(&l_fillet, 10.0), 2.0),
        ("countersunk plate, bottom removed", sunk.clone(), bottom(&sunk), 2.0),
        ("two chamfers meeting (a vertex of four faces), bottom removed", chamfered.clone(), bottom(&chamfered), 1.0),
        ("a blind hole, bottom removed (a concave cylinder-plane edge)", blind.clone(), bottom(&blind), 2.0),
        ("rounded bottom corners (spheres), top removed", tub.clone(), top(&tub, 10.0), 1.0),
        ("a rounded pocket (tori at its floor corners), bottom removed", round_pocket.clone(), bottom(&round_pocket), 1.0),
        ("a free-form corner (R2 / R3 / R4 at one vertex), top removed", mixed.clone(), top(&mixed, 10.0), 1.0),
        ("a vaulted top removed (a curved opening)", vault.clone(), faces_where(&vault, |c| c.z > 12.0), 1.0),
        ("a hole's wall removed (a closed curved opening)", plate_hole.clone(), plate_hole.faces().into_iter().filter(|f| matches!(f.face_surface(), Ok(Surface::Cylinder { .. }))).collect(), 2.0),
        ("qa plate R2, top removed, 1.5", qa_plate.clone(), top(&qa_plate, 10.0), 1.5),
        ("qa plate R2, top removed, 3 (the fillets collapse)", qa_plate.clone(), top(&qa_plate, 10.0), 3.0),
    ];
    let images = |r: &OpResult, s: &Shape| -> Vec<(usize, usize)> {
        (0..s.faces().len()).map(|i| (r.history.modified(0, ShapeType::Face, i as u32).len(), r.history.generated(0, ShapeType::Face, i as u32).len())).collect()
    };
    let mut report = String::new();
    let shown = |x: &Shape| {
        let st = x.shown_topology().unwrap();
        [x.count(ShapeType::Face), st.edges.iter().filter(|b| **b).count(), st.vertices.iter().filter(|b| **b).count()]
    };
    for (name, s, rm, t) in cases {
        let q = shell_solid(&s, &rm, t, ShellSide::Inside, ShellMethod::Slabs).unwrap();
        match shell_solid(&s, &rm, t, ShellSide::Inside, ShellMethod::Quadrics) {
            Err(e) => report += &format!("{name}: quadrics declined: {e}\n"),
            Ok(p) => {
                let counts = shown;
                let (a, b) = (shown(&p.shape), shown(&q.shape));
                // The slabs over-split (a seam opposite another's, an edge imprinted by a corner filler, an offset face
                // left in pieces where their union isn't unified, which depends on the process's working scale): ours
                // may have fewer shown edges and vertices and fewer generated faces, never more, and the same faces.
                let (ia, ib) = (images(&p, &s), images(&q, &s));
                let images_ok = ia.iter().zip(&ib).all(|(x, y)| x.0 == y.0 && x.1 <= y.1);
                let counts_ok = a[0] <= b[0] && a[1] <= b[1] && a[2] <= b[2];
                if !p.shape.is_valid() || !close(vol(&p.shape), vol(&q.shape)) || !counts_ok || !images_ok {
                    report += &format!("{name}: quadrics {} {:?} vs slabs {} {:?}; images {:?} vs {:?}\n", vol(&p.shape), counts(&p.shape), vol(&q.shape), counts(&q.shape), images(&p, &s), images(&q, &s));
                    let mine: Vec<DVec3> = p.shape.vertices().iter().map(|v| v.vertex_point().unwrap()).collect();
                    for v in q.shape.vertices() {
                        let x = v.vertex_point().unwrap();
                        if mine.iter().all(|m| m.distance(x) > 1e-6) {
                            eprintln!("  slabs' extra vertex at {x:.4?}");
                        }
                    }
                }
            }
        }
    }
    assert!(report.is_empty(), "{report}");
}

/// SHELL-SLOW's plate (80 × 50 × 10, a Ø20 hole, R2 vertical outer fillets): the automatic path and its time.
#[test]
#[ignore = "T2 probe"]
fn qa_plate_shell_path() {
    let p = unify_same_domain(&cut(&bx([0.0; 3], [80.0, 50.0, 10.0]), &cyl_z(40.0, 25.0, -1.0, 10.0, 12.0)).unwrap().shape).unwrap().shape;
    let v = edges_where(&p, |i| (i.start.z - i.end.z).abs() > 1e-6 && matches!(i.curve, Curve::Line { .. }) && (i.start.x.abs() < 1e-9 || (i.start.x - 80.0).abs() < 1e-9));
    let p = fillet(&p, &v, 2.0).unwrap().shape;
    for t in [1.5, 2.5, 3.0] {
        let t0 = std::time::Instant::now();
        let r = shell_solid(&p, &top(&p, 10.0), t, ShellSide::Inside, ShellMethod::Auto).map(|r| vol(&r.shape));
        println!("qa plate t {t}: {r:?} by {} in {:.0} ms", last_shell_path(), t0.elapsed().as_secs_f64() * 1e3);
    }
}

/// The shell corpus's cases the direct walls take (~/agents/shellcorpus: cases/<name>.brep + .txt as NOSHAPE_DUMP_OPS
/// writes them, `taken` lists them), each shelled with and without the direct walls, best of 3: one line each way.
/// `tools/tiers/perf.sh -p noshape-kernel --test all -- shell::shell_corpus_time --ignored --nocapture --test-threads=1`
#[test]
#[ignore = "T2 bench: needs ~/agents/shellcorpus"]
fn shell_corpus_time() {
    let dir = std::path::PathBuf::from(format!("{}/agents/shellcorpus", std::env::var("HOME").unwrap_or_default()));
    let names: Vec<String> = std::fs::read_to_string(dir.join("taken")).expect("taken").lines().map(str::to_string).filter(|l| !l.is_empty()).collect();
    let run = |direct: bool| {
        // SAFETY: the bench is single-threaded (--test-threads=1); the kernel reads these per call.
        unsafe {
            if direct {
                std::env::remove_var("NOSHAPE_SHELL_NO_QUADRICS");
                std::env::remove_var("NOSHAPE_SHELL_NO_PLANES");
            } else {
                std::env::set_var("NOSHAPE_SHELL_NO_QUADRICS", "1");
                std::env::set_var("NOSHAPE_SHELL_NO_PLANES", "1");
            }
        }
        let (mut total, mut vols) = (0.0, Vec::new());
        for n in &names {
            let body = read_brep(&dir.join(format!("cases/{n}.brep"))).unwrap();
            let txt = std::fs::read_to_string(dir.join(format!("cases/{n}.txt"))).unwrap();
            let faces = body.faces();
            let picked: Vec<Shape> = txt.lines().filter_map(|l| l.strip_prefix("face ")).map(|i| faces[i.trim().parse::<usize>().unwrap()].clone()).collect();
            let w: Vec<&str> = txt.lines().last().unwrap().split_whitespace().collect();
            let t: f64 = w[2].parse().unwrap();
            let side = if w[4] == "1" { ShellSide::Outside } else { ShellSide::Inside };
            let mut best = f64::MAX;
            let mut v = 0.0;
            for _ in 0..3 {
                let t0 = std::time::Instant::now();
                v = shell_solid(&body, &picked, t, side, ShellMethod::Auto).map(|r| vol(&r.shape)).unwrap_or(f64::NAN);
                best = best.min(t0.elapsed().as_secs_f64() * 1e3);
            }
            total += best;
            vols.push(v);
        }
        (total, vols)
    };
    let (slab_ms, slab_v) = run(false);
    let (direct_ms, direct_v) = run(true);
    let same = slab_v.iter().zip(&direct_v).all(|(a, b)| (a - b).abs() <= 1e-9 * a.abs().max(1e-12));
    println!("shell corpus: {} cases taken; slabs {slab_ms:.1} ms, direct {direct_ms:.1} ms (best of 3 each); volumes {}", names.len(), if same { "the same" } else { "DIFFER" });
    unsafe {
        std::env::remove_var("NOSHAPE_SHELL_NO_QUADRICS");
        std::env::remove_var("NOSHAPE_SHELL_NO_PLANES");
    }
}

/// Parasolid refuses to shell a solid with a void, a box or a cylinder void in a plain block alike (live 1757: "Could
/// not shell part with selections."); without the void the block shells.
#[test]
fn a_solid_with_a_void_doesnt_shell() {
    let block = bx([0.0; 3], [60.0, 40.0, 20.0]);
    for void in [bx([20.0, 10.0, 5.0], [40.0, 30.0, 15.0]), cyl_z(30.0, 20.0, 5.0, 5.0, 10.0)] {
        let s = cut(&block, &void).unwrap().shape;
        assert_eq!(s.solids()[0].count(ShapeType::Shell), 2);
        for m in [ShellMethod::Auto, ShellMethod::Quadrics, ShellMethod::Slabs] {
            assert!(shell_solid(&s, &top(&s, 20.0), 1.0, ShellSide::Inside, m).is_err(), "{m:?}");
        }
    }
    assert!(close(vol(&shell_solid(&block, &top(&block, 20.0), 1.0, ShellSide::Inside, ShellMethod::Auto).unwrap().shape), 60.0 * 40.0 * 20.0 - 58.0 * 38.0 * 19.0));
}

/// A removed fillet as the opening (live 1752): Parasolid ends the walls next to it in flat rims normal to them,
/// 2175.269908 mm³ and 16 faces, as the slabs; the direct construction (its walls run on to the fillet's surface,
/// 2330.6 mm³) declines it.
#[test]
fn a_removed_fillet_opening_is_lives() {
    let b = bx([0.0; 3], [40.0, 20.0, 10.0]);
    let e = edges_where(&b, |i| i.start.y.abs() < 1e-9 && i.end.y.abs() < 1e-9 && i.start.z > 9.99 && i.end.z > 9.99);
    assert_eq!(e.len(), 1);
    let s = fillet(&b, &e, 5.0).unwrap().shape;
    let rm: Vec<Shape> = s.faces().into_iter().filter(|f| !matches!(f.face_surface(), Ok(Surface::Plane { .. }))).collect();
    assert_eq!(rm.len(), 1);
    assert!(shell_solid(&s, &rm, 1.0, ShellSide::Inside, ShellMethod::Quadrics).is_err());
    let r = shell_solid(&s, &rm, 1.0, ShellSide::Inside, ShellMethod::Auto).unwrap();
    assert!(r.shape.is_valid());
    let v = vol(&r.shape);
    assert!((v - 2175.269908169872).abs() < 1e-4, "{v}");
    assert_eq!(r.shape.count(ShapeType::Face), 16);
}

/// Spheres, tori and free-form faces taken directly: the tub's, the rounded pocket's and the free-form corner's Shell
/// by each path, best of 5.
#[test]
#[ignore = "T2 bench: tools/tiers/perf.sh"]
fn direct_shell_time() {
    let (t, p, m) = (tub(), round_pocket(), mixed_corner());
    for (name, s, rm) in [
        ("tub, top removed, 1 mm", &t, top(&t, 10.0)),
        ("rounded pocket, bottom removed, 1 mm", &p, faces_where(&p, |c| c.z.abs() < 1e-9)),
        ("free-form corner, top removed, 1 mm", &m, top(&m, 10.0)),
    ] {
        let best = |m: ShellMethod| {
            (0..5)
                .map(|_| {
                    let t0 = std::time::Instant::now();
                    shell_solid(s, &rm, 1.0, ShellSide::Inside, m).unwrap();
                    t0.elapsed().as_secs_f64() * 1e3
                })
                .fold(f64::INFINITY, f64::min)
        };
        let auto = best(ShellMethod::Auto);
        let path = last_shell_path();
        let (direct, offset, slabs) = (best(ShellMethod::Quadrics), best(ShellMethod::Offset), best(ShellMethod::Slabs));
        println!("timing {name}: direct {direct:.1} ms, OCCT's offset {offset:.1} ms, slabs {slabs:.1} ms; Auto ({path}) {auto:.1} ms");
    }
}

/// CLI fuzz part 82ac (metres), shelled 0.5 mm inward with its top and one sloped face removed, against live's body
/// (live 1758 / 1759, its Parasolid export): its r 6 / r 7 tubes are 2t walls, a boss half sunk in the part; one half
/// of the boss's split cylinder came out invalid, had no slab, and the slabs hollowed that half of the wall (570.5 mm³
/// short). Now the same solid as live's both ways, 70 faces, the two sliver voids live has too.
#[test]
fn a_sunk_boss_keeps_its_wall_as_live() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/shell");
    let s = read_brep(&dir.join("82ac-input.brep")).unwrap();
    let rm: Vec<Shape> = s
        .faces()
        .into_iter()
        .filter(|f| {
            let c = f.centroid().unwrap() * 1e3;
            let a = f.area().unwrap() * 1e6;
            ((c.z - 20.0).abs() < 1e-6 && (a - 4781.6514).abs() < 1e-3) || ((c - DVec3::new(34.378, 38.044, 17.870)).length() < 1e-2 && (a - 222.2683).abs() < 1e-3)
        })
        .collect();
    assert_eq!(rm.len(), 2);
    let r = shell_solid(&s, &rm, 0.5e-3, ShellSide::Inside, ShellMethod::Auto).unwrap().shape;
    let live = read_parasolid(&dir.join("82ac-live-shelled.x_t"), LengthUnit::Meter).unwrap();
    let (a, b) = (vol(&cut(&live, &r).unwrap().shape) * 1e9, vol(&cut(&r, &live).unwrap().shape) * 1e9);
    assert!(a < 1e-3 && b < 1e-3, "live - ours {a} mm³, ours - live {b} mm³ (ours {} mm³)", vol(&r) * 1e9);
    assert_eq!(r.count(ShapeType::Face), 70);
    assert_eq!(r.solids()[0].count(ShapeType::Shell), 3);
}

/// A user's twisted hexagon vase (11-02): a loft through five regular hexagons on parallel planes, circumradii
/// 35 / 45 / 40 / 28 / 35 mm at z 0 / 30 / 60 / 90 / 120, each turned 15° more; six B-spline sides with sharp edges
/// between them, the two hexagon caps.
fn vase() -> Shape {
    let sections: Vec<Shape> = [(0.0, 35.0), (30.0, 45.0), (60.0, 40.0), (90.0, 28.0), (120.0, 35.0)]
        .iter()
        .enumerate()
        .map(|(k, &(z, r))| {
            let pts: Vec<DVec3> = (0..6).map(|i| {
                let a = (15.0 * k as f64 + 60.0 * i as f64).to_radians();
                DVec3::new(r * a.cos(), r * a.sin(), z)
            }).collect();
            make_polyline(&pts, true).unwrap()
        })
        .collect();
    loft(&sections, &LoftOptions { solid: true, ruled: false, closed: false, ..Default::default() }).unwrap().shape
}

/// A user's twisted hexagon vase (11-02), every Shell failing: our loft left ThruSections' 3.1 mm tolerances on its
/// side edges (their old curves on the replaced surfaces) and triple knots at the sections (C0 to OCCT though the
/// spline is C2). Live's loft (.x_t) shelled 1.6 mm, top removed, is live's shell both ways (live 1769: 47868.386 mm³,
/// 15 faces); ours shells outward (2 mm, the case that ran for minutes), 15 faces.
#[test]
fn a_twisted_vase_shells_as_live() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/shell");
    let s = read_parasolid(&dir.join("vase-live-loft.x_t"), LengthUnit::Millimeter).unwrap();
    let live = read_parasolid(&dir.join("vase-live-shelled.x_t"), LengthUnit::Millimeter).unwrap();
    let r = shell_solid(&s, &top(&s, 120.0), 1.6, ShellSide::Inside, ShellMethod::Auto).unwrap().shape;
    // (Volume, area and faces: a Boolean against live's coincident offset walls takes 16-26 s.)
    let area = |x: &Shape| surface_properties(x).unwrap().mass;
    assert!((vol(&r) - vol(&live)).abs() < 0.05 && (area(&r) - area(&live)).abs() < 1e-5 * area(&live) && r.count(ShapeType::Face) == 15, "{} mm³ (live {}), {} mm² (live {}), {} faces", vol(&r), vol(&live), area(&r), area(&live), r.count(ShapeType::Face));
    // Outward the foot's corner lies past the sides, which run on as their own polynomials (live 1799's body).
    let out = read_parasolid(&dir.join("vase-shell2-outward-live.x_t"), LengthUnit::Millimeter).unwrap();
    let r = shell_solid(&s, &top(&s, 120.0), 2.0, ShellSide::Outside, ShellMethod::Auto).unwrap().shape;
    assert!((vol(&r) - vol(&out)).abs() < 0.1 && r.count(ShapeType::Face) == 15, "2 mm outward: {} mm³ (live {}), {} faces", vol(&r), vol(&out), r.count(ShapeType::Face));
    let v = vase();
    let r = shell_solid(&v, &top(&v, 120.0), 2.0, ShellSide::Outside, ShellMethod::Auto).unwrap_or_else(|e| panic!("2 mm outward: {e}")).shape;
    assert!(r.is_valid() && r.count(ShapeType::Face) == 15, "2 mm outward");
}

/// The vase with every section's corners rounded R5 (each hexagon offset 5 mm in, then 5 mm out with arcs): 12 sides,
/// line faces between arc faces.
fn smooth_vase() -> Shape {
    let sections: Vec<Shape> = [(0.0, 35.0), (30.0, 45.0), (60.0, 40.0), (90.0, 28.0), (120.0, 35.0)]
        .iter()
        .enumerate()
        .map(|(k, &(z, r))| {
            let pts: Vec<DVec3> = (0..6).map(|i| {
                let a = (15.0 * k as f64 + 60.0 * i as f64).to_radians();
                DVec3::new(r * a.cos(), r * a.sin(), z)
            }).collect();
            let hex = make_polyline(&pts, true).unwrap();
            let inner = offset_wire_2d(&hex, -5.0, JoinType::Intersection, false).unwrap();
            offset_wire_2d(&inner, 5.0, JoinType::Arc, false).unwrap()
        })
        .collect();
    loft(&sections, &LoftOptions { solid: true, ruled: false, closed: false, ..Default::default() }).unwrap().shape
}

/// The user's vase with rounded corners (11-02): every Shell failed after 30 s. Our loft gave each side face its own
/// chord-length parameters, so the line and arc faces met in different curves at up to 1.8°; and the direct path
/// placed a corner on a curved tangent edge along a straight line, fitted that edge's offset off both offsets, and
/// built no outward walls (the faces' offsets end where the vase's foot flares out). Now the loft's sides are tangent
/// and both ways build directly.
#[test]
fn a_smooth_twisted_vase_shells() {
    // Each section's line and arc faces tangent along their edges: one side face, as live's (live 1782: 3 faces).
    let s = smooth_vase();
    assert_eq!(s.count(ShapeType::Face), 3);
    let faces = s.faces();
    let ef = s.edge_faces().unwrap();
    for (i, e) in s.edges().iter().enumerate() {
        let inf = e.edge_info().unwrap();
        if ef[i].len() != 2 || (inf.start.z - inf.end.z).abs() < 1.0 {
            continue; // a section's edge
        }
        for k in 0..=24 {
            let p = e.point_at(inf.first_param + (inf.last_param - inf.first_param) * k as f64 / 24.0).unwrap();
            let (a, b) = (faces[ef[i][0] as usize].normal_at_point(p).unwrap(), faces[ef[i][1] as usize].normal_at_point(p).unwrap());
            assert!(a.angle_between(b) < 1e-6, "sides meet at {} rad at z {}", a.angle_between(b), p.z);
        }
    }
    // Live 1782's Shells, 2 mm: inward 58695.0522, outward 63461.6237 mm³ (ours 0.66 / 2.40 off: our loft is 43 mm³ off
    // live's, which has one side face, LOFT-ONE-FACE; inward OCCT's offset, where it builds, agrees with ours to 1e-4).
    for (side, live) in [(ShellSide::Inside, 58695.0522), (ShellSide::Outside, 63461.6237)] {
        let r = shell_solid(&s, &top(&s, 120.0), 2.0, side, ShellMethod::Auto).unwrap_or_else(|e| panic!("2 mm {side:?}: {e}")).shape;
        assert!(r.is_valid() && r.count(ShapeType::Face) == 5, "{side:?}: {} faces", r.count(ShapeType::Face));
        assert!((vol(&r) - live).abs() < 5e-5 * live, "{side:?}: {} mm³, live {live}", vol(&r));
    }
}

/// Every way of building the smooth vase's Shell, inward and outward (SWEEP_M = offset / quadrics, SWEEP_T = 1.6,2,...).
#[test]
#[ignore = "T2: probe"]
fn smooth_vase_sweep_probe() {
    // (SWEEP_VASE=sharp: the hexagons' vase.)
    let s = if std::env::var("SWEEP_VASE").is_ok_and(|v| v == "sharp") { vase() } else { smooth_vase() };
    for (m, method) in [("offset", ShellMethod::Offset), ("quadrics", ShellMethod::Quadrics)].into_iter().filter(|(m, _)| std::env::var("SWEEP_M").map(|x| x == *m).unwrap_or(true)) {
        for t in std::env::var("SWEEP_T").map(|v| v.split(',').map(|x| x.parse().unwrap()).collect::<Vec<f64>>()).unwrap_or(vec![1.0, 1.6, 1.8, 1.9, 2.0, 2.2, 2.5, 3.0]) {
            for side in [ShellSide::Inside, ShellSide::Outside] {
                let t0 = std::time::Instant::now();
                match shell_solid(&s, &top(&s, 120.0), t, side, method) {
                    Ok(r) => println!("{m} {t} {side:?}: {:.6} mm3, {} faces, valid {}, {:.0} ms", vol(&r.shape), r.shape.count(ShapeType::Face), r.shape.is_valid(), t0.elapsed().as_secs_f64() * 1e3),
                    Err(e) => println!("{m} {t} {side:?}: {e} ({:.0} ms)", t0.elapsed().as_secs_f64() * 1e3),
                }
            }
        }
    }
}

#[test]
#[ignore = "T2: probe"]
fn smooth_vase_live_loft_probe() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/shell");
    let s = read_parasolid(&dir.join("vase-smooth-live-loft.x_t"), LengthUnit::Millimeter).unwrap();
    let live = read_parasolid(&dir.join("vase-smooth-live-shelled.x_t"), LengthUnit::Millimeter).unwrap();
    println!("live loft {} faces {} edges, {:.6} mm3, valid {}; live shell {:.6} mm3, {} faces", s.count(ShapeType::Face), s.count(ShapeType::Edge), vol(&s), s.is_valid(), vol(&live), live.count(ShapeType::Face));
    for (t, side) in [(1.6, ShellSide::Inside), (2.0, ShellSide::Inside), (2.0, ShellSide::Outside)] {
        let t0 = std::time::Instant::now();
        match shell_solid(&s, &top(&s, 120.0), t, side, ShellMethod::Auto) {
            Ok(r) => {
                let d = if t == 1.6 { format!(", live - ours {:.6}, ours - live {:.6}", vol(&cut(&live, &r.shape).unwrap().shape), vol(&cut(&r.shape, &live).unwrap().shape)) } else { String::new() };
                println!("{t} {side:?}: {:.6} mm3, {} faces, valid {}, {:.0} ms via {}{d}", vol(&r.shape), r.shape.count(ShapeType::Face), r.shape.is_valid(), t0.elapsed().as_secs_f64() * 1e3, last_shell_path())
            }
            Err(e) => println!("{t} {side:?}: {e} ({:.0} ms)", t0.elapsed().as_secs_f64() * 1e3),
        }
    }
}

#[test]
#[ignore = "T2: probe"]
fn smooth_vase_loft_probe() {
    let s = smooth_vase();
    println!("smooth vase loft: {} faces, {} edges, {:.4} mm3 (live 437833.2291, {:+.4}), valid {}", s.count(ShapeType::Face), s.count(ShapeType::Edge), vol(&s), vol(&s) - 437833.2291, s.is_valid());
}

/// shell-hang's Shell 3 (live 1761: 177929.974087 mm³, 42 faces, one shell, no face meeting itself).
#[test]
#[ignore = "T2: probe"]
fn shellhang_shell3_probe() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/shell");
    let s = read_step(&dir.join("shellhang-shell3-input.step"), LengthUnit::Millimeter).unwrap().remove(0).shape;
    let rm = faces_where(&s, |c| (c - DVec3::new(0.5316, 24.7526, -20.0)).length() < 0.01);
    println!("input {:.6} mm3, {} faces; removing {}", vol(&s), s.count(ShapeType::Face), rm.len());
    let t0 = std::time::Instant::now();
    let r = shell_solid(&s, &rm, 5.0, ShellSide::Inside, ShellMethod::Auto).unwrap().shape;
    let ef = r.edge_faces().unwrap();
    let slits: Vec<usize> = (0..ef.len()).filter(|&i| ef[i].len() == 2 && ef[i][0] == ef[i][1] && matches!(r.faces()[ef[i][0] as usize].face_surface(), Ok(Surface::Plane(_)))).collect();
    println!("{:.6} mm3 (live 177929.974087), {} faces, {} shells, valid {}, {:.0} ms via {}; slit edges {:?}", vol(&r), r.count(ShapeType::Face), r.count(ShapeType::Shell), r.is_valid(), t0.elapsed().as_secs_f64() * 1e3, last_shell_path(), slits);
}

/// Live 1794 (qa SHELL-INNER-OFFSET-DELTA): a fuzz part of 0.1-0.2 mm slabs, the face removed the bottom of one: every
/// inward Shell fails in Parasolid (nothing behind the opening to hollow), outward 3 mm builds. Ours built a wall.
#[test]
fn an_opening_with_nothing_behind_it_fails_as_live() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/shell");
    let s = read_step(&dir.join("sid-shell2-input.step"), LengthUnit::Millimeter).unwrap().remove(0).shape;
    let rm = faces_where(&s, |c| (c - DVec3::new(30.515171, 20.052576, -19.4)).length() < 1e-3);
    assert_eq!(rm.len(), 1);
    for t in [3.0, 0.5] {
        assert!(shell_solid(&s, &rm, t, ShellSide::Inside, ShellMethod::Auto).is_err(), "{t} mm inward built");
    }
    assert!(shell_solid(&s, &rm, 3.0, ShellSide::Outside, ShellMethod::Auto).is_ok(), "outward");
}

/// shell-hang's Shell 5 (live 1795: OK in 0.3 s, 175040.1025 mm³, 52 faces, one body).
#[test]
#[ignore = "T2: probe"]
fn shellhang_shell5_probe() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/shell");
    let s = read_step(&dir.join("shellhang-shell5-input.step"), LengthUnit::Millimeter).unwrap().remove(0).shape;
    let rm = faces_where(&s, |c| (c - DVec3::new(-57.5, -37.0, 0.0)).length() < 1e-3 || (c - DVec3::new(2.509939, -4.556823, 9.985755)).length() < 1e-3);
    let live = read_parasolid(&dir.join("shellhang-shell5-live.x_t"), LengthUnit::Millimeter).unwrap();
    println!("input {:.4} mm3 {} faces; removing {}; live {:.4} mm3 {} faces", vol(&s), s.count(ShapeType::Face), rm.len(), vol(&live), live.count(ShapeType::Face));
    let t0 = std::time::Instant::now();
    match shell_solid(&s, &rm, 3.0, ShellSide::Inside, ShellMethod::Auto) {
        Ok(r) => println!("ours {:.4} mm3, {} faces, valid {}, {:.0} ms via {}; live - ours {:.4}, ours - live {:.4}", vol(&r.shape), r.shape.count(ShapeType::Face), r.shape.is_valid(), t0.elapsed().as_secs_f64() * 1e3, last_shell_path(), vol(&cut(&live, &r.shape).unwrap().shape), vol(&cut(&r.shape, &live).unwrap().shape)),
        Err(e) => println!("ours: {e} ({:.0} ms)", t0.elapsed().as_secs_f64() * 1e3),
    }
}

#[test]
#[ignore = "T2: probe"]
fn twisted_vase_cut_time_probe() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/shell");
    let s = read_parasolid(&dir.join("vase-live-loft.x_t"), LengthUnit::Millimeter).unwrap();
    let live = read_parasolid(&dir.join("vase-live-shelled.x_t"), LengthUnit::Millimeter).unwrap();
    for (m, method) in [("offset", ShellMethod::Offset), ("quadrics", ShellMethod::Quadrics)] {
        let t0 = std::time::Instant::now();
        let r = shell_solid(&s, &top(&s, 120.0), 1.6, ShellSide::Inside, method).unwrap().shape;
        let t1 = std::time::Instant::now();
        let a = vol(&cut(&live, &r).unwrap().shape);
        let t2 = std::time::Instant::now();
        let b = vol(&cut(&r, &live).unwrap().shape);
        let t3 = std::time::Instant::now();
        println!("{m}: shell {:.0} ms, live - ours {a:.4} in {:.0} ms, ours - live {b:.4} in {:.0} ms, kinds {:?}", (t1 - t0).as_secs_f64() * 1e3, (t2 - t1).as_secs_f64() * 1e3, (t3 - t2).as_secs_f64() * 1e3, r.faces().iter().map(|f| format!("{:?}", f.face_surface().ok()).chars().take(10).collect::<String>()).collect::<Vec<_>>());
    }
}

/// Live 1799: the sharp vase's 2 mm Shells as live's bodies (live's loft shelled here, each way).
#[test]
#[ignore = "T2: probe"]
fn sharp_vase_2mm_probe() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/shell");
    let s = read_parasolid(&dir.join("vase-live-loft.x_t"), LengthUnit::Millimeter).unwrap();
    for (side, file) in [(ShellSide::Outside, "vase-shell2-outward-live.x_t"), (ShellSide::Inside, "vase-shell2-inward-live.x_t")] {
        let live = read_parasolid(&dir.join(file), LengthUnit::Millimeter).unwrap();
        for (m, method) in [("auto", ShellMethod::Auto), ("offset", ShellMethod::Offset), ("quadrics", ShellMethod::Quadrics)] {
            match shell_solid(&s, &top(&s, 120.0), 2.0, side, method) {
                Ok(r) => println!("{side:?} {m}: {:.4} mm3 (live {:.4}, {:+.4}), {} faces", vol(&r.shape), vol(&live), vol(&r.shape) - vol(&live), r.shape.count(ShapeType::Face)),
                Err(e) => println!("{side:?} {m}: {e}"),
            }
        }
    }
}
