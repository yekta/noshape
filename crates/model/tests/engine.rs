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
        self.add("extrude", &[("entities", q(vec![region])), ("depth", e(depth)), ("operationType", en(op))])
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
            assert!(matches!(s.status, FeatureStatus::Ok), "feature {i} ({}) {:?}: {:?}", self.ps.features[i].type_id, s.status, s.message);
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

    // Open line: thickness 1 on the left of its direction, thickness 2 right.
    let mut t = T::new();
    let mut sk = noshape_sketch::Sketch::new();
    let l = sk.add_line(DVec2::new(0.0, 0.0), DVec2::new(0.04, 0.0));
    let s = t.sketch(Reference::Plane(DatumPlane::Top), sk);
    thin_extrude(&mut t, vec![Reference::SketchEntity { sketch: s, entity: l.0 }], &[("thickness1", e("2 mm")), ("thickness2", e("1 mm"))]);
    let res = t.ok();
    assert!(near(total_volume(&res.context), 0.04 * 0.003 * 0.01, 1e-6), "{}", total_volume(&res.context));
    let bb = res.context.bodies[0].shape.bounding_box(true).unwrap();
    assert!((bb.max.y - 0.002).abs() < 1e-7 && (bb.min.y + 0.001).abs() < 1e-7, "{bb:?}");

    // Open L polyline turning left: wall on the inside of the corner.
    let mut t = T::new();
    let mut sk = noshape_sketch::Sketch::new();
    let p: Vec<_> = [(0.0, 0.0), (0.04, 0.0), (0.04, 0.03)].iter().map(|(x, y)| sk.add_point(DVec2::new(*x, *y))).collect();
    let l1 = sk.add_line_between(p[0], p[1]).unwrap();
    let l2 = sk.add_line_between(p[1], p[2]).unwrap();
    let s = t.sketch(Reference::Plane(DatumPlane::Top), sk);
    thin_extrude(&mut t, vec![Reference::SketchEntity { sketch: s, entity: l1.0 }, Reference::SketchEntity { sketch: s, entity: l2.0 }], &[("thickness1", e("2 mm"))]);
    let res = t.ok();
    let area = 0.04 * 0.002 + 0.002 * 0.028;
    assert!(near(total_volume(&res.context), area * 0.01, 1e-6), "{}", total_volume(&res.context));
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
