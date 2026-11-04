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

// Live G46's chamfer first, then the fillets ending on it: OCCT's fillet
// can't run them into the chamfer face, so the fallback fillets the part
// without it. With noshape's chamfer the block's faces come in another order,
// and removing its sides (square to the fillet edges) too made OCCT's
// defeaturing return a sliver.
#[test]
fn fillets_after_a_rust_chamfer_are_occts() {
    setup();
    let b = bx([0.0; 3], [50.0, 30.0, 30.0]);
    let corner = edges_where(&b, |i| (i.start.x - 50.0 * MM).abs() < 1e-12 && i.start.y.abs() < 1e-12 && (i.end.x - 50.0 * MM).abs() < 1e-12 && i.end.y.abs() < 1e-12);
    assert_eq!(corner.len(), 1);
    let mut got = vec![];
    for rust in [false, true] {
        set_rust_blend(Some(rust));
        reset_blend_decline();
        let c = chamfer_checked(&b, &corner, ChamferSpec::Equal(5.0 * MM), &[]).unwrap();
        assert_eq!(last_blend_decline().is_empty(), rust, "{}", last_blend_decline());
        let top = edges_where(&c.shape, |i| (i.start.z - 30.0 * MM).abs() < 1e-12 && (i.end.z - 30.0 * MM).abs() < 1e-12 && ((i.start.y.abs() < 1e-12 && i.end.y.abs() < 1e-12) || ((i.start.x - 50.0 * MM).abs() < 1e-12 && (i.end.x - 50.0 * MM).abs() < 1e-12)));
        assert_eq!(top.len(), 2);
        let f = fillet_checked(&c.shape, &top, &[3.0 * MM]);
        set_rust_blend(None);
        let f = f.unwrap_or_else(|e| panic!("rust {rust}: {e}"));
        got.push((vol_mm3(&f.shape), f.shape.count(ShapeType::Face), f.shape.count(ShapeType::Edge)));
    }
    assert!((got[0].0 - got[1].0).abs() < 1e-6 * got[0].0, "{got:?}");
    assert_eq!((got[0].1, got[0].2), (got[1].1, got[1].2), "{got:?}");
}

/// noshape's own blends of straight edges ending on caps at an angle to them: a drafted block's slanted vertical
/// edges (the top and bottom tilted against them) and a block's top edges ending on a corner chamfer (a cap at 45°
/// along them), filleted (the cylinder cut by the cap: an ellipse) and chamfered: OCCT's counts and volume (1e-9;
/// OCCT approximates the ellipses).
#[test]
fn rust_blends_on_oblique_caps_are_occts() {
    let block = make_box_corners(DVec3::ZERO, DVec3::new(40.0, 20.0, 10.0)).unwrap();
    let sides: Vec<Shape> = block.faces().into_iter().filter(|f| f.centroid().is_ok_and(|c| c.z > 1.0 && c.z < 9.0)).collect();
    assert_eq!(sides.len(), 4);
    let drafted = draft(&block, &sides, DVec3::Z, &Plane::from_normal(DVec3::ZERO, DVec3::Z), 8f64.to_radians()).unwrap().shape;
    let slanted: Vec<Shape> = drafted.edges().into_iter().filter(|e| e.edge_info().is_ok_and(|i| (i.start.z - i.end.z).abs() > 9.9)).collect();
    assert_eq!(slanted.len(), 4);
    let corner: Vec<Shape> = block.edges().into_iter().filter(|e| e.edge_info().is_ok_and(|i| (i.start.x - 40.0).abs() < 1e-9 && i.start.y.abs() < 1e-9 && (i.end.x - 40.0).abs() < 1e-9 && i.end.y.abs() < 1e-9)).collect();
    let cut = chamfer(&block, &corner, ChamferSpec::Equal(6.0), &[]).unwrap().shape;
    let top: Vec<Shape> = cut.edges().into_iter().filter(|e| e.edge_info().is_ok_and(|i| i.start.z > 9.9 && i.end.z > 9.9 && ((i.start.y.abs() < 1e-9 && i.end.y.abs() < 1e-9) || ((i.start.x - 40.0).abs() < 1e-9 && (i.end.x - 40.0).abs() < 1e-9)))).collect();
    assert_eq!(top.len(), 2);
    for (what, part, edges) in [("drafted block's edges", &drafted, slanted), ("top edges to a chamfer", &cut, top)] {
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
            assert!(ours.shape.is_valid(), "{what} {spec}: invalid");
            let (a, b) = (bool_corpus::Summary::of(&ours), bool_corpus::Summary::of(&occt));
            assert_eq!((a.faces, a.edges, a.vertices), (b.faces, b.edges, b.vertices), "{what} {spec}");
            assert!((a.volume - b.volume).abs() <= 1e-7 * b.volume, "{what} {spec}: {} vs {}", a.volume, b.volume);
            if what == "top edges to a chamfer" && spec == "fillet" {
                // Exact: the strips' section area times their centroid lines, from the square caps to the chamfer
                // plane x - y = 34 (OCCT's is 8e-5 mm³ off).
                let (r, pi) = (2.0f64, std::f64::consts::PI);
                let (area, inset) = (r * r * (1.0 - pi / 4.0), r * (5.0 / 6.0 - pi / 4.0) / (1.0 - pi / 4.0));
                let exact = 40.0 * 20.0 * 10.0 - 0.5 * 36.0 * 10.0 - area * ((34.0 + inset) + (20.0 - (6.0 - inset)));
                assert!((a.volume - exact).abs() <= 1e-9 * exact, "{what} {spec}: {} vs exact {exact}", a.volume);
            }
        }
    }
}

/// noshape's own distance-angle chamfers (OCCT's AddDA: the distance on the reference face, the angle there): a
/// block's vertical edges and a hole's rim, at 30° from either face: OCCT's result and history.
#[test]
fn rust_distance_angle_chamfers_are_occts() {
    let plate = make_box_corners(DVec3::ZERO, DVec3::new(40.0, 20.0, 10.0)).unwrap();
    let hole = make_cylinder(&Plane::new(DVec3::new(20.0, 10.0, -1.0), DVec3::Z, DVec3::X), 5.0, 12.0, std::f64::consts::TAU).unwrap();
    let part = cut(&plate, &hole).unwrap().shape;
    let vertical: Vec<Shape> = part.edges().into_iter().filter(|e| e.edge_info().is_ok_and(|i| (i.start.z - i.end.z).abs() > 9.9 && matches!(i.curve, Curve::Line { .. }) && (i.start.x.abs() < 1e-9 || (i.start.x - 40.0).abs() < 1e-9))).collect();
    let rim: Vec<Shape> = part.edges().into_iter().filter(|e| e.edge_info().is_ok_and(|i| matches!(i.curve, Curve::Circle { center, .. } if center.z > 9.9))).collect();
    let top = part.faces().into_iter().find(|f| f.centroid().is_ok_and(|c| c.z > 9.9)).unwrap();
    let front = part.faces().into_iter().find(|f| f.centroid().is_ok_and(|c| c.y.abs() < 1e-9)).unwrap();
    assert_eq!((vertical.len(), rim.len()), (4, 1));
    let corner = vec![vertical.iter().find(|e| e.edge_info().is_ok_and(|i| i.start.x.abs() < 1e-9 && i.start.y.abs() < 1e-9)).unwrap().clone()];
    for (what, edges, face) in [("a block's corner, front face", &corner, &front), ("a hole's rim, top face", &rim, &top)] {
        for angle in [30f64, 60.0] {
            let run = || chamfer(&part, edges, ChamferSpec::DistanceAngle(2.0, angle.to_radians()), std::slice::from_ref(face));
            set_rust_blend(Some(false));
            let occt = run().unwrap();
            set_rust_blend(Some(true));
            reset_blend_decline();
            let ours = run();
            set_rust_blend(None);
            let why = last_blend_decline();
            let ours = ours.unwrap();
            assert_eq!(why, "", "{what} {angle}°: declined");
            let d = bool_corpus::Summary::of(&ours).differences(&bool_corpus::Summary::of(&occt));
            assert!(d.is_empty(), "{what} {angle}°: {}", d.join(", "));
        }
    }
}

/// noshape's own vertex blends: three blended edges at a block's corner (each order: OCCT's sphere takes its
/// frame from it) and all twelve of its edges, filleted (a sphere's octant, a degenerate edge at its pole) and
/// chamfered (a triangle): OCCT's result and history.
#[test]
fn rust_vertex_blends_are_occts() {
    let block = make_box_corners(DVec3::ZERO, DVec3::new(60.0, 40.0, 20.0)).unwrap();
    let corner = DVec3::new(60.0, 40.0, 20.0);
    let at: Vec<Shape> = block.edges().into_iter().filter(|e| e.edge_info().is_ok_and(|i| i.start.distance(corner) < 1e-9 || i.end.distance(corner) < 1e-9)).collect();
    assert_eq!(at.len(), 3);
    let mut cases: Vec<(String, Vec<Shape>)> = [[0, 1, 2], [0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0]].iter().map(|o| (format!("corner {o:?}"), o.iter().map(|&i| at[i].clone()).collect())).collect();
    cases.push(("all edges".into(), block.edges()));
    for (what, edges) in &cases {
        for spec in ["fillet", "chamfer"] {
            let run = || match spec {
                "fillet" => fillet(&block, edges, 3.0),
                _ => chamfer(&block, edges, ChamferSpec::Equal(2.0), &[]),
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
            assert!(ours.shape.is_valid(), "{what} {spec}: invalid");
            let d = bool_corpus::Summary::of(&ours).differences(&bool_corpus::Summary::of(&occt));
            assert!(d.is_empty(), "{what} {spec}: {}", d.join(", "));
        }
    }
}

/// noshape's own chamfers' planes framed as OCCT's (a sketch on the face takes its axes from them: qa's
/// BLEND-RUST-DEFAULT-REGEN, a sketch on a chamfer turned a quarter): every planar face's origin, x and normal,
/// and its outward normal, the same as OCCT's, for single edges with and without a reference face and a top loop
/// with corners.
#[test]
fn rust_chamfer_planes_are_framed_as_occts() {
    let block = make_box_corners(DVec3::ZERO, DVec3::new(60.0, 40.0, 20.0)).unwrap();
    let mut cases: Vec<(String, Vec<Shape>, Vec<Shape>, ChamferSpec)> = Vec::new();
    for (i, e) in block.edges().into_iter().enumerate() {
        cases.push((format!("edge {i}"), vec![e.clone()], vec![], ChamferSpec::Equal(2.0)));
        let faces: Vec<Shape> = block.faces().into_iter().filter(|f| f.edges().iter().any(|x| x.is_same(&e))).collect();
        for (k, f) in faces.iter().enumerate() {
            cases.push((format!("edge {i} ref {k}"), vec![e.clone()], vec![f.clone()], ChamferSpec::TwoDistances(2.0, 3.0)));
            cases.push((format!("edge {i} ref {k} angle"), vec![e.clone()], vec![f.clone()], ChamferSpec::DistanceAngle(2.0, 0.6)));
        }
    }
    let top: Vec<Shape> = block.edges().into_iter().filter(|e| e.edge_info().is_ok_and(|i| i.start.z > 19.9 && i.end.z > 19.9)).collect();
    cases.push(("top loop".into(), top, vec![], ChamferSpec::Equal(2.0)));
    let frames = |s: &Shape| -> Vec<String> {
        let mut v: Vec<String> = s
            .faces()
            .into_iter()
            .filter_map(|f| {
                let Ok(Surface::Plane(p)) = f.face_surface() else { return None };
                let o = f.face_plane().ok()??;
                let k = |x: f64| (x * 1e6).round() / 1e6 + 0.0;
                let r = |v: DVec3| format!("({:.6} {:.6} {:.6})", k(v.x), k(v.y), k(v.z));
                Some(format!("{} x {} n {} out {}", r(p.origin), r(p.x_dir), r(p.normal), r(o.normal)))
            })
            .collect();
        v.sort();
        v
    };
    for (what, edges, refs, spec) in &cases {
        let refs = if refs.is_empty() { vec![] } else { vec![refs[0].clone(); edges.len()] };
        set_rust_blend(Some(false));
        let occt = chamfer(&block, edges, *spec, &refs).unwrap();
        set_rust_blend(Some(true));
        reset_blend_decline();
        let ours = chamfer(&block, edges, *spec, &refs);
        set_rust_blend(None);
        let why = last_blend_decline();
        let ours = ours.unwrap();
        assert_eq!(why, "", "{what}: declined");
        assert_eq!(frames(&ours.shape), frames(&occt.shape), "{what}");
    }
}

/// Live request 1646: a 60 x 40 x 20 block, R5 on its vertical edges, then the whole top loop (4 lines, 4 arcs)
/// blended by as much: the arcs' offsets close to their centres. Live: a fillet's corners are sphere octants
/// (R5 about the point 5 below each arc's centre), a chamfer's 45° cones with their apex on the top face; both
/// 18 / 36 / 20. noshape's blends build those (OCCT's are B-spline patches).
#[test]
fn rust_blends_close_arcs_like_onshape() {
    setup();
    let b = bx([0.0; 3], [60.0, 40.0, 20.0]);
    let v = edges_where(&b, |i| (i.start.z - i.end.z).abs() > 1e-9);
    let s = fillet_checked(&b, &v, &[5.0 * MM]).unwrap().shape;
    let top = edges_where(&s, |i| i.start.z > 19.99 * MM && i.end.z > 19.99 * MM);
    assert_eq!(top.len(), 8);
    let count = |s: &Shape, want: fn(&Surface) -> bool| s.sub_shapes(ShapeType::Face).iter().filter(|f| f.face_surface().is_ok_and(|x| want(&x))).count();
    set_rust_blend(Some(true));
    let f = fillet_checked(&s, &top, &[5.0 * MM]);
    let fd = last_blend_decline();
    let c = chamfer_checked(&s, &top, ChamferSpec::Equal(5.0 * MM), &[]);
    let cd = last_blend_decline();
    set_rust_blend(None);
    let (f, c) = (f.unwrap().shape, c.unwrap().shape);
    assert_eq!(fd, "", "fillet declined");
    assert_eq!(cd, "", "chamfer declined");
    for (s, want, what) in [(&f, 46581.48928648512, "fillet"), (&c, 45308.99693899578, "chamfer")] {
        assert!(s.is_valid(), "{what}: invalid");
        assert!((vol_mm3(s) - want).abs() < 1e-5, "{what}: {} vs live {want}", vol_mm3(s));
        // (Live's counts; ours have a degenerate edge at each pole besides.)
        assert_eq!((s.count(ShapeType::Face), edges_where(s, |_| true).len(), s.count(ShapeType::Vertex)), (18, 36, 20), "{what}");
    }
    assert_eq!(count(&f, |x| matches!(x, Surface::Sphere { .. })), 4);
    assert_eq!(count(&c, |x| matches!(x, Surface::Cone { .. })), 4);
}

/// noshape's own blends round a drafted block's top loop (live N5's reapplied R2: the walls turn at the
/// corners, the blends meet in the plane the two walls are mirror images in): OCCT's result and history, and
/// the exact volume (the frustum less the section times its centroid line round the corners).
#[test]
fn rust_blends_on_drafted_walls_are_occts() {
    let block = make_box_corners(DVec3::ZERO, DVec3::new(40.0, 20.0, 10.0)).unwrap();
    let sides: Vec<Shape> = block.faces().into_iter().filter(|f| f.centroid().is_ok_and(|c| c.z > 1.0 && c.z < 9.0)).collect();
    assert_eq!(sides.len(), 4);
    let pi = std::f64::consts::PI;
    for pull in [DVec3::Z, -DVec3::Z] {
        let drafted = draft(&block, &sides, pull, &Plane::from_normal(DVec3::ZERO, DVec3::Z), 5f64.to_radians()).unwrap().shape;
        let top: Vec<Shape> = drafted.edges().into_iter().filter(|e| e.edge_info().is_ok_and(|i| i.start.z > 9.9 && i.end.z > 9.9)).collect();
        assert_eq!(top.len(), 4);
        // The top's sides and the walls' angle to it (inside the material).
        let bb = drafted.bounding_box(false).unwrap();
        let widening = bb.max.x > 40.0 + 1e-6;
        let k = if widening { 5f64.to_radians().tan() } else { -5f64.to_radians().tan() };
        let (a, b, h) = (40.0 + 20.0 * k, 20.0 + 20.0 * k, 10.0);
        let frustum = 40.0 * 20.0 * h + (40.0 + 20.0) * k * h * h + 4.0 / 3.0 * k * k * h * h * h;
        let alpha = if widening { 0.5 * pi - 5f64.to_radians() } else { 0.5 * pi + 5f64.to_radians() };
        for spec in ["fillet", "chamfer"] {
            let what = format!("{} {spec}", if widening { "widening" } else { "narrowing" });
            let run = || match spec {
                "fillet" => fillet(&drafted, &top, 2.0),
                _ => chamfer(&drafted, &top, ChamferSpec::Equal(1.5), &[]),
            };
            set_rust_blend(Some(false));
            let occt = run().unwrap();
            set_rust_blend(Some(true));
            reset_blend_decline();
            let ours = run();
            set_rust_blend(None);
            let why = last_blend_decline();
            let ours = ours.unwrap();
            assert_eq!(why, "", "{what}: declined");
            assert!(ours.shape.is_valid(), "{what}: invalid");
            let (x, y) = (bool_corpus::Summary::of(&ours), bool_corpus::Summary::of(&occt));
            assert_eq!((x.faces, x.edges, x.vertices), (y.faces, y.edges, y.vertices), "{what}");
            assert!((x.volume - y.volume).abs() <= 1e-7 * y.volume, "{what}: {} vs {}", x.volume, y.volume);
            // The section (the edge at 0, P along +x, the wall at alpha from it): its area and centroid's x.
            let (area, inset) = if spec == "fillet" {
                let r = 2.0f64;
                let d = r / (0.5 * alpha).tan();
                let cx = r / (0.5 * alpha).sin() * (0.5 * alpha).cos();
                let kite = d * r;
                let kite_x = 0.5 * d * r * (d + cx) / 3.0 + 0.5 * d * r * (cx + d * alpha.cos()) / 3.0;
                let phi = pi - alpha;
                let sector = 0.5 * phi * r * r;
                let sector_x = sector * (cx - (0.5 * alpha).cos() * 4.0 * r * (0.5 * phi).sin() / (3.0 * phi));
                (kite - sector, (kite_x - sector_x) / (kite - sector))
            } else {
                let c = 1.5f64;
                (0.5 * c * c * alpha.sin(), (c + c * alpha.cos()) / 3.0)
            };
            let exact = frustum - area * (2.0 * (a + b) - 8.0 * inset);
            assert!((x.volume - exact).abs() <= 1e-9 * exact, "{what}: {} vs exact {exact}", x.volume);
        }
    }
}

/// The same round a drafted boss (on a plate, its sides drafted 5° from the plate's top): its base loop (concave)
/// and its top loop (convex), fillet and chamfer: OCCT's counts and volume.
#[test]
fn rust_blends_round_a_drafted_boss_are_occts() {
    let plate = make_box_corners(DVec3::ZERO, DVec3::new(60.0, 40.0, 5.0)).unwrap();
    let boss = make_box_corners(DVec3::new(20.0, 10.0, 5.0), DVec3::new(40.0, 30.0, 15.0)).unwrap();
    let part = fuse(&plate, &boss).unwrap().shape;
    let sides: Vec<Shape> = part.faces().into_iter().filter(|f| f.centroid().is_ok_and(|c| c.z > 6.0 && c.z < 14.0)).collect();
    assert_eq!(sides.len(), 4);
    let drafted = draft(&part, &sides, DVec3::Z, &Plane::from_normal(DVec3::new(0.0, 0.0, 5.0), DVec3::Z), 5f64.to_radians()).unwrap().shape;
    let inside = |p: DVec3| p.x > 15.0 && p.x < 45.0 && p.y > 5.0 && p.y < 35.0;
    let lp = |z: f64| -> Vec<Shape> { drafted.edges().into_iter().filter(|e| e.edge_info().is_ok_and(|i| (i.start.z - z).abs() < 1e-9 && (i.end.z - z).abs() < 1e-9 && inside(i.start) && inside(i.end))).collect() };
    for (what, edges) in [("base", lp(5.0)), ("top", lp(15.0))] {
        assert_eq!(edges.len(), 4, "{what}");
        for spec in ["fillet", "chamfer"] {
            let run = || match spec {
                "fillet" => fillet(&drafted, &edges, 2.0),
                _ => chamfer(&drafted, &edges, ChamferSpec::Equal(1.5), &[]),
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
            assert!(ours.shape.is_valid(), "{what} {spec}: invalid");
            let (x, y) = (bool_corpus::Summary::of(&ours), bool_corpus::Summary::of(&occt));
            assert_eq!((x.faces, x.edges, x.vertices), (y.faces, y.edges, y.vertices), "{what} {spec}");
            assert!((x.volume - y.volume).abs() <= 1e-7 * y.volume, "{what} {spec}: {} vs {}", x.volume, y.volume);
        }
    }
}

/// Live 1686 (b): R4 on a block's vertical edges and R2 round its top, as two fillets. Live's result is the same
/// in either order (each corner the torus of the R2 round the R4 arc, 8759.36625936351 mm³, 18 faces); OCCT's
/// R4 after the R2 failed. The R2 blends are taken off, the R4 made, the R2 made again; each R2 face taken off is
/// Modified to the R2 faces made again. One vertical edge: the same as with it first.
#[test]
fn a_fillet_over_smaller_blends_is_as_with_it_first() {
    setup();
    let b = bx([0.0; 3], [45.0, 20.0, 10.0]);
    let vert = |s: &Shape| edges_where(s, |i| (i.start.z - i.end.z).abs() > 1e-9 && (i.start.x - i.end.x).abs() < 1e-9 && (i.start.y - i.end.y).abs() < 1e-9);
    let top = |s: &Shape| edges_where(s, |i| i.start.z > 9.99 * MM && i.end.z > 9.99 * MM);
    let corner = |s: &Shape| -> Vec<Shape> { vert(s).into_iter().filter(|e| e.edge_info().map(|i| i.start.x > 44.0 * MM && i.start.y > 19.0 * MM).unwrap_or(false)).collect() };
    let first = |pick: &dyn Fn(&Shape) -> Vec<Shape>| {
        let s = fillet_checked(&b, &pick(&b), &[4.0 * MM]).unwrap().shape;
        fillet_checked(&s, &top(&s), &[2.0 * MM]).unwrap().shape
    };
    let all: &dyn Fn(&Shape) -> Vec<Shape> = &vert;
    let one: &dyn Fn(&Shape) -> Vec<Shape> = &corner;
    for (name, pick, want) in [("all four", all, Some(8759.36625936351)), ("one", one, None)] {
        let s = fillet_checked(&b, &top(&b), &[2.0 * MM]).unwrap().shape;
        let edges = pick(&s);
        let res = fillet_checked(&s, &edges, &[4.0 * MM]).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(res.shape.is_valid(), "{name}");
        let like = first(pick);
        let v = vol_mm3(&res.shape);
        assert!((v - vol_mm3(&like)).abs() < 1e-6, "{name}: {v} vs {}", vol_mm3(&like));
        assert_eq!(res.shape.faces().len(), like.faces().len(), "{name}");
        if let Some(w) = want {
            assert!((v - w).abs() < 1e-3, "{name}: {v}");
            assert_eq!(res.shape.faces().len(), 18);
        }
        // The R2 faces live on as the R2 faces made again.
        for (i, f) in s.faces().iter().enumerate() {
            if format!("{:?}", f.face_surface().unwrap()).starts_with("Cylinder") {
                assert!(!res.history.modified(0, ShapeType::Face, i as u32).is_empty(), "{name}: R2 face {i}");
            }
        }
    }
}

/// Live 1686 (c): a 60 x 40 x 20 block drafted 5° (narrowing upward), R3 on its four slanted edges and R2 round its
/// top in one fillet (Draft's reapply makes this call): live 44304.933893372094 mm³, 18 faces, each corner one
/// OTHER face. OCCT's corner patches are within 0.02 mm³; their orientation search walked ~1e5 steps per corner
/// (5.5 s of the call) until OCCT patch 0014 (80 ms).
#[test]
fn a_drafted_blocks_mixed_radius_corners_are_lives() {
    setup();
    let b = bx([0.0; 3], [60.0, 40.0, 20.0]);
    let sides: Vec<Shape> = b.faces().into_iter().filter(|f| f.bounding_box(false).is_ok_and(|bb| bb.max.z - bb.min.z > 1.0 * MM)).collect();
    let d = draft(&b, &sides, DVec3::Z, &Plane::new(DVec3::ZERO, DVec3::Z, DVec3::X), 5f64.to_radians()).unwrap().shape;
    let mut es = edges_where(&d, |i| (i.start.z - i.end.z).abs() > 1.0 * MM);
    let n = es.len();
    es.extend(edges_where(&d, |i| i.start.z > 19.99 * MM && i.end.z > 19.99 * MM));
    assert_eq!((n, es.len()), (4, 8));
    let radii: Vec<f64> = (0..es.len()).map(|i| if i < n { 3.0 * MM } else { 2.0 * MM }).collect();
    let r = fillet_checked(&d, &es, &radii).unwrap().shape;
    assert!(r.is_valid());
    assert!((vol_mm3(&r) - 44304.933893372094).abs() < 0.05, "{}", vol_mm3(&r));
    assert_eq!(r.faces().len(), 18);
}

/// Live 1686 (c) as two Fillet features: R3 on the drafted block's slanted edges, then R2 round its top loop (4 lines
/// and the R3 rounds' 4 arcs). Live: 18 faces, each corner one 4-sided OTHER face of 10.1582447 mm² in (55.42137,
/// 35.42137, 18.174311) .. (58.409954, 38.409954, 20); 44304.933893 mm³. OCCT's left each corner two patches meeting
/// in a degenerated edge, its vertex 0.9 mm off the top face (22 faces, 44305.088 mm³); now the R2 ball rolling on the
/// top and the R3 cylinder (ours 44304.984 mm³: within live's mass-property band, its corner areas to 1e-7).
#[test]
fn a_drafted_blocks_top_loop_over_its_corner_rounds_is_lives() {
    setup();
    let b = bx([0.0; 3], [60.0, 40.0, 20.0]);
    let sides: Vec<Shape> = b.faces().into_iter().filter(|f| f.bounding_box(false).is_ok_and(|bb| bb.max.z - bb.min.z > 1.0 * MM)).collect();
    let d = draft(&b, &sides, DVec3::Z, &Plane::new(DVec3::ZERO, DVec3::Z, DVec3::X), 5f64.to_radians()).unwrap().shape;
    let slanted = edges_where(&d, |i| (i.start.z - i.end.z).abs() > 1.0 * MM);
    let d = fillet_checked(&d, &slanted, &[3.0 * MM; 4]).unwrap().shape;
    let top = edges_where(&d, |i| i.start.z > 19.99 * MM && i.end.z > 19.99 * MM);
    assert_eq!(top.len(), 8);
    let r = fillet_checked(&d, &top, &[2.0 * MM; 8]).unwrap().shape;
    assert!(r.is_valid());
    assert_eq!(r.faces().len(), 18);
    assert!((vol_mm3(&r) - 44304.933893372094).abs() < 0.1, "{}", vol_mm3(&r));
    assert!(r.edges().iter().all(|e| !e.edge_info().unwrap().degenerated));
    let top_face = r.faces().into_iter().find(|f| f.bounding_box(true).is_ok_and(|bb| bb.min.z > 19.99 * MM)).unwrap();
    assert!(top_face.vertices().iter().all(|v| (v.vertex_point().unwrap().z - 20.0 * MM).abs() < 1e-6 * MM));
    let corner = r.faces().into_iter().find(|f| f.bounding_box(true).is_ok_and(|bb| bb.min.x > 55.0 * MM && bb.min.y > 35.0 * MM && bb.min.z > 18.0 * MM)).unwrap();
    let (a, bb) = (corner.area().unwrap(), corner.bounding_box(true).unwrap());
    assert!((a / (MM * MM) - 10.1582447).abs() < 1e-5, "{}", a / (MM * MM));
    let (lo, hi) = (DVec3::new(55.42137, 35.42137, 18.174311) * MM, DVec3::new(58.409954, 38.409954, 20.0) * MM);
    assert!((bb.min - lo).abs().max_element() < 2e-4 * MM && (bb.max - hi).abs().max_element() < 2e-4 * MM, "{:?} .. {:?}", bb.min / MM, bb.max / MM);
}

/// Blends that come close but stay apart are noshape's, as OCCT's: R4.99 round all twelve edges of a 40 x 20 x 10
/// block (the top and bottom loops' blends 0.02 apart on the sides). Declined before: the two lines' distance was
/// sampled (9.79 for 10) and each chain counted its ball's centre (r√2) as its reach.
#[test]
fn rust_blends_close_but_apart_are_occts() {
    let block = make_box_corners(DVec3::ZERO, DVec3::new(40.0, 20.0, 10.0)).unwrap();
    let edges = block.edges();
    set_rust_blend(Some(false));
    let occt = fillet(&block, &edges, 4.99).unwrap();
    set_rust_blend(Some(true));
    reset_blend_decline();
    let ours = fillet(&block, &edges, 4.99);
    set_rust_blend(None);
    let why = last_blend_decline();
    let ours = ours.unwrap();
    assert_eq!(why, "", "declined");
    assert!(ours.shape.is_valid());
    let d = bool_corpus::Summary::of(&ours).differences(&bool_corpus::Summary::of(&occt));
    assert!(d.is_empty(), "{}", d.join(", "));
}

/// Live "kern f2 W1": a 60 x 40 x 20 block with a 4 x 40 x 5 rib on top, R2 on both of the rib's top edges: a full
/// round, one half-cylinder (48731.32741228718 mm³, 10 faces: 9 planes, 1 cylinder). OCCT's (a hair smaller, the
/// rib top removed) is two cylinders on one axis, merged. R2.5 (past half the rib): two cylinders, as live.
#[test]
fn a_full_round_is_one_face_as_lives() {
    setup();
    let rib = unify_same_domain(&fuse(&bx([0.0; 3], [60.0, 40.0, 20.0]), &bx([28.0, 0.0, 20.0], [32.0, 40.0, 25.0])).unwrap().shape).unwrap().shape;
    assert_eq!(rib.faces().len(), 10);
    let top = edges_where(&rib, |i| i.start.z > 24.99 * MM && i.end.z > 24.99 * MM && (i.start.y - i.end.y).abs() > 1.0 * MM);
    assert_eq!(top.len(), 2);
    let cylinders = |s: &Shape| s.faces().iter().filter(|f| matches!(f.face_surface(), Ok(Surface::Cylinder { .. }))).count();
    for (r, want, faces, cyl) in [(2.0, 48731.32741228718, 10, 1), (2.5, 48693.369806645474, 11, 2)] {
        let res = fillet_checked(&rib, &top, &[r * MM]).unwrap_or_else(|e| panic!("R{r}: {e}"));
        assert!(res.shape.is_valid(), "R{r}");
        assert!((vol_mm3(&res.shape) - want).abs() < 1e-3, "R{r}: {}", vol_mm3(&res.shape));
        assert_eq!((res.shape.faces().len(), cylinders(&res.shape)), (faces, cyl), "R{r}");
    }
}

/// noshape's own rolling-ball corners (a top loop R over vertical corner rounds Rc < R, live G24 / G25): the same
/// counts and volume as OCCT's fillet with its corner patches replaced by the exact face (vertex_blends_match_onshape
/// holds those to live), valid. The corpus's twelve "an arc the blend closes up" cases are these.
#[test]
fn rust_rolling_ball_corners_are_occts() {
    setup();
    for (h, rc, r) in [(10.0, 3.0, 6.0), (10.0, 2.0, 4.0), (10.0, 1.0, 4.0), (12.0, 3.0, 8.0), (10.0, 3.0, 3.5), (10.0, 3.0, 4.0)] {
        let b = bx([0.0; 3], [40.0, 20.0, h]);
        let v = edges_where(&b, |i| (i.start.z - i.end.z).abs() > 1e-9);
        set_rust_blend(Some(false));
        let s = fillet_checked(&b, &v, &[rc * MM]).unwrap().shape;
        let top = edges_where(&s, |i| i.start.z > (h - 0.01) * MM && i.end.z > (h - 0.01) * MM);
        let occt = fillet_checked(&s, &top, &[r * MM]).unwrap();
        set_rust_blend(Some(true));
        reset_blend_decline();
        let ours = fillet_checked(&s, &top, &[r * MM]);
        set_rust_blend(None);
        let why = last_blend_decline();
        let ours = ours.unwrap_or_else(|e| panic!("H{h} Rc{rc} R{r}: {e} ({why})"));
        assert_eq!(why, "", "H{h} Rc{rc} R{r}: declined");
        assert!(ours.shape.is_valid(), "H{h} Rc{rc} R{r}: invalid");
        let (vo, vu) = (vol_mm3(&occt.shape), vol_mm3(&ours.shape));
        assert!((vo - vu).abs() < 1e-7 * vo, "H{h} Rc{rc} R{r}: {vu} vs OCCT's {vo}");
        for k in [ShapeType::Face, ShapeType::Edge, ShapeType::Vertex] {
            assert_eq!(ours.shape.count(k), occt.shape.count(k), "H{h} Rc{rc} R{r}: {k:?}");
        }
    }
}

/// A drafted pocket's floor loop R2 over its slanted corner rounds R3 (the concave form of 1686 (c), live 1826): 23
/// faces, each corner one face of 10.158244513 mm², the volume in live's band (40308.406618 [40303.107547,
/// 40313.705689]).
#[test]
fn a_drafted_pockets_floor_loop_over_its_corner_rounds_is_lives() {
    setup();
    let b = bx([0.0; 3], [60.0, 40.0, 20.0]);
    let p = cut(&b, &bx([15.0, 10.0, 8.0], [45.0, 30.0, 21.0])).unwrap().shape;
    let walls: Vec<Shape> = p.faces().into_iter().filter(|f| f.bounding_box(false).is_ok_and(|bb| bb.min.z > 7.9 * MM && bb.max.z - bb.min.z > 1.0 * MM && bb.min.x > 14.9 * MM && bb.max.x < 45.1 * MM && bb.min.y > 9.9 * MM && bb.max.y < 30.1 * MM)).collect();
    assert_eq!(walls.len(), 4);
    let d = draft(&p, &walls, DVec3::Z, &Plane::new(DVec3::new(0.0, 0.0, 8.0 * MM), DVec3::Z, DVec3::X), 5f64.to_radians()).unwrap().shape;
    let slanted = edges_where(&d, |i| (i.start.z - i.end.z).abs() > 1.0 * MM && i.start.z > 7.9 * MM && i.end.z > 7.9 * MM && i.start.x > 10.0 * MM && i.start.x < 50.0 * MM && i.start.y > 5.0 * MM && i.start.y < 35.0 * MM);
    assert_eq!(slanted.len(), 4);
    let d = fillet_checked(&d, &slanted, &[3.0 * MM; 4]).unwrap().shape;
    let floor = edges_where(&d, |i| (i.start.z - 8.0 * MM).abs() < 1e-6 * MM && (i.end.z - 8.0 * MM).abs() < 1e-6 * MM);
    let r = fillet_checked(&d, &floor, &vec![2.0 * MM; floor.len()]).unwrap().shape;
    assert!(r.is_valid());
    assert!(r.edges().iter().all(|e| !e.edge_info().unwrap().degenerated));
    assert_eq!(r.faces().len(), 23);
    let v = vol_mm3(&r);
    assert!((40303.107547..40313.705689).contains(&v), "{v}");
    let corner = r.faces().into_iter().find(|f| f.bounding_box(true).is_ok_and(|bb| bb.min.x > 42.0 * MM && bb.min.y > 27.0 * MM && bb.max.z < 10.0 * MM)).unwrap();
    assert!((corner.area().unwrap() / (MM * MM) - 10.158244513).abs() < 1e-5, "{}", corner.area().unwrap() / (MM * MM));
}

/// The drafted block of 1686 (c) with slanted rounds smaller than the top loop's (R1 under R2, live 1831): each corner
/// is the R1 ball rolling between the R2 cylinders from where the slanted R1 axis meets its spine up to the top face's
/// corner, 2.989487134 mm² (OCCT's patch 3.605); 18 faces; the volume in live's band (44437.600073 [44426.355232,
/// 44448.844913]).
#[test]
fn drafted_small_corner_rounds_under_the_top_loop_are_lives() {
    setup();
    let b = bx([0.0; 3], [60.0, 40.0, 20.0]);
    let sides: Vec<Shape> = b.faces().into_iter().filter(|f| f.bounding_box(false).is_ok_and(|bb| bb.max.z - bb.min.z > 1.0 * MM)).collect();
    let d = draft(&b, &sides, DVec3::Z, &Plane::new(DVec3::ZERO, DVec3::Z, DVec3::X), 5f64.to_radians()).unwrap().shape;
    let slanted = edges_where(&d, |i| (i.start.z - i.end.z).abs() > 1.0 * MM);
    let d = fillet_checked(&d, &slanted, &[1.0 * MM; 4]).unwrap().shape;
    let top = edges_where(&d, |i| i.start.z > 19.99 * MM && i.end.z > 19.99 * MM);
    let r = fillet_checked(&d, &top, &vec![2.0 * MM; top.len()]).unwrap().shape;
    assert!(r.is_valid());
    assert_eq!(r.faces().len(), 18);
    let v = vol_mm3(&r);
    assert!((44426.355232..44448.844913).contains(&v), "{v}");
    let corner = r.faces().into_iter().find(|f| f.bounding_box(true).is_ok_and(|bb| bb.min.x > 56.0 * MM && bb.min.y > 36.0 * MM && bb.min.z > 18.0 * MM)).unwrap();
    assert!((corner.area().unwrap() / (MM * MM) - 2.989487134).abs() < 1e-5, "{}", corner.area().unwrap() / (MM * MM));
}

/// The largest vertex tolerance of a shape (its B-rep text).
fn max_vertex_tolerance(s: &Shape) -> f64 {
    let bytes = s.to_brep_bytes().unwrap();
    let text = String::from_utf8_lossy(&bytes);
    let mut lines = text.lines();
    let mut worst = 0.0f64;
    while let Some(l) = lines.next() {
        if l.trim() == "Ve"
            && let Some(t) = lines.next().and_then(|t| t.split_whitespace().next()).and_then(|t| t.parse::<f64>().ok())
        {
            worst = worst.max(t);
        }
    }
    worst
}

/// A dumped chamfer (NOSHAPE_DUMP_OPS: CHAMFER_CASE=<dir>/chamfer-<n>, 0-based sub-shape indices) again: the largest
/// vertex tolerance before and after.
#[test]
#[ignore = "T2 tool: CHAMFER_CASE"]
fn dumped_chamfer_tolerance_probe() {
    let Some(case) = std::env::var_os("CHAMFER_CASE") else { return };
    let case = std::path::PathBuf::from(case);
    let s = read_brep(&case.with_extension("brep")).unwrap();
    let spec = std::fs::read_to_string(case.with_extension("txt")).unwrap();
    let (edges, faces) = (s.edges(), s.faces());
    let es: Vec<Shape> = spec.lines().filter_map(|l| l.strip_prefix("edge ")).filter_map(|n| n.trim().parse::<usize>().ok()).map(|n| edges[n].clone()).collect();
    let fs: Vec<Shape> = spec.lines().filter_map(|l| l.strip_prefix("face ")).filter_map(|n| n.trim().parse::<usize>().ok()).map(|n| faces[n].clone()).collect();
    let head: Vec<&str> = spec.lines().find(|l| l.starts_with("chamfer ")).unwrap().split_whitespace().collect();
    let (mode, d1, d2): (u8, f64, f64) = (head[4].parse().unwrap(), head[6].parse().unwrap(), head[8].parse().unwrap());
    let sp = match mode { 0 => ChamferSpec::Equal(d1), 1 => ChamferSpec::TwoDistances(d1, d2), _ => ChamferSpec::DistanceAngle(d1, d2) };
    println!("input: {} edges chamfered, mode {mode}, largest vertex tolerance {:.3e}", es.len(), max_vertex_tolerance(&s));
    let r = chamfer_checked(&s, &es, sp, &fs).unwrap();
    println!("result: largest vertex tolerance {:.3e}, valid {}, blend decline {:?}; the input's after it {:.3e}", max_vertex_tolerance(&r.shape), r.shape.is_valid(), last_blend_decline(), max_vertex_tolerance(&s));
}

/// shell-hang's Chamfer 4 (8 mm on two edges; its input as the document has it, metres): OCCT's chamfer in metres
/// left its approximation tolerance, 0.1 mm, on the vertices it made, and every later Shell's slab fuse of that part
/// came out broken (Shell 5 failed after 31 s; live builds it). Its corners are exact: the vertices get 1e-7.
#[test]
fn a_chamfer_in_metres_keeps_tight_vertices() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/shell");
    let s = read_brep(&dir.join("shellhang-chamfer4-input.brep")).unwrap();
    let edges = s.edges();
    let before = vol_mm3(&s);
    let r = chamfer_checked(&s, &[edges[1].clone(), edges[2].clone()], ChamferSpec::Equal(0.008), &[]).unwrap().shape;
    assert!(r.is_valid());
    assert!(max_vertex_tolerance(&r) < 1e-6, "{:e}", max_vertex_tolerance(&r));
    assert!(vol_mm3(&r) < before);
}

/// A dumped fillet (NOSHAPE_DUMP_OPS: FILLET_CASE=<dir>/fillet-<n>, 0-based edge indices) run FILLET_RUNS times
/// (default 20): with MALLOC_CHECK_=3 / MALLOC_PERTURB_ a blend that writes past its memory shows (qa CRASH-2045).
#[test]
#[ignore = "T2 tool: FILLET_CASE"]
fn dumped_fillet_repeat_probe() {
    let Some(case) = std::env::var_os("FILLET_CASE") else { return };
    let case = std::path::PathBuf::from(case);
    let s = read_brep(&case.with_extension("brep")).unwrap();
    let spec = std::fs::read_to_string(case.with_extension("txt")).unwrap();
    let edges = s.edges();
    let es: Vec<Shape> = spec.lines().filter_map(|l| l.strip_prefix("edge ")).filter_map(|n| n.trim().parse::<usize>().ok()).map(|n| edges[n].clone()).collect();
    let head: Vec<&str> = spec.lines().find(|l| l.starts_with("fillet ")).unwrap().split_whitespace().collect();
    let radii: Vec<f64> = head[4..].iter().map(|r| r.parse().unwrap()).collect();
    let runs: usize = std::env::var("FILLET_RUNS").ok().and_then(|v| v.parse().ok()).unwrap_or(20);
    // The model's working scale (regenerations run blends at it).
    let _ = set_working_scale(std::env::var("FILLET_SCALE").ok().and_then(|v| v.parse().ok()).unwrap_or(1000.0));
    if std::env::var_os("FILLET_UNCHECKED").is_some() {
        // One plain OCCT fillet, then the input walked again (a freed sub-shape crashes here).
        println!("input before: {} faces, {} edges, valid {}", s.faces().len(), s.edges().len(), s.is_valid());
        let r = fillet_radii(&s, &es, &radii);
        println!("unchecked: {}", r.map(|r| format!("{:.6} mm3, {} faces", vol_mm3(&r.shape), r.shape.faces().len())).unwrap_or_else(|e| e.to_string()));
        println!("input after: {} faces, {} edges, valid {}", s.faces().len(), s.edges().len(), s.is_valid());
        return;
    }
    for i in 0..runs {
        let r = fillet_checked(&s, &es, &radii);
        println!("run {i}: {}", r.map(|r| format!("{:.6} mm3, {} faces, valid {}", vol_mm3(&r.shape), r.shape.faces().len(), r.shape.is_valid())).unwrap_or_else(|e| e.to_string()));
    }
}

/// FULL-ROUND-CORNER-SPHERES: R5 on every edge of a 40 × 30 × 10 block (live 1830's Fillet Everything): 10 faces,
/// each corner one quarter sphere (OCCT's two vertex blends merged), 10450.5896 mm³ in live's band.
#[test]
fn full_round_corners_are_one_sphere_each() {
    setup();
    let b = bx([0.0; 3], [40.0, 30.0, 10.0]);
    let all = edges_where(&b, |_| true);
    let r = fillet_checked(&b, &all, &vec![5.0 * MM; all.len()]).unwrap().shape;
    assert!(r.is_valid());
    let spheres: Vec<f64> = r.faces().iter().filter(|f| matches!(f.face_surface(), Ok(Surface::Sphere { .. }))).map(|f| f.area().unwrap() / (MM * MM)).collect();
    assert_eq!(r.faces().len(), 10, "{spheres:?}");
    assert!(spheres.len() == 4 && spheres.iter().all(|a| (a - 25.0 * std::f64::consts::PI).abs() < 1e-3), "{spheres:?}");
    let v = vol_mm3(&r);
    assert!((10450.111593..=10451.067592).contains(&v), "{v}");
}

/// A dumped fillet (NOSHAPE_DUMP_OPS's `fillet-<n>`; NOSHAPE_FILLET_FACES_CASE = its path without `.brep`): its
/// edges, then the result's faces (type, area, centroid) and each small face's edges.
#[test]
#[ignore = "T2 probe: NOSHAPE_FILLET_FACES_CASE"]
fn dumped_fillet_faces_probe() {
    let Ok(case) = std::env::var("NOSHAPE_FILLET_FACES_CASE") else { return };
    setup();
    let s = read_brep(std::path::Path::new(&format!("{case}.brep"))).unwrap();
    let txt = std::fs::read_to_string(format!("{case}.txt")).unwrap();
    let idx: Vec<usize> = txt.lines().filter_map(|l| l.strip_prefix("edge ")).map(|v| v.parse().unwrap()).collect();
    let r: f64 = txt.split("radii ").nth(1).and_then(|r| r.split_whitespace().next()).unwrap().parse().unwrap();
    let all = s.edges();
    let es: Vec<Shape> = idx.iter().map(|&i| all[i].clone()).collect();
    for e in &es {
        let x = e.edge_info().unwrap();
        println!("edge {:.4?} -> {:.4?}", x.start / MM, x.end / MM);
    }
    // (noshape's own blend as the app has it: on.)
    set_rust_blend(Some(true));
    let res = fillet_checked(&s, &es, &vec![r; es.len()]).unwrap();
    set_rust_blend(None);
    let generated = res.history.records.iter().filter(|x| x.relation == Relation::Generated && x.output_kind == ShapeType::Face).count();
    let un = unify_same_domain(&res.shape).unwrap();
    println!("generated faces {generated}; unified: {} faces ({})", un.shape.faces().len(), last_unify_path());
    for f in res.shape.faces() {
        if let Ok(Surface::Torus { center, axis, major_radius, minor_radius }) = f.face_surface() {
            println!("torus {:.9?} {:.9?} R {:.9} r {:.9}", center / MM, axis, major_radius / MM, minor_radius / MM);
        }
    }
    let out = res.shape;
    println!("faces {}, volume {:.6}, area {:.6}", out.faces().len(), vol_mm3(&out), out.area().unwrap() / (MM * MM));
    println!("noshape's blend: {:?}, then {:?}", last_blend_decline(), last_rust_decline());
    for (i, f) in out.faces().iter().enumerate() {
        let c = f.centroid().unwrap() / MM;
        let a = f.area().unwrap() / (MM * MM);
        println!("F{i} {} area {a:.4} at ({:.3}, {:.3}, {:.3})", format!("{:?}", f.face_surface().ok()).chars().take(14).collect::<String>(), c.x, c.y, c.z);
        if a < 1.0 || format!("{:?}", f.face_surface().ok()).contains("Cylinder") {
            for e in f.edges() {
                let x = e.edge_info().unwrap();
                println!("    edge {} {:.4?} -> {:.4?} len {:.4}", format!("{:?}", x.curve).chars().take(8).collect::<String>(), x.start / MM, x.end / MM, x.length / MM);
            }
        }
    }
}

/// FILLET-SPLIT-EDGE-FACES (dx's bracket03-mattie, live: one face a corner): a block's corner edge cut in two by a
/// groove 2.5 mm deep, both pieces R3: one cylinder over the whole edge, rounding off the 0.5 mm of material the
/// groove leaves in the corner (its strip: arc asin(1/6) × 3 mm, 6 mm long).
#[test]
fn fillet_over_a_groove_is_one_face() {
    setup();
    let block = bx([0.0; 3], [40.0, 20.0, 20.0]);
    let groove = bx([37.5, -1.0, 7.0], [41.0, 21.0, 13.0]);
    let b = cut(&block, &groove).unwrap().shape;
    let pieces = edges_where(&b, |i| (i.start.x - 40.0 * MM).abs() < 1e-9 && i.start.y.abs() < 1e-9 && (i.end.x - 40.0 * MM).abs() < 1e-9 && i.end.y.abs() < 1e-9);
    assert_eq!(pieces.len(), 2);
    let r = fillet_checked(&b, &pieces, &[3.0 * MM]).unwrap().shape;
    assert!(r.is_valid());
    let cyl: Vec<f64> = r.faces().iter().filter(|f| matches!(f.face_surface(), Ok(Surface::Cylinder { .. }))).map(|f| f.area().unwrap() / (MM * MM)).collect();
    let want = 2.0 * 3.0 * std::f64::consts::FRAC_PI_2 * 7.0 + 3.0 * (1.0f64 / 6.0).asin() * 6.0;
    assert!(cyl.len() == 1 && (cyl[0] - want).abs() < 1e-4, "{cyl:?}, want {want}");
}

/// A dumped equal-distance chamfer (NOSHAPE_DUMP_OPS's `chamfer-<n>`; NOSHAPE_CHAMFER_FACES_CASE = its path without
/// `.brep`): the result's counts, and every vertex where only two edges meet (an edge split where nothing ends).
#[test]
#[ignore = "T2 probe: NOSHAPE_CHAMFER_FACES_CASE"]
fn dumped_chamfer_faces_probe() {
    let Ok(case) = std::env::var("NOSHAPE_CHAMFER_FACES_CASE") else { return };
    setup();
    let s = read_brep(std::path::Path::new(&format!("{case}.brep"))).unwrap();
    let txt = std::fs::read_to_string(format!("{case}.txt")).unwrap();
    let idx: Vec<usize> = txt.lines().filter_map(|l| l.strip_prefix("edge ")).map(|v| v.parse().unwrap()).collect();
    let d: f64 = txt.split(" d1 ").nth(1).and_then(|r| r.split_whitespace().next()).unwrap().parse().unwrap();
    let all = s.edges();
    let es: Vec<Shape> = idx.iter().map(|&i| all[i].clone()).collect();
    let out = chamfer_checked(&s, &es, ChamferSpec::Equal(d), &[]).unwrap().shape;
    println!("faces {} edges {} vertices {}, volume {:.6}", out.faces().len(), out.edges().len(), out.count(ShapeType::Vertex), vol_mm3(&out));
    let mut at: std::collections::HashMap<[i64; 3], Vec<String>> = Default::default();
    for e in out.edges() {
        let x = e.edge_info().unwrap();
        for p in [x.start, x.end] {
            let k = [(p.x / MM * 1e4).round() as i64, (p.y / MM * 1e4).round() as i64, (p.z / MM * 1e4).round() as i64];
            at.entry(k).or_default().push(format!("{} len {:.4}", format!("{:?}", x.curve).chars().take(10).collect::<String>(), x.length / MM));
        }
    }
    for (k, v) in &at {
        if v.len() == 2 {
            println!("two edges at ({:.4}, {:.4}, {:.4}): {v:?}", k[0] as f64 / 1e4, k[1] as f64 / 1e4, k[2] as f64 / 1e4);
        }
    }
}

/// The groove's floor and ceiling edges chamfered 0.5 mm after that fillet (bracket03-mattie's Chamfer 1, live
/// 58/136/80): no edge is left in two pieces where nothing ends (a vertex of just two lines along one line).
#[test]
fn chamfered_groove_edges_stay_whole() {
    setup();
    let block = bx([0.0; 3], [40.0, 20.0, 20.0]);
    let groove = bx([37.5, -1.0, 7.0], [41.0, 21.0, 13.0]);
    let b = cut(&block, &groove).unwrap().shape;
    let pieces = edges_where(&b, |i| (i.start.x - 40.0 * MM).abs() < 1e-9 && i.start.y.abs() < 1e-9 && (i.end.x - 40.0 * MM).abs() < 1e-9 && i.end.y.abs() < 1e-9);
    let f = fillet_checked(&b, &pieces, &[3.0 * MM]).unwrap().shape;
    // (Its outer rims: not the back wall's concave edges at x 37.5.)
    let rims = edges_where(&f, |i| [7.0, 13.0].iter().any(|z| (i.start.z - z * MM).abs() < 1e-9 && (i.end.z - z * MM).abs() < 1e-9) && (i.start.x + i.end.x) / 2.0 > 37.6 * MM);
    assert!(!rims.is_empty());
    let r = chamfer_checked(&f, &rims, ChamferSpec::Equal(0.5 * MM), &[]).unwrap().shape;
    assert!(r.is_valid());
    let mut at: std::collections::HashMap<[i64; 3], Vec<DVec3>> = Default::default();
    for e in r.edges() {
        let x = e.edge_info().unwrap();
        if !matches!(x.curve, Curve::Line { .. }) {
            continue;
        }
        for p in [x.start, x.end] {
            at.entry([(p.x * 1e7).round() as i64, (p.y * 1e7).round() as i64, (p.z * 1e7).round() as i64]).or_default().push((x.end - x.start).normalize());
        }
    }
    let degree: std::collections::HashMap<[i64; 3], usize> = r.edges().iter().flat_map(|e| { let x = e.edge_info().unwrap(); [x.start, x.end] }).fold(Default::default(), |mut m, p| { *m.entry([(p.x * 1e7).round() as i64, (p.y * 1e7).round() as i64, (p.z * 1e7).round() as i64]).or_insert(0) += 1; m });
    let split: Vec<_> = at.iter().filter(|(k, d)| degree[*k] == 2 && d.len() == 2 && d[0].cross(d[1]).length() < 1e-9).map(|(k, _)| *k).collect();
    assert!(split.is_empty(), "{split:?}");
}

/// FILLET-BLEND-SPLIT (dx's FeatureScript corpus, cf-planetary-gears): a tooth's two root edges (dedendum cylinder ×
/// extruded involute, 5.08 mm long) filleted R0.85: one face each as live (OCCT's walk at its default tolerances
/// restarted 0.5 mm in, two faces each).
#[test]
fn a_tooth_root_fillet_is_one_face() {
    setup();
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/blend");
    let s = read_brep(&dir.join("planetary-tooth-roots.brep")).unwrap();
    let all = s.edges();
    let es = vec![all[1].clone(), all[7].clone()];
    let out = fillet_checked(&s, &es, &[0.000_850_543_903_162_302_9; 2]).unwrap().shape;
    assert!(out.is_valid());
    assert_eq!(out.faces().len(), s.faces().len() + 2);
    assert!((vol_mm3(&out) - 82.94049).abs() < 1e-4, "{}", vol_mm3(&out));
}

/// cf-spur-gear's root fillets (PARITY-CUSTOM-RUN-FAILURES): R0.72 on both top edges of a 1.2 mm rib, so the rounds
/// overlap and the rib's top face goes (OCCT fails): each round made alone, the part their common, as Parasolid's.
#[test]
fn overlapping_rounds_of_a_narrow_rib_are_their_common() {
    setup();
    let part = boolean(BooleanOp::Fuse, &[bx([0.0; 3], [10.0, 10.0, 2.0])], &[bx([4.4, 0.0, 2.0], [5.6, 10.0, 5.0])], &BooleanOptions::default()).unwrap().shape;
    let es = edges_where(&part, |i| i.start.z > 4.99 * MM && i.end.z > 4.99 * MM && (i.start.y - i.end.y).abs() > 1.0 * MM);
    assert_eq!(es.len(), 2);
    let r = fillet_checked(&part, &es, &[0.72 * MM; 2]).unwrap().shape;
    assert!(r.is_valid());
    assert_eq!(r.faces().len(), part.faces().len() + 1);
    let one = |e: &Shape| fillet_checked(&part, std::slice::from_ref(e), &[0.72 * MM]).unwrap().shape;
    let want = boolean(BooleanOp::Common, &[one(&es[0])], &[one(&es[1])], &BooleanOptions::default()).unwrap().shape;
    assert!((vol_mm3(&r) - vol_mm3(&want)).abs() < 1e-6, "{} {}", vol_mm3(&r), vol_mm3(&want));
    assert!(vol_mm3(&r) < vol_mm3(&one(&es[0])));
}

/// FILLET-MEETING-FILLETS-ONE-FACE (pulley02-maxwell): a revolved groove 6.35 mm high, both bottom corners R3.175: the
/// two rounds meet on one torus (the floor goes). OCCT's unify can't tell two tori are one (it intersects them): the
/// unify gives them one surface first, then merges them: one face, as live's (5 faces).
#[test]
fn a_grooves_two_rounds_on_one_torus_unify_to_one_face() {
    setup();
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/blend");
    let s = read_brep(&dir.join("pulley-groove.brep")).unwrap();
    let all = s.edges();
    let r = fillet_checked(&s, &[all[4].clone(), all[7].clone()], &[3.175 * MM; 2]).unwrap().shape;
    assert_eq!(r.faces().len(), 6);
    let u = unify_same_domain(&r).unwrap().shape;
    assert!(u.is_valid());
    assert_eq!(u.count(ShapeType::Face), 5);
    assert_eq!(u.faces().iter().filter(|f| matches!(f.face_surface(), Ok(Surface::Torus { .. }))).count(), 1);
    assert!((vol_mm3(&u) - vol_mm3(&r)).abs() < 1e-9 * vol_mm3(&r), "{} {}", vol_mm3(&u), vol_mm3(&r));
}

/// FILLET-KNOB-JASON-VOLUME (knob-jason's Fillet 1, live's exact faces in behaviours.md "core (2026-11-03 20:58, P1)
/// knob-jason after Fillet 1"): R2.5 round the knob's top and bottom outlines, arcs meeting at the lobes' sharp
/// corners. noshape's blend takes the corners where two arcs' rounds meet (their tori's meeting curve, exact points
/// fitted): every torus live's to 1e-5 mm² (OCCT's corners left them 0.02-0.05 mm² short, by its edges' direction).
#[test]
fn rounds_of_arcs_meeting_at_corners_are_lives() {
    setup();
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/blend");
    let s = read_brep(&dir.join("knob-outline.brep")).unwrap();
    let all = s.edges();
    let es: Vec<Shape> = [0, 2, 4, 5, 7, 9, 10, 11, 13, 15, 16, 17, 19, 21, 22, 23, 26, 27, 28, 29, 32, 33, 34, 36].iter().map(|&i| all[i].clone()).collect();
    set_rust_blend(Some(true));
    let r = fillet_checked(&s, &es, &[2.5 * MM; 24]);
    set_rust_blend(None);
    let r = r.unwrap().shape;
    assert_eq!(last_blend_decline(), "");
    assert!(r.is_valid());
    let tori: Vec<f64> = r.faces().iter().filter(|f| matches!(f.face_surface(), Ok(Surface::Torus { .. }))).map(|f| f.area().unwrap() / (MM * MM)).collect();
    assert_eq!(tori.len(), 24);
    for a in tori {
        assert!((a - 47.162566997).abs() < 1e-5 || (a - 78.315557040).abs() < 1e-5, "{a}");
    }
}

/// stef-radijator's Fillet 1 (live "core (2026-11-04 00:09)" (b): 81 faces, cylinders and planes only, 11179.174219
/// mm³, 5211.140462 mm²): R1 on the 36 fin ends' vertical edges, each between the outer cylinder and a gap's,
/// ending on the top and the hub's face. noshape's blend takes it (each round the section's fillet swept between
/// the caps), live's to the last digits; OCCT's took 170 ms (live 31).
#[test]
fn fin_end_rounds_between_two_cylinders_are_lives() {
    setup();
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/blend");
    let s = read_brep(&dir.join("radiator-fins.brep")).unwrap();
    let es: Vec<Shape> = s
        .edges()
        .into_iter()
        .filter(|e| {
            e.edge_info().is_ok_and(|i| {
                let on_rim = |p: glam::DVec3| ((p.x * p.x + p.y * p.y).sqrt() / MM - 22.5).abs() < 1e-3;
                matches!(i.curve, Curve::Line { .. }) && ((i.end - i.start).z.abs() / MM - 5.0).abs() < 1e-6 && on_rim(i.start) && on_rim(i.end)
            })
        })
        .collect();
    assert_eq!(es.len(), 36);
    set_rust_blend(Some(true));
    let t = std::time::Instant::now();
    let r = fillet_checked(&s, &es, &[MM; 36]);
    let ms = t.elapsed().as_secs_f64() * 1e3;
    set_rust_blend(None);
    let r = r.unwrap().shape;
    assert_eq!(last_blend_decline(), "");
    assert!(r.is_valid());
    let u = unify_same_domain(&r).unwrap().shape;
    assert_eq!(u.faces().len(), 81);
    assert!(u.faces().iter().all(|f| matches!(f.face_surface(), Ok(Surface::Cylinder { .. } | Surface::Plane(_)))));
    assert!((vol_mm3(&u) - 11179.174219).abs() < 1e-4, "{}", vol_mm3(&u));
    assert!((u.area().unwrap() / (MM * MM) - 5211.140462).abs() < 1e-4);
    eprintln!("fin end rounds: {ms:.1} ms");
}

/// knob-jason's Fillet 2 (live behaviours "knob-jason after Fillet 2": 81 / 152 / 72, 26947.696685 mm³, each lobe
/// edge's R2.5 round between two sphere corners where it meets Fillet 1's tori): on Fillet 1's result, the 12
/// vertical lobe edges and (tangent propagation) the 24 curves where Fillet 1's tori meet at them. noshape's blend
/// takes it: each corner a sphere about the point on both tori's spines, the tori cut back to their meridians there.
/// OCCT's took 236 ms with its own corner patches (105 faces); live 22.
#[test]
fn lobe_edge_rounds_meeting_earlier_rounds_end_in_spheres_as_live() {
    setup();
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/blend");
    let s = read_brep(&dir.join("knob-lobe-edges.brep")).unwrap();
    let all = s.edges();
    let idx = [15, 17, 19, 21, 23, 25, 27, 29, 31, 33, 35, 37, 41, 43, 44, 47, 48, 51, 52, 55, 56, 59, 60, 63, 68, 69, 71, 73, 75, 77, 79, 81, 83, 85, 87, 89];
    let es: Vec<Shape> = idx.iter().map(|&i| all[i].clone()).collect();
    set_rust_blend(Some(true));
    let t = std::time::Instant::now();
    let r = fillet_checked(&s, &es, &[2.5 * MM; 36]);
    let ms = t.elapsed().as_secs_f64() * 1e3;
    set_rust_blend(None);
    let r = r.unwrap().shape;
    assert_eq!(last_blend_decline(), "");
    assert!(r.is_valid());
    let u = unify_same_domain(&r).unwrap().shape;
    let shown = u.shown_topology().unwrap();
    let counts = (u.faces().len(), shown.edges.iter().filter(|&&x| x).count(), shown.vertices.iter().filter(|&&x| x).count());
    assert_eq!(counts, (81, 152, 72));
    assert_eq!(u.faces().iter().filter(|f| matches!(f.face_surface(), Ok(Surface::Sphere { .. }))).count(), 24);
    assert!((vol_mm3(&u) / 26947.696685 - 1.0).abs() < 1e-7, "{}", vol_mm3(&u));
    assert!((u.area().unwrap() / (MM * MM) - 7596.222510).abs() < 1e-4);
    eprintln!("lobe edge rounds: {ms:.1} ms");
}
