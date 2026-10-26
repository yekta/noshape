//! Assembly solve latency on large assemblies (perf.yml `model` job, not
//! ci.yml): `cargo test --release -p noshape-ops --test assembly_perf --
//! --ignored --nocapture --test-threads=1`.
//!
//! - 200 pins in 20 stacks of 10 with 300 mates: each pin Planar on the one
//!   below, and from the fifth up also Parallel to its stack's fixed base.
//! - 1000 free pins on a 40 × 25 grid (tests/perf/pins-assembly-1000).
//!
//! Timed as the app runs them: the evaluation after an edit (a mate added:
//! solve, DOF, mate status), and drag steps (one solve per pointer move,
//! no DOF). Targets: a drag step within a 60 Hz frame, adding a mate without
//! a stall (under 100 ms).

use glam::DVec3;
use noshape_model::assembly::eval::SolveOptions;
use noshape_model::assembly::{Assembly, InstanceId, Mate, MateId, MateType, Placement};
use noshape_model::document::TabContent;
use noshape_ops::Ctx;
use noshape_ops::registry;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// One display frame at 60 Hz.
const FRAME: Duration = Duration::from_micros(16_667);
/// Adding a mate shows within this.
const EDIT: Duration = Duration::from_millis(100);

fn call(ctx: &mut Ctx, tool: &str, args: Value) -> Value {
    let op = registry::by_tool_name(tool).unwrap_or_else(|| panic!("no tool {tool}"));
    registry::run(ctx, op, &args).unwrap_or_else(|e| panic!("{tool} failed: {}", e.to_json())).value
}

/// The face of `inst` whose world normal is `n`.
fn face(ctx: &mut Ctx, inst: &str, n: [f64; 3]) -> String {
    let faces = call(ctx, "assembly_entities", json!({ "instance": inst, "kind": "faces" }));
    let f = faces["faces"].as_array().unwrap().iter().find(|f| (0..3).all(|k| (f["normal"][k].as_f64().unwrap_or(9.0) - n[k]).abs() < 1e-6)).expect("face");
    f["id"].as_str().unwrap().to_string()
}

struct Stats {
    median: Duration,
    worst: Duration,
}

fn stats(mut t: Vec<Duration>) -> Stats {
    t.sort();
    Stats { median: t[t.len() / 2], worst: *t.last().unwrap() }
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1e3
}

/// A document with a pin (r 2 mm, 10 mm) Part Studio and an Assembly built
/// by `build` from two inserted pins and their Planar mate (bottom on top).
fn assembly(name: &str, build: impl FnOnce(&mut Assembly)) -> (Ctx, String) {
    let dir = std::env::temp_dir().join(format!("noshape-asm-perf-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let scratch = dir.join(format!("{name}-seed.noshape"));
    let out = dir.join(format!("{name}.noshape"));
    let _ = std::fs::remove_file(&scratch);
    let _ = std::fs::remove_file(&out);
    let mut ctx = Ctx::new();
    call(&mut ctx, "document_create", json!({ "path": scratch.to_str().unwrap() }));
    let sk = call(&mut ctx, "sketch_create", json!({ "plane": "plane:top" }))["sketch"].as_str().unwrap().to_string();
    let st = call(&mut ctx, "sketch_draw", json!({ "sketch": sk, "tool": "circle", "points": [[0, 0], [2, 0]] }));
    let region = st["regions"][0]["id"].as_str().unwrap().to_string();
    call(&mut ctx, "feature_add", json!({ "type": "extrude", "params": { "entities": [region], "depth": "10 mm" } }));
    call(&mut ctx, "assembly_insert", json!({ "source": "Part Studio 1", "part": "Part 1" }));
    call(&mut ctx, "assembly_insert", json!({ "source": "Part Studio 1", "part": "Part 1" }));
    call(&mut ctx, "assembly_set_state", json!({ "instances": ["Part 1 <1>"], "fixed": true }));
    call(&mut ctx, "assembly_transform", json!({ "instance": "Part 1 <2>", "translation": [0, 0, 10] }));
    let a = face(&mut ctx, "Part 1 <2>", [0.0, 0.0, -1.0]);
    let b = face(&mut ctx, "Part 1 <1>", [0.0, 0.0, 1.0]);
    call(&mut ctx, "assembly_mate", json!({ "type": "planar", "a": { "instance": "Part 1 <2>", "entity": a, "inference": "centroid" }, "b": { "instance": "Part 1 <1>", "entity": b, "inference": "centroid" } }));
    let mut doc = ctx.doc(scratch.to_str()).unwrap().doc().clone();
    let tab = noshape_ops::doc::find_tab(&doc, "Assembly 1").unwrap();
    let Some(TabContent::Assembly(asm)) = doc.tab_mut(tab).map(|t| &mut t.content) else { panic!("Assembly 1") };
    build(asm);
    ctx.create(&out, doc).unwrap();
    (ctx, out.to_str().unwrap().to_string())
}

/// `asm`'s seed pin (instance 2) copied to `at` (mm), fixed or not.
fn pin_at(asm: &mut Assembly, seed: &noshape_model::assembly::Instance, at: DVec3, fixed: bool) -> InstanceId {
    let id = InstanceId(asm.next_id());
    // (Named directly: next_instance_name scans every name.)
    let name = format!("Part 1 <{}>", asm.instances.len() + 1);
    asm.instances.push(noshape_model::assembly::Instance { id, name, fixed, placement: Placement { translation: (at * 1e-3).into(), ..Placement::default() }, ..seed.clone() });
    id
}

/// A copy of the seed Planar mate between `a` (its bottom) and `b` (top).
fn mate(asm: &mut Assembly, seed: &Mate, kind: MateType, a: InstanceId, b: InstanceId) {
    let id = MateId(asm.next_id());
    let mut m = seed.clone();
    m.id = id;
    m.name = format!("{kind:?} {}", id.0);
    m.mate_type = kind;
    m.a.instance = a;
    m.b.instance = b;
    asm.mates.push(m);
}

/// Drag `inst`'s top centre in `steps` pointer moves along `dir` (mm per
/// step), each a solve as the app runs it (from the last step's poses, no
/// DOF).
fn drag(ctx: &mut Ctx, path: &str, inst: InstanceId, dir: DVec3, steps: usize) -> Stats {
    let od = ctx.doc(Some(path)).unwrap();
    let tab = noshape_ops::doc::find_tab(od.doc(), "Assembly 1").unwrap();
    let Some(TabContent::Assembly(asm)) = od.doc().tab(tab).map(|t| t.content.clone()) else { panic!() };
    let res = od.evaluate_assembly(tab, &asm, &SolveOptions::default());
    let local = DVec3::new(0.0, 0.0, 0.01);
    let p0 = res.solved.placement(inst).unwrap();
    let grab = DVec3::from(p0.translation) + local;
    let mut start: HashMap<InstanceId, Placement> = res.solved.placements.iter().cloned().collect();
    let mut times = Vec::new();
    for i in 1..=steps {
        let target = grab + dir * 1e-3 * i as f64;
        let t = Instant::now();
        let s = res.prepared.solve(&SolveOptions { drags: vec![(inst, local, target)], start: Some(start.clone()), skip_dof: true, ..Default::default() });
        times.push(t.elapsed());
        start = s.placements.iter().cloned().collect();
    }
    stats(times)
}

fn evaluate(ctx: &mut Ctx, path: &str, edit: impl FnOnce(&mut Assembly)) -> Duration {
    let od = ctx.doc(Some(path)).unwrap();
    let tab = noshape_ops::doc::find_tab(od.doc(), "Assembly 1").unwrap();
    let Some(TabContent::Assembly(mut asm)) = od.doc().tab(tab).map(|t| t.content.clone()) else { panic!() };
    edit(&mut asm);
    let t = Instant::now();
    let res = od.evaluate_assembly(tab, &asm, &SolveOptions::default());
    let d = t.elapsed();
    assert!(res.mates.iter().all(|m| m.ok), "a mate fails");
    d
}

#[test]
#[ignore]
fn assembly_drag_and_mate_latency() {
    // 20 stacks of 10 pins, 300 mates.
    let mut tops = Vec::new();
    let (mut ctx, path) = assembly("stacks", |asm| {
        let seed = asm.instances[1].clone();
        let template = asm.mates[0].clone();
        asm.instances.clear();
        asm.mates.clear();
        for s in 0..20 {
            let x = s as f64 * 20.0;
            let base = pin_at(asm, &seed, DVec3::new(x, 0.0, 0.0), true);
            let mut below = base;
            for k in 1..10 {
                let p = pin_at(asm, &seed, DVec3::new(x + 0.3 * k as f64, 0.2 * k as f64, 10.0 * k as f64), false);
                mate(asm, &template, MateType::Planar, p, below);
                if k >= 4 {
                    mate(asm, &template, MateType::Parallel, p, base);
                }
                below = p;
            }
            tops.push(below);
        }
        assert_eq!(asm.instances.len(), 200);
        assert_eq!(asm.mates.len(), 300);
    });
    let first = evaluate(&mut ctx, &path, |_| {});
    let again = stats((0..7).map(|_| evaluate(&mut ctx, &path, |_| {})).collect()).median;
    println!("200 instances, 300 mates: first evaluation {:.1} ms (with the Part Studio), again {:.1} ms (median of 7)", ms(first), ms(again));
    let template = {
        let od = ctx.doc(Some(path.as_str())).unwrap();
        let tab = noshape_ops::doc::find_tab(od.doc(), "Assembly 1").unwrap();
        let Some(TabContent::Assembly(asm)) = od.doc().tab(tab).map(|t| t.content.clone()) else { panic!() };
        asm.mates[0].clone()
    };
    // A mate between two stacks' tops (7 pairs, the median).
    let add = stats((0..7).map(|i| evaluate(&mut ctx, &path, |asm| mate(asm, &template, MateType::Parallel, tops[2 * i], tops[2 * i + 1]))).collect()).median;
    println!("  adding a mate: {:.1} ms (median of 7)", ms(add));
    // In the stack's plane (the top pin alone moves), and off it with the
    // pointer in the view plane (the stack's Planar mates hold it down).
    let s = drag(&mut ctx, &path, tops[3], DVec3::new(0.5, 0.3, 0.0), 60);
    println!("  drag a top pin in its plane: median {:.2} ms, worst {:.2} ms", ms(s.median), ms(s.worst));
    let f = drag(&mut ctx, &path, tops[5], DVec3::new(0.5, 0.0, 0.4), 60);
    println!("  drag a top pin off its plane: median {:.2} ms, worst {:.2} ms", ms(f.median), ms(f.worst));

    // 1000 free pins.
    let mut grid = Vec::new();
    let (mut ctx2, path2) = assembly("pins-1000", |asm| {
        let seed = asm.instances[1].clone();
        asm.instances.clear();
        asm.mates.clear();
        for i in 0..1000 {
            grid.push(pin_at(asm, &seed, DVec3::new((i % 40) as f64 * 6.0, (i / 40) as f64 * 6.0, 0.0), false));
        }
    });
    let first2 = evaluate(&mut ctx2, &path2, |_| {});
    let again2 = stats((0..7).map(|_| evaluate(&mut ctx2, &path2, |_| {})).collect()).median;
    println!("1000 free pins: first evaluation {:.1} ms, again {:.1} ms (median of 7)", ms(first2), ms(again2));
    let g = drag(&mut ctx2, &path2, grid[500], DVec3::new(0.5, 0.3, 0.0), 60);
    println!("  drag a pin: median {:.2} ms, worst {:.2} ms", ms(g.median), ms(g.worst));

    assert!(add < EDIT, "adding a mate took {add:?}");
    for (what, s) in [("in-plane drag", &s), ("off-plane drag", &f), ("1000-pin drag", &g)] {
        assert!(s.median < FRAME, "{what}: median {:?}", s.median);
    }
}
