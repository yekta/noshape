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
