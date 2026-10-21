//! Pattern instances that stay parts (Part pattern, New) are rigidly moved
//! copies of one TShape: the display meshes it once (`SharedMesh`), and each
//! copy's world mesh is that mesh placed by the copy's transform, so the
//! renderer can upload it once and draw every copy with one instanced call.

use glam::DVec2;
use noshape_model::configuration::ConfigValues;
use noshape_model::display::SharedMesh;
use noshape_model::params::ParamValue;
use noshape_model::partstudio::PartStudio;
use noshape_model::reference::{BodyId, DatumPlane, FeatureId, Reference};
use noshape_model::regen::{RegenOptions, RegenResult, Regenerator};
use noshape_model::sketch_feature::SketchData;
use noshape_units::{ExprValue, UnitPrefs};
use std::sync::Arc;

fn e(s: &str) -> ParamValue {
    ParamValue::Expr(ExprValue::new(s))
}
fn q(v: Vec<Reference>) -> ParamValue {
    ParamValue::Query(v)
}
fn en(s: &str) -> ParamValue {
    ParamValue::Enum(s.into())
}

fn add(ps: &mut PartStudio, units: &UnitPrefs, ty: &str, params: &[(&str, ParamValue)]) -> FeatureId {
    let mut f = ps.new_feature(ty, units).unwrap();
    for (k, v) in params {
        f.params.set(k, v.clone());
    }
    let id = f.id;
    ps.features.push(f);
    id
}

fn regen(ps: &PartStudio, units: &UnitPrefs) -> RegenResult {
    Regenerator::new().regenerate(ps, &ConfigValues::default(), units, &RegenOptions::default())
}

#[test]
fn part_pattern_copies_share_one_placed_mesh() {
    let units = UnitPrefs::default();
    let mut ps = PartStudio::new();
    // A cylinder (curved faces, so the mesh is more than a box) of radius 3
    // mm, 5 mm high, patterned 4 times 10 mm along x as new parts.
    let mut sk = noshape_sketch::Sketch::new();
    sk.add_circle(DVec2::new(0.0, 0.0), 0.003);
    let s = add(&mut ps, &units, "sketch", &[("sketchPlane", q(vec![Reference::Plane(DatumPlane::Top)])), ("sketch", ParamValue::Sketch(Box::new(SketchData { sketch: sk, ..Default::default() })))]);
    let r = regen(&ps, &units);
    let region = Reference::SketchRegion { sketch: s, key: r.display.sketches.iter().find(|x| x.feature == s).unwrap().regions[0].key.clone() };
    let pin = add(&mut ps, &units, "extrude", &[("entities", q(vec![region])), ("depth", e("5 mm")), ("operationType", en("NEW"))]);
    add(
        &mut ps,
        &units,
        "linearPattern",
        &[
            ("patternType", en("PART")),
            ("entities", q(vec![Reference::Body(BodyId { feature: pin, index: 0 })])),
            ("directionOne", q(vec![Reference::Plane(DatumPlane::Right)])),
            ("distance", e("10 mm")),
            ("instanceCount", e("4")),
            ("operationType", en("NEW")),
        ],
    );
    let r = regen(&ps, &units);
    let d = &r.display;
    assert_eq!(d.bodies.len(), 4, "the seed and three copies are parts");
    let shared: Vec<&SharedMesh> = d.bodies.iter().filter_map(|b| SharedMesh::of(d, b)).collect();
    assert_eq!(shared.len(), 4, "every copy (and the seed) draws from the shared mesh");
    assert!(shared.iter().all(|s| Arc::ptr_eq(&s.mesh, &shared[0].mesh)), "one mesh for all copies");
    let mut xs: Vec<f64> = shared.iter().map(|s| s.transform[3] * 1e3).collect();
    xs.sort_by(f64::total_cmp);
    let x0 = xs[0];
    for (k, x) in xs.iter().enumerate() {
        assert!((x - x0 - 10.0 * k as f64).abs() < 1e-6, "copy {k} placed at x {x} mm");
    }
    for b in &d.bodies {
        let s = SharedMesh::of(d, b).unwrap();
        let placed = s.mesh.transformed(&s.transform);
        assert_eq!(placed.positions.len(), b.mesh.positions.len());
        assert_eq!(placed.face_ranges, b.mesh.face_ranges, "same faces in the same order (pick ids)");
        assert_eq!(placed.edges.len(), b.mesh.edges.len());
        let off = placed.positions.iter().zip(&b.mesh.positions).map(|(p, q)| (0..3).map(|i| (p[i] - q[i]).abs()).fold(0.0f32, f32::max)).fold(0.0f32, f32::max);
        assert!(off < 1e-7, "{}: placed shared mesh differs from the world mesh by {off} m", b.name);
    }
    // A replaced world mesh (e.g. a mirrored preview) is not drawn from the
    // shared one any more.
    let mut d2 = d.clone();
    d2.bodies[1].mesh = Arc::new((*d2.bodies[1].mesh).clone());
    assert!(SharedMesh::of(&d2, &d2.bodies[1]).is_none());
}
