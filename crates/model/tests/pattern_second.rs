//! User bug (v0.4.7, lead 10-24): a plate with a wall on top patterned (Linear
//! pattern 1, quick), then the same wall on the bottom patterned with the same
//! settings (Linear pattern 2) took far longer than the first, and deleting
//! it while it ran didn't stop it. Both patterns must cost about the same, and
//! an aborted regeneration must return at once.

use glam::DVec2;
use noshape_model::configuration::ConfigValues;
use noshape_model::display::FeatureStatus;
use noshape_model::params::ParamValue;
use noshape_model::partstudio::PartStudio;
use noshape_model::reference::{DatumPlane, FeatureId, Reference};
use noshape_model::regen::{RegenCancel, RegenOptions, RegenResult, Regenerator};
use noshape_model::sketch_feature::SketchData;
use noshape_units::{ExprValue, UnitPrefs};
use std::time::{Duration, Instant};

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

struct Studio {
    ps: PartStudio,
    units: UnitPrefs,
}

impl Studio {
    fn add(&mut self, ty: &str, params: &[(&str, ParamValue)]) -> FeatureId {
        let mut f = self.ps.new_feature(ty, &self.units).unwrap();
        for (k, v) in params {
            f.params.set(k, v.clone());
        }
        let id = f.id;
        self.ps.features.push(f);
        id
    }
    fn regen(&self, cancel: Option<RegenCancel>) -> RegenResult {
        Regenerator::new().regenerate(&self.ps, &ConfigValues::default(), &self.units, &RegenOptions { cancel, ..Default::default() })
    }
    fn sketch_region(&mut self, sk: noshape_sketch::Sketch) -> Reference {
        let s = self.add("sketch", &[("sketchPlane", q(vec![Reference::Plane(DatumPlane::Top)])), ("sketch", ParamValue::Sketch(Box::new(SketchData { sketch: sk, ..Default::default() })))]);
        let r = self.regen(None);
        let d = r.display.sketches.iter().find(|x| x.feature == s).unwrap();
        Reference::SketchRegion { sketch: s, key: d.regions[0].key.clone() }
    }
}

/// The user's fin plate (lead 10-24, user/2026-10-24-noshape-fins-pattern-top):
/// plate (2n+10) × 20 × 10 mm; a 1 × 20 × 5 mm fin on top, the plate's whole
/// depth, patterned ×n along X every 2 mm (LP1); the same fin under the
/// plate, the same pattern (LP2).
fn model(n: usize, reapply: bool) -> (Studio, FeatureId, FeatureId) {
    let mut s = Studio { ps: PartStudio::new(), units: UnitPrefs::default() };
    let plate = s.sketch_region(rect(0.0, 0.0, (2 * n + 10) as f64 * 1e-3, 0.02));
    s.add("extrude", &[("entities", q(vec![plate])), ("depth", e("10 mm")), ("operationType", en("NEW"))]);
    let pattern = |s: &mut Studio, f: FeatureId| {
        s.add("linearPattern", &[("patternType", en("FEATURE")), ("instanceFunction", q(vec![Reference::Feature(f)])), ("directionOne", q(vec![Reference::Plane(DatumPlane::Right)])), ("distance", e("2 mm")), ("instanceCount", e(&n.to_string())), ("operationType", en("ADD")), ("defaultScope", b(true)), ("fullFeaturePattern", b(reapply))])
    };
    let top = s.sketch_region(rect(0.002, 0.0, 0.003, 0.02));
    let w1 = s.add("extrude", &[("entities", q(vec![top])), ("operationType", en("ADD")), ("defaultScope", b(true)), ("startOffset", b(true)), ("startOffsetBound", en("BLIND")), ("startOffsetDistance", e("10 mm")), ("depth", e("5 mm"))]);
    let lp1 = pattern(&mut s, w1);
    let bottom = s.sketch_region(rect(0.002, 0.0, 0.003, 0.02));
    let w2 = s.add("extrude", &[("entities", q(vec![bottom])), ("operationType", en("ADD")), ("defaultScope", b(true)), ("oppositeDirection", b(true)), ("depth", e("5 mm"))]);
    let lp2 = pattern(&mut s, w2);
    (s, lp1, lp2)
}

/// NOSHAPE_PERF=1 RUST_LOG=perf=info: the regeneration's timing breakdown.
fn log() {
    let _ = env_logger::builder().is_test(true).try_init();
}

fn count() -> usize {
    std::env::var("NOSHAPE_PATTERN_N").ok().and_then(|v| v.parse().ok()).unwrap_or(100)
}

#[test]
fn second_pattern_costs_about_the_same_as_the_first() {
    log();
    for reapply in [false, true] {
        second_pattern_case(reapply);
    }
}

fn second_pattern_case(reapply: bool) {
    let n = count();
    let (s, lp1, lp2) = model(n, reapply);
    let t0 = Instant::now();
    let r = s.regen(None);
    let total = t0.elapsed();
    let ms = |f: FeatureId| r.display.features.iter().find(|x| x.id == f).unwrap().time_ms as f64;
    if let Some(st) = r.display.features.iter().find(|st| !matches!(st.status, FeatureStatus::Ok)) {
        // Without Reapply the fins merge with the plate's sides: Onshape's face
        // pattern fails too (live G45).
        eprintln!("×{n} reapply {reapply}: {:?} {:?} (LP1 {:.0} ms, LP2 {:.0} ms, whole {:.0} ms)", st.status, st.message, ms(lp1), ms(lp2), total.as_secs_f64() * 1e3);
        assert!(!reapply, "with Reapply every feature must regenerate");
        return;
    }
    let ms = |f: FeatureId| r.display.features.iter().find(|x| x.id == f).unwrap().time_ms as f64;
    let (a, c) = (ms(lp1), ms(lp2));
    let solids: Vec<_> = r.context.bodies.iter().filter(|b| b.kind == noshape_model::display::BodyKind::Solid).collect();
    assert_eq!(solids.len(), 1);
    let v = solids[0].shape.volume().unwrap() * 1e9;
    let want = (2 * n + 10) as f64 * 20.0 * 10.0 + 2.0 * (n as f64) * 100.0;
    assert!((v - want).abs() < 1e-6 * want, "volume {v} mm³, want {want}");
    eprintln!("×{n} reapply {reapply}: LP1 {a:.0} ms, LP2 {c:.0} ms, whole regeneration {:.0} ms", total.as_secs_f64() * 1e3);
    assert!(c < 2.0 * a + 300.0, "reapply {reapply}: LP2 {c:.0} ms against LP1 {a:.0} ms");
}

#[test]
fn deleting_the_running_second_pattern_stops_it_at_once() {
    let n = count();
    let (s, lp1, _) = model(n, true);
    // How long the regeneration takes up to the middle of LP2.
    let r = s.regen(None);
    let ms = |f: FeatureId| r.display.features.iter().find(|x| x.id == f).unwrap().time_ms as f64;
    let before_lp2: f64 = r.display.features.iter().take_while(|x| x.id != lp1).map(|x| x.time_ms as f64).sum::<f64>() + ms(lp1);
    let lp2_ms = r.display.features.last().unwrap().time_ms as f64;
    let at = Duration::from_secs_f64((before_lp2 + lp2_ms / 2.0) / 1e3);
    let cancel = RegenCancel::default();
    let c2 = cancel.clone();
    let s = std::sync::Arc::new(s);
    let s2 = s.clone();
    let worker = std::thread::spawn(move || s2.regen(Some(c2)));
    std::thread::sleep(at);
    let t = Instant::now();
    cancel.abort();
    let out = worker.join().unwrap();
    let latency = t.elapsed();
    eprintln!("×{n}: aborted {:.0} ms in (LP2 runs {:.0} ms), returned {:.0} ms after the abort", at.as_secs_f64() * 1e3, lp2_ms, latency.as_secs_f64() * 1e3);
    assert!(out.cancelled, "the regeneration finished before the abort");
    assert!(latency < Duration::from_millis(200), "returned {:.0} ms after the abort", latency.as_secs_f64() * 1e3);
}
