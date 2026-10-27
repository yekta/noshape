//! The demo Feature Studio (its source is the core's `demo::DEMO_FEATURE_STUDIO`), compiled into a document.

use crate::configuration::ConfigValues;
use noshape_model_core::demo::DEMO_FEATURE_STUDIO;
use crate::document::{Document, TabContent};
use crate::params::ParamValue;
use crate::partstudio::PartStudio;
use crate::reference::Reference;
use crate::regen::{RegenOptions, Regenerator};
use noshape_units::UnitPrefs;

/// Add the demo Feature Studio and a "Flange" Part Studio using its
/// features (Flange, then Bolt circle on the hub's top face).
pub fn add_demo_feature_studio(doc: &mut Document) {
    let fs = doc.add_tab("Custom features", TabContent::FeatureStudio(crate::custom::FeatureStudio::new(DEMO_FEATURE_STUDIO)));
    let scripts = crate::custom::DocumentScripts::from_document(doc);
    let units = UnitPrefs::default();
    let mut ps = PartStudio::new();
    let Some(flange) = ps.new_feature(&crate::custom::type_id(fs, "flange"), &units) else { return };
    ps.features.push(flange);
    let opts = RegenOptions { custom_features: Some(scripts), ..Default::default() };
    let r = Regenerator::new().regenerate(&ps, &ConfigValues::default(), &units, &opts);
    // Hub top: the highest planar face.
    let top = r.context.bodies.first().and_then(|b| {
        (0..b.faces.len())
            .filter_map(|i| {
                let f = b.shape.sub_shape(noshape_kernel::ShapeType::Face, i).ok()?;
                f.face_plane().ok()??;
                Some((i, f.centroid().ok()?.z))
            })
            .max_by(|a, c| a.1.total_cmp(&c.1))
            .map(|(i, _)| Reference::Face((*b.faces[i]).clone()))
    });
    if let (Some(top), Some(mut bc)) = (top, ps.new_feature(&crate::custom::type_id(fs, "boltCircle"), &units)) {
        bc.params.set("face", ParamValue::Query(vec![top]));
        ps.features.push(bc);
    }
    doc.add_tab("Flange", TabContent::PartStudio(ps));
}
