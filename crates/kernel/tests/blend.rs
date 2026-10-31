//! Checked blends (`fillet_checked`, `chamfer_checked`) on parts modelled in
//! metres with the working scale the model uses: cases where OCCT on its own
//! fails or builds an invalid solid. Volumes in mm³ are Onshape's (live capture).

use glam::DVec3;
use noshape_kernel::*;

const MM: f64 = 1e-3;

fn setup() {
    set_working_scale(1000.0).unwrap();
}

fn bx(a: [f64; 3], b: [f64; 3]) -> Shape {
    make_box_corners(DVec3::from(a) * MM, DVec3::from(b) * MM).unwrap()
}

fn vol_mm3(s: &Shape) -> f64 {
    volume_properties(s).unwrap().mass / (MM * MM * MM)
}

fn edges_where(s: &Shape, f: impl Fn(&EdgeInfo) -> bool) -> Vec<Shape> {
    s.edges().into_iter().filter(|e| e.edge_info().map(|i| !i.degenerated && f(&i)).unwrap_or(false)).collect()
}

/// 40 x 20 x 10 mm block with 3 mm vertical corner fillets.
fn filleted_block() -> Shape {
    let b = bx([0.0; 3], [40.0, 20.0, 10.0]);
    let v = edges_where(&b, |i| (i.start.z - i.end.z).abs() > 1e-9);
    fillet_checked(&b, &v, &[3.0 * MM]).unwrap().shape
}

fn top_loop(s: &Shape) -> Vec<Shape> {
    edges_where(s, |i| i.start.z > 9.99 * MM && i.end.z > 9.99 * MM)
}

#[test]
fn blend_over_smaller_fillets_at_metre_scale() {
    setup();
    let s = filleted_block();
    // OCCT fails these in metres (absolute blend tolerances); the working scale fixes it.
    for (r, want) in [(4.0, 7545.734690323423), (6.0, 7094.739677144683)] {
        let res = fillet_checked(&s, &top_loop(&s), &[r * MM]).expect("fillet");
        assert!(res.shape.is_valid());
        assert!((vol_mm3(&res.shape) - want).abs() < 0.12, "R{r}: {}", vol_mm3(&res.shape));
    }
    // A chamfer as wide as the corner fillets: OCCT fails, a hair smaller works.
    let res = chamfer_checked(&s, &top_loop(&s), ChamferSpec::Equal(3.0 * MM), &[]).expect("chamfer 3");
    assert!(res.shape.is_valid());
    assert!((vol_mm3(&res.shape) - 7434.194671058468).abs() < 1e-3, "{}", vol_mm3(&res.shape));
    // Wider: Onshape's "Distance of chamfer on face too large.".
    let e = chamfer_checked(&s, &top_loop(&s), ChamferSpec::Equal(4.0 * MM), &[]).unwrap_err();
    assert!(e.to_string().contains("chamfer distance too large"), "{e}");
}

/// A top-loop fillet R over vertical corner fillets Rc < R: Parasolid's corner
/// face is a rolling ball of radius Rc between the two R cylinders (G24/G25).
/// Onshape's corner face areas match to 1e-7. Volumes: `exact` is this
/// geometry's volume by the divergence theorem over the exact faces
/// (/root/kern-scratch/g8/exactvol.py); Onshape's mass properties of its
/// procedural corner face are off from that by -0.04..+0.10 mm³.
#[test]
fn vertex_blends_match_onshape() {
    setup();
    // (H, Rc, R, Onshape's volume mm³, exact volume, Onshape's corner face area mm²)
    for (h, rc, r, want, exact, area) in [
        (10.0, 3.0, 6.0, 7094.739677144684, 7094.796033, 29.7034992),
        (10.0, 2.0, 4.0, 7582.912205, 7583.000811, 13.2015552),
        (10.0, 1.0, 4.0, 7605.12518, 7605.162120, 6.7595739),
        (12.0, 3.0, 8.0, 8077.548982, 8077.509672, 40.0810543),
        (10.0, 3.0, 3.5, 7632.777402, 7632.875233, 16.7315556),
        (10.0, 3.0, 4.0, 7545.734690323423, 7545.834615, 19.3259443),
    ] {
        let b = bx([0.0; 3], [40.0, 20.0, h]);
        let v = edges_where(&b, |i| (i.start.z - i.end.z).abs() > 1e-9);
        let s = fillet_checked(&b, &v, &[rc * MM]).unwrap().shape;
        let top = edges_where(&s, |i| i.start.z > (h - 0.01) * MM && i.end.z > (h - 0.01) * MM);
        let res = fillet_checked(&s, &top, &[r * MM]).unwrap_or_else(|e| panic!("H{h} Rc{rc} R{r}: {e}"));
        assert!(res.shape.is_valid());
        let faces = res.shape.faces();
        assert_eq!(faces.len(), 18, "H{h} Rc{rc} R{r}");
        // The corner face at the origin: inside (0,0,H-R)..(R,R,H).
        let corner: Vec<f64> = faces
            .iter()
            .filter(|f| {
                let bb = f.bounding_box(true).unwrap();
                bb.max.x < (r + 0.01) * MM && bb.max.y < (r + 0.01) * MM && bb.min.z > (h - r - 0.01) * MM && !matches!(f.face_surface(), Ok(Surface::Cylinder { .. } | Surface::Plane { .. }))
            })
            .map(|f| f.area().unwrap() / (MM * MM))
            .collect();
        assert_eq!(corner.len(), 1, "H{h} Rc{rc} R{r}: {corner:?}");
        assert!((corner[0] - area).abs() < 2e-6 * area, "H{h} Rc{rc} R{r}: area {} vs {area}", corner[0]);
        let v = vol_mm3(&res.shape);
        assert!((v - exact).abs() < 2e-4 && (v - want).abs() < 0.12, "H{h} Rc{rc} R{r}: volume {v} vs {exact} (Onshape {want})");
        for i in 0..faces.len() as u32 {
            assert!(res.history.of_output(ShapeType::Face, i).next().is_some(), "H{h} Rc{rc} R{r}: face {i} has no parent");
        }
    }
}

#[test]
fn blends_that_consume_faces() {
    setup();
    // R5 on every edge of a 10 mm block: the side faces vanish.
    let b = bx([0.0; 3], [40.0, 20.0, 10.0]);
    let res = fillet_checked(&b, &b.edges(), &[5.0 * MM]).expect("R5");
    assert!(res.shape.is_valid());
    assert!((vol_mm3(&res.shape) - 6665.191429188093).abs() < 1e-3, "{}", vol_mm3(&res.shape));
    // Every face of the result has a parent in the history.
    for i in 0..res.shape.faces().len() as u32 {
        assert!(res.history.of_output(ShapeType::Face, i).next().is_some(), "face {i}");
    }
    assert!(fillet_checked(&b, &b.edges(), &[6.0 * MM]).is_err(), "R6 fails like Onshape");
    // A 5 mm thick L bracket with a hole, R2.5 on every edge (needs the 1e-5 step).
    let l = cut(&cut(&bx([0.0; 3], [40.0, 30.0, 30.0]), &bx([5.0, -1.0, 5.0], [41.0, 31.0, 31.0])).unwrap().shape, &make_cylinder(&Plane::from_normal(DVec3::new(25.0, 15.0, -1.0) * MM, DVec3::Z), 4.0 * MM, 10.0 * MM, std::f64::consts::TAU).unwrap()).unwrap().shape;
    let res = fillet_checked(&l, &l.edges(), &[2.5 * MM]).expect("L R2.5");
    assert!(res.shape.is_valid());
    // Pocket walls 6 mm high, R3 top and bottom.
    let p = cut(&bx([0.0; 3], [60.0, 30.0, 10.0]), &bx([10.0, 10.0, 4.0], [50.0, 20.0, 11.0])).unwrap().shape;
    let res = fillet_checked(&p, &p.edges(), &[3.0 * MM]).expect("pocket R3");
    assert!(res.shape.is_valid());
}

/// Draft by wedges against OCCT's draft (where it works) and Onshape (D1).
#[test]
fn draft_wedges_like_onshape() {
    setup();
    let b = bx([0.0; 3], [40.0, 20.0, 10.0]);
    let v = edges_where(&b, |i| (i.start.z - i.end.z).abs() > 1e-9);
    let s = fillet_checked(&b, &v, &[3.0 * MM]).unwrap().shape;
    let sides: Vec<Shape> = s
        .faces()
        .into_iter()
        .filter(|f| match f.face_surface().unwrap() {
            Surface::Plane(p) => p.normal.z.abs() < 1e-9,
            Surface::Cylinder { .. } => true,
            _ => false,
        })
        .collect();
    assert_eq!(sides.len(), 8);
    let neutral = Plane::from_normal(DVec3::ZERO, DVec3::Z);
    for (deg, pull, want) in [(5.0, 1.0, 7428.357159985565), (30.0, 1.0, 4966.961583068534), (5.0, -1.0, 8433.160575397696), (30.0, -1.0, 11587.230290602416)] {
        let a = f64::to_radians(deg);
        let r = draft_wedges(&s, &sides, DVec3::Z * pull, &neutral, a).unwrap_or_else(|e| panic!("{deg} {pull}: {e}"));
        assert!(r.shape.is_valid());
        assert!((vol_mm3(&r.shape) - want).abs() < 1e-3, "{deg}° pull {pull}: {} vs {want}", vol_mm3(&r.shape));
        // Drafted faces keep their identity: every side face is Modified into the result.
        for f in &sides {
            let i = s.index_of(f, ShapeType::Face).unwrap() as u32;
            assert!(r.history.of_input(0, ShapeType::Face, i).any(|h| h.relation == Relation::Modified), "{deg}° face {i}");
        }
    }
}

/// A checked blend that OCCT builds fine must be OCCT's own result, untouched
/// (no scaled copy): exact HLR lost fillet end arcs on a scaled-back plate
/// (drawing test `demo_plate_views_have_expected_lines`, 6 visible lines instead of 8).
#[test]
fn checked_blend_happy_path_is_occt_result() {
    setup();
    let b = bx([0.0; 3], [100.0, 60.0, 12.0]);
    let top = edges_where(&b, |i| i.start.z > 11.99 * MM && i.end.z > 11.99 * MM);
    let plain = fillet(&b, &top, 2.0 * MM).unwrap().shape;
    let checked = fillet_checked(&b, &top, &[2.0 * MM]).unwrap().shape;
    assert_eq!(plain.faces().len(), checked.faces().len());
    assert_eq!(plain.edges().len(), checked.edges().len());
    assert!((vol_mm3(&plain) - vol_mm3(&checked)).abs() < 1e-9);
    // Same geometry objects, not a rebuilt copy: the plain and checked results share no
    // TShapes with each other, but each edge of `checked` has the exact same curve type and length.
    for (a, b) in plain.edges().iter().zip(checked.edges().iter()) {
        let (ia, ib) = (a.edge_info().unwrap(), b.edge_info().unwrap());
        assert!((ia.length - ib.length).abs() < 1e-15, "{} vs {}", ia.length, ib.length);
        assert_eq!(std::mem::discriminant(&ia.curve), std::mem::discriminant(&ib.curve));
    }
}

/// Volumes stay right where one of OCCT's integrators is off: a prism of a
/// strong conic section (adaptive +4.5 %); plain Gauss is off on rational
/// NURBS cylinders (+0.9 %), checked in kern-scratch/gprop.cpp.
#[test]
fn volume_survives_integrator_weaknesses() {
    let cyl = make_cylinder(&Plane::from_normal(DVec3::ZERO, DVec3::Z), 10.0, 20.0, std::f64::consts::TAU).unwrap();
    let exact = std::f64::consts::PI * 100.0 * 20.0;
    assert!((volume_properties(&cyl).unwrap().mass - exact).abs() < 1e-6 * exact);
    // Conic section, weight 4, between (3,0,0), (0,0,0), (0,3,0), extruded 52.
    let curve = make_bspline(&[DVec3::new(3.0, 0.0, 0.0), DVec3::ZERO, DVec3::new(0.0, 3.0, 0.0)], Some(&[1.0, 4.0, 1.0]), &[0.0, 1.0], &[3, 3], 2, false).unwrap();
    let wire = make_wire(&[make_line(DVec3::ZERO, DVec3::new(3.0, 0.0, 0.0)).unwrap(), curve, make_line(DVec3::new(0.0, 3.0, 0.0), DVec3::ZERO).unwrap()]).unwrap();
    let face = make_face_from_wire(&wire, true).unwrap();
    let tool = extrude(&face, DVec3::Z * 52.0).unwrap().shape;
    let want = face.area().unwrap() * 52.0;
    let v = volume_properties(&tool).unwrap().mass;
    assert!((v - want).abs() < 1e-4 * want, "{v} vs {want}");
}

/// Thicken a closed surface (a whole sphere) into a hollow ball, either side.
#[test]
fn thicken_closed_surface() {
    let sph = make_sphere(&Plane::from_normal(DVec3::ZERO, DVec3::Z), 10.0, std::f64::consts::TAU).unwrap();
    let face = sph.faces()[0].clone();
    let ball = |r: f64| 4.0 / 3.0 * std::f64::consts::PI * r * r * r;
    for (t1, t2, want) in [(2.0, 0.0, ball(12.0) - ball(10.0)), (0.0, 2.0, ball(10.0) - ball(8.0)), (1.0, 1.0, ball(11.0) - ball(9.0))] {
        let r = thicken(&face, t1, t2).unwrap_or_else(|e| panic!("{t1} {t2}: {e}"));
        assert!(r.shape.is_valid());
        let v = volume_properties(&r.shape).unwrap().mass;
        assert!((v - want).abs() < 1e-3 * want, "{t1} {t2}: {v} vs {want}");
    }
}

/// A loft through three sections interpolates them with a natural cubic
/// spline like Parasolid (Onshape G10e: Ø20 at z0 → square 12 at z15 → Ø20 at
/// z30, 6125.607 mm³); OCCT's own parabola gives 5917.6.
#[test]
fn loft_through_three_sections_is_natural() {
    let circle = |z: f64| make_wire(&[make_circle(DVec3::new(0.0, 0.0, z), DVec3::Z, 10.0).unwrap()]).unwrap();
    let c = [(-6.0, -6.0), (6.0, -6.0), (6.0, 6.0), (-6.0, 6.0)];
    let square = make_wire(&(0..4).map(|i| make_line(DVec3::new(c[i].0, c[i].1, 15.0), DVec3::new(c[(i + 1) % 4].0, c[(i + 1) % 4].1, 15.0)).unwrap()).collect::<Vec<_>>()).unwrap();
    let opts = LoftOptions { solid: true, ..Default::default() };
    let r = loft(&[circle(0.0), square, circle(30.0)], &opts).unwrap();
    assert!(r.shape.is_valid());
    let v = volume_properties(&r.shape).unwrap().mass;
    assert!((v - 6125.606828000875).abs() < 0.05, "{v}");
    // Circles only (one face closed on itself): r = 10 → 6 → 10 with a natural
    // spline in z, r(z) = 10 - 4 (1.5 s - 0.5 s³) for s = z/15 up to the middle.
    let ring = |z: f64, r: f64| make_wire(&[make_circle(DVec3::new(0.0, 0.0, z), DVec3::Z, r).unwrap()]).unwrap();
    let r = loft(&[ring(0.0, 10.0), ring(15.0, 6.0), ring(30.0, 10.0)], &opts).unwrap();
    assert!(r.shape.is_valid());
    let v = volume_properties(&r.shape).unwrap().mass;
    let n = 20000;
    let want: f64 = (0..n)
        .map(|i| {
            let s = (i as f64 + 0.5) / n as f64;
            let rr = 10.0 - 4.0 * (1.5 * s - 0.5 * s * s * s);
            std::f64::consts::PI * rr * rr * 15.0 / n as f64
        })
        .sum::<f64>()
        * 2.0;
    // (ThruSections carries circles as polynomial B-splines: ~1e-6 off.)
    assert!((v - want).abs() < 1e-5 * want, "{v} vs {want}");
}

/// Live "fail preview 2": a 50 mm fillet on a top edge of a 40 x 30 x 20 mm
/// block, wider than both faces it rolls on, regenerates in Onshape as the
/// block cut by the ball's sweep (24000 -> 7458.6158 mm³); OCCT can't build
/// it. Smaller radii stay OCCT's fillet.
#[test]
fn fillet_wider_than_its_faces_like_onshape() {
    setup();
    let b = bx([0.0; 3], [40.0, 30.0, 20.0]);
    let edge = edges_where(&b, |i| i.start.y.abs() < 1e-9 && i.end.y.abs() < 1e-9 && i.start.z > 19.99 * MM && i.end.z > 19.99 * MM);
    assert_eq!(edge.len(), 1);
    for (r, want) in [(2.0, 23965.66371), (19.0, 20901.14951), (50.0, 7458.615817)] {
        let res = fillet_checked(&b, &edge, &[r * MM]).unwrap_or_else(|e| panic!("R{r}: {e}"));
        assert!(res.shape.is_valid(), "R{r}");
        assert!((vol_mm3(&res.shape) - want).abs() < 1e-3, "R{r}: {}", vol_mm3(&res.shape));
    }
    // R50: the top and front faces are gone, the round replaces them.
    let res = fillet_checked(&b, &edge, &[50.0 * MM]).unwrap();
    assert_eq!(res.shape.count(ShapeType::Face), 5);
}

/// Fillets on a block's four top edges meet the sharp vertical edges in an
/// exact mitre, as live (behaviours "core (2026-10-27)" SHELL-WALLS-CORNER:
/// 40 × 30 × 20, R2 23882.890721 / R4 23543.834555 mm³ at HIGH, 10 faces,
/// no corner patch; the exact mitre gives 23882.890820 / 23543.834784).
#[test]
fn top_edge_fillets_mitre_at_sharp_corners_as_live() {
    setup();
    for (r, want) in [(2.0, 23882.890820), (4.0, 23543.834784)] {
        let s = bx([0.0; 3], [40.0, 30.0, 20.0]);
        let top = edges_where(&s, |i| (i.start.z - 20.0 * MM).abs() < 1e-9 && (i.end.z - 20.0 * MM).abs() < 1e-9);
        let f = fillet_checked(&s, &top, &[r * MM]).unwrap().shape;
        let v = vol_mm3(&f);
        // Volume integration noise: R4 +0.0016 (live HIGH itself −0.0002).
        assert!((v - want).abs() < 5e-3, "R{r}: {v} vs live {want}");
        assert_eq!(f.faces().len(), 10, "R{r}");
    }
}

/// A box's corner edge filleted directly (features.cpp fillet_square_corner):
/// the exact solid, one cylinder face generated from the edge.
#[test]
fn a_square_corner_fillet_built_directly() {
    setup();
    let s = bx([0.0; 3], [40.0, 30.0, 20.0]);
    let edge = edges_where(&s, |i| (i.start.x - 40.0 * MM).abs() < 1e-9 && (i.end.x - 40.0 * MM).abs() < 1e-9 && (i.start.y - 30.0 * MM).abs() < 1e-9 && (i.end.y - 30.0 * MM).abs() < 1e-9);
    assert_eq!(edge.len(), 1);
    let edge_index = s.edges().iter().position(|e| e.is_same(&edge[0])).unwrap() as u32;
    let r = fillet_checked(&s, &edge, &[3.0 * MM]).unwrap();
    assert!(r.shape.is_valid());
    let want = 40.0 * 30.0 * 20.0 - (1.0 - std::f64::consts::FRAC_PI_4) * 9.0 * 20.0;
    assert!((vol_mm3(&r.shape) - want).abs() < 1e-6, "{} vs {want}", vol_mm3(&r.shape));
    assert_eq!(r.shape.faces().len(), 7);
    let generated = r.history.generated(0, ShapeType::Edge, edge_index);
    assert!(generated.iter().any(|(k, _)| *k == ShapeType::Face), "{generated:?}");
}


/// noshape's own blends (crates/brep blend.rs) against OCCT's on whole loops of a plate's top: a hole's rim
/// (convex), a boss's top rim (convex) and its base (concave), filleted and chamfered: the same faces, edges,
/// vertices, volume (1e-9) and history, and noshape's way taken.
#[test]
fn rust_blends_of_rims_are_occts() {
    let plate = make_box_corners(DVec3::ZERO, DVec3::new(60.0, 30.0, 10.0)).unwrap();
    let hole = make_cylinder(&Plane::new(DVec3::new(15.0, 15.0, -1.0), DVec3::Z, DVec3::X), 5.0, 12.0, std::f64::consts::TAU).unwrap();
    let boss = make_cylinder(&Plane::new(DVec3::new(42.0, 15.0, 10.0), DVec3::Z, DVec3::X), 6.0, 8.0, std::f64::consts::TAU).unwrap();
    let part = fuse(&cut(&plate, &hole).unwrap().shape, &boss).unwrap().shape;
    let circle_at = |s: &Shape, x: f64, z: f64| -> Vec<Shape> {
        s.edges().into_iter().filter(|e| e.edge_info().is_ok_and(|i| matches!(i.curve, Curve::Circle { center, .. } if (center.x - x).abs() < 1e-9 && (center.z - z).abs() < 1e-9))).collect()
    };
    for (what, x, z) in [("hole rim", 15.0, 10.0), ("boss top", 42.0, 18.0), ("boss base", 42.0, 10.0)] {
        let edges = circle_at(&part, x, z);
        assert_eq!(edges.len(), 1, "{what}");
        for spec in ["fillet", "chamfer", "chamfer 2"] {
            let run = || match spec {
                "fillet" => fillet(&part, &edges, 2.0),
                "chamfer" => chamfer(&part, &edges, ChamferSpec::Equal(1.5), &[]),
                _ => chamfer(&part, &edges, ChamferSpec::TwoDistances(1.0, 2.0), &[]),
            };
            set_rust_blend(Some(false));
            let occt = run().unwrap();
            set_rust_blend(Some(true));
            reset_blend_decline();
            let ours = run();
            set_rust_blend(None);
            let why = last_blend_decline();
            let ours = ours.unwrap();
            assert_eq!(why, "", "{what} {spec}: declined");
            let d = bool_corpus::Summary::of(&ours).differences(&bool_corpus::Summary::of(&occt));
            assert!(d.is_empty(), "{what} {spec}: {}", d.join(", "));
        }
    }
}

/// noshape's own blends of straight edges ending on caps square to them: a block's four vertical edges
/// (each its own chain, the top's and bottom's edges cut short by two of them), and a step's concave edge,
/// filleted and chamfered: OCCT's result and history.
#[test]
fn rust_blends_of_open_chains_are_occts() {
    let block = make_box_corners(DVec3::ZERO, DVec3::new(40.0, 20.0, 10.0)).unwrap();
    let step = fuse(&block, &make_box_corners(DVec3::new(0.0, 0.0, 10.0), DVec3::new(15.0, 20.0, 20.0)).unwrap()).unwrap().shape;
    let step = unify_same_domain(&step).unwrap().shape;
    let vertical = |s: &Shape| -> Vec<Shape> { s.edges().into_iter().filter(|e| e.edge_info().is_ok_and(|i| (i.start.z - i.end.z).abs() > 9.9 && (i.start.x - i.end.x).abs() < 1e-9 && (i.start.y - i.end.y).abs() < 1e-9 && i.start.z.min(i.end.z) < 1e-9)).collect() };
    let concave = |s: &Shape| -> Vec<Shape> { s.edges().into_iter().filter(|e| e.edge_info().is_ok_and(|i| (i.start.x - 15.0).abs() < 1e-9 && (i.end.x - 15.0).abs() < 1e-9 && (i.start.z - 10.0).abs() < 1e-9 && (i.end.z - 10.0).abs() < 1e-9)).collect() };
    for (what, part, edges) in [("block's verticals", &block, vertical(&block)), ("step's concave edge", &step, concave(&step))] {
        assert!(!edges.is_empty(), "{what}");
        for spec in ["fillet", "chamfer"] {
            let run = || match spec {
                "fillet" => fillet(part, &edges, 2.0),
                _ => chamfer(part, &edges, ChamferSpec::Equal(1.5), &[]),
            };
            set_rust_blend(Some(false));
            let occt = run().unwrap();
            set_rust_blend(Some(true));
            reset_blend_decline();
            let ours = run();
            set_rust_blend(None);
            let why = last_blend_decline();
            let ours = ours.unwrap();
            assert_eq!(why, "", "{what} {spec}: declined");
            let d = bool_corpus::Summary::of(&ours).differences(&bool_corpus::Summary::of(&occt));
            assert!(d.is_empty(), "{what} {spec}: {}", d.join(", "));
        }
    }
}

/// noshape's own blends round corners between blended straight edges whose edge down the walls stays sharp: a
/// block's top loop (four corners), filleted (the cylinders meet along ellipses) and chamfered (planes along
/// lines): OCCT's volume (1e-9), counts, and history to 1e-7 of the part (OCCT approximates the corners' curves).
#[test]
fn rust_blends_round_corners_are_occts() {
    let block = make_box_corners(DVec3::ZERO, DVec3::new(40.0, 20.0, 10.0)).unwrap();
    let top: Vec<Shape> = block.edges().into_iter().filter(|e| e.edge_info().is_ok_and(|i| i.start.z > 9.9 && i.end.z > 9.9)).collect();
    assert_eq!(top.len(), 4);
    for spec in ["fillet", "chamfer"] {
        let run = || match spec {
            "fillet" => fillet(&block, &top, 2.0),
            _ => chamfer(&block, &top, ChamferSpec::Equal(1.5), &[]),
        };
        set_rust_blend(Some(false));
        let occt = run().unwrap();
        set_rust_blend(Some(true));
        reset_blend_decline();
        let ours = run();
        set_rust_blend(None);
        let why = last_blend_decline();
        let ours = ours.unwrap();
        assert_eq!(why, "", "{spec}: declined");
        assert!(ours.shape.is_valid(), "{spec}: invalid");
        let (a, b) = (bool_corpus::Summary::of(&ours), bool_corpus::Summary::of(&occt));
        assert_eq!((a.faces, a.edges, a.vertices), (b.faces, b.edges, b.vertices), "{spec}");
        assert!((a.volume - b.volume).abs() <= 1e-9 * b.volume, "{spec}: {} vs {}", a.volume, b.volume);
        // Exact: the cross-section along the frame's centroid line (mitered at the corners).
        let (r, l, w) = (if spec == "fillet" { 2.0f64 } else { 1.5 }, 40.0, 20.0);
        let (area, inset) = if spec == "fillet" {
            let pi = std::f64::consts::PI;
            (r * r * (1.0 - pi / 4.0), r * (5.0 / 6.0 - pi / 4.0) / (1.0 - pi / 4.0))
        } else {
            (0.5 * r * r, r / 3.0)
        };
        let exact = l * w * 10.0 - area * (2.0 * (l + w) - 8.0 * inset);
        assert!((a.volume - exact).abs() <= 1e-9 * exact, "{spec}: {} vs exact {exact}", a.volume);
    }
}
