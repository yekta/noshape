//! qa5 PERF1 / live "qa5 big plate": a 400 × 400 × 10 mm plate, 400 Ø8 holes
//! from one sketch cut Through all, a 0.5 mm chamfer on their 400 top edges,
//! then the plate's depth 10 → 12. Onshape: 617 ms, 1.02 s and 1.03 s for
//! the whole regeneration (Extrude 2 491 ms, Chamfer 1 407 ms). Each step's
//! whole regeneration (with the display) must stay under 2 s here, and the
//! geometry is Onshape's.

use glam::DVec2;
use noshape_model::configuration::ConfigValues;
use noshape_model::display::{BodyKind, FeatureStatus};
use noshape_model::params::ParamValue;
use noshape_model::partstudio::PartStudio;
use noshape_model::reference::{DatumPlane, FeatureId, Reference};
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

struct Studio {
    ps: PartStudio,
    units: UnitPrefs,
    reg: Regenerator,
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
    /// The whole regeneration, as the app runs it: (result, seconds).
    fn regen(&mut self) -> (RegenResult, f64) {
        let t0 = std::time::Instant::now();
        let r = self.reg.regenerate(&self.ps, &ConfigValues::default(), &self.units, &RegenOptions::default());
        let t = t0.elapsed().as_secs_f64();
        for (i, st) in r.display.features.iter().enumerate() {
            assert!(matches!(st.status, FeatureStatus::Ok), "feature {i} {:?}: {:?}", st.status, st.message);
        }
        (r, t)
    }
    fn sketch(&mut self, sk: noshape_sketch::Sketch) -> FeatureId {
        self.add("sketch", &[("sketchPlane", q(vec![Reference::Plane(DatumPlane::Top)])), ("sketch", ParamValue::Sketch(Box::new(SketchData { sketch: sk, ..Default::default() })))])
    }
}

fn part(r: &RegenResult) -> &noshape_model::regen::context::Body {
    let solids: Vec<_> = r.context.bodies.iter().filter(|b| b.kind == BodyKind::Solid).collect();
    assert_eq!(solids.len(), 1);
    solids[0]
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

#[test]
#[ignore = "PERF1 in progress (kern): 2.4 / 2.1 / 4.0 s against 2 s"]
fn plate_with_400_holes_regenerates_like_onshape() {
    if std::env::var_os("NOSHAPE_PERF").is_some() && log::set_logger(&PerfLog).is_ok() {
        log::set_max_level(log::LevelFilter::Info);
    }
    let mut s = Studio { ps: PartStudio::new(), units: UnitPrefs::default(), reg: Regenerator::new() };
    let mut sk = noshape_sketch::Sketch::new();
    let p = [DVec2::ZERO, DVec2::new(0.4, 0.0), DVec2::new(0.4, 0.4), DVec2::new(0.0, 0.4)];
    let pts: Vec<_> = p.iter().map(|x| sk.add_point(*x)).collect();
    for i in 0..4 {
        sk.add_line_between(pts[i], pts[(i + 1) % 4]).unwrap();
    }
    let s1 = s.sketch(sk);
    let (r, _) = s.regen();
    let plate = r.display.sketches.iter().find(|x| x.feature == s1).unwrap().regions[0].key.clone();
    let x1 = s.add("extrude", &[("entities", q(vec![Reference::SketchRegion { sketch: s1, key: plate }])), ("depth", e("10 mm")), ("operationType", en("NEW"))]);
    let mut sk = noshape_sketch::Sketch::new();
    for i in 0..20 {
        for j in 0..20 {
            sk.add_circle(DVec2::new(0.01 + 0.02 * i as f64, 0.01 + 0.02 * j as f64), 0.004);
        }
    }
    let s2 = s.sketch(sk);
    let (r, _) = s.regen();
    let holes: Vec<Reference> = r.display.sketches.iter().find(|x| x.feature == s2).unwrap().regions.iter().map(|g| Reference::SketchRegion { sketch: s2, key: g.key.clone() }).collect();
    assert_eq!(holes.len(), 400);
    s.add("extrude", &[("entities", q(holes)), ("endBound", en("THROUGH_ALL")), ("operationType", en("REMOVE"))]);
    let (r, t_holes) = s.regen();
    let v = |r: &RegenResult| part(r).shape.volume().unwrap() * 1e9;
    let faces = |r: &RegenResult| part(r).shape.count(noshape_kernel::ShapeType::Face);
    assert!((v(&r) - 1398938.07017026).abs() < 1e-3, "{}", v(&r));
    assert_eq!(faces(&r), 406);
    // The holes' 400 top edges.
    let b = part(&r);
    let top: Vec<Reference> = b
        .shape
        .edges()
        .iter()
        .enumerate()
        .filter(|(_, x)| matches!(x.edge_curve(), Ok(noshape_kernel::Curve::Circle { .. })) && x.edge_info().map(|i| (i.start.z - 0.01).abs() < 1e-9).unwrap_or(false))
        .map(|(i, _)| Reference::Edge((*b.edges[i]).clone()))
        .collect();
    assert_eq!(top.len(), 400);
    s.add("chamfer", &[("entities", q(top)), ("width", e("0.5 mm"))]);
    let (r, t_chamfer) = s.regen();
    assert!((v(&r) - 1397629.07323126).abs() < 1e-3, "{}", v(&r));
    assert_eq!(faces(&r), 806);
    s.ps.feature_mut(x1).unwrap().params.set("depth", e("12 mm"));
    let (r, t_depth) = s.regen();
    assert!((v(&r) - 1677416.68726529).abs() < 1e-3, "{}", v(&r));
    assert_eq!(faces(&r), 806);
    let times: Vec<String> = r.display.features.iter().zip(&s.ps.features).map(|(st, f)| format!("{} {} ms", f.type_id, st.time_ms)).collect();
    eprintln!("big plate: holes {t_holes:.2} s, chamfer {t_chamfer:.2} s, depth edit {t_depth:.2} s ({})", times.join(", "));
    for (what, t) in [("holes", t_holes), ("chamfer", t_chamfer), ("depth edit", t_depth)] {
        assert!(t < 2.0, "{what}: {t:.2} s (Onshape ~1 s)");
    }
}
