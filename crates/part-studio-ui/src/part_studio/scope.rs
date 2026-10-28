//! The variables an expression can use at a place in the feature list, as
//! regeneration layers them (Onshape): the Variable Studio tables shown in
//! this Part Studio's Variable table, then the configuration variables of
//! the active configuration, then the Part Studio's own Variable features
//! defined before that place.

use super::PartStudioView;
use gpui_kit::*;
use noshape_units::Quantity;
use std::collections::BTreeMap;

impl PartStudioView {
    /// Variables visible to the feature at index `at` (`None`: after every
    /// feature).
    pub(super) fn scope_variables(&self, at: Option<usize>, cx: &App) -> BTreeMap<String, Quantity> {
        let doc = &self.doc.read(cx).session.doc;
        let mut vars = noshape_model::variables::element_variables(doc, self.tab);
        let Some(studio) = self.studio(cx) else { return vars };
        vars.extend(studio.configuration.variable_values(&self.config_values, &doc.settings.units));
        // Own variables: only those defined before `at` (a name defined
        // later doesn't hide a table's or configuration's value).
        let mut own = self.display.variables.clone();
        if let Some(i) = at {
            for name in noshape_model::variables::variables_defined_later(studio, i) {
                own.remove(&name);
            }
            // A name defined again later: the value it has at `at`
            // (DIALOG-EXPR-SCOPE: `#w = 5 mm` before Extrude 1, `#w = 8 mm`
            // after it, Extrude 1's Depth reads 5 mm as live).
            for f in studio.features.iter().take(i) {
                if let Some((name, q)) = self.display.features.iter().find(|s| s.id == f.id).and_then(|s| s.variable.clone()) {
                    own.insert(name, q);
                }
            }
        }
        vars.extend(own);
        vars
    }

    /// Variables a dimension of the sketch being edited can use: those
    /// visible to the sketch feature.
    pub(super) fn sketch_variables(&self, cx: &App) -> BTreeMap<String, Quantity> {
        let at = self.sketch.as_ref().and_then(|s| self.studio(cx).and_then(|ps| ps.index_of(s.feature)));
        self.scope_variables(at, cx)
    }
}
