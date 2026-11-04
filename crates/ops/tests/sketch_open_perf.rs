//! PLAN 2.7 sketch speed on real sketches: the biggest sketches of the model
//! suite (/root/parity/models, translated as parity_models does) and of the
//! solver fixtures (a 94-point fit spline, qa's fuzz sketch, a spur gear):
//! opening one for edit (a solve with the DOF analysis and its regions),
//! changing a dimension, and a drag step (the editor's, solve included).
//! `tools/tiers/perf.sh -p noshape-ops --test all -- sketch_open_perf:: --ignored --nocapture --test-threads=1`

use glam::DVec2;
use noshape_model::display::PlaneFrame;
use noshape_model::params::ParamValue;
use noshape_model::reference::FeatureId;
use noshape_model::sketch_feature::SketchData;
use noshape_ops::Ctx;
use noshape_scene::sketch_mode::{SketchEditor, Tool};
use noshape_sketch::{EntityKind, Sketch, SolveOptions};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// (model directory, sketch name): the model suite's biggest sketches.
const MODELS: [(&str, &str); 4] = [("cf-fix-pcb", "Sketch 3"), ("cf-straight-spline", "Sketch 1"), ("cf-socket-head-cap-screw-creator", "Sketch 7"), ("flange-josh", "Sketch 1")];

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1e3
}

/// Median and worst of `n` runs of `f`.
fn time(n: usize, mut f: impl FnMut()) -> (f64, f64) {
    let mut t: Vec<Duration> = (0..n)
        .map(|_| {
            let s = Instant::now();
            f();
            s.elapsed()
        })
        .collect();
    t.sort();
    (ms(t[t.len() / 2]), ms(t[t.len() - 1]))
}

/// A model's sketch as ours translates it.
fn model_sketch(dir: &Path, name: &str) -> Option<SketchData> {
    let (features, _, refs) = super::parity::read_model(dir)?;
    let mut ctx = Ctx::new();
    let path = std::env::temp_dir().join(format!("noshape-sketch-open-perf-{}-{}.noshape", std::process::id(), dir.file_name()?.to_string_lossy()));
    let all = noshape_ops::parity::parse_refs(&refs);
    let mut t = noshape_ops::parity::Translator::open(&mut ctx, &path, &features, all).ok()?;
    for k in 0..t.len() {
        let _ = t.step(k);
    }
    drop(t);
    let doc = ctx.doc(path.to_str()).ok()?.doc().clone();
    let _ = std::fs::remove_file(&path);
    doc.tabs.iter().find_map(|tab| match &tab.content {
        noshape_model::document::TabContent::PartStudio(ps) => ps.features.iter().find(|f| f.type_id == "sketch" && f.name == name).and_then(|f| match f.params.get("sketch") {
            Some(ParamValue::Sketch(d)) => Some((**d).clone()),
            _ => None,
        }),
        _ => None,
    })
}

fn fixture(name: &str) -> Option<SketchData> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../sketch/tests/fixtures/solve").join(name);
    let sketch: Sketch = serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()?;
    Some(SketchData { sketch, ..Default::default() })
}

/// Open, a dimension change and a drag step of one sketch, printed.
fn measure(what: &str, data: &SketchData) {
    let sk = &data.sketch;
    let curves = sk.entities().filter(|e| e.kind() != EntityKind::Point).count();
    let (open, open_worst) = time(15, || {
        let mut s = sk.clone();
        s.solve_with(&SolveOptions { analyze: true, ..Default::default() });
        let _ = s.regions();
    });
    let dim = sk.constraints().find(|c| c.driving && c.kind.is_dimension() && c.kind.value().is_some_and(|v| v.abs() > 1e-9));
    let edit = dim.map(|c| {
        let v = c.kind.value().unwrap_or(0.0);
        let mut k = 0;
        time(15, || {
            let mut s = sk.clone();
            k += 1;
            let _ = s.set_dimension_value(c.id, v * (1.0 + 0.002 * k as f64));
            s.solve();
        })
    });
    // A drag: a free end of the first line that isn't model geometry, moved 30 steps.
    let point = sk.entities().filter(|e| e.kind() == EntityKind::Line && !e.external).flat_map(|e| e.point_refs()).find(|p| sk.point(*p).is_some());
    let drag = point.map(|p| {
        let mut d = data.clone();
        let mut ed = SketchEditor::new(FeatureId(1), PlaneFrame::TOP);
        ed.world_per_px = 1e-4;
        ed.set_tool(Tool::Select);
        let from = d.sketch.point(p).unwrap_or_default();
        ed.hover = Some(p);
        ed.pointer_move(&mut d, from, false, false);
        ed.tool_down(&mut d, from, Some(p), false, false);
        let mut steps = Vec::new();
        for k in 1..=30 {
            let to = from + DVec2::new(0.0002 * k as f64, 0.0001 * k as f64);
            let s = Instant::now();
            ed.tool_drag(&mut d, to);
            steps.push(s.elapsed());
        }
        ed.tool_up(&mut d, from);
        steps.sort();
        (ms(steps[steps.len() / 2]), ms(steps[steps.len() - 1]))
    });
    let f = |x: Option<(f64, f64)>| x.map(|(m, w)| format!("median {m:.2} ms, worst {w:.2} ms")).unwrap_or_else(|| "n/a".into());
    println!("{what}: {} entities ({curves} curves), {} constraints", sk.entity_count(), sk.constraint_count());
    println!("  open (solve with DOF + regions): median {open:.2} ms, worst {open_worst:.2} ms");
    println!("  change a dimension: {}", f(edit));
    println!("  drag step: {}", f(drag));
}

#[test]
#[ignore = "T2: timings of the model suite's and the solver fixtures' biggest sketches"]
fn biggest_sketches() {
    // The perf server keeps its copy in ~/parity/models.
    let home = std::env::var("HOME").map(|h| PathBuf::from(h).join("parity/models")).unwrap_or_default();
    let root = std::env::var("NOSHAPE_PARITY_MODELS").map(PathBuf::from).unwrap_or_else(|_| if Path::new("/root/parity/models").is_dir() { PathBuf::from("/root/parity/models") } else { home });
    for (dir, name) in MODELS {
        match model_sketch(&root.join(dir), name) {
            Some(d) => measure(&format!("{dir} {name}"), &d),
            None => println!("{dir} {name}: not translated"),
        }
    }
    for f in ["sk211-step89.json", "fz211-final.json", "spur-gear-fillet.json"] {
        match fixture(f) {
            Some(d) => measure(f, &d),
            None => println!("{f}: unreadable"),
        }
    }
}
