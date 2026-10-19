//! Fabrication sample content: a sheet metal U channel and a welded frame.

use crate::params::ParamValue;
use crate::partstudio::PartStudio;
use crate::reference::{DatumPlane, FeatureId, Reference};
use crate::sketch_feature::SketchData;
use glam::DVec2;
use noshape_units::{ExprValue, UnitPrefs};

fn polyline(sk: &mut noshape_sketch::Sketch, pts: &[(f64, f64)], closed: bool) -> Vec<u32> {
    let ids: Vec<_> = pts.iter().map(|&(x, y)| sk.add_point(DVec2::new(x * 1e-3, y * 1e-3))).collect();
    let n = if closed { ids.len() } else { ids.len() - 1 };
    (0..n).filter_map(|i| sk.add_line_between(ids[i], ids[(i + 1) % ids.len()]).ok().map(|c| c.0)).collect()
}

fn entities(sketch: FeatureId, ids: &[u32]) -> ParamValue {
    ParamValue::Query(ids.iter().map(|&e| Reference::SketchEntity { sketch, entity: e }).collect())
}

fn sketch(ps: &mut PartStudio, units: &UnitPrefs, plane: DatumPlane, sk: noshape_sketch::Sketch) -> FeatureId {
    let mut f = ps.new_feature("sketch", units).expect("sketch");
    f.params.set("sketchPlane", ParamValue::Query(vec![Reference::Plane(plane)]));
    f.params.set("sketch", ParamValue::Sketch(Box::new(SketchData { sketch: sk, ..Default::default() })));
    let id = f.id;
    ps.features.push(f);
    id
}

/// A sheet metal U channel (T 2 mm, R 3 mm) and a mitered SHS frame.
pub fn fabrication_studio() -> PartStudio {
    let units = UnitPrefs::default();
    let mut ps = PartStudio::new();
    let mut sk = noshape_sketch::Sketch::new();
    let ids = polyline(&mut sk, &[(0.0, 30.0), (0.0, 0.0), (50.0, 0.0), (50.0, 30.0)], false);
    let sid = sketch(&mut ps, &units, DatumPlane::Front, sk);
    if let Some(mut f) = ps.new_feature("sheetMetalStart", &units) {
        f.params.set("process", ParamValue::Enum("EXTRUDE".into()));
        f.params.set("sketchCurves", entities(sid, &ids));
        f.params.set("depth", ParamValue::Expr(ExprValue::new("80 mm")));
        f.params.set("thickness", ParamValue::Expr(ExprValue::new("2 mm")));
        f.params.set("radius", ParamValue::Expr(ExprValue::new("3 mm")));
        f.params.set("kFactor", ParamValue::Expr(ExprValue::new("0.45")));
        ps.features.push(f);
    }
    let mut sk = noshape_sketch::Sketch::new();
    let ids = polyline(&mut sk, &[(100.0, 0.0), (300.0, 0.0), (300.0, 100.0), (100.0, 100.0)], true);
    let sid = sketch(&mut ps, &units, DatumPlane::Top, sk);
    if let Some(mut f) = ps.new_feature("frame", &units) {
        f.params.set("profileSketch", ParamValue::Text("SHS 40x40x3".into()));
        f.params.set("selections", entities(sid, &ids));
        f.params.set("defaultCornerType", ParamValue::Enum("MITER".into()));
        ps.features.push(f);
    }
    ps
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::configuration::ConfigValues;
    use crate::display::FeatureStatus;
    use crate::regen::{RegenOptions, Regenerator};

    #[test]
    fn fabrication_sample_regenerates() {
        let ps = fabrication_studio();
        let r = Regenerator::new().regenerate(&ps, &ConfigValues::default(), &UnitPrefs::default(), &RegenOptions::default());
        for s in &r.display.features {
            assert!(matches!(s.status, FeatureStatus::Ok | FeatureStatus::Warning), "{:?}", s.message);
        }
        assert_eq!(r.context.sheet_metal.len(), 1);
        assert_eq!(r.context.frame_members.len(), 4);
    }
}

#[cfg(test)]
mod clip {
    /// Writes the sample as a feature clipboard payload (paste with Ctrl+V):
    /// `cargo test -p noshape-model write_fab_clip -- --ignored`.
    #[test]
    #[ignore]
    fn write_fab_clip() {
        let ps = super::fabrication_studio();
        let v = serde_json::json!({ "noshape_features": ps.features });
        std::fs::write(std::env::temp_dir().join("noshape-fab-clip.json"), v.to_string()).unwrap();
    }
}

/// A partial revolve (120°) of a rectangle about a sketch line.
pub fn revolve_studio() -> PartStudio {
    use crate::configuration::ConfigValues;
    use crate::regen::{RegenOptions, Regenerator};
    let units = UnitPrefs::default();
    let mut ps = PartStudio::new();
    let mut sk = noshape_sketch::Sketch::new();
    polyline(&mut sk, &[(20.0, 0.0), (40.0, 0.0), (40.0, 30.0), (20.0, 30.0)], true);
    let axis = polyline(&mut sk, &[(0.0, -10.0), (0.0, 40.0)], false)[0];
    let sid = sketch(&mut ps, &units, DatumPlane::Front, sk);
    let r = Regenerator::new().regenerate(&ps, &ConfigValues::default(), &units, &RegenOptions::default());
    let Some(key) = r.display.sketches.first().and_then(|s| s.regions.first()).map(|g| g.key.clone()) else { return ps };
    if let Some(mut f) = ps.new_feature("revolve", &units) {
        f.params.set("entities", ParamValue::Query(vec![Reference::SketchRegion { sketch: sid, key }]));
        f.params.set("axis", ParamValue::Query(vec![Reference::SketchEntity { sketch: sid, entity: axis }]));
        f.params.set("fullRevolve", ParamValue::Bool(false));
        f.params.set("angle", ParamValue::Expr(ExprValue::new("120 deg")));
        ps.features.push(f);
    }
    ps
}

#[cfg(test)]
mod clip_revolve {
    #[test]
    #[ignore]
    fn write_revolve_clip() {
        let ps = super::revolve_studio();
        assert_eq!(ps.features.len(), 2);
        let v = serde_json::json!({ "noshape_features": ps.features });
        std::fs::write(std::env::temp_dir().join("noshape-revolve-clip.json"), v.to_string()).unwrap();
    }
}

/// A document whose Part Studio is [`fabrication_studio`] (`--demo-fab`).
pub fn fabrication_document() -> crate::document::Document {
    let mut doc = crate::document::Document::new();
    if let Some(t) = doc.tabs.iter_mut().find(|t| matches!(t.content, crate::document::TabContent::PartStudio(_))) {
        t.content = crate::document::TabContent::PartStudio(fabrication_studio());
    }
    doc
}
