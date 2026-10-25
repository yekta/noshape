//! Feature evaluation engine: feature patterns that re-apply features,
//! thin extrude / revolve / sweep / loft, two-sided and up-to end types,
//! query variables, hole table edit logic and Edit curve control points.

use glam::{DVec2, DVec3};
use noshape_model::configuration::ConfigValues;
use noshape_model::display::{BodyKind, FeatureStatus};
use noshape_model::params::{ParamValue, ParamValues};
use noshape_model::partstudio::PartStudio;
use noshape_model::reference::{BodyId, DatumPlane, FeatureId, Reference};
use noshape_model::regen::context::{Body, Context};
use noshape_model::regen::{RegenOptions, RegenResult, Regenerator};
use noshape_model::sketch_feature::SketchData;
use noshape_units::{ExprValue, UnitPrefs};
use std::sync::Arc;

const PI: f64 = std::f64::consts::PI;

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
fn b(v: bool) -> ParamValue {
    ParamValue::Bool(v)
}

impl T {
    fn new() -> Self {
        T { ps: PartStudio::new(), units: UnitPrefs::default(), reg: Regenerator::new() }
    }
    fn regen(&mut self) -> RegenResult {
        self.reg.regenerate(&self.ps, &ConfigValues::default(), &self.units, &RegenOptions::default())
    }
    fn add(&mut self, ty: &str, params: &[(&str, ParamValue)]) -> FeatureId {
        // A Hole needs a Merge scope (live L56): the parts so far, as the
        // dialog fills it, unless the test gives one.
        let scope = (ty == "hole" && !params.iter().any(|(k, _)| *k == "scope")).then(|| {
            let r = self.regen();
            r.context.bodies.iter().filter(|b| b.kind == noshape_model::display::BodyKind::Solid).map(|b| Reference::Body(b.id.clone())).collect::<Vec<_>>()
        });
        let mut f = self.ps.new_feature(ty, &self.units).unwrap_or_else(|| panic!("no feature type {ty}"));
        if let Some(scope) = scope {
            f.params.set("scope", ParamValue::Query(scope));
        }
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
    fn sketch(&mut self, plane: Reference, sk: noshape_sketch::Sketch) -> FeatureId {
        self.add("sketch", &[("sketchPlane", q(vec![plane])), ("sketch", ParamValue::Sketch(Box::new(SketchData { sketch: sk, ..Default::default() })))])
    }
    fn set_sketch(&mut self, id: FeatureId, sk: noshape_sketch::Sketch) {
        self.set(id, "sketch", ParamValue::Sketch(Box::new(SketchData { sketch: sk, ..Default::default() })));
    }
    /// Region keys of a sketch, largest first.
    fn regions(&mut self, sketch: FeatureId) -> Vec<Reference> {
        let r = self.regen();
        let s = r.display.sketches.iter().find(|s| s.feature == sketch).expect("sketch displayed");
        let mut regs: Vec<_> = s.regions.iter().collect();
        regs.sort_by(|a, b| b.area.partial_cmp(&a.area).unwrap());
        regs.iter().map(|x| Reference::SketchRegion { sketch, key: x.key.clone() }).collect()
    }
    fn extrude(&mut self, region: Reference, depth: &str, op: &str) -> FeatureId {
        // Add / Remove merge with what they touch ("Merge with all").
        self.add("extrude", &[("entities", q(vec![region])), ("depth", e(depth)), ("operationType", en(op)), ("defaultScope", ParamValue::Bool(op != "NEW"))])
    }
    /// Block [x0,x1]x[y0,y1]x[0,h] on the Top plane.
    fn block(&mut self, x0: f64, y0: f64, x1: f64, y1: f64, h: &str) -> (FeatureId, FeatureId) {
        let s = self.sketch(Reference::Plane(DatumPlane::Top), rect(x0, y0, x1, y1));
        let r = self.regions(s)[0].clone();
        let x = self.extrude(r, h, "NEW");
        (s, x)
    }
    /// Offset plane parallel to Top.
    fn top_plane(&mut self, z: &str) -> FeatureId {
        self.add("cPlane", &[("entities", q(vec![Reference::Plane(DatumPlane::Top)])), ("cplaneType", en("OFFSET")), ("offset", e(z))])
    }
    fn ok(&mut self) -> RegenResult {
        let r = self.regen();
        for (i, s) in r.display.features.iter().enumerate() {
            assert!(matches!(s.status, FeatureStatus::Ok | FeatureStatus::Info), "feature {i} ({}) {:?}: {:?}", self.ps.features[i].type_id, s.status, s.message);
        }
        r
    }
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

fn circle(c: DVec2, r: f64) -> noshape_sketch::Sketch {
    let mut sk = noshape_sketch::Sketch::new();
    sk.add_circle(c, r);
    sk
}

fn total_volume(ctx: &Context) -> f64 {
    ctx.bodies.iter().filter(|b| b.kind == BodyKind::Solid).filter_map(|b| b.shape.volume().ok()).sum()
}

fn near(a: f64, b: f64, rel: f64) -> bool {
    (a - b).abs() <= rel * b.abs().max(1e-12)
}

/// Edge of a body nearest to a point.
fn edge_near(b: &Body, p: DVec3) -> Reference {
    let (i, _) = b
        .shape
        .edges()
        .iter()
        .enumerate()
        .map(|(i, e)| {
            let v = noshape_kernel::make_vertex(p).unwrap();
            let d = noshape_kernel::distance(&v, e).unwrap().distance;
            // Prefer edges through the point; break ties by midpoint distance.
            let info = e.edge_info().unwrap();
            let m = e.point_at((info.first_param + info.last_param) * 0.5).unwrap();
            (i, d * 1e3 + m.distance(p))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .unwrap();
    Reference::Edge((*b.edges[i]).clone())
}

fn copies(b: &Arc<Body>) -> usize {
    b.faces.iter().filter(|n| format!("{n:?}").contains("Copy")).count()
}

// ------------------------------------------------------------------ feature pattern (reapply)

const BLOCK_V: f64 = 0.1 * 0.02 * 0.01;

/// Block 100x20x10 mm with a cut cylinder pocket at (10, 10) mm, r = 3 mm,
/// 5 mm deep. Returns (pocket sketch, cut).
fn pocket_setup(t: &mut T) -> (FeatureId, FeatureId) {
    t.block(0.0, 0.0, 0.1, 0.02, "10 mm");
    let pl = t.top_plane("10 mm");
    let cs = t.sketch(Reference::FeaturePlane(pl), circle(DVec2::new(0.01, 0.01), 0.003));
    let r = t.regions(cs)[0].clone();
    let cut = t.extrude(r, "5 mm", "REMOVE");
    t.set(cut, "oppositeDirection", b(true));
    (cs, cut)
}

fn feature_pattern(t: &mut T, feats: Vec<FeatureId>, count: &str, reapply: bool) -> FeatureId {
    t.add(
        "linearPattern",
        &[
            ("patternType", en("FEATURE")),
            ("instanceFunction", q(feats.into_iter().map(Reference::Feature).collect())),
            ("directionOne", q(vec![Reference::Plane(DatumPlane::Right)])),
            ("distance", e("20 mm")),
            ("instanceCount", e(count)),
            ("fullFeaturePattern", b(reapply)),
        ],
    )
}

#[test]
fn reapplied_pocket_pattern_follows_seed_edits() {
    let mut t = T::new();
    let (cs, cut) = pocket_setup(&mut t);
    feature_pattern(&mut t, vec![cut], "4", true);
    let res = t.ok();
    assert_eq!(res.context.bodies.len(), 1);
    let pocket = |r: f64, d: f64| PI * r * r * d;
    let removed = BLOCK_V - total_volume(&res.context);
    assert!(near(removed, 4.0 * pocket(0.003, 0.005), 1e-3), "{removed}");
    // Wall + floor of three instances are named as copies.
    assert_eq!(copies(&res.context.bodies[0]), 6);
    // Editing the seed sketch updates every instance.
    t.set_sketch(cs, circle(DVec2::new(0.01, 0.01), 0.004));
    let res = t.ok();
    let removed = BLOCK_V - total_volume(&res.context);
    assert!(near(removed, 4.0 * pocket(0.004, 0.005), 1e-3), "{removed}");
    // ... and so does editing the seed feature.
    t.set(cut, "depth", e("7 mm"));
    let res = t.ok();
    let removed = BLOCK_V - total_volume(&res.context);
    assert!(near(removed, 4.0 * pocket(0.004, 0.007), 1e-3), "{removed}");
}

#[test]
fn reapplied_boss_pattern_and_mirror() {
    let mut t = T::new();
    t.block(0.0, 0.0, 0.1, 0.02, "10 mm");
    let pl = t.top_plane("10 mm");
    let cs = t.sketch(Reference::FeaturePlane(pl), circle(DVec2::new(0.01, 0.01), 0.003));
    let r = t.regions(cs)[0].clone();
    let boss = t.extrude(r, "5 mm", "ADD");
    let boss_v = PI * 0.003f64.powi(2) * 0.005;
    feature_pattern(&mut t, vec![boss], "3", true);
    let res = t.ok();
    assert_eq!(res.context.bodies.len(), 1, "bosses merge into the part");
    assert!(near(total_volume(&res.context), BLOCK_V + 3.0 * boss_v, 1e-4), "{}", total_volume(&res.context));
    // Mirror of the boss across x = 50 mm (Reapply is the Mirror default).
    let mp = t.add("cPlane", &[("entities", q(vec![Reference::Plane(DatumPlane::Right)])), ("cplaneType", en("OFFSET")), ("offset", e("50 mm"))]);
    t.add("mirror", &[("patternType", en("FEATURE")), ("instanceFunction", q(vec![Reference::Feature(boss)])), ("mirrorPlane", q(vec![Reference::FeaturePlane(mp)]))]);
    let res = t.ok();
    assert!(near(total_volume(&res.context), BLOCK_V + 4.0 * boss_v, 1e-4), "{}", total_volume(&res.context));
    let bb = res.context.bodies[0].shape.bounding_box(true).unwrap();
    assert!((bb.max.z - 0.015).abs() < 1e-6);
}

#[test]
fn reapplied_instance_references_survive_edits() {
    let mut t = T::new();
    let (cs, cut) = pocket_setup(&mut t);
    let fp = feature_pattern(&mut t, vec![cut], "3", true);
    let res = t.ok();
    let body = res.context.bodies[0].clone();
    // Rim edge of the third pocket (x = 50 mm).
    let rim = edge_near(&body, DVec3::new(0.05, 0.01 + 0.003, 0.01));
    assert!(format!("{rim:?}").contains("Copy"), "{rim:?}");
    t.add("chamfer", &[("entities", q(vec![rim])), ("width", e("0.5 mm"))]);
    let res = t.ok();
    let f0 = res.context.bodies[0].faces.len();
    // Upstream edits: seed depth, then the seed sketch.
    t.set(cut, "depth", e("6 mm"));
    let res = t.ok();
    assert_eq!(res.context.bodies[0].faces.len(), f0);
    t.set_sketch(cs, circle(DVec2::new(0.01, 0.01), 0.0035));
    let res = t.ok();
    assert_eq!(res.context.bodies[0].faces.len(), f0);
    // The geometric path names instances the same way: the chamfer still resolves.
    t.set(fp, "fullFeaturePattern", b(false));
    let res = t.ok();
    assert_eq!(res.context.bodies[0].faces.len(), f0);
}

#[test]
fn reapply_reevaluates_up_to_next_per_instance() {
    let mut t = T::new();
    t.block(0.0, 0.0, 0.1, 0.02, "10 mm");
    // Roof: high over x < 50 mm, low over x > 50 mm.
    let p30 = t.top_plane("30 mm");
    let s1 = t.sketch(Reference::FeaturePlane(p30), rect(0.0, 0.0, 0.05, 0.02));
    let r1 = t.regions(s1)[0].clone();
    t.extrude(r1, "10 mm", "NEW");
    let p20 = t.top_plane("20 mm");
    let s2 = t.sketch(Reference::FeaturePlane(p20), rect(0.05, 0.0, 0.1, 0.02));
    let r2 = t.regions(s2)[0].clone();
    t.extrude(r2, "20 mm", "NEW");
    // Pillar from the block top up to the next part.
    let p10 = t.top_plane("10 mm");
    let cs = t.sketch(Reference::FeaturePlane(p10), circle(DVec2::new(0.03, 0.01), 0.003));
    let r = t.regions(cs)[0].clone();
    let pillar = t.add("extrude", &[("entities", q(vec![r])), ("endBound", en("UP_TO_NEXT")), ("operationType", en("NEW"))]);
    let fp = t.add(
        "linearPattern",
        &[
            ("patternType", en("FEATURE")),
            ("instanceFunction", q(vec![Reference::Feature(pillar)])),
            ("directionOne", q(vec![Reference::Plane(DatumPlane::Right)])),
            ("distance", e("40 mm")),
            ("instanceCount", e("2")),
            ("fullFeaturePattern", b(true)),
        ],
    );
    let res = t.ok();
    let area = PI * 0.003f64.powi(2);
    let pillars: Vec<f64> = res.context.bodies.iter().filter(|b| b.id.feature == pillar || b.id.feature == fp).map(|b| b.shape.volume().unwrap()).collect();
    assert_eq!(pillars.len(), 2);
    assert!(pillars.iter().any(|v| near(*v, area * 0.02, 1e-4)), "seed pillar 20 mm: {pillars:?}");
    assert!(pillars.iter().any(|v| near(*v, area * 0.01, 1e-4)), "instance pillar 10 mm: {pillars:?}");
    // Without Reapply the instance is a copy of the seed (20 mm).
    t.set(fp, "fullFeaturePattern", b(false));
    let res = t.ok();
    let pillars: Vec<f64> = res.context.bodies.iter().filter(|b| b.id.feature == pillar || b.id.feature == fp).map(|b| b.shape.volume().unwrap()).collect();
    assert!(pillars.iter().all(|v| near(*v, area * 0.02, 1e-4)), "{pillars:?}");
}

#[test]
fn reapply_respects_feature_list_order() {
    // Boss then a hole through the boss, selected in reverse order.
    let mut t = T::new();
    t.block(0.0, 0.0, 0.1, 0.02, "10 mm");
    let pl = t.top_plane("10 mm");
    let cs = t.sketch(Reference::FeaturePlane(pl), circle(DVec2::new(0.01, 0.01), 0.004));
    let r = t.regions(cs)[0].clone();
    let boss = t.extrude(r, "5 mm", "ADD");
    let pl2 = t.top_plane("15 mm");
    let hs = t.sketch(Reference::FeaturePlane(pl2), circle(DVec2::new(0.01, 0.01), 0.002));
    let hr = t.regions(hs)[0].clone();
    let hole = t.extrude(hr, "5 mm", "REMOVE");
    t.set(hole, "oppositeDirection", b(true));
    feature_pattern(&mut t, vec![hole, boss], "2", true);
    let res = t.ok();
    let ring = PI * (0.004f64.powi(2) - 0.002f64.powi(2)) * 0.005;
    assert!(near(total_volume(&res.context), BLOCK_V + 2.0 * ring, 1e-4), "{}", total_volume(&res.context));
}

// ------------------------------------------------------------------ thin

fn thin_extrude(t: &mut T, wall: Vec<Reference>, params: &[(&str, ParamValue)]) -> FeatureId {
    let mut all: Vec<(&str, ParamValue)> = vec![("bodyType", en("THIN")), ("wallShape", q(wall)), ("depth", e("10 mm"))];
    all.extend(params.iter().cloned());
    t.add("extrude", &all)
}

#[test]
fn thin_extrude_closed_and_open_profiles() {
    // Closed rectangle region, wall outside.
    let mut t = T::new();
    let s = t.sketch(Reference::Plane(DatumPlane::Top), rect(0.0, 0.0, 0.04, 0.02));
    let r = t.regions(s)[0].clone();
    let x = thin_extrude(&mut t, vec![r], &[("thickness1", e("2 mm"))]);
    let res = t.ok();
    assert!(near(total_volume(&res.context), (0.044 * 0.024 - 0.04 * 0.02) * 0.01, 1e-6), "{}", total_volume(&res.context));
    // Flip wall: inside.
    t.set(x, "flipWall", b(true));
    let res = t.ok();
    assert!(near(total_volume(&res.context), (0.04 * 0.02 - 0.036 * 0.016) * 0.01, 1e-6), "{}", total_volume(&res.context));
    // Mid plane.
    t.set(x, "midplane", b(true));
    t.set(x, "thickness", e("2 mm"));
    let res = t.ok();
    assert!(near(total_volume(&res.context), (0.042 * 0.022 - 0.038 * 0.018) * 0.01, 1e-6), "{}", total_volume(&res.context));
    // Wall faces have distinct names.
    let names = &res.context.bodies[0].faces;
    let mut u = names.clone();
    u.sort();
    u.dedup();
    assert_eq!(u.len(), names.len());

    // Open line (live "psb (10-24 e)"): thickness 1 on the right of its
    // direction seen down the sketch normal, thickness 2 left.
    let mut t = T::new();
    let mut sk = noshape_sketch::Sketch::new();
    let l = sk.add_line(DVec2::new(0.0, 0.0), DVec2::new(0.04, 0.0));
    let s = t.sketch(Reference::Plane(DatumPlane::Top), sk);
    thin_extrude(&mut t, vec![Reference::SketchEntity { sketch: s, entity: l.0 }], &[("thickness1", e("2 mm")), ("thickness2", e("1 mm"))]);
    let res = t.ok();
    assert!(near(total_volume(&res.context), 0.04 * 0.003 * 0.01, 1e-6), "{}", total_volume(&res.context));
    let bb = res.context.bodies[0].shape.bounding_box(true).unwrap();
    assert!((bb.min.y + 0.002).abs() < 1e-7 && (bb.max.y - 0.001).abs() < 1e-7, "{bb:?}");

    // Open L polyline turning left: wall on the outside of the corner.
    let mut t = T::new();
    let mut sk = noshape_sketch::Sketch::new();
    let p: Vec<_> = [(0.0, 0.0), (0.04, 0.0), (0.04, 0.03)].iter().map(|(x, y)| sk.add_point(DVec2::new(*x, *y))).collect();
    let l1 = sk.add_line_between(p[0], p[1]).unwrap();
    let l2 = sk.add_line_between(p[1], p[2]).unwrap();
    let s = t.sketch(Reference::Plane(DatumPlane::Top), sk);
    thin_extrude(&mut t, vec![Reference::SketchEntity { sketch: s, entity: l1.0 }, Reference::SketchEntity { sketch: s, entity: l2.0 }], &[("thickness1", e("2 mm"))]);
    let res = t.ok();
    let area = 0.042 * 0.002 + 0.002 * 0.03;
    assert!(near(total_volume(&res.context), area * 0.01, 1e-6), "{}", total_volume(&res.context));
}

/// A Thin wall that is one sketch circle edge: OCCT's 2D wire offset
/// (BRepOffsetAPI_MakeOffset) segfaulted on it, so the circle is offset
/// directly. Outward, inward (Flip wall), both sides, and too thick inward
/// (an error, not a crash).
#[test]
fn thin_wall_of_a_lone_circle_edge() {
    let run = |params: &[(&str, ParamValue)]| {
        let mut t = T::new();
        let mut sk = noshape_sketch::Sketch::new();
        let c = sk.add_circle(DVec2::ZERO, 0.01);
        let s = t.sketch(Reference::Plane(DatumPlane::Top), sk);
        thin_extrude(&mut t, vec![Reference::SketchEntity { sketch: s, entity: c.0 }], params);
        let res = t.regen();
        let st = res.display.features.last().unwrap().clone();
        (st.status, total_volume(&res.context))
    };
    let ring = |r1: f64, r2: f64| PI * (r2 * r2 - r1 * r1) * 0.01;
    let (st, v) = run(&[("thickness1", e("2 mm"))]);
    assert!(st == FeatureStatus::Ok && near(v, ring(0.01, 0.012), 1e-7), "{st:?} {v}");
    let (st, v) = run(&[("thickness1", e("2 mm")), ("flipWall", b(true))]);
    assert!(st == FeatureStatus::Ok && near(v, ring(0.008, 0.01), 1e-7), "{st:?} {v}");
    let (st, v) = run(&[("midplane", b(true)), ("thickness", e("4 mm"))]);
    assert!(st == FeatureStatus::Ok && near(v, ring(0.008, 0.012), 1e-7), "{st:?} {v}");
    let (st, _) = run(&[("thickness1", e("15 mm")), ("flipWall", b(true))]);
    assert_eq!(st, FeatureStatus::Error);
}

#[test]
fn thin_revolve_sweep_and_loft() {
    // Revolve: a line at x = 10 mm on the Front plane, full turn about Z.
    let mut t = T::new();
    let mut sk = noshape_sketch::Sketch::new();
    let l = sk.add_line(DVec2::new(0.01, 0.0), DVec2::new(0.01, 0.02));
    let s = t.sketch(Reference::Plane(DatumPlane::Front), sk);
    let rv = t.add(
        "revolve",
        &[("bodyType", en("THIN")), ("wallShape", q(vec![Reference::SketchEntity { sketch: s, entity: l.0 }])), ("axis", q(vec![Reference::Plane(DatumPlane::Top)])), ("thickness1", e("2 mm"))],
    );
    let res = t.ok();
    let v = total_volume(&res.context);
    let inner = PI * (0.01f64.powi(2) - 0.008f64.powi(2)) * 0.02;
    let outer = PI * (0.012f64.powi(2) - 0.01f64.powi(2)) * 0.02;
    assert!(near(v, inner, 1e-4) || near(v, outer, 1e-4), "{v}");
    t.set(rv, "flipWall", b(true));
    let v2 = total_volume(&t.ok().context);
    assert!(near(v + v2, inner + outer, 1e-4), "{v} + {v2}");

    // Sweep: a circle wall along a line.
    let mut t = T::new();
    let prof = t.sketch(Reference::Plane(DatumPlane::Front), circle(DVec2::ZERO, 0.005));
    let mut path = noshape_sketch::Sketch::new();
    let pl = path.add_line(DVec2::new(0.0, 0.0), DVec2::new(0.0, 0.1));
    let path_id = t.sketch(Reference::Plane(DatumPlane::Top), path);
    let r = t.regions(prof)[0].clone();
    t.add("sweep", &[("bodyType", en("THIN")), ("wallShape", q(vec![r])), ("thickness1", e("1 mm")), ("path", q(vec![Reference::SketchEntity { sketch: path_id, entity: pl.0 }]))]);
    let res = t.ok();
    let v = total_volume(&res.context);
    assert!(near(v, PI * (0.006f64.powi(2) - 0.005f64.powi(2)) * 0.1, 2e-3), "{v}");

    // Loft: two circles 30 mm apart, thickened.
    let mut t = T::new();
    let c1 = t.sketch(Reference::Plane(DatumPlane::Top), circle(DVec2::ZERO, 0.01));
    let p30 = t.top_plane("30 mm");
    let c2 = t.sketch(Reference::FeaturePlane(p30), circle(DVec2::ZERO, 0.01));
    let item = |s: FeatureId| {
        let mut v = ParamValues::default();
        v.set("wireProfileEntities", q(vec![Reference::Sketch(s)]));
        v
    };
    t.add("loft", &[("bodyType", en("THIN")), ("wireProfilesArray", ParamValue::Array(vec![item(c1), item(c2)])), ("thickness1", e("1 mm"))]);
    let res = t.ok();
    let v = total_volume(&res.context);
    let a = PI * (0.011f64.powi(2) - 0.01f64.powi(2)) * 0.03;
    let b2 = PI * (0.01f64.powi(2) - 0.009f64.powi(2)) * 0.03;
    assert!(near(v, a, 2e-3) || near(v, b2, 2e-3), "{v}");
}

// ------------------------------------------------------------------ end types

#[test]
fn extrude_second_direction_up_to_next() {
    let mut t = T::new();
    t.block(0.0, 0.0, 0.02, 0.02, "10 mm");
    let p50 = t.top_plane("50 mm");
    let s = t.sketch(Reference::FeaturePlane(p50), rect(0.005, 0.005, 0.015, 0.015));
    let r = t.regions(s)[0].clone();
    // A point at z = 70 mm for the up-to-vertex case below.
    let mut sk = noshape_sketch::Sketch::new();
    let pt = sk.add_point(DVec2::new(0.0, 0.0));
    let p70 = t.top_plane("70 mm");
    let ps = t.sketch(Reference::FeaturePlane(p70), sk);
    let x = t.extrude(r, "10 mm", "NEW");
    t.set(x, "hasSecondDirection", b(true));
    t.set(x, "secondDirectionBound", en("UP_TO_NEXT"));
    let res = t.ok();
    let nb = res.context.bodies.iter().find(|b| b.id.feature == x).unwrap();
    assert!(near(nb.shape.volume().unwrap(), 1e-4 * 0.05, 1e-6), "{}", nb.shape.volume().unwrap());
    let bb = nb.shape.bounding_box(true).unwrap();
    assert!((bb.min.z - 0.01).abs() < 1e-7 && (bb.max.z - 0.06).abs() < 1e-7, "{bb:?}");
    // Coplanar side faces of both halves are unified: a plain box (6 faces).
    assert_eq!(nb.faces.len(), 6);
    // First direction up to a vertex, second blind.
    t.set(x, "endBound", en("UP_TO_VERTEX"));
    t.set(x, "endBoundEntityVertex", q(vec![Reference::SketchEntity { sketch: ps, entity: pt.0 }]));
    t.set(x, "secondDirectionBound", en("BLIND"));
    t.set(x, "secondDirectionDepth", e("5 mm"));
    let res = t.ok();
    let nb = res.context.bodies.iter().find(|b| b.id.feature == x).unwrap();
    assert!(near(nb.shape.volume().unwrap(), 1e-4 * 0.025, 1e-6), "{}", nb.shape.volume().unwrap());
}

#[test]
fn revolve_up_to_vertex_face_and_next() {
    let quarter = 0.25 * PI * (0.02f64.powi(2) - 0.01f64.powi(2)) * 0.01;
    let setup = |t: &mut T| -> FeatureId {
        let s = t.sketch(Reference::Plane(DatumPlane::Front), rect(0.01, 0.0, 0.02, 0.01));
        let r = t.regions(s)[0].clone();
        t.add("revolve", &[("entities", q(vec![r])), ("axis", q(vec![Reference::Plane(DatumPlane::Top)])), ("fullRevolve", b(false))])
    };
    // Up to vertex at +Y (90 degrees).
    let mut t = T::new();
    let mut sk = noshape_sketch::Sketch::new();
    let p = sk.add_point(DVec2::new(0.0, 0.015));
    let ps = t.sketch(Reference::Plane(DatumPlane::Top), sk);
    let rv = setup(&mut t);
    t.set(rv, "endBound", en("UP_TO_VERTEX"));
    t.set(rv, "endBoundEntityVertex", q(vec![Reference::SketchEntity { sketch: ps, entity: p.0 }]));
    let res = t.ok();
    assert!(near(total_volume(&res.context), quarter, 1e-6), "{}", total_volume(&res.context));
    // With a 30 degree offset.
    t.set(rv, "endBoundHasOffset", b(true));
    t.set(rv, "endBoundOffset", e("30 deg"));
    let res = t.ok();
    assert!(near(total_volume(&res.context), quarter * 60.0 / 90.0, 1e-6), "{}", total_volume(&res.context));

    // Up to the Right plane (first hit at 90 degrees).
    let mut t = T::new();
    let rv = setup(&mut t);
    t.set(rv, "endBound", en("UP_TO_SURFACE"));
    t.set(rv, "endBoundEntityFace", q(vec![Reference::Plane(DatumPlane::Right)]));
    let res = t.ok();
    assert!(near(total_volume(&res.context), quarter, 1e-5), "{}", total_volume(&res.context));
    let names = &res.context.bodies[0].faces;
    assert!(names.iter().any(|n| format!("{n:?}").contains("CapEnd")), "{names:?}");

    // Up to next: a wall block at -X stops the revolve at 180 degrees... and
    // a second end blind 45 degrees backwards.
    let mut t = T::new();
    t.block(-0.03, -0.001, -0.005, 0.001, "20 mm");
    let rv = setup(&mut t);
    t.set(rv, "endBound", en("UP_TO_NEXT"));
    t.set(rv, "hasStartBound", b(true));
    t.set(rv, "startBound", en("BLIND"));
    t.set(rv, "angleBack", e("45 deg"));
    let res = t.ok();
    let rbody = res.context.bodies.iter().find(|b| b.id.feature == rv).unwrap();
    let v = rbody.shape.volume().unwrap();
    // 0 .. ~(180 - asin(1/r)) degrees plus 45 back: between 3/4 and 1 of a half turn + 45.
    let full = PI * (0.02f64.powi(2) - 0.01f64.powi(2)) * 0.01;
    assert!(v > full * (170.0 + 45.0) / 360.0 && v < full * (180.0 + 45.0) / 360.0, "{v} vs {full}");
}

// ------------------------------------------------------------------ query variables

#[test]
fn query_variables_feed_query_parameters() {
    let mut t = T::new();
    let (_, x) = t.block(0.0, 0.0, 0.02, 0.02, "10 mm");
    let res = t.ok();
    let edge = edge_near(&res.context.bodies[0], DVec3::new(0.02, 0.01, 0.01));
    t.add("queryVariable", &[("name", ParamValue::Text("rim".into())), ("selectionQuery", q(vec![edge]))]);
    t.add("fillet", &[("entities", q(vec![Reference::QueryVariable("rim".into())])), ("radius", e("2 mm"))]);
    let res = t.ok();
    assert_eq!(res.context.bodies[0].faces.len(), 7);
    assert_eq!(res.context.query_variables.get("rim").map(|v| v.len()), Some(1));
    // Created by: every edge of the extrude, chamfered through the variable.
    let mut t = T::new();
    let (_, x2) = t.block(0.0, 0.0, 0.02, 0.02, "10 mm");
    t.add("queryVariable", &[("name", ParamValue::Text("#all".into())), ("selectionType", en("CREATED_BY")), ("createdByFeatures", q(vec![Reference::Feature(x2)])), ("entityType", en("EDGE"))]);
    t.add("chamfer", &[("entities", q(vec![Reference::QueryVariable("all".into())])), ("width", e("1 mm"))]);
    let res = t.ok();
    assert_eq!(res.context.query_variables.get("all").map(|v| v.len()), Some(12));
    assert_eq!(res.context.bodies[0].faces.len(), 26);
    let _ = x;
    // Unknown variables read as an empty selection.
    let mut t = T::new();
    t.block(0.0, 0.0, 0.02, 0.02, "10 mm");
    t.add("fillet", &[("entities", q(vec![Reference::QueryVariable("missing".into())])), ("radius", e("2 mm"))]);
    let res = t.regen();
    assert_eq!(res.display.features[2].status, FeatureStatus::Error);
}

// ------------------------------------------------------------------ hole table edit logic

#[test]
fn hole_dialog_table_values_then_edits() {
    let mut t = T::new();
    t.block(0.0, 0.0, 0.1, 0.05, "25 mm");
    let pl = t.top_plane("25 mm");
    let mut sk = noshape_sketch::Sketch::new();
    let p = sk.add_point(DVec2::new(0.03, 0.02));
    let s = t.sketch(Reference::FeaturePlane(pl), sk);
    let h = t.add("hole", &[("locations", q(vec![Reference::SketchEntity { sketch: s, entity: p.0 }])), ("unitsSystem", en("METRIC")), ("endStyleV2", en("THROUGH"))]);
    // The dialog picks a Standard: table values are copied into the parameters.
    {
        let f = t.ps.feature_mut(h).unwrap();
        f.params.set("isoHoleTableEx", ParamValue::Lookup(vec!["Clearance".into(), "M6".into()]));
        assert!(noshape_model::features::hole_tables::apply_table_to_params(&mut f.params));
    }
    let d = t.ps.feature_mut(h).unwrap().params.expr("holeDiameterV3").unwrap().text.clone();
    let res = t.ok();
    let v0 = 0.1 * 0.05 * 0.025;
    let dm: f64 = d.trim_end_matches(" mm").parse::<f64>().unwrap() * 1e-3;
    assert!(near(v0 - total_volume(&res.context), PI * (dm / 2.0).powi(2) * 0.025, 1e-3));
    // A user edit after the table change wins.
    t.set(h, "holeDiameterV3", e("8 mm"));
    let res = t.ok();
    assert!(near(v0 - total_volume(&res.context), PI * 0.004f64.powi(2) * 0.025, 1e-3));
}

// ------------------------------------------------------------------ edit curve

#[test]
fn edit_curve_moves_control_points() {
    let mut t = T::new();
    let mut sk = noshape_sketch::Sketch::new();
    let pts: Vec<_> = [(0.0, 0.0), (0.01, 0.01), (0.02, -0.005), (0.03, 0.0)].iter().map(|(x, y)| sk.add_point(DVec2::new(*x, *y))).collect();
    let s = t.sketch(Reference::Plane(DatumPlane::Top), sk);
    let fs = t.add("fitSpline", &[("vertices", q(pts.iter().map(|p| Reference::SketchEntity { sketch: s, entity: p.0 }).collect()))]);
    let res = t.ok();
    let wire = res.context.bodies.iter().find(|b| b.kind == BodyKind::Wire).unwrap().clone();
    let before = wire.shape.edges()[0].edge_bspline().unwrap();
    let mut item = ParamValues::default();
    item.set("index", e("1"));
    item.set("z", e("5 mm"));
    item.set("weight", e("2"));
    t.add("editCurve", &[("wire", q(vec![Reference::Body(wire.id.clone())])), ("editControlPoints", b(true)), ("controlPointEdits", ParamValue::Array(vec![item]))]);
    let res = t.ok();
    let edited = res.context.bodies.iter().find(|b| b.kind == BodyKind::Wire).unwrap();
    assert_eq!(edited.id, wire.id, "edited in place");
    let after = edited.shape.edges()[0].edge_bspline().unwrap();
    assert_eq!(after.poles.len(), before.poles.len());
    assert!((after.poles[1] - (before.poles[1] + DVec3::new(0.0, 0.0, 0.005))).length() < 1e-9, "{:?} vs {:?}", after.poles[1], before.poles[1]);
    assert!((after.weights.as_ref().unwrap()[1] - 2.0).abs() < 1e-9);
    assert!((after.poles[0] - before.poles[0]).length() < 1e-12);
    let _ = (fs, BodyId { feature: fs, index: 0 });
}

// ------------------------------------------------------------------ hole records (drawing callouts)

#[test]
fn hole_feature_records_holes_for_callouts() {
    let mut t = T::new();
    t.block(0.0, 0.0, 0.1, 0.05, "25 mm");
    let pl = t.top_plane("25 mm");
    let mut sk = noshape_sketch::Sketch::new();
    let p1 = sk.add_point(DVec2::new(0.03, 0.02));
    let p2 = sk.add_point(DVec2::new(0.07, 0.02));
    let s = t.sketch(Reference::FeaturePlane(pl), sk);
    let locs = q(vec![Reference::SketchEntity { sketch: s, entity: p1.0 }, Reference::SketchEntity { sketch: s, entity: p2.0 }]);
    t.add("hole", &[("locations", locs), ("unitsSystem", en("METRIC")), ("styleV2", en("C_BORE")), ("endStyleV2", en("BLIND")), ("holeDiameterV3", e("6 mm")), ("holeDepthV3", e("12 mm")), ("cBoreDiameterV3", e("11 mm")), ("cBoreDepthV3", e("4 mm"))]);
    let res = t.ok();
    let holes = &res.context.holes;
    assert_eq!(holes.len(), 2);
    let h = &holes[0];
    assert!((h.diameter - 0.006).abs() < 1e-9 && h.cbore.is_some_and(|(d, dd)| (d - 0.011).abs() < 1e-9 && (dd - 0.004).abs() < 1e-9));
    assert!(!h.through && h.depth.is_some_and(|d| (d - 0.012).abs() < 1e-9));
    // The bore circle on the top face belongs to the hole; another radius does not.
    let top = h.start;
    assert!(h.matches_circle(top + h.dir * 0.004, 0.003, 1e-6));
    assert!(h.matches_circle(top, 0.0055, 1e-6));
    assert!(!h.matches_circle(top, 0.004, 1e-6));
}

#[test]
fn cancelled_regeneration_is_discarded_and_superseded_one_is_reused() {
    use noshape_model::regen::RegenCancel;
    let mut t = T::new();
    let (_, cut) = pocket_setup(&mut t);
    feature_pattern(&mut t, vec![cut], "4", true);
    let run = |t: &mut T, cancel: Option<RegenCancel>| {
        let opts = RegenOptions { cancel, ..Default::default() };
        t.reg.regenerate(&t.ps, &ConfigValues::default(), &t.units, &opts)
    };
    // Aborted: nothing comes back.
    let abort = RegenCancel::default();
    abort.abort();
    let r = run(&mut t, Some(abort));
    assert!(r.cancelled && r.display.features.is_empty() && r.context.bodies.is_empty());
    // Superseded: the features run (and are cached) but no display is built.
    let stale = RegenCancel::default();
    stale.supersede();
    let r = run(&mut t, Some(stale));
    assert!(r.cancelled && r.display.features.is_empty());
    // The next run gives the same model as an uninterrupted one.
    let r = t.ok();
    assert!(!r.cancelled);
    let removed = BLOCK_V - total_volume(&r.context);
    assert!(near(removed, 4.0 * PI * 0.003 * 0.003 * 0.005, 1e-3), "{removed}");
}

#[test]
fn aborted_pattern_regeneration_stops_quickly() {
    use noshape_model::regen::RegenCancel;
    use std::time::{Duration, Instant};
    // A 1000-instance Feature pattern (Reapply features, which patterns
    // without error) of a rib through a block, the user's report: it takes
    // seconds, and an abort stops it (inside the kernel boolean), so a newer
    // edit doesn't wait behind it.
    let mut t = T::new();
    t.block(0.0, 0.0, 2.01, 0.02, "10 mm");
    let rs = t.sketch(Reference::Plane(DatumPlane::Top), rect(0.002, 0.005, 0.003, 0.015));
    let r = t.regions(rs)[0].clone();
    let rib = t.extrude(r, "15 mm", "ADD");
    t.ok();
    let pattern = feature_pattern(&mut t, vec![rib], "1000", true);
    t.set(pattern, "distance", e("2 mm"));
    t.set(pattern, "operationType", en("ADD"));
    t.set(pattern, "defaultScope", ParamValue::Bool(true));
    let cancel = RegenCancel::default();
    let (latency, result) = std::thread::scope(|s| {
        let c = cancel.clone();
        let t = &mut t;
        let run = s.spawn(move || {
            let opts = RegenOptions { cancel: Some(c), ..Default::default() };
            let r = t.reg.regenerate(&t.ps, &ConfigValues::default(), &t.units, &opts);
            (Instant::now(), r)
        });
        std::thread::sleep(Duration::from_millis(1500));
        let at = Instant::now();
        cancel.abort();
        let (end, r) = run.join().unwrap();
        (end.saturating_duration_since(at), r)
    });
    // The whole pattern, uncancelled (upstream features come from the cache).
    let t0 = Instant::now();
    let r = t.ok();
    let full = t0.elapsed();
    assert!(!r.cancelled && r.context.bodies.iter().filter(|b| b.kind == BodyKind::Solid).count() == 1);
    eprintln!("stopped {latency:?} after the abort; the whole pattern takes {full:?}");
    if full < Duration::from_millis(2500) {
        // Too fast to abort part-way on this machine: nothing to measure.
        return;
    }
    assert!(result.cancelled, "the run finished before the abort: {:?}", result.display.features.iter().map(|f| (&f.status, &f.message)).collect::<Vec<_>>());
    // Relative to the work (machine load scales both): an abort 1.5 s in
    // stops within a fifth of the whole run.
    assert!(latency * 5 < full, "stopped {latency:?} after the abort; the whole pattern takes {full:?}");
}

/// The live "Pending state" setup as a feature clipboard payload (paste
/// with Ctrl+V; then set the pattern's count to 1000 in its dialog): a
/// 520 × 60 × 10 mm block, a 0.2 × 40 mm rib extruded 15 mm (Add) and a
/// Feature-type Linear pattern of it (Reapply features), 0.5 mm apart.
/// `cargo test -p noshape-model --test engine write_pending_pattern_clip -- --ignored`.
#[test]
#[ignore]
fn write_pending_pattern_clip() {
    let mut t = T::new();
    t.block(0.0, 0.0, 0.52, 0.06, "10 mm");
    let rs = t.sketch(Reference::Plane(DatumPlane::Top), rect(0.005, 0.01, 0.0052, 0.05));
    let r = t.regions(rs)[0].clone();
    let rib = t.extrude(r, "15 mm", "ADD");
    // Reapply features: the rib passes through the block, whose faces it
    // merges with (a plain feature pattern would fail as in Onshape, G45).
    let pattern = feature_pattern(&mut t, vec![rib], "10", true);
    t.set(pattern, "distance", e("0.5 mm"));
    t.set(pattern, "operationType", en("ADD"));
    t.set(pattern, "defaultScope", ParamValue::Bool(true));
    t.ok();
    let v = serde_json::json!({ "noshape_features": t.ps.features });
    std::fs::write(std::env::temp_dir().join("noshape-pending-clip.json"), v.to_string()).unwrap();
}

/// qa4 core workflow (data loss): a sketch dimension typed as a bare
/// number keeps its size when the workspace units change (live stores the
/// unit on commit; older documents get it pinned on the change).
#[test]
fn workspace_unit_change_keeps_unitless_sketch_dimensions() {
    use noshape_model::document::{Document, TabContent};
    let mut t = T::new();
    let mut sk = rect(0.0, 0.0, 0.02, 0.02);
    let bottom = sk.entities().find(|e| e.kind() == noshape_sketch::EntityKind::Line).unwrap().id;
    let cid = sk.add_reference(noshape_sketch::ConstraintKind::Length { curve: bottom, value: 0.02 }).unwrap();
    sk.set_driving(cid, true).unwrap();
    let mut data = SketchData { sketch: sk, ..Default::default() };
    data.dimension_exprs.insert(cid.0, ExprValue::new("25"));
    let s = t.add("sketch", &[("sketchPlane", q(vec![Reference::Plane(DatumPlane::Top)])), ("sketch", ParamValue::Sketch(Box::new(data)))]);
    let r = t.regions(s)[0].clone();
    t.extrude(r, "10 mm", "NEW");
    let mm = total_volume(&t.ok().context);
    // Without the pin, inch units re-solve the sketch 25.4 times larger.
    let mut unpinned = T { ps: t.ps.clone(), units: UnitPrefs { length: noshape_units::LengthUnit::Inch, ..UnitPrefs::default() }, reg: Regenerator::new() };
    assert!(!near(total_volume(&unpinned.ok().context), mm, 1e-3), "the test needs the unit to matter");
    let mut doc = Document::new();
    let tab = doc.tabs.iter_mut().find(|x| matches!(x.content, TabContent::PartStudio(_))).unwrap();
    tab.content = TabContent::PartStudio(t.ps.clone());
    let changed = noshape_model::quantity_text::pin_sketch_units(&mut doc, &UnitPrefs::default());
    assert_eq!(changed.len(), 1);
    let TabContent::PartStudio(ps) = &doc.tabs.iter().find(|x| matches!(x.content, TabContent::PartStudio(_))).unwrap().content else { unreachable!() };
    let Some(ParamValue::Sketch(d)) = ps.features[0].params.get("sketch") else { panic!() };
    assert_eq!(d.dimension_exprs[&cid.0].text, "25 mm");
    t.ps = ps.clone();
    t.units = UnitPrefs { length: noshape_units::LengthUnit::Inch, ..UnitPrefs::default() };
    let inch = total_volume(&t.ok().context);
    assert!(near(inch, mm, 1e-9), "the part keeps its size: {inch} vs {mm}");
    // Unitless expressions are wrapped (live "(10+5) mm"); units and variables stay.
    use noshape_model::quantity_text::commit_text;
    let e = noshape_units::EmptyScope;
    assert_eq!(commit_text("10+5", noshape_units::Dim::LENGTH, &UnitPrefs::default(), &e), "(10+5) mm");
    assert_eq!(commit_text("1 in", noshape_units::Dim::LENGTH, &UnitPrefs::default(), &e), "1 in");
    assert_eq!(commit_text("#var", noshape_units::Dim::LENGTH, &UnitPrefs::default(), &e), "#var");
}

// ------------------------------------------------------------------ qa4 F12

/// qa4 F12: a Feature linear pattern of a Hole (two holes on the part)
/// copies the holes; editing the block's depth keeps the part. (qa4's third
/// location was the projected origin; live psb 10-21 f: the origin is never
/// a location, fixed as F10.)
#[test]
fn feature_pattern_of_hole_copies_the_holes() {
    let mut t = T::new();
    let (_, x1) = t.block(0.01, 0.01, 0.06, 0.05, "30 mm");
    let pl = t.top_plane("30 mm");
    let mut sk = noshape_sketch::Sketch::new();
    let pts = [sk.add_point(DVec2::new(0.025, 0.025)), sk.add_point(DVec2::new(0.025, 0.035))];
    let s = t.sketch(Reference::FeaturePlane(pl), sk);
    let locs = q(pts.iter().map(|p| Reference::SketchEntity { sketch: s, entity: p.0 }).collect());
    let h = t.add("hole", &[("locations", locs), ("unitsSystem", en("METRIC"))]);
    let one = |t: &mut T, h: f64| {
        let r = t.regen();
        let v = total_volume(&r.context);
        (0.05 * 0.04 * h - v) / 2.0
    };
    let hole = one(&mut t, 0.03);
    assert!(hole > 0.0);
    t.add(
        "linearPattern",
        &[
            ("patternType", en("FEATURE")),
            ("instanceFunction", q(vec![Reference::Feature(h)])),
            ("directionOne", q(vec![Reference::Plane(DatumPlane::Right)])),
            ("distance", e("10 mm")),
            ("instanceCount", e("2")),
        ],
    );
    for depth in [30.0, 25.0] {
        t.set(x1, "depth", e(&format!("{depth} mm")));
        t.set(pl, "offset", e(&format!("{depth} mm")));
        let r = t.ok();
        let solids: Vec<_> = r.context.bodies.iter().filter(|b| b.kind == noshape_model::display::BodyKind::Solid).collect();
        assert_eq!(solids.len(), 1, "depth {depth}");
        let removed = 0.05 * 0.04 * depth * 1e-3 - total_volume(&r.context);
        assert!(near(removed, 4.0 * hole, 1e-6), "depth {depth}: removed {removed}, 4 holes {}", 4.0 * hole);
    }
}

/// qa4 F12: a Feature-type linear pattern of a Hole cuts holes into the
/// part (it doesn't turn it into a shell with cylinders), and keeps doing so
/// after the block's depth changes.
#[test]
fn feature_pattern_of_a_hole_keeps_the_part() {
    hole_pattern_case();
    // qa4's case: the projected origin also counted as a location (F10,
    // fixed: it doesn't any more), a hole on the block's corner whose faces
    // merge with the block's sides. Never a silently wrecked part: the
    // pattern reports Onshape's error (G45) and the part stays as it was.
    let mut t = T::new();
    t.block(0.0, 0.0, 0.1, 0.05, "30 mm");
    let pl = t.top_plane("30 mm");
    let mut sk = noshape_sketch::Sketch::new();
    let p1 = sk.add_point(DVec2::new(0.02, 0.025));
    let p0 = sk.add_point(DVec2::new(0.0, 0.0));
    let s = t.sketch(Reference::FeaturePlane(pl), sk);
    let locs = q(vec![Reference::SketchEntity { sketch: s, entity: p1.0 }, Reference::SketchEntity { sketch: s, entity: p0.0 }]);
    let hole = t.add("hole", &[("locations", locs), ("unitsSystem", en("METRIC")), ("styleV2", en("SIMPLE")), ("endStyleV2", en("BLIND")), ("holeDiameterV3", e("6 mm")), ("holeDepthV3", e("12 mm"))]);
    let before = total_volume(&t.ok().context);
    let pattern = feature_pattern(&mut t, vec![hole], "2", false);
    t.set(pattern, "distance", e("10 mm"));
    let r = t.regen();
    let st = r.display.features.last().unwrap();
    assert!(matches!(st.status, FeatureStatus::Error), "the pattern reports its failure: {:?}", st.message);
    assert!(near(total_volume(&r.context), before, 1e-9), "and the part is unchanged");
}

/// Live "qa4 round 5 v2 s1 follow-up" (2): a Ø6 through Hole at (3, 10) on
/// the block's top, tangent to the x = 0 face, fails with hole.fs's
/// HOLE_CUT_FAIL text and cuts nothing. A Reapply pattern of it along −y
/// (4 mm × 4) is OK and silently skips the tangent copies (y 6, 2): only
/// the one at y −2 notches the front face (a 3.0975 mm² segment × 15 mm).
#[test]
fn tangent_hole_fails_and_its_reapplied_pattern_cuts_the_good_copies() {
    let mut t = T::new();
    t.block(0.0, 0.0, 0.08, 0.05, "15 mm");
    let pl = t.top_plane("15 mm");
    let mut sk = noshape_sketch::Sketch::new();
    let p = sk.add_point(DVec2::new(0.003, 0.01));
    let s = t.sketch(Reference::FeaturePlane(pl), sk);
    let locs = q(vec![Reference::SketchEntity { sketch: s, entity: p.0 }]);
    let hole = t.add("hole", &[("locations", locs), ("unitsSystem", en("METRIC")), ("styleV2", en("SIMPLE")), ("endStyleV2", en("THROUGH")), ("holeDiameterV3", e("6 mm"))]);
    let block = 0.08 * 0.05 * 0.015;
    let r = t.regen();
    let st = r.display.features.last().unwrap();
    assert!(matches!(st.status, FeatureStatus::Error), "{:?}", st.status);
    assert_eq!(st.message.as_deref(), Some("Hole creates invalid geometry."));
    assert!(near(total_volume(&r.context), block, 1e-9));
    t.add(
        "linearPattern",
        &[
            ("patternType", en("FEATURE")),
            ("instanceFunction", q(vec![Reference::Feature(hole)])),
            // The Front plane's normal is −y.
            ("directionOne", q(vec![Reference::Plane(DatumPlane::Front)])),
            ("distance", e("4 mm")),
            ("instanceCount", e("4")),
            ("fullFeaturePattern", b(true)),
        ],
    );
    let r = t.regen();
    let st = r.display.features.last().unwrap();
    assert!(matches!(st.status, FeatureStatus::Ok), "{:?}: {:?}", st.status, st.message);
    let notch = (9.0 * (2.0f64 / 3.0).acos() - 2.0 * 5f64.sqrt()) * 15.0 * 1e-9;
    let removed = block - total_volume(&r.context);
    assert!(near(removed, notch, 1e-6), "removed {} mm³, live 46.462", removed * 1e9);
}

fn hole_pattern_case() {
    let mut t = T::new();
    let (_, block) = t.block(0.0, 0.0, 0.1, 0.05, "30 mm");
    let pl = t.top_plane("30 mm");
    let mut sk = noshape_sketch::Sketch::new();
    let p1 = sk.add_point(DVec2::new(0.02, 0.025));
    let s = t.sketch(Reference::FeaturePlane(pl), sk);
    let locs = q(vec![Reference::SketchEntity { sketch: s, entity: p1.0 }]);
    let hole = t.add("hole", &[("locations", locs), ("unitsSystem", en("METRIC")), ("styleV2", en("SIMPLE")), ("endStyleV2", en("BLIND")), ("holeDiameterV3", e("6 mm")), ("holeDepthV3", e("12 mm"))]);
    let one = BLOCK_V3 - total_volume(&t.ok().context);
    let pattern = feature_pattern(&mut t, vec![hole], "2", false);
    t.set(pattern, "distance", e("10 mm"));
    let check = |t: &mut T, depth_mm: f64| {
        let r = t.ok();
        let solids: Vec<_> = r.context.bodies.iter().filter(|b| b.kind == BodyKind::Solid).collect();
        assert_eq!(solids.len(), 1, "one part");
        let v = total_volume(&r.context);
        let block = 0.1 * 0.05 * depth_mm * 1e-3;
        assert!(near(block - v, 2.0 * one, 1e-3), "two holes cut: removed {} vs {}", block - v, 2.0 * one);
    };
    check(&mut t, 30.0);
    t.set(block, "depth", e("25 mm"));
    t.set(pl, "offset", e("25 mm"));
    check(&mut t, 25.0);
}

const BLOCK_V3: f64 = 0.1 * 0.05 * 0.03;

/// The live "Pending state" model at ×1000 (Reapply features): every rib
/// fuses into the block, one part (the app showed 911 parts, 10-21).
#[test]
#[ignore]
fn pending_pattern_clip_at_1000_is_one_part() {
    let mut t = T::new();
    t.block(0.0, 0.0, 0.52, 0.06, "10 mm");
    let rs = t.sketch(Reference::Plane(DatumPlane::Top), rect(0.005, 0.01, 0.0052, 0.05));
    let r = t.regions(rs)[0].clone();
    let rib = t.extrude(r, "15 mm", "ADD");
    let pattern = feature_pattern(&mut t, vec![rib], "1000", true);
    t.set(pattern, "distance", e("0.5 mm"));
    t.set(pattern, "operationType", en("ADD"));
    t.set(pattern, "defaultScope", ParamValue::Bool(true));
    let r = t.ok();
    let solids = r.context.bodies.iter().filter(|b| b.kind == BodyKind::Solid).count();
    assert_eq!(solids, 1, "every rib fuses into the block");
}

/// Live "Pattern face merging" ("fins 1000") as a feature clipboard payload:
/// a 100 × 2000 × 40 mm block, a 1 mm fin flush with its front face, 40 mm
/// high on top, and a Feature linear pattern of it (Reapply features, 2 mm,
/// 2 instances; type 1000 in the dialog as live did).
/// `cargo test -p noshape-model --test engine write_fins_clip -- --ignored`.
#[test]
#[ignore]
fn write_fins_clip() {
    let mut t = T::new();
    t.block(0.0, 0.0, 0.1, 2.0, "40 mm");
    let fs = t.sketch(Reference::Plane(DatumPlane::Top), rect(0.0, 0.0, 0.1, 0.001));
    let r = t.regions(fs)[0].clone();
    let fin = t.extrude(r, "40 mm", "ADD");
    t.set(fin, "startOffset", b(true));
    t.set(fin, "startOffsetBound", en("BLIND"));
    t.set(fin, "startOffsetDistance", e("40 mm"));
    t.add(
        "linearPattern",
        &[
            ("patternType", en("FEATURE")),
            ("instanceFunction", q(vec![Reference::Feature(fin)])),
            ("directionOne", q(vec![Reference::Plane(DatumPlane::Front)])),
            ("oppositeDirection", b(true)),
            ("distance", e("2 mm")),
            ("instanceCount", e("2")),
            ("operationType", en("ADD")), ("defaultScope", ParamValue::Bool(true)),
            ("fullFeaturePattern", b(true)),
        ],
    );
    t.ok();
    let v = serde_json::json!({ "noshape_features": t.ps.features });
    std::fs::write(std::env::temp_dir().join("noshape-fins-clip.json"), v.to_string()).unwrap();
}

/// qa5 ADV10: a Feature linear pattern with a second direction makes the
/// full grid (3 × 3 holes), not only the first row.
#[test]
fn feature_pattern_second_direction_makes_the_grid() {
    let mut t = T::new();
    t.block(0.0, 0.0, 0.1, 0.1, "20 mm");
    let pl = t.top_plane("20 mm");
    let mut sk = noshape_sketch::Sketch::new();
    let p1 = sk.add_point(DVec2::new(0.02, 0.02));
    let s = t.sketch(Reference::FeaturePlane(pl), sk);
    let h = t.add("hole", &[("locations", q(vec![Reference::SketchEntity { sketch: s, entity: p1.0 }])), ("unitsSystem", en("METRIC")), ("styleV2", en("SIMPLE")), ("endStyleV2", en("BLIND")), ("holeDiameterV3", e("6 mm")), ("holeDepthV3", e("10 mm"))]);
    let one = 0.1 * 0.1 * 0.02 - total_volume(&t.ok().context);
    t.add(
        "linearPattern",
        &[
            ("patternType", en("FEATURE")),
            ("instanceFunction", q(vec![Reference::Feature(h)])),
            ("directionOne", q(vec![Reference::Plane(DatumPlane::Right)])),
            ("distance", e("20 mm")),
            ("instanceCount", e("3")),
            ("hasSecondDir", b(true)),
            ("directionTwo", q(vec![Reference::Plane(DatumPlane::Front)])),
            ("oppositeDirectionTwo", b(true)),
            ("distanceTwo", e("20 mm")),
            ("instanceCountTwo", e("3")),
        ],
    );
    let removed = 0.1 * 0.1 * 0.02 - total_volume(&t.ok().context);
    assert!(near(removed, 9.0 * one, 1e-6), "removed {removed}, 9 holes {}", 9.0 * one);
}

/// qa4 F12, the path after a Part pattern (v0.4.5 release blocker): block,
/// two top-edge fillets, a chamfer on the vertical edge between them, a Hole
/// at two sketch points on the top face, a Part pattern of Part 1 (40 mm × 3)
/// and then a Feature pattern of the Hole along Part 1's bottom edge
/// (10 mm × 2). Live copies the holes on Part 1 only, and an Extrude 1 edit
/// keeps every part. Ours split off a sliver part, then lost Part 1 silently.
#[test]
fn feature_pattern_of_a_hole_after_a_part_pattern_keeps_the_parts() {
    for (w, along_x) in [(0.1, false), (0.1, true), (0.03, true), (0.03, false)] {
        f12_case(w, along_x);
    }
}

fn f12_case(w: f64, along_x: bool) {
    let d = 0.05;
    let mut t = T::new();
    let (_, block) = t.block(0.0, 0.0, w, d, "30 mm");
    let r = t.ok();
    let part = r.context.bodies[0].clone();
    t.add("fillet", &[("entities", q(vec![f12_edge(&part, DVec3::new(w / 2.0, 0.0, 0.03)), f12_edge(&part, DVec3::new(w, d / 2.0, 0.03))])), ("radius", e("3 mm"))]);
    let r = t.ok();
    let part = r.context.bodies[0].clone();
    t.add("chamfer", &[("entities", q(vec![f12_edge(&part, DVec3::new(w, 0.0, 0.015))])), ("width", e("2 mm"))]);
    let r = t.ok();
    let part = r.context.bodies[0].clone();
    let top = part
        .shape
        .faces()
        .iter()
        .enumerate()
        .find(|(_, f)| (f.centroid().unwrap().z - 0.03).abs() < 1e-6)
        .map(|(i, _)| Reference::Face((*part.faces[i]).clone()))
        .expect("top face");
    let mut sk = noshape_sketch::Sketch::new();
    let p1 = sk.add_point(DVec2::new(w * 0.25, d * 0.5));
    let p2 = sk.add_point(DVec2::new(w * 0.5, d * 0.3));
    let s = t.sketch(top, sk);
    let locs = q(vec![Reference::SketchEntity { sketch: s, entity: p1.0 }, Reference::SketchEntity { sketch: s, entity: p2.0 }]);
    let pre = total_volume(&r.context);
    let hole = t.add("hole", &[("locations", locs), ("scope", q(vec![Reference::Body(part.id.clone())]))]);
    let r = t.ok();
    let part = r.context.bodies[0].clone();
    let holes = pre - total_volume(&r.context);
    assert!(holes > 0.0, "the Hole cuts");
    let dir = if along_x { DVec3::new(w / 2.0, 0.0, 0.0) } else { DVec3::new(w, d / 2.0, 0.0) };
    t.add(
        "linearPattern",
        &[("patternType", en("PART")), ("entities", q(vec![Reference::Body(part.id.clone())])), ("directionOne", q(vec![f12_edge(&part, DVec3::new(w / 2.0, 0.0, 0.0))])), ("distance", e("40 mm")), ("instanceCount", e("3"))],
    );
    let r = t.ok();
    assert_eq!(r.context.bodies.iter().filter(|b| b.kind == BodyKind::Solid).count(), 3, "Part pattern: three parts");
    let part = r.context.bodies.iter().find(|b| b.id == part.id).expect("Part 1").clone();
    let before = total_volume(&r.context);
    let de = f12_edge(&part, dir);
    t.add(
        "linearPattern",
        &[("patternType", en("FEATURE")), ("instanceFunction", q(vec![Reference::Feature(hole)])), ("directionOne", q(vec![de])), ("distance", e("10 mm")), ("instanceCount", e("2"))],
    );
    let check = |t: &mut T, before: f64, what: &str| {
        let r = t.regen();
        let solids: Vec<_> = r.context.bodies.iter().filter(|b| b.kind == BodyKind::Solid).collect();
        let names: Vec<_> = solids.iter().map(|b| (b.name.clone(), b.shape.volume().unwrap_or(0.0))).collect();
        // Live cuts every copy, also the one running into the fillet (OCCT
        // used to return an empty cut there, on the corrupt corner OCCT's own
        // chamfer left; the chamfer is Parasolid's blend patch now).
        for (i, s) in r.display.features.iter().enumerate() {
            assert!(matches!(s.status, FeatureStatus::Ok), "{what} (w {w}, x {along_x}): feature {i} ({}) {:?}: {:?}", t.ps.features[i].type_id, s.status, s.message);
        }
        assert_eq!(solids.len(), 3, "{what} (w {w}, x {along_x}): still three parts: {names:?}");
        assert!(solids.iter().any(|b| b.id == part.id), "{what}: Part 1 kept: {names:?}");
        let removed = before - total_volume(&r.context);
        assert!(removed > 0.5 * holes && removed < 1.05 * holes, "{what} (w {w}, x {along_x}): the copies cut two more holes from Part 1: {removed} vs {holes}; {names:?}");
    };
    check(&mut t, before, "Feature pattern");
    t.set(block, "depth", e("25 mm"));
    // Volumes after the edit: the same pattern on a 25 mm block.
    let before25 = before - w * d * 0.005 * 3.0;
    check(&mut t, before25, "Extrude 1 → 25 mm");
}

/// Like `edge_near`, skipping degenerate edges (the hole's cone tip).
fn f12_edge(b: &Body, p: DVec3) -> Reference {
    let v = noshape_kernel::make_vertex(p).unwrap();
    let (i, _) = b
        .shape
        .edges()
        .iter()
        .enumerate()
        .filter_map(|(i, e)| {
            let d = noshape_kernel::distance(&v, e).ok()?.distance;
            let info = e.edge_info().ok()?;
            let m = e.point_at((info.first_param + info.last_param) * 0.5).ok()?;
            Some((i, d * 1e3 + m.distance(p)))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .unwrap();
    Reference::Edge((*b.edges[i]).clone())
}

/// qa5's "qa5 bool" studio: Part 1 block (0,0)–(40,20)×10 mm, Part 2
/// cylinder Ø10 centred (40,10) ×20 (New, overlapping Part 1's x=40 face),
/// Part 3 block (60,0)–(80,20)×10.
fn qa5_bool() -> (T, [BodyId; 3]) {
    let mut t = T::new();
    t.block(0.0, 0.0, 0.04, 0.02, "10 mm");
    let cs = t.sketch(Reference::Plane(DatumPlane::Top), circle(DVec2::new(0.04, 0.01), 0.005));
    let r = t.regions(cs)[0].clone();
    t.extrude(r, "20 mm", "NEW");
    t.block(0.06, 0.0, 0.08, 0.02, "10 mm");
    let r = t.ok();
    let ids: Vec<BodyId> = r.context.bodies.iter().map(|b| b.id.clone()).collect();
    assert_eq!(ids.len(), 3);
    (t, [ids[0].clone(), ids[1].clone(), ids[2].clone()])
}

fn parts(r: &RegenResult) -> Vec<(BodyId, String, f64)> {
    r.context.bodies.iter().filter(|b| b.kind == BodyKind::Solid).map(|b| (b.id.clone(), b.name.clone(), b.shape.volume().unwrap() * 1e9)).collect()
}

/// qa5 BOOL1: a Boolean union of two parts apart (Part 1 + Part 3) leaves
/// both parts as they were (live) and never touches Part 2.
#[test]
fn union_of_parts_apart_keeps_every_part() {
    for keep in [false, true] {
        let (mut t, [p1, p2, p3]) = qa5_bool();
        t.add("booleanBodies", &[("operationType", en("UNION")), ("tools", q(vec![Reference::Body(p1.clone()), Reference::Body(p3.clone())])), ("keepTools", b(keep))]);
        let r = t.regen();
        let st = r.display.features.last().unwrap();
        assert!(matches!(st.status, FeatureStatus::Info), "keep {keep}: the union succeeds with a note: {:?}", st.message);
        assert_eq!(st.message.as_deref(), Some("Boolean resulted in no geometry change. The parts either do not intersect or are totally contained."));
        let ps = parts(&r);
        assert_eq!(ps.len(), 3, "keep {keep}: {ps:?}");
        let vol = |id: &BodyId| ps.iter().find(|p| p.0 == *id).map(|p| p.2).unwrap_or_else(|| panic!("keep {keep}: {id:?} gone: {ps:?}"));
        assert!((vol(&p1) - 8000.0).abs() < 1e-6, "{ps:?}");
        assert!((vol(&p2) - 25.0 * PI * 20.0).abs() < 1e-6, "Part 2 untouched: {ps:?}");
        assert!((vol(&p3) - 4000.0).abs() < 1e-6, "{ps:?}");
        assert_eq!(ps.iter().map(|p| p.1.as_str()).collect::<Vec<_>>(), ["Part 1", "Part 2", "Part 3"]);
    }
}

/// qa5 BOOL3 (live "qa5 booleans"): an Extrude Add whose tool touches Part
/// 1, the pin and Part 3: Merge with all makes one part of the tool and
/// every part ("Part 1", 14785.398163 mm³); the scope Part 1 + Part 3 makes
/// those two and the tool one part (14000) and the pin stays; an empty
/// scope fails ("No merge scope selected.").
#[test]
fn add_into_several_parts_is_one_union() {
    for case in ["empty", "all", "p1p3"] {
        let (mut t, [p1, p2, p3]) = qa5_bool();
        let s = t.sketch(Reference::Plane(DatumPlane::Top), rect(0.03, 0.005, 0.07, 0.015));
        let reg = t.regions(s)[0].clone();
        let x = t.extrude(reg, "10 mm", "ADD");
        t.set(x, "defaultScope", b(false));
        match case {
            "all" => t.set(x, "defaultScope", b(true)),
            "p1p3" => t.set(x, "booleanScope", q(vec![Reference::Body(p1.clone()), Reference::Body(p3.clone())])),
            _ => {}
        }
        if case == "empty" {
            // Live: the feature fails, "No merge scope selected.", and the
            // parts stay as they were.
            let r = t.regen();
            let st = r.display.features.last().unwrap();
            assert!(matches!(st.status, FeatureStatus::Error) && st.message.as_deref() == Some("No merge scope selected."), "{:?}", st.message);
            assert_eq!(parts(&r).len(), 3);
            continue;
        }
        let r = t.ok();
        let ps = parts(&r);
        if case == "p1p3" {
            assert_eq!(ps.len(), 2, "{case}: {ps:?}");
            let merged = ps.iter().find(|p| p.0 == p1).expect("Part 1 kept");
            assert!((merged.2 - 14000.0).abs() < 1e-5, "{case}: {ps:?}");
            let pin = ps.iter().find(|p| p.0 == p2).expect("the pin stays");
            assert!((pin.2 - 25.0 * PI * 20.0).abs() < 1e-6, "{case}: {ps:?}");
        } else {
            assert_eq!(ps.len(), 1, "{case}: one part: {ps:?}");
            assert_eq!(ps[0].0, p1, "{case}: it is Part 1");
            assert!((ps[0].2 - 14785.398163).abs() < 1e-5, "{case}: {ps:?}");
            assert!(!ps.iter().any(|p| p.0 == p2 || p.0 == p3), "{case}: merged into Part 1");
        }
    }
}

/// Live "qa5 booleans": Union Part 1 + Part 2 is one part of 9178.097 mm³
/// keeping the first pick's id, name and colour, and the old Part 3 is then
/// "Part 2"; Subtract (tools Part 2, targets Part 1) leaves Part 1 7607.301;
/// Intersect makes a new part (new id and colour) of 392.699.
#[test]
fn boolean_results_and_names_as_live() {
    let (mut t, [p1, p2, p3]) = qa5_bool();
    let color = |r: &RegenResult, id: &BodyId| r.context.bodies.iter().find(|b| b.id == *id).map(|b| b.color);
    let c1 = color(&t.ok(), &p1);
    let u = t.add("booleanBodies", &[("operationType", en("UNION")), ("tools", q(vec![Reference::Body(p1.clone()), Reference::Body(p2.clone())]))]);
    let r = t.ok();
    let ps = parts(&r);
    assert_eq!(ps.len(), 2, "{ps:?}");
    assert_eq!((ps[0].0.clone(), ps[0].1.as_str()), (p1.clone(), "Part 1"));
    assert!((ps[0].2 - 9178.097245).abs() < 1e-4, "{ps:?}");
    assert_eq!(color(&r, &p1), c1);
    assert_eq!((ps[1].0.clone(), ps[1].1.as_str()), (p3.clone(), "Part 2"), "the old Part 3 renumbers");
    t.ps.features.retain(|f| f.id != u);

    let s = t.add("booleanBodies", &[("operationType", en("SUBTRACTION")), ("tools", q(vec![Reference::Body(p2.clone())])), ("targets", q(vec![Reference::Body(p1.clone())]))]);
    let ps = parts(&t.ok());
    assert_eq!(ps.len(), 2, "{ps:?}");
    assert!((ps.iter().find(|p| p.0 == p1).unwrap().2 - 7607.300919).abs() < 1e-4, "{ps:?}");
    t.ps.features.retain(|f| f.id != s);

    t.add("booleanBodies", &[("operationType", en("INTERSECTION")), ("tools", q(vec![Reference::Body(p1.clone()), Reference::Body(p2.clone())]))]);
    let r = t.ok();
    let ps = parts(&r);
    assert_eq!(ps.len(), 2, "{ps:?}");
    let new = ps.iter().find(|p| p.0 != p3).unwrap();
    assert!(new.0 != p1 && new.0 != p2, "a new part: {ps:?}");
    assert!((new.2 - 392.699082).abs() < 1e-4, "{ps:?}");
    assert_ne!(color(&r, &new.0), c1, "with a new colour");
}

/// Live NAME2 / BOOL2 ("qa5 booleans"): a union keeps the first-picked
/// part's id and colour (the pin picked first); Intersect with ☑ Keep tools
/// keeps all three parts and adds the common volume as a new part, last.
#[test]
fn union_pick_order_and_intersect_keep_tools() {
    let (mut t, [p1, p2, p3]) = qa5_bool();
    let color = |r: &RegenResult, id: &BodyId| r.context.bodies.iter().find(|b| b.id == *id).map(|b| b.color);
    let c2 = color(&t.ok(), &p2);
    let u = t.add("booleanBodies", &[("operationType", en("UNION")), ("tools", q(vec![Reference::Body(p2.clone()), Reference::Body(p1.clone())]))]);
    let r = t.ok();
    let ps = parts(&r);
    assert_eq!(ps.len(), 2, "{ps:?}");
    assert!(ps.iter().any(|p| p.0 == p2 && (p.2 - 9178.097245).abs() < 1e-4), "the pin's id holds the union: {ps:?}");
    assert_eq!(color(&r, &p2), c2);
    t.ps.features.retain(|f| f.id != u);

    t.add("booleanBodies", &[("operationType", en("INTERSECTION")), ("tools", q(vec![Reference::Body(p1.clone()), Reference::Body(p2.clone())])), ("keepTools", b(true))]);
    let ps = parts(&t.ok());
    assert_eq!(ps.len(), 4, "{ps:?}");
    assert_eq!(ps[..3].iter().map(|p| p.0.clone()).collect::<Vec<_>>(), [p1, p2, p3], "the tools stay: {ps:?}");
    assert!((ps[3].2 - 392.699082).abs() < 1e-4 && ps[3].1 == "Part 4", "{ps:?}");
}

/// qa5 INT2: Intersect of two parts that don't touch (Part 1 and Part 3),
/// with or without ☑ Keep tools, makes nothing and changes nothing.
#[test]
fn intersect_of_parts_apart_makes_nothing() {
    for keep in [true, false] {
        let (mut t, [p1, _, p3]) = qa5_bool();
        let before = parts(&t.ok());
        t.add("booleanBodies", &[("operationType", en("INTERSECTION")), ("tools", q(vec![Reference::Body(p1.clone()), Reference::Body(p3.clone())])), ("keepTools", b(keep))]);
        let ps = parts(&t.regen());
        assert_eq!(ps.len(), 3, "keep {keep}: {ps:?}");
        assert_eq!(ps.iter().map(|p| p.0.clone()).collect::<Vec<_>>(), before.iter().map(|p| p.0.clone()).collect::<Vec<_>>(), "keep {keep}");
    }
}

/// Live "fail preview 2": Fillet 1 (2 mm on the top front edge of a
/// 40 x 30 x 20 mm block) edited to 50 mm regenerates OK in Onshape, the
/// volume going from 24000 to 7458.6158 mm³ (wider than both faces: the
/// block cut by the ball's sweep).
#[test]
fn fillet_wider_than_its_faces_regenerates_like_onshape() {
    let mut t = T::new();
    t.block(0.0, 0.0, 0.04, 0.03, "20 mm");
    let r = t.ok();
    let edge = edge_near(&r.context.bodies[0], DVec3::new(0.02, 0.0, 0.02));
    let fillet = t.add("fillet", &[("entities", q(vec![edge])), ("radius", e("2 mm"))]);
    t.ok();
    t.set(fillet, "radius", e("50 mm"));
    let r = t.ok();
    let v = total_volume(&r.context) * 1e9;
    assert!((v - 7458.6158).abs() < 1e-3, "{v}");
    assert_eq!(r.context.bodies.iter().filter(|b| b.kind == BodyKind::Solid).count(), 1);
}
