//! Export options and writers: STEP schemas and units (read back).

use glam::DVec3;
use noshape_kernel::*;

fn part() -> Shape {
    let b = make_box_corners(DVec3::ZERO, DVec3::new(0.04, 0.02, 0.01)).unwrap();
    let c = make_cylinder(&Plane::from_normal(DVec3::new(0.02, 0.01, -0.001), DVec3::Z), 0.005, 0.012, std::f64::consts::TAU).unwrap();
    cut(&b, &c).unwrap().shape
}

#[test]
#[ignore = "T2: export round trips (platform last digits; 36 s)"]
fn step_schemas_and_units() {
    let s = part();
    let want = volume_properties(&s).unwrap().mass;
    let dir = std::env::temp_dir().join(format!("noshape-export-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for (schema, tag) in [(StepSchema::Ap242, "AP242"), (StepSchema::Ap214, "AUTOMOTIVE_DESIGN"), (StepSchema::Ap203, "CONFIG_CONTROL_DESIGN")] {
        for (unit, utag) in [(LengthUnit::Millimeter, "MILLI"), (LengthUnit::Inch, "INCH"), (LengthUnit::Yard, "'YARD'")] {
            let path = dir.join(format!("{tag}-{}.step", utag.trim_matches('\'')));
            write_step_with(&path, &[ExportShape { shape: s.clone(), name: Some("Part 1".into()), color: None }], LengthUnit::Meter, &StepOptions { schema, file_unit: Some(unit), pmi: Vec::new() }).unwrap();
            let text = std::fs::read_to_string(&path).unwrap();
            let header = text.split("ENDSEC").next().unwrap();
            assert!(header.to_uppercase().contains(tag), "{tag}: {header}");
            assert!(text.contains(utag), "{tag} {utag}: unit not written");
            let back = read_step(&path, LengthUnit::Meter).unwrap();
            let v: f64 = back.iter().map(|b| volume_properties(&b.shape).unwrap().mass).sum();
            assert!((v - want).abs() < 1e-6 * want, "{tag} {utag}: {v} vs {want}");
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// Parts covering every kind of geometry the Parasolid writer emits (metres).
fn xt_parts() -> Vec<(&'static str, Shape)> {
    use std::f64::consts::TAU;
    let z = |p: DVec3| Plane::from_normal(p, DVec3::Z);
    let mm = 1e-3;
    let block = make_box_corners(DVec3::ZERO, DVec3::new(40.0, 20.0, 10.0) * mm).unwrap();
    let rounded = fillet(&block, &block.edges(), 3.0 * mm).unwrap().shape;
    // Two crossing cylinders: their intersection is an approximated (tolerant) curve.
    let c1 = make_cylinder(&z(DVec3::ZERO), 5.0 * mm, 30.0 * mm, TAU).unwrap();
    let c2 = make_cylinder(&Plane::from_normal(DVec3::new(-15.0, 0.0, 15.0) * mm, DVec3::X), 4.0 * mm, 30.0 * mm, TAU).unwrap();
    let cross = fuse(&c1, &c2).unwrap().shape;
    // Natural loft: B-spline faces.
    let ring = |zz: f64, r: f64| make_wire(&[make_circle(DVec3::new(0.0, 0.0, zz) * mm, DVec3::Z, r * mm).unwrap()]).unwrap();
    let c = [(-6.0, -6.0), (6.0, -6.0), (6.0, 6.0), (-6.0, 6.0)];
    let sq = make_wire(&(0..4).map(|i| make_line(DVec3::new(c[i].0, c[i].1, 15.0) * mm, DVec3::new(c[(i + 1) % 4].0, c[(i + 1) % 4].1, 15.0) * mm).unwrap()).collect::<Vec<_>>()).unwrap();
    let lofted = loft(&[ring(0.0, 10.0), sq, ring(30.0, 10.0)], &LoftOptions { solid: true, ..Default::default() }).unwrap().shape;
    // A spline profile revolved all round: a closed B-spline surface (split for XT).
    let prof = make_bspline(&[DVec3::new(5.0, 0.0, 0.0) * mm, DVec3::new(9.0, 0.0, 4.0) * mm, DVec3::new(6.0, 0.0, 8.0) * mm, DVec3::new(7.0, 0.0, 12.0) * mm], None, &[0.0, 1.0], &[4, 4], 3, false).unwrap();
    let w = make_wire(&[make_line(DVec3::ZERO, DVec3::new(5.0, 0.0, 0.0) * mm).unwrap(), prof, make_line(DVec3::new(7.0, 0.0, 12.0) * mm, DVec3::new(0.0, 0.0, 12.0) * mm).unwrap(), make_line(DVec3::new(0.0, 0.0, 12.0) * mm, DVec3::ZERO).unwrap()]).unwrap();
    let vase = revolve(&make_face_from_wire(&w, true).unwrap(), DVec3::ZERO, DVec3::Z, TAU).unwrap().shape;
    // A cavity: a void region inside the solid.
    let hollow = cut(&block, &make_box_corners(DVec3::new(5.0, 5.0, 3.0) * mm, DVec3::new(35.0, 15.0, 7.0) * mm).unwrap()).unwrap().shape;
    vec![
        ("block", block),
        ("cylinder", make_cylinder(&z(DVec3::ZERO), 5.0 * mm, 10.0 * mm, TAU).unwrap()),
        ("cone", make_cone(&z(DVec3::ZERO), 6.0 * mm, 2.0 * mm, 10.0 * mm, TAU).unwrap()),
        ("pointed cone", make_cone(&z(DVec3::ZERO), 6.0 * mm, 0.0, 10.0 * mm, TAU).unwrap()),
        ("sphere", make_sphere(&z(DVec3::ZERO), 5.0 * mm, TAU).unwrap()),
        ("torus", make_torus(&z(DVec3::ZERO), 10.0 * mm, 3.0 * mm, TAU).unwrap()),
        ("rounded block", rounded),
        ("crossing cylinders", cross),
        ("loft", lofted),
        ("revolved spline", vase),
        ("hollow block", hollow),
        ("part", part()),
    ]
}

/// Parasolid: every part written as .x_t and .x_b reads back (with the
/// orientations from the file, not repaired) as a valid solid of the same volume.
#[test]
#[ignore = "T2: export round trips (platform last digits; 36 s)"]
fn parasolid_round_trip() {
    let dir = std::env::temp_dir().join(format!("noshape-xt-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for (name, s) in xt_parts() {
        assert!(s.is_valid(), "{name}: input");
        let want = volume_properties(&s).unwrap().mass;
        for binary in [false, true] {
            let path = dir.join(format!("{}.{}", name.replace(' ', "-"), if binary { "x_b" } else { "x_t" }));
            write_parasolid(&path, std::slice::from_ref(&s), LengthUnit::Meter, binary).unwrap_or_else(|e| panic!("{name}: {e}"));
            let back = read_parasolid(&path, LengthUnit::Meter).unwrap_or_else(|e| panic!("{name} binary={binary}: {e}"));
            assert!(back.is_valid(), "{name} binary={binary}: invalid");
            assert_eq!(back.count(ShapeType::Solid), s.count(ShapeType::Solid), "{name}");
            // Parasolid's faces: periodic ones (a revolved spline's spun face too)
            // whole; a face closed on a B-surface with a seam that isn't tangent
            // (the loft's side face, through the square's corner) in two.
            let split = usize::from(name == "loft");
            assert_eq!(back.count(ShapeType::Face), s.count(ShapeType::Face) + split, "{name} binary={binary}: faces");
            let v = volume_properties(&back).unwrap().mass;
            // Exact but for tolerant edges rebuilt from their fins' curves.
            assert!((v - want).abs() < 5e-6 * want, "{name} binary={binary}: {v} vs {want}");
        }
    }
    // Several parts in one file.
    let parts: Vec<Shape> = xt_parts().into_iter().map(|p| p.1).take(3).collect();
    let path = dir.join("three.x_t");
    write_parasolid(&path, &parts, LengthUnit::Meter, false).unwrap();
    let back = read_parasolid(&path, LengthUnit::Meter).unwrap();
    assert_eq!(back.count(ShapeType::Solid), 3);
    let _ = std::fs::remove_dir_all(&dir);
}

/// A bare binary transmit file (`B`, little-endian, as Cadenas PARTsolutions writes vendor parts:
/// XT-B3-BINARY-TRANSMIT) reads as its text twin does: one valid solid with the same faces and volume.
#[test]
fn parasolid_bare_binary_reads_as_text() {
    let dir = std::env::temp_dir().join(format!("noshape-xt-bare-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let s = part();
    let (text, bare) = (dir.join("part.x_t"), dir.join("part-bare.x_b"));
    write_parasolid(&text, std::slice::from_ref(&s), LengthUnit::Meter, false).unwrap();
    write_parasolid_bare(&bare, std::slice::from_ref(&s), LengthUnit::Meter).unwrap();
    let body = std::fs::read(&bare).unwrap();
    let at = body.windows(15).position(|w| w == b"**END_OF_HEADER").expect("header");
    let start = at + body[at..].iter().position(|b| *b == b'\n').unwrap() + 1;
    assert_eq!(body[start], b'B', "a bare binary body");
    let (a, b) = (read_parasolid(&text, LengthUnit::Meter).unwrap(), read_parasolid(&bare, LengthUnit::Meter).unwrap());
    assert!(b.is_valid(), "bare: invalid");
    assert_eq!((b.count(ShapeType::Solid), b.count(ShapeType::Face), b.count(ShapeType::Edge)), (a.count(ShapeType::Solid), a.count(ShapeType::Face), a.count(ShapeType::Edge)));
    let (va, vb) = (volume_properties(&a).unwrap().mass, volume_properties(&b).unwrap().mass);
    assert!((va - vb).abs() <= 1e-9 * va, "{vb} vs {va}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Onshape's file of three parts (qa4 round 4 s5's Part Studio, live's export: a PART_XMT_BLOCK root in its embedded
/// schema) reads as its three parts (the block's type described there as a new one, not a change to a known one).
#[test]
fn an_onshape_file_of_three_parts_reads_as_them() {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/xt-parts/onshape-three-parts.x_t");
    assert_eq!(read_parasolid(&p, LengthUnit::Millimeter).unwrap().count(ShapeType::Solid), 3);
}

/// Onshape's Parasolid of a Part Studio with a mesh part (live c2-hex-bookcase-derived's zip: the B-rep "shrunk" and the
/// imported mesh "imported") keeps the mesh in the .xmm_txt written beside the .x_t: both parts read, the mesh as a
/// mesh solid of live's volume. Without its mesh file the file fails, naming it, never reads as part of itself.
#[test]
fn an_onshape_parasolid_with_a_mesh_part_reads_the_mesh_beside_it() {
    let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/xt-parts");
    let parts = read_parasolid_parts(&data.join("hex-bookcase-mesh.x_t"), LengthUnit::Millimeter).unwrap();
    let mut names: Vec<&str> = parts.iter().filter_map(|p| p.name.as_deref()).collect();
    names.sort();
    assert_eq!(names, ["imported", "shrunk"]);
    let part = |n: &str| &parts.iter().find(|p| p.name.as_deref() == Some(n)).unwrap().shape;
    let mesh = part("imported");
    assert!(mesh.is_mesh() && mesh.count(ShapeType::Solid) == 1, "{} solids", mesh.count(ShapeType::Solid));
    // Live's import of the same zip: 10364.977 mm³ (band 10364.502 .. 10365.452), 9366.251 (9358.891 .. 9373.612).
    let v = volume_properties(mesh).unwrap().mass;
    assert!((10364.502..=10365.452).contains(&v), "mesh volume {v}");
    let v = volume_properties(part("shrunk")).unwrap().mass;
    assert!((9358.891..=9373.612).contains(&v), "shrunk volume {v}");
    let dir = std::env::temp_dir().join(format!("noshape-xt-mesh-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::copy(data.join("hex-bookcase-mesh.x_t"), dir.join("alone.x_t")).unwrap();
    let e = read_parasolid_parts(&dir.join("alone.x_t"), LengthUnit::Millimeter).map(|_| ()).unwrap_err().to_string();
    assert!(e.contains(".xmm_txt"), "{e}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A mesh part written to Parasolid as live writes one (c2-hex-bookcase-derived's "imported"): from 28.1 on a MESH
/// whose triangles go in the .xmm_txt beside the .x_t (the file naming 28.1 its minimum version), read back as the same
/// mesh solid; 28.0 has no meshes and refuses it, as live's export.
#[test]
fn a_mesh_part_goes_to_parasolid_as_a_mesh_beside_the_file() {
    let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/xt-parts");
    let parts = read_parasolid_parts(&data.join("hex-bookcase-mesh.x_t"), LengthUnit::Millimeter).unwrap();
    let mesh = parts.iter().find(|p| p.name.as_deref() == Some("imported")).unwrap().shape.clone();
    let want = volume_properties(&mesh).unwrap().mass;
    let dir = std::env::temp_dir().join(format!("noshape-xt-mesh-out-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("part.x_t");
    for version in ["38.1", "30.0", "28.1"] {
        write_parasolid_version(&path, std::slice::from_ref(&mesh), LengthUnit::Millimeter, false, version).unwrap();
        let text = std::fs::read_to_string(dir.join("part.xmm_txt")).unwrap();
        assert!(text.contains(": MESH FILE created by modeller version"), "{version}: {}", &text[..text.len().min(400)]);
        let part = std::fs::read_to_string(&path).unwrap().replace('\n', "");
        assert!(part.contains(": TRANSMIT FILE created by modeller version 3801231 minimum version 2801000"), "{version}");
        let back = read_parasolid_parts(&path, LengthUnit::Millimeter).unwrap();
        assert_eq!(back.len(), 1, "{version}");
        assert!(back[0].shape.is_mesh() && back[0].shape.count(ShapeType::Solid) == 1, "{version}");
        let v = volume_properties(&back[0].shape).unwrap().mass;
        assert!((v - want).abs() <= 1e-9 * want, "{version}: {v} vs {want}");
    }
    std::fs::remove_file(dir.join("part.xmm_txt")).unwrap();
    let e = write_parasolid_version(&path, std::slice::from_ref(&mesh), LengthUnit::Millimeter, false, "28.0").unwrap_err().to_string();
    assert!(e.contains("The selected Parasolid version does not support the current geometry contents."), "{e}");
    assert!(!dir.join("part.xmm_txt").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

/// Onshape's STEP of a mesh part (live c2-hex-bookcase-derived's "imported", trimmed to it: a TESSELLATED_SHELL of one
/// COMPLEX_TRIANGULATED_FACE) reads as a mesh solid of live's volume, as live's import of it.
#[test]
fn an_onshape_step_tessellated_part_reads_as_a_mesh_solid() {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/step/hex-bookcase-mesh.step");
    let parts = read_step(&p, LengthUnit::Millimeter).unwrap();
    let shapes: Vec<_> = parts.iter().map(|p| (p.name.clone(), p.shape.shape_type(), p.shape.count(ShapeType::Solid), p.shape.count(ShapeType::Face), p.shape.is_mesh())).collect();
    assert_eq!(parts.len(), 1, "{shapes:?}");
    let part = &parts[0];
    assert_eq!(part.name.as_deref(), Some("imported"));
    assert!(part.shape.is_mesh() && part.shape.count(ShapeType::Solid) == 1, "{shapes:?}");
    // Live's import of its STEP: 10364.977 mm³ (band 10364.502 .. 10365.453).
    let v = volume_properties(&part.shape).unwrap().mass;
    assert!((10364.502..=10365.453).contains(&v), "volume {v}");
}

/// An IGES part whose faces come split on one surface (IGES writers split periodic faces: Onshape's own writes a
/// cylinder in halves) reads with them merged, as Onshape's import (the exchange suite's pulley: 16 faces, live 9).
#[test]
fn an_iges_parts_faces_on_one_surface_read_merged() {
    let dir = std::env::temp_dir().join(format!("noshape-iges-merge-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let cylinder = make_cylinder(&Plane::from_normal(DVec3::ZERO, DVec3::Z), 10.0, 20.0, std::f64::consts::TAU).unwrap();
    // Halved through its axis: its side and caps in two pieces each, one solid still (the pieces glued).
    let halves = split_by_plane(&cylinder, &Plane::new(DVec3::ZERO, DVec3::X, DVec3::Y)).unwrap().shape;
    let one = fuse(&halves.solids()[0], &halves.solids()[1]).map(|r| r.shape).unwrap_or(halves.clone());
    assert!(one.count(ShapeType::Face) > 3, "{} faces written", one.count(ShapeType::Face));
    let path = dir.join("halves.igs");
    write_iges(&path, std::slice::from_ref(&one), LengthUnit::Millimeter).unwrap();
    let parts = read_iges_parts(&path, LengthUnit::Millimeter).unwrap();
    assert_eq!(parts.iter().map(|p| p.shape.count(ShapeType::Face)).sum::<usize>(), 3);
    let _ = std::fs::remove_dir_all(&dir);
}

/// A boss whose round is wider than the boss less the round (a spindle torus: OCCT keeps it a circle turned about the
/// axis, as Onshape's IGES writes it) reads with its round in one face when written in halves, as Onshape's import
/// (cf-snap-hook's live.igs: 128 faces, ours 136 before).
#[test]
fn an_iges_spindle_torus_in_halves_reads_as_one_face() {
    let dir = std::env::temp_dir().join(format!("noshape-iges-spindle-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    // Revolved about z: a boss of r 4.5, its top rim rounded by r 2.5 about a circle 2 from the axis.
    let p = |x: f64, z: f64| DVec3::new(x, 0.0, z);
    let a = std::f64::consts::FRAC_1_SQRT_2 * 2.5;
    let edges = [
        make_line(p(0.0, 0.0), p(4.5, 0.0)).unwrap(),
        make_line(p(4.5, 0.0), p(4.5, 7.5)).unwrap(),
        make_arc_3pts(p(4.5, 7.5), p(2.0 + a, 7.5 + a), p(2.0, 10.0)).unwrap(),
        make_line(p(2.0, 10.0), p(0.0, 10.0)).unwrap(),
        make_line(p(0.0, 10.0), p(0.0, 0.0)).unwrap(),
    ];
    let profile = make_face(&Plane::new(DVec3::ZERO, DVec3::Y, DVec3::X), &make_wire(&edges).unwrap(), &[]).unwrap();
    let boss = revolve(&profile, DVec3::ZERO, DVec3::Z, std::f64::consts::TAU).unwrap().shape;
    let halves = split_by_plane(&boss, &Plane::new(DVec3::ZERO, DVec3::X, DVec3::Y)).unwrap().shape;
    let one = fuse(&halves.solids()[0], &halves.solids()[1]).map(|r| r.shape).unwrap_or(halves.clone());
    let path = dir.join("boss.igs");
    write_iges(&path, std::slice::from_ref(&one), LengthUnit::Millimeter).unwrap();
    let parts = read_iges_parts(&path, LengthUnit::Millimeter).unwrap();
    assert_eq!(parts.iter().map(|p| p.shape.count(ShapeType::Face)).sum::<usize>(), boss.count(ShapeType::Face));
    let want = volume_properties(&boss).unwrap().mass;
    let v: f64 = parts.iter().map(|p| volume_properties(&p.shape).unwrap().mass).sum();
    assert!((v - want).abs() < 1e-6 * want, "{v} vs {want}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The same spindle-rounded boss shelled from its bottom (its inner round an offset of the round) writes to x_t with
/// both rounds whole tori, as live writes cf-snap-hook's (ours cut its offsets in two: 136 faces read back for 128).
#[test]
fn a_spindle_round_and_its_shell_offset_write_to_xt_as_whole_tori() {
    let dir = std::env::temp_dir().join(format!("noshape-xt-spindle-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = |x: f64, z: f64| DVec3::new(x, 0.0, z);
    let a = std::f64::consts::FRAC_1_SQRT_2 * 2.5;
    let edges = [
        make_line(p(0.0, 0.0), p(4.5, 0.0)).unwrap(),
        make_line(p(4.5, 0.0), p(4.5, 7.5)).unwrap(),
        make_arc_3pts(p(4.5, 7.5), p(2.0 + a, 7.5 + a), p(2.0, 10.0)).unwrap(),
        make_line(p(2.0, 10.0), p(0.0, 10.0)).unwrap(),
        make_line(p(0.0, 10.0), p(0.0, 0.0)).unwrap(),
    ];
    let profile = make_face(&Plane::new(DVec3::ZERO, DVec3::Y, DVec3::X), &make_wire(&edges).unwrap(), &[]).unwrap();
    let boss = revolve(&profile, DVec3::ZERO, DVec3::Z, std::f64::consts::TAU).unwrap().shape;
    let bottom = boss.faces().into_iter().find(|f| f.centroid().is_ok_and(|c| c.z.abs() < 1e-9)).unwrap();
    let cup = shell(&boss, &[bottom], 1.0, ShellSide::Inside, JoinType::Arc).unwrap().shape;
    let kinds = |s: &Shape| s.faces().iter().map(|f| format!("{:?}", f.face_surface().unwrap()).split([' ', '(', '{']).next().unwrap().to_string()).collect::<Vec<_>>();
    let model = kinds(&cup);
    assert!(model.iter().any(|k| k == "Offset"), "the case: the inner round an offset ({model:?})");
    let path = dir.join("cup.x_t");
    write_parasolid(&path, std::slice::from_ref(&cup), LengthUnit::Millimeter, false).unwrap();
    let back = read_parasolid(&path, LengthUnit::Millimeter).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(back.count(ShapeType::Face), cup.count(ShapeType::Face), "{:?}", kinds(&back));
    assert_eq!(kinds(&back).iter().filter(|k| *k == "Torus").count(), 2, "{:?}", kinds(&back));
    let (v0, v1) = (volume_properties(&cup).unwrap().mass, volume_properties(&back).unwrap().mass);
    assert!((v1 - v0).abs() < 1e-6 * v0, "{v1} vs {v0}");
}

/// Meshes of one file read one per object or group (3MF, OBJ) are a part each, never joined by shared edges nor made
/// another's hollow (live meshparts-1119: c2-duct-multipart's touching parts stay apart; meshcheck-1119:
/// cf-sheet-metal-model-plus' part wholly inside another stays a part, the outer one keeps its full volume).
#[test]
fn mesh_groups_are_parts_each_even_one_inside_another() {
    let dir = std::env::temp_dir().join(format!("noshape-mesh-groups-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let boxes = [(DVec3::ZERO, DVec3::splat(20.0)), (DVec3::splat(5.0), DVec3::splat(15.0)), (DVec3::new(20.0, 0.0, 0.0), DVec3::new(30.0, 20.0, 20.0))];
    let groups: Vec<Shape> = boxes
        .iter()
        .enumerate()
        .map(|(i, (a, b))| {
            let path = dir.join(format!("g{i}.stl"));
            write_stl(&make_box_corners(*a, *b).unwrap(), &path, 0.1, StlFormat::Binary).unwrap();
            read_mesh_bodies(&path, LengthUnit::Millimeter).unwrap()
        })
        .collect();
    let _ = std::fs::remove_dir_all(&dir);
    let parts = mesh_group_parts(&groups).unwrap();
    let volume = |s: &Shape| volume_properties(s).unwrap().mass * 1e9;
    let volumes: Vec<f64> = parts.iter().map(volume).collect();
    assert_eq!(volumes.len(), 3);
    for (v, want) in volumes.iter().zip([8000.0, 1000.0, 4000.0]) {
        assert!((v - want).abs() < 1e-6, "each box whole, its own part: {volumes:?}");
    }
}

/// A sphere written as a B-spline surface (an IGES writer's rational surfaces: live's o-ring) reads as a sphere, as
/// Onshape's import.
#[test]
fn an_iges_bspline_sphere_reads_as_a_sphere() {
    let dir = std::env::temp_dir().join(format!("noshape-iges-sphere-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let ball = make_sphere(&Plane::from_normal(DVec3::ZERO, DVec3::Z), 10.0, std::f64::consts::TAU).unwrap();
    let data = ball.faces()[0].face_bspline().unwrap();
    let spline = make_bspline_surface(&data, &[], true).unwrap();
    assert!(matches!(spline.faces()[0].face_surface(), Ok(Surface::BSpline { .. })));
    let path = dir.join("ball.igs");
    write_iges(&path, std::slice::from_ref(&spline), LengthUnit::Millimeter).unwrap();
    let parts = read_iges_parts(&path, LengthUnit::Millimeter).unwrap();
    let faces: Vec<Shape> = parts.iter().flat_map(|p| p.shape.faces()).collect();
    assert_eq!(faces.len(), 1);
    assert!(matches!(faces[0].face_surface(), Ok(Surface::Sphere { .. })), "{:?}", faces[0].face_surface());
    let want = 4.0 / 3.0 * std::f64::consts::PI * 1000.0;
    let v: f64 = parts.iter().map(|p| volume_properties(&p.shape).unwrap().mass).sum();
    assert!((v - want).abs() < 1e-6 * want, "{v} vs {want}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A cone written as a B-surface (live's c2-hex-bookcase-derived "shrunk": a u-periodic cubic B-surface live's import
/// makes a CONE) reads back as a cone from Parasolid, at the cone's area.
#[test]
fn a_parasolid_bspline_cone_reads_as_a_cone() {
    let dir = std::env::temp_dir().join(format!("noshape-xt-cone-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    // The side of a cone from r 6 mm at z 0 to r 2 mm at z 10 mm: an exact rational circle along u, a line along v.
    let mm = 1e-3;
    let square = [(1.0, 0.0), (1.0, 1.0), (0.0, 1.0), (-1.0, 1.0), (-1.0, 0.0), (-1.0, -1.0), (0.0, -1.0), (1.0, -1.0), (1.0, 0.0)];
    let poles = square.iter().flat_map(|&(x, y)| [DVec3::new(6.0 * mm * x, 6.0 * mm * y, 0.0), DVec3::new(2.0 * mm * x, 2.0 * mm * y, 10.0 * mm)]).collect();
    let weights = (0..9).flat_map(|i| [if i % 2 == 0 { 1.0 } else { std::f64::consts::FRAC_1_SQRT_2 }; 2]).collect();
    let data = BSplineSurfaceData {
        poles,
        nu: 9,
        nv: 2,
        weights: Some(weights),
        u_knots: vec![0.0, 0.0, 0.0, 0.25, 0.25, 0.5, 0.5, 0.75, 0.75, 1.0, 1.0, 1.0],
        v_knots: vec![0.0, 0.0, 1.0, 1.0],
        u_degree: 2,
        v_degree: 1,
        u_periodic: false,
        v_periodic: false,
    };
    let spline = make_bspline_surface(&data, &[], false).unwrap();
    let want = std::f64::consts::PI * (6.0 + 2.0) * mm * ((10.0 * mm).powi(2) + (4.0 * mm).powi(2)).sqrt();
    assert!(matches!(spline.faces()[0].face_surface(), Ok(Surface::BSpline { .. })));
    let path = dir.join("cone.x_t");
    write_parasolid(&path, std::slice::from_ref(&spline), LengthUnit::Meter, false).unwrap();
    let back = read_parasolid(&path, LengthUnit::Meter).unwrap();
    // (Ours writes a closed, non-periodic face in two.)
    for f in back.faces() {
        assert!(matches!(f.face_surface(), Ok(Surface::Cone { .. })), "{:?}", f.face_surface());
    }
    let a: f64 = back.faces().iter().map(|f| f.area().unwrap()).sum();
    assert!((a - want).abs() < 1e-6 * want, "{a} vs {want}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A B-spline sheet in two pieces, each written as its own copy of the surface (an IGES writer's pieces of a face:
/// live's hose and motor-mount), reads as one face, as Onshape's import.
#[test]
fn an_iges_bspline_face_in_pieces_reads_as_one() {
    let dir = std::env::temp_dir().join(format!("noshape-iges-pieces-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let n = 6;
    let heights: Vec<f64> = (0..n * n).map(|k| ((k / n) as f64 * 0.7).sin() + ((k % n) as f64 * 0.5).cos()).collect();
    let sheet = bspline_surface_from_grid(&Plane::xy(), 0.0, 2.0, 0.0, 2.0, &heights, n, n, 3, 1e-6).unwrap();
    let pieces = split_by_plane(&sheet, &Plane::new(DVec3::new(5.0, 0.0, 0.0), DVec3::X, DVec3::Y)).unwrap().shape;
    assert_eq!(pieces.count(ShapeType::Face), 2, "written in two");
    let path = dir.join("sheet.igs");
    write_iges(&path, std::slice::from_ref(&pieces), LengthUnit::Millimeter).unwrap();
    let parts = read_iges_parts(&path, LengthUnit::Millimeter).unwrap();
    assert_eq!(parts.iter().map(|p| p.shape.count(ShapeType::Face)).sum::<usize>(), 1);
    let want = surface_properties(&sheet).unwrap().mass;
    let a: f64 = parts.iter().map(|p| surface_properties(&p.shape).unwrap().mass).sum();
    assert!((a - want).abs() < 1e-6 * want, "{a} vs {want}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Several parts in one Parasolid file (the exchange suite: Onshape rejected our pointer-list root, "Invalid or corrupt
/// input file"): node 1, first, is a PART_XMT_BLOCK listing the bodies, as Onshape's own (with no index_map_offset before
/// 30.1, as live's 28.0 / 28.1 / 30.0 files: "176 2 1 2 0 0 0 <entries>"); 12.0 keeps the pointer list; all read back
/// as the parts.
#[test]
fn several_parasolid_parts_are_listed_by_a_part_block() {
    let dir = std::env::temp_dir().join(format!("noshape-xt-block-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let a = make_box_corners(DVec3::ZERO, DVec3::splat(0.01)).unwrap();
    let b = make_box_corners(DVec3::new(0.02, 0.0, 0.0), DVec3::new(0.03, 0.01, 0.01)).unwrap();
    for (version, root, zeros) in [("38.1", "176", 4), ("30.0", "176", 3), ("28.1", "176", 3), ("28.0", "176", 3), ("12.0", "74", 1)] {
        let path = dir.join(format!("two-{version}.x_t"));
        write_parasolid_version(&path, &[a.clone(), b.clone()], LengthUnit::Meter, false, version).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        let body = &text[text.find("**END_OF_HEADER").unwrap()..];
        let joined: String = body.lines().skip(1).collect();
        let tokens: Vec<&str> = joined.split_whitespace().skip_while(|t| !t.contains("SCH_")).skip(1).collect();
        // (The user field size follows the schema's name unspaced.) The first node: its type, entry count and index 1.
        assert_eq!(&tokens[..3], [root, "2", "1"], "{version}");
        // Then its entry count, its fixed fields (all null here) and the first entry.
        assert_eq!(tokens[3], "2", "{version}");
        assert!(tokens[4..4 + zeros].iter().all(|t| *t == "0") && tokens[4 + zeros] != "0", "{version}: {:?}", &tokens[..10]);
        assert_eq!(read_parasolid(&path, LengthUnit::Meter).unwrap().count(ShapeType::Solid), 2, "{version}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// Named, coloured parts in one Parasolid file: each part's attributes are chained only among its own (Onshape's
/// multi-part files; ours chained a type's attributes across parts, a pointer out of each part).
#[test]
fn several_parasolid_parts_keep_their_attributes_to_themselves() {
    let dir = std::env::temp_dir().join(format!("noshape-xt-attrs-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let part = |at: f64, name: &str| ExportShape { shape: make_box_corners(DVec3::new(at, 0.0, 0.0), DVec3::new(at + 0.01, 0.01, 0.01)).unwrap(), name: Some(name.into()), color: Some([0.2, 0.4, 0.6]) };
    let path = dir.join("three.x_t");
    write_parasolid_parts(&path, &[part(0.0, "A"), part(0.02, "B"), part(0.04, "C")], LengthUnit::Meter, false, "38.1").unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    let joined: String = text[text.find("**END_OF_HEADER").unwrap()..].lines().skip(1).collect();
    // (A name's characters run straight into the next record: "A81 1 …".)
    let mut spaced = String::new();
    for c in joined.chars() {
        if c.is_ascii_digit() && spaced.ends_with(|p: char| p.is_ascii_uppercase()) {
            spaced.push(' ');
        }
        spaced.push(c);
    }
    let t: Vec<i64> = spaced.split_whitespace().skip_while(|t| !t.contains("SCH_")).skip(1).map(|t| t.parse().unwrap_or(-1)).collect();
    // The part block (176, entry count, index 1, n_entries, offset, 3 pointers, entries) gives the bodies.
    let n = t[1] as usize;
    let bodies = &t[8..8 + n];
    // An ATTRIBUTE: 81, field count 1, index, node id, definition, owner, next, previous, next / previous of its type.
    let mut owner_of = std::collections::HashMap::new();
    let mut chains = Vec::new();
    for w in t.windows(10) {
        if w[0] == 81 && w[1] == 1 && bodies.contains(&w[5]) {
            owner_of.insert(w[2], w[5]);
            chains.push((w[5], w[8], w[9]));
        }
    }
    assert_eq!(chains.len(), 2 * n, "a name and a colour per part");
    for (owner, next, prev) in chains {
        for x in [next, prev].into_iter().filter(|&x| x != 0) {
            assert_eq!(owner_of.get(&x), Some(&owner), "an attribute of part {owner} chained to {x}");
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// An extruded ellipse through Parasolid (the exchange suite, motor-mount-jack): its two ellipses stay whole ring
/// edges (live writes them without a vertex), the side one face; the volume as it was.
#[test]
fn an_extruded_ellipse_keeps_its_ring_edges_through_parasolid() {
    let dir = std::env::temp_dir().join(format!("noshape-xt-ellipse-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mm = 1e-3;
    let plane = Plane::from_normal(DVec3::ZERO, DVec3::Z);
    let face = make_face_2d(&plane, &[Segment2d::Ellipse { center: glam::DVec2::ZERO, major_radius: 22.5 * mm, minor_radius: 7.5 * mm, rotation: 0.3 }], &[]).unwrap();
    let solid = extrude(&face, DVec3::new(0.0, 0.0, 10.0 * mm)).unwrap().shape;
    let want = volume_properties(&solid).unwrap().mass;
    for binary in [false, true] {
        let path = dir.join(if binary { "ellipse.x_b" } else { "ellipse.x_t" });
        write_parasolid(&path, std::slice::from_ref(&solid), LengthUnit::Meter, binary).unwrap();
        let back = read_parasolid(&path, LengthUnit::Meter).unwrap();
        // (The ellipses as ellipses: exact, as live writes them.)
        let ellipses = back.edges().iter().filter(|e| e.edge_info().is_ok_and(|i| matches!(i.curve, Curve::Ellipse { .. }))).count();
        assert_eq!(ellipses, 2, "binary={binary}");
        assert_eq!((back.count(ShapeType::Face), back.count(ShapeType::Edge)), (solid.count(ShapeType::Face), solid.count(ShapeType::Edge)), "binary={binary}");
        let v = volume_properties(&back).unwrap().mass;
        assert!((v - want).abs() < 1e-9 * want, "binary={binary}: {v} vs {want}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// An assembly as Parasolid (qa ASM-PARASOLID-FLAT): in every version each
/// part once with its instances, read back as the same assembly.
#[test]
fn parasolid_assembly_round_trip() {
    let dir = std::env::temp_dir().join(format!("noshape-xt-asm-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let parts: Vec<Shape> = xt_parts().into_iter().map(|p| p.1).take(2).collect();
    // Named and coloured: read back as such (qa ASM-XT-ROUNDTRIP-NAMES).
    let named: Vec<ExportShape> = parts.iter().enumerate().map(|(i, s)| ExportShape { shape: s.clone(), name: Some(format!("Part {}", i + 1)), color: Some([0.25 * (i + 1) as f32, 0.5, 0.75]) }).collect();
    let turned = glam::DMat4::from_translation(DVec3::new(0.05, 0.0, 0.01)) * glam::DMat4::from_rotation_z(std::f64::consts::FRAC_PI_2);
    let placed = [glam::DMat4::IDENTITY, glam::DMat4::from_translation(DVec3::new(0.1, 0.0, 0.0)), turned];
    let instances: Vec<StepInstance> = [0, 1, 1].into_iter().zip(placed).map(|(part, transform)| StepInstance { part, transform }).collect();
    let path = dir.join("asm.x_t");
    for version in PARASOLID_VERSIONS {
        write_parasolid_assembly(&path, "Gearbox", &named, &instances, LengthUnit::Meter, version).unwrap();
        let (root, back, inst) = read_parasolid_assembly(&path, LengthUnit::Meter).unwrap().unwrap_or_else(|| panic!("{version}: an assembly"));
        assert_eq!((back.len(), inst.len()), (2, 3), "{version}");
        assert_eq!(root, "Gearbox", "{version}");
        for (i, b) in back.iter().enumerate() {
            assert_eq!(b.name.as_deref(), Some(format!("Part {}", i + 1).as_str()), "{version}");
            let c = b.color.unwrap_or_else(|| panic!("{version}: a colour"));
            assert!((c[0] - 0.25 * (i + 1) as f32).abs() < 1e-6 && (c[2] - 0.75).abs() < 1e-6, "{version}: {c:?}");
        }
        for (b, p) in back.iter().zip(&parts) {
            let (v, want) = (volume_properties(&b.shape).unwrap().mass, volume_properties(p).unwrap().mass);
            assert!((v - want).abs() < 5e-6 * want, "{version}: {v} vs {want}");
        }
        for (i, want) in inst.iter().zip(&instances) {
            assert_eq!(i.part, want.part);
            assert!(i.transform.abs_diff_eq(want.transform, 1e-12), "{version}: {:?} vs {:?}", i.transform, want.transform);
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// Parts' names and colours as Parasolid attributes in every version (as
/// Onshape's: SDL/TYSA_NAME, SDL/TYSA_COLOUR_2), read back.
#[test]
fn parasolid_part_names_and_colours() {
    let dir = std::env::temp_dir().join(format!("noshape-xt-names-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let parts: Vec<ExportShape> = xt_parts().into_iter().take(3).enumerate().map(|(i, p)| ExportShape { shape: p.1, name: Some(format!("Bracket {}", i + 1)), color: (i != 1).then_some([0.2, 0.4, 0.6]) }).collect();
    let path = dir.join("named.x_t");
    for version in PARASOLID_VERSIONS {
        write_parasolid_parts(&path, &parts, LengthUnit::Meter, false, version).unwrap();
        let back = read_parasolid_parts(&path, LengthUnit::Meter).unwrap_or_else(|e| panic!("{version}: {e}"));
        let names: Vec<Option<&str>> = back.iter().map(|p| p.name.as_deref()).collect();
        assert_eq!(names, [Some("Bracket 1"), Some("Bracket 2"), Some("Bracket 3")], "{version}");
        assert!(back[0].color.is_some_and(|c| (c[1] - 0.4).abs() < 1e-6) && back[1].color.is_none(), "{version}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// Live's own assembly (a block and a Ø6 × 30 pin, three instances) in each
/// Parasolid version reads as the assembly it is (qa XT-ASM-VERSIONS).
#[test]
fn parasolid_assemblies_from_onshape_in_every_version() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/xt-asm");
    for v in ["12.0", "25.0", "28.0", "28.1", "30.0", "30.1", "31.0", "31.1", "33.0", "33.1", "37.1", "38.1", "latest"] {
        let (_, parts, inst) = read_parasolid_assembly(&dir.join(format!("{v}.x_t")), LengthUnit::Millimeter).unwrap_or_else(|e| panic!("{v}: {e}")).unwrap_or_else(|| panic!("{v}: an assembly"));
        assert_eq!((parts.len(), inst.len()), (2, 3), "{v}");
        let mut vols: Vec<f64> = parts.iter().map(|p| volume_properties(&p.shape).unwrap().mass).collect();
        vols.sort_by(f64::total_cmp);
        assert!((vols[0] - 848.23).abs() < 0.01, "{v}: the pin {vols:?}");
    }
}

/// Parasolid versions (the Export dialog's list): each writes the body in
/// that version's layouts with Onshape's header (live G36: one key per line,
/// part 2 the writer's schema, the first body line naming the version's
/// schema), and reads back with the same volume.
#[test]
#[ignore = "T2: export round trips (platform last digits; 36 s)"]
fn parasolid_versions() {
    let dir = std::env::temp_dir().join(format!("noshape-xtv-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let (_, s) = xt_parts().into_iter().find(|p| p.0 == "rounded block").expect("rounded block");
    let want = volume_properties(&s).unwrap().mass;
    // Schema per version, as Onshape's files name it (samples/g36-g24b-v*.x_t).
    let schemas = [("38.1", "SCH_3801231_38101"), ("33.0", "SCH_3300000_32001"), ("29.0", "SCH_2900000_28101"), ("26.1", "SCH_2601000_26105"), ("25.0", "SCH_2500000_25001"), ("12.0", "SCH_1200000_12006")];
    for v in PARASOLID_VERSIONS {
        for binary in [false, true] {
            let path = dir.join(format!("v{v}.{}", if binary { "x_b" } else { "x_t" }));
            write_parasolid_version(&path, std::slice::from_ref(&s), LengthUnit::Meter, binary, v).unwrap_or_else(|e| panic!("{v}: {e}"));
            let back = read_parasolid(&path, LengthUnit::Meter).unwrap_or_else(|e| panic!("{v} binary={binary}: {e}"));
            assert_eq!(back.count(ShapeType::Face), s.count(ShapeType::Face), "{v} binary={binary}");
            let got = volume_properties(&back).unwrap().mass;
            assert!((got - want).abs() < 5e-6 * want, "{v} binary={binary}: {got} vs {want}");
            if binary {
                continue;
            }
            let text = std::fs::read_to_string(&path).unwrap();
            let (head, body) = text.split_once("**END_OF_HEADER").unwrap();
            for key in ["**PART1;\nMC=unknown;\n", "\nFORMAT=text;\nGUISE=transmit;\n", &format!("\nFILE=v{v}.x_t;\n"), "**PART2;\nSCH=SCH_3801231_38101;\nUSFLD_SIZE=0;\n**PART3;\n"] {
                assert!(head.contains(key), "{v}: header lacks {key:?}: {head}");
            }
            let first = body.lines().nth(1).unwrap();
            // The modeller is 38.1.231's in every version (live: "… version 3801231" + "17" + "SCH_2500000_25001").
            assert!(first.starts_with("T51 : TRANSMIT FILE created by modeller version 3801231"), "{v}: {first}");
            if let Some((_, sch)) = schemas.iter().find(|x| x.0 == v) {
                assert!(first.starts_with(&format!("T51 : TRANSMIT FILE created by modeller version 3801231{} {sch}", sch.len())), "{v}: {first}");
            }
        }
    }
    // Unknown versions are refused.
    assert!(write_parasolid_version(&dir.join("bad.x_t"), std::slice::from_ref(&s), LengthUnit::Meter, false, "24.0").is_err());
    let _ = std::fs::remove_dir_all(&dir);
}

/// IGES: the Start and Global sections laid out as Onshape's (live "qa5
/// iges header"), the part back with its volume.
#[test]
#[ignore = "T2: export round trips (platform last digits; 36 s)"]
fn iges_header() {
    let dir = std::env::temp_dir().join(format!("noshape-igs-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let s = part();
    let want = volume_properties(&s).unwrap().mass;
    let path = dir.join("block - Part 1.iges");
    write_iges(&path, std::slice::from_ref(&s), LengthUnit::Meter).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines[0], format!("{:72}S      1", ""));
    assert!(lines[1].starts_with(&format!("File generated by Noshape Version {}", noshape_kernel::VERSION)), "{}", lines[1]);
    assert!(lines.iter().all(|l| l.len() == 80), "80-column records");
    let global: String = lines.iter().filter(|l| &l[72..73] == "G").map(|l| &l[..72]).collect();
    assert!(global.starts_with("1H,,1H;,14Hblock - Part 1,19Hblock - Part 1.iges,7HNoshape,"), "{global}");
    assert!(global.contains(",32,24,8,56,15,14Hblock - Part 1,1.,2,2HMM,1000,1.,13H"), "{global}");
    assert!(global.contains(",9,0,13H"), "{global}");
    // Sequence numbers right-aligned.
    assert!(lines.iter().any(|l| l.ends_with("D      1")), "{}", lines.iter().find(|l| &l[72..73] == "D").unwrap());
    let back = read_iges(&path, LengthUnit::Meter).unwrap();
    let got = volume_properties(&back).unwrap().mass;
    assert!((got - want).abs() < 1e-6 * want, "{got} vs {want}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// IGES "Use compatibility mode for surfaces and curves" (live exchange batch 1, 11-20): each face a Trimmed Surface
/// (144), no Manifold Solid (186), every surface and curve a B-spline (128 / 126 / 102, no face of revolution over 180
/// degrees: live c2-housing-draft's live-opt-compat.igs); its faces read back sewn into the solid, as live imports them.
#[test]
fn iges_compatibility_mode_reads_back_as_a_solid() {
    let dir = std::env::temp_dir().join(format!("noshape-igs-compat-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let block = (make_box(&Plane::xy(), 0.04, 0.03, 0.01).unwrap(), 6, 1.2e-5);
    // Its side in halves, and the two caps.
    let cylinder = (make_cylinder(&Plane::xy(), 0.01, 0.02, std::f64::consts::TAU).unwrap(), 4, std::f64::consts::PI * 1e-4 * 0.02);
    // A 270-degree wedge: its side in two (no face of revolution over 180 degrees), two caps, two flat sides.
    let wedge = (make_cylinder(&Plane::xy(), 0.01, 0.02, 1.5 * std::f64::consts::PI).unwrap(), 6, 0.75 * std::f64::consts::PI * 1e-4 * 0.02);
    for (i, (s, faces, want)) in [block, cylinder, wedge].into_iter().enumerate() {
        let path = dir.join(format!("compat-{i}.igs"));
        write_iges_parts_as(&path, &[ExportShape { shape: s, name: None, color: None }], LengthUnit::Meter, true).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        let types: Vec<u32> = text.lines().filter(|l| l.len() >= 73 && &l[72..73] == "D").step_by(2).filter_map(|l| l[..8].trim().parse().ok()).collect();
        let back = read_iges_parts(&path, LengthUnit::Meter).unwrap();
        assert_eq!(types.iter().filter(|t| **t == 144).count(), faces, "{types:?}");
        assert!(types.iter().all(|t| [102, 126, 128, 142, 144, 314, 402, 406].contains(t)), "{types:?}");
        assert_eq!(back.len(), 1);
        let solids = back[0].shape.solids();
        assert_eq!(solids.len(), 1, "sewn into one solid");
        let v = volume_properties(&solids[0]).unwrap().mass;
        assert!((v - want).abs() < 1e-6 * want, "{v} vs {want}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// Parts' names and colours in IGES as Onshape writes them (live
/// qa5-iges-header.igs: the name in the solid's, edge list's and vertex
/// list's labels, the colour a Color Definition the solid and its faces
/// point to, the whole name as a Name property), read back as live reads
/// them: the label; an unnamed solid " DE" and its directory line (live 1818;
/// qa IGES-IMPORT-NAME-SOLID: XDE named it "SOLID").
#[test]
fn iges_part_names_and_colours() {
    let dir = std::env::temp_dir().join(format!("noshape-igs-names-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let parts: Vec<ExportShape> = xt_parts().into_iter().take(3).enumerate().map(|(i, p)| ExportShape { shape: p.1, name: (i != 2).then(|| format!("Part {}", i + 1)), color: (i != 1).then_some([0.2, 0.4, 0.6]) }).collect();
    let path = dir.join("named.igs");
    write_iges_parts(&path, &parts, LengthUnit::Meter).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    let dir_lines: Vec<&str> = text.lines().filter(|l| &l[72..73] == "D").collect();
    let labelled = |t: &str| dir_lines.chunks(2).filter(|d| d[1][..8].trim() == t && &d[1][56..72] == "  Part 1       0").count();
    assert_eq!((labelled("186"), labelled("504"), labelled("502")), (1, 1, 1), "Part 1's labels");
    assert_eq!(dir_lines.chunks(2).filter(|d| d[0][..8].trim() == "314").count(), 1, "one Color Definition per colour");
    let colour = text.lines().find(|l| l.starts_with("314,") && &l[72..73] == "P").unwrap_or_default();
    let rgb: Vec<f64> = colour.split(',').skip(1).take(3).map(|v| v.parse().unwrap()).collect();
    assert!(rgb.iter().zip([20.0, 40.0, 60.0]).all(|(a, b)| (a - b).abs() < 1e-4) && colour.contains(",5HCOLOR;"), "the colour in percent: {colour}");
    // Back-pointers right-aligned as Onshape's ("      1P", not "0000001P").
    assert!(colour[64..72].starts_with(' '), "{colour}");
    let back = read_iges_parts(&path, LengthUnit::Meter).unwrap();
    // The full name also as a Name property (406 form 15), as live's.
    assert!(text.lines().any(|l| l.starts_with("406,1,6HPart 1")), "a 406 Name property");
    let names: Vec<Option<&str>> = back.iter().map(|p| p.name.as_deref()).collect();
    assert_eq!(&names[..2], [Some("Part 1"), Some("Part 2")]);
    // Live names a solid without a label " DE" and its directory line (1818).
    assert!(names[2].is_some_and(|n| n.starts_with(" DE") && n.len() == 12 && n.trim_start_matches(" DE").trim().parse::<u32>().is_ok()), "{names:?}");
    assert!(back[0].color.is_some_and(|c| (c[1] - 0.4).abs() < 1e-3) && back[1].color.is_none() && back[2].color.is_some(), "{:?}", back.iter().map(|p| p.color).collect::<Vec<_>>());
    let _ = std::fs::remove_dir_all(&dir);
}

/// ACIS import: our own .sat of every test part back with its volume, and
/// Onshape's SAT exports (outside the repo; skipped when absent).
#[test]
#[ignore = "T2: export round trips (platform last digits; 36 s)"]
fn acis_import() {
    let dir = std::env::temp_dir().join(format!("noshape-satin-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for (name, s) in xt_parts() {
        let want = volume_properties(&s).unwrap().mass;
        let path = dir.join(format!("{}.sat", name.replace(' ', "-")));
        write_acis(&path, std::slice::from_ref(&s), LengthUnit::Meter).unwrap();
        let back = read_acis_parts(&path, LengthUnit::Meter).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(back.len(), 1, "{name}");
        assert!(back[0].shape.is_valid(), "{name}: invalid");
        let v = volume_properties(&back[0].shape).unwrap().mass;
        assert!((v - want).abs() < 5e-6 * want, "{name}: {v} vs {want}");
    }
    let _ = std::fs::remove_dir_all(&dir);
    let samples = std::path::Path::new("/root/onshape-research/live/samples");
    for (file, name, exact) in [("onshape-cone.sat", "Part 1", 120.0 * std::f64::consts::PI), ("g26-revolved-spline.sat", "revolved spline", 1726.080764)] {
        if !samples.join(file).exists() {
            continue;
        }
        let parts = read_acis_parts(&samples.join(file), LengthUnit::Meter).unwrap_or_else(|e| panic!("{file}: {e}"));
        let v = volume_properties(&parts[0].shape).unwrap().mass * 1e9;
        assert!(parts[0].shape.is_valid() && (v - exact).abs() < 1e-3, "{file}: {v} vs {exact}");
        assert_eq!(parts[0].name.as_deref(), Some(name), "{file}");
    }
}

/// JT import: our own JT of every test part back through its XT segment,
/// and a JT 10.5 assembly (XZ-compressed segments) from NX when present.
#[test]
#[ignore = "T2: export round trips (platform last digits; 36 s)"]
fn jt_import() {
    let dir = std::env::temp_dir().join(format!("noshape-jtin-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for (name, s) in xt_parts() {
        let want = volume_properties(&s).unwrap().mass;
        let path = dir.join(format!("{}.jt", name.replace(' ', "-")));
        write_jt(&path, &[ExportShape { shape: s.clone(), name: Some(name.into()), color: None }], LengthUnit::Meter).unwrap_or_else(|e| panic!("{name}: {e}"));
        let back = read_jt_parts(&path, LengthUnit::Meter).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(back.len(), 1, "{name}");
        let v = volume_properties(&back[0].shape).unwrap().mass;
        assert!(back[0].shape.is_valid() && (v - want).abs() < 5e-6 * want, "{name}: {v} vs {want}");
    }
    let _ = std::fs::remove_dir_all(&dir);
    let nist = std::path::Path::new("/root/kern-native-samples/jt/nist_mtc_crada_assembly_NX.jt");
    if nist.exists() {
        let parts = read_jt_parts(nist, LengthUnit::Meter).unwrap();
        // Placed by the scene graph: every instance (10 nuts, 18 + 4 cap screws, 4
        // flat-head screws, box, cover, plate, window), the box from a multi-XT
        // segment.
        assert_eq!(parts.len(), 36);
        let centre = |p: &ImportedShape| {
            let b = p.shape.bounding_box(false).unwrap();
            (b.min + b.max) * 0.5e3
        };
        let nuts: Vec<_> = parts.iter().filter(|p| p.name.as_deref().is_some_and(|n| n.contains("HEX NUT"))).map(centre).collect();
        let screws: Vec<_> = parts.iter().filter(|p| p.name.as_deref().is_some_and(|n| n.contains("91274A118"))).map(centre).collect();
        assert_eq!(nuts.len(), 10);
        // Each nut sits on a screw's axis.
        for n in &nuts {
            assert!(screws.iter().any(|s| (s.x - n.x).hypot(s.y - n.y) < 0.6), "a nut off every screw at {n:?}");
        }
        assert!(parts.iter().any(|p| p.shape.count(ShapeType::Face) > 300), "the box (multi-XT segment)");
        let polycarb = parts.iter().find(|p| p.shape.count(ShapeType::Face) == 35).expect("the polycarbonate window");
        let v = volume_properties(&polycarb.shape).unwrap().mass * 1e9;
        assert!((v - 17025.66823867636).abs() < 1e-3, "polycarb {v}"); // = Onshape's import of the NX part (G38)
    }
}

/// Sample files for checking in Onshape (live capture): every test part as
/// .x_t, .x_b, .sat, .pvz, .jt, .3dm (and OCCT's .brep) in $NOSHAPE_XT_SAMPLES (default /tmp/noshape-xt-samples),
/// with their volumes in volumes.txt (mm³).
#[test]
#[ignore]
fn write_xt_samples() {
    let dir = std::path::PathBuf::from(std::env::var("NOSHAPE_XT_SAMPLES").unwrap_or_else(|_| "/tmp/noshape-xt-samples".into()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut list = String::new();
    for (name, s) in xt_parts() {
        let stem = name.replace(' ', "-");
        for binary in [false, true] {
            write_parasolid(&dir.join(format!("{stem}.{}", if binary { "x_b" } else { "x_t" })), std::slice::from_ref(&s), LengthUnit::Meter, binary).unwrap();
        }
        write_acis(&dir.join(format!("{stem}.sat")), std::slice::from_ref(&s), LengthUnit::Meter).unwrap();
        write_pvz(&dir.join(format!("{stem}.pvz")), &[ExportShape { shape: s.clone(), name: Some(name.into()), color: None }], LengthUnit::Meter).unwrap();
        write_jt(&dir.join(format!("{stem}.jt")), &[ExportShape { shape: s.clone(), name: Some(name.into()), color: None }], LengthUnit::Meter).unwrap();
        let _ = write_rhino(&dir.join(format!("{stem}.3dm")), &[ExportShape { shape: s.clone(), name: Some(name.into()), color: None }], LengthUnit::Meter, 8);
        write_brep(&s, &dir.join(format!("{stem}.brep"))).unwrap();
        list += &format!("{stem}: {:.6} mm3, {} faces\n", volume_properties(&s).unwrap().mass * 1e9, s.count(ShapeType::Face));
    }
    std::fs::write(dir.join("volumes.txt"), list).unwrap();
}

/// ACIS: the records of every test part hang together (pointers to the right
/// record types, coedge rings closed, partners paired with opposite senses,
/// one lump per solid, faces' loops on their faces).
#[test]
#[ignore = "T2: export round trips (platform last digits; 36 s)"]
fn acis_structure() {
    let dir = std::env::temp_dir().join(format!("noshape-sat-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for (name, s) in xt_parts() {
        let path = dir.join(format!("{}.sat", name.replace(' ', "-")));
        write_acis(&path, std::slice::from_ref(&s), LengthUnit::Meter).unwrap_or_else(|e| panic!("{name}: {e}"));
        let text = std::fs::read_to_string(&path).unwrap();
        let mut lines = text.lines();
        assert_eq!(lines.next().unwrap(), "700 0 1 0", "{name}");
        lines.next();
        assert!(lines.next().unwrap().starts_with("1 "), "{name}: millimetres");
        let body: String = lines.take_while(|l| *l != "End-of-ACIS-data").collect::<Vec<_>>().join(" ");
        let recs: Vec<Vec<&str>> = body.split('#').map(|r| r.split_whitespace().collect::<Vec<_>>()).filter(|r| !r.is_empty()).collect();
        let ty = |i: usize| recs[i][0];
        let p = |r: &Vec<&str>, k: usize| -> Option<usize> {
            let t = r[k];
            assert!(t.starts_with('$'), "{name}: {t} is not a pointer in {r:?}");
            t[1..].parse::<i64>().ok().filter(|v| *v >= 0).map(|v| v as usize)
        };
        for r in &recs {
            assert_eq!((r[1], r[2], r[3]), ("$-1", "-1", "$-1"), "{name}: {r:?}");
        }
        assert_eq!(ty(0), "body");
        let mut faces = 0;
        for (i, r) in recs.iter().enumerate() {
            match r[0] {
                "coedge" => {
                    let (next, prev, edge, lp) = (p(r, 4).unwrap(), p(r, 5).unwrap(), p(r, 7).unwrap(), p(r, 9).unwrap());
                    assert_eq!(ty(next), "coedge");
                    assert_eq!(p(&recs[next], 5), Some(i), "{name}: ring");
                    assert_eq!(p(&recs[prev], 4), Some(i), "{name}: ring");
                    assert_eq!(ty(edge), "edge");
                    assert_eq!(ty(lp), "loop");
                    if let Some(partner) = p(r, 6) {
                        assert_eq!(p(&recs[partner], 6), Some(i), "{name}: partner");
                        assert_ne!(recs[partner][8], r[8], "{name}: partners run opposite ways");
                    }
                }
                "loop" => {
                    assert_eq!(ty(p(r, 5).unwrap()), "coedge");
                    assert_eq!(ty(p(r, 6).unwrap()), "face");
                }
                "face" => {
                    faces += 1;
                    let surf = p(r, 8).unwrap();
                    assert!(ty(surf).ends_with("-surface"), "{name}: {}", ty(surf));
                    assert!(matches!(r[9], "forward" | "reversed"));
                }
                "edge" => {
                    assert_eq!(ty(p(r, 4).unwrap()), "vertex");
                    assert_eq!(ty(p(r, 6).unwrap()), "vertex");
                    assert_eq!(ty(p(r, 8).unwrap()), "coedge");
                    assert!(ty(p(r, 9).unwrap()).ends_with("-curve"));
                }
                _ => {}
            }
        }
        let lumps = recs.iter().filter(|r| r[0] == "lump").count();
        assert_eq!(lumps, s.count(ShapeType::Solid), "{name}");
        assert!(faces >= s.count(ShapeType::Face), "{name}: {faces} faces");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// Rhino: every test part written as a .3dm (each Rhino version the export
/// dialog offers) is one Brep that openNURBS reads back as valid. Only with
/// openNURBS (OPENNURBS_ROOT); without it the writer must say so.
#[test]
#[ignore = "T2: export round trips (platform last digits; 36 s)"]
fn rhino_breps_are_valid() {
    let dir = std::env::temp_dir().join(format!("noshape-3dm-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for (name, s) in xt_parts() {
        for version in [8, 7, 6, 5, 4, 3, 2] {
            let path = dir.join(format!("{}-{version}.3dm", name.replace(' ', "-")));
            let shapes = [ExportShape { shape: s.clone(), name: Some(name.into()), color: None }];
            match write_rhino(&path, &shapes, LengthUnit::Meter, version) {
                Ok(()) => {
                    assert_eq!(rhino_brep_count(&path).unwrap_or_else(|e| panic!("{name} v{version}: {e}")), 1, "{name} v{version}");
                    if version == 8 {
                        // And back through our reader: the same solid, named.
                        let back = read_rhino_parts(&path, LengthUnit::Meter).unwrap_or_else(|e| panic!("{name}: {e}"));
                        assert_eq!(back.len(), 1, "{name}");
                        assert_eq!(back[0].name.as_deref(), Some(name), "{name}");
                        // Within 0.01 %: the sphere comes back as a NURBS surface whose
                        // integrated volume differs by 4e-5 on Windows (live's own bands
                        // for spline solids are ±0.1 %).
                        let (v, want) = (volume_properties(&back[0].shape).unwrap().mass, volume_properties(&s).unwrap().mass);
                        assert!(back[0].shape.is_valid() && (v - want).abs() < 1e-4 * want, "{name}: {v} vs {want}");
                    }
                }
                Err(e) => {
                    assert!(e.to_string().contains("openNURBS"), "{name} v{version}: {e}");
                    return;
                }
            }
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// JT: every test part (and several in one file) gives a JT 9.5 file whose
/// table of contents points at whole segments: the scene graph, a mesh and a
/// B-rep per part. (The encodings are checked against Onshape's own files in
/// the writer's development; Onshape imports: live G21.)
#[test]
#[ignore = "T2: export round trips (platform last digits; 36 s)"]
fn jt_structure() {
    let dir = std::env::temp_dir().join(format!("noshape-jt-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let parts = xt_parts();
    let check = |path: &std::path::Path, n: usize, name: &str| {
        let d = std::fs::read(path).unwrap();
        assert!(d.starts_with(b"Version 9.5 JT"), "{name}");
        let i32_at = |o: usize| i32::from_le_bytes(d[o..o + 4].try_into().unwrap()) as usize;
        let toc = i32_at(85);
        let entries = i32_at(toc);
        assert_eq!(entries, 1 + 2 * n, "{name}: scene graph + mesh + B-rep per part");
        let mut types = Vec::new();
        for e in 0..entries {
            let at = toc + 4 + 28 * e;
            let (off, len, ty) = (i32_at(at + 16), i32_at(at + 20), u32::from_le_bytes(d[at + 24..at + 28].try_into().unwrap()) >> 24);
            assert!(off + len <= d.len(), "{name}: segment {e} past the end");
            assert_eq!(d[at..at + 16], d[off..off + 16], "{name}: segment {e} GUID");
            assert_eq!(i32_at(off + 20), len, "{name}: segment {e} length");
            types.push(ty);
        }
        types.sort();
        let mut want = vec![1];
        want.extend(std::iter::repeat_n(7, n));
        want.extend(std::iter::repeat_n(17, n));
        assert_eq!(types, want, "{name}");
    };
    for (name, s) in &parts {
        let path = dir.join(format!("{}.jt", name.replace(' ', "-")));
        write_jt(&path, &[ExportShape { shape: s.clone(), name: Some((*name).into()), color: None }], LengthUnit::Meter).unwrap_or_else(|e| panic!("{name}: {e}"));
        check(&path, 1, name);
    }
    let three: Vec<ExportShape> = parts.iter().take(3).map(|(n, s)| ExportShape { shape: s.clone(), name: Some((*n).into()), color: None }).collect();
    let path = dir.join("three.jt");
    write_jt(&path, &three, LengthUnit::Meter).unwrap();
    check(&path, 3, "three");
    let _ = std::fs::remove_dir_all(&dir);
}

/// PVZ: every test part (and three in one file) gives a zip with one .pvs
/// structure member and one .ol geometry member per part. (Decoded against
/// Onshape's samples in the writer's development; Onshape imports: live G23.)
#[test]
#[ignore = "T2: export round trips (platform last digits; 36 s)"]
fn pvz_structure() {
    let dir = std::env::temp_dir().join(format!("noshape-pvz-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let parts = xt_parts();
    let check = |path: &std::path::Path, n: usize, name: &str| {
        let d = std::fs::read(path).unwrap();
        assert!(d.starts_with(b"PK\x03\x04"), "{name}");
        // Member names from the central directory (end record: entry count at +10, offset at +16).
        let eocd = d.len() - 22;
        assert_eq!(&d[eocd..eocd + 4], b"PK\x05\x06", "{name}: end of central directory");
        let u16_at = |o: usize| u16::from_le_bytes(d[o..o + 2].try_into().unwrap()) as usize;
        let u32_at = |o: usize| u32::from_le_bytes(d[o..o + 4].try_into().unwrap()) as usize;
        let (entries, mut at) = (u16_at(eocd + 10), u32_at(eocd + 16));
        let mut names = Vec::new();
        for _ in 0..entries {
            assert_eq!(&d[at..at + 4], b"PK\x01\x02", "{name}: central directory entry");
            let (nl, xl, cl) = (u16_at(at + 28), u16_at(at + 30), u16_at(at + 32));
            names.push(String::from_utf8_lossy(&d[at + 46..at + 46 + nl]).to_string());
            at += 46 + nl + xl + cl;
        }
        assert_eq!(names.iter().filter(|m| m.ends_with(".pvs")).count(), 1, "{name}: {names:?}");
        assert_eq!(names.iter().filter(|m| m.ends_with(".ol")).count(), n, "{name}: {names:?}");
    };
    for (name, s) in &parts {
        let path = dir.join(format!("{}.pvz", name.replace(' ', "-")));
        write_pvz(&path, &[ExportShape { shape: s.clone(), name: Some((*name).into()), color: None }], LengthUnit::Meter).unwrap_or_else(|e| panic!("{name}: {e}"));
        check(&path, 1, name);
    }
    let three: Vec<ExportShape> = parts.iter().take(3).map(|(n, s)| ExportShape { shape: s.clone(), name: Some((*n).into()), color: None }).collect();
    let path = dir.join("three.pvz");
    write_pvz(&path, &three, LengthUnit::Meter).unwrap();
    check(&path, 3, "three");
    let _ = std::fs::remove_dir_all(&dir);
}

/// PVZ import (Onshape imports PVZ as meshes only, live G22/G23): our own
/// files read back as closed mesh solids within tessellation error of the
/// exact volume; Onshape's samples (outside the repo, skipped when absent)
/// give Onshape's own mesh volumes, and the loft whose face meshes leave
/// gaps stays open.
#[test]
#[ignore = "T2: export round trips (platform last digits; 36 s)"]
fn pvz_import() {
    let dir = std::env::temp_dir().join(format!("noshape-pvz-import-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for (name, s) in xt_parts() {
        let path = dir.join(format!("{}.pvz", name.replace(' ', "-")));
        write_pvz(&path, &[ExportShape { shape: s.clone(), name: Some(name.into()), color: None }], LengthUnit::Meter).unwrap();
        let back = read_pvz(&path, LengthUnit::Meter).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(back.count(ShapeType::Solid), 1, "{name}: not a closed mesh");
        assert!(back.is_mesh() && back.is_valid(), "{name}");
        assert_eq!(back.count(ShapeType::Edge), 0, "{name}: a mesh body has no edges");
        let (v, want) = (volume_properties(&back).unwrap().mass, volume_properties(&s).unwrap().mass);
        assert!((v - want).abs() < 5e-3 * want, "{name}: {v} vs {want}");
    }
    let _ = std::fs::remove_dir_all(&dir);
    let samples = std::path::Path::new("/root/onshape-research/live/samples");
    let mm3 = |s: &Shape| volume_properties(s).unwrap().mass * 1e9;
    if let Ok(s) = read_pvz(&samples.join("pvz-fillet.pvz"), LengthUnit::Meter) {
        assert_eq!(s.count(ShapeType::Solid), 1);
        assert!((mm3(&s) - 7484.089596687392).abs() < 1e-6, "{}", mm3(&s));
    }
    if let Ok(s) = read_pvz(&samples.join("pvz-shapes.pvz"), LengthUnit::Meter) {
        // Cone, Loft, Torus, Cylinder; the loft is 2 open pieces (Onshape G27:
        // a closed composite of 2 surfaces).
        let solids: Vec<f64> = s.solids().iter().map(mm3).collect();
        assert_eq!(solids.len(), 3, "{solids:?}");
        for want in [1569.176626930149, 1770.239206699627, 9407.77811303131] {
            assert!(solids.iter().any(|v| (v - want).abs() < 1e-5), "{want} not in {solids:?}");
        }
        assert_eq!(s.count(ShapeType::Shell), 5);
    }
    if let Ok(s) = read_pvz(&samples.join("pvz-rot-asm.pvz"), LengthUnit::Meter) {
        // Second instance: 90° about z, then +50 mm x.
        let b = s.solids()[1].bounding_box(false).unwrap();
        assert!((b.min - DVec3::new(0.03, 0.0, 0.0)).length() < 1e-6 && (b.max - DVec3::new(0.05, 0.04, 0.01)).length() < 1e-6, "{b:?}");
    }
}

/// Parasolid files written by Onshape (Parasolid 38.1, embedded schemas;
/// samples outside the repo, skipped when absent): the G24 corner-blend parts
/// import as one solid of 18 faces whose rolling-ball corners (BLENDED_EDGE)
/// give the exact volume of that geometry (kernel/tests/blend.rs).
#[test]
#[ignore = "T2: export round trips (platform last digits; 36 s)"]
fn parasolid_from_onshape() {
    let samples = std::path::Path::new("/root/onshape-research/live/samples");
    for (file, exact) in [("g25-b.x_t", 7583.000811), ("g25-f1r4.x_t", 7545.834615), ("g25-a.x_t", 7094.796033)] {
        if !samples.join(file).exists() {
            continue;
        }
        let parts = read_parasolid_parts(&samples.join(file), LengthUnit::Meter).unwrap_or_else(|e| panic!("{file}: {e}"));
        assert_eq!(parts.len(), 1, "{file}");
        let s = &parts[0].shape;
        assert!(s.is_valid(), "{file}: invalid");
        assert_eq!((s.count(ShapeType::Solid), s.count(ShapeType::Face)), (1, 18), "{file}");
        let v = volume_properties(s).unwrap().mass * 1e9;
        assert!((v - exact).abs() < 1e-3, "{file}: {v} vs {exact}");
        assert_eq!(parts[0].name.as_deref(), Some("Part 1"), "{file}");
    }
    // The same parts exported at every Parasolid version 25.0-38.1 (no embedded
    // schema: each version's own layouts, live G36).
    let mut n = 0;
    for e in std::fs::read_dir(samples).into_iter().flatten().flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        let exact = if name.starts_with("g36-g24b-v") { 7583.000811 } else if name.starts_with("g36-pvzfillet-v") { 7495.362698 } else { continue };
        let parts = read_parasolid_parts(&e.path(), LengthUnit::Meter).unwrap_or_else(|err| panic!("{name}: {err}"));
        let v = volume_properties(&parts[0].shape).unwrap().mass * 1e9;
        assert!(parts[0].shape.is_valid() && (v - exact).abs() < 1e-3, "{name}: {v} vs {exact}");
        n += 1;
    }
    assert!(n == 0 || n == 50, "{n} G36 files");
    // Onshape's own Parasolid of its fluxpad import (live G38b): cliff-edge
    // blends, against Onshape's mass properties of it (G38).
    if samples.join("g38b-fluxpad_102_STEM.x_t").exists() {
        let parts = read_parasolid_parts(&samples.join("g38b-fluxpad_102_STEM.x_t"), LengthUnit::Meter).unwrap();
        let s = &parts[0].shape;
        assert!(s.is_valid(), "fluxpad: invalid");
        assert_eq!((s.count(ShapeType::Solid), s.count(ShapeType::Face)), (1, 167), "fluxpad");
        let v = volume_properties(s).unwrap().mass * 1e9;
        assert!((v - 255.7688571778665).abs() < 5e-3, "fluxpad: {v}");
    }
    // Onshape's cone (r 6 mm, apex 10 mm up): the axis points the way it widens.
    if samples.join("onshape-cone.x_t").exists() {
        let parts = read_parasolid_parts(&samples.join("onshape-cone.x_t"), LengthUnit::Meter).unwrap();
        let v = volume_properties(&parts[0].shape).unwrap().mass * 1e9;
        assert!(parts[0].shape.is_valid() && (v - 120.0 * std::f64::consts::PI).abs() < 1e-6, "cone: {v}");
    }
}

/// Onshape's own Parasolid of three more G38 imports (live G38b) against
/// Onshape's mass properties of them, which are approximate: inside the
/// bounds Onshape reports with each value (live G38c). The bracket is all
/// planes and cylinders: its exact volume (a 40 x 100 x 5 plate, two r 3.5
/// holes, r 2 top fillets mitred at the corners) is checked as well.
#[test]
#[ignore = "T2: export round trips (platform last digits; 36 s)"]
fn parasolid_from_onshape_mass_bounds() {
    let samples = std::path::Path::new("/root/onshape-research/live/samples");
    for (file, faces, lo, hi) in [
        // Thread faces; its 8 helix edges are tolerant (SP-curves only).
        ("g38b-mcmaster-91274A118.x_t", 25, 248.20339711199576, 260.6041624549888),
        ("g38b-psas_Bell_Crank.x_t", 27, 2050.432593287002, 2051.240461945237),
        ("g38b-ktane_Bracket.x_t", 12, 19370.14999307919, 19387.54422567558),
    ] {
        if !samples.join(file).exists() {
            continue;
        }
        let parts = read_parasolid_parts(&samples.join(file), LengthUnit::Meter).unwrap_or_else(|e| panic!("{file}: {e}"));
        assert_eq!(parts.len(), 1, "{file}");
        let s = &parts[0].shape;
        assert!(s.is_valid(), "{file}: invalid");
        assert_eq!((s.count(ShapeType::Solid), s.count(ShapeType::Face)), (1, faces), "{file}");
        let v = volume_properties(s).unwrap().mass * 1e9;
        assert!(v > lo && v < hi, "{file}: {v} outside Onshape's [{lo}, {hi}]");
        if file == "g38b-ktane_Bracket.x_t" {
            let r = 2.0f64;
            let exact = 20000.0 - 2.0 * std::f64::consts::PI * 3.5 * 3.5 * 5.0 - (r * r - std::f64::consts::PI * r * r / 4.0) * 280.0
                + 4.0 * 2.0 * r.powi(3) * (5.0 / 6.0 - std::f64::consts::PI / 4.0);
            assert!((v - exact).abs() < 1e-4, "bracket: {v} vs {exact}");
        }
        // Tessellation of the tolerant thread edges follows the faces (a 3 mm
        // edge tolerance once gave 451.8 mm² of triangles for 407.6).
        let area = surface_properties(s).unwrap().mass * 1e6;
        let t = tessellate(s, 1e-5, 0.5, true).unwrap();
        let p = |k: u32| DVec3::from_array(t.positions[k as usize].map(f64::from));
        let mesh_area = t.indices.chunks(3).map(|i| (p(i[1]) - p(i[0])).cross(p(i[2]) - p(i[0])).length() / 2.0).sum::<f64>() * 1e6;
        assert!((mesh_area - area).abs() < 2e-3 * area, "{file}: mesh {mesh_area} vs {area}");
    }
}

#[test]
#[ignore = "T2: export round trips (platform last digits; 36 s)"]
fn native_cad_parts() {
    // Native part files (outside the repo; skipped when absent) against
    // Onshape's import of the same file (live G38) or another independent
    // reference. Tolerances: Onshape's mass properties of procedural/spline
    // faces are approximate (±0.03 here).
    let dir = std::path::Path::new("/root/kern-native-samples");
    for (file, faces, want, tol) in [
        ("sldprt/psas_Bell_Crank.SLDPRT", 27, 2050.83652761612, 0.05),       // Onshape (2 blend faces)
        ("sldprt/fluxpad_102_STEM.SLDPRT", 167, 255.7688571778665, 5e-3),   // Onshape (cliff-edge blends)
        ("se_par/ktane_Hull_ModuleHolder_Long.par", 14, 76000.0, 1e-4),     // a 76 cm³ block
        ("nx_prt/nist_d2mi_827-9999-905.prt", 9, 272.2753042096831, 1e-4),   // Onshape = its STEP
        ("nx_prt/nist_mtc_nx_NIST_mtc_crada_polycarb.prt", 35, 17025.66823867636, 1e-3), // Onshape
        // Threaded: 16-turn helical faces (seamed by hand, integrated by volume_props' own rule).
        ("sldprt/nist_mtc_91274A118_COATED_ALLOY_STEEL_SOCKET_HEAD_CAP_SCREW.SLDPRT", 25, 254.4037797834923, 0.03), // Onshape
        ("ipt/respira_M3_washer_9mm.ipt", 6, 50.0172966, 1e-4), // Inventor (ShapeManager); π(4.5²−1.6²)·0.9 ≈ 50.02
    ] {
        let path = dir.join(file);
        if !path.exists() {
            continue;
        }
        let parts = read_native_parts(&path, LengthUnit::Meter).unwrap_or_else(|e| panic!("{file}: {e}"));
        let solids: Vec<_> = parts.iter().filter(|p| p.shape.count(ShapeType::Face) > 0).collect();
        assert_eq!(solids.len(), 1, "{file}");
        let s = &solids[0].shape;
        assert!(s.is_valid(), "{file}: invalid");
        assert_eq!((s.count(ShapeType::Solid), s.count(ShapeType::Face)), (1, faces), "{file}");
        let v = volume_properties(s).unwrap().mass * 1e9;
        assert!((v - want).abs() < tol, "{file}: {v} vs {want}");
        // The part's colour as Onshape imports it (live G38).
        let rgb = |c: [f32; 3]| c.map(|v| (v * 255.0).round() as u8);
        let colour = [
            ("sldprt/psas_Bell_Crank", [229, 234, 237]),
            ("sldprt/fluxpad", [255, 241, 135]),
            ("sldprt/nist_mtc_91274A118", [255, 242, 232]),
        ]
        .iter()
        .find(|(f, _)| file.starts_with(f))
        .map(|(_, c)| *c);
        if let Some(want) = colour {
            assert_eq!(solids[0].color.map(rgb), Some(want), "{file}: colour");
        }
        // SolidWorks curves come from the configuration's ghost partition
        // (live G38d: the McMaster screw's "Curve 1", a 228.335439 mm helix).
        let curves: Vec<_> = parts.iter().filter(|p| p.shape.count(ShapeType::Face) == 0).collect();
        if file.contains("91274A118") {
            assert_eq!(curves.len(), 1, "{file}");
            assert_eq!(curves[0].shape.count(ShapeType::Edge), 1);
            let l = linear_properties(&curves[0].shape).unwrap().mass * 1e3;
            assert!((l - 228.335439).abs() < 1e-3, "helix {l}");
        } else if file.starts_with("sldprt/") {
            assert!(curves.is_empty(), "{file}");
        }
    }
}

/// SOLIDWORKS assemblies (samples outside the repo, skipped when absent):
/// the component tree of the active configuration with each component's
/// file, configuration and placement.
#[test]
#[ignore = "T2: export round trips (platform last digits; 36 s)"]
fn native_assembly_tree() {
    let dir = std::path::Path::new("/root/kern-native-samples");
    let shaft = dir.join("sldasm/psas_Shaft_With_Plate_Asm.SLDASM");
    if shaft.exists() {
        let c = read_native_assembly(&shaft).unwrap();
        let names: Vec<_> = c.iter().map(|x| (x.name.as_str(), x.number, x.file.as_str(), x.parent)).collect();
        assert_eq!(names, [("Canard_Shaft_Plate", 1, "Canard_Shaft_Plate.SLDPRT", None), ("Shaft", 1, "Shaft.SLDPRT", None)]);
        // The shaft's axis (its z) along the assembly's +y; its box then
        // matches the assembly's (y −46.93..6.23 mm with the plate).
        let s = &c[1];
        assert!((s.transform.transform_vector3(DVec3::Z) - DVec3::Y).length() < 1e-12, "{:?}", s.transform);
        assert!((s.transform.w_axis.truncate() - DVec3::new(0.000107227874821092, -0.04438756908378874, -0.00156579007000346)).length() < 1e-15);
    }
    let crada = dir.join("sldasm/nist_mtc_crada_assembly_rev-D.SLDASM");
    if crada.exists() {
        let c = read_native_assembly(&crada).unwrap();
        assert_eq!(c.len(), 36);
        assert!(c.iter().all(|x| x.parent.is_none() && !x.assembly && !x.suppressed));
        // Every nut on a cap screw's axis (as the same assembly's JT has them).
        let centre = |file: &str, config: Option<i32>| {
            let parts = read_native_parts_configuration(&dir.join(file), LengthUnit::Meter, config).unwrap();
            let b = parts.iter().find(|p| p.shape.count(ShapeType::Face) > 0).unwrap().shape.bounding_box(true).unwrap();
            (b.min + b.max) * 0.5
        };
        let nut = c.iter().find(|x| x.name.contains("HEX NUT")).unwrap();
        let screw = c.iter().find(|x| x.name.contains("91274A118")).unwrap();
        let nut_c = centre("sldprt/nist_mtc_90591A141_ZINC_PLATED_STEEL_HEX_NUT.SLDPRT", nut.configuration_id);
        let screw_c = centre("sldprt/nist_mtc_91274A118_COATED_ALLOY_STEEL_SOCKET_HEAD_CAP_SCREW.SLDPRT", screw.configuration_id);
        let screws: Vec<DVec3> = c.iter().filter(|x| x.name.contains("91274A118")).map(|x| x.transform.transform_point3(screw_c)).collect();
        let nuts: Vec<DVec3> = c.iter().filter(|x| x.name.contains("HEX NUT")).map(|x| x.transform.transform_point3(nut_c)).collect();
        assert_eq!((nuts.len(), screws.len()), (10, 14));
        for n in &nuts {
            assert!(screws.iter().any(|s| (s.x - n.x).hypot(s.y - n.y) < 6e-4), "a nut off every screw at {n:?}");
        }
    }
}

/// Native parts' appearance as Onshape imports it (live G38/G38e): the part
/// colour (and opacity) from the file; no face of its own colour.
#[test]
#[ignore = "T2: export round trips (platform last digits; 36 s)"]
fn native_part_appearances() {
    let dir = std::path::Path::new("/root/kern-native-samples");
    for (file, rgb, alpha) in [
        ("sldprt/nist_mtc_91304A112_BLUE_FLAT-HEAD_SOCKET_CAP_SCREW.SLDPRT", [255, 242, 232], 255),
        ("sldprt/fluxpad_101_HOUSING.SLDPRT", [71, 227, 255], 255),
        ("sldprt/nist_mtc_crada_polycarb-window.SLDPRT", [255, 255, 255], 223),
        ("se_par/ktane_Hull_Left.par", [129, 136, 140], 255),
        ("se_par/ktane_LedStrip.par", [129, 136, 140], 255),
        ("se_par/ktane_Bracket.par", [129, 136, 140], 255),
    ] {
        let path = dir.join(file);
        if !path.exists() {
            continue;
        }
        let parts = read_native_parts(&path, LengthUnit::Meter).unwrap_or_else(|e| panic!("{file}: {e}"));
        let solid = parts.iter().find(|p| p.shape.count(ShapeType::Face) > 0).unwrap();
        assert_eq!(solid.color.map(|c| c.map(|v| (v * 255.0).round() as u8)), Some(rgb), "{file}");
        assert_eq!((solid.opacity.unwrap_or(1.0) * 255.0).round() as u8, alpha, "{file}");
        assert!(parts.iter().all(|p| p.face_colors.is_empty()), "{file}: face colours");
        // The blue screw's thread helix ("Curve 1", 179.7916 mm).
        if file.contains("91304A112") {
            let curves: Vec<_> = parts.iter().filter(|p| p.shape.count(ShapeType::Face) == 0).collect();
            assert_eq!(curves.len(), 1);
            let l = linear_properties(&curves[0].shape).unwrap().mass * 1e3;
            assert!((l - 179.7916).abs() < 1e-3, "{l}");
        }
    }
}

/// Live G47 (qa4 F26): a Part Studio's STEP has one independent product per
/// part (last part first), no assembly, no "SOLID" product, Onshape's header,
/// metres, and each part's name on its product, shape representation, solid
/// and context; entities grouped by type from #10.
#[test]
#[ignore = "T2: export round trips (platform last digits; 36 s)"]
fn step_layout_like_onshape() {
    let cube = make_box_corners(DVec3::ZERO, DVec3::splat(0.01)).unwrap();
    // Parts 2 and 3 share Part 1's geometry, moved (as a Part pattern makes them).
    let parts: Vec<ExportShape> = [(0.0, [0.6157, 0.8118, 0.9294]), (0.02, [0.6471, 0.6471, 0.6471]), (0.04, [0.2314, 0.3804, 0.7059])]
        .iter()
        .enumerate()
        .map(|(i, (x, c))| ExportShape { shape: if *x == 0.0 { cube.clone() } else { cube.translated(DVec3::new(*x, 0.0, 0.0)).unwrap() }, name: Some(format!("Part {}", i + 1)), color: Some(*c) })
        .collect();
    let dir = std::env::temp_dir().join(format!("noshape-g47-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("g47 3parts.step");
    write_step_with(&path, &parts, LengthUnit::Meter, &StepOptions { schema: StepSchema::Ap242, file_unit: None, pmi: Vec::new() }).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    let count = |s: &str| text.matches(s).count();
    assert!(text.starts_with("ISO-10303-21;\nHEADER;\n"));
    assert!(text.contains("FILE_DESCRIPTION(\n/* description */ ('STEP AP242 Edition 2',"), "{text}");
    assert!(text.contains("/* name */ 'g47 3parts',"));
    assert!(text.contains("FILE_SCHEMA (('AP242_MANAGED_MODEL_BASED_3D_ENGINEERING_MIM_LF { 1 0 10303 442 3 1 4 }'));"));
    assert!(!text.contains("Open CASCADE"));
    assert!(text.contains("\nDATA;\n#10=SHAPE_REPRESENTATION_RELATIONSHIP('','',"), "{}", &text[..2000.min(text.len())]);
    assert_eq!(count("=PRODUCT("), 3);
    assert_eq!(count("'SOLID'"), 0, "no SOLID product");
    assert_eq!(count("NEXT_ASSEMBLY_USAGE_OCCURRENCE") + count("CONTEXT_DEPENDENT_SHAPE_REPRESENTATION") + count("ITEM_DEFINED_TRANSFORMATION"), 0);
    assert_eq!(count("PCURVE"), 0);
    for i in 1..=3 {
        let n = format!("Part {i}");
        assert!(text.contains(&format!("=PRODUCT('{n}','{n}','{n}',(#")), "{n}");
        assert!(text.contains(&format!("=SHAPE_REPRESENTATION('{n}',(#")), "{n}");
        assert!(text.contains(&format!("=MANIFOLD_SOLID_BREP('{n}',#")), "{n}");
        assert!(text.contains(&format!("REPRESENTATION_CONTEXT('{n}','TOP_LEVEL_ASSEMBLY_PART')")), "{n}");
    }
    // Last part first.
    assert!(text.find("PRODUCT('Part 3'").unwrap() < text.find("PRODUCT('Part 1'").unwrap());
    assert_eq!(count("SI_UNIT($,.METRE.)"), 3);
    assert_eq!(count("UNCERTAINTY_MEASURE_WITH_UNIT(LENGTH_MEASURE(1.E-8),"), 3);
    assert!(text.contains("COLOUR_RGB('',0.231372549019608,0.380392156862745,0.705882352941177);"), "colours in 1/255 steps");
    assert_eq!(count("=ADVANCED_FACE("), 18);
    assert_eq!(count("=EDGE_CURVE("), 36);
    assert_eq!(count("=VERTEX_POINT("), 24);
    // Grouped by type: the order of first appearance follows Onshape's.
    let firsts: Vec<usize> = ["=SHAPE_REPRESENTATION_RELATIONSHIP(", "=ADVANCED_BREP_SHAPE_REPRESENTATION(", "=ORIENTED_EDGE(", "=EDGE_CURVE(", "=VERTEX_POINT(", "=LINE(", "=VECTOR(", "=EDGE_LOOP(", "=PLANE(", "=ADVANCED_FACE(", "=CLOSED_SHELL(", "=STYLED_ITEM(", "=COLOUR_RGB(", "=MANIFOLD_SOLID_BREP(", "=SHAPE_DEFINITION_REPRESENTATION(", "=SHAPE_REPRESENTATION(", "=AXIS2_PLACEMENT_3D(", "=DIRECTION(", "=CARTESIAN_POINT(", "=MECHANICAL_DESIGN", "=PRODUCT_DEFINITION_SHAPE(", "=PRODUCT(", "=APPLICATION_CONTEXT("]
        .iter()
        .map(|t| text.find(t).unwrap_or_else(|| panic!("{t}")))
        .collect();
    assert!(firsts.windows(2).all(|w| w[0] < w[1]), "type order");
    let back = read_step(&path, LengthUnit::Meter).unwrap();
    assert_eq!(back.len(), 3);
    let names: Vec<_> = back.iter().map(|b| b.name.clone().unwrap_or_default()).collect();
    assert_eq!(names, ["Part 3", "Part 2", "Part 1"]);
    for b in &back {
        assert!((volume_properties(&b.shape).unwrap().mass - 1e-6).abs() < 1e-15);
    }
    let xs: Vec<f64> = back.iter().map(|b| b.shape.bounding_box(false).unwrap().min.x).collect();
    assert!((xs[0] - 0.04).abs() < 1e-6 && (xs[1] - 0.02).abs() < 1e-6 && xs[2].abs() < 1e-6, "{xs:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Live G48: a part with a void (BREP_WITH_VOIDS) written after a plain cube:
/// the types only it brings are numbered first (Onshape numbers the type it
/// created last first); AP214 / AP203 / inch variants.
#[test]
#[ignore = "T2: export round trips (platform last digits; 36 s)"]
fn step_layout_variants_like_onshape() {
    let cube = make_box_corners(DVec3::new(0.06, 0.0, 0.0), DVec3::new(0.07, 0.01, 0.01)).unwrap();
    let block = make_box_corners(DVec3::ZERO, DVec3::new(0.04, 0.03, 0.02)).unwrap();
    let void = make_sphere(&Plane::from_normal(DVec3::new(0.02, 0.015, 0.01), DVec3::Z), 0.003, std::f64::consts::TAU).unwrap();
    let holed = cut(&block, &void).unwrap().shape;
    let dir = std::env::temp_dir().join(format!("noshape-g48-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let parts = [ExportShape { shape: holed.clone(), name: Some("Part 1".into()), color: None }, ExportShape { shape: cube.clone(), name: Some("Part 2".into()), color: None }];
    let path = dir.join("g48 two.step");
    write_step_with(&path, &parts, LengthUnit::Meter, &StepOptions::default()).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    let at = |t: &str| text.find(t).unwrap_or_else(|| panic!("{t}"));
    assert!(text.contains("=BREP_WITH_VOIDS('Part 1',#"));
    assert!(text.contains("=MANIFOLD_SOLID_BREP('Part 2',#"));
    // Here the sphere only bounds the void, so its type is created after the
    // void's shell (in G48 it came first, from a hemisphere on the outside).
    assert!(at("=SPHERICAL_SURFACE(") < at("=ORIENTED_CLOSED_SHELL("));
    assert!(at("=ORIENTED_CLOSED_SHELL(") < at("=BREP_WITH_VOIDS("));
    assert!(at("=BREP_WITH_VOIDS(") < at("=SHAPE_REPRESENTATION_RELATIONSHIP("));
    assert!(at("=SHAPE_REPRESENTATION_RELATIONSHIP(") < at("=MANIFOLD_SOLID_BREP("));
    let back = read_step(&path, LengthUnit::Meter).unwrap();
    let v: f64 = back.iter().map(|b| volume_properties(&b.shape).unwrap().mass).sum();
    let want = volume_properties(&holed).unwrap().mass + 1e-6;
    assert!((v - want).abs() < 1e-12, "{v} vs {want}");

    let one = [ExportShape { shape: cube.clone(), name: Some("Part 1".into()), color: Some([0.6157, 0.8118, 0.9294]) }];
    let p214 = dir.join("a214.step");
    write_step_with(&p214, &one, LengthUnit::Meter, &StepOptions { schema: StepSchema::Ap214, file_unit: None, pmi: Vec::new() }).unwrap();
    let t = std::fs::read_to_string(&p214).unwrap();
    assert!(t.contains("/* description */ ('STEP AP214',\n'CAx-IF Rec.Pracs.---User Defined Attributes---1.5---2016-08-15'),"));
    assert!(t.contains("FILE_SCHEMA (('AUTOMOTIVE_DESIGN {1 0 10303 214 3 1 1}'));"));
    assert!(t.contains("\nDATA;\n#10=PROPERTY_DEFINITION_REPRESENTATION(#14,#12);\n"), "{}", &t[..1500]);
    assert!(t.contains("=VALUE_REPRESENTATION_ITEM('number of annotations',COUNT_MEASURE(0.));"));
    assert!(t.contains("=APPLICATION_CONTEXT(\n'core data for automotive mechanical design processes');"));
    let p203 = dir.join("a203.step");
    write_step_with(&p203, &one, LengthUnit::Meter, &StepOptions { schema: StepSchema::Ap203, file_unit: None, pmi: Vec::new() }).unwrap();
    let t = std::fs::read_to_string(&p203).unwrap();
    assert!(t.contains("/* description */ ('STEP AP203'),"));
    assert!(t.contains("FILE_SCHEMA (('CONFIG_CONTROL_DESIGN'));"));
    assert!(t.contains("=DESIGN_CONTEXT('',#") && t.contains("=MECHANICAL_CONTEXT('',#"));
    assert!(t.contains("=APPLICATION_CONTEXT(\n'configuration controlled 3D designs of mechanical parts and assemblie\ns');"));
    let pin = dir.join("inch.step");
    write_step_with(&pin, &one, LengthUnit::Meter, &StepOptions { schema: StepSchema::Ap242, file_unit: Some(LengthUnit::Inch), pmi: Vec::new() }).unwrap();
    let t = std::fs::read_to_string(&pin).unwrap();
    assert!(t.contains("CONVERSION_BASED_UNIT('INCH',#"));
    assert!(t.contains("=UNCERTAINTY_MEASURE_WITH_UNIT(LENGTH_MEASURE(3.93700787401575E-7),"), "{t}");
    assert!(t.contains("SI_UNIT(.MILLI.,.METRE.)"));
    for b in read_step(&pin, LengthUnit::Meter).unwrap() {
        assert!((volume_properties(&b.shape).unwrap().mass - 1e-6).abs() < 1e-14);
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// Writes the G47 three-cube Part Studio as STEP to $NOSHAPE_STEP_OUT (for a
/// line-by-line comparison with Onshape's samples/g47-3parts.step):
/// `cargo test -p noshape-kernel --test all export::write_g47_step -- --ignored`.
#[test]
#[ignore]
fn write_g47_step() {
    let Some(dir) = std::env::var_os("NOSHAPE_STEP_OUT") else { return };
    let colors = [[0.6157, 0.8118, 0.9294], [0.6471, 0.6471, 0.6471], [0.2314, 0.3804, 0.7059]];
    let parts: Vec<ExportShape> = (0..3).map(|i| ExportShape { shape: make_box_corners(DVec3::new(0.02 * i as f64, 0.0, 0.0), DVec3::new(0.02 * i as f64 + 0.01, 0.01, 0.01)).unwrap(), name: Some(format!("Part {}", i + 1)), color: Some(colors[i]) }).collect();
    write_step_with(&std::path::Path::new(&dir).join("g47 3parts.step"), &parts, LengthUnit::Meter, &StepOptions { schema: StepSchema::Ap242, file_unit: None, pmi: Vec::new() }).unwrap();
}

/// Prints the volume and faces (kind, area mm², box mm) of the parts of the
/// Parasolid file $NOSHAPE_XT (comparing Onshape's bodies with ours):
/// `cargo test -p noshape-kernel --test all export::report_xt -- --ignored --nocapture`.
#[test]
#[ignore]
fn report_xt() {
    let Some(path) = std::env::var_os("NOSHAPE_XT") else { return };
    for p in read_parasolid_parts(std::path::Path::new(&path), LengthUnit::Meter).unwrap() {
        eprintln!("{:?}: {:.6} mm³, {} faces", p.name, volume_properties(&p.shape).unwrap().mass * 1e9, p.shape.count(ShapeType::Face));
        for f in p.shape.faces() {
            let kind = f.face_surface().map(|s| format!("{s:?}").split(['(', ' ', '{']).next().unwrap_or("?").to_string()).unwrap_or_default();
            let b = f.bounding_box(false).unwrap();
            eprintln!("    {kind} {:.6} {:.3?}..{:.3?}", f.area().unwrap() * 1e6, (b.min * 1e3).to_array(), (b.max * 1e3).to_array());
            if f.area().unwrap() < 0.0 || (b.max - b.min).length() > 1.0 {
                for e in f.edges() {
                    let i = e.edge_info().unwrap();
                    let eb = e.bounding_box(false).unwrap();
                    let kind = format!("{:?}", i.curve).split(['(', ' ', '{']).next().unwrap_or("?").to_string();
                    eprintln!("        edge {kind} len {:.4} {:.3?}..{:.3?} from {:.3?} to {:.3?}", e.length().unwrap() * 1e3, (eb.min * 1e3).to_array(), (eb.max * 1e3).to_array(), (i.start * 1e3).to_array(), (i.end * 1e3).to_array());
                }
            }
        }
    }
}

/// qa5 IM4 / IM5 (samples outside the repo, skipped when absent): Onshape's
/// shell of a chamfer blend patch (its inner face an offset of a B-spline
/// whose boundary collapses to a point, trimmed intersection edges on it;
/// live 6052.697 mm³, ours 6053.32) and eight circle wire bodies (live
/// G38d) import, nothing refused.
#[test]
#[ignore = "T2: export round trips (platform last digits; 36 s)"]
fn parasolid_offset_patch_and_wire_circles_from_onshape() {
    let samples = std::path::Path::new("/root/onshape-research/live/samples");
    let shell = samples.join("g50-g46-shell1.x_t");
    if shell.exists() {
        let parts = read_parasolid_parts(&shell, LengthUnit::Meter).unwrap_or_else(|e| panic!("g50-g46-shell1: {e}"));
        assert_eq!(parts.len(), 1);
        let s = &parts[0].shape;
        assert!(s.is_valid(), "invalid");
        assert_eq!(s.count(ShapeType::Face), 21);
        let v = volume_properties(s).unwrap().mass * 1e9;
        // Live's estimate 6052.76072 in its band 6048.911 – 6056.610.
        assert!((v - 6052.76).abs() < 0.2, "{v}");
        // Per face (live IM2b, Geom dump evArea): the faces at the shell's
        // tolerant vertices count the slivers between their edges' ends, as
        // Parasolid's do (F18 82.8478429, F11 46).
        let areas: Vec<f64> = (0..21).map(|i| s.sub_shape(ShapeType::Face, i).unwrap().area().unwrap() * 1e6).collect();
        for want in [82.8478429, 46.0, 145.4373552, 224.4762898, 130.2285065, 1211.2304474, 691.2303132] {
            assert!(areas.iter().any(|a| (a - want).abs() < 2e-6 * want), "no face of {want} mm²: {areas:?}");
        }
        let b = s.bounding_box(false).unwrap();
        assert!((b.max - b.min).length() < 0.07, "{:?}", b.max - b.min);
    }
    let curves = samples.join("g38d-polycarb-curves.x_t");
    if curves.exists() {
        let parts = read_parasolid_parts(&curves, LengthUnit::Meter).unwrap_or_else(|e| panic!("g38d-polycarb-curves: {e}"));
        assert_eq!(parts.len(), 8);
        for p in &parts {
            assert_eq!((p.shape.count(ShapeType::Edge), p.shape.count(ShapeType::Face)), (1, 0), "{:?}", p.name);
        }
    }
}

#[test]
#[ignore = "T2: export round trips (platform last digits; 36 s)"]
fn binary_stl_header_is_zeros() {
    let path = std::env::temp_dir().join(format!("noshape-stl-header-{}.stl", std::process::id()));
    write_stl(&part(), &path, 0.0005, StlFormat::Binary).unwrap();
    let bytes = std::fs::read(&path).unwrap();
    std::fs::remove_file(&path).ok();
    assert!(bytes[..80].iter().all(|&b| b == 0), "header {:?}", &bytes[..24]);
    let n = u32::from_le_bytes(bytes[80..84].try_into().unwrap()) as usize;
    assert!(n > 0 && bytes.len() == 84 + 50 * n);
}

/// A spline profile revolved to the axis (a pole: a degenerated edge with no
/// 3D curve) writes as Parasolid: OCCT's modifier asked SweptToElementary
/// about it with a null face and crashed (parts of the NIST assembly). Its
/// spun face goes as a B-spline (split, as other closed ones).
#[test]
fn parasolid_spun_spline_to_the_axis() {
    let mm = 1e-3;
    let prof = make_bspline(&[DVec3::new(8.0, 0.0, 0.0) * mm, DVec3::new(9.0, 0.0, 5.0) * mm, DVec3::new(4.0, 0.0, 9.0) * mm, DVec3::new(0.0, 0.0, 10.0) * mm], None, &[0.0, 1.0], &[4, 4], 3, false).unwrap();
    let w = make_wire(&[make_line(DVec3::ZERO, DVec3::new(8.0, 0.0, 0.0) * mm).unwrap(), prof, make_line(DVec3::new(0.0, 0.0, 10.0) * mm, DVec3::ZERO).unwrap()]).unwrap();
    let dome = revolve(&make_face_from_wire(&w, true).unwrap(), DVec3::ZERO, DVec3::Z, std::f64::consts::TAU).unwrap().shape;
    let want = volume_properties(&dome).unwrap().mass;
    let path = std::env::temp_dir().join(format!("noshape-xt-dome-{}.x_t", std::process::id()));
    write_parasolid(&path, std::slice::from_ref(&dome), LengthUnit::Meter, false).unwrap();
    let back = read_parasolid(&path, LengthUnit::Meter).unwrap();
    assert!(back.is_valid());
    let _ = std::fs::remove_file(&path);
    let v = volume_properties(&back).unwrap().mass;
    assert!((v - want).abs() < 1e-6 * want, "{v} vs {want}");
}

/// A Parasolid assembly (live's export of the NIST MTC CRADA assembly,
/// tests/perf): the top assembly's name, its 8 distinct bodies and their 36
/// placed instances, as live imports it (behaviours "dx speed: … NIST import").
#[test]
#[ignore = "T2: reads a 3.5 MB transmit file (a few seconds)"]
fn parasolid_assembly_tree() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/perf/nist-mtc-crada-assembly.x_t");
    let (root, parts, instances) = read_parasolid_assembly(&path, LengthUnit::Millimeter).unwrap().expect("an assembly");
    assert_eq!(root, "nist_mtc_crada_assembly_rev-D");
    assert_eq!(parts.len(), 8, "{:?}", parts.iter().map(|p| p.name.clone()).collect::<Vec<_>>());
    assert_eq!(instances.len(), 36);
    assert!(parts.iter().all(|p| p.shape.count(ShapeType::Solid) == 1 && p.shape.is_valid()));
    assert!(parts.iter().any(|p| p.name.as_deref() == Some("nist_mtc_crada_box_rev-D-1")), "{:?}", parts.iter().map(|p| p.name.clone()).collect::<Vec<_>>());
    // Bodies alone: no tree.
    let dir = std::env::temp_dir().join(format!("noshape-xt-tree-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let one = dir.join("block.x_t");
    write_parasolid(&one, &[make_box_corners(DVec3::ZERO, DVec3::splat(0.01)).unwrap()], LengthUnit::Meter, false).unwrap();
    assert!(read_parasolid_assembly(&one, LengthUnit::Meter).unwrap().is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

/// PVZ of a through hole cut by noshape's own Boolean (PVZ-RUST-BOOL): its seam's curves at u = 0 and 2π and
/// its circles on the cylinder's own x axis, as OCCT's (a seam a rounding below 0 failed the writer's trimming).
#[test]
fn pvz_of_a_hole_cut_by_the_rust_boolean() {
    set_rust_boolean(Some(true));
    let s = part();
    assert_eq!(last_boolean_path(), "rust");
    set_rust_boolean(None);
    let path = std::env::temp_dir().join(format!("noshape-pvz-rust-{}.pvz", std::process::id()));
    write_pvz(&path, &[ExportShape { shape: s, name: Some("part".into()), color: None }], LengthUnit::Meter).unwrap();
    let _ = std::fs::remove_file(&path);
}

/// MBD in STEP AP242 as Onshape writes it (live 1746, CAx-IF practices),
/// read back by an independent reader (OCCT's XDE): a datum on a face and a
/// toleranced distance between two faces of a 10 × 20 × 30 mm block, each
/// with its tessellated presentation.
#[test]
fn step_ap242_pmi_round_trips() {
    let block = make_box_corners(DVec3::ZERO, DVec3::new(0.01, 0.02, 0.03)).unwrap();
    let look = |label: &str| Some(PmiPresentation { label: label.into(), origin: [0.005, 0.01, 0.03], normal: [0.0, 0.0, 1.0], x: [1.0, 0.0, 0.0], triangles: vec![[0.0, 0.0, 0.03], [0.002, 0.0, 0.03], [0.001, 0.001, 0.03]], lines: vec![[0.0, 0.0, 0.03], [0.01, 0.0, 0.03]] });
    let pmi = PartPmi {
        datums: vec![PmiDatum { faces: vec![5], letter: "A".into(), presentation: look("datum") }],
        dims: vec![PmiDimension { kind: PmiKind::Distance, a: vec![1], b: vec![2], value: 0.01, tolerance: Some((-0.0001, 0.0001)), name: "DISTANCE.1".into(), precision: 3, presentation: look("linear dimension") }],
    };
    let dir = std::env::temp_dir().join(format!("noshape-pmi-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("pmi block.step");
    write_step_with(&path, &[ExportShape { shape: block, name: Some("Block".into()), color: None }], LengthUnit::Meter, &StepOptions { schema: StepSchema::Ap242, file_unit: None, pmi: vec![Some(pmi)] }).unwrap();
    let lines = read_step_pmi(&path).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    // The distance as XDE reads it: a linear distance (type 2) of 10 mm
    // between one face and another, ±0.1 (XDE keeps the lower deviation's
    // size).
    let dim = lines.iter().find(|l| l.starts_with("dim ")).unwrap_or_else(|| panic!("{lines:?}\n{text}"));
    assert_eq!(dim, "dim 2 10 tol 0.1 0.1 faces 1+1");
    for e in [
        "=SHAPE_ASPECT('DISTANCE.1',$,",
        "=DIMENSIONAL_LOCATION('linear distance','DISTANCE.1',",
        "=MEASURE_WITH_UNIT(LENGTH_MEASURE(-0.1),",
        "SI_UNIT(.MILLI.,.METRE.)",
        "POSITIVE_LENGTH_MEASURE(10.)",
        "=PRECISION_QUALIFIER(3);",
        "=TESSELLATED_ANNOTATION_OCCURRENCE('linear dimension',",
        "=DRAUGHTING_CALLOUT('linear dimension',",
        "=COMPLEX_TRIANGULATED_SURFACE_SET('surface set',",
        "=TESSELLATED_CURVE_SET('curve set',",
        "=DATUM('Datum A','Datum A',",
        "=DATUM_FEATURE('Datum_feature A','Datum_feature A',",
        "=DRAUGHTING_CALLOUT('datum',",
    ] {
        assert!(text.contains(e), "{e}\n{text}");
    }
    assert_eq!(text.matches("=DRAUGHTING_MODEL(").count(), 1);
    assert_eq!(text.matches("=DRAUGHTING_MODEL_ITEM_ASSOCIATION(").count(), 4, "the datum feature; the dimension and its two aspects");
    // The datum feature on one face of this product, related to its datum.
    let ids = |pat: &str| text.lines().filter(|l| l.contains(pat)).filter_map(|l| l.split('=').next()).map(str::to_string).collect::<Vec<_>>();
    let (feature, datum) = (ids("=DATUM_FEATURE("), ids("=DATUM('"));
    assert!(text.contains(&format!("=SHAPE_ASPECT_RELATIONSHIP('','',{},{})", feature[0], datum[0])), "{text}");
    assert!(text.lines().any(|l| l.contains("=GEOMETRIC_ITEM_SPECIFIC_USAGE(") && l.contains(&format!(",{},", feature[0]))), "{text}");
    // AP214 carries none (live 1746).
    let ap214 = dir.join("pmi block 214.step");
    let block = make_box_corners(DVec3::ZERO, DVec3::new(0.01, 0.02, 0.03)).unwrap();
    let pmi = PartPmi { datums: vec![PmiDatum { faces: vec![5], letter: "A".into(), presentation: None }], dims: Vec::new() };
    write_step_with(&ap214, &[ExportShape { shape: block, name: Some("Block".into()), color: None }], LengthUnit::Meter, &StepOptions { schema: StepSchema::Ap214, file_unit: None, pmi: vec![Some(pmi)] }).unwrap();
    assert!(!std::fs::read_to_string(&ap214).unwrap().contains("DATUM"));
}

/// Debug aid: the PMI OCCT's XDE reads from `NOSHAPE_PMI_READ`.
#[test]
#[ignore = "debug aid"]
fn dump_step_pmi() {
    let Some(p) = std::env::var_os("NOSHAPE_PMI_READ") else { return };
    println!("{:?}", read_step_pmi(std::path::Path::new(&p)).unwrap());
}

/// Debug aid: the parts, colours and face colours `read_step` gives for
/// `NOSHAPE_STEP_READ`.
#[test]
#[ignore = "debug aid"]
fn dump_step_colours() {
    let Some(p) = std::env::var_os("NOSHAPE_STEP_READ") else { return };
    for s in read_step(std::path::Path::new(&p), LengthUnit::Meter).unwrap() {
        println!("part {:?} colour {:?} face colours {:?}", s.name, s.color, s.face_colors);
    }
}

/// NIST's cap screw, healed on import, written to STEP and read back: each
/// edge writes only the part of its B-spline it uses (the helix was split
/// into 20 edges of one 848-pole curve: 4 MB, now 0.66 MB), and the file
/// declares the part's own tolerance (at Onshape's 1e-8 m readers healed it
/// again at that precision: 20 s, now 0.5 s; the time is T2's
/// `step_reads_the_used_part_of_long_splines_back_quickly`).
#[test]
fn step_writes_the_used_part_of_long_splines() {
    screw_written_and_read_back();
}

/// The same file read back within 5 s (a wall-clock budget: T2, a busy host misses it).
#[test]
#[ignore = "T2: a wall-clock budget"]
fn step_reads_the_used_part_of_long_splines_back_quickly() {
    let read = screw_written_and_read_back();
    assert!(read.as_secs_f64() < 5.0, "read back in {read:?}");
}

/// The cap screw written and read back, checked; the time its reading took.
fn screw_written_and_read_back() -> std::time::Duration {
    let nist = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/perf/nist-mtc-crada-assembly.step");
    let ((_, parts, _), _) = read_step_assembly_nested(&nist, LengthUnit::Millimeter).unwrap().unwrap();
    let screw = parts.into_iter().find(|p| p.name.as_deref().is_some_and(|n| n.starts_with("91274A141"))).expect("the cap screw").shape;
    let want = volume_properties(&screw).unwrap().mass;
    let dir = std::env::temp_dir().join(format!("noshape-compact-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("screw.step");
    write_step(&path, &[ExportShape { shape: screw.clone(), name: Some("Screw".into()), color: None }], LengthUnit::Millimeter).unwrap();
    let size = std::fs::metadata(&path).unwrap().len();
    assert!(size < 1_000_000, "{size} bytes");
    let t = std::time::Instant::now();
    let back = read_step(&path, LengthUnit::Millimeter).unwrap();
    let read = t.elapsed();
    // Healed again on reading (as reading Onshape's file did): within 0.03 %.
    let got = volume_properties(&back[0].shape).unwrap().mass;
    assert!((got - want).abs() < want * 3e-4, "volume {got} vs {want}");
    assert_eq!(back[0].shape.count(ShapeType::Face), screw.count(ShapeType::Face));
    assert!(back[0].shape.is_valid());
    // The document's shape keeps its own curves (the export works on a copy).
    assert!((volume_properties(&screw).unwrap().mass - want).abs() < 1e-9);
    let _ = std::fs::remove_dir_all(&dir);
    read
}

/// NIST's cap screw, healed on import, written as Parasolid: an edge whose
/// refitted curve misses a vertex drops the refit (the writer read the
/// dropped curve's other end and crashed), and the screw reads back whole.
#[test]
fn parasolid_writes_an_imported_thread() {
    let nist = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/perf/nist-mtc-crada-assembly.step");
    let ((_, parts, _), _) = read_step_assembly_nested(&nist, LengthUnit::Meter).unwrap().unwrap();
    let screw = parts.into_iter().find(|p| p.name.as_deref().is_some_and(|n| n.starts_with("91274A141"))).expect("the cap screw").shape;
    let want = volume_properties(&screw).unwrap().mass;
    let path = std::env::temp_dir().join(format!("noshape-xt-screw-{}.x_t", std::process::id()));
    write_parasolid(&path, std::slice::from_ref(&screw), LengthUnit::Meter, false).unwrap();
    let back = read_parasolid(&path, LengthUnit::Meter).unwrap();
    let got = volume_properties(&back).unwrap().mass;
    // (Read back, a tolerant edge takes its free-form fin's curve, as
    // Parasolid measures it: 1.006e-4 off here.)
    assert!((got - want).abs() < want * 1.5e-4, "volume {got} vs {want}");
    assert_eq!(back.count(ShapeType::Face), screw.count(ShapeType::Face));
    let _ = std::fs::remove_file(&path);
}

/// Every NIST part as imported (healed STEP) written to Rhino and PVZ and
/// read back. Rhino (7 of 8 parts failed openNURBS' check): trims a period
/// apart on closed surfaces are shifted to meet, trim ends matched, seam
/// trims put on the domain's edge, and a part still failing is converted
/// again with every closed face split (repaired first where OCCT's NURBS
/// conversion throws, as PVZ): all 8 write, at their volumes. PVZ: a healed edge's range
/// past its curve's ends is trimmed to it, and a shape OCCT's NURBS
/// conversion throws on is repaired first (the screws threw).
#[test]
fn imported_nist_parts_write_to_rhino_and_pvz() {
    let nist = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/perf/nist-mtc-crada-assembly.step");
    let ((_, parts, _), _) = read_step_assembly_nested(&nist, LengthUnit::Meter).unwrap().unwrap();
    let dir = std::env::temp_dir().join(format!("noshape-nist-out-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let (mut wrong, mut rhino_ok) = (Vec::new(), Vec::new());
    // Kernels built without openNURBS (no OPENNURBS_ROOT: agents' dev
    // builds, the main watcher) have no Rhino writer: PVZ only.
    let mut rhino_built = true;
    for p in parts.iter().filter(|p| p.shape.count(ShapeType::Face) > 0) {
        let name = p.name.clone().unwrap_or_default();
        let want = volume_properties(&p.shape).unwrap().mass;
        let shapes = [ExportShape { shape: p.shape.clone(), name: Some(name.clone()), color: None }];
        let path = dir.join("part.3dm");
        match write_rhino(&path, &shapes, LengthUnit::Meter, 8).and_then(|_| read_rhino_parts(&path, LengthUnit::Meter)) {
            Ok(back) => {
                let v = volume_properties(&back[0].shape).unwrap().mass;
                if (v - want).abs() > 1e-3 * want {
                    wrong.push(format!("rhino {name}: volume {v} vs {want}"));
                }
                rhino_ok.push(name.clone());
            }
            Err(e) if e.to_string().contains("needs openNURBS") => rhino_built = false,
            Err(e) => wrong.push(format!("rhino {name}: {e}")),
        }
        let path = dir.join("part.pvz");
        match write_pvz(&path, &shapes, LengthUnit::Meter).and_then(|_| read_pvz(&path, LengthUnit::Meter)) {
            Ok(back) => {
                let v = volume_properties(&back).unwrap().mass;
                // PVZ reads back as its mesh: a thread's 1.7 % under.
                if (v - want).abs() > 3e-2 * want {
                    wrong.push(format!("pvz {name}: volume {v} vs {want}"));
                }
            }
            Err(e) => wrong.push(format!("pvz {name}: {e}")),
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
    assert!(wrong.is_empty(), "{wrong:#?}");
    if !rhino_built {
        return;
    }
    let solids = parts.iter().filter(|p| p.shape.count(ShapeType::Face) > 0).count();
    assert_eq!(rhino_ok.len(), solids, "every part to Rhino: {rhino_ok:?}");
}

/// Live's smooth twisted vase loft (Parasolid from live, XT-PERIODIC-SPLINE-FACE):
/// its side face is one periodic B-surface with no seam edge; it reads as
/// one face (made periodic in OCCT, then seamed), 3 faces in all, live's
/// volume (437833.229 mm³) and valid. It read as 4 faces, 126983 mm³.
#[test]
fn parasolid_periodic_bsurface_face_reads_whole() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/shell");
    let s = read_parasolid(&dir.join("vase-smooth-live-loft.x_t"), LengthUnit::Millimeter).unwrap();
    let v = volume_properties(&s).unwrap().mass;
    assert_eq!(s.count(ShapeType::Face), 3);
    assert!((v - 437833.229).abs() < 437833.229 * 1e-6, "{v}");
    assert!(s.is_valid());
}

/// A tolerant edge between an analytic and a free-form face takes the
/// free-form face's SP-curve (live api 10-24 "IM2b follow-up": the McMaster
/// screw's thread helices are 159.392935 and 159.3450619 mm; the cylinder's
/// fins gave 159.394 / 159.325). The file is live's sample (not in the repo).
#[test]
#[ignore = "T2: live's sample file /root/onshape-research/live/samples (a vendor's part, never in the repo)"]
fn a_thread_helix_is_as_long_as_parasolids() {
    let root = std::env::var("NOSHAPE_LIVE_SAMPLES").unwrap_or_else(|_| "/root/onshape-research/live/samples".into());
    let path = std::path::Path::new(&root).join("g38b-mcmaster-91274A118.x_t");
    if !path.is_file() {
        return;
    }
    let s = read_parasolid(&path, LengthUnit::Millimeter).unwrap();
    // (In mm whichever unit the kernel keeps.)
    let mm = |l: f64| if l < 1.0 { l * 1000.0 } else { l };
    let mut long: Vec<f64> = s.edges().iter().filter_map(|e| e.length().ok()).map(mm).filter(|l| (150.0..170.0).contains(l)).collect();
    long.sort_by(f64::total_cmp);
    assert_eq!(long.len(), 2, "{long:?}");
    assert!((long[0] - 159.3450619).abs() < 2e-3 && (long[1] - 159.392935).abs() < 2e-3, "{long:?}");
}

/// A Parasolid cylinder face between two rings with other loops on it (a fan
/// hub with seven blade roots, live's cf-wiring export): its seam runs in a
/// gap between the roots, so no root edge is cut there; live's counts per
/// part. The file is live's corpus (not in the repo).
#[test]
#[ignore = "T2: live's corpus file /root/parity/models/cf-wiring (other people's work, never in the repo)"]
fn a_hub_seam_cuts_no_blade_root() {
    let root = std::env::var("NOSHAPE_PARITY_MODELS").unwrap_or_else(|_| "/root/parity/models".into());
    let path = std::path::Path::new(&root).join("cf-wiring/imports/6ae9ae35100f83b7021d3619.x_t");
    if !path.is_file() {
        return;
    }
    let s = read_parasolid(&path, LengthUnit::Millimeter).unwrap();
    let mut got: Vec<(usize, usize, usize)> = s
        .solids()
        .iter()
        .map(|p| {
            let t = p.shown_topology().unwrap();
            (p.count(ShapeType::Face), t.edges.iter().filter(|e| **e).count(), t.vertices.iter().filter(|v| **v).count())
        })
        .collect();
    got.sort();
    assert_eq!(got, [(36, 82, 40), (40, 88, 56)]);
}

/// Live's closed sketch spline extruded 10 mm (request 1836: 3 faces, 2
/// edges, 0 vertices; its caps one closed B-curve each, ring edges): written
/// back each cap is still one edge without a vertex (they were split in two,
/// 4 edges / 4 vertices), the volume the same. Also a cylinder: its circles
/// are rings (live 1855).
#[test]
fn parasolid_writes_closed_edges_as_rings() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/shell");
    let spline = read_parasolid(&dir.join("closed-spline-live.x_t"), LengthUnit::Millimeter).unwrap();
    let cylinder = extrude(&make_face_from_wire(&make_wire(&[make_circle(DVec3::ZERO, DVec3::Z, 10.0).unwrap()]).unwrap(), true).unwrap(), DVec3::new(0.0, 0.0, 10.0)).unwrap().shape;
    let out = std::env::temp_dir().join(format!("noshape-xt-rings-{}", std::process::id()));
    std::fs::create_dir_all(&out).unwrap();
    for (name, s) in [("closed-spline", spline), ("cylinder", cylinder)] {
        let shown = |s: &Shape| {
            let t = s.shown_topology().unwrap();
            (s.count(ShapeType::Face), t.edges.iter().filter(|e| **e).count(), t.vertices.iter().filter(|v| **v).count())
        };
        assert_eq!(shown(&s), (3, 2, 0), "{name}: read");
        let path = out.join(format!("{name}.x_t"));
        write_parasolid(&path, std::slice::from_ref(&s), LengthUnit::Millimeter, false).unwrap();
        let back = read_parasolid(&path, LengthUnit::Millimeter).unwrap();
        assert_eq!(shown(&back), (3, 2, 0), "{name}: written back");
        let (a, b) = (volume_properties(&s).unwrap().mass, volume_properties(&back).unwrap().mass);
        assert!((a - b).abs() < 1e-6 * a, "{name}: {a} vs {b}");
        assert!(back.is_valid(), "{name}");
    }
    let _ = std::fs::remove_dir_all(&out);
}

/// Written back, a face going round a periodic B-surface (the vase's side
/// face, as in live's file) or one closed with a tangent seam (a loft between
/// circles) stays one face: no seam, not split. `NOSHAPE_XT_SAMPLES=<dir>`
/// keeps the files.
#[test]
fn parasolid_writes_a_periodic_bsurface_face_whole() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/shell");
    let vase = read_parasolid(&dir.join("vase-smooth-live-loft.x_t"), LengthUnit::Millimeter).unwrap();
    // In millimeters, as the vase read in millimeters (live 1808: written in meters it was 1000x too small).
    let ring = |z: f64, r: f64| make_wire(&[make_circle(DVec3::new(0.0, 0.0, z), DVec3::Z, r).unwrap()]).unwrap();
    let lofted = loft(&[ring(0.0, 10.0), ring(15.0, 6.0), ring(30.0, 9.0)], &LoftOptions { solid: true, ..Default::default() }).unwrap().shape;
    let out = std::env::var_os("NOSHAPE_XT_SAMPLES").map(std::path::PathBuf::from).unwrap_or_else(|| std::env::temp_dir().join(format!("noshape-xt-periodic-{}", std::process::id())));
    std::fs::create_dir_all(&out).unwrap();
    // Live's loft through the same circles (request 1808: 3 faces, 2 edges, 5156.5974 mm³).
    let live = read_parasolid(&dir.join("loft-circles-live.x_t"), LengthUnit::Millimeter).unwrap();
    assert_eq!(live.count(ShapeType::Face), 3);
    for (name, s) in [("vase-smooth", vase), ("loft-circles", lofted), ("loft-circles-live", live)] {
        let want = volume_properties(&s).unwrap().mass;
        if name.starts_with("loft-circles") {
            // Live's loft through the same circles: 5156.5974 mm³ (massproperties, request 1808).
            assert!((want - 5156.5974).abs() < 5.0, "{want}");
        }
        for binary in [false, true] {
            let path = out.join(format!("{name}-noshape.{}", if binary { "x_b" } else { "x_t" }));
            write_parasolid(&path, std::slice::from_ref(&s), LengthUnit::Millimeter, binary).unwrap();
            let back = read_parasolid(&path, LengthUnit::Millimeter).unwrap();
            assert_eq!(back.count(ShapeType::Face), s.count(ShapeType::Face), "{name} binary={binary}");
            assert!(back.is_valid(), "{name} binary={binary}");
            let v = volume_properties(&back).unwrap().mass;
            assert!((v - want).abs() < want * 1e-6, "{name} binary={binary}: {v} vs {want}");
        }
    }
}

/// `NOSHAPE_STEP_READ=<assembly.step> cargo test … export::dump_step_assembly_parts -- --ignored --nocapture`:
/// each part's volume, face / edge counts, validity and BREP size (comparing
/// readers' settings).
#[test]
#[ignore]
fn dump_step_assembly_parts() {
    let Some(p) = std::env::var_os("NOSHAPE_STEP_READ") else { return };
    let path = std::path::Path::new(&p);
    let parts = match read_step_assembly_nested(path, LengthUnit::Millimeter).unwrap() {
        Some(((_, parts, _), _)) => parts,
        None => read_step(path, LengthUnit::Millimeter).unwrap(),
    };
    for s in parts {
        let v = volume_properties(&s.shape).map(|m| m.mass).unwrap_or(f64::NAN);
        if let Some(d) = std::env::var_os("NOSHAPE_STEP_DUMP_DIR") {
            std::fs::write(std::path::Path::new(&d).join(format!("{}.brep", s.name.clone().unwrap_or_default().replace(['/', ' '], "_"))), s.shape.to_brep_bytes().unwrap()).unwrap();
        }
        println!("part {:?}: volume {v:.4} mm3, faces {}, edges {}, valid {}, brep {} KB", s.name, s.shape.count(ShapeType::Face), s.shape.count(ShapeType::Edge), s.shape.is_valid(), s.shape.to_brep_bytes().unwrap().len() / 1000);
    }
}

/// A STEP face's own colour (a STYLED_ITEM on an ADVANCED_FACE; live's
/// g48 export with one face made red) comes in on that face, the parts keep
/// theirs. Read from the part's sub-shape labels (not a document-wide
/// search per face: quadratic, 2.9 s for 2000 parts).
#[test]
fn step_face_colour_reads_from_the_parts_sub_shapes() {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/step/face-colour.step");
    let parts = read_step(&p, LengthUnit::Meter).unwrap();
    assert_eq!(parts.len(), 3);
    let coloured: Vec<_> = parts.iter().filter(|s| !s.face_colors.is_empty()).collect();
    assert_eq!(coloured.len(), 1);
    assert_eq!(coloured[0].face_colors, vec![(0, [1.0, 0.0, 0.0])]);
    assert!(parts.iter().all(|s| s.color.is_some()));
}

/// Live's IGES of a part named "Long bracket part name 1": the 186 label
/// holds "Long bra", a 406 Name property the whole name; live imports it as
/// "Long bra" (request 1818), and so do we.
#[test]
fn iges_from_live_takes_the_label() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/iges/live-long-name.igs");
    let parts = read_iges_parts(&path, LengthUnit::Meter).unwrap();
    let names: Vec<Option<&str>> = parts.iter().map(|p| p.name.as_deref()).collect();
    assert_eq!(names, [Some("Long bra")]);
}

/// Live's IGES of Assembly 1 = Part 1 + Sub (Part 2, Part 3) (request 2511): Sub is a subfigure (308) of two solids
/// placed by one instance (408); a single import gives three parts, each named by its solid's label, as live's.
#[test]
fn iges_subfigure_of_two_solids_is_two_parts() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/iges/asm2.igs");
    let parts = read_iges_parts(&path, LengthUnit::Meter).unwrap();
    let mut names: Vec<Option<&str>> = parts.iter().map(|p| p.name.as_deref()).collect();
    names.sort();
    assert_eq!(names, [Some("Part 1"), Some("Part 2"), Some("Part 3")]);
    assert!(parts.iter().all(|p| p.shape.solids().len() == 1), "one solid each");
}

/// The same subassembly placed twice (studios' asm2-twice.igs: a second 408 50 mm along x): a single import gives each
/// placed solid as a part of its own, Part 1 and Part 2 / Part 3 twice, the second pair 50 mm along x.
#[test]
fn iges_subfigure_placed_twice_is_each_placed_solid() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/iges/asm2-twice.igs");
    let parts = read_iges_parts(&path, LengthUnit::Millimeter).unwrap();
    let mut names: Vec<Option<&str>> = parts.iter().map(|p| p.name.as_deref()).collect();
    names.sort();
    assert_eq!(names, [Some("Part 1"), Some("Part 2"), Some("Part 2"), Some("Part 3"), Some("Part 3")]);
    assert!(parts.iter().all(|p| p.shape.solids().len() == 1), "one solid each");
    let xs = |n: &str| -> Vec<f64> { parts.iter().filter(|p| p.name.as_deref() == Some(n)).map(|p| p.shape.bounding_box(false).unwrap().min.x).collect() };
    for n in ["Part 2", "Part 3"] {
        let x = xs(n);
        assert!((x[0] - x[1]).abs() > 49.99 && (x[0] - x[1]).abs() < 50.01, "{n}: {x:?}");
    }
}

/// An STL's B-spline tube on a grid as live's facetter makes it (cf-hose-routing's swept hoses: even 5° steps round
/// the tube, its volume within 0.1 % of the model's): a Ø 6 mm disc swept along a bent spline at REST's 5°, no chord
/// limit, comes within 0.3 % (BRepMesh's own mesh lost 0.8 % on the hoses).
#[test]
fn stl_bspline_tube_on_a_grid() {
    let pts = [DVec3::ZERO, DVec3::new(0.03, 0.0, 0.012), DVec3::new(0.06, 0.0, -0.012), DVec3::new(0.09, 0.01, 0.0)];
    let path = make_wire(&[make_interpolated_spline(&pts, &[], false).unwrap()]).unwrap();
    let start = path.edges()[0].tangent_at(0.0).unwrap();
    let disc = make_face_from_wire(&make_wire(&[make_circle(DVec3::ZERO, start, 0.003).unwrap()]).unwrap(), true).unwrap();
    let tube = sweep(&disc, &path, &SweepOptions { with_correction: true, ..Default::default() }).unwrap().shape;
    assert!(tube.faces().iter().any(|f| matches!(f.face_surface(), Ok(Surface::BSpline { .. }))), "a B-spline wall");
    let exact = volume_properties(&tube).unwrap().mass;
    let t = tessellate_export(&tube, 100.0 * 2e-6, 10f64.to_radians(), 0.0).unwrap();
    let p = |i: u32| DVec3::from(t.positions[i as usize].map(f64::from));
    let v: f64 = t.indices.chunks(3).map(|c| p(c[0]).dot(p(c[1]).cross(p(c[2])))).sum::<f64>() / 6.0;
    assert!((v - exact).abs() < 3e-3 * exact, "mesh {:.3} mm³, exact {:.3}", v * 1e9, exact * 1e9);
}

/// A cone written with its seam where its coaxial cylinder's is (cf-o-ring-generator's chamfer cones, 22.5° off their
/// cylinders': live's import cut each circle between them in two, at both seams; live's own file has them together).
#[test]
fn parasolid_cone_seam_on_its_cylinders() {
    let dir = std::env::temp_dir().join(format!("noshape-xt-seam-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let turned = DVec3::new(22.5f64.to_radians().cos(), 22.5f64.to_radians().sin(), 0.0);
    let cyl = make_cylinder(&Plane::new(DVec3::ZERO, DVec3::Z, DVec3::X), 0.01, 0.02, std::f64::consts::TAU).unwrap();
    let cone = make_cone(&Plane::new(DVec3::new(0.0, 0.0, 0.02), DVec3::Z, turned), 0.01, 0.006, 0.004, std::f64::consts::TAU).unwrap();
    let part = fuse(&cyl, &cone).unwrap().shape;
    let x_dirs = |s: &Shape| -> Vec<DVec3> {
        s.faces().iter().filter_map(|f| match f.face_surface() {
            Ok(Surface::Cylinder { x_dir, .. } | Surface::Cone { x_dir, .. }) => Some(x_dir),
            _ => None,
        }).collect()
    };
    let before = x_dirs(&part);
    assert!(before.len() == 2 && before[0].distance(before[1]) > 0.1, "the model's seams apart: {before:?}");
    let path = dir.join("seam.x_t");
    write_parasolid(&path, std::slice::from_ref(&part), LengthUnit::Meter, false).unwrap();
    let back = read_parasolid(&path, LengthUnit::Meter).unwrap();
    let after = x_dirs(&back);
    assert!(after.len() == 2 && after[0].distance(after[1]) < 1e-9, "the file's seams apart: {after:?}");
    // The circle between them, cut in the model where each face's seam meets it, is one ring without a vertex in the
    // file, as Parasolid writes it (live 2411: its import kept the cuts): no point of the file lies on it.
    let text = std::fs::read_to_string(&path).unwrap();
    let joined: String = text[text.find("**END_OF_HEADER").unwrap()..].lines().skip(1).collect();
    let numbers: Vec<f64> = joined.split_whitespace().filter_map(|t| t.parse().ok()).collect();
    let on_ring = numbers.windows(3).filter(|p| (p[0].hypot(p[1]) - 0.01).abs() < 1e-9 && (p[2] - 0.02).abs() < 1e-9).count();
    assert_eq!(on_ring, 0, "a vertex on the cylinder's and cone's circle");
    let (a, b) = (volume_properties(&part).unwrap().mass, volume_properties(&back).unwrap().mass);
    assert!((a - b).abs() < 1e-9 * a, "{a} vs {b}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// An STL's curved wall made of an extruded B-spline curve (a user's printed tab copied from a STEP, lead 11-10) is
/// facetted in strips along its straight rulings, as Parasolid's: no node inside the face (BRepMesh's node insertion
/// put ~1,400 to 15,000 there, up to 11 s a face, and tilted the user's facets up to 3.3°: a zig-zag band in the
/// slicer), every facet along the surface (its normal within 0.1° of the surface's at its centre).
#[test]
fn an_extruded_spline_walls_facets_follow_the_surface() {
    let mm = 1e-3;
    let n = 8;
    let arc = |r: f64| -> Vec<DVec3> {
        (0..=n).map(|i| {
            let s = i as f64 / n as f64;
            let a = (-30.0 + 60.0 * (s + 0.3 * s * (1.0 - s))).to_radians();
            let r = r * (1.0 + 0.04 * (3.0 * a).sin());
            DVec3::new(r * a.cos(), r * a.sin(), 0.0) * mm
        }).collect()
    };
    let (outer, inner) = (arc(40.0), arc(38.0));
    let edges = [
        make_interpolated_spline(&outer, &[], false).unwrap(),
        make_line(outer[n], inner[n]).unwrap(),
        make_interpolated_spline(&inner.iter().rev().copied().collect::<Vec<_>>(), &[], false).unwrap(),
        make_line(inner[0], outer[0]).unwrap(),
    ];
    let profile = make_face_from_wire(&make_wire(&edges).unwrap(), true).unwrap();
    let tab = extrude(&profile, DVec3::Z * 38.0 * mm).unwrap().shape;
    let faces = tab.faces();
    let walls: Vec<usize> = (0..faces.len()).filter(|&k| matches!(faces[k].face_surface(), Ok(Surface::Extrusion { .. }))).collect();
    assert_eq!(walls.len(), 2, "two extruded spline walls");
    // An STL at Medium (chordal 0.12 mm, 6.25°) as the export takes it.
    let t = tessellate_export(&tab, 0.12 * 2e-6, 12.5f64.to_radians(), 0.0).unwrap();
    assert_eq!(t.faces.len(), faces.len());
    let p = |i: u32| DVec3::from(t.positions[i as usize].map(f64::from));
    for &k in &walls {
        let f = &t.faces[k];
        let tris = &t.indices[f.first_index as usize..(f.first_index + f.index_count) as usize];
        let worst = tris
            .chunks(3)
            .map(|c| {
                let (a, b, c) = (p(c[0]), p(c[1]), p(c[2]));
                let n = (b - a).cross(c - a).normalize();
                n.dot(faces[k].normal_at_point((a + b + c) / 3.0).unwrap()).abs().min(1.0).acos().to_degrees()
            })
            .fold(0.0, f64::max);
        assert!(worst < 0.1, "wall {k}: a facet {worst:.3}° off the surface");
        assert_eq!(f.vertex_count as usize, tris.len() / 3 + 2, "wall {k}: strips between its edges' nodes, none inside ({} nodes, {} triangles)", f.vertex_count, tris.len() / 3);
    }
}

/// A STEP name with a raw byte 0x80-0x9F (asm-tvc-mount's servo "Pe\x87a", no UTF-8: the import failed whole) reads
/// as live reads it, Windows-1252's character ("Pe‡a"); other non-ASCII names come through whole.
#[test]
fn step_names_with_windows_1252_bytes_read_as_live() {
    let dir = std::env::temp_dir().join(format!("noshape-step-name-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let block = make_box(&Plane::xy(), 0.01, 0.01, 0.01).unwrap();
    let path = dir.join("names.step");
    write_step(&path, &[ExportShape { shape: block.clone(), name: Some("PeXa".into()), color: None }, ExportShape { shape: block, name: Some("Peça".into()), color: None }], LengthUnit::Millimeter).unwrap();
    let mut bytes = std::fs::read(&path).unwrap();
    let at: Vec<usize> = bytes.windows(4).enumerate().filter(|(_, w)| *w == b"PeXa").map(|(i, _)| i).collect();
    assert!(!at.is_empty(), "the name in the file");
    for i in at {
        bytes[i + 2] = 0x87;
    }
    std::fs::write(&path, &bytes).unwrap();
    let names: Vec<String> = read_step(&path, LengthUnit::Millimeter).unwrap().into_iter().filter_map(|s| s.name).collect();
    assert!(names.iter().any(|n| n == "Pe‡a"), "{names:?}");
    assert!(names.iter().any(|n| n == "Peça"), "{names:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Import › IGES postprocessing ("repairs linear edge translation"): an edge an IGES file carries as a spline that runs
/// straight becomes a line, a bowed one stays a spline, and the solid is unchanged. A 20 mm square plate, its right side
/// a cubic B-spline with its poles on the side, its top a cubic bowed 1 mm, extruded 10 mm.
#[test]
fn iges_postprocessing_makes_straight_spline_edges_lines() {
    use glam::DVec2;
    let cubic = |poles: Vec<DVec2>| Segment2d::BSpline { poles, weights: None, knots: vec![0.0, 1.0], mults: vec![4, 4], degree: 3, periodic: false };
    let profile = [
        Segment2d::Line { a: DVec2::new(0.0, 0.0), b: DVec2::new(0.02, 0.0) },
        cubic(vec![DVec2::new(0.02, 0.0), DVec2::new(0.02, 0.005), DVec2::new(0.02, 0.012), DVec2::new(0.02, 0.02)]),
        cubic(vec![DVec2::new(0.02, 0.02), DVec2::new(0.013, 0.021333), DVec2::new(0.007, 0.021333), DVec2::new(0.0, 0.02)]),
        Segment2d::Line { a: DVec2::new(0.0, 0.02), b: DVec2::new(0.0, 0.0) },
    ];
    let face = make_face_2d(&Plane::xy(), &profile, &[]).unwrap();
    let solid = extrude(&face, DVec3::new(0.0, 0.0, 0.01)).unwrap().shape;
    let kinds = |s: &Shape| {
        let curves: Vec<Curve> = s.edges().iter().map(|e| e.edge_info().unwrap().curve).collect();
        (curves.iter().filter(|c| matches!(c, Curve::Line { .. })).count(), curves.iter().filter(|c| matches!(c, Curve::BSpline { .. })).count())
    };
    let (lines, splines) = kinds(&solid);
    assert_eq!(splines, 4, "the two spline sides, top and bottom");
    let out = straighten_edges(&solid, 1e-6).unwrap();
    assert!(out.is_valid());
    assert_eq!(kinds(&out), (lines + 2, 2), "the straight spline side's two edges are lines, the bowed side's stay");
    let (v0, v1) = (solid.volume().unwrap(), out.volume().unwrap());
    assert!((v0 - v1).abs() < 1e-12, "{v0} vs {v1}");
}

/// A STEP file whose uncertainty is loose (live's: 2e-5 m) has its edges projected onto curved faces as tightly as
/// they lie on them (cf-spur-gear's chamfer cones, bounded by the flanks' B-spline edges: 7.6 µm off, the 32-tooth
/// gear 0.7 mm² short): a cone cut by planes along its axis (hyperbolas on it) reads back with its area.
#[test]
fn a_loose_step_files_edges_on_a_cone_read_back_tight() {
    let mm = 1e-3;
    let cone = make_cone(&Plane::from_normal(DVec3::ZERO, DVec3::Z), 10.0 * mm, 8.0 * mm, mm, std::f64::consts::TAU).unwrap();
    let block = make_box_corners(DVec3::new(-9.0, -9.0, -1.0) * mm, DVec3::new(9.0, 9.0, 2.0) * mm).unwrap();
    let part = common(&cone, &block).unwrap().shape;
    let want = surface_properties(&part).unwrap().mass;
    let dir = std::env::temp_dir().join(format!("noshape-loose-step-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("cone.step");
    write_step(&path, &[ExportShape { shape: part, name: Some("Part 1".into()), color: None }], LengthUnit::Meter).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    let at = text.find("UNCERTAINTY_MEASURE_WITH_UNIT(LENGTH_MEASURE(").expect("an uncertainty") + "UNCERTAINTY_MEASURE_WITH_UNIT(LENGTH_MEASURE(".len();
    let end = at + text[at..].find(')').unwrap();
    std::fs::write(&path, format!("{}0.02{}", &text[..at], &text[end..])).unwrap();
    let back = read_step(&path, LengthUnit::Meter).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    let got: f64 = back.iter().map(|p| surface_properties(&p.shape).unwrap().mass).sum();
    assert!((got - want).abs() < 1e-6 * want, "area {} mm², the part's {}", got / (mm * mm), want / (mm * mm));
}

/// A STEP surface model of one closed shell (FreeCAD's: SHELL_BASED_SURFACE_MODEL, no MANIFOLD_SOLID_BREP) reads as a
/// solid, as live reads it (asm-infento-subs' shaft, lever, halfClutch); an open shell stays a surface.
#[test]
fn a_step_surface_models_closed_shell_reads_as_a_solid() {
    let s = part();
    let want = volume_properties(&s).unwrap().mass;
    let shell = s.shells().into_iter().next().unwrap();
    let dir = std::env::temp_dir().join(format!("noshape-shell-model-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("shell.step");
    write_step(&path, &[ExportShape { shape: shell.clone(), name: Some("shaft".into()), color: None }], LengthUnit::Meter).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("SHELL_BASED_SURFACE_MODEL") && !text.contains("MANIFOLD_SOLID_BREP"), "written as a surface model");
    let back = read_step(&path, LengthUnit::Meter).unwrap();
    assert_eq!(back.len(), 1);
    assert_eq!(back[0].shape.solids().len(), 1, "the closed shell a solid");
    let v = volume_properties(&back[0].shape).unwrap().mass;
    assert!((v - want).abs() < 1e-6 * want, "{v} vs {want}");
    // Without one face: open, a surface.
    let faces = shell.faces();
    let open = sew(&faces[1..], 1e-7, false).unwrap().shape;
    let path = dir.join("open.step");
    write_step(&path, &[ExportShape { shape: open, name: None, color: None }], LengthUnit::Meter).unwrap();
    let back = read_step(&path, LengthUnit::Meter).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(back[0].shape.solids().is_empty(), "an open shell stays a surface");
}

/// cf-hose-routing's hose (its Part Studio's part, tests/data/xt-parts): its inner walls (the Shell's offsets of the
/// outer walls, approximated as non-rational degree-12 single-span B-surfaces) won't go round periodic, and Parasolid
/// dropped the hoses whose walls were written so (live 2452, 2975). Written as live writes its own hose, offsets of the
/// periodic outer walls (OFFSET_SURF), every face whole: 12 faces read back (before, the one stiff seam had every
/// closed face split in two: 22 / 44 / 24); live imports the four hoses so (3054). Only its tolerant end rings are cut
/// (10-20 nm off their walls), the others kept whole (before, one tolerant ring had all twelve cut: 12 / 34 / 24).
#[test]
fn a_hoses_walls_write_whole() {
    let dir = std::env::temp_dir().join(format!("noshape-xt-hose-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let hose = read_brep(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/xt-parts/hose-routing-hose.brep")).unwrap();
    assert_eq!((hose.count(ShapeType::Face), hose.count(ShapeType::Edge), hose.count(ShapeType::Vertex)), (12, 22, 12), "the model, seams included");
    let file = dir.join("hose.x_t");
    write_parasolid(&file, std::slice::from_ref(&hose), LengthUnit::Meter, false).unwrap();
    let back = read_parasolid(&file, LengthUnit::Meter).unwrap();
    // (Its inner walls' tolerant end rings split in two: tolerant edges closed on one vertex made Parasolid drop the hose,
    // live 3054.)
    assert_eq!((back.count(ShapeType::Face), back.count(ShapeType::Edge), back.count(ShapeType::Vertex)), (12, 28, 18), "every face whole, six rings cut");
    // (The model's inner walls are the offsets approximated, ~1 nm off on the 1 mm wall: 1e-6 of the volume.)
    let (v0, v1) = (hose.volume().unwrap(), back.volume().unwrap());
    assert!((v1 - v0).abs() < 2e-6 * v0, "{v1} vs {v0}");
}

/// A plane's cut across a spline extrusion leaves OCCT's approximated intersection edge (µm off its faces; a STEP or
/// IGES reader refits it, live's import of cf-sheet-metal-model-plus's sheet metal part made 61 faces of 58):
/// `exact_edges` rebuilds it as the basis curve carried onto the plane, within 1e-7 m, the solid unchanged.
#[test]
fn a_planes_cut_across_a_spline_extrusion_is_rebuilt_exact() {
    use glam::DVec2;
    let spline = Segment2d::BSpline { poles: vec![DVec2::new(0.0, 0.0), DVec2::new(0.01, 0.008), DVec2::new(0.02, -0.004), DVec2::new(0.03, 0.006)], weights: None, knots: vec![0.0, 1.0], mults: vec![4, 4], degree: 3, periodic: false };
    let outline = [spline, Segment2d::Line { a: DVec2::new(0.03, 0.006), b: DVec2::new(0.03, -0.01) }, Segment2d::Line { a: DVec2::new(0.03, -0.01), b: DVec2::new(0.0, -0.01) }, Segment2d::Line { a: DVec2::new(0.0, -0.01), b: DVec2::new(0.0, 0.0) }];
    let face = make_face_2d(&Plane::from_normal(DVec3::ZERO, DVec3::Z), &outline, &[]).unwrap();
    let solid = extrude(&face, DVec3::new(0.0, 0.0, 0.01)).unwrap().shape;
    let tool = make_box_corners(DVec3::new(-0.01, -0.03, 0.005), DVec3::new(0.05, 0.03, 0.03)).unwrap();
    let tool = tool.transformed(&Transform::Rotate { origin: DVec3::new(0.015, 0.0, 0.005), axis: DVec3::new(1.0, 0.3, 0.0).normalize(), angle: 0.35 }).unwrap();
    let cut = cut(&solid, &tool).unwrap().shape;
    let before = cut.max_edge_tolerance();
    assert!(before > 1e-7, "OCCT's intersection edge is exact already ({before:e} m): the case no longer tests anything");
    let exact = exact_edges(&cut).unwrap();
    assert!(exact.is_valid());
    assert!(exact.max_edge_tolerance() <= 1.0001e-7, "{:e} m (was {before:e})", exact.max_edge_tolerance());
    let (v0, v1) = (volume_properties(&cut).unwrap().mass, volume_properties(&exact).unwrap().mass);
    assert!((v1 - v0).abs() <= 1e-6 * v0, "{v0} -> {v1}");
}

/// Two adjacent faces on one plane up to rounding (1e-9 rad apart) are exported on one plane exactly: live's import
/// merges faces on one surface only when their planes are equal (cf-sheet-metal-model-plus: 61 faces of ours, 58 of
/// its own, where three adjacent pairs differed by 3e-11 to 5e-9 m). Neither moves by more than that.
#[test]
fn adjacent_faces_on_one_plane_up_to_rounding_export_on_one_plane() {
    use glam::DVec2;
    let rect = |y0: f64, y1: f64| vec![Segment2d::Line { a: DVec2::new(0.0, y0), b: DVec2::new(0.01, y0) }, Segment2d::Line { a: DVec2::new(0.01, y0), b: DVec2::new(0.01, y1) }, Segment2d::Line { a: DVec2::new(0.01, y1), b: DVec2::new(0.0, y1) }, Segment2d::Line { a: DVec2::new(0.0, y1), b: DVec2::new(0.0, y0) }];
    let a = make_face_2d(&Plane::from_normal(DVec3::ZERO, DVec3::Z), &rect(-0.01, 0.0), &[]).unwrap();
    let tilt = 1e-9_f64;
    let b = make_face_2d(&Plane::from_normal(DVec3::ZERO, DVec3::new(0.0, -tilt.sin(), tilt.cos())), &rect(0.0, 0.01), &[]).unwrap();
    let shell = sew(&[a, b], 1e-6, false).unwrap().shape;
    let normals = |s: &Shape| -> Vec<(DVec3, f64)> {
        s.sub_shapes(ShapeType::Face).iter().map(|f| match f.face_surface().unwrap() {
            Surface::Plane(p) => (p.normal, p.normal.dot(p.origin)),
            other => panic!("{other:?}"),
        }).collect()
    };
    let before = normals(&shell);
    assert_eq!(before.len(), 2);
    assert_ne!(before[0].0.abs(), before[1].0.abs(), "the sewn faces' planes are one already: the case no longer tests anything");
    let after = normals(&exact_edges(&shell).unwrap());
    assert_eq!(after[0].0.abs(), after[1].0.abs(), "{after:?}");
    assert!((after[0].1.abs() - after[1].1.abs()).abs() < 1e-15, "{after:?}");
    assert!(after.iter().zip(&before).all(|(x, y)| (x.0 - y.0).length() < 1e-8), "{before:?} -> {after:?}");
}

/// One product whose shape is several B-rep representations, each by its own SHAPE_REPRESENTATION_RELATIONSHIP (FreeCAD's
/// STEP: asm-corexy's Pulley_GT2-6mm-20T_bore_5_001, three solids): XDE makes each a component named by its link
/// ("=>[0:1:1:2]"); every part takes the product's name, as live.
#[test]
fn a_products_solids_by_representation_relationships_take_its_name() {
    let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/step/face-colour.step")).unwrap();
    // face-colour.step: three products, each SHAPE_REPRESENTATION (#333-#335) related to its own B-rep (#21-#23). All three
    // B-reps to the first product's representation, the other two products dropped, names only on the product.
    let freecad: String = text
        .lines()
        .filter(|l| !l.starts_with("#331=") && !l.starts_with("#332="))
        .map(|l| match l {
            _ if l.starts_with("#19=") => "#19=SHAPE_REPRESENTATION_RELATIONSHIP('','',#333,#22);".to_string(),
            _ if l.starts_with("#20=") => "#20=SHAPE_REPRESENTATION_RELATIONSHIP('','',#333,#23);".to_string(),
            _ if l.starts_with("#549=") => l.replace("'Part 3','Part 3','Part 3'", "'Pulley','Pulley',''"),
            _ if l.starts_with("#333=") || l.contains("=MANIFOLD_SOLID_BREP(") => l.replace("'Part 3'", "''").replace("'Part 2'", "''").replace("'Part 1'", "''"),
            _ => l.to_string(),
        })
        .collect::<Vec<_>>()
        .join("\n");
    let dir = std::env::temp_dir().join(format!("noshape-step-srr-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("pulley.step");
    std::fs::write(&path, freecad).unwrap();
    let names: Vec<String> = read_step(&path, LengthUnit::Meter).unwrap().into_iter().map(|b| b.name.unwrap_or_default()).collect();
    assert_eq!(names, ["Pulley", "Pulley", "Pulley"]);
    let _ = std::fs::remove_dir_all(&dir);
}

/// STEP and IGES write an offset surface (the hose's outer walls) as a B-surface, as live's files have none (c2-rings-loft:
/// B-spline surfaces only): without p-curves a reader trimmed the offsets wrong (ours read the ring's Part 1 back
/// at 176 mm³ for 440.8). The walls read back with the model's volume.
#[test]
fn step_and_iges_write_offset_surfaces_as_bsurfaces() {
    let dir = std::env::temp_dir().join(format!("noshape-step-offset-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let hose = read_brep(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/xt-parts/hose-routing-hose.brep")).unwrap();
    let v0 = hose.volume().unwrap();
    let step = dir.join("hose.step");
    write_step(&step, &[ExportShape { shape: hose.clone(), name: None, color: None }], LengthUnit::Meter).unwrap();
    let text = std::fs::read_to_string(&step).unwrap();
    assert!(!text.contains("OFFSET_SURFACE(") && text.contains("B_SPLINE_SURFACE"), "offsets written as such");
    let back = read_step(&step, LengthUnit::Meter).unwrap();
    let v1: f64 = back.iter().map(|s| s.shape.volume().unwrap()).sum();
    assert!((v1 - v0).abs() < 1e-5 * v0, "STEP: {v1} vs {v0}");
    let iges = dir.join("hose.igs");
    write_iges(&iges, std::slice::from_ref(&hose), LengthUnit::Meter).unwrap();
    let v2 = read_iges(&iges, LengthUnit::Meter).unwrap().volume().unwrap();
    assert!((v2 - v0).abs() < 1e-5 * v0, "IGES: {v2} vs {v0}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A face on a periodic extrusion across its start (half an elliptic prism, the ellipse's start in the face's middle)
/// is written on a B-surface cut to the part it covers: converted closed but not periodic, its p-curves ran past the
/// surface's seam and readers trimmed it wrong (c2-rings-loft's top band: Part 1 read back at 659 mm³ for 440.8).
#[test]
fn a_face_across_a_periodic_extrusions_start_reads_back() {
    use glam::DVec2;
    let dir = std::env::temp_dir().join(format!("noshape-step-seam-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let ellipse = make_face_2d(&Plane::from_normal(DVec3::ZERO, DVec3::Z), &[Segment2d::Ellipse { center: DVec2::ZERO, major_radius: 0.02, minor_radius: 0.01, rotation: 0.0 }], &[]).unwrap();
    let prism = extrude(&ellipse, DVec3::new(0.0, 0.0, 0.01)).unwrap().shape;
    let half = cut(&prism, &make_box_corners(DVec3::new(-0.05, -0.05, -0.01), DVec3::new(0.0, 0.05, 0.02)).unwrap()).unwrap().shape;
    let v0 = half.volume().unwrap();
    let step = dir.join("half.step");
    write_step(&step, &[ExportShape { shape: half.clone(), name: None, color: None }], LengthUnit::Meter).unwrap();
    let v1: f64 = read_step(&step, LengthUnit::Meter).unwrap().iter().map(|s| s.shape.volume().unwrap()).sum();
    assert!((v1 - v0).abs() < 1e-5 * v0, "STEP: {v1} vs {v0}");
    let iges = dir.join("half.igs");
    write_iges(&iges, std::slice::from_ref(&half), LengthUnit::Meter).unwrap();
    let v2 = read_iges(&iges, LengthUnit::Meter).unwrap().volume().unwrap();
    assert!((v2 - v0).abs() < 1e-4 * v0, "IGES: {v2} vs {v0}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A cylinder whose side is two faces (split down the side at 90° and 270°): the top disc's rim is arcs the disc has as
/// a loop of their own, its side faces in a longer one. Parasolid keeps the arcs (one ring would leave the side faces'
/// arcs unshared: cf-snap-hook's hook tips read back as 13 sheets): one closed solid of the cylinder's volume.
#[test]
fn a_disc_whose_rim_arcs_bound_a_split_side_exports_as_one_solid() {
    let mm = 1e-3;
    let cyl = make_cylinder(&Plane::from_normal(DVec3::ZERO, DVec3::Z), 5.0 * mm, 10.0 * mm, std::f64::consts::TAU).unwrap();
    let down = |y: f64| make_line(DVec3::new(0.0, y, 10.0) * mm, DVec3::new(0.0, y, 0.0) * mm).unwrap();
    let s = split_on(&cyl, &[down(5.0), down(-5.0)], 1e-7).unwrap().shape;
    assert!(s.count(ShapeType::Face) >= 4);
    let want = volume_properties(&s).unwrap().mass;
    let path = std::env::temp_dir().join(format!("noshape-xt-rim-{}.x_t", std::process::id()));
    write_parasolid(&path, std::slice::from_ref(&s), LengthUnit::Meter, false).unwrap();
    let back = read_parasolid(&path, LengthUnit::Meter).unwrap();
    let _ = std::fs::remove_file(&path);
    assert_eq!(back.count(ShapeType::Solid), 1);
    assert!(back.is_valid());
    let v = volume_properties(&back).unwrap().mass;
    assert!((v - want).abs() < 1e-6 * want, "{v} vs {want}");
}

/// An edge on a B-spline its end vertex can't be projected onto (degree 14, 108 poles: a cf-sheet-metal-model-plus hem
/// face's) reads back over the range the SAT file gives it: the projection ended it 0.07 mm in for 19.7, and the part
/// read back inside out, so its import failed.
#[test]
fn a_sat_edge_keeps_its_range_where_projection_misses() {
    let face = read_brep(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/xt-parts/sheet-metal-plus-hem-face.brep")).unwrap();
    let want = surface_properties(&face).unwrap().mass;
    let path = std::env::temp_dir().join(format!("noshape-sat-range-{}.sat", std::process::id()));
    write_acis(&path, &[sew(&[face], 1e-7, false).unwrap().shape], LengthUnit::Meter).unwrap();
    let back = read_acis_parts(&path, LengthUnit::Meter).unwrap();
    let _ = std::fs::remove_file(&path);
    assert_eq!(back.len(), 1);
    let a = surface_properties(&back[0].shape).unwrap().mass;
    assert!((a - want).abs() < 1e-5 * want, "{a} vs {want}");
}

/// An ACIS import merges a body's faces on one surface, as live's STEP / IGES / ACIS imports do (c2-sm-louver's live.sat:
/// 50 faces a louver for its Parasolid's 56; cf-sheet-metal-model-plus' 30 for 54): two boxes fused side by side, their
/// top, bottom and sides each two faces, read back a 6-face box.
#[test]
fn an_acis_import_merges_faces_on_one_surface() {
    let a = make_box_corners(DVec3::ZERO, DVec3::new(0.02, 0.02, 0.01)).unwrap();
    let b = make_box_corners(DVec3::new(0.02, 0.0, 0.0), DVec3::new(0.04, 0.02, 0.01)).unwrap();
    let both = fuse(&a, &b).unwrap().shape;
    assert_eq!(both.count(ShapeType::Face), 10, "the fuse keeps the faces split");
    let path = std::env::temp_dir().join(format!("noshape-sat-merge-{}.sat", std::process::id()));
    write_acis(&path, std::slice::from_ref(&both), LengthUnit::Meter).unwrap();
    let back = read_acis_parts(&path, LengthUnit::Meter).unwrap();
    let _ = std::fs::remove_file(&path);
    assert_eq!(back.len(), 1);
    assert_eq!(back[0].shape.count(ShapeType::Face), 6);
}

/// Two Shell walls (offsets of a loft's periodic B-surface) and the cylinder between them, as Parasolid: the offsets
/// written as OFFSET_SURF over the basis (approximated, the edges along them were refitted badly) and each fin's curve
/// in the period of the basis's range as written (one a period off put an edge 9 mm away): c2-rings-loft's Part 1
/// read back at 303 mm³ for 441.
#[test]
fn offsets_of_a_periodic_bsurface_round_trip_through_parasolid() {
    let walls = read_brep(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/xt-parts/rings-loft-shell-walls.brep")).unwrap();
    let want = surface_properties(&walls).unwrap().mass;
    let path = std::env::temp_dir().join(format!("noshape-xt-walls-{}.x_t", std::process::id()));
    write_parasolid(&path, std::slice::from_ref(&walls), LengthUnit::Meter, false).unwrap();
    let back = read_parasolid(&path, LengthUnit::Meter).unwrap();
    let _ = std::fs::remove_file(&path);
    assert_eq!(back.count(ShapeType::Face), 3);
    let a = surface_properties(&back).unwrap().mass;
    assert!((a - want).abs() < 1e-4 * want, "{a} vs {want}");
}

/// The same Shell walls through ACIS: written as B-surfaces there (kept as offsets for Parasolid only, their face
/// range past the basis's made SAT fail: c2-rings-loft's ours.sat "V parameters out of range"). The approximation
/// loses 0.17 % of the area (open: ACIS offset surfaces).
#[test]
fn offset_walls_export_to_acis() {
    let walls = read_brep(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/xt-parts/rings-loft-shell-walls.brep")).unwrap();
    let want = surface_properties(&walls).unwrap().mass;
    let path = std::env::temp_dir().join(format!("noshape-sat-walls-{}.sat", std::process::id()));
    write_acis(&path, std::slice::from_ref(&walls), LengthUnit::Meter).unwrap();
    let back = read_acis_parts(&path, LengthUnit::Meter).unwrap();
    let _ = std::fs::remove_file(&path);
    let a: f64 = back.iter().map(|p| surface_properties(&p.shape).unwrap().mass).sum();
    assert!((a - want).abs() < 5e-3 * want, "{a} vs {want}");
}

/// A loft's wall going round (a rational B-surface whose end rows' weights differ by 6e-12): one periodic face in
/// Parasolid, not split at its seam (c2-rings-loft's Part 1 read back 275 faces for 273).
#[test]
fn a_loft_wall_closed_but_for_its_weights_last_digits_stays_one_face() {
    let wall = read_brep(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/xt-parts/rings-loft-wall.brep")).unwrap();
    let want = surface_properties(&wall).unwrap().mass;
    let path = std::env::temp_dir().join(format!("noshape-xt-loft-wall-{}.x_t", std::process::id()));
    write_parasolid(&path, std::slice::from_ref(&wall), LengthUnit::Meter, false).unwrap();
    let back = read_parasolid(&path, LengthUnit::Meter).unwrap();
    let _ = std::fs::remove_file(&path);
    assert_eq!(back.count(ShapeType::Face), 1);
    let a = surface_properties(&back).unwrap().mass;
    assert!((a - want).abs() < 1e-6 * want, "{a} vs {want}");
}

/// A loft's wall going round (c2-rings-loft's): one face over ACIS's periodic B-surface (clamped knots, the "periodic"
/// form, no seam), as live's own SAT writes it, read back as one face (it was split at its seam, two faces).
#[test]
fn a_loft_wall_goes_round_as_one_acis_face() {
    let wall = read_brep(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/xt-parts/rings-loft-wall.brep")).unwrap();
    let want = surface_properties(&wall).unwrap().mass;
    let path = std::env::temp_dir().join(format!("noshape-sat-loft-wall-{}.sat", std::process::id()));
    write_acis(&path, std::slice::from_ref(&wall), LengthUnit::Meter).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    let back = read_acis_parts(&path, LengthUnit::Meter).unwrap();
    let _ = std::fs::remove_file(&path);
    assert!(text.contains(" periodic open ") || text.contains(" open periodic "), "no periodic B-surface");
    assert_eq!(back.iter().map(|p| p.shape.count(ShapeType::Face)).sum::<usize>(), 1);
    let a: f64 = back.iter().map(|p| surface_properties(&p.shape).unwrap().mass).sum();
    assert!((a - want).abs() < 1e-6 * want, "{a} vs {want}");
}

/// A sketch ellipse of equal radii is a circle (as Parasolid's): revolved, a torus, one face through ACIS
/// (cf-o-ring-generator's rings: a surface of revolution of an ellipse, written as a B-surface split in four).
#[test]
fn an_equal_radii_ellipse_revolves_to_a_torus() {
    let mm = 1e-3;
    let plane = Plane::new(DVec3::ZERO, DVec3::Y, DVec3::X);
    let section = make_face_2d(&plane, &[Segment2d::Ellipse { center: DVec2::new(20.0 * mm, 0.0), major_radius: 3.175 * mm, minor_radius: 3.175 * mm, rotation: 0.4 }], &[]).unwrap();
    let ring = revolve(&section, DVec3::ZERO, DVec3::Z, std::f64::consts::TAU).unwrap().shape;
    assert!(ring.faces().iter().all(|f| matches!(f.face_surface(), Ok(Surface::Torus { .. }))));
    let path = std::env::temp_dir().join(format!("noshape-sat-oring-{}.sat", std::process::id()));
    write_acis(&path, std::slice::from_ref(&ring), LengthUnit::Meter).unwrap();
    let back = read_acis_parts(&path, LengthUnit::Meter).unwrap();
    let _ = std::fs::remove_file(&path);
    assert_eq!(back.iter().map(|p| p.shape.count(ShapeType::Face)).sum::<usize>(), 1);
}

/// A draft's cone face OCCT cut in two at its seam (shown as one face), with the four faces round it: written as one
/// face, its rims one arc each (Parasolid has no seam), as shown: c2-housing-draft's Part 1 read back 37/82/46 for
/// its 36/79/44.
#[test]
fn a_cone_face_cut_at_its_seam_writes_as_one_face() {
    let faces = read_brep(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/xt-parts/housing-draft-cone-pieces.brep")).unwrap();
    let shown = |s: &Shape| {
        let t = s.shown_topology().unwrap();
        (t.face_count(), t.edges.iter().enumerate().filter(|&(i, e)| *e && t.joined[i] == i).count(), t.vertices.iter().filter(|v| **v).count())
    };
    let want = shown(&faces);
    let area = surface_properties(&faces).unwrap().mass;
    let path = std::env::temp_dir().join(format!("noshape-xt-cone-pieces-{}.x_t", std::process::id()));
    write_parasolid(&path, std::slice::from_ref(&faces), LengthUnit::Meter, false).unwrap();
    let back = read_parasolid(&path, LengthUnit::Meter).unwrap();
    let _ = std::fs::remove_file(&path);
    assert_eq!(want.0, 4);
    assert_eq!(back.count(ShapeType::Face), 4);
    assert_eq!(shown(&back), want);
    let a = surface_properties(&back).unwrap().mass;
    assert!((a - area).abs() < 1e-6 * area, "{a} vs {area}");
}

/// A tube wall whose closed section's seam is kinked by under 2 mrad (as our sweep's degree-12 sections, core's
/// SWEEP-SEAM-KINK: cf-hose-routing's hose walls 0.41 to 1.98 mrad came back from our SAT split in two) goes round as
/// one ACIS face, its seam made tangent by moving the poles beside it under a micrometre; one kinked more stays split.
#[test]
fn a_tube_wall_with_a_slightly_kinked_seam_goes_round_as_one_acis_face() {
    let faces_back = |eps: f64| {
        let r = 0.003;
        let mut poles: Vec<DVec3> = (0..8).map(|k| {
            let a = std::f64::consts::TAU * k as f64 / 8.0;
            DVec3::new(r * a.cos(), r * a.sin(), 0.0)
        }).collect();
        let t = poles[1].distance(poles[0]) * 0.5;
        poles.insert(1, DVec3::new(r, t, 0.0));
        poles.push(DVec3::new(r - t * eps.sin(), -t * eps.cos(), 0.0));
        poles.push(DVec3::new(r, 0.0, 0.0));
        let spans = poles.len() - 3;
        let knots: Vec<f64> = (0..=spans).map(|i| i as f64).collect();
        let mut mults = vec![1u32; spans + 1];
        mults[0] = 4;
        mults[spans] = 4;
        let section = make_bspline(&poles, None, &knots, &mults, 3, false).unwrap();
        let wall = extrude(&section, DVec3::new(0.0, 0.0, 0.02)).unwrap().shape;
        assert_eq!(wall.count(ShapeType::Face), 1);
        let want = surface_properties(&wall).unwrap().mass;
        let path = std::env::temp_dir().join(format!("noshape-sat-kinked-wall-{}-{eps}.sat", std::process::id()));
        write_acis(&path, std::slice::from_ref(&wall), LengthUnit::Meter).unwrap();
        let back = read_acis_parts(&path, LengthUnit::Meter).unwrap();
        let _ = std::fs::remove_file(&path);
        let a: f64 = back.iter().map(|p| surface_properties(&p.shape).unwrap().mass).sum();
        assert!((a - want).abs() < 1e-6 * want, "{eps}: {a} vs {want}");
        back.iter().map(|p| p.shape.count(ShapeType::Face)).sum::<usize>()
    };
    assert_eq!(faces_back(4e-4), 1);
    assert_eq!(faces_back(1.5e-3), 1);
    assert!(faces_back(2.5e-3) > 1);
}

/// c2-rings-loft Part 2's 510 mm² cylinder face and the 0.007 mm² piece of it across a seam segment the big face meets
/// once (shown as one face; UnifySameDomain leaves them): written as the one face of live's model, in x_t and SAT (ours
/// read both back with 133 faces for 132; the SAT cylinder's reference now lies where no cut crosses, so the reader's
/// seam doesn't split the face again).
#[test]
fn a_seam_piece_across_a_seam_segment_is_written_with_its_face() {
    let s = read_brep(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/seam/rings-loft-seam-piece.brep")).unwrap();
    assert_eq!(s.count(ShapeType::Face), 2);
    let want = surface_properties(&s).unwrap().mass;
    let path = std::env::temp_dir().join(format!("noshape-seam-piece-{}.x_t", std::process::id()));
    write_parasolid(&path, std::slice::from_ref(&s), LengthUnit::Meter, false).unwrap();
    let back = read_parasolid(&path, LengthUnit::Meter).unwrap();
    let _ = std::fs::remove_file(&path);
    assert_eq!(back.count(ShapeType::Face), 1);
    let a = surface_properties(&back).unwrap().mass;
    assert!((a - want).abs() < 1e-6 * want, "{a} vs {want}");
    let path = path.with_extension("sat");
    write_acis(&path, std::slice::from_ref(&s), LengthUnit::Meter).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    let back = read_acis_parts(&path, LengthUnit::Meter).unwrap();
    let _ = std::fs::remove_file(&path);
    assert_eq!(text.lines().filter(|l| l.starts_with("face ")).count(), 1);
    assert_eq!(back.iter().map(|p| p.shape.count(ShapeType::Face)).sum::<usize>(), 1);
    let a: f64 = back.iter().map(|p| surface_properties(&p.shape).unwrap().mass).sum();
    assert!((a - want).abs() < 1e-6 * want, "sat: {a} vs {want}");
    assert_eq!(s.count(ShapeType::Face), 2, "the model's shape is left as it was");
}

/// A STEP solid whose one shell is two boxes meeting nowhere imports as a part per piece, both named "<part>__<solid>"
/// (live2 q-step-twopiece: two solid parts "Part 2__Two pieces", 6 / 12 / 8 each, 4800 and 1000 mm³; ours was one part
/// "Part 2" of two solids).
#[test]
fn a_solid_of_disconnected_pieces_imports_as_a_part_each() {
    let a = make_box_corners(DVec3::ZERO, DVec3::new(0.01, 0.01, 0.01)).unwrap();
    let b = make_box_corners(DVec3::new(0.02, 0.0, 0.0), DVec3::new(0.03, 0.02, 0.01)).unwrap();
    let path = std::env::temp_dir().join(format!("noshape-step-pieces-{}.step", std::process::id()));
    write_step(&path, &[ExportShape { shape: Shape::compound(&[a, b]), name: Some("Combine1".into()), color: None }], LengthUnit::Meter).unwrap();
    // The two solids' shells as one shell of the first solid; the second solid dropped from the representation.
    let text = std::fs::read_to_string(&path).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    let id = |l: &str| l.split('=').next().unwrap().trim().to_string();
    let shells: Vec<&str> = lines.iter().copied().filter(|l| l.contains("CLOSED_SHELL")).collect();
    let solids: Vec<&str> = lines.iter().copied().filter(|l| l.contains("MANIFOLD_SOLID_BREP")).collect();
    assert_eq!((shells.len(), solids.len()), (2, 2), "{text}");
    let faces = |l: &str| l[l.rfind('(').unwrap() + 1..l.find(')').unwrap()].to_string();
    let merged = shells[0].replacen(&faces(shells[0]), &format!("{},{}", faces(shells[0]), faces(shells[1])), 1);
    let gone = id(solids[1]);
    let rename = |l: &str| {
        let (at, name) = (l.find("MANIFOLD_SOLID_BREP").unwrap(), l.find('\'').unwrap());
        assert!(name > at, "{l}");
        format!("{}'Two pieces'{}", &l[..name], &l[name + 1 + l[name + 1..].find('\'').unwrap() + 1..])
    };
    let mut out = String::new();
    for l in &lines {
        let l = if *l == shells[0] { merged.clone() } else if *l == solids[0] { rename(l) } else { l.to_string() };
        if l.starts_with(&gone) || l.starts_with(&id(shells[1])) {
            continue;
        }
        out += &l.replace(&format!("{gone},"), "").replace(&format!(",{gone})"), ")");
        out.push('\n');
    }
    std::fs::write(&path, out).unwrap();
    let parts = read_step(&path, LengthUnit::Meter).unwrap();
    let _ = std::fs::remove_file(&path);
    // (One part: the reader's healing makes two solids of the shell.)
    assert_eq!((parts.len(), parts[0].shape.count(ShapeType::Face)), (1, 12));
    let pieces = split_connected_pieces(parts);
    assert_eq!(pieces.len(), 2);
    for p in &pieces {
        assert_eq!(p.name.as_deref(), Some("Combine1__Two pieces"));
        assert_eq!((p.shape.count(ShapeType::Solid), p.shape.count(ShapeType::Face)), (1, 6));
        assert!(volume_properties(&p.shape).unwrap().mass > 0.0, "a piece inside out");
    }
    let v: f64 = pieces.iter().map(|p| volume_properties(&p.shape).unwrap().mass).sum();
    assert!((v - 3e-6).abs() < 1e-12, "{v}");
}

/// A concave round revolved whole (metres): its torus face is bounded by one loop, its two circles and the seam between them.
fn revolved_round(big: f64, small: f64) -> Shape {
    let p = |x: f64, z: f64| DVec3::new(x, 0.0, z);
    // In XZ: the rectangle 0..R × 0..r without the quarter disc about (R, 0).
    let profile = make_wire(&[
        make_line(p(0.0, 0.0), p(big - small, 0.0)).unwrap(),
        make_arc(p(big, 0.0), DVec3::Y, DVec3::NEG_X, small, 0.0, std::f64::consts::FRAC_PI_2).unwrap(),
        make_line(p(big, small), p(0.0, small)).unwrap(),
        make_line(p(0.0, small), p(0.0, 0.0)).unwrap(),
    ])
    .unwrap();
    revolve(&make_face_from_wire(&profile, true).unwrap(), DVec3::ZERO, DVec3::Z, std::f64::consts::TAU).unwrap().shape
}

/// The one torus face of a STEP file as a reader without p-curves sees it (as live does): the band of the torus between its
/// two latitude circles on the side left of each circle's edge, as its bound runs it, about the face's normal; as a
/// share of the whole torus.
fn step_torus_band(text: &str) -> f64 {
    use std::f64::consts::{PI, TAU};
    let ents: std::collections::HashMap<&str, &str> = text
        .split(';')
        .filter_map(|s| {
            let (k, v) = s.trim().split_once('=')?;
            Some((k.trim().strip_prefix('#')?, v.trim()))
        })
        .collect();
    fn refs(s: &str) -> Vec<&str> {
        s.split('#').skip(1).map(|r| &r[..r.find(|c: char| !c.is_ascii_digit()).unwrap_or(r.len())]).collect()
    }
    fn nums(s: &str) -> Vec<f64> {
        s[s.rfind('\'').map_or(0, |i| i + 1)..].split(|c: char| !(c.is_ascii_digit() || "+-.eE".contains(c))).filter_map(|x| x.parse().ok()).collect()
    }
    let last = |s: &str| ents[*refs(s).last().unwrap()];
    let point = |id: &str| {
        let n = nums(ents[id]);
        DVec3::new(n[0], n[1], n[2])
    };
    let axis = |id: &str| {
        let r = refs(ents[id]);
        (point(r[0]), point(r[1]).normalize(), point(r[2]).normalize())
    };
    let sign = |s: &str| if s.ends_with(".T.)") { 1.0 } else { -1.0 };
    let face = ents.values().copied().find(|f| f.starts_with("ADVANCED_FACE") && last(f).starts_with("TOROIDAL_SURFACE")).expect("a torus face");
    let surface = last(face);
    let (c, z, _) = axis(refs(surface)[0]);
    let n = nums(surface);
    let (big, small) = (n[n.len() - 2], n[n.len() - 1]);
    let mut sides = Vec::new();
    for bound in &refs(face)[..refs(face).len() - 1] {
        for oriented in refs(ents[refs(ents[*bound])[0]]) {
            let o = ents[oriented];
            let (edge, curve) = (last(o), last(last(o)));
            let along = sign(o) * sign(edge) * sign(ents[*bound]);
            if !curve.starts_with("CIRCLE") {
                continue;
            }
            let (cc, a, x) = axis(refs(curve)[0]);
            if a.dot(z).abs() < 1.0 - 1e-6 {
                continue;
            }
            let rc = *nums(curve).last().unwrap();
            let p = cc + x * rc;
            let d = a.cross(x) * along;
            let rad = (p - c - z * (p - c).dot(z)).normalize();
            let mut v = ((p - c).dot(z) / small).atan2((rc - big) / small);
            if v < -PI + 1e-6 {
                v += TAU;
            }
            let normal = (p - (c + rad * big)).normalize() * sign(face);
            sides.push((v, normal.cross(d).dot(rad * -v.sin() + z * v.cos()) > 0.0));
        }
    }
    sides.sort_by(|a, b| a.0.total_cmp(&b.0));
    let band = |v0: f64, v1: f64| (big * (v1 - v0) + small * (v1.sin() - v0.sin())) / (TAU * big);
    match sides[..] {
        [(a, true), (b, false)] => band(a, b),
        [(a, false), (b, true)] => band(b, a + TAU),
        _ => panic!("the loop's circles disagree: {sides:?}"),
    }
}

/// A torus face bounded by one loop (two circles and the seam between them) is written to STEP bounding the same part of
/// the torus: as a revolve leaves it, mirrored, and with its wire stored out of chain order (bool's Shell case 17488d26,
/// a round's quarter of a torus, 435.1 mm², was written bounding the other three quarters, as live read it).
#[test]
fn a_one_loop_torus_face_round_trips_through_step() {
    use std::f64::consts::{FRAC_PI_2, TAU};
    let (big, small) = (0.012, 0.005);
    let quarter = TAU * small * (big * FRAC_PI_2 - small) / (TAU * TAU * big * small);
    let round = revolved_round(big, small);
    let mirrored = round.transformed(&Transform::Mirror(Plane::from_normal(DVec3::ZERO, DVec3::Y))).unwrap();
    // The round's torus face with its wire stored seam, inner circle, top circle, seam (the revolve stores them chained).
    let unchained = read_brep(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/step/torus-face-unchained.brep")).unwrap();
    let dir = std::env::temp_dir().join(format!("noshape-torus-step-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for (name, shape) in [("round", round), ("mirrored", mirrored), ("unchained", unchained)] {
        let path = dir.join(format!("{name}.step"));
        write_step(&path, &[ExportShape { shape: shape.clone(), name: Some("Part 1".into()), color: None }], LengthUnit::Meter).unwrap();
        let share = step_torus_band(&std::fs::read_to_string(&path).unwrap());
        assert!((share - quarter).abs() < 1e-6, "{name}: the round's loop bounds {:.4} of the torus, not its {quarter:.4}", share);
        let back = read_step(&path, LengthUnit::Meter).unwrap();
        let (a0, a1) = (shape.area().unwrap(), back[0].shape.area().unwrap());
        assert!((a1 - a0).abs() < 1e-6 * a0, "{name}: area {a1} vs {a0}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}
