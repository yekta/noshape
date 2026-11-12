//! Hidden line removal: silhouettes of curved faces, hidden lines of holes
//! and merging of fragments into polylines (exact and polygonal modes).

use noshape_kernel::*;
use std::f64::consts::PI;

/// 100 x 60 x 10 plate (centered on the origin in XY, z = 0..10) with a
/// through hole of radius `r` along Z at each of `holes`.
fn plate_with_holes(r: f64, holes: &[(f64, f64)]) -> Shape {
    let mut plate = make_box(&Plane::new(DVec3::new(-50.0, -30.0, 0.0), DVec3::Z, DVec3::X), 100.0, 60.0, 10.0).unwrap();
    for &(x, y) in holes {
        let c = make_cylinder(&Plane::new(DVec3::new(x, y, -1.0), DVec3::Z, DVec3::X), r, 12.0, 2.0 * PI).unwrap();
        plate = cut(&plate, &c).unwrap().shape;
    }
    plate
}

fn dump(edges: &[ProjectedEdge]) -> String {
    edges
        .iter()
        .map(|e| {
            let a = e.polyline[0];
            let b = e.polyline[e.polyline.len() - 1];
            format!("{:?} {:?} n={} ({:.3},{:.3})-({:.3},{:.3}) src={:?}\n", e.visibility, e.class, e.polyline.len(), a.x, a.y, b.x, b.y, e.source_edge)
        })
        .collect()
}

/// Straight vertical polylines (constant x within tolerance) spanning z0..z1.
fn vertical_lines(edges: &[ProjectedEdge], vis: Visibility, z0: f64, z1: f64) -> Vec<f64> {
    let mut xs: Vec<f64> = edges
        .iter()
        .filter(|e| e.visibility == vis && e.class != EdgeClass::Seam && e.class != EdgeClass::Smooth)
        .filter_map(|e| {
            let x = e.polyline[0].x;
            if e.polyline.iter().any(|p| (p.x - x).abs() > 1e-6) {
                return None;
            }
            let lo = e.polyline.iter().map(|p| p.y).fold(f64::INFINITY, f64::min);
            let hi = e.polyline.iter().map(|p| p.y).fold(f64::NEG_INFINITY, f64::max);
            ((lo - z0).abs() < 1e-6 && (hi - z1).abs() < 1e-6).then_some(x)
        })
        .collect();
    xs.sort_by(f64::total_cmp);
    xs
}

#[test]
fn hole_side_view_shows_two_hidden_lines_per_hole() {
    let plate = plate_with_holes(6.0, &[(0.0, 0.0), (30.0, 10.0)]);
    for exact in [true, false] {
        // Front view: looking along +Y, Z up; the holes are behind the front face.
        let e = project_hlr(&plate, DVec3::Y, DVec3::Z, exact).unwrap();
        let hidden = vertical_lines(&e, Visibility::Hidden, 0.0, 10.0);
        let expect = [-6.0, 6.0, 24.0, 36.0];
        assert_eq!(hidden.len(), 4, "exact={exact}: hidden {hidden:?}\n{}", dump(&e));
        for (x, w) in hidden.iter().zip(expect) {
            assert!((x - w).abs() < 1e-3, "exact={exact}: {hidden:?}");
        }
        // Outline lines of the holes have no source edge.
        let outlines = e.iter().filter(|x| x.class == EdgeClass::Outline && x.visibility == Visibility::Hidden).count();
        assert_eq!(outlines, 4, "exact={exact}\n{}", dump(&e));
        // Visible: the 4 plate outline edges only (merged into one piece each).
        let visible = e.iter().filter(|x| x.visibility == Visibility::Visible && x.class != EdgeClass::Seam).count();
        assert_eq!(visible, 4, "exact={exact}\n{}", dump(&e));
    }
}

#[test]
fn cylinder_side_view_has_two_outlines() {
    let c = make_cylinder(&Plane::xy(), 5.0, 10.0, 2.0 * PI).unwrap();
    for exact in [true, false] {
        let e = project_hlr(&c, DVec3::Y, DVec3::Z, exact).unwrap();
        let outl: Vec<&ProjectedEdge> = e.iter().filter(|x| x.class == EdgeClass::Outline && x.visibility == Visibility::Visible).collect();
        assert_eq!(outl.len(), 2, "exact={exact}\n{}", dump(&e));
        let xs = vertical_lines(&e, Visibility::Visible, 0.0, 10.0);
        assert_eq!(xs.len(), 2, "exact={exact}: {xs:?}\n{}", dump(&e));
        assert!((xs[0] + 5.0).abs() < 1e-3 && (xs[1] - 5.0).abs() < 1e-3, "{xs:?}");
        // Top and bottom circles project to one straight line each.
        let horizontal = e
            .iter()
            .filter(|x| x.visibility == Visibility::Visible && x.class == EdgeClass::Sharp)
            .filter(|x| x.polyline.iter().all(|p| (p.y - x.polyline[0].y).abs() < 1e-6))
            .count();
        assert_eq!(horizontal, 2, "exact={exact}\n{}", dump(&e));
    }
}

/// A tube revolved 360° about Z draws its outlines only: the revolve's seam
/// (in the sketch plane) is no line in a side view, and where it lies on the
/// outline the outline stays (qa DWG-REVOLVE-SEAM).
#[test]
fn revolved_tube_side_views_have_no_seam_line() {
    let front = Plane::new(DVec3::ZERO, -DVec3::Y, DVec3::X);
    let seg = |a: (f64, f64), b: (f64, f64)| Segment2d::Line { a: DVec2::new(a.0, a.1), b: DVec2::new(b.0, b.1) };
    let rect = make_face_2d(&front, &[seg((10.0, 0.0), (20.0, 0.0)), seg((20.0, 0.0), (20.0, 30.0)), seg((20.0, 30.0), (10.0, 30.0)), seg((10.0, 30.0), (10.0, 0.0))], &[]).unwrap();
    let tube = revolve(&rect, DVec3::ZERO, DVec3::Z, 2.0 * PI).unwrap().shape;
    for exact in [true, false] {
        // Across the seam's plane: the outlines at ±20 only (no line at the axis).
        let e = project_hlr(&tube, DVec3::X, DVec3::Z, exact).unwrap();
        let xs = vertical_lines(&e, Visibility::Visible, 0.0, 30.0);
        assert_eq!(xs.len(), 2, "exact={exact}: {xs:?}\n{}", dump(&e));
        assert!((xs[0] + 20.0).abs() < 1e-3 && (xs[1] - 20.0).abs() < 1e-3, "exact={exact}: {xs:?}");
        // Along it: both outlines, the seam's side too.
        let e = project_hlr(&tube, DVec3::Y, DVec3::Z, exact).unwrap();
        let xs = vertical_lines(&e, Visibility::Visible, 0.0, 30.0);
        assert_eq!(xs.len(), 2, "exact={exact}: {xs:?}\n{}", dump(&e));
    }
}

#[test]
fn fragments_are_merged_into_polylines() {
    // A face-on circle must come out as a single closed polyline per edge,
    // and the polygonal mode must not return many tiny pieces.
    let plate = plate_with_holes(6.0, &[(0.0, 0.0)]);
    for exact in [true, false] {
        let e = project_hlr(&plate, DVec3::NEG_Z, DVec3::Y, exact).unwrap();
        let visible: Vec<&ProjectedEdge> = e.iter().filter(|x| x.visibility == Visibility::Visible && x.class != EdgeClass::Seam).collect();
        // 4 rectangle sides + the hole's top circle.
        assert_eq!(visible.len(), 5, "exact={exact}\n{}", dump(&e));
        let circle = visible.iter().find(|x| x.polyline.len() > 2).expect("circle polyline");
        let (a, b) = (circle.polyline[0], circle.polyline[circle.polyline.len() - 1]);
        assert!(a.distance(b) < 1e-6, "closed circle, exact={exact}");
        assert!(circle.source_edge.is_some());
        // The bottom circle lies exactly under the top one: not drawn hidden.
        let hidden = e.iter().filter(|x| x.visibility == Visibility::Hidden && x.class != EdgeClass::Seam).count();
        assert_eq!(hidden, 0, "exact={exact}\n{}", dump(&e));
    }
}

/// Many bodies (a big assembly) are hidden in parallel chunks of
/// neighbours: the same lines as one hidden-line pass over them all (a
/// compound nested one level deeper takes that path), by source edge,
/// visibility and class, in a front, top and isometric view of a grid of
/// blocks and cylinders of different heights, rows hiding the rows behind.
#[test]
fn many_bodies_hide_in_chunks_as_in_one_pass() {
    let mut bodies = Vec::new();
    for r in 0..12 {
        for c in 0..12 {
            let (x, y, h) = (c as f64 * 12.0, r as f64 * 12.0, 4.0 + ((r * 12 + c) % 5) as f64 * 3.0);
            let p = Plane::new(DVec3::new(x, y, 0.0), DVec3::Z, DVec3::X);
            bodies.push(if (r + c) % 3 == 0 { make_cylinder(&Plane::new(DVec3::new(x + 5.0, y + 5.0, 0.0), DVec3::Z, DVec3::X), 5.0, h, 2.0 * PI).unwrap() } else { make_box(&p, 10.0, 10.0, h).unwrap() });
        }
    }
    let flat = Shape::compound(&bodies);
    let nested = Shape::compound(&[Shape::compound(&bodies)]);
    type Key = (bool, u8, Option<u32>);
    let lengths = |edges: Vec<ProjectedEdge>| {
        let mut m: std::collections::BTreeMap<Key, f64> = Default::default();
        for e in edges {
            let class = match e.class {
                EdgeClass::Sharp => 0,
                EdgeClass::Smooth => 1,
                EdgeClass::Outline => 2,
                EdgeClass::Seam => 3,
            };
            *m.entry((e.visibility == Visibility::Visible, class, e.source_edge)).or_default() += e.polyline.windows(2).map(|w| w[0].distance(w[1])).sum::<f64>();
        }
        m.retain(|_, l| *l > 1e-6);
        m
    };
    for (name, dir, up) in [("front", DVec3::Y, DVec3::Z), ("top", -DVec3::Z, DVec3::Y), ("iso", DVec3::new(-1.0, 1.0, -1.0), DVec3::Z)] {
        let chunked = lengths(project_hlr_raw(&flat, dir, up, true, 0.05).unwrap());
        let one = lengths(project_hlr_raw(&nested, dir, up, true, 0.05).unwrap());
        assert!(one.len() > 400 && one.keys().filter(|k| !k.0).count() > 100, "{name}: {} pieces, {} hidden", one.len(), one.keys().filter(|k| !k.0).count());
        assert_eq!(chunked.keys().collect::<Vec<_>>(), one.keys().collect::<Vec<_>>(), "{name}: the same edges, visible and hidden");
        for (k, l) in &one {
            assert!((chunked[k] - l).abs() < 1e-3, "{name} {k:?}: {} vs {l}", chunked[k]);
        }
    }
}

/// An edge between two faces of one plane (two boxes fused side by side,
/// their top split at x = 0) is no edge in Onshape's topology: the top view
/// shows the outline only (live 1857: no line across a part's split faces).
#[test]
fn an_edge_between_faces_of_one_surface_is_not_drawn() {
    let a = make_box(&Plane::new(DVec3::new(-20.0, -10.0, 0.0), DVec3::Z, DVec3::X), 20.0, 20.0, 10.0).unwrap();
    let b = make_box(&Plane::new(DVec3::new(0.0, -10.0, 0.0), DVec3::Z, DVec3::X), 20.0, 20.0, 10.0).unwrap();
    let s = fuse(&a, &b).unwrap().shape;
    let tops = s.faces().iter().filter(|f| matches!(f.face_surface(), Ok(Surface::Plane(p)) if p.normal.z.abs() > 0.99 && (p.origin.z - 10.0).abs() < 1e-9)).count();
    assert_eq!(tops, 2, "the fused top stays split (else this test shows nothing)");
    for exact in [true, false] {
        let e = project_hlr(&s, -DVec3::Z, DVec3::Y, exact).unwrap();
        // (A seam's class, which drawings leave out, is allowed.)
        let across = e.iter().filter(|e| e.visibility == Visibility::Visible && e.class != EdgeClass::Seam && e.polyline.iter().all(|p| p.x.abs() < 1e-6));
        assert_eq!(across.count(), 0, "exact={exact}:\n{}", dump(&e));
    }
}

/// Lead 11-12 (drawing speed): an exact view past its work bound (cf-wave-spring's ran 35 min and on) is hidden on
/// the triangulation at the same deflection instead, so the drawing finishes; the bound is work, the same everywhere.
#[test]
fn an_exact_view_past_its_work_bound_is_hidden_on_the_triangulation() {
    let plate = plate_with_holes(8.0, &[(-25.0, 0.0), (25.0, 0.0)]);
    let (dir, up) = (DVec3::new(-1.0, 1.0, -1.0), DVec3::Z);
    let exact = project_hlr_with(&plate, dir, up, true, 0.05).unwrap();
    let poly = project_hlr_with(&plate, dir, up, false, 0.05).unwrap();
    assert_ne!(dump(&exact), dump(&poly), "within its bound the view is exact");
    let spent = with_hlr_exact_work(10, || project_hlr_with(&plate, dir, up, true, 0.05).unwrap());
    assert_eq!(dump(&spent), dump(&poly), "past it, the triangulation's");
    let again = project_hlr_with(&plate, dir, up, true, 0.05).unwrap();
    assert_eq!(dump(&again), dump(&exact), "the default bound back after");
}

/// cf-wave-spring (the exchange corpus' wave spring, 6 B-spline faces): OCCT's exact Hide ran 35 min and on; the
/// views finish (on the triangulation, past the exact pass's work bound).
#[test]
#[ignore = "T2: the exact pass runs to its work bound, ~25 s"]
fn the_wave_springs_exact_views_finish() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/hlr/wave-spring.brep");
    let spring = read_brep(&path).unwrap();
    for (dir, up) in [(DVec3::Y, DVec3::Z), (DVec3::NEG_Z, DVec3::Y)] {
        let exact = project_hlr_with(&spring, dir, up, true, 0.0).unwrap();
        let poly = project_hlr_with(&spring, dir, up, false, 0.0).unwrap();
        assert_eq!(exact.len(), poly.len(), "hidden on the triangulation");
        assert!(exact.iter().any(|e| e.visibility == Visibility::Visible) && exact.iter().any(|e| e.visibility == Visibility::Hidden));
    }
}
