//! Pattern performance: a block with a small rib, patterned ×200 and ×1000
//! as a Feature pattern (the rib standing on the block, and passing through
//! it) and as a Part pattern. The pattern feature's own regeneration time
//! (not loading or saving) must grow about linearly: ×1000 within 8 times
//! ×200 (linear is 5; we measure 5.6-6.4 under load, the rest is OCCT's
//! shell orientation pass on the 4000-edge top face; quadratic would be 25).
//! Onshape's server takes ~4.5 s for ×200 and ~13 s for ×1000 of such a
//! Feature pattern (live, "Pending state").

use glam::DVec2;
use noshape_model::configuration::ConfigValues;
use noshape_model::display::FeatureStatus;
use noshape_model::params::ParamValue;
use noshape_model::partstudio::PartStudio;
use noshape_model::reference::{BodyId, DatumPlane, FeatureId, Reference};
use noshape_model::regen::{RegenOptions, RegenResult, Regenerator};
use noshape_model::sketch_feature::SketchData;
use noshape_units::{ExprValue, UnitPrefs};

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

#[derive(Clone, Copy, Debug)]
enum Case {
    /// Feature pattern of a rib standing on the block's top face.
    FeatureOnTop,
    /// Feature pattern of a rib extruded from the block's bottom through it.
    FeatureThrough,
    /// Part pattern (Add) of a separate rib part standing on the block.
    Part,
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
    fn regen(&self) -> RegenResult {
        Regenerator::new().regenerate(&self.ps, &ConfigValues::default(), &self.units, &RegenOptions::default())
    }
    /// A sketch's only region.
    fn sketch_region(&mut self, sk: noshape_sketch::Sketch) -> Reference {
        let s = self.add("sketch", &[("sketchPlane", q(vec![Reference::Plane(DatumPlane::Top)])), ("sketch", ParamValue::Sketch(Box::new(SketchData { sketch: sk, ..Default::default() })))]);
        let r = self.regen();
        let d = r.display.sketches.iter().find(|x| x.feature == s).unwrap();
        Reference::SketchRegion { sketch: s, key: d.regions[0].key.clone() }
    }
}

/// Regenerate the case with `n` instances: (pattern feature's time in s, result).
fn run(case: Case, n: usize) -> (f64, RegenResult) {
    let mut s = Studio { ps: PartStudio::new(), units: UnitPrefs::default() };
    // Block 2n+10 × 20 × 10 mm.
    let block = s.sketch_region(rect(0.0, 0.0, (2 * n + 10) as f64 * 1e-3, 0.02));
    s.add("extrude", &[("entities", q(vec![block])), ("depth", e("10 mm")), ("operationType", en("NEW"))]);
    // Rib 1 × 10 mm (x 2..3, y 5..15).
    let rib = s.sketch_region(rect(0.002, 0.005, 0.003, 0.015));
    let on_top = [("startOffset", b(true)), ("startOffsetBound", en("BLIND")), ("startOffsetDistance", e("10 mm")), ("depth", e("5 mm"))];
    let rib_feature = match case {
        Case::FeatureOnTop => {
            let mut p = vec![("entities", q(vec![rib])), ("operationType", en("ADD")), ("defaultScope", ParamValue::Bool(true))];
            p.extend(on_top.iter().cloned());
            s.add("extrude", &p)
        }
        Case::FeatureThrough => s.add("extrude", &[("entities", q(vec![rib])), ("depth", e("15 mm")), ("operationType", en("ADD")), ("defaultScope", ParamValue::Bool(true))]),
        Case::Part => {
            let mut p = vec![("entities", q(vec![rib])), ("operationType", en("NEW"))];
            p.extend(on_top.iter().cloned());
            s.add("extrude", &p)
        }
    };
    let dir = q(vec![Reference::Plane(DatumPlane::Right)]);
    let common = [("directionOne", dir), ("distance", e("2 mm")), ("instanceCount", e(&n.to_string())), ("operationType", en("ADD")), ("defaultScope", ParamValue::Bool(true))];
    let mut p: Vec<(&str, ParamValue)> = match case {
        Case::Part => vec![("patternType", en("PART")), ("entities", q(vec![Reference::Body(BodyId { feature: rib_feature, index: 0 })]))],
        _ => vec![("patternType", en("FEATURE")), ("instanceFunction", q(vec![Reference::Feature(rib_feature)]))],
    };
    p.extend(common.iter().cloned());
    s.add("linearPattern", &p);
    let r = s.regen();
    for (i, st) in r.display.features.iter().enumerate() {
        assert!(matches!(st.status, FeatureStatus::Ok), "{case:?} ×{n}: feature {i} {:?}: {:?}", st.status, st.message);
    }
    let t = r.display.features.last().unwrap().time_ms as f64 / 1000.0;
    (t, r)
}

/// With NOSHAPE_PERF set, the regeneration's timing logs (target "perf") on stderr.
struct PerfLog;
impl log::Log for PerfLog {
    fn enabled(&self, m: &log::Metadata) -> bool {
        m.target() == "perf"
    }
    fn log(&self, r: &log::Record) {
        if self.enabled(r.metadata()) {
            eprintln!("  perf: {}", r.args());
        }
    }
    fn flush(&self) {}
}

/// The timing cases run one at a time: run in parallel, one case's ×1000
/// shares the machine with the others while its ×200 didn't.
static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn check(case: Case, timed: bool) {
    let _one_at_a_time = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    if std::env::var_os("NOSHAPE_PERF").is_some() && log::set_logger(&PerfLog).is_ok() {
        log::set_max_level(log::LevelFilter::Info);
    }
    let mut times = Vec::new();
    for n in [200usize, 1000] {
        let (t, r) = run(case, n);
        // The block with every rib (n of them, 1 × 10 × 5 mm above it): one
        // part ("Merge with all" also takes a Part pattern's seed rib, which
        // stands on the block).
        let solids: Vec<_> = r.context.bodies.iter().filter(|b| b.kind == noshape_model::display::BodyKind::Solid).collect();
        assert_eq!(solids.len(), 1, "{case:?} ×{n}");
        let v: f64 = solids.iter().map(|b| b.shape.volume().unwrap()).sum::<f64>() * 1e9;
        let want = (2 * n + 10) as f64 * 20.0 * 10.0 + n as f64 * 1.0 * 10.0 * 5.0;
        assert!((v - want).abs() < 1e-6 * want, "{case:?} ×{n}: volume {v} mm³, want {want}");
        eprintln!("{case:?} ×{n}: pattern {t:.2} s, {} faces", solids.iter().map(|b| b.faces.len()).sum::<usize>());
        times.push(t);
    }
    let ratio = times[1] / times[0].max(1e-3);
    if timed {
        assert!(ratio < 8.0, "{case:?}: ×1000 took {:.2} s, {ratio:.1} times ×200 ({:.2} s): superlinear", times[1], times[0]);
    }
}

#[test]
fn feature_pattern_on_top_regenerates() {
    check(Case::FeatureOnTop, false);
}

#[test]
#[ignore = "timed: perf.yml model job (wall clock; ci.yml tests are deterministic)"]
fn timed_feature_pattern_on_top_is_linear() {
    check(Case::FeatureOnTop, true);
}

#[test]
fn feature_pattern_through_regenerates() {
    check(Case::FeatureThrough, false);
}

#[test]
#[ignore = "timed: perf.yml model job (wall clock; ci.yml tests are deterministic)"]
fn timed_feature_pattern_through_is_linear() {
    check(Case::FeatureThrough, true);
}

#[test]
fn part_pattern_regenerates() {
    check(Case::Part, false);
}

#[test]
#[ignore = "timed: perf.yml model job (wall clock; ci.yml tests are deterministic)"]
fn timed_part_pattern_is_linear() {
    check(Case::Part, true);
}

/// Live G45: block 30 × 20 × 10 mm, a rib (x 2..3, 15 mm high) added from
/// the Top plane, Feature pattern of the rib along x, 5 mm, 5 instances.
/// Returns the regeneration and the part's faces (area mm², normal).
fn g45(full_width: bool, reapply: bool) -> (RegenResult, Vec<(f64, glam::DVec3)>) {
    let mut s = Studio { ps: PartStudio::new(), units: UnitPrefs::default() };
    let block = s.sketch_region(rect(0.0, 0.0, 0.03, 0.02));
    s.add("extrude", &[("entities", q(vec![block])), ("depth", e("10 mm")), ("operationType", en("NEW"))]);
    let rib = if full_width { rect(0.002, 0.0, 0.003, 0.02) } else { rect(0.002, 0.005, 0.003, 0.015) };
    let rib = s.sketch_region(rib);
    let rib = s.add("extrude", &[("entities", q(vec![rib])), ("depth", e("15 mm")), ("operationType", en("ADD")), ("defaultScope", ParamValue::Bool(true))]);
    s.add(
        "linearPattern",
        &[
            ("patternType", en("FEATURE")),
            ("instanceFunction", q(vec![Reference::Feature(rib)])),
            ("directionOne", q(vec![Reference::Plane(DatumPlane::Right)])),
            ("distance", e("5 mm")),
            ("instanceCount", e("5")),
            ("operationType", en("ADD")), ("defaultScope", ParamValue::Bool(true)),
            ("fullFeaturePattern", b(reapply)),
        ],
    );
    let r = s.regen();
    let faces = r
        .context
        .bodies
        .iter()
        .flat_map(|b| b.shape.faces())
        .map(|f| {
            let c = f.centroid().unwrap();
            (f.area().unwrap() * 1e6, f.normal_at_point(c).unwrap())
        })
        .collect();
    (r, faces)
}

/// Live G45: Onshape merges the faces a pattern leaves coplanar, and a
/// default Feature pattern of a rib whose sides merged into the block's
/// faces fails, asking for Reapply features.
#[test]
fn feature_pattern_of_merged_rib_matches_onshape() {
    let status = |r: &RegenResult| r.display.features.last().map(|f| (f.status, f.message.clone())).unwrap();
    // Narrow rib: 11 + 4 × 5 faces.
    let (r, faces) = g45(false, false);
    assert!(matches!(status(&r).0, FeatureStatus::Ok), "{:?}", status(&r));
    assert_eq!(faces.len(), 31);
    // Full-width rib, default: Onshape's error.
    let (r, _) = g45(true, false);
    let (st, msg) = status(&r);
    assert!(matches!(st, FeatureStatus::Error), "{st:?}");
    assert_eq!(msg.as_deref(), Some("Could not create all instances as entered. Try selecting \"Reapply features\" option."));
    // Full-width rib, Reapply features: 26 faces, the front and back faces
    // one face each of 300 + 5 × 5 mm².
    let (r, faces) = g45(true, true);
    assert!(matches!(status(&r).0, FeatureStatus::Ok), "{:?}", status(&r));
    assert_eq!(faces.len(), 26);
    let side: Vec<f64> = faces.iter().filter(|(_, n)| n.y.abs() > 0.99).map(|(a, _)| *a).collect();
    assert_eq!(side.len(), 2, "{side:?}");
    assert!(side.iter().all(|a| (a - 325.0).abs() < 1e-6), "{side:?}");
}
