//! Big assemblies against Onshape (user 11-01: 200 / 1000 instances, a few
//! thousand mates, patterns, subassemblies, standard content): open,
//! regenerate, mate solve, drag frame and insert, timed as the app runs them.
//! `tools/tiers/perf.sh -p noshape-ops --test all big_assembly_perf:: -- --ignored --nocapture --test-threads=1`
//! (memory-heavy: `NOSHAPE_RB_HOST=vibe4` for rb.sh runs).
//!
//! The case (built by [`build`]): stacks of pins, each pin Planar on the one
//! below, Parallel to its stack's fixed base and Parallel to the one below
//! (redundant, as models often are); subassembly instances (a stack of 10
//! pins with 9 Planar mates inside), each Fastened to a fixed pin of its
//! own; standard content bolts Fastened on stack pins; a linear pattern of
//! the first subassembly. Numbers go to docs/handoff/skasm.md with live's.

use super::assembly_perf::{Stats, assembly, call, drag, evaluate, evaluated, face, fold_solved, mate, ms, pin_at, scene, stats};
use glam::DVec3;
use noshape_model::assembly::standard::{StandardSpec, default_component};
use noshape_model::assembly::{Assembly, Instance, InstanceId, InstanceSource, MateType, Placement};
use noshape_model::document::TabContent;
use noshape_ops::Ctx;
use noshape_scene::asm_scene::AsmGpuCache;
use serde_json::json;
use std::time::{Duration, Instant};

/// A case's size.
#[derive(Clone, Copy)]
struct Case {
    name: &'static str,
    stacks: usize,
    height: usize,
    subs: usize,
    bolts: usize,
    pattern: u32,
}

/// What [`build`] made.
struct Built {
    ctx: Ctx,
    path: String,
    /// The top pin of each stack.
    tops: Vec<InstanceId>,
    instances: usize,
    occurrences: usize,
    mates: usize,
}

/// Pins in each subassembly.
const SUB_PINS: usize = 10;

fn build(case: Case) -> Built {
    let mut tops = Vec::new();
    let mut sub_tab = None;
    let (mut ctx, path) = assembly(case.name, |asm| {
        let seed = asm.instances[1].clone();
        let template = asm.mates[0].clone();
        asm.instances.clear();
        asm.mates.clear();
        for s in 0..case.stacks {
            let (x, y) = ((s % 30) as f64 * 20.0, (s / 30) as f64 * 20.0);
            let base = pin_at(asm, &seed, DVec3::new(x, y, 0.0), true);
            let mut below = base;
            for k in 1..case.height {
                let p = pin_at(asm, &seed, DVec3::new(x + 0.3 * k as f64, y + 0.2 * k as f64, 10.0 * k as f64), false);
                mate(asm, &template, MateType::Planar, p, below);
                mate(asm, &template, MateType::Parallel, p, base);
                if k >= 2 {
                    mate(asm, &template, MateType::Parallel, p, below);
                }
                below = p;
            }
            tops.push(below);
        }
        // Standard content bolts on the stacks' second pins (their tops).
        let spec = StandardSpec::new(default_component());
        for b in 0..case.bolts {
            let host = InstanceId(asm.instances[(b % case.stacks) * case.height + 1].id.0);
            let id = InstanceId(asm.next_id());
            asm.instances.push(Instance::new(id, format!("{} <{}>", spec.part_name(), b + 1), InstanceSource::Standard { spec: spec.clone() }, Placement { translation: [0.0, 0.0, -0.05 - 0.01 * b as f64], ..Placement::default() }));
            let mut m = template.clone();
            m.id = noshape_model::assembly::MateId(asm.next_id());
            m.name = format!("Fastened bolt {b}");
            m.mate_type = MateType::Fastened;
            m.a = noshape_model::assembly::MateConnectorRef::on(id, vec![], noshape_model::reference::Reference::Origin);
            m.b.instance = host;
            asm.mates.push(m);
        }
    });
    // The subassembly: a stack of pins (a tab of its own), inserted `subs` times.
    if case.subs > 0 {
        let od = ctx.doc(Some(path.as_str())).unwrap();
        let mut doc = od.doc().clone();
        let top = noshape_ops::doc::find_tab(&doc, "Assembly 1").unwrap();
        let Some(TabContent::Assembly(top_asm)) = doc.tab(top).map(|t| t.content.clone()) else { panic!() };
        let seed = top_asm.instances[1].clone();
        let template = top_asm.mates[0].clone();
        let mut sub = Assembly::default();
        let mut below = pin_at(&mut sub, &seed, DVec3::ZERO, true);
        let sub_base = below;
        for k in 1..SUB_PINS {
            let p = pin_at(&mut sub, &seed, DVec3::new(0.0, 0.0, 10.0 * k as f64), false);
            mate(&mut sub, &template, MateType::Planar, p, below);
            below = p;
        }
        let tab = doc.add_tab("Sub", TabContent::Assembly(Box::new(sub)));
        sub_tab = Some(tab);
        let Some(TabContent::Assembly(asm)) = doc.tab_mut(top).map(|t| &mut t.content) else { panic!() };
        for s in 0..case.subs {
            let at = DVec3::new((s % 30) as f64 * 20.0, -100.0 - (s / 30) as f64 * 20.0, 0.0);
            let ground = pin_at(asm, &seed, at, true);
            let id = InstanceId(asm.next_id());
            asm.instances.push(Instance::new(id, format!("Sub <{}>", s + 1), InstanceSource::Assembly { assembly: tab, config: Default::default() }, Placement { translation: ((at + DVec3::new(3.0, 3.0, 12.0)) * 1e-3).into(), ..Placement::default() }));
            // The sub's bottom pin Fastened on its ground pin's top.
            let mut m = template.clone();
            m.id = noshape_model::assembly::MateId(asm.next_id());
            m.name = format!("Fastened sub {s}");
            m.mate_type = MateType::Fastened;
            m.a.instance = id;
            m.a.path = vec![sub_base];
            m.b.instance = ground;
            asm.mates.push(m);
        }
        let od = ctx.doc(Some(path.as_str())).unwrap();
        od.session.edit_document("Subassemblies", |d| *d = doc).unwrap();
    }
    // A linear pattern of the first subassembly, along the first base's top normal.
    if case.pattern > 1 && case.subs > 0 {
        let up = face(&mut ctx, "Part 1 <1>", [0.0, 0.0, 1.0]);
        call(&mut ctx, "assembly_pattern", json!({ "doc": path, "kind": "linear", "seeds": ["Sub <1>"], "direction": { "instance": "Part 1 <1>", "entity": up }, "distance": "120 mm", "count": case.pattern }));
    }
    let res = evaluated(&mut ctx, &path);
    let od = ctx.doc(Some(path.as_str())).unwrap();
    let top = noshape_ops::doc::find_tab(od.doc(), "Assembly 1").unwrap();
    let Some(TabContent::Assembly(asm)) = od.doc().tab(top).map(|t| t.content.clone()) else { panic!() };
    let _ = sub_tab;
    Built { instances: asm.instances.len(), occurrences: res.occurrences.len(), mates: asm.mates.len(), ctx, path, tops }
}

/// Open the saved document in a fresh session as the app does: load, the
/// first evaluation (the Part Studio regenerated, the solve from the saved
/// placements) and the scene; each step's time.
fn open(path: &str) -> (Duration, Duration, Option<Duration>) {
    // As a fresh app process: no DOF analysis cached.
    noshape_model::assembly::solver::clear_dof_cache();
    let mut ctx = Ctx::new();
    let t = Instant::now();
    call(&mut ctx, "document_open", json!({ "path": path }));
    let load = t.elapsed();
    let t = Instant::now();
    let res = evaluated(&mut ctx, path);
    let first = t.elapsed();
    let scene = scene(&mut ctx, &res, &mut AsmGpuCache::default()).map(|s| s.0);
    (load, first, scene)
}

/// Regenerate: the pin's Extrude depth edited (the Part Studio regenerated),
/// then the assembly evaluated with the moved faces.
fn regenerate(ctx: &mut Ctx, path: &str, depth: &str) -> Duration {
    let t = Instant::now();
    call(ctx, "feature_edit", json!({ "doc": path, "tab": "Part Studio 1", "feature": "Extrude 1", "params": { "depth": depth } }));
    let edit = t.elapsed();
    let _ = evaluated(ctx, path);
    println!("    (the edit {:.1} ms, the assembly {:.1} ms)", ms(edit), ms(t.elapsed() - edit));
    t.elapsed()
}

fn run(case: Case) {
    let t = Instant::now();
    let mut b = build(case);
    let made = t.elapsed();
    fold_solved(&mut b.ctx, &b.path);
    println!(
        "{}: {} instances ({} occurrences), {} mates; built in {:.1} s",
        case.name,
        b.instances,
        b.occurrences,
        b.mates,
        made.as_secs_f64()
    );
    let (load, first, scene_t) = open(&b.path);
    println!("  open: load {:.1} ms, first evaluation {:.1} ms, scene {}", ms(load), ms(first), scene_t.map(|d| format!("{:.1} ms", ms(d))).unwrap_or("(no GPU)".into()));
    let again = stats((0..5).map(|_| evaluate(&mut b.ctx, &b.path, |_| {})).collect()).median;
    println!("  evaluate again: {:.1} ms (median of 5)", ms(again));
    let regen = stats((0..3).map(|i| regenerate(&mut b.ctx, &b.path, if i % 2 == 0 { "12 mm" } else { "10 mm" })).collect()).median;
    println!("  regenerate (pin depth edited): {:.1} ms (median of 3)", ms(regen));
    fold_solved(&mut b.ctx, &b.path);
    let template = {
        let od = b.ctx.doc(Some(b.path.as_str())).unwrap();
        let tab = noshape_ops::doc::find_tab(od.doc(), "Assembly 1").unwrap();
        let Some(TabContent::Assembly(asm)) = od.doc().tab(tab).map(|t| t.content.clone()) else { panic!() };
        asm.mates[0].clone()
    };
    let tops = b.tops.clone();
    let pairs = tops.len() / 2;
    let add = stats((0..5).map(|i| evaluate(&mut b.ctx, &b.path, |asm| mate(asm, &template, MateType::Parallel, tops[2 * (i % pairs)], tops[2 * (i % pairs) + 1]))).collect()).median;
    println!("  add a mate (solve): {:.1} ms (median of 5)", ms(add));
    let (s, frame): (Stats, Option<Stats>) = drag(&mut b.ctx, &b.path, tops[3], DVec3::new(0.5, 0.3, 0.0), 40);
    let fps = |d: Duration| 1.0 / d.as_secs_f64();
    println!(
        "  drag a top pin: solve median {:.2} ms (worst {:.2}){}",
        ms(s.median),
        ms(s.worst),
        frame.as_ref().map(|f| format!(", frame {:.2} ms = {:.0} fps (worst {:.2} ms)", ms(f.median), fps(f.median), ms(f.worst))).unwrap_or_default()
    );
    let insert = stats((0..3).map(|i| evaluate(&mut b.ctx, &b.path, |asm| {
        let seed = asm.instances[1].clone();
        pin_at(asm, &seed, DVec3::new(-200.0 - 10.0 * i as f64, 0.0, 0.0), false);
    })).collect()).median;
    println!("  insert a pin: {:.1} ms (median of 3)", ms(insert));
}

const A200: Case = Case { name: "a200", stacks: 8, height: 15, subs: 20, bolts: 30, pattern: 6 };
const A1000: Case = Case { name: "a1000", stacks: 40, height: 20, subs: 60, bolts: 80, pattern: 10 };

#[test]
#[ignore]
fn big_assembly_200() {
    run(A200);
}

#[test]
#[ignore]
fn big_assembly_1000() {
    run(A1000);
}

/// A1000's drag alone, long (to profile a frame: `perf record` this test).
#[test]
#[ignore]
fn big_assembly_1000_drag_profile() {
    let mut b = build(A1000);
    fold_solved(&mut b.ctx, &b.path);
    let (s, frame) = drag(&mut b.ctx, &b.path, b.tops[3], DVec3::new(0.05, 0.03, 0.0), 600);
    println!("a1000 drag: solve median {:.2} ms, frame {}", ms(s.median), frame.map(|f| format!("{:.2} ms", ms(f.median))).unwrap_or_default());
}

/// A1000 evaluated again and again with nothing changed (to profile what
/// every edit pays: `perf record` this test).
#[test]
#[ignore]
fn big_assembly_1000_evaluate_profile() {
    let mut b = build(A1000);
    fold_solved(&mut b.ctx, &b.path);
    let again = stats((0..300).map(|_| evaluate(&mut b.ctx, &b.path, |_| {})).collect());
    println!("a1000 evaluate again: median {:.1} ms", ms(again.median));
}

/// A1000 opened again and again in fresh sessions (to profile opening:
/// `perf record` this test).
#[test]
#[ignore]
fn big_assembly_1000_open_profile() {
    let mut b = build(A1000);
    fold_solved(&mut b.ctx, &b.path);
    let firsts: Vec<Duration> = (0..20).map(|_| open(&b.path).1).collect();
    println!("a1000 open, first evaluation: median {:.1} ms", ms(stats(firsts).median));
}
