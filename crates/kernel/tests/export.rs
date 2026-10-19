//! Export options and writers: STEP schemas and units (read back).

use glam::DVec3;
use noshape_kernel::*;

fn part() -> Shape {
    let b = make_box_corners(DVec3::ZERO, DVec3::new(0.04, 0.02, 0.01)).unwrap();
    let c = make_cylinder(&Plane::from_normal(DVec3::new(0.02, 0.01, -0.001), DVec3::Z), 0.005, 0.012, std::f64::consts::TAU).unwrap();
    cut(&b, &c).unwrap().shape
}

#[test]
fn step_schemas_and_units() {
    let s = part();
    let want = volume_properties(&s).unwrap().mass;
    let dir = std::env::temp_dir().join(format!("noshape-export-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for (schema, tag) in [(StepSchema::Ap242, "AP242"), (StepSchema::Ap214, "AUTOMOTIVE_DESIGN"), (StepSchema::Ap203, "CONFIG_CONTROL_DESIGN")] {
        for (unit, utag) in [(LengthUnit::Millimeter, "MILLI"), (LengthUnit::Inch, "INCH")] {
            let path = dir.join(format!("{tag}-{utag}.step"));
            write_step_with(&path, &[ExportShape { shape: s.clone(), name: Some("Part 1".into()), color: None }], LengthUnit::Meter, &StepOptions { schema, file_unit: Some(unit) }).unwrap();
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
    let lofted = loft(&[ring(0.0, 10.0), sq, ring(30.0, 10.0)], &LoftOptions { solid: true, ruled: false, closed: false, start_points: None }).unwrap().shape;
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
            let v = volume_properties(&back).unwrap().mass;
            // Exact but for tolerant edges rebuilt from their fins' curves (loft:
            // 1.1e-6) and OCCT's B-spline form of a revolved spline (2.9e-6).
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

/// Sample files for checking in Onshape (live capture): every test part as
/// .x_t, .x_b and .sat in $NOSHAPE_XT_SAMPLES (default /tmp/noshape-xt-samples),
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
        list += &format!("{stem}: {:.6} mm3, {} faces\n", volume_properties(&s).unwrap().mass * 1e9, s.count(ShapeType::Face));
    }
    std::fs::write(dir.join("volumes.txt"), list).unwrap();
}

/// ACIS: the records of every test part hang together (pointers to the right
/// record types, coedge rings closed, partners paired with opposite senses,
/// one lump per solid, faces' loops on their faces).
#[test]
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
fn rhino_breps_are_valid() {
    let dir = std::env::temp_dir().join(format!("noshape-3dm-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for (name, s) in xt_parts() {
        for version in [8, 7, 6, 5, 4, 3, 2] {
            let path = dir.join(format!("{}-{version}.3dm", name.replace(' ', "-")));
            let shapes = [ExportShape { shape: s.clone(), name: Some(name.into()), color: None }];
            match write_rhino(&path, &shapes, LengthUnit::Meter, version) {
                Ok(()) => assert_eq!(rhino_brep_count(&path).unwrap_or_else(|e| panic!("{name} v{version}: {e}")), 1, "{name} v{version}"),
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
