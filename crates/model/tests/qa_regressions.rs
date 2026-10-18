//! Regressions found by end-to-end QA of the app (robustness of the model
//! against bad user input).

use glam::DVec2;
use noshape_model::configuration::ConfigValues;
use noshape_model::display::FeatureStatus;
use noshape_model::params::{ParamKind, ParamValue};
use noshape_model::partstudio::PartStudio;
use noshape_model::reference::{DatumPlane, FeatureId, Reference};
use noshape_model::regen::{RegenOptions, RegenResult, Regenerator};
use noshape_model::sketch_feature::SketchData;
use noshape_units::{ExprValue, LengthUnit, UnitPrefs};

struct T {
    ps: PartStudio,
    units: UnitPrefs,
    reg: Regenerator,
}

fn e(s: &str) -> ParamValue {
    ParamValue::Expr(ExprValue::new(s))
}
fn q(v: Vec<Reference>) -> ParamValue {
    ParamValue::Query(v)
}
fn en(s: &str) -> ParamValue {
    ParamValue::Enum(s.into())
}

impl T {
    fn new() -> Self {
        T { ps: PartStudio::new(), units: UnitPrefs::default(), reg: Regenerator::new() }
    }
    fn regen(&mut self) -> RegenResult {
        self.reg.regenerate(&self.ps, &ConfigValues::default(), &self.units, &RegenOptions::default())
    }
    fn add(&mut self, ty: &str, params: &[(&str, ParamValue)]) -> FeatureId {
        let mut f = self.ps.new_feature(ty, &self.units).unwrap_or_else(|| panic!("no feature type {ty}"));
        for (k, v) in params {
            f.params.set(k, v.clone());
        }
        let id = f.id;
        self.ps.features.push(f);
        id
    }
    fn set(&mut self, id: FeatureId, k: &str, v: ParamValue) {
        self.ps.feature_mut(id).unwrap().params.set(k, v);
    }
    fn status(&mut self, id: FeatureId) -> (FeatureStatus, Option<String>) {
        let r = self.regen();
        let s = r.display.features.iter().find(|s| s.id == id).expect("feature state");
        (s.status, s.message.clone())
    }
    /// 40 x 20 x 10 mm block on Top; returns (sketch, extrude).
    fn block(&mut self) -> (FeatureId, FeatureId) {
        let mut sk = noshape_sketch::Sketch::new();
        let p = [DVec2::new(0.0, 0.0), DVec2::new(0.04, 0.0), DVec2::new(0.04, 0.02), DVec2::new(0.0, 0.02)];
        let pts: Vec<_> = p.iter().map(|q| sk.add_point(*q)).collect();
        for i in 0..4 {
            sk.add_line_between(pts[i], pts[(i + 1) % 4]).unwrap();
        }
        let s = self.add("sketch", &[("sketchPlane", q(vec![Reference::Plane(DatumPlane::Top)])), ("sketch", ParamValue::Sketch(Box::new(SketchData { sketch: sk, ..Default::default() })))]);
        let r = self.regen();
        let key = r.display.sketches.iter().find(|x| x.feature == s).unwrap().regions[0].key.clone();
        let x = self.add("extrude", &[("entities", q(vec![Reference::SketchRegion { sketch: s, key }])), ("depth", e("10 mm")), ("operationType", en("NEW"))]);
        (s, x)
    }
}

/// Every built-in feature's default values lie inside their own bounds, in
/// both unit systems (otherwise the bounds check would fail new features).
#[test]
fn feature_defaults_are_within_bounds() {
    fn check(specs: &[noshape_model::params::ParamSpec], ty: &str) {
        for s in specs {
            match &s.kind {
                ParamKind::Num(n) => {
                    for d in [n.default_metric, n.default_imperial] {
                        assert!(d >= n.min - 1e-12 && d <= n.max + 1e-12, "{ty}.{}: default {d} outside [{}, {}]", s.id, n.min, n.max);
                    }
                }
                ParamKind::Array { item, .. } => check(item, ty),
                _ => {}
            }
        }
    }
    for ft in noshape_model::features::all() {
        check(&ft.params, ft.id);
    }
}

/// QA: a linear pattern with an absurd instance count used to run (and hang
/// the regeneration thread); a negative fillet radius or pattern distance
/// reached the kernel. Out-of-range values now fail the feature with a clear message.
#[test]
fn out_of_range_values_fail_with_message() {
    let mut t = T::new();
    t.block();
    let body = t.regen().context.bodies[0].id.clone();
    let pat = t.add(
        "linearPattern",
        &[
            ("patternType", en("PART")),
            ("entities", q(vec![Reference::Body(body)])),
            ("directionOne", q(vec![Reference::Plane(DatumPlane::Right)])),
            ("distance", e("50 mm")),
            ("instanceCount", e("1000000")),
        ],
    );
    let t0 = std::time::Instant::now();
    let (st, msg) = t.status(pat);
    assert_eq!(st, FeatureStatus::Error);
    assert!(msg.as_deref().unwrap_or("").contains("Instance count: must be at most 2500"), "{msg:?}");
    assert!(t0.elapsed().as_secs() < 30);
    t.set(pat, "instanceCount", e("0"));
    let (st, msg) = t.status(pat);
    assert_eq!(st, FeatureStatus::Error);
    assert!(msg.as_deref().unwrap_or("").contains("must be at least 1"), "{msg:?}");
    t.set(pat, "instanceCount", e("3"));
    assert_eq!(t.status(pat).0, FeatureStatus::Ok);
    t.set(pat, "distance", e("-5 mm"));
    let (st, msg) = t.status(pat);
    assert_eq!(st, FeatureStatus::Error);
    assert!(msg.as_deref().unwrap_or("").contains("Distance: must not be negative"), "{msg:?}");

    // Negative fillet radius: a field error, not a kernel failure.
    let mut t = T::new();
    t.block();
    let r = t.regen();
    let body = &r.context.bodies[0];
    let edge = Reference::Edge((*body.edges[0]).clone());
    let fil = t.add("fillet", &[("entities", q(vec![edge])), ("radius", e("-2 mm"))]);
    let (st, msg) = t.status(fil);
    assert_eq!(st, FeatureStatus::Error);
    assert!(msg.as_deref().unwrap_or("").contains("Radius: must not be negative"), "{msg:?}");
    t.set(fil, "radius", e("1 mm"));
    assert_eq!(t.status(fil).0, FeatureStatus::Ok);
    // Imperial documents format the bound in inches.
    t.units.length = LengthUnit::Inch;
    t.set(fil, "radius", e("30000 in"));
    let (_, msg) = t.status(fil);
    assert!(msg.as_deref().unwrap_or("").contains("must be at most") && msg.as_deref().unwrap_or("").contains("in"), "{msg:?}");
}

/// QA: a new Hole in a millimetre document defaulted to the Inch standard
/// with drill size 1/4" while its Diameter field said 5 mm (the evaluator
/// used the 5 mm). New holes now follow the document units and start with
/// the table values of their default standard.
#[test]
fn new_hole_follows_document_units_and_table() {
    use noshape_model::features::hole_tables as ht;
    let ps = PartStudio::new();
    let metric = UnitPrefs::default();
    let f = ps.new_feature("hole", &metric).unwrap();
    assert!(f.params.is("unitsSystem", "METRIC"));
    let (pid, table) = ht::hole_table_param(&f.params);
    assert!(!f.params.lookup(pid).is_empty());
    let spec = ht::table_values(table, &ht::complete_path(table, f.params.lookup(pid))).unwrap();
    let d = f.params.expr("holeDiameterV3").unwrap().text.clone();
    assert!(d.ends_with("mm"), "{d}");
    let v: f64 = d.trim_end_matches("mm").trim().parse().unwrap();
    assert!((v / 1000.0 - spec.hole_diameter).abs() < 1e-9, "{d} vs {}", spec.hole_diameter);

    let imperial = UnitPrefs { length: LengthUnit::Inch, ..UnitPrefs::default() };
    let mut f = ps.new_feature("hole", &imperial).unwrap();
    assert!(f.params.is("unitsSystem", "INCH"));
    assert!(f.params.expr("holeDiameterV3").unwrap().text.ends_with("in"));
    // Switching to Metric in the dialog re-applies the (ISO) table.
    f.params.set("unitsSystem", en("METRIC"));
    assert!(ht::sync_hole_standard(&mut f.params));
    assert!(f.params.expr("holeDiameterV3").unwrap().text.ends_with("mm"));
}
