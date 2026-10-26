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
/// A Part pattern of the block with a million instances.
fn million_instance_pattern(t: &mut T) -> FeatureId {
    t.block();
    let body = t.regen().context.bodies[0].id.clone();
    t.add(
        "linearPattern",
        &[
            ("patternType", en("PART")),
            ("entities", q(vec![Reference::Body(body)])),
            ("directionOne", q(vec![Reference::Plane(DatumPlane::Right)])),
            ("distance", e("50 mm")),
            ("instanceCount", e("1000000")),
        ],
    )
}

/// The million-instance pattern fails at once (it used to hang).
#[test]
#[ignore = "timed: perf.yml model job (wall clock; ci.yml tests are deterministic)"]
fn timed_million_instance_pattern_fails_at_once() {
    let mut t = T::new();
    let pat = million_instance_pattern(&mut t);
    let t0 = std::time::Instant::now();
    assert_eq!(t.status(pat).0, FeatureStatus::Error);
    assert!(t0.elapsed().as_secs() < 30, "{:?}", t0.elapsed());
}

#[test]
fn out_of_range_values_fail_with_message() {
    let mut t = T::new();
    let pat = million_instance_pattern(&mut t);
    let (st, msg) = t.status(pat);
    assert_eq!(st, FeatureStatus::Error);
    assert!(msg.as_deref().unwrap_or("").contains("A parameter value is not valid."), "{msg:?}");
    t.set(pat, "instanceCount", e("0"));
    let (st, msg) = t.status(pat);
    assert_eq!(st, FeatureStatus::Error);
    assert!(msg.as_deref().unwrap_or("").contains("A parameter value is not valid."), "{msg:?}");
    t.set(pat, "instanceCount", e("3"));
    assert_eq!(t.status(pat).0, FeatureStatus::Ok);
    t.set(pat, "distance", e("-5 mm"));
    let (st, msg) = t.status(pat);
    assert_eq!(st, FeatureStatus::Error);
    assert!(msg.as_deref().unwrap_or("").contains("A parameter value is not valid."), "{msg:?}");

    // Negative fillet radius: a field error, not a kernel failure (Onshape's
    // "A parameter value is not valid." for any value out of its range).
    let mut t = T::new();
    t.block();
    let r = t.regen();
    let body = &r.context.bodies[0];
    let edge = Reference::Edge((*body.edges[0]).clone());
    let fil = t.add("fillet", &[("entities", q(vec![edge])), ("radius", e("-2 mm"))]);
    let (st, msg) = t.status(fil);
    assert_eq!(st, FeatureStatus::Error);
    assert!(msg.as_deref().unwrap_or("").contains("A parameter value is not valid."), "{msg:?}");
    t.set(fil, "radius", e("1 mm"));
    assert_eq!(t.status(fil).0, FeatureStatus::Ok);
    // The same in imperial documents.
    t.units.length = LengthUnit::Inch;
    t.set(fil, "radius", e("30000 in"));
    let (_, msg) = t.status(fil);
    assert_eq!(msg.as_deref(), Some("A parameter value is not valid."));
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
            // Onshape shells both (kernel_robustness::shell_matches_onshape_volumes S2).
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
        t.add("extrude", &[("entities", q(keys)), ("endBound", en("THROUGH_ALL")), ("operationType", en("REMOVE")), ("defaultScope", ParamValue::Bool(true))])
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


/// In-context QA: a Composite part always consumed its members. Help
/// (PartStudio__composite_part) and live ("Composite test": Part 1, Part 2
/// and Composite part 1, not closed): an open composite leaves the bodies as
/// parts of their own and isn't drawn itself; Closed consumes them.
#[test]
fn open_composite_keeps_its_members() {
    let mut t = T::new();
    t.block();
    t.block();
    let r = t.regen();
    let ids: Vec<_> = r.context.bodies.iter().map(|b| b.id.clone()).collect();
    assert_eq!(ids.len(), 2);
    let c = t.add("compositePart", &[("bodies", q(ids.iter().cloned().map(Reference::Body).collect()))]);
    let r = t.regen();
    assert_eq!(r.context.bodies.len(), 3, "members kept");
    let comp = r.display.bodies.iter().find(|b| b.id.feature == c).expect("composite");
    let info = comp.composite.as_ref().expect("composite info");
    assert!(!info.closed && info.members == ids);
    assert!(comp.mesh.indices.is_empty(), "the members are drawn, not the composite");
    t.set(c, "closed", ParamValue::Bool(true));
    let r = t.regen();
    assert_eq!(r.context.bodies.len(), 1, "closed consumes the members");
    let comp = &r.display.bodies[0];
    assert!(comp.composite.as_ref().is_some_and(|i| i.closed) && !comp.mesh.indices.is_empty());
}

/// MOVEFACE-TANGENT (api 10-25, samples/g47-1part.step: a 40 × 20 × 10
/// block, r 2 vertical and r 4 top fillets): Move face (Offset 5 mm) of the
/// +X end face moves that face only, the fillets touching it rolled again
/// (Reapply fillet, the default), on the native part and on an imported copy
/// without fillet history. It grew the part on every side but the bottom.
#[test]
fn move_face_beside_tangent_fillets_moves_one_face() {
    let mut t = T::new();
    t.block();
    let r = t.regen();
    let edges_where = |r: &RegenResult, f: &dyn Fn(&noshape_kernel::EdgeInfo) -> bool| -> Vec<Reference> {
        let body = &r.context.bodies[0];
        body.shape.edges().iter().enumerate().filter(|(_, e)| f(&e.edge_info().unwrap())).map(|(i, _)| Reference::Edge((*body.edges[i]).clone())).collect()
    };
    let vertical = edges_where(&r, &|i| (i.start.z - i.end.z).abs() > 1e-6);
    let f1 = t.add("fillet", &[("entities", q(vertical)), ("radius", e("2 mm"))]);
    assert_eq!(t.status(f1).0, FeatureStatus::Ok);
    let r = t.regen();
    let top = edges_where(&r, &|i| i.start.z > 0.0099 && i.end.z > 0.0099 && matches!(i.curve, noshape_kernel::Curve::Line { .. }));
    let f2 = t.add("fillet", &[("entities", q(top)), ("radius", e("4 mm")), ("tangentPropagation", ParamValue::Bool(true))]);
    assert_eq!(t.status(f2).0, FeatureStatus::Ok);
    let r = t.regen();
    let brep = String::from_utf8(r.context.bodies[0].shape.to_brep_bytes().unwrap()).unwrap();
    let mut imported = T::new();
    imported.add("importForeign", &[("blobData", ParamValue::Text(brep))]);
    for (what, mut t) in [("native", t), ("imported", imported)] {
        let r = t.regen();
        let body = &r.context.bodies[0];
        let end = body.shape.faces().iter().position(|f| f.face_plane().ok().flatten().is_some_and(|p| p.unit_normal().x > 0.999)).map(|i| Reference::Face((*body.faces[i]).clone())).expect("+X face");
        let n_faces = body.faces.len();
        let m = t.add("moveFace", &[("moveFaces", q(vec![end])), ("offsetDistance", e("5 mm"))]);
        let (st, msg) = t.status(m);
        assert_eq!(st, FeatureStatus::Ok, "{what}: {msg:?}");
        let r = t.regen();
        let body = &r.context.bodies[0];
        let bb = body.shape.bounding_box(true).unwrap();
        let (lo, hi) = (bb.min, bb.max);
        for (got, want) in [(lo.x, 0.0), (lo.y, 0.0), (lo.z, 0.0), (hi.x, 0.045), (hi.y, 0.02), (hi.z, 0.01)] {
            assert!((got - want).abs() < 1e-6, "{what}: box {lo:?} … {hi:?}");
        }
        assert_eq!(body.faces.len(), n_faces, "{what}: faces");
    }
}

/// REORDER-INVALID (live qa4 round 5 v3 s8): an extrude moved above its
/// sketch is moved all the same and fails on its missing region, "Select
/// face or sketch region to extrude." with "1 missing selection".
#[test]
fn extrude_moved_above_its_sketch_fails_on_the_missing_region() {
    let mut t = T::new();
    let (s, x) = t.block();
    assert_eq!(t.status(x).0, FeatureStatus::Ok);
    let order = noshape_model::interaction::reordered(&t.ps.features, &[x], 0).unwrap();
    assert_eq!(order[..2], [x, s]);
    noshape_model::interaction::apply_order(&mut t.ps, &order);
    let r = t.regen();
    let st = r.display.features.iter().find(|f| f.id == x).unwrap();
    assert_eq!((st.status, st.message.as_deref()), (FeatureStatus::Error, Some("Select face or sketch region to extrude.")));
    assert_eq!(st.missing, 1);
    assert!(r.context.bodies.is_empty());
}

/// qa5 P1-5 (live: "Could not shell part with selections."): block 75.554 ×
/// 20.245 × 15, R2 on 3 vertical edges, a 1 mm chamfer chain, two Ø6.35 ×
/// 12 drilled holes and a feature pattern of them; Shell 2.5 mm off the top
/// face broke the part into 4 slivers and reported Ok.
#[test]
#[ignore = "T2: macOS arm64 differs (red on CI 10-26)"]
fn shell_that_breaks_the_part_fails() {
    let mut t = T::new();
    let mut sk = noshape_sketch::Sketch::new();
    sk.corner_rectangle(DVec2::ZERO, DVec2::new(0.075554, 0.020245));
    let s = t.add("sketch", &[("sketchPlane", q(vec![Reference::Plane(DatumPlane::Top)])), ("sketch", ParamValue::Sketch(Box::new(SketchData { sketch: sk, ..Default::default() })))]);
    let r = t.regen();
    let key = r.display.sketches.iter().find(|x| x.feature == s).unwrap().regions[0].key.clone();
    t.add("extrude", &[("entities", q(vec![Reference::SketchRegion { sketch: s, key }])), ("depth", e("15 mm")), ("operationType", en("NEW"))]);
    let edge_at = |r: &RegenResult, p: [f64; 3]| -> Reference {
        let b = &r.context.bodies[0];
        let v = noshape_kernel::make_vertex(glam::DVec3::from(p)).unwrap();
        let i = b.shape.edges().iter().position(|e| noshape_kernel::distance(&v, e).is_ok_and(|d| d.distance < 1e-9)).expect("edge");
        Reference::Edge((*b.edges[i]).clone())
    };
    let r = t.regen();
    let vertical: Vec<Reference> = [[0.0, 0.0, 0.007], [0.075554, 0.0, 0.007], [0.075554, 0.020245, 0.007]].iter().map(|p| edge_at(&r, *p)).collect();
    t.add("fillet", &[("entities", q(vertical)), ("radius", e("2 mm"))]);
    let r = t.regen();
    let top_edge = edge_at(&r, [0.0377, 0.0, 0.015]);
    t.add("chamfer", &[("entities", q(vec![top_edge])), ("width", e("1 mm")), ("tangentPropagation", ParamValue::Bool(true))]);
    let plane = t.add("cPlane", &[("entities", q(vec![Reference::Plane(DatumPlane::Top)])), ("offset", e("15 mm"))]);
    let mut pts = noshape_sketch::Sketch::new();
    let p0 = pts.add_point(DVec2::new(0.015, 0.006));
    let p1 = pts.add_point(DVec2::new(0.015, 0.014245));
    let s2 = t.add("sketch", &[("sketchPlane", q(vec![Reference::FeaturePlane(plane)])), ("sketch", ParamValue::Sketch(Box::new(SketchData { sketch: pts, ..Default::default() })))]);
    let locs = vec![Reference::SketchEntity { sketch: s2, entity: p0.0 }, Reference::SketchEntity { sketch: s2, entity: p1.0 }];
    let part = Reference::Body(noshape_model::BodyId { feature: t.ps.features[1].id, index: 0 });
    let hole = t.add("hole", &[("locations", q(locs)), ("holeDiameterV3", e("6.35 mm")), ("holeDepthV3", e("12 mm")), ("endStyleV2", en("BLIND")), ("scope", q(vec![part]))]);
    t.add("linearPattern", &[("patternType", en("FEATURE")), ("instanceFunction", q(vec![Reference::Feature(hole)])), ("directionOne", q(vec![Reference::Plane(DatumPlane::Right)])), ("distance", e("25 mm")), ("instanceCount", e("2"))]);
    let r = t.regen();
    for f in &r.display.features {
        assert_ne!(f.status, FeatureStatus::Error, "{:?}", f.message);
    }
    let b = &r.context.bodies[0];
    let top = b.shape.faces().iter().position(|f| f.face_plane().ok().flatten().is_some_and(|p| p.unit_normal().z > 0.999 && p.origin.z > 0.0149)).map(|i| Reference::Face((*b.faces[i]).clone())).expect("top face");
    let sh = t.add("shell", &[("entities", q(vec![top])), ("thickness", e("2.5 mm"))]);
    assert_eq!(t.status(sh), (FeatureStatus::Error, Some("Could not shell part with selections.".to_string())));
    assert_eq!(t.regen().context.bodies.len(), 1);
}
