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

/// QA: a new Hole's Diameter field disagreed with its drill size (the
/// evaluator used a 5 mm default while the dialog said 1/4"). New holes now
/// start with the table values of their default standard. Like Onshape
/// (live, behaviours.md "Hole with several sketch points": a mm document
/// opens a new Hole on the **Inch** tab, Drilled 1/4 = 6.35 mm, title
/// "Ø 6.35 mm"), the standard is Inch whatever the document units;
/// switching the dialog to Metric re-applies the ISO table.
#[test]
fn new_hole_follows_document_units_and_table() {
    use noshape_model::features::hole_tables as ht;
    let ps = PartStudio::new();
    let imperial = UnitPrefs { length: LengthUnit::Inch, ..UnitPrefs::default() };
    for units in [UnitPrefs::default(), imperial] {
        let mut f = ps.new_feature("hole", &units).unwrap();
        assert!(f.params.is("unitsSystem", "INCH"));
        let (pid, table) = ht::hole_table_param(&f.params);
        assert!(!f.params.lookup(pid).is_empty());
        let spec = ht::table_values(table, &ht::complete_path(table, f.params.lookup(pid))).unwrap();
        // Drilled 1/4: 6.35 mm.
        assert!((spec.hole_diameter - 6.35e-3).abs() < 1e-9, "{}", spec.hole_diameter);
        let d = f.params.expr("holeDiameterV3").unwrap().text.clone();
        let v = noshape_units::evaluate_as(&d, noshape_units::Dim::LENGTH, &units, &noshape_units::EmptyScope).unwrap().value;
        assert!((v - spec.hole_diameter).abs() < 1e-9, "{d} vs {}", spec.hole_diameter);
        // Switching to Metric in the dialog re-applies the (ISO) table.
        f.params.set("unitsSystem", en("METRIC"));
        assert!(ht::sync_hole_standard(&mut f.params));
        assert!(f.params.expr("holeDiameterV3").unwrap().text.ends_with("mm"));
    }
}

/// QA3: on a block with 3 mm corner fillets, a 5 mm chamfer of a top edge
/// (tangent propagation runs it around the whole top loop) "succeeded" with
/// an invalid solid (BRepCheck fails; the app showed holes through missing
/// faces). Blends now either give a valid solid or fail with Onshape's
/// message; Onshape (live capture F1, behaviours.md "Geometry ground truth"):
/// chamfers wider than the 3 mm corner fillets fail with "Distance of chamfer
/// on face too large.", fillets of any of these sizes succeed.
#[test]
fn blend_over_smaller_fillets_is_valid_or_fails() {
    let mut t = T::new();
    t.block();
    let r = t.regen();
    let body = &r.context.bodies[0];
    let vertical: Vec<_> = body
        .shape
        .edges()
        .iter()
        .enumerate()
        .filter(|(_, e)| {
            let i = e.edge_info().unwrap();
            (i.start.z - i.end.z).abs() > 1e-6
        })
        .map(|(i, _)| Reference::Edge((*body.edges[i]).clone()))
        .collect();
    assert_eq!(vertical.len(), 4);
    let fil = t.add("fillet", &[("entities", q(vertical)), ("radius", e("3 mm"))]);
    assert_eq!(t.status(fil).0, FeatureStatus::Ok);
    let r = t.regen();
    let body = &r.context.bodies[0];
    let top = body
        .shape
        .edges()
        .iter()
        .position(|e| {
            let i = e.edge_info().unwrap();
            i.start.z > 0.009 && i.end.z > 0.009 && matches!(i.curve, noshape_kernel::Curve::Line { .. }) && i.length > 0.02
        })
        .map(|i| Reference::Edge((*body.edges[i]).clone()))
        .expect("top edge");
    for (ty, size, ok) in [("chamfer", "1 mm", true), ("chamfer", "5 mm", false), ("chamfer", "8 mm", false), ("fillet", "1 mm", true), ("fillet", "5 mm", true)] {
        let mut t2 = T { ps: t.ps.clone(), units: t.units, reg: Regenerator::new() };
        let size_param = if ty == "chamfer" { "width" } else { "radius" };
        let f = t2.add(ty, &[("entities", q(vec![top.clone()])), (size_param, e(size)), ("tangentPropagation", ParamValue::Bool(true))]);
        let (st, msg) = t2.status(f);
        let r = t2.regen();
        if ok {
            assert_eq!(st, FeatureStatus::Ok, "{ty} {size}: {msg:?}");
        } else {
            assert_eq!(st, FeatureStatus::Error, "{ty} {size} should fail");
            let want = if ty == "chamfer" { "Distance of chamfer on face too large." } else { "Failed to fillet selections." };
            assert_eq!(msg.as_deref(), Some(want), "{ty} {size}");
        }
        assert!(r.context.bodies[0].shape.is_valid(), "{ty} {size}: invalid solid");
    }
}

/// QA3: Shell (top face removed) of a block with 3 mm corner fillets returned
/// an invalid solid (Arc join; right geometry, ShapeFix repairs it). With a
/// 1 mm chamfer around the top, OCCT fails, returns garbage or returns the
/// part unshelled ("OK" with the top face still there). The Shell feature now
/// keeps only a valid result that really removed the faces (trying both join
/// types and ShapeFix); otherwise it fails with Onshape's message.
#[test]
fn shell_is_valid_and_real_or_fails() {
    fn edges_where(t: &mut T, f: impl Fn(&noshape_kernel::EdgeInfo) -> bool) -> Vec<Reference> {
        let r = t.regen();
        let b = &r.context.bodies[0];
        b.shape.edges().iter().enumerate().filter(|(_, e)| f(&e.edge_info().unwrap())).map(|(i, _)| Reference::Edge((*b.edges[i]).clone())).collect()
    }
    let mut t = T::new();
    t.block();
    let vertical = edges_where(&mut t, |i| (i.start.z - i.end.z).abs() > 1e-6);
    t.add("fillet", &[("entities", q(vertical)), ("radius", e("3 mm"))]);
    for chamfer in [false, true] {
        if chamfer {
            let top = edges_where(&mut t, |i| i.start.z > 0.0099 && i.end.z > 0.0099 && matches!(i.curve, noshape_kernel::Curve::Line { .. }) && i.length > 0.02);
            t.add("chamfer", &[("entities", q(vec![top[0].clone()])), ("width", e("1 mm")), ("tangentPropagation", ParamValue::Bool(true))]);
        }
        let r = t.regen();
        let b = &r.context.bodies[0];
        let fi = b.shape.faces().iter().position(|f| f.centroid().unwrap().z > 0.0099 && f.area().unwrap() > 1e-4).unwrap();
        let top_face = Reference::Face((*b.faces[fi]).clone());
        let volume_before = noshape_kernel::volume_properties(&b.shape).unwrap().mass;
        for th in ["1 mm", "2.5 mm"] {
            let mut t2 = T { ps: t.ps.clone(), units: t.units, reg: Regenerator::new() };
            let s = t2.add("shell", &[("entities", q(vec![top_face.clone()])), ("thickness", e(th))]);
            let (st, msg) = t2.status(s);
            if chamfer && st == FeatureStatus::Error {
                // OCCT can't shell this one (Onshape can); never a silent wrong result.
                assert_eq!(msg.as_deref(), Some("Could not shell part with selections."));
                continue;
            }
            assert_eq!(st, FeatureStatus::Ok, "chamfer={chamfer} {th}: {msg:?}");
            let r = t2.regen();
            let shape = &r.context.bodies[0].shape;
            assert!(shape.is_valid(), "chamfer={chamfer} {th}: invalid solid");
            let v = noshape_kernel::volume_properties(shape).unwrap().mass;
            assert!(v > 0.0 && v < volume_before * 0.8, "chamfer={chamfer} {th}: volume {v} vs {volume_before}");
        }
    }
    // Nothing selected: Onshape's prompt.
    let mut t3 = T { ps: t.ps.clone(), units: t.units, reg: Regenerator::new() };
    let s = t3.add("shell", &[]);
    assert_eq!(t3.status(s).1.as_deref(), Some("Select faces to remove."));
}



/// QA3: a mate on the rim of a hole broke ("Mate connector could not be
/// resolved (missing geometry)") when a linear pattern whose hole cut into
/// that rim was suppressed: the reference held split face names
/// (Split{parent, by: pattern}) and the now-whole faces only match their
/// parents. References to split pieces now fall back to the whole face when
/// the splitting feature no longer splits it (Onshape keeps such mates).
#[test]
fn reference_to_split_pieces_survives_when_the_split_goes_away() {
    fn cut_circles(t: &mut T, circles: &[(DVec2, f64)]) -> FeatureId {
        let mut sk = noshape_sketch::Sketch::new();
        for (c, r) in circles {
            sk.add_circle(*c, *r);
        }
        let s = t.add("sketch", &[("sketchPlane", q(vec![Reference::Plane(DatumPlane::Top)])), ("sketch", ParamValue::Sketch(Box::new(SketchData { sketch: sk, ..Default::default() })))]);
        let rr = t.regen();
        let keys: Vec<_> = rr.display.sketches.iter().find(|x| x.feature == s).unwrap().regions.iter().map(|g| Reference::SketchRegion { sketch: s, key: g.key.clone() }).collect();
        t.add("extrude", &[("entities", q(keys)), ("endBound", en("THROUGH_ALL")), ("operationType", en("REMOVE"))])
    }
    let mut t = T::new();
    t.block();
    cut_circles(&mut t, &[(DVec2::new(0.010, 0.010), 0.005)]);
    // Two small holes cutting the Ø10 hole on both sides split its wall in two.
    let second = cut_circles(&mut t, &[(DVec2::new(0.0045, 0.010), 0.002), (DVec2::new(0.0155, 0.010), 0.002)]);
    let r = t.regen();
    let b = &r.context.bodies[0];
    // One of the Ø10 hole's top rim arcs, named after a split piece of the wall.
    let rim = b.shape.edges().iter().position(|e| {
        let i = e.edge_info().unwrap();
        i.start.z > 0.0099 && matches!(i.curve, noshape_kernel::Curve::Circle { radius, .. } if (radius - 0.005).abs() < 1e-7)
    }).expect("rim");
    let rim_name = b.edges[rim].clone();
    let noshape_model::reference::TopoName::Edge { faces, .. } = &*rim_name else { panic!("{rim_name:?}") };
    let wall_name = faces.iter().find(|f| matches!(&***f, noshape_model::reference::TopoName::Split { .. })).expect("rim names a split wall piece").clone();
    // Suppress the second hole: the rim is a full circle again.
    t.ps.feature_mut(second).unwrap().suppressed = true;
    let r = t.regen();
    let b = &r.context.bodies[0];
    let found = b.find_edges(&rim_name);
    assert_eq!(found.len(), 1, "rim edge not resolved");
    let info = b.shape.edges()[found[0]].edge_info().unwrap();
    assert!(matches!(info.curve, noshape_kernel::Curve::Circle { radius, .. } if (radius - 0.005).abs() < 1e-7) && info.start.z > 0.0099);
    assert_eq!(b.find_faces(&wall_name).len(), 1, "hole wall not resolved");
}

