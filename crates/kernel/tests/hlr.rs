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
