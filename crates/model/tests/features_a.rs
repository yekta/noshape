//! Integration tests for the batch A feature evaluators: sweep, loft,
//! thicken, enclose, rib, hole, external thread, draft, face blend, modify
//! fillet, split, patterns, mirror, transform, copy, delete/move face.

use glam::{DVec2, DVec3};
use noshape_model::configuration::ConfigValues;
use noshape_model::display::FeatureStatus;
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
    fn ok(&mut self) -> RegenResult {
        let r = self.regen();
        for (i, s) in r.display.features.iter().enumerate() {
            // An info (a blue bubble, e.g. a Hole's "Parameters have been
            // overridden.") is still a successful regeneration.
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
    ctx.bodies.iter().filter_map(|b| b.shape.volume().ok()).sum()
}

fn solids(ctx: &Context) -> Vec<&Arc<Body>> {
    ctx.bodies.iter().filter(|b| b.kind == noshape_model::display::BodyKind::Solid).collect()
}

fn near(a: f64, b: f64, rel: f64) -> bool {
    (a - b).abs() <= rel * b.abs().max(1e-12)
}

/// Face of a body whose outward plane normal is `n` (first match).
fn face_with_normal(b: &Body, n: DVec3) -> Reference {
    for (i, f) in b.shape.faces().iter().enumerate() {
        if let Ok(Some(pl)) = f.face_plane()
            && pl.unit_normal().dot(n) > 0.999
        {
            return Reference::Face((*b.faces[i]).clone());
        }
    }
    panic!("no face with normal {n:?}");
}

/// Edge of a body nearest to a point.
fn edge_near(b: &Body, p: DVec3) -> Reference {
    let (i, _) = b
        .shape
        .edges()
        .iter()
        .enumerate()
        .map(|(i, e)| {
            let (a, c) = e.endpoints().unwrap();
            (i, ((a + c) * 0.5).distance(p))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .unwrap();
    Reference::Edge((*b.edges[i]).clone())
}

fn body_ref(id: &BodyId) -> Reference {
    Reference::Body(id.clone())
}

// ------------------------------------------------------------------ solid

#[test]
fn sweep_circle_along_line() {
    let mut t = T::new();
    let prof = t.sketch(Reference::Plane(DatumPlane::Front), circle(DVec2::ZERO, 0.005));
    let mut path = noshape_sketch::Sketch::new();
    let l = path.add_line(DVec2::new(0.0, 0.0), DVec2::new(0.0, 0.1));
    let path_id = t.sketch(Reference::Plane(DatumPlane::Top), path);
    let r = t.regions(prof)[0].clone();
    t.add("sweep", &[("profiles", q(vec![r])), ("path", q(vec![Reference::SketchEntity { sketch: path_id, entity: l.0 }]))]);
    let res = t.ok();
    let v = total_volume(&res.context);
    assert!(near(v, PI * 0.005f64.powi(2) * 0.1, 1e-3), "sweep volume {v}");
    assert_eq!(res.context.bodies[0].faces.len(), 3);
}

#[test]
fn loft_between_squares() {
    let mut t = T::new();
    let s1 = t.sketch(Reference::Plane(DatumPlane::Top), rect(-0.01, -0.01, 0.01, 0.01));
    let pl = t.add("cPlane", &[("entities", q(vec![Reference::Plane(DatumPlane::Top)])), ("cplaneType", en("OFFSET")), ("offset", e("30 mm"))]);
    let s2 = t.sketch(Reference::FeaturePlane(pl), rect(-0.005, -0.005, 0.005, 0.005));
    let r1 = t.regions(s1)[0].clone();
    let r2 = t.regions(s2)[0].clone();
    let item = |r: Reference| {
        let mut v = ParamValues::default();
        v.set("sheetProfileEntities", q(vec![r]));
        v
    };
    t.add("loft", &[("sheetProfilesArray", ParamValue::Array(vec![item(r1), item(r2)]))]);
    let res = t.ok();
    let v = total_volume(&res.context);
    let expect = 0.03 / 3.0 * (4e-4 + 1e-4 + 2e-4);
    assert!(near(v, expect, 1e-3), "loft volume {v} vs {expect}");
    assert_eq!(res.context.bodies[0].faces.len(), 6);
}

#[test]
fn thicken_region_and_face() {
    let mut t = T::new();
    let s = t.sketch(Reference::Plane(DatumPlane::Top), rect(0.0, 0.0, 0.04, 0.02));
    let r = t.regions(s)[0].clone();
    t.add("thicken", &[("entities", q(vec![r])), ("thickness1", e("5 mm"))]);
    let res = t.ok();
    assert!(near(total_volume(&res.context), 0.04 * 0.02 * 0.005, 1e-6), "thicken volume {} faces {}", total_volume(&res.context), res.context.bodies[0].faces.len());
    // Thicken the top face of the new part with Add: the part grows.
    let top = face_with_normal(&res.context.bodies[0], DVec3::Z);
    t.add("thicken", &[("entities", q(vec![top])), ("thickness1", e("3 mm")), ("operationType", en("ADD")), ("defaultScope", ParamValue::Bool(true))]);
    let res = t.ok();
    assert_eq!(res.context.bodies.len(), 1);
    assert!(near(total_volume(&res.context), 0.04 * 0.02 * 0.008, 1e-6));
}

#[test]
fn enclose_body() {
    let mut t = T::new();
    let (_, x) = t.block(0.0, 0.0, 0.02, 0.02, "20 mm");
    t.add("enclose", &[("entities", q(vec![body_ref(&BodyId { feature: x, index: 0 })])), ("keepTools", b(true))]);
    let res = t.ok();
    assert_eq!(res.context.bodies.len(), 2);
    assert!(near(total_volume(&res.context), 2.0 * 8e-6, 1e-6));
}

#[test]
fn rib_in_l_bracket() {
    let mut t = T::new();
    // L profile on the Front plane (x right, z up), extruded symmetric.
    let mut sk = noshape_sketch::Sketch::new();
    let pts = [(0.0, 0.0), (0.1, 0.0), (0.1, 0.01), (0.01, 0.01), (0.01, 0.1), (0.0, 0.1)];
    let ids: Vec<_> = pts.iter().map(|p| sk.add_point(DVec2::new(p.0, p.1))).collect();
    for i in 0..ids.len() {
        sk.add_line_between(ids[i], ids[(i + 1) % ids.len()]).unwrap();
    }
    let s = t.sketch(Reference::Plane(DatumPlane::Front), sk);
    let r = t.regions(s)[0].clone();
    let x = t.extrude(r, "50 mm", "NEW");
    t.set(x, "symmetric", b(true));
    let base = t.ok();
    let v0 = total_volume(&base.context);
    let mut rs = noshape_sketch::Sketch::new();
    let l = rs.add_line(DVec2::new(0.01, 0.06), DVec2::new(0.06, 0.01));
    let rsk = t.sketch(Reference::Plane(DatumPlane::Front), rs);
    // Live L22: no Parts is an error (the dialog fills them).
    let rib = t.add("rib", &[("profiles", q(vec![Reference::SketchEntity { sketch: rsk, entity: l.0 }])), ("thickness", e("5 mm"))]);
    let st = t.regen().display.features.last().cloned().unwrap();
    assert_eq!(st.message.as_deref(), Some("Select parts where the rib will be fitted into."));
    t.set(rib, "parts", q(vec![Reference::Body(BodyId { feature: x, index: 0 })]));
    let res = t.ok();
    assert_eq!(res.context.bodies.len(), 1);
    let v = total_volume(&res.context);
    assert!(near(v - v0, 0.05 * 0.05 * 0.5 * 0.005, 2e-3), "rib volume {}", v - v0);
}

#[test]
fn primitives() {
    let mut t = T::new();
    t.add("cube", &[("sideLength", e("10 mm"))]);
    t.add("fSphere", &[("center", q(vec![Reference::Origin])), ("radius", e("20 mm"))]);
    let res = t.ok();
    assert_eq!(res.context.bodies.len(), 2);
    let v = total_volume(&res.context);
    assert!(near(v, 1e-6 + 4.0 / 3.0 * PI * 0.02f64.powi(3), 1e-3), "{v}");
}

// ------------------------------------------------------------------ hole

/// Block 100x50x25 mm, plane on its top face, sketch with one point.
fn hole_setup(t: &mut T) -> (FeatureId, u32) {
    t.block(0.0, 0.0, 0.1, 0.05, "25 mm");
    let pl = t.add("cPlane", &[("entities", q(vec![Reference::Plane(DatumPlane::Top)])), ("cplaneType", en("OFFSET")), ("offset", e("25 mm"))]);
    let mut sk = noshape_sketch::Sketch::new();
    let p = sk.add_point(DVec2::new(0.03, 0.02));
    let s = t.sketch(Reference::FeaturePlane(pl), sk);
    (s, p.0)
}

fn hole_with(params: &[(&str, ParamValue)]) -> (f64, usize) {
    let mut t = T::new();
    let (s, p) = hole_setup(&mut t);
    let v0 = 0.1 * 0.05 * 0.025;
    let mut all: Vec<(&str, ParamValue)> = vec![("locations", q(vec![Reference::SketchEntity { sketch: s, entity: p }])), ("unitsSystem", en("METRIC"))];
    all.extend(params.iter().cloned());
    t.add("hole", &all);
    let res = t.ok();
    assert_eq!(res.context.bodies.len(), 1);
    (v0 - total_volume(&res.context), res.context.bodies[0].faces.len())
}

#[test]
fn hole_simple_blind_flat() {
    let (removed, faces) = hole_with(&[("holeDiameterV3", e("10 mm")), ("holeDepthV3", e("10 mm")), ("tipAngleStyle", en("FLAT"))]);
    assert!(near(removed, PI * 0.005f64.powi(2) * 0.01, 1e-3), "{removed}");
    assert_eq!(faces, 8);
}

#[test]
fn hole_blind_with_tip() {
    let (removed, _) = hole_with(&[("holeDiameterV3", e("10 mm")), ("holeDepthV3", e("10 mm"))]);
    let tip = 0.005 / (59f64.to_radians()).tan();
    let expect = PI * 0.005f64.powi(2) * (0.01 + tip / 3.0);
    assert!(near(removed, expect, 1e-3), "{removed} vs {expect}");
}

#[test]
fn hole_through_counterbore() {
    let (removed, _) = hole_with(&[
        ("styleV2", en("C_BORE")),
        ("endStyleV2", en("THROUGH")),
        ("holeDiameterV3", e("6 mm")),
        ("cBoreDiameterV3", e("12 mm")),
        ("cBoreDepthV3", e("5 mm")),
    ]);
    let expect = PI * 0.003f64.powi(2) * 0.02 + PI * 0.006f64.powi(2) * 0.005;
    assert!(near(removed, expect, 1e-3), "{removed} vs {expect}");
}

#[test]
fn hole_countersink_through() {
    let (removed, _) = hole_with(&[
        ("styleV2", en("C_SINK")),
        ("endStyleV2", en("THROUGH")),
        ("holeDiameterV3", e("6 mm")),
        ("cSinkDiameterV3", e("12 mm")),
        ("cSinkAngleV3", e("90 deg")),
    ]);
    // Cone frustum of height 3 mm from r=6 to r=3, then the shaft.
    let h = 0.003;
    let frustum = PI * h / 3.0 * (0.006f64.powi(2) + 0.003f64.powi(2) + 0.006 * 0.003);
    let expect = frustum + PI * 0.003f64.powi(2) * (0.025 - h);
    assert!(near(removed, expect, 1e-3), "{removed} vs {expect}");
}

/// A tapped hole cuts its Diameter, which the standard sets to the tap drill
/// (holetables.gen.fs: M5 × 0.8 holeDiameter = tapDrillDiameter = 4.2 mm, as
/// the dialog copies it); the tap drill only replaces it with a fastener fit
/// (hole.fs `hasClearance`), so an edited Diameter is what gets cut.
#[test]
fn hole_tapped_cuts_its_diameter() {
    let path = || ParamValue::Lookup(vec!["Tapped".into(), "Straight tap".into(), "M5".into()]);
    let (removed, _) = hole_with(&[("isoHoleTableEx", path()), ("endStyleV2", en("THROUGH")), ("holeDiameterV3", e("4.2 mm"))]);
    assert!(near(removed, PI * 0.0021f64.powi(2) * 0.025, 1e-3), "{removed}");
    let (removed, _) = hole_with(&[("isoHoleTableEx", path()), ("endStyleV2", en("THROUGH")), ("holeDiameterV3", e("4.5 mm"))]);
    assert!(near(removed, PI * 0.00225f64.powi(2) * 0.025, 1e-3), "{removed}");
}

#[test]
fn hole_up_to_next_from_part() {
    let (removed, _) = hole_with(&[("holeDiameterV3", e("8 mm")), ("endStyleV2", en("UP_TO_NEXT")), ("tipAngleStyle", en("FLAT"))]);
    assert!(near(removed, PI * 0.004f64.powi(2) * 0.025, 1e-3), "{removed}");
}

#[test]
fn external_thread_is_cosmetic() {
    let mut t = T::new();
    let s = t.sketch(Reference::Plane(DatumPlane::Top), circle(DVec2::ZERO, 0.004));
    let r = t.regions(s)[0].clone();
    t.extrude(r, "20 mm", "NEW");
    let res = t.ok();
    let v0 = total_volume(&res.context);
    let edge = edge_near(&res.context.bodies[0], DVec3::new(0.004, 0.0, 0.02));
    let faces0 = res.context.bodies[0].shape.faces().len();
    t.add("externalThread", &[("entities", q(vec![edge]))]);
    let res = t.ok();
    assert!(near(total_volume(&res.context), v0, 1e-9));
    // Live (ADV7): "Split face" (on by default) splits the cylinder where the
    // 10 mm thread ends: 3 → 4 faces, one boundary circle at z = 10 mm.
    let b = &res.context.bodies[0];
    assert_eq!(b.shape.faces().len(), faces0 + 1);
    let split = noshape_kernel::make_vertex(DVec3::new(0.004, 0.0, 0.01)).unwrap();
    assert!(b.shape.edges().iter().any(|e| noshape_kernel::distance(&split, e).is_ok_and(|d| d.distance < 1e-9)), "no edge at z = 10 mm");
}

// ------------------------------------------------------------------ detail extras

#[test]
fn draft_side_face() {
    let mut t = T::new();
    t.block(0.0, 0.0, 0.04, 0.04, "20 mm");
    let res = t.ok();
    let v0 = total_volume(&res.context);
    let side = face_with_normal(&res.context.bodies[0], DVec3::X);
    t.add("draft", &[("neutralPlane", q(vec![Reference::Plane(DatumPlane::Top)])), ("draftFaces", q(vec![side])), ("angle", e("5 deg"))]);
    let res = t.ok();
    let v = total_volume(&res.context);
    let dv = 0.5 * 0.02 * (0.02 * 5f64.to_radians().tan()) * 0.04;
    assert!(near((v - v0).abs(), dv, 1e-3), "draft volume change {} vs {dv}", v - v0);
}

#[test]
fn face_blend_adjacent_faces() {
    let mut t = T::new();
    t.block(0.0, 0.0, 0.04, 0.04, "20 mm");
    let res = t.ok();
    let b0 = res.context.bodies[0].clone();
    let top = face_with_normal(&b0, DVec3::Z);
    let side = face_with_normal(&b0, DVec3::X);
    t.add("faceBlend", &[("side1", q(vec![top])), ("side2", q(vec![side])), ("radius", e("4 mm"))]);
    let res = t.ok();
    assert_eq!(res.context.bodies[0].faces.len(), 7);
    let removed = 0.04 * 0.04 * 0.02 - total_volume(&res.context);
    assert!(near(removed, (1.0 - PI / 4.0) * 0.004f64.powi(2) * 0.04, 1e-3));
}

#[test]
fn modify_fillet_radius_and_remove() {
    let mut t = T::new();
    t.block(0.0, 0.0, 0.04, 0.04, "20 mm");
    let res = t.ok();
    let edge = edge_near(&res.context.bodies[0], DVec3::new(0.04, 0.02, 0.02));
    t.add("fillet", &[("entities", q(vec![edge])), ("radius", e("3 mm"))]);
    let res = t.ok();
    let b = res.context.bodies[0].clone();
    let fillet_face = b.faces.iter().enumerate().find(|(i, _)| b.shape.faces()[*i].face_plane().unwrap().is_none()).map(|(_, n)| Reference::Face((**n).clone())).unwrap();
    let m = t.add("modifyFillet", &[("faces", q(vec![fillet_face.clone()])), ("radius", e("6 mm"))]);
    let res = t.ok();
    assert_eq!(res.context.bodies[0].faces.len(), 7);
    let removed = 0.04 * 0.04 * 0.02 - total_volume(&res.context);
    assert!(near(removed, (1.0 - PI / 4.0) * 0.006f64.powi(2) * 0.04, 1e-3), "{removed}");
    // The new fillet face keeps the original name.
    assert!(res.context.bodies[0].find_faces(match &fillet_face {
        Reference::Face(n) => n,
        _ => unreachable!(),
    })
    .len()
        == 1);
    t.set(m, "modifyFilletType", en("REMOVE_FILLET"));
    let res = t.ok();
    assert_eq!(res.context.bodies[0].faces.len(), 6);
    assert!(near(total_volume(&res.context), 0.04 * 0.04 * 0.02, 1e-6));
}

#[test]
fn unsupported_features_fail_clearly() {
    let mut t = T::new();
    t.block(0.0, 0.0, 0.04, 0.04, "20 mm");
    t.add("bodyDraft", &[]);
    t.add("replaceFace", &[]);
    let res = t.regen();
    assert_eq!(res.display.features[2].status, FeatureStatus::Error);
    assert!(res.display.features[2].message.as_deref().unwrap().contains("not supported"));
    assert_eq!(res.display.features[3].status, FeatureStatus::Error);
}

// ------------------------------------------------------------------ split

#[test]
fn split_part_by_plane() {
    let mut t = T::new();
    let (_, x) = t.block(0.0, 0.0, 0.1, 0.04, "20 mm");
    let pl = t.add("cPlane", &[("entities", q(vec![Reference::Plane(DatumPlane::Right)])), ("cplaneType", en("OFFSET")), ("offset", e("30 mm"))]);
    let sp = t.add("splitPart", &[("targets", q(vec![body_ref(&BodyId { feature: x, index: 0 })])), ("tool", q(vec![Reference::FeaturePlane(pl)]))]);
    let res = t.ok();
    assert_eq!(res.context.bodies.len(), 2);
    // Live ("qa5 booleans"): both pieces are new parts, the front piece
    // (positive x side) first.
    let b0 = &res.context.bodies[0];
    assert!(res.context.bodies.iter().all(|b| b.id != BodyId { feature: x, index: 0 }));
    assert_eq!(b0.color, res.context.bodies[1].color, "both keep the source colour");
    assert!(near(b0.shape.volume().unwrap(), 0.07 * 0.04 * 0.02, 1e-6));
    assert!(near(res.context.bodies[1].shape.volume().unwrap(), 0.03 * 0.04 * 0.02, 1e-6));
    t.set(sp, "keepBothSides", b(false));
    t.set(sp, "keepFront", b(false));
    let res = t.ok();
    assert_eq!(res.context.bodies.len(), 1);
    assert!(near(total_volume(&res.context), 0.03 * 0.04 * 0.02, 1e-6));
}

#[test]
fn split_face_with_sketch_line() {
    let mut t = T::new();
    t.block(0.0, 0.0, 0.1, 0.04, "20 mm");
    let res = t.ok();
    let top = face_with_normal(&res.context.bodies[0], DVec3::Z);
    let mut sk = noshape_sketch::Sketch::new();
    let l = sk.add_line(DVec2::new(0.05, -0.01), DVec2::new(0.05, 0.05));
    let s = t.sketch(Reference::Plane(DatumPlane::Top), sk);
    t.add("splitPart", &[("splitType", en("FACE")), ("faceTargets", q(vec![top])), ("faceTools", q(vec![Reference::SketchEntity { sketch: s, entity: l.0 }]))]);
    let res = t.ok();
    assert_eq!(res.context.bodies[0].faces.len(), 7);
}

// ------------------------------------------------------------------ patterns

fn x_axis_dir() -> Reference {
    Reference::Plane(DatumPlane::Right)
}

#[test]
fn linear_part_pattern_new_add_and_skip() {
    let mut t = T::new();
    let (_, x) = t.block(0.0, 0.0, 0.01, 0.01, "10 mm");
    let lp = t.add(
        "linearPattern",
        &[("entities", q(vec![body_ref(&BodyId { feature: x, index: 0 })])), ("directionOne", q(vec![x_axis_dir()])), ("distance", e("20 mm")), ("instanceCount", e("3"))],
    );
    let res = t.ok();
    assert_eq!(solids(&res.context).len(), 3);
    assert!(near(total_volume(&res.context), 3e-6, 1e-6));
    // Skip instance 1.
    let mut item = ParamValues::default();
    item.set("index1", e("1"));
    item.set("index2", e("0"));
    t.set(lp, "skipInstances", b(true));
    t.set(lp, "skippedInstances", ParamValue::Array(vec![item]));
    let res = t.ok();
    assert_eq!(solids(&res.context).len(), 2);
    // Add with overlapping instances merges into one part.
    t.set(lp, "skipInstances", b(false));
    t.set(lp, "distance", e("5 mm"));
    t.set(lp, "operationType", en("ADD"));
    t.set(lp, "defaultScope", ParamValue::Bool(true));
    let res = t.ok();
    assert_eq!(res.context.bodies.len(), 1);
    assert!(near(total_volume(&res.context), 0.02 * 0.01 * 0.01, 1e-6));
}

#[test]
fn circular_and_mirror_part_pattern() {
    let mut t = T::new();
    let (_, x) = t.block(0.02, -0.005, 0.03, 0.005, "10 mm");
    let id = BodyId { feature: x, index: 0 };
    let axis = edge_axis_z();
    t.add("circularPattern", &[("entities", q(vec![body_ref(&id)])), ("axis", q(vec![axis])), ("instanceCount", e("4"))]);
    let res = t.ok();
    assert_eq!(res.context.bodies.len(), 4);
    let c: DVec3 = res.context.bodies.iter().map(|b| noshape_kernel::volume_properties(&b.shape).unwrap().center).sum::<DVec3>() / 4.0;
    assert!(c.truncate().length() < 1e-9, "instances evenly around the axis: {c:?}");
    t.add("mirror", &[("entities", q(vec![body_ref(&id)])), ("mirrorPlane", q(vec![Reference::Plane(DatumPlane::Right)]))]);
    let res = t.ok();
    assert_eq!(res.context.bodies.len(), 5);
    let last = res.context.bodies.last().unwrap();
    let bb = last.shape.bounding_box(true).unwrap();
    assert!((bb.min.x + 0.03).abs() < 1e-6 && (bb.max.x + 0.02).abs() < 1e-6, "{bb:?}");
}

/// The Z axis as a direction/axis reference (Top plane normal).
fn edge_axis_z() -> Reference {
    Reference::Plane(DatumPlane::Top)
}

#[test]
fn curve_pattern_along_line() {
    let mut t = T::new();
    let (_, x) = t.block(0.0, 0.0, 0.01, 0.01, "10 mm");
    let mut sk = noshape_sketch::Sketch::new();
    let l = sk.add_line(DVec2::new(0.0, 0.0), DVec2::new(0.1, 0.0));
    let s = t.sketch(Reference::Plane(DatumPlane::Top), sk);
    t.add(
        "curvePattern",
        &[("entities", q(vec![body_ref(&BodyId { feature: x, index: 0 })])), ("edges", q(vec![Reference::SketchEntity { sketch: s, entity: l.0 }])), ("instanceCount", e("5"))],
    );
    let res = t.ok();
    assert_eq!(res.context.bodies.len(), 5);
    let maxx = res.context.bodies.iter().map(|b| b.shape.bounding_box(true).unwrap().max.x).fold(0.0, f64::max);
    assert!((maxx - 0.11).abs() < 1e-6, "{maxx}");
}

/// Block with a cut cylinder pocket at (10, 10) mm, r = 3 mm, 5 mm deep.
fn pocket_setup(t: &mut T) -> (FeatureId, FeatureId, FeatureId) {
    let (s, x) = t.block(0.0, 0.0, 0.1, 0.02, "10 mm");
    let pl = t.add("cPlane", &[("entities", q(vec![Reference::Plane(DatumPlane::Top)])), ("cplaneType", en("OFFSET")), ("offset", e("10 mm"))]);
    let cs = t.sketch(Reference::FeaturePlane(pl), circle(DVec2::new(0.01, 0.01), 0.003));
    let r = t.regions(cs)[0].clone();
    let cut = t.extrude(r, "5 mm", "REMOVE");
    t.set(cut, "oppositeDirection", b(true));
    let _ = (s, x);
    (s, x, cut)
}

#[test]
fn feature_and_face_pattern_of_pocket() {
    let mut t = T::new();
    let (_, _, cut) = pocket_setup(&mut t);
    let res = t.ok();
    let pocket = PI * 0.003f64.powi(2) * 0.005;
    assert!(near(0.1 * 0.02 * 0.01 - total_volume(&res.context), pocket, 1e-3));
    let fp = t.add("linearPattern", &[("patternType", en("FEATURE")), ("instanceFunction", q(vec![Reference::Feature(cut)])), ("directionOne", q(vec![x_axis_dir()])), ("distance", e("20 mm")), ("instanceCount", e("4"))]);
    let res = t.ok();
    assert_eq!(res.context.bodies.len(), 1);
    let removed = 0.1 * 0.02 * 0.01 - total_volume(&res.context);
    assert!(near(removed, 4.0 * pocket, 1e-3), "{removed}");
    // Face pattern of the pocket's faces gives the same result.
    let faces: Vec<Reference> = {
        t.ps.features.pop();
        let res = t.ok();
        let b = &res.context.bodies[0];
        b.faces.iter().filter(|n| n.creator() == Some(cut)).map(|n| Reference::Face((**n).clone())).collect()
    };
    assert_eq!(faces.len(), 2, "pocket wall + floor");
    let _ = fp;
    t.add("linearPattern", &[("patternType", en("FACE")), ("faces", q(faces)), ("directionOne", q(vec![x_axis_dir()])), ("distance", e("30 mm")), ("instanceCount", e("2"))]);
    let res = t.ok();
    let removed = 0.1 * 0.02 * 0.01 - total_volume(&res.context);
    assert!(near(removed, 2.0 * pocket, 1e-3), "{removed}");
}

#[test]
fn pattern_instance_references_survive_upstream_edit() {
    let mut t = T::new();
    let (s, x) = t.block(0.0, 0.0, 0.01, 0.01, "10 mm");
    t.add(
        "linearPattern",
        &[("entities", q(vec![body_ref(&BodyId { feature: x, index: 0 })])), ("directionOne", q(vec![x_axis_dir()])), ("distance", e("30 mm")), ("instanceCount", e("3"))],
    );
    let res = t.ok();
    // An edge on the top of instance 2 (second copy).
    let inst = res.context.bodies.iter().find(|b| b.shape.bounding_box(true).unwrap().min.x > 0.05).unwrap().clone();
    let edge = edge_near(&inst, DVec3::new(0.07, 0.005, 0.01));
    assert!(format!("{edge:?}").contains("Copy"), "{edge:?}");
    t.add("fillet", &[("entities", q(vec![edge])), ("radius", e("2 mm"))]);
    let res = t.ok();
    let filleted = res.context.bodies.iter().filter(|b| b.faces.len() == 7).count();
    assert_eq!(filleted, 1);
    // Upstream edit: make the seed wider. The fillet still finds its edge.
    if let Some(ParamValue::Sketch(d)) = t.ps.features[0].params.0.get_mut("sketch") {
        let ids: Vec<_> = d.sketch.entities().filter(|e| e.kind() == noshape_sketch::EntityKind::Point).map(|e| e.id).collect();
        for id in [ids[1], ids[2]] {
            let p = d.sketch.point(id).unwrap();
            d.sketch.set_point(id, p + DVec2::new(0.005, 0.0)).unwrap();
        }
    }
    let _ = s;
    let res = t.ok();
    assert_eq!(res.context.bodies.len(), 3);
    assert_eq!(res.context.bodies.iter().filter(|b| b.faces.len() == 7).count(), 1);
    assert!(near(res.context.bodies[0].shape.volume().unwrap(), 0.015 * 0.01 * 0.01, 1e-6));
}

#[test]
fn feature_pattern_references_survive_upstream_edit() {
    let mut t = T::new();
    let (_, _, cut) = pocket_setup(&mut t);
    t.add("linearPattern", &[("patternType", en("FEATURE")), ("instanceFunction", q(vec![Reference::Feature(cut)])), ("directionOne", q(vec![x_axis_dir()])), ("distance", e("20 mm")), ("instanceCount", e("3"))]);
    let res = t.ok();
    let b = res.context.bodies[0].clone();
    // Rim edge of the third pocket (instance 2 -> code 4).
    let rim = edge_near(&b, DVec3::new(0.05 + 0.003, 0.01, 0.01));
    assert!(format!("{rim:?}").contains("Copy"), "{rim:?}");
    t.add("chamfer", &[("entities", q(vec![rim])), ("width", e("0.5 mm"))]);
    let res = t.ok();
    let f0 = res.context.bodies[0].faces.len();
    // Upstream: make the pocket deeper.
    t.set(cut, "depth", e("7 mm"));
    let res = t.ok();
    assert_eq!(res.context.bodies[0].faces.len(), f0);
}

// ------------------------------------------------------------------ transform / copy

#[test]
fn transform_move_copy_rotate_scale() {
    let mut t = T::new();
    let (_, x) = t.block(0.0, 0.0, 0.01, 0.01, "10 mm");
    let id = BodyId { feature: x, index: 0 };
    let tr = t.add("transform", &[("entities", q(vec![body_ref(&id)])), ("transformType", en("TRANSLATION_3D")), ("dx", e("5 mm")), ("dz", e("2 mm"))]);
    let res = t.ok();
    assert_eq!(res.context.bodies.len(), 1);
    assert_eq!(res.context.bodies[0].id, id);
    let bb = res.context.bodies[0].shape.bounding_box(true).unwrap();
    assert!((bb.min.x - 0.005).abs() < 1e-6 && (bb.min.z - 0.002).abs() < 1e-6, "{bb:?}");
    t.set(tr, "makeCopy", b(true));
    let res = t.ok();
    assert_eq!(res.context.bodies.len(), 2);
    t.set(tr, "makeCopy", b(false));
    t.set(tr, "transformType", en("SCALE_UNIFORMLY"));
    t.set(tr, "scale", e("2"));
    let res = t.ok();
    assert!(near(total_volume(&res.context), 8e-6, 1e-6));
    t.set(tr, "transformType", en("ROTATION"));
    t.set(tr, "transformAxis", q(vec![edge_axis_z()]));
    t.set(tr, "angle", e("90 deg"));
    let res = t.ok();
    let bb = res.context.bodies[0].shape.bounding_box(true).unwrap();
    assert!((bb.max.x).abs() < 1e-6 && (bb.min.x + 0.01).abs() < 1e-6, "{bb:?}");
    t.add("copyPart", &[("entities", q(vec![body_ref(&id)]))]);
    let res = t.ok();
    assert_eq!(res.context.bodies.len(), 2);
}

// ------------------------------------------------------------------ direct edit

#[test]
fn delete_face_heal_cap_void() {
    let mut t = T::new();
    let (_, _, cut) = pocket_setup(&mut t);
    let res = t.ok();
    let b = res.context.bodies[0].clone();
    let wall: Vec<Reference> = b.faces.iter().filter(|n| n.creator() == Some(cut)).map(|n| Reference::Face((**n).clone())).collect();
    let d = t.add("deleteFace", &[("deleteFaces", q(wall))]);
    let res = t.ok();
    assert!(near(total_volume(&res.context), 0.1 * 0.02 * 0.01, 1e-6));
    assert_eq!(res.context.bodies[0].faces.len(), 6);
    // Cap the block's top face instead.
    let top = face_with_normal(&b, DVec3::Z);
    t.set(d, "deleteFaces", q(vec![top.clone()]));
    t.set(d, "healType", en("CAP"));
    let res = t.ok();
    assert!(near(total_volume(&res.context), 0.1 * 0.02 * 0.01 - PI * 0.003f64.powi(2) * 0.005, 1e-4));
    t.set(d, "healType", en("VOID"));
    let res = t.ok();
    assert_eq!(res.context.bodies[0].kind, noshape_model::display::BodyKind::Surface);
}

#[test]
fn move_face_offset_and_translate() {
    let mut t = T::new();
    t.block(0.0, 0.0, 0.02, 0.02, "10 mm");
    let res = t.ok();
    let top = face_with_normal(&res.context.bodies[0], DVec3::Z);
    let m = t.add("moveFace", &[("moveFaces", q(vec![top])), ("offsetDistance", e("5 mm"))]);
    let res = t.ok();
    assert!(near(total_volume(&res.context), 0.02 * 0.02 * 0.015, 1e-6));
    t.set(m, "moveFaceType", en("TRANSLATE"));
    t.set(m, "direction", q(vec![edge_axis_z()]));
    t.set(m, "translationDistance", e("2 mm"));
    t.set(m, "oppositeDirection", b(true));
    let res = t.ok();
    assert!(near(total_volume(&res.context), 0.02 * 0.02 * 0.008, 1e-6));
    t.set(m, "moveFaceType", en("ROTATE"));
    let res = t.regen();
    assert_eq!(res.display.features[2].status, FeatureStatus::Error);
}

#[test]
fn sweep_along_line_and_arc() {
    let mut t = T::new();
    let prof = t.sketch(Reference::Plane(DatumPlane::Front), circle(DVec2::ZERO, 0.003));
    // Path on Top: line along +Y then a tangent quarter arc turning to +X.
    let mut path = noshape_sketch::Sketch::new();
    let l = path.add_line(DVec2::new(0.0, 0.0), DVec2::new(0.0, 0.05));
    let a = path.add_arc(DVec2::new(0.02, 0.05), DVec2::new(0.02, 0.07), DVec2::new(0.0, 0.05));
    let pid = t.sketch(Reference::Plane(DatumPlane::Top), path);
    let r = t.regions(prof)[0].clone();
    t.add("sweep", &[("profiles", q(vec![r])), ("path", q(vec![Reference::SketchEntity { sketch: pid, entity: l.0 }, Reference::SketchEntity { sketch: pid, entity: a.0 }]))]);
    let res = t.ok();
    let len = 0.05 + 0.5 * PI * 0.02;
    let v = total_volume(&res.context);
    assert!(near(v, PI * 0.003f64.powi(2) * len, 2e-3), "{v}");
}

#[test]
fn mirror_feature_pattern_of_pocket() {
    let mut t = T::new();
    let (_, _, cut) = pocket_setup(&mut t);
    let pl = t.add("cPlane", &[("entities", q(vec![Reference::Plane(DatumPlane::Right)])), ("cplaneType", en("OFFSET")), ("offset", e("50 mm"))]);
    t.add("mirror", &[("patternType", en("FEATURE")), ("instanceFunction", q(vec![Reference::Feature(cut)])), ("mirrorPlane", q(vec![Reference::FeaturePlane(pl)]))]);
    let res = t.ok();
    let removed = 0.1 * 0.02 * 0.01 - total_volume(&res.context);
    assert!(near(removed, 2.0 * PI * 0.003f64.powi(2) * 0.005, 1e-3), "{removed}");
    assert_eq!(res.context.bodies[0].faces.len(), 6 + 4);
}

/// hole.fs: a blind hole started from a sketch plane above the part that
/// ends before reaching it cuts nothing and fails ("None of the holes
/// intersected a part."); the counterbore check comes before the points.
#[test]
fn hole_misses_and_order_as_onshape() {
    let mut t = T::new();
    t.block(0.0, 0.0, 0.1, 0.05, "25 mm");
    let pl = t.add("cPlane", &[("entities", q(vec![Reference::Plane(DatumPlane::Top)])), ("cplaneType", en("OFFSET")), ("offset", e("40 mm"))]);
    let mut sk = noshape_sketch::Sketch::new();
    let p = sk.add_point(DVec2::new(0.03, 0.02));
    let s = t.sketch(Reference::FeaturePlane(pl), sk);
    t.add("hole", &[("locations", q(vec![Reference::SketchEntity { sketch: s, entity: p.0 }])), ("unitsSystem", en("METRIC")), ("startStyle", en("SKETCH")), ("holeDepthV3", e("5 mm"))]);
    let st = t.regen().display.features.last().cloned().unwrap();
    assert_eq!((st.status, st.message.as_deref()), (FeatureStatus::Error, Some("None of the holes intersected a part.")));

    let mut t = T::new();
    t.block(0.0, 0.0, 0.1, 0.05, "25 mm");
    t.add("hole", &[("unitsSystem", en("METRIC")), ("styleV2", en("C_BORE")), ("holeDiameterV3", e("10 mm")), ("cBoreDiameterV3", e("8 mm"))]);
    let st = t.regen().display.features.last().cloned().unwrap();
    assert_eq!(st.message.as_deref(), Some("The counterbore diameter must be larger than the hole diameter."));
}

/// Block 40×20×10 with a sketch point at (20, 10) on its top.
fn hole_block(t: &mut T) -> (FeatureId, u32) {
    t.block(0.0, 0.0, 0.04, 0.02, "10 mm");
    let pl = t.add("cPlane", &[("entities", q(vec![Reference::Plane(DatumPlane::Top)])), ("cplaneType", en("OFFSET")), ("offset", e("10 mm"))]);
    let mut sk = noshape_sketch::Sketch::new();
    let p = sk.add_point(DVec2::new(0.02, 0.01));
    (t.sketch(Reference::FeaturePlane(pl), sk), p.0)
}

/// Live L47, L61 ("psb L46-61 hole"): a countersink deeper than its blind
/// hole is allowed (6969.1508 mm³ left); Up to next with a 2 mm offset puts
/// the tip's apex 2 mm before the bottom (shaft 6.0923 deep) and the Depth
/// field reads 4.092.
#[test]
fn hole_countersink_and_offset_as_live() {
    let mut t = T::new();
    let (s, p) = hole_block(&mut t);
    t.add("hole", &[
        ("locations", q(vec![Reference::SketchEntity { sketch: s, entity: p }])),
        ("unitsSystem", en("METRIC")),
        ("tableSynced", b(true)),
        ("styleV2", en("C_SINK")),
        ("holeDiameterV3", e("5 mm")),
        ("cSinkDiameterV3", e("20 mm")),
        ("cSinkAngleV3", e("90 deg")),
        ("holeDepthV3", e("2 mm")),
    ]);
    let r = t.ok();
    // The frustum Ø20 → Ø5 over 7.5 mm leaves 6969.165; live reports
    // 6969.1508 (0.014 mm³ apart).
    let v = total_volume(&r.context) * 1e9;
    assert!((v - 6969.1508).abs() < 0.02, "{v}");

    let mut t = T::new();
    let (s, p) = hole_block(&mut t);
    t.add("hole", &[
        ("locations", q(vec![Reference::SketchEntity { sketch: s, entity: p }])),
        ("unitsSystem", en("METRIC")),
        ("tableSynced", b(true)),
        ("holeDiameterV3", e("6.35 mm")),
        ("endStyleV2", en("UP_TO_NEXT")),
        ("offset", b(true)),
        ("offsetDistance", e("2 mm")),
    ]);
    let r = t.ok();
    let shown = r.display.features.last().unwrap().computed.iter().find(|(k, _)| k == "holeDepthComputedV3").map(|(_, v)| v.clone());
    match shown {
        Some(noshape_model::display::ComputedValue::Length(d)) => assert!((d * 1e3 - 4.0923).abs() < 1e-3, "{d}"),
        other => panic!("{other:?}"),
    }
    let bb = r.context.bodies[0].shape.bounding_box(true).unwrap();
    assert!(bb.min.z.abs() < 1e-9);
}

/// Live L53: a location off the part fails the whole hole.
#[test]
fn hole_point_off_the_part_fails_the_hole() {
    let mut t = T::new();
    t.block(0.0, 0.0, 0.04, 0.02, "10 mm");
    let pl = t.add("cPlane", &[("entities", q(vec![Reference::Plane(DatumPlane::Top)])), ("cplaneType", en("OFFSET")), ("offset", e("10 mm"))]);
    let mut sk = noshape_sketch::Sketch::new();
    let a = sk.add_point(DVec2::new(0.01, 0.01));
    let off = sk.add_point(DVec2::new(0.08, 0.01));
    let s = t.sketch(Reference::FeaturePlane(pl), sk);
    t.add("hole", &[("locations", q(vec![Reference::SketchEntity { sketch: s, entity: a.0 }, Reference::SketchEntity { sketch: s, entity: off.0 }])), ("unitsSystem", en("METRIC")), ("tableSynced", b(true))]);
    let st = t.regen().display.features.last().cloned().unwrap();
    assert_eq!((st.status, st.message.as_deref()), (FeatureStatus::Error, Some("Hole creates invalid geometry.")));
}

/// Live L59: the Hole table lists a part's holes, one row per distinct
/// hole (A, B) with its count.
#[test]
fn hole_table_groups_holes_per_part() {
    let mut t = T::new();
    t.block(0.0, 0.0, 0.04, 0.02, "10 mm");
    let pl = t.add("cPlane", &[("entities", q(vec![Reference::Plane(DatumPlane::Top)])), ("cplaneType", en("OFFSET")), ("offset", e("10 mm"))]);
    let mut sk = noshape_sketch::Sketch::new();
    let a = sk.add_point(DVec2::new(0.01, 0.01));
    let b2 = sk.add_point(DVec2::new(0.02, 0.01));
    let c = sk.add_point(DVec2::new(0.03, 0.01));
    let s = t.sketch(Reference::FeaturePlane(pl), sk);
    let pt = |e: noshape_sketch::EntityId| Reference::SketchEntity { sketch: s, entity: e.0 };
    t.add("hole", &[("locations", q(vec![pt(a), pt(b2)])), ("unitsSystem", en("METRIC")), ("tableSynced", b(true)), ("holeDiameterV3", e("5 mm")), ("holeDepthV3", e("5 mm"))]);
    t.add("hole", &[("locations", q(vec![pt(c)])), ("unitsSystem", en("METRIC")), ("tableSynced", b(true)), ("holeDiameterV3", e("3 mm")), ("endStyleV2", en("THROUGH"))]);
    let r = t.ok();
    let units = UnitPrefs::default();
    let tables = noshape_model::interaction::hole_table(&r.context, &units);
    assert_eq!(tables.len(), 1);
    let rows: Vec<(String, String, String, String)> = tables[0].rows.iter().map(|r| (r.text("tag").into(), r.text("size").into(), r.text("angle").into(), r.text("qty").into())).collect();
    assert_eq!(rows, [("A".into(), "⌀5 mm ↧ 5 mm".into(), "118 deg".into(), "2".into()), ("B".into(), "⌀3 mm THRU".into(), "".into(), "1".into())]);
}

/// Live L49/L50: Tapped M6 × 1.00, Blind. A new size gives Tap clearance 0;
/// Depth 20 → clearance 15; clearance 2 → Tapped depth 18; Depth 25 →
/// clearance 7; Depth 8 → Tapped depth 1.
#[test]
fn tapped_hole_depths_stay_linked_as_live() {
    use noshape_model::features::hole_tables::link_tapped_depths;
    let units = UnitPrefs::default();
    let mut v = ParamValues::default();
    v.set("unitsSystem", ParamValue::Enum("METRIC".into()));
    v.set("styleV2", ParamValue::Enum("SIMPLE".into()));
    v.set("endStyleV2", ParamValue::Enum("BLIND".into()));
    v.set("showTappedDepth", ParamValue::Bool(true));
    v.set("isoHoleTableEx", ParamValue::Lookup(vec!["Tapped".into(), "Straight tap".into(), "M5".into()]));
    v.set("holeDepthV3", ParamValue::Expr(ExprValue::new("5 mm")));
    v.set("tappedDepthV3", ParamValue::Expr(ExprValue::new("5 mm")));
    v.set("tapClearance", ParamValue::Expr(ExprValue::new("3")));
    let step = |v: &mut ParamValues, id: &str, text: &str| -> Vec<(String, String)> {
        let prev = v.clone();
        if id == "size" {
            v.set("isoHoleTableEx", ParamValue::Lookup(vec!["Tapped".into(), "Straight tap".into(), "M6".into(), "1.00 mm".into()]));
        } else {
            v.set(id, ParamValue::Expr(ExprValue::new(text)));
        }
        let out: Vec<(String, String)> = link_tapped_depths(&prev, v, &units).into_iter().map(|(k, p)| (k.to_string(), match &p { ParamValue::Expr(e) => e.text.clone(), _ => String::new() })).collect();
        for (k, t) in &out {
            v.set(Box::leak(k.clone().into_boxed_str()), ParamValue::Expr(ExprValue::new(t.as_str())));
        }
        out
    };
    assert_eq!(step(&mut v, "size", ""), [("tapClearance".to_string(), "0".to_string())]);
    assert_eq!(step(&mut v, "holeDepthV3", "20 mm"), [("tapClearance".to_string(), "15".to_string())]);
    assert_eq!(step(&mut v, "tapClearance", "2"), [("tappedDepthV3".to_string(), "18 mm".to_string())]);
    assert_eq!(step(&mut v, "holeDepthV3", "25 mm"), [("tapClearance".to_string(), "7".to_string())]);
    assert_eq!(step(&mut v, "holeDepthV3", "8 mm"), [("tappedDepthV3".to_string(), "1 mm".to_string())]);
}

/// Live L52: Drilled 1/4 with Diameter 0.3 in reads "Parameters have been
/// overridden. Check input."; changing only the counterbore doesn't.
#[test]
fn hole_overridden_parameters_info() {
    let info = |extra: &[(&str, ParamValue)]| -> Option<String> {
        let mut t = T::new();
        let (s, p) = hole_block(&mut t);
        let mut ps: Vec<(&str, ParamValue)> = vec![("locations", q(vec![Reference::SketchEntity { sketch: s, entity: p }])), ("unitsSystem", en("INCH")), ("tableSynced", b(true))];
        ps.extend(extra.iter().cloned());
        t.add("hole", &ps);
        let r = t.ok();
        let st = r.display.features.last().unwrap();
        (st.status == FeatureStatus::Info).then(|| st.message.clone().unwrap_or_default())
    };
    let path = |k: &str| (k.to_string(), ParamValue::Lookup(vec!["Drilled".into(), "1/4 (0.25)".into()]));
    let (k1, p1) = path("ansiHoleTableEx");
    assert_eq!(info(&[(Box::leak(k1.into_boxed_str()), p1), ("holeDiameterV3", e("0.3 in"))]).as_deref(), Some("Parameters have been overridden. Check input."));
    let (k2, p2) = path("ansiHoleTable");
    assert_eq!(info(&[(Box::leak(k2.into_boxed_str()), p2), ("styleV2", en("C_BORE")), ("holeDiameterV3", e("0.25 in")), ("cBoreDiameterV3", e("0.5 in")), ("cBoreDepthV3", e("0.1 in"))]), None);
}

/// Live L41: a Feature pattern of a sketch only copies the sketch to each
/// instance (no part); Reapply on adds the blue info.
#[test]
fn feature_pattern_of_a_sketch_copies_it() {
    for reapply in [false, true] {
        let mut t = T::new();
        t.block(0.0, 0.0, 0.1, 0.02, "10 mm");
        let s = t.sketch(Reference::Plane(DatumPlane::Top), circle(DVec2::new(0.01, 0.01), 0.003));
        let fp = t.add("linearPattern", &[("patternType", en("FEATURE")), ("instanceFunction", q(vec![Reference::Feature(s)])), ("directionOne", q(vec![x_axis_dir()])), ("distance", e("20 mm")), ("instanceCount", e("3")), ("fullFeaturePattern", b(reapply))]);
        let res = t.ok();
        let st = res.display.features.last().unwrap();
        assert_eq!(st.status, if reapply { FeatureStatus::Info } else { FeatureStatus::Ok }, "{:?}", st.message);
        assert_eq!(solids(&res.context).len(), 1);
        let src = res.context.sketches.iter().find(|k| k.feature == s).unwrap().frame.origin;
        let copies: Vec<_> = res.context.sketches.iter().filter(|k| res.context.sketch_copies.iter().any(|(p, c)| *p == fp && *c == k.feature)).collect();
        assert_eq!(copies.len(), 2);
        for (i, c) in copies.iter().enumerate() {
            assert!((c.frame.origin - src).abs_diff_eq(DVec3::X * 0.02 * (i + 1) as f64, 1e-9), "{:?}", c.frame.origin);
            assert_eq!(c.regions.len(), 1);
        }
        // A copy's region can be extruded like the sketch's.
        let r = t.regions(copies[1].feature)[0].clone();
        t.extrude(r, "5 mm", "NEW");
        let res = t.ok();
        assert_eq!(solids(&res.context).len(), 2);
    }
}

/// Live L22 ("psb L22-24 rib"): an open box 40×20×10 (walls 2) and ribs on
/// its mid plane, 2.5 mm, parallel to the sketch plane, with and without
/// "Extend profiles to part".
#[test]
fn rib_extend_profiles_as_live() {
    let run = |a: (f64, f64), b2: (f64, f64), extend: bool| -> Result<f64, String> {
        let mut t = T::new();
        let (_, x) = t.block(0.0, 0.0, 0.04, 0.02, "10 mm");
        let top = t.add("cPlane", &[("entities", q(vec![Reference::Plane(DatumPlane::Top)])), ("cplaneType", en("OFFSET")), ("offset", e("10 mm"))]);
        let ps = t.sketch(Reference::FeaturePlane(top), rect(0.002, 0.002, 0.038, 0.018));
        let r = t.regions(ps)[0].clone();
        let cut = t.extrude(r, "8 mm", "REMOVE");
        t.set(cut, "oppositeDirection", b(true));
        let v0 = total_volume(&t.ok().context);
        assert!((v0 * 1e9 - 3392.0).abs() < 1e-6, "{}", v0 * 1e9);
        let mid = t.add("cPlane", &[("entities", q(vec![Reference::Plane(DatumPlane::Front)])), ("cplaneType", en("OFFSET")), ("offset", e("10 mm")), ("oppositeDirection", b(true))]);
        let mut sk = noshape_sketch::Sketch::new();
        let l = sk.add_line(DVec2::new(a.0, a.1), DVec2::new(b2.0, b2.1));
        let s = t.sketch(Reference::FeaturePlane(mid), sk);
        t.add("rib", &[("profiles", q(vec![Reference::SketchEntity { sketch: s, entity: l.0 }])), ("parts", q(vec![Reference::Body(BodyId { feature: x, index: 0 })])), ("thickness", e("2.5 mm")), ("extendProfilesUpToPart", b(extend))]);
        let res = t.regen();
        let st = res.display.features.last().unwrap();
        match st.status {
            FeatureStatus::Ok => Ok(total_volume(&res.context) * 1e9),
            _ => Err(st.message.clone().unwrap_or_default()),
        }
    };
    let ok = |r: Result<f64, String>, v: f64| {
        let got = r.unwrap();
        assert!((got - v).abs() < 1e-3, "{got} vs {v}");
    };
    ok(run((0.01, 0.008), (0.03, 0.008), false), 3692.0);
    ok(run((0.01, 0.008), (0.03, 0.008), true), 3932.0);
    ok(run((0.002, 0.01), (0.038, 0.01), false), 4112.0);
    ok(run((0.002, 0.015), (0.038, 0.015), false), 4562.0);
    assert_eq!(run((0.002, 0.015), (0.038, 0.015), true), Err("Failed to create a rib from a selected profile.".into()));
}

/// Live "lp wall over" (behaviours "URGENT (10-22)") with 5 instances, as
/// the user's shot of 2026-10-23 and live 2256–2262: the failing Feature
/// pattern carries a ghost of every instance it couldn't make (the wall's
/// tool body and the floor and end face it merged into, moved there).
#[test]
fn failed_feature_pattern_shows_its_instances() {
    let mut t = T::new();
    t.block(0.0, 0.0, 0.1, 0.02, "10 mm");
    // Right plane: sketch x = world Y, y = Z.
    let s = t.sketch(Reference::Plane(DatumPlane::Right), rect(-0.001, 0.0, 0.021, 0.011));
    let r = t.regions(s)[0].clone();
    let w = t.extrude(r, "0.5 mm", "ADD");
    let res = t.regen();
    let edge = edge_near(&res.context.bodies[0], DVec3::new(0.05, 0.0, 0.0));
    t.add("linearPattern", &[("patternType", en("FEATURE")), ("instanceFunction", q(vec![Reference::Feature(w)])), ("directionOne", q(vec![edge])), ("distance", e("2 mm")), ("instanceCount", e("5"))]);
    let res = t.regen();
    let st = res.display.features.last().unwrap();
    assert_eq!(st.status, FeatureStatus::Error);
    let m = st.error_mesh.as_ref().expect("error geometry");
    // 4 ghosts × (the wall's 6 faces + the floor it merged into; the end
    // face it merged into is just the wall's own end face).
    assert_eq!(m.face_ranges.len(), 28);
    let xs: Vec<f32> = m.positions.iter().map(|p| p[0]).collect();
    let (lo, hi) = xs.iter().fold((f32::MAX, f32::MIN), |(a, b), x| (a.min(*x), b.max(*x)));
    // 2 to 8 mm along the edge (its direction either way), the floor's
    // copies running the block's length.
    let fwd = (lo - 0.002).abs() < 1e-6 && (hi - 0.108).abs() < 1e-6;
    let back = (lo + 0.008).abs() < 1e-6 && (hi - 0.098).abs() < 1e-6;
    assert!(fwd || back, "{lo} {hi}");
    // Reapply on: no error, no error geometry.
    let lp = t.ps.features.last().unwrap().id;
    t.set(lp, "fullFeaturePattern", b(true));
    let res = t.regen();
    assert!(res.display.features.last().unwrap().error_mesh.is_none());
}

/// Live L56/L57: two stacked parts and a point on the top one. A hole with
/// no Merge scope fails "No target parts selected for hole." on the scope
/// and reports the parts it reaches (the dialog's default): Blind 5 the top
/// part, 15 both. Holes saved before that rule cut every part.
#[test]
fn hole_merge_scope_as_live() {
    let mut t = T::new();
    let (_, a) = t.block(0.0, 0.0, 0.04, 0.02, "10 mm");
    let top = t.add("cPlane", &[("entities", q(vec![Reference::Plane(DatumPlane::Top)])), ("cplaneType", en("OFFSET")), ("offset", e("10 mm"))]);
    let s = t.sketch(Reference::FeaturePlane(top), rect(0.0, 0.0, 0.04, 0.02));
    let r = t.regions(s)[0].clone();
    let bx = t.extrude(r, "10 mm", "NEW");
    let top2 = t.add("cPlane", &[("entities", q(vec![Reference::Plane(DatumPlane::Top)])), ("cplaneType", en("OFFSET")), ("offset", e("20 mm"))]);
    let mut sk = noshape_sketch::Sketch::new();
    let p = sk.add_point(DVec2::new(0.02, 0.01));
    let ps = t.sketch(Reference::FeaturePlane(top2), sk);
    let h = t.add("hole", &[("locations", q(vec![Reference::SketchEntity { sketch: ps, entity: p.0 }])), ("unitsSystem", en("METRIC")), ("scope", q(vec![])), ("holeDiameterV3", e("5 mm")), ("holeDepthV3", e("5 mm"))]);
    let reached = |t: &mut T| {
        let r = t.regen();
        let st = r.display.features.last().unwrap().clone();
        assert_eq!(st.message.as_deref(), Some("No target parts selected for hole."));
        assert_eq!(st.faulty, vec!["scope".to_string()]);
        st.touched.iter().map(|b| b.feature).collect::<Vec<_>>()
    };
    assert_eq!(reached(&mut t), vec![bx]);
    t.set(h, "holeDepthV3", e("15 mm"));
    let mut both = reached(&mut t);
    both.sort();
    let mut want = vec![a, bx];
    want.sort();
    assert_eq!(both, want);
    // With the scope the dialog fills in, the hole cuts both parts.
    t.set(h, "scope", q(vec![Reference::Body(BodyId { feature: a, index: 0 }), Reference::Body(BodyId { feature: bx, index: 0 })]));
    let res = t.ok();
    assert!(res.display.features.last().unwrap().touched.len() == 2);
    // A hole saved before the rule (no scope) is upgraded on load and cuts as before.
    t.set(h, "scope", q(vec![]));
    t.ps.scope_rule = noshape_model::partstudio::ScopeRule(3);
    t.ps.upgrade_saved_params();
    assert!(t.ps.features.iter().find(|f| f.id == h).unwrap().params.bool("legacyAllParts"));
    t.ok();
}
