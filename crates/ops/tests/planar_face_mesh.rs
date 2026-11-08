//! Lead 11-07 P0 (a flat face shaded in dark wedges): a flat face renders one flat colour from every angle, so every
//! planar face's display triangles face along the face's outward normal, have area, and carry that normal exactly.
//! Synthetic plates like the user's (a rounded box's cavity floor cut by a round recess, slots, a rectangular seat,
//! holes), meshed by noshape's mesher and by BRepMesh; the parity corpus in T2.

use glam::DVec3;
use noshape_kernel::{self as k, BooleanOp, BooleanOptions, Plane, Shape, ShapeType};
use noshape_model::display::MeshBuffers;

/// What is wrong with the planar faces of `mesh` (the display mesh of `shape`, its faces in order).
pub fn planar_face_problems(shape: &Shape, mesh: &MeshBuffers) -> Vec<String> {
    let mut out = Vec::new();
    for (fi, &(first, count)) in mesh.face_ranges.iter().enumerate() {
        let Ok(face) = shape.sub_shape(ShapeType::Face, fi) else { continue };
        // A plane surface (`face_plane` also takes a curved face whose boundary is flat: a sphere's meridian).
        if !matches!(face.face_surface(), Ok(k::Surface::Plane(_))) {
            continue;
        }
        let Ok(Some(plane)) = face.face_plane() else { continue };
        let n = plane.normal;
        let p = |i: u32| DVec3::from(mesh.positions[i as usize].map(f64::from));
        let tris = &mesh.indices[first as usize..(first + count) as usize];
        let (mut backwards, mut slivers, mut normals) = (0, 0, 0);
        let (mut worst_tri, mut worst_normal, mut biggest_backwards) = (1.0f64, 1.0f64, 0.0f64);
        for t in tris.as_chunks::<3>().0 {
            let (a, b, c) = (p(t[0]), p(t[1]), p(t[2]));
            let cross = (b - a).cross(c - a);
            let longest = (b - a).length().max((c - b).length()).max((a - c).length());
            // Twice the area against the longest side squared: under 1e-9, a needle with no direction of its own.
            if cross.length() <= 1e-9 * longest * longest {
                slivers += 1;
            } else {
                let d = cross.normalize().dot(n);
                worst_tri = worst_tri.min(d);
                // Wound against the normal (its tilt off the plane is the f32 corners' rounding on a small one: the
                // shading takes the vertex normals).
                if d <= 0.0 {
                    backwards += 1;
                    biggest_backwards = biggest_backwards.max(cross.length() / (longest * longest));
                }
            }
            for &i in t {
                let vn = DVec3::from(mesh.normals[i as usize].map(f64::from));
                worst_normal = worst_normal.min(vn.dot(n));
                if (vn - n).length() > 1e-5 {
                    normals += 1;
                }
            }
        }
        if backwards + slivers + normals > 0 {
            out.push(format!("face {fi} (normal {n:.3}): {} triangles, {backwards} not facing along the normal, {slivers} without area, {normals} vertex normals off it (least dot: triangles {worst_tri:.4}, vertex normals {worst_normal:.4}; the biggest backwards one's 2A/l² {biggest_backwards:.2e})", tris.len() / 3));
        }
    }
    out
}

/// Where `mesh` covers more or less than its faces of `shape` (each face's triangles' area against the face's own,
/// any surface) or doesn't close (the triangles' signed volume against the solid's): a triangle across a hole or
/// two faces' triangles over one another, which the section view's caps count as material (hatched over empty
/// space, red over the walls).
pub fn coverage_problems(shape: &Shape, mesh: &MeshBuffers) -> Vec<String> {
    let mut out = Vec::new();
    let p = |i: u32| DVec3::from(mesh.positions[i as usize].map(f64::from));
    let mut volume6 = 0.0;
    for (fi, &(first, count)) in mesh.face_ranges.iter().enumerate() {
        let Ok(face) = shape.sub_shape(ShapeType::Face, fi) else { continue };
        let Ok(area) = face.area() else { continue };
        let tris = &mesh.indices[first as usize..(first + count) as usize];
        let mut meshed = 0.0;
        for t in tris.as_chunks::<3>().0 {
            let (a, b, c) = (p(t[0]), p(t[1]), p(t[2]));
            meshed += 0.5 * (b - a).cross(c - a).length();
            volume6 += a.dot(b.cross(c));
        }
        // Chords move a face's area by a few per cent at the coarsest level (a flat face's arcs too); a triangle
        // across a hole or a channel adds that hole's area.
        let planar = matches!(face.face_surface(), Ok(k::Surface::Plane(_)));
        let slack = if planar { 0.03 } else { 0.08 };
        if (meshed - area).abs() > slack * area + 0.15e-6 {
            out.push(format!("face {fi} ({}): its triangles cover {:.3} mm², the face {:.3} mm²", if planar { "plane" } else { "curved" }, meshed * 1e6, area * 1e6));
        }
    }
    if let Ok(props) = k::mass_properties(shape)
        && props.mass > 0.0
        && (volume6 / 6.0 - props.mass).abs() > 0.03 * props.mass
    {
        out.push(format!("the mesh encloses {:.1} mm³, the solid {:.1} mm³", volume6 / 6.0 * 1e9, props.mass * 1e9));
    }
    out
}

/// Faces of any surface whose triangles turn their back on the face's outward normal there (area-weighted, beyond
/// the chords' tilt): a section view counts such a face's triangles with the wrong sign (red over a wall, material
/// where there is none) and the viewport culls them (the solid's inside shows).
pub fn facing_problems(shape: &Shape, mesh: &MeshBuffers) -> Vec<String> {
    let mut out = Vec::new();
    let p = |i: u32| DVec3::from(mesh.positions[i as usize].map(f64::from));
    for (fi, &(first, count)) in mesh.face_ranges.iter().enumerate() {
        let Ok(face) = shape.sub_shape(ShapeType::Face, fi) else { continue };
        let tris = &mesh.indices[first as usize..(first + count) as usize];
        let (mut total, mut backwards, mut n) = (0.0, 0.0, 0);
        for t in tris.as_chunks::<3>().0 {
            let (a, b, c) = (p(t[0]), p(t[1]), p(t[2]));
            let cross = (b - a).cross(c - a);
            let area = 0.5 * cross.length();
            let Ok(normal) = face.normal_at_point((a + b + c) / 3.0) else { continue };
            total += area;
            if area > 0.0 && cross.normalize().dot(normal) < -0.5 {
                backwards += area;
                n += 1;
            }
        }
        if n > 0 && backwards > 0.01 * total {
            out.push(format!("face {fi} ({}): {n} of {} triangles ({:.4} of {:.4} mm²) face away from its outward normal", if matches!(face.face_surface(), Ok(k::Surface::Plane(_))) { "plane" } else { "curved" }, tris.len() / 3, backwards * 1e6, total * 1e6));
        }
    }
    out
}

fn frame(origin: DVec3) -> Plane {
    Plane::new(origin, DVec3::Z, DVec3::X)
}

fn cut(a: Shape, tool: Shape) -> Shape {
    k::boolean(BooleanOp::Cut, &[a], &[tool], &BooleanOptions::default()).unwrap().shape
}

fn fuse(a: Shape, b: Shape) -> Shape {
    k::boolean(BooleanOp::Fuse, &[a], &[b], &BooleanOptions::default()).unwrap().shape
}

/// A box from `a` to `b` with its vertical edges rounded by `r`.
fn rounded_box(a: DVec3, b: DVec3, r: f64) -> Shape {
    let bx = k::make_box_corners(a, b).unwrap();
    if r <= 0.0 {
        return bx;
    }
    let vertical: Vec<Shape> = bx.sub_shapes(ShapeType::Edge).into_iter().filter(|e| e.endpoints().is_ok_and(|(p, q)| (p - q).normalize().z.abs() > 0.99)).collect();
    k::fillet(&bx, &vertical, r).unwrap_or_else(|e| panic!("fillet r {r} of {} edges: {e:?}", vertical.len())).shape
}

/// A slot of width `2 r` from `a` to `b` (centres, on z = `z0`), `h` high.
fn slot(a: DVec3, b: DVec3, r: f64, h: f64) -> Shape {
    let d = (b - a).normalize();
    let side = DVec3::new(-d.y, d.x, 0.0) * r;
    let pts = [a + side, b + side, b - side, a - side];
    let lo = pts.iter().fold(DVec3::splat(f64::MAX), |m, p| m.min(*p));
    let hi = pts.iter().fold(DVec3::splat(f64::MIN), |m, p| m.max(*p)) + DVec3::Z * h;
    // Axis-aligned slots only (a box between the centres).
    let body = k::make_box_corners(lo, hi).unwrap();
    let ends = fuse(k::make_cylinder(&frame(a), r, h, std::f64::consts::TAU).unwrap(), k::make_cylinder(&frame(b), r, h, std::f64::consts::TAU).unwrap());
    fuse(body, ends)
}

/// The user's kind of part: a rounded-rectangle box with a cable cavity whose floor carries a round puck recess, a
/// plug channel (slot), a USB seat (rectangle) and holes; `s` moves the cuts about (mm).
fn cable_box(s: f64) -> Shape {
    let mm = 1e-3;
    let v = |x: f64, y: f64, z: f64| DVec3::new(x, y, z) * mm;
    let mut b = rounded_box(v(0., 0., 0.), v(60., 75., 20.), 9. * mm);
    // The cavity: open at the top, its floor at z 4.
    b = cut(b, rounded_box(v(2., 2., 4.), v(58., 73., 25.), 7. * mm));
    // The puck recess in the floor.
    b = cut(b, k::make_cylinder(&frame(v(30. + s, 37. - s, 2.5)), 12. * mm, 5. * mm, std::f64::consts::TAU).unwrap());
    // The plug channel from the recess to the wall, and one along the top.
    b = cut(b, slot(v(30. + s, 45., 2.5), v(30. + s, 70., 2.5), 3. * mm, 5. * mm));
    b = cut(b, slot(v(12., 66. - s, 3.), v(48., 66. - s, 3.), 2. * mm, 5. * mm));
    // The USB seat at the wall: a rectangle into the wall.
    b = cut(b, k::make_box_corners(v(24. + s, 68., 3.), v(36. + s, 76., 9.)).unwrap());
    // Holes through the floor and a rounded rectangle pocket.
    for (x, y, r) in [(10., 10., 1.6), (50., 10., 1.6), (10. + s, 30., 2.5), (50. - s, 52., 1.0)] {
        b = cut(b, k::make_cylinder(&frame(v(x, y, -1.)), r * mm, 10. * mm, std::f64::consts::TAU).unwrap());
    }
    b = cut(b, rounded_box(v(40. - s, 18., 3.), v(54., 30. + s, 6.), 2. * mm));
    b
}

/// A long strip (1400 × 24 mm) with a hairline slot cut in from its side, its round end r 0.1 mm: the end's arc is
/// flatter than the strip's triangulation grid (c2-sm-louver's louver faces), where noshape's mesher left slivers
/// turned over.
fn strip_with_hairline_slot() -> Shape {
    let mm = 1e-3;
    let v = |x: f64, y: f64, z: f64| DVec3::new(x, y, z) * mm;
    let strip = k::make_box_corners(v(0., 0., 0.), v(1400., 24., 3.)).unwrap();
    cut(strip, slot(v(687.5, -1., -1.), v(687.5, 1.3, -1.), 0.1 * mm, 5. * mm))
}

fn display_mesh(shape: &Shape, quality: f64) -> MeshBuffers {
    noshape_model::regen::tessellate_shape(shape, quality).expect("a display mesh")
}

/// Every planar face of the synthetic cable boxes, meshed by noshape's mesher and by BRepMesh, faces along its
/// normal with area and the normal on every vertex.
#[test]
fn planar_faces_mesh_flat_along_their_normal() {
    let (mut problems, mut planar) = (Vec::new(), 0);
    for s in [0.0, 0.7, 1.9, 3.3, -1.0] {
        let make = || if s < 0.0 { strip_with_hairline_slot() } else { cable_box(s) };
        planar += make().sub_shapes(ShapeType::Face).iter().filter(|f| matches!(f.face_surface(), Ok(k::Surface::Plane(_)))).count();
        // View in high quality's level, Auto, and the coarser levels and previews; each mesher on a shape of its own
        // (BRepMesh keeps a triangulation already on a face).
        for rust in [true, false] {
            k::set_rust_mesh(rust);
            let shape = make();
            for quality in [0.25, 1.0, 2.0, 4.0] {
                let mesh = display_mesh(&shape, quality);
                for p in planar_face_problems(&shape, &mesh).into_iter().chain(coverage_problems(&shape, &mesh)) {
                    problems.push(format!("plate {s} at {quality} ({}): {p}", if rust { "noshape's mesher" } else { "BRepMesh" }));
                }
            }
        }
        k::set_rust_mesh(true);
    }
    assert!(planar >= 4 * 20, "{planar} planar faces");
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

/// The parity corpus: every Part Studio's parts (NOSHAPE_PARITY_SAVE's translated documents under
/// NOSHAPE_PLANAR_CORPUS, default ~/agents/psui/corpus), their planar faces and every face's coverage.
#[test]
#[ignore = "T2: the parity corpus lives outside the repo (other people's work)"]
fn corpus_planar_faces_mesh_flat_along_their_normal() {
    let dir = std::env::var("NOSHAPE_PLANAR_CORPUS").unwrap_or_else(|_| format!("{}/agents/psui/corpus", std::env::var("HOME").unwrap_or_default()));
    let mut docs: Vec<_> = std::fs::read_dir(&dir).expect("the corpus directory").flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "noshape")).collect();
    docs.sort();
    assert!(!docs.is_empty(), "no documents in {dir}");
    let mut problems = Vec::new();
    // NOSHAPE_PLANAR_BREPMESH=1: every face meshed by BRepMesh (which mesher made a face's triangles).
    if std::env::var_os("NOSHAPE_PLANAR_BREPMESH").is_some() {
        k::set_rust_mesh_everywhere(false);
    }
    for path in &docs {
        let Ok(session) = noshape_ops::session::Session::open(path) else {
            problems.push(format!("{}: doesn't open", path.display()));
            continue;
        };
        for tab in session.doc.ordered_tabs() {
            let noshape_model::document::TabContent::PartStudio(ps) = &tab.content else { continue };
            let r = noshape_model::regen::Regenerator::new().regenerate(ps, &Default::default(), &session.doc.settings.units, &Default::default());
            for b in &r.display.bodies {
                let Some(ctx) = r.context.bodies.iter().find(|c| c.id == b.id) else { continue };
                for (level, mesh) in b.coarser.iter().chain(std::iter::once(&b.mesh)).enumerate() {
                    for p in planar_face_problems(&ctx.shape, mesh).into_iter().chain(coverage_problems(&ctx.shape, mesh)).chain(facing_problems(&ctx.shape, mesh)) {
                        problems.push(format!("{} / {} / {} level {level}: {p}", path.file_stem().unwrap().to_string_lossy(), tab.name, b.name));
                    }
                }
            }
        }
    }
    eprintln!("{} documents", docs.len());
    assert!(problems.is_empty(), "{} planar faces:\n{}", problems.len(), problems.join("\n"));
}


/// The checks see the two faults the user's dock showed on a Mac (v0.4.33): a flat face's triangle turned over (the
/// solid's inside shown black, a hatched wedge in a section) and one stretched across the face over a hole or channel
/// (the section's caps hatched over empty space and red over the walls).
#[test]
fn the_checks_catch_a_triangle_turned_over_or_across_a_hole() {
    let shape = cable_box(0.0);
    let mesh = display_mesh(&shape, 1.0);
    assert!(planar_face_problems(&shape, &mesh).is_empty() && coverage_problems(&shape, &mesh).is_empty());
    // The flat face with the most triangles: the cavity's floor round the recess.
    let (fi, &(first, count)) = mesh.face_ranges.iter().enumerate().filter(|(i, _)| shape.sub_shape(ShapeType::Face, *i).is_ok_and(|f| matches!(f.face_surface(), Ok(k::Surface::Plane(_))))).max_by_key(|(_, r)| r.1).expect("a flat face");
    let mut turned = mesh.clone();
    turned.indices.swap(first as usize + 1, first as usize + 2);
    assert!(planar_face_problems(&shape, &turned).iter().any(|p| p.starts_with(&format!("face {fi} "))), "a turned triangle passed");
    // Its first triangle's corner moved to the face's corner farthest from it: across the face.
    let mut across = mesh.clone();
    let tri = &mesh.indices[first as usize..first as usize + 3];
    let p = |i: u32| DVec3::from(mesh.positions[i as usize].map(f64::from));
    let far = mesh.indices[first as usize..(first + count) as usize].iter().copied().max_by(|&a, &b| p(a).distance(p(tri[0])).total_cmp(&p(b).distance(p(tri[0])))).expect("corners");
    across.indices[first as usize + 2] = far;
    assert!(coverage_problems(&shape, &across).iter().any(|p| p.starts_with(&format!("face {fi} "))), "a triangle across the face passed: {:?}", coverage_problems(&shape, &across));
}
