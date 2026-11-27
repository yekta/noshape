//! Helical thread teeth (FeatureScript "Y Threads": a tooth profile swept along
//! a helix, clipped to the threaded region and added to the turned-down core).

use glam::DVec3;
use noshape_kernel::*;

const MM: f64 = 1e-3;

/// The default Y Threads geometry on a Ø10 x 20 shaft: core r 4.7, a six-sided
/// tooth (r 4.55..5, 0.8 wide at the root, 0.2 flat crest) swept 22 turns
/// down from z 20.5, clipped to z 0..20 and unioned with the core.
fn teeth_and_core() -> (Shape, Shape, Shape) {
    let z = |p: DVec3| Plane::from_normal(p, DVec3::Z);
    let core = make_cylinder(&z(DVec3::ZERO), 4.7 * MM, 20.0 * MM, std::f64::consts::TAU).unwrap();
    // Helix: axis -z through the origin, start at (5, 0, 20.5), right-handed travel down.
    let frame = Plane::new(DVec3::new(0.0, 0.0, 20.5) * MM, -DVec3::Z, DVec3::X);
    let helix = make_helix(&frame, 5.0 * MM, 1.0 * MM, 22.0 * MM, true, 0.0).unwrap();
    // Tooth profile in the plane y = 0 (x radial, z axial).
    let pts = [(4.55, 20.1), (4.55, 20.9), (4.7, 20.9), (5.0, 20.6), (5.0, 20.4), (4.7, 20.1)];
    let p = |(x, zz): (f64, f64)| DVec3::new(x, 0.0, zz) * MM;
    let edges: Vec<Shape> = (0..pts.len()).map(|i| make_line(p(pts[i]), p(pts[(i + 1) % pts.len()])).unwrap()).collect();
    let face = make_face_from_wire(&make_wire(&edges).unwrap(), true).unwrap();
    // Default frame: OCCT's corrected Frenet fails on a helix, `sweep` falls back to the Frenet frame.
    let teeth = sweep(&face, &helix, &SweepOptions { solid: true, ..Default::default() }).unwrap().shape;
    assert!(teeth.is_valid());
    let clip = make_cylinder(&z(DVec3::ZERO), 6.3 * MM, 20.0 * MM, std::f64::consts::TAU).unwrap();
    (core, teeth, clip)
}

/// The tooth stays in the axial plane (screw motion), so by Pappus each turn of
/// the clipped teeth outside the core adds 2π ∫ r dA over the profile part beyond
/// r 4.7: the trapezoid 0.8 → 0.2 wide from r 4.7 to 5 (∫ r dA = 0.723 mm³).
#[test]
fn helical_teeth_add_to_the_core() {
    let (core, teeth, clip) = teeth_and_core();
    let clipped = common(&teeth, &clip).unwrap().shape;
    assert!(clipped.is_valid());
    let all = fuse(&core, &clipped).unwrap().shape;
    assert!(all.is_valid());
    assert_eq!(all.count(ShapeType::Solid), 1);
    let v = volume_properties(&all).unwrap().mass / MM.powi(3);
    let expected = std::f64::consts::PI * 4.7f64.powi(2) * 20.0 + 20.0 * std::f64::consts::TAU * 0.723;
    assert!((v - expected).abs() < 0.01, "{v} vs {expected}");
}

/// Onshape's helix is Parasolid's curve (live 2440, a Ø6 helix of 9.5 turns over 15.5 mm, std helix() read with
/// /featurescript): a rational quadratic arc every 9°. Its start tangent's slope and its length are live's.
#[test]
fn a_helix_is_onshapes_curve() {
    let frame = Plane::from_normal(DVec3::ZERO, DVec3::Z);
    let helix = make_helix(&frame, 0.003, 0.0155 / 9.5, 0.0155, false, 0.0).unwrap();
    let edge = helix.edges().into_iter().next().unwrap();
    let start = edge.tangent_at(0.0).unwrap();
    assert!((start.z - 0.08605943162649814).abs() < 1e-12, "start tangent's slope {}", start.z);
    let end = edge.tangent_at(1.0).unwrap();
    assert!((end.z - 0.08605943162649814).abs() < 1e-12, "end tangent's slope {}", end.z);
    let length = edge.length().unwrap();
    assert!((length - 0.179740319485531).abs() < 1e-6 * 0.179740319485531, "length {} mm", length * 1e3);
    // 380 arcs: 761 control points, as live's.
    let Ok(Curve::BSpline { degree, nb_poles, rational, .. }) = edge.edge_curve() else { panic!("{:?}", edge.edge_curve()) };
    assert_eq!((degree, nb_poles, rational), (2, 761, true));
}

/// Onshape's helix is fitted whatever its turns: three turns put a third and two thirds of its knots on whole turns,
/// where the tangents are one (c2-loft-guide-curves-driven-pattern's 3-turn Helix 1 wasn't known as a helix).
#[test]
fn a_helix_of_whole_turns_is_fitted() {
    let frame = Plane::from_normal(DVec3::ZERO, DVec3::Z);
    for turns in [2.0, 3.0, 6.0] {
        let helix = make_helix(&frame, 0.00127, 0.000508, 0.000508 * turns, true, 0.0).unwrap();
        let edge = helix.edges().into_iter().next().unwrap();
        let fit = edge.helix_fit().unwrap().unwrap_or_else(|| panic!("{turns} turns: no helix"));
        assert!((fit.radius - 0.00127).abs() < 1e-12 && (fit.pitch - 0.000508).abs() < 1e-12, "{turns} turns: {fit:?}");
        assert!(fit.axis.cross(DVec3::Z).length() < 1e-9 && fit.origin.length() < 1e-12 && fit.left_handed, "{turns} turns: {fit:?}");
    }
}

/// A helical gear's tooth (scissor-lift's herringbone Spur Gear): a profile in the plane the helix starts on, swept up
/// it. Its start cap lies on that plane and its wire runs the cap's way round, whichever way the axis points and the
/// profile runs and faces: the cap re-seated on a left-handed plane was BRepCheck's BadOrientationOfSubshape, and the
/// gear's cut by its teeth failed.
#[test]
fn a_helical_tooths_start_cap_is_valid_either_way_its_profile_faces() {
    let ccw = [(9.0, -0.6), (10.5, -0.4), (10.5, 0.4), (9.0, 0.6)];
    for (axis, x) in [(DVec3::Z, DVec3::X), (-DVec3::Z, DVec3::X), (DVec3::Y, DVec3::Z), (-DVec3::Y, DVec3::Z)] {
        let frame = Plane::new(DVec3::ZERO, axis, x);
        let helix = make_helix(&frame, 10.0 * MM, 120.0 * MM, 5.0 * MM, false, 0.0).unwrap();
        let p = |(u, v): (f64, f64)| (x * u + axis.cross(x) * v) * MM;
        let polygon = |pts: &[(f64, f64)]| -> Vec<Shape> { (0..pts.len()).map(|i| make_line(p(pts[i]), p(pts[(i + 1) % pts.len()])).unwrap()).collect() };
        // And the gear's: a gap between arcs round the axis (r 9 and 10.5, 0.1 rad either side).
        let at = |r: f64, t: f64| p((r * t.cos(), r * t.sin()));
        let arcs = vec![
            make_arc(DVec3::ZERO, axis, x, 10.5 * MM, -0.1, 0.1).unwrap(),
            make_line(at(10.5, 0.1), at(9.0, 0.1)).unwrap(),
            make_arc(DVec3::ZERO, axis, x, 9.0 * MM, -0.1, 0.1).unwrap().reversed().unwrap(),
            make_line(at(9.0, -0.1), at(10.5, -0.1)).unwrap(),
        ];
        // And spline flanks, as its involutes.
        let spline = |pts: &[(f64, f64)]| make_interpolated_spline(&pts.iter().map(|&q| p(q)).collect::<Vec<_>>(), &[], false).unwrap();
        let flanks = vec![
            spline(&[(9.0, -0.6), (9.75, -0.52), (10.5, -0.4)]),
            make_line(p((10.5, -0.4)), p((10.5, 0.4))).unwrap(),
            spline(&[(10.5, 0.4), (9.75, 0.52), (9.0, 0.6)]),
            make_line(p((9.0, 0.6)), p((9.0, -0.6))).unwrap(),
        ];
        let rev: Vec<(f64, f64)> = ccw.iter().rev().copied().collect();
        for edges in [polygon(&ccw), polygon(&rev), arcs, flanks] {
            let face = make_face_from_wire(&make_wire(&edges).unwrap(), true).unwrap();
            let area = surface_properties(&face).unwrap().mass / MM.powi(2);
            for profile in [face.reversed().unwrap(), face] {
                let tooth = sweep(&profile, &helix, &SweepOptions { solid: true, ..Default::default() }).unwrap().shape;
                let on_start = tooth.faces().iter().filter(|f| matches!(f.face_surface(), Ok(Surface::Plane(q)) if q.unit_normal().dot(axis).abs() > 1.0 - 1e-12 && q.origin.dot(axis).abs() < 1e-12)).count();
                assert_eq!(on_start, 1, "the start cap on the profile's plane ({axis})");
                assert!(tooth.faces().iter().all(|f| f.is_valid()), "every face valid ({axis})");
                assert!(tooth.is_valid());
                let v = volume_properties(&tooth).unwrap().mass / MM.powi(3);
                assert!((v - area * 5.0).abs() < 0.05, "{v}");
            }
        }
    }
}

/// A helical gear's tooth gap (scissor-lift's Spur Gear): its profile's arcs round the helix's axis sweep pieces of
/// that axis's cylinders, exact ones, and the outer one is the blank's own: on OCCT's B-spline fit, 0.3 um off it, the
/// gear's cut by its teeth was non-manifold.
#[test]
fn a_helical_tooth_gaps_arcs_sweep_exact_cylinders_and_cut_its_blank_cleanly() {
    let frame = Plane::from_normal(DVec3::ZERO, DVec3::Z);
    let helix = make_helix(&frame, 10.0 * MM, 120.0 * MM, 5.0 * MM, false, 0.0).unwrap();
    let a = 0.1;
    let at = |r: f64, t: f64| DVec3::new(r * t.cos(), r * t.sin(), 0.0) * MM;
    let edges = [
        make_arc(DVec3::ZERO, DVec3::Z, DVec3::X, 10.0 * MM, -a, a).unwrap(),
        make_line(at(10.0, a), at(8.0, a)).unwrap(),
        make_arc(DVec3::ZERO, DVec3::Z, DVec3::X, 8.0 * MM, -a, a).unwrap().reversed().unwrap(),
        make_line(at(8.0, -a), at(10.0, -a)).unwrap(),
    ];
    let face = make_face_from_wire(&make_wire(&edges).unwrap(), true).unwrap();
    let gap = sweep(&face, &helix, &SweepOptions { solid: true, ..Default::default() }).unwrap().shape;
    let radii: Vec<f64> = gap.faces().iter().filter_map(|f| match f.face_surface() {
        Ok(Surface::Cylinder { radius, .. }) => Some(radius / MM),
        _ => None,
    }).collect();
    assert_eq!(radii.len(), 2, "{radii:?}");
    assert!(radii.iter().any(|r| (r - 10.0).abs() < 1e-9) && radii.iter().any(|r| (r - 8.0).abs() < 1e-9), "{radii:?}");
    assert!(gap.is_valid());
    let area = a * (10.0f64.powi(2) - 8.0f64.powi(2));
    let v = volume_properties(&gap).unwrap().mass / MM.powi(3);
    assert!((v - area * 5.0).abs() < 0.01, "{v}");
    let blank = make_cylinder(&frame, 10.0 * MM, 5.0 * MM, std::f64::consts::TAU).unwrap();
    let gear = cut(&blank, &gap).unwrap().shape;
    assert!(gear.is_valid());
    assert!(gear.edge_faces().unwrap().iter().all(|f| f.len() == 2), "every edge between two faces");
    let v = volume_properties(&gear).unwrap().mass / MM.powi(3);
    assert!((v - (std::f64::consts::PI * 100.0 * 5.0 - area * 5.0)).abs() < 0.01, "{v}");
}

/// A nut's thread groove (c2-config-parts-nuts' Sweep 1): its flat crest, a profile line along the axis on the bore,
/// sweeps one exact cylinder face round 6 turns. OCCT's cut by it removed 3x the groove; the Booleans now take that face
/// in pieces under a turn, and the cut removes the groove, by Pappus 6 turns of 2π ∫ r dA over the trapezoid (r 1.6 to 2,
/// 0.6 wide at the bore, 0.2 at r 2), its root one face again (live's).
#[test]
fn a_nuts_thread_groove_round_six_turns_on_its_bore_cuts_the_groove_out() {
    let z = |p: DVec3| Plane::from_normal(p, DVec3::Z);
    let tau = std::f64::consts::TAU;
    let tube = cut(&make_cylinder(&z(DVec3::ZERO), 4.0 * MM, 5.4 * MM, tau).unwrap(), &make_cylinder(&z(DVec3::ZERO), 1.6 * MM, 5.4 * MM, tau).unwrap()).unwrap().shape;
    let frame = Plane::new(DVec3::new(0.0, 0.0, 0.5) * MM, DVec3::Z, DVec3::X);
    let helix = make_helix(&frame, 1.6 * MM, 0.7 * MM, 4.2 * MM, true, 0.0).unwrap();
    let pts = [(1.6, 0.8), (1.6, 0.2), (2.0, 0.4), (2.0, 0.6)];
    let p = |(x, zz): (f64, f64)| DVec3::new(x, 0.0, zz) * MM;
    let edges: Vec<Shape> = (0..pts.len()).map(|i| make_line(p(pts[i]), p(pts[(i + 1) % pts.len()])).unwrap()).collect();
    let face = make_face_from_wire(&make_wire(&edges).unwrap(), true).unwrap();
    let groove = sweep(&face, &helix, &SweepOptions { solid: true, ..Default::default() }).unwrap().shape;
    assert!(groove.is_valid());
    let crest = groove.faces().iter().filter(|f| matches!(f.face_surface(), Ok(Surface::Cylinder { radius, .. }) if (radius / MM - 1.6).abs() < 1e-9)).count();
    assert_eq!(crest, 1, "the crest one exact cylinder face");
    let per_turn = tau * (1.1 * (2.0f64.powi(2) - 1.6f64.powi(2)) - (2.0f64.powi(3) - 1.6f64.powi(3)) / 3.0);
    let vol = |s: &Shape| volume_properties(s).unwrap().mass / MM.powi(3);
    assert!((vol(&groove) - 6.0 * per_turn).abs() < 1e-3, "{} vs {}", vol(&groove), 6.0 * per_turn);
    let nut = cut(&tube, &groove).unwrap().shape;
    assert!(nut.is_valid());
    assert_eq!(nut.count(ShapeType::Solid), 1);
    assert!(nut.edge_faces().unwrap().iter().all(|f| f.len() <= 2), "no edge of more than two faces");
    assert!((vol(&tube) - vol(&nut) - 6.0 * per_turn).abs() < 1e-3, "removes {} of {}", vol(&tube) - vol(&nut), 6.0 * per_turn);
    let root = nut.faces().iter().filter(|f| matches!(f.face_surface(), Ok(Surface::Cylinder { radius, .. }) if (radius / MM - 2.0).abs() < 1e-9)).count();
    assert_eq!(root, 1, "the root one face");
}
