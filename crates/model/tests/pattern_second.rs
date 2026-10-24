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
    // CI release ×400: 1.9-2.7× (runner noise); it was 40×.
    assert!(c < 3.0 * a + 300.0, "reapply {reapply}: LP2 {c:.0} ms against LP1 {a:.0} ms");
}

#[test]
fn deleting_the_running_second_pattern_stops_it_at_once() {
    let n = count();
    let (s, _, lp2) = model(n, true);
    // Where LP2 runs in a regeneration: after the features before it (a
    // second, warm run).
    let _ = s.regen(None);
    let r = s.regen(None);
    let lp2_ms = r.display.features.iter().find(|x| x.id == lp2).unwrap().time_ms as f64;
    let start: f64 = r.display.features.iter().take_while(|x| x.id != lp2).map(|x| x.time_ms as f64).sum();
    let s = std::sync::Arc::new(s);
    for frac in [0.2, 0.5, 0.8] {
        // A run can be faster than the measured one: when it finishes before
        // the abort, abort earlier.
        let mut at_ms = start + lp2_ms * frac;
        let mut done = false;
        for _ in 0..5 {
            let at = Duration::from_secs_f64(at_ms / 1e3);
            let cancel = RegenCancel::default();
            let (c2, s2) = (cancel.clone(), s.clone());
            let worker = std::thread::spawn(move || s2.regen(Some(c2)));
            std::thread::sleep(at);
            let t = Instant::now();
            cancel.abort();
            let out = worker.join().unwrap();
            let latency = t.elapsed();
            if !out.cancelled {
                at_ms *= 0.7;
                continue;
            }
            eprintln!("×{n}: aborted {at_ms:.0} ms in (about {:.0} % into LP2, which runs {lp2_ms:.0} ms), returned {:.0} ms after the abort", frac * 100.0, latency.as_secs_f64() * 1e3);
            // 200 ms in release (the UI's budget); a debug build runs ~4× slower.
            let budget = Duration::from_millis(if cfg!(debug_assertions) { 600 } else { 200 });
            assert!(latency < budget, "returned {:.0} ms after the abort ({:.0} % into LP2)", latency.as_secs_f64() * 1e3, frac * 100.0);
            done = true;
            break;
        }
        assert!(done, "every run finished before the abort ({:.0} % into LP2)", frac * 100.0);
    }
}

/// The app's path (part_studio schedule_regen, model regen::runs): one
/// shared Regenerator; LP2 added in its dialog (the preview runs it), ✓ (the
/// final run only extends the preview, which goes on into the cache), then
/// LP2 deleted (or suppressed) while it still computes: both runs stop and
/// the model without LP2 shows at once (v0.4.8 gate: it waited 7-9 s for
/// the superseded preview).
#[test]
fn deleting_the_running_second_pattern_in_the_app_shows_the_first_at_once() {
    use noshape_model::regen::runs::{Run, Runs};
    use std::sync::{Arc, Mutex};
    log();
    let n = count();
    for how in ["delete", "suppress"] {
        let (s, _, lp2) = model(n, true);
        let regen = Arc::new(Mutex::new(Regenerator::new()));
        let mut runs = Runs::default();
        // Starts a run the way schedule_regen does (the worker holds its cancel).
        let mut start = |ps: PartStudio, upto: Option<usize>| {
            let studio = Arc::new(ps);
            let cancel = RegenCancel::default();
            runs.start(Run { cancel: cancel.clone(), studio: studio.clone(), upto, config: ConfigValues::default(), doc_vars: Default::default(), editing_sketch: None });
            let (regen, units) = (regen.clone(), s.units);
            let t = Instant::now();
            std::thread::spawn(move || {
                let mut g = regen.lock().unwrap();
                let waited = t.elapsed().as_secs_f64() * 1e3;
                let r = g.regenerate(&studio, &ConfigValues::default(), &units, &RegenOptions { upto, preview_meshes: true, cancel: Some(cancel), ..Default::default() });
                eprintln!("run upto {upto:?}: waited {waited:.0} ms for the Regenerator, ran {:.0} ms, cancelled {}", t.elapsed().as_secs_f64() * 1e3 - waited, r.cancelled);
                (r, t.elapsed().as_secs_f64() * 1e3)
            })
        };
        let mut without = s.ps.clone();
        without.remove(lp2);
        let (_, open) = start(without.clone(), None).join().unwrap();
        // The dialog's preview (rolled to just after LP2), then ✓.
        let index = s.ps.index_of(lp2).unwrap();
        let preview = start(s.ps.clone(), Some(index + 1));
        std::thread::sleep(Duration::from_millis(100));
        let accepted = start(s.ps.clone(), None);
        std::thread::sleep(Duration::from_millis(200));
        let edited = if how == "delete" {
            without.clone()
        } else {
            let mut p = s.ps.clone();
            p.feature_mut(lp2).unwrap().suppressed = true;
            p
        };
        let (r, shown) = start(edited, None).join().unwrap();
        let (preview, accepted) = (preview.join().unwrap().0, accepted.join().unwrap().0);
        eprintln!("×{n} {how}: open {open:.0} ms; preview cancelled {}, ✓ run cancelled {}; shown {shown:.0} ms after the {how}; features {:?}", preview.cancelled, accepted.cancelled, r.display.features.iter().map(|f| (f.status, f.time_ms as u32)).collect::<Vec<_>>());
        assert!(preview.cancelled && accepted.cancelled && !r.cancelled);
        assert_eq!(runs.running(), 0);
        let budget = if cfg!(debug_assertions) { 1000.0 } else { 300.0 };
        assert!(shown < budget, "{how}: the model without LP2 showed {shown:.0} ms after the {how}");
    }
}
