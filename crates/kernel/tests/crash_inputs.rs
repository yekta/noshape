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
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/crash");
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
    let (s, subs, p) = load(name);
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
            shell_solid(&s, &subs, after(&p, "thickness", 1)[0], side, ShellMethod::Auto)
        }
        other => panic!("unknown op {other}"),
    };
    match r {
        Ok(r) => format!("ok valid {}", r.shape.is_valid()),
        Err(e) => format!("error {e}"),
    }
}

#[test]
fn crashing_kernel_inputs_return() {
    let _ = set_working_scale(1000.0);
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/crash");
    let mut names: Vec<String> = std::fs::read_dir(&dir).unwrap().filter_map(|e| e.ok()).filter_map(|e| e.file_name().to_str()?.strip_suffix(".brep").map(str::to_string)).collect();
    names.sort();
    assert!(!names.is_empty());
    for n in &names {
        eprintln!("{n}: {}", replay(n));
    }
    // The kernel still works.
    let b = make_box_corners(DVec3::ZERO, DVec3::ONE).unwrap();
    assert!((b.volume().unwrap() - 1.0).abs() < 1e-12);
}
