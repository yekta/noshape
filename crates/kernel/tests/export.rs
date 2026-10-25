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
        for (unit, utag) in [(LengthUnit::Millimeter, "MILLI"), (LengthUnit::Inch, "INCH"), (LengthUnit::Yard, "'YARD'")] {
            let path = dir.join(format!("{tag}-{}.step", utag.trim_matches('\'')));
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
            // Parasolid's faces: periodic ones (a revolved spline's spun face too) whole.
            assert_eq!(back.count(ShapeType::Face), s.count(ShapeType::Face), "{name} binary={binary}: faces");
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

/// Parasolid versions (the Export dialog's list): each writes the body in
/// that version's layouts with Onshape's header (live G36: one key per line,
/// part 2 the writer's schema, the first body line naming the version's
/// schema), and reads back with the same volume.
#[test]
fn parasolid_versions() {
    let dir = std::env::temp_dir().join(format!("noshape-xtv-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let (_, s) = xt_parts().into_iter().find(|p| p.0 == "rounded block").expect("rounded block");
    let want = volume_properties(&s).unwrap().mass;
    // Schema per version, as Onshape's files name it (samples/g36-g24b-v*.x_t).
    let schemas = [("38.1", "_38101"), ("33.0", "_32001"), ("29.0", "_28101"), ("26.1", "_26105"), ("25.0", "_25001"), ("12.0", "_12006")];
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
            for key in ["**PART1;\nMC=unknown;\n", "\nFORMAT=text;\nGUISE=transmit;\n", &format!("\nFILE=v{v}.x_t;\n"), "**PART2;\nSCH=SCH_3801000_38101;\nUSFLD_SIZE=0;\n**PART3;\n"] {
                assert!(head.contains(key), "{v}: header lacks {key:?}: {head}");
            }
            let first = body.lines().nth(1).unwrap();
            assert!(first.starts_with("T51 : TRANSMIT FILE created by modeller version "), "{v}: {first}");
            if let Some((_, sch)) = schemas.iter().find(|x| x.0 == v) {
                assert!(first.contains(sch), "{v}: {first}");
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
    assert!(lines[1].starts_with(&format!("File generated by NOSHAPE Version {}", env!("CARGO_PKG_VERSION"))), "{}", lines[1]);
    assert!(lines.iter().all(|l| l.len() == 80), "80-column records");
    let global: String = lines.iter().filter(|l| &l[72..73] == "G").map(|l| &l[..72]).collect();
    assert!(global.starts_with("1H,,1H;,14Hblock - Part 1,19Hblock - Part 1.iges,7HNOSHAPE,"), "{global}");
    assert!(global.contains(",32,24,8,56,15,14Hblock - Part 1,1.,2,2HMM,1000,1.,13H"), "{global}");
    assert!(global.contains(",9,0,13H"), "{global}");
    // Sequence numbers right-aligned.
    assert!(lines.iter().any(|l| l.ends_with("D      1")), "{}", lines.iter().find(|l| &l[72..73] == "D").unwrap());
    let back = read_iges(&path, LengthUnit::Meter).unwrap();
    let got = volume_properties(&back).unwrap().mass;
    assert!((got - want).abs() < 1e-6 * want, "{got} vs {want}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// ACIS import: our own .sat of every test part back with its volume, and
/// Onshape's SAT exports (outside the repo; skipped when absent).
#[test]
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
                Ok(()) => {
                    assert_eq!(rhino_brep_count(&path).unwrap_or_else(|e| panic!("{name} v{version}: {e}")), 1, "{name} v{version}");
                    if version == 8 {
                        // And back through our reader: the same solid, named.
                        let back = read_rhino_parts(&path, LengthUnit::Meter).unwrap_or_else(|e| panic!("{name}: {e}"));
                        assert_eq!(back.len(), 1, "{name}");
                        assert_eq!(back[0].name.as_deref(), Some(name), "{name}");
                        let (v, want) = (volume_properties(&back[0].shape).unwrap().mass, volume_properties(&s).unwrap().mass);
                        assert!(back[0].shape.is_valid() && (v - want).abs() < 2e-5 * want, "{name}: {v} vs {want}");
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

/// PVZ import (Onshape imports PVZ as meshes only, live G22/G23): our own
/// files read back as closed mesh solids within tessellation error of the
/// exact volume; Onshape's samples (outside the repo, skipped when absent)
/// give Onshape's own mesh volumes, and the loft whose face meshes leave
/// gaps stays open.
#[test]
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
    write_step_with(&path, &parts, LengthUnit::Meter, &StepOptions { schema: StepSchema::Ap242, file_unit: None }).unwrap();
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
    write_step_with(&p214, &one, LengthUnit::Meter, &StepOptions { schema: StepSchema::Ap214, file_unit: None }).unwrap();
    let t = std::fs::read_to_string(&p214).unwrap();
    assert!(t.contains("/* description */ ('STEP AP214',\n'CAx-IF Rec.Pracs.---User Defined Attributes---1.5---2016-08-15'),"));
    assert!(t.contains("FILE_SCHEMA (('AUTOMOTIVE_DESIGN {1 0 10303 214 3 1 1}'));"));
    assert!(t.contains("\nDATA;\n#10=PROPERTY_DEFINITION_REPRESENTATION(#14,#12);\n"), "{}", &t[..1500]);
    assert!(t.contains("=VALUE_REPRESENTATION_ITEM('number of annotations',COUNT_MEASURE(0.));"));
    assert!(t.contains("=APPLICATION_CONTEXT(\n'core data for automotive mechanical design processes');"));
    let p203 = dir.join("a203.step");
    write_step_with(&p203, &one, LengthUnit::Meter, &StepOptions { schema: StepSchema::Ap203, file_unit: None }).unwrap();
    let t = std::fs::read_to_string(&p203).unwrap();
    assert!(t.contains("/* description */ ('STEP AP203'),"));
    assert!(t.contains("FILE_SCHEMA (('CONFIG_CONTROL_DESIGN'));"));
    assert!(t.contains("=DESIGN_CONTEXT('',#") && t.contains("=MECHANICAL_CONTEXT('',#"));
    assert!(t.contains("=APPLICATION_CONTEXT(\n'configuration controlled 3D designs of mechanical parts and assemblie\ns');"));
    let pin = dir.join("inch.step");
    write_step_with(&pin, &one, LengthUnit::Meter, &StepOptions { schema: StepSchema::Ap242, file_unit: Some(LengthUnit::Inch) }).unwrap();
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
/// `cargo test -p noshape-kernel --test export write_g47_step -- --ignored`.
#[test]
#[ignore]
fn write_g47_step() {
    let Some(dir) = std::env::var_os("NOSHAPE_STEP_OUT") else { return };
    let colors = [[0.6157, 0.8118, 0.9294], [0.6471, 0.6471, 0.6471], [0.2314, 0.3804, 0.7059]];
    let parts: Vec<ExportShape> = (0..3).map(|i| ExportShape { shape: make_box_corners(DVec3::new(0.02 * i as f64, 0.0, 0.0), DVec3::new(0.02 * i as f64 + 0.01, 0.01, 0.01)).unwrap(), name: Some(format!("Part {}", i + 1)), color: Some(colors[i]) }).collect();
    write_step_with(&std::path::Path::new(&dir).join("g47 3parts.step"), &parts, LengthUnit::Meter, &StepOptions { schema: StepSchema::Ap242, file_unit: None }).unwrap();
}

/// Prints the volume and faces (kind, area mm², box mm) of the parts of the
/// Parasolid file $NOSHAPE_XT (comparing Onshape's bodies with ours):
/// `cargo test -p noshape-kernel --test export report_xt -- --ignored --nocapture`.
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
