//! Kernel inputs that crashed the process in qa's fuzzed documents (tickets
//! FILLET-SEGV, FILLET-SEGV-BLEND, DRAFT-SEGV-DOC, SHELL-SLAB-GPF), captured
//! with NOSHAPE_DUMP_OPS (common.cpp `dump_op`): each op must return, Ok or
//! an error, and leave the kernel usable. fillet-blend-segv still crashes
//! inside OCCT's surface/restriction blend and returns through the crash
//! guard; the others return without it (OCCT patch 0002).

use noshape_kernel::*;
use std::path::Path;

/// (input shape, its face / edge arguments, the op's parameter line).
fn load(name: &str) -> (Shape, Vec<Shape>, Vec<String>) {
    let crash = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/crash");
    let (dir, name) = match Path::new(name).parent() {
        Some(d) if !d.as_os_str().is_empty() => (d.to_path_buf(), Path::new(name).file_name().unwrap().to_str().unwrap()),
        _ => (crash, name),
    };
    let s = read_brep(&dir.join(format!("{name}.brep"))).unwrap();
    let (faces, edges) = (s.faces(), s.edges());
    let text = std::fs::read_to_string(dir.join(format!("{name}.txt"))).unwrap();
    let mut subs = Vec::new();
    let mut params = Vec::new();
    for line in text.lines() {
        let w: Vec<&str> = line.split_whitespace().collect();
        match w.as_slice() {
            ["face", i] => subs.push(faces[i.parse::<usize>().unwrap()].clone()),
            ["edge", i] => subs.push(edges[i.parse::<usize>().unwrap()].clone()),
            _ => params = w.iter().map(|x| x.to_string()).collect(),
        }
    }
    (s, subs, params)
}

fn after(p: &[String], key: &str, n: usize) -> Vec<f64> {
    let i = p.iter().position(|x| x == key).unwrap_or_else(|| panic!("no {key} in {p:?}"));
    p[i + 1..i + 1 + n].iter().map(|x| x.parse().unwrap()).collect()
}

fn v3(v: &[f64]) -> DVec3 {
    DVec3::new(v[0], v[1], v[2])
}

/// Runs the captured op; returns its outcome as text (the op must return).
fn replay(name: &str) -> String {
    replay_as(name, true)
}

/// [`replay`]; `describe` false: the op only (profiling).
fn replay_as(name: &str, describe: bool) -> String {
    let (s, subs, p) = load(name);
    if p[0] == "project_along" {
        let parts = s.children();
        return match project_curves(&parts[0], &parts[1], Some(v3(&after(&p, "dir", 3)))) {
            Ok(r) => format!("ok edges {} length {:.9e}", r.edges().len(), r.edges().iter().map(|e| e.length().unwrap_or(0.0)).sum::<f64>()),
            Err(e) => format!("error {e}"),
        };
    }
    let r = match p[0].as_str() {
        "fillet" => {
            let radii: Vec<f64> = p[p.iter().position(|x| x == "radii").unwrap() + 1..].iter().map(|x| x.parse().unwrap()).collect();
            if p[2] == "1" { fillet_checked(&s, &subs, &radii) } else { fillet(&s, &subs, radii[0]) }
        }
        "draft" => {
            let plane = Plane::new(v3(&after(&p, "origin", 3)), v3(&after(&p, "normal", 3)), v3(&after(&p, "xdir", 3)));
            draft(&s, &subs, v3(&after(&p, "pull", 3)), &plane, after(&p, "angle", 1)[0])
        }
        "shell" => {
            let side = if p[4] == "1" { ShellSide::Outside } else { ShellSide::Inside };
            // NOSHAPE_REPLAY_METHOD: 1 offset only, 2 slabs only (compare paths).
            let method = match std::env::var("NOSHAPE_REPLAY_METHOD").as_deref() {
                Ok("1") => ShellMethod::Offset,
                Ok("2") => ShellMethod::Slabs,
                _ => ShellMethod::Auto,
            };
            shell_solid(&s, &subs, after(&p, "thickness", 1)[0], side, method)
        }
        "prism_until" => {
            let parts = s.children();
            extrude_up_to(&parts[0], v3(&after(&p, "dir", 3)), &parts[1], after(&p, "start_offset", 1)[0])
        }
        "unify" => unify_same_domain(&s),
        "defeature" => delete_faces(&s, &subs),
        "offset" => offset_faces(&s, &subs, after(&p, "offset", 1)[0]),
        other => panic!("unknown op {other}"),
    };
    match r {
        Ok(_) if !describe => "ok".into(),
        Ok(r) => format!("ok valid {} volume {:.9e} faces {}", r.shape.is_valid(), r.shape.volume().unwrap_or(0.0), r.shape.faces().len()),
        Err(e) => format!("error {e}"),
    }
}

// fillet-blend-segv returns only through the crash guard (signals; structured exceptions on Windows,
// cpp/seh.cpp).
#[test]
fn crashing_kernel_inputs_return() {
    let _ = set_working_scale(1000.0);
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/crash");
    let mut names: Vec<String> = std::fs::read_dir(&dir).unwrap().filter_map(|e| e.ok()).filter_map(|e| e.file_name().to_str()?.strip_suffix(".brep").map(str::to_string)).collect();
    names.sort();
    assert!(!names.is_empty());
    for n in &names {
        // Uncaptured: the last line names the input if one kills the process.
        let _ = std::io::Write::write_all(&mut std::io::stderr(), format!("crash input {n}\n").as_bytes());
        eprintln!("{n}: {}", replay(n));
    }
    // The kernel still works.
    let b = make_box_corners(DVec3::ZERO, DVec3::ONE).unwrap();
    assert!((b.volume().unwrap() - 1.0).abs() < 1e-12);
}

/// Replays one NOSHAPE_DUMP_OPS capture (fillet, draft, shell, prism_until, unify, defeature, offsetfaces, project_along) and times it:
/// `NOSHAPE_REPLAY=<dir>/<name> cargo test -p noshape-kernel --test all crash_inputs::replay_one -- --ignored --nocapture`.
#[test]
#[ignore = "T2: a tool; needs NOSHAPE_REPLAY"]
fn replay_one() {
    let _ = set_working_scale(1000.0);
    let Ok(name) = std::env::var("NOSHAPE_REPLAY") else { return };
    // NOSHAPE_REPLAY_REPEAT=n: n times (profiling); the last one's time.
    let n: usize = std::env::var("NOSHAPE_REPLAY_REPEAT").ok().and_then(|v| v.parse().ok()).unwrap_or(1);
    for _ in 1..n {
        replay_as(&name, false);
    }
    let t = std::time::Instant::now();
    let r = replay(&name);
    eprintln!("{name}: {r} in {:.1} ms", t.elapsed().as_secs_f64() * 1e3);
    kernel_profile_mark("replay");
}

/// qa's cli-draft-die Draft 5 (BLEND-RUST-DEFAULT-REGEN): a 0.1 mm chamfer face drafted 30° that OCCT's draft
/// can't do, so the draft wedges do it. The same solid with OCCT's blends and with noshape's (another frame for
/// the face's plane, other vertices at its 0.1 mm tolerance) gave no wedge on the first and +1.66 mm³ on the
/// second; the wedge (the face, 0.204 mm², times ~8.1 mm between its plane and the drafted one) is the same on
/// both now.
#[test]
fn draft_wedges_of_a_chamfer_sliver_are_the_same_on_both_inputs() {
    let _ = set_working_scale(1000.0);
    let mut got = Vec::new();
    for name in ["draft-die-occt-blends", "draft-die-noshape-blends"] {
        let (s, subs, p) = load(name);
        let plane = Plane::new(v3(&after(&p, "origin", 3)), v3(&after(&p, "normal", 3)), v3(&after(&p, "xdir", 3)));
        let r = draft_wedges(&s, &subs, v3(&after(&p, "pull", 3)), &plane, after(&p, "angle", 1)[0]).unwrap();
        assert!(r.shape.is_valid(), "{name}");
        let grown = (volume_properties(&r.shape).unwrap().mass - volume_properties(&s).unwrap().mass) * 1e9;
        got.push((grown, r.shape.count(ShapeType::Face)));
    }
    assert!((got[0].0 - got[1].0).abs() < 1e-6 && (got[0].0 - 1.6629).abs() < 1e-3, "{got:?}");
    assert_eq!((got[0].1, got[1].1), (25, 25));
}

/// Fuzz seed 1151's Draft (RUST-EXTRUDE-SCOPE-SLOW): OCCT's draft of this valid part left a vertex of tolerance
/// 1.3e154 in a result its checker called valid, and every later Boolean's classification on it ground for minutes
/// (an extrude's collision check: 7 min, then the extrude failed). The draft fails on it now, and the wedge draft
/// (draft_robust's fallback) builds it.
#[test]
fn a_draft_leaving_a_huge_tolerance_fails_and_the_wedges_build_it() {
    let _ = set_working_scale(1000.0);
    let (s, subs, p) = load("draft-huge-tolerance");
    let plane = Plane::new(v3(&after(&p, "origin", 3)), v3(&after(&p, "normal", 3)), v3(&after(&p, "xdir", 3)));
    let (pull, angle) = (v3(&after(&p, "pull", 3)), after(&p, "angle", 1)[0]);
    let e = draft(&s, &subs, pull, &plane, angle).expect_err("OCCT's draft taken with its huge tolerance");
    assert!(e.to_string().contains("tolerance"), "{e}");
    let r = draft_wedges(&s, &subs, pull, &plane, angle).expect("the wedge draft");
    assert!(r.shape.is_valid());
}

/// asm-bjr-config 5d8bc1's Kerf Compensation 1 (dx): a 1.5 mm triangular plate's three sides, three countersinks and
/// their bores offset out by 0.09 mm. OCCT's offset in metres came back empty ("Could not offset selected faces as
/// requested."); in millimetres it builds: 11 faces, the input's 3685.749 mm³ plus ~0.09 mm over ~483 mm² of faces.
#[test]
fn a_plates_sides_and_countersinks_offset_by_a_kerf() {
    let _ = set_working_scale(1000.0);
    let (s, subs, _) = load("kerf-offset");
    let r = offset_faces(&s, &subs, 0.00009).unwrap().shape;
    assert!(r.is_valid());
    assert_eq!(r.faces().len(), 11);
    let v = r.volume().unwrap() * 1e9;
    assert!((v - 3729.04).abs() < 0.01, "{v} mm³");
}

#[test]
fn open_edges_of_an_open_solid() {
    let open = read_brep(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/open-box.brep")).unwrap();
    assert_eq!(open.open_edges().unwrap(), 4);
    let cube = make_box_corners(DVec3::ZERO, DVec3::splat(0.01)).unwrap();
    assert_eq!(cube.open_edges().unwrap(), 0);
    let cylinder = make_cylinder(&Plane::xy(), 0.005, 0.01, std::f64::consts::TAU).unwrap();
    assert_eq!(cylinder.open_edges().unwrap(), 0, "a seam bounds one face twice");
}
