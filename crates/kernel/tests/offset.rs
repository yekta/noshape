//! offset_solid: a solid's faces moved along their outward normals by their own distances, the others trimmed or run
//! on to meet them (free-form faces as their exact offsets).

use glam::{DVec2, DVec3};
use noshape_kernel::*;

const MM: f64 = 1e-3;

fn mm(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z) * MM
}

fn vol_mm3(s: &Shape) -> f64 {
    volume_properties(s).unwrap().mass / (MM * MM * MM)
}

/// `face`'s image in `r`.
fn image(r: &OpResult, input: &Shape, face: &Shape) -> Shape {
    let i = input.faces().iter().position(|f| f.is_same(face)).unwrap();
    let o = r.history.modified(0, ShapeType::Face, i as u32);
    assert_eq!(o.len(), 1);
    r.shape.faces()[o[0] as usize].clone()
}

fn face_at(s: &Shape, f: impl Fn(DVec3) -> bool) -> Shape {
    let v: Vec<Shape> = s.faces().into_iter().filter(|x| f(x.centroid().unwrap() / MM)).collect();
    assert_eq!(v.len(), 1);
    v[0].clone()
}

/// A 40 x 20 x 10 mm box, its top moved 2 mm up and its x = 40 side 1 mm in: a 39 x 20 x 12 box.
#[test]
fn a_boxs_faces_moved_by_their_own_distances() {
    let b = make_box_corners(mm(0.0, 0.0, 0.0), mm(40.0, 20.0, 10.0)).unwrap();
    let top = face_at(&b, |c| c.z > 9.9);
    let side = face_at(&b, |c| c.x > 39.9);
    let r = offset_solid(&b, &[top.clone(), side], &[2.0 * MM, -MM], 0).unwrap();
    assert!(r.shape.is_valid());
    assert_eq!(r.shape.faces().len(), 6);
    assert!((vol_mm3(&r.shape) - 39.0 * 20.0 * 12.0).abs() < 1e-6, "{} mm3", vol_mm3(&r.shape));
    assert!((image(&r, &b, &top).centroid().unwrap().z / MM - 12.0).abs() < 1e-9);
}

/// A Ø10 x 10 mm boss on a 40 x 40 x 10 mm block: its side moved 1 mm out (a seam's ends found along the seam) and its
/// top 2 mm up.
#[test]
fn a_cylinder_boss_moved_out_and_up() {
    let block = make_box_corners(mm(-20.0, -20.0, 0.0), mm(20.0, 20.0, 10.0)).unwrap();
    let boss = make_cylinder(&Plane::from_normal(mm(0.0, 0.0, 10.0), DVec3::Z), 5.0 * MM, 10.0 * MM, std::f64::consts::TAU).unwrap();
    let body = fuse(&block, &boss).unwrap().shape;
    let side = face_at(&body, |c| c.z > 10.1 && c.z < 19.9 && c.x.abs() < 1.0);
    let top = face_at(&body, |c| c.z > 19.9);
    let r = offset_solid(&body, &[side, top], &[1.0 * MM, 2.0 * MM], 0).unwrap_or_else(|e| panic!("{e}: {}", last_offset_report()));
    assert!(r.shape.is_valid());
    assert_eq!(r.shape.faces().len(), body.faces().len());
    let want = 40.0 * 40.0 * 10.0 + std::f64::consts::PI * 36.0 * 12.0;
    assert!((vol_mm3(&r.shape) - want).abs() < 1e-6 * want, "{} mm3, want {want}", vol_mm3(&r.shape));
}

/// A block whose back is a fit spline (its sides square planes past the spline's ends at an angle), the spline's face
/// moved 0.2 mm out: the new face lies 0.2 mm from the old one all over (an exact offset), its neighbours run on to it.
#[test]
fn a_spline_face_moved_lies_its_distance_from_the_old_one() {
    let spline = make_interpolated_spline(&[mm(0.0, 10.0, 0.0), mm(10.0, 13.0, 0.0), mm(20.0, 8.0, 0.0), mm(30.0, 11.0, 0.0)], &[], false).unwrap();
    let wire = make_wire(&[
        spline,
        make_line(mm(30.0, 11.0, 0.0), mm(30.0, 0.0, 0.0)).unwrap(),
        make_line(mm(30.0, 0.0, 0.0), mm(0.0, 0.0, 0.0)).unwrap(),
        make_line(mm(0.0, 0.0, 0.0), mm(0.0, 10.0, 0.0)).unwrap(),
    ])
    .unwrap();
    let body = extrude(&make_face_from_wire(&wire, true).unwrap(), mm(0.0, 0.0, 10.0)).unwrap().shape;
    let back: Vec<Shape> = body.faces().into_iter().filter(|f| !matches!(f.face_surface(), Ok(Surface::Plane(_)))).collect();
    assert_eq!(back.len(), 1);
    let r = offset_solid(&body, &back, &[0.2 * MM], 0).unwrap_or_else(|e| panic!("{e}: {}", last_offset_report()));
    assert!(r.shape.is_valid(), "{}", last_offset_report());
    assert_eq!(r.shape.faces().len(), 6);
    let moved = image(&r, &body, &back[0]);
    let [u0, u1, v0, v1] = moved.uv_bounds().unwrap();
    let [ou0, ou1, _, _] = back[0].uv_bounds().unwrap();
    for a in 0..=6 {
        for b in 0..=6 {
            let p = moved.point_at_uv(u0 + (u1 - u0) * a as f64 / 6.0, v0 + (v1 - v0) * b as f64 / 6.0).unwrap();
            let d = back[0].project_point(p).unwrap();
            // (Inside the old face: past its ends the foot is on its edge.)
            if d.u > ou0 + 1e-6 * (ou1 - ou0) && d.u < ou1 - 1e-6 * (ou1 - ou0) {
                assert!((d.distance / MM - 0.2).abs() < 1e-6, "{} mm at {:?}", d.distance / MM, DVec2::new(d.u, d.v));
            }
        }
    }
    let grown = vol_mm3(&r.shape) - vol_mm3(&body);
    let want = back[0].area().unwrap() / (MM * MM) * 0.2;
    assert!((grown - want).abs() < 0.05 * want, "{grown} mm3, about {want}");
}

/// A 40 x 20 x 10 mm box with a 0.1 mm triangle cut off its corner, the three faces there moved 1 mm in: the
/// triangle's offset vanishes (its corners pass each other), the three meet in a sharp corner again (a 39 x 19 x 9 box).
#[test]
fn a_corner_triangle_whose_offset_vanishes_goes() {
    let b = make_box_corners(mm(0.0, 0.0, 0.0), mm(40.0, 20.0, 10.0)).unwrap();
    let n = DVec3::ONE.normalize();
    let x = DVec3::new(1.0, -1.0, 0.0).normalize();
    let y = n.cross(x);
    let o = mm(40.0, 20.0, 10.0) - DVec3::ONE * 0.05 * MM - (x + y) * 2.5 * MM;
    let corner = make_box(&Plane::new(o, n, x), 5.0 * MM, 5.0 * MM, 5.0 * MM).unwrap();
    let body = cut(&b, &corner).unwrap().shape;
    assert_eq!(body.faces().len(), 7);
    let moved: Vec<Shape> = ["x", "y", "z"]
        .iter()
        .map(|a| match *a {
            "x" => face_at(&body, |c| c.x > 39.9 && c.y < 19.0),
            "y" => face_at(&body, |c| c.y > 19.9 && c.x < 39.0),
            _ => face_at(&body, |c| c.z > 9.9 && c.x < 39.0),
        })
        .collect();
    let r = offset_solid(&body, &moved, &[-MM], 0).unwrap_or_else(|e| panic!("{e}: {}", last_offset_report()));
    assert!(r.shape.is_valid());
    assert_eq!(r.shape.faces().len(), 6);
    assert!((vol_mm3(&r.shape) - 39.0 * 19.0 * 9.0).abs() < 1e-6, "{} mm3", vol_mm3(&r.shape));
    assert!(last_offset_report().contains("vanishes"));
}

/// A face whose move would turn an edge round (a 1 mm wide step's side moved 2 mm past its other side) fails, naming
/// the edge or vertex, and the input is left as it was.
#[test]
fn an_offset_that_changes_the_topology_fails() {
    let b = make_box_corners(mm(0.0, 0.0, 0.0), mm(1.0, 20.0, 10.0)).unwrap();
    let side = face_at(&b, |c| c.x > 0.9);
    let e = offset_solid(&b, &[side], &[-2.0 * MM], 0).unwrap_err();
    assert!(e.to_string().contains("offset:"), "{e}");
    assert!((vol_mm3(&b) - 200.0).abs() < 1e-9);
}

/// A dumped offset (NOSHAPE_OFFSET_DEBUG=<dir> writes offset-<n>.brep and .txt: each moved face's index and distance)
/// replayed: NOSHAPE_OFFSET_CASE = the dump's path without its extension.
#[test]
#[ignore = "T2 probe: NOSHAPE_OFFSET_CASE"]
fn dumped_offset_probe() {
    let Ok(case) = std::env::var("NOSHAPE_OFFSET_CASE") else { return };
    let s = read_brep(std::path::Path::new(&format!("{case}.brep"))).unwrap();
    let all = s.faces();
    let (mut faces, mut dist) = (Vec::new(), Vec::new());
    for l in std::fs::read_to_string(format!("{case}.txt")).unwrap().lines() {
        let mut w = l.split_whitespace();
        faces.push(all[w.next().unwrap().parse::<usize>().unwrap()].clone());
        dist.push(w.next().unwrap().parse::<f64>().unwrap());
    }
    for (i, f) in all.iter().enumerate() {
        let c = f.centroid().unwrap() / MM;
        let d = faces.iter().position(|x| x.is_same(f)).map(|k| dist[k] / MM).unwrap_or(0.0);
        println!("face {} {} area {:.4} at ({:.3}, {:.3}, {:.3}) moved {d}", i + 1, format!("{:?}", f.face_surface().ok()).chars().take(40).collect::<String>(), f.area().unwrap() / (MM * MM), c.x, c.y, c.z);
    }
    let edges = s.edges();
    for f in std::env::var("NOSHAPE_OFFSET_FACES").unwrap_or_default().split(',').filter_map(|x| x.parse::<usize>().ok()) {
        for e in all[f - 1].edges() {
            let k = edges.iter().position(|x| x.is_same(&e)).unwrap() + 1;
            let other: Vec<usize> = (0..all.len()).filter(|&g| g != f - 1 && all[g].edges().iter().any(|x| x.is_same(&e))).map(|g| g + 1).collect();
            let i = e.edge_info().unwrap();
            println!("face {f} edge {k} to {other:?} {} len {:.5} {:.4?} -> {:.4?}", format!("{:?}", i.curve).chars().take(12).collect::<String>(), i.length / MM, i.start / MM, i.end / MM);
        }
    }
    let t = std::time::Instant::now();
    let r = offset_solid(&s, &faces, &dist, 0);
    println!("{:?} in {:?}\n{}", r.as_ref().map(|r| vol_mm3(&r.shape)), t.elapsed(), last_offset_report());
}
