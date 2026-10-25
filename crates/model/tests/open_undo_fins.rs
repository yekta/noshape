//! Open and undo on the user's fin plate (qa4's v0.4.8 gate, core-workflow
//! (a)): opening took ~20 s and undoing "delete Linear pattern 2" ~18 s where
//! a regeneration takes ~3 s. The app regenerates twice per change (preview
//! meshes, then refined; part_studio/mod.rs `schedule_regen`) with one
//! Regenerator per Part Studio; this replays open, delete and undo that way
//! and prints the time of each pass. `NOSHAPE_PATTERN_N` (default 100),
//! `NOSHAPE_PERF=1 RUST_LOG=perf=info` for the breakdown.

use glam::DVec2;
use noshape_model::configuration::ConfigValues;
use noshape_model::params::ParamValue;
use noshape_model::partstudio::PartStudio;
use noshape_model::reference::{DatumPlane, FeatureId, Reference};
use noshape_model::regen::{RegenOptions, RegenResult, Regenerator};
use noshape_model::sketch_feature::SketchData;
use noshape_units::{ExprValue, UnitPrefs};
use std::time::Instant;

fn e(s: &str) -> ParamValue {
    ParamValue::Expr(ExprValue::new(s))
}
fn q(v: Vec<Reference>) -> ParamValue {
    ParamValue::Query(v)
}
fn en(s: &str) -> ParamValue {
    ParamValue::Enum(s.into())
}
fn b(v: bool) -> ParamValue {
    ParamValue::Bool(v)
}

fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> noshape_sketch::Sketch {
    let mut sk = noshape_sketch::Sketch::new();
    let p = [DVec2::new(x0, y0), DVec2::new(x1, y0), DVec2::new(x1, y1), DVec2::new(x0, y1)];
    let pts: Vec<_> = p.iter().map(|q| sk.add_point(*q)).collect();
    for i in 0..4 {
        sk.add_line_between(pts[i], pts[(i + 1) % 4]).unwrap();
    }
    sk
}

fn add(ps: &mut PartStudio, units: &UnitPrefs, ty: &str, params: &[(&str, ParamValue)]) -> FeatureId {
    let mut f = ps.new_feature(ty, units).unwrap();
    for (k, v) in params {
        f.params.set(k, v.clone());
    }
    let id = f.id;
    ps.features.push(f);
    id
}

fn sketch_region(ps: &mut PartStudio, units: &UnitPrefs, sk: noshape_sketch::Sketch) -> Reference {
    let s = add(ps, units, "sketch", &[("sketchPlane", q(vec![Reference::Plane(DatumPlane::Top)])), ("sketch", ParamValue::Sketch(Box::new(SketchData { sketch: sk, ..Default::default() })))]);
    let r = Regenerator::new().regenerate(ps, &ConfigValues::default(), units, &RegenOptions::default());
    let d = r.display.sketches.iter().find(|x| x.feature == s).unwrap();
    Reference::SketchRegion { sketch: s, key: d.regions[0].key.clone() }
}

/// pattern_second.rs's model with Reapply: plate (2n+10) × 20 × 10 mm, a fin
/// on top patterned ×n (LP1), the same under the plate (LP2).
fn model(n: usize) -> (PartStudio, FeatureId) {
    let units = UnitPrefs::default();
    let mut ps = PartStudio::new();
    let plate = sketch_region(&mut ps, &units, rect(0.0, 0.0, (2 * n + 10) as f64 * 1e-3, 0.02));
    add(&mut ps, &units, "extrude", &[("entities", q(vec![plate])), ("depth", e("10 mm")), ("operationType", en("NEW"))]);
    let pattern = |ps: &mut PartStudio, f: FeatureId| {
        add(ps, &units, "linearPattern", &[("patternType", en("FEATURE")), ("instanceFunction", q(vec![Reference::Feature(f)])), ("directionOne", q(vec![Reference::Plane(DatumPlane::Right)])), ("distance", e("2 mm")), ("instanceCount", e(&n.to_string())), ("operationType", en("ADD")), ("defaultScope", b(true)), ("fullFeaturePattern", b(true))])
    };
    let top = sketch_region(&mut ps, &units, rect(0.002, 0.0, 0.003, 0.02));
    let w1 = add(&mut ps, &units, "extrude", &[("entities", q(vec![top])), ("operationType", en("ADD")), ("defaultScope", b(true)), ("startOffset", b(true)), ("startOffsetBound", en("BLIND")), ("startOffsetDistance", e("10 mm")), ("depth", e("5 mm"))]);
    pattern(&mut ps, w1);
    let bottom = sketch_region(&mut ps, &units, rect(0.002, 0.0, 0.003, 0.02));
    let w2 = add(&mut ps, &units, "extrude", &[("entities", q(vec![bottom])), ("operationType", en("ADD")), ("defaultScope", b(true)), ("oppositeDirection", b(true)), ("depth", e("5 mm"))]);
    let lp2 = pattern(&mut ps, w2);
    (ps, lp2)
}

/// One app change: the preview pass, then the refine pass if asked for.
fn change(regen: &mut Regenerator, ps: &PartStudio, what: &str) -> f64 {
    let units = UnitPrefs::default();
    let config = ConfigValues::default();
    let t = Instant::now();
    let r: RegenResult = regen.regenerate(ps, &config, &units, &RegenOptions { preview_meshes: true, ..Default::default() });
    let preview = t.elapsed().as_secs_f64();
    let refine = if r.needs_refine {
        let t = Instant::now();
        regen.regenerate(ps, &config, &units, &RegenOptions::default());
        t.elapsed().as_secs_f64()
    } else {
        0.0
    };
    let name = |id: FeatureId| ps.features.iter().find(|f| f.id == id).map_or(String::new(), |f| f.name.clone());
    let slowest = r.display.features.iter().max_by(|a, b| a.time_ms.total_cmp(&b.time_ms)).map(|f| format!("{} {:.0} ms", name(f.id), f.time_ms)).unwrap_or_default();
    eprintln!("{what}: preview {preview:.2} s + refine {refine:.2} s = {:.2} s (slowest feature: {slowest})", preview + refine);
    preview + refine
}

#[test]
#[ignore]
fn fins_open_and_undo_cost_about_one_regeneration() {
    let _ = env_logger::builder().is_test(true).try_init();
    let n = std::env::var("NOSHAPE_PATTERN_N").ok().and_then(|v| v.parse().ok()).unwrap_or(100);
    let (ps, lp2) = model(n);
    let units = UnitPrefs::default();
    let t = Instant::now();
    Regenerator::new().regenerate(&ps, &ConfigValues::default(), &units, &RegenOptions::default());
    let one = t.elapsed().as_secs_f64();
    eprintln!("×{n}: one regeneration (cold, final meshes) {one:.2} s");
    let mut regen = Regenerator::new();
    let open = change(&mut regen, &ps, "open");
    let mut deleted = ps.clone();
    deleted.features.retain(|f| f.id != lp2);
    let delete = change(&mut regen, &deleted, "delete LP2");
    let undo = change(&mut regen, &ps, "undo");
    eprintln!("×{n}: open {:.1}×, delete {:.1}×, undo {:.1}× one regeneration", open / one, delete / one, undo / one);
}
