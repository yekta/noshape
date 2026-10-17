//! Data stored by a Sketch feature (the sketch itself plus expression-driven
//! dimensions and links to external 3D geometry).

use crate::reference::Reference;
use noshape_units::ExprValue;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SketchData {
    pub sketch: noshape_sketch::Sketch,
    /// Expression text of driving dimensions, keyed by constraint id.
    #[serde(default)]
    pub dimension_exprs: BTreeMap<u32, ExprValue>,
    /// Sketch entities that are projections of 3D geometry ("Use" / convert),
    /// keyed by entity id; re-projected on every regeneration.
    #[serde(default)]
    pub external_refs: BTreeMap<u32, Reference>,
    /// Sketch text and image entities are stored outside the solver sketch.
    #[serde(default)]
    pub texts: Vec<SketchText>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SketchText {
    pub id: u32,
    pub text: String,
    pub font: String,
    pub bold: bool,
    pub italic: bool,
    /// Corner point entity ids (lower-left, upper-right) in the solver sketch.
    pub corners: [u32; 2],
    pub construction: bool,
}

impl PartialEq for SketchData {
    fn eq(&self, other: &Self) -> bool {
        // The solver sketch has no structural equality; compare serialized forms.
        self.dimension_exprs == other.dimension_exprs
            && self.external_refs == other.external_refs
            && self.texts == other.texts
            && serde_json::to_value(&self.sketch).ok() == serde_json::to_value(&other.sketch).ok()
    }
}
