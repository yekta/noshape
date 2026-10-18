//! Variable Studios: document-level variable tables usable from any Part
//! Studio or Assembly that references them.

use noshape_units::{Dim, ExprValue};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VariableDef {
    pub name: String,
    pub expr: ExprValue,
    /// Expected dimension; None = any.
    pub kind: Option<VarKind>,
    #[serde(default)]
    pub description: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum VarKind {
    Length,
    Angle,
    Number,
    Any,
}

impl VarKind {
    pub fn dim(self) -> Option<Dim> {
        match self {
            VarKind::Length => Some(Dim::LENGTH),
            VarKind::Angle => Some(Dim::ANGLE),
            VarKind::Number => Some(Dim::UNITLESS),
            VarKind::Any => None,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct VariableStudio {
    pub variables: Vec<VariableDef>,
}

impl VariableStudio {
    /// Evaluate in dependency order (Variable Studios allow any order).
    pub fn evaluate(&self, units: &noshape_units::UnitPrefs) -> noshape_units::TableEvaluation {
        let table = noshape_units::VariableTable {
            order: noshape_units::EvalOrder::Dependency,
            entries: self
                .variables
                .iter()
                .map(|v| noshape_units::VariableDef { name: v.name.clone(), expr: v.expr.text.clone(), kind: v.kind.and_then(|k| k.dim()), description: v.description.clone() })
                .collect(),
        };
        table.evaluate(&noshape_units::EmptyScope, units)
    }
}

/// Names of Part Studio variables that are not yet defined where feature
/// `index` evaluates: assigned only by (unsuppressed) Variable features at or
/// after `index`. Onshape offers only variables "that exist in the current
/// context" in numeric fields; callers remove these from the final values.
pub fn variables_defined_later(studio: &crate::partstudio::PartStudio, index: usize) -> std::collections::BTreeSet<String> {
    let name_of = |f: &crate::partstudio::Feature| (f.type_id == "assignVariable" && !f.suppressed).then(|| f.params.text("name").trim().to_string()).filter(|n| !n.is_empty());
    let before: std::collections::BTreeSet<String> = studio.features.iter().take(index).filter_map(name_of).collect();
    studio.features.iter().skip(index).filter_map(name_of).filter(|n| !before.contains(n)).collect()
}

/// Variables of every Variable Studio in the document (later tabs win).
pub fn document_variables(doc: &crate::Document) -> std::collections::BTreeMap<String, noshape_units::Quantity> {
    let mut out = std::collections::BTreeMap::new();
    for t in doc.ordered_tabs() {
        if let crate::TabContent::VariableStudio(vs) = &t.content {
            for (k, v) in vs.evaluate(&doc.settings.units).values {
                out.insert(k, v);
            }
        }
    }
    out
}

#[cfg(test)]
mod scope_tests {
    use super::variables_defined_later;
    use crate::params::{ParamValue, ParamValues};
    use crate::partstudio::{Feature, PartStudio};
    use crate::reference::FeatureId;

    fn var(id: u64, name: &str, suppressed: bool) -> Feature {
        let mut p = ParamValues::default();
        p.set("name", ParamValue::Text(name.into()));
        Feature { id: FeatureId(id), type_id: "assignVariable".into(), name: format!("Variable {id}"), suppressed, params: p, folder: None, custom_name: false, hidden: false, suppression_expr: None }
    }

    #[test]
    fn only_variables_defined_before_a_feature_are_in_scope() {
        let mut ps = PartStudio::new();
        let ext = Feature { id: FeatureId(9), type_id: "extrude".into(), name: "Extrude 1".into(), suppressed: false, params: ParamValues::default(), folder: None, custom_name: false, hidden: false, suppression_expr: None };
        ps.features = vec![var(1, "a", false), var(2, "b", true), ext, var(3, "width", false), var(4, "a", false), var(5, "b", false)];
        // At the extrude (index 2): `a` exists, `width` and `b` (suppressed before) do not.
        let later = variables_defined_later(&ps, 2);
        assert_eq!(later.into_iter().collect::<Vec<_>>(), ["b", "width"]);
        // At the end everything is defined.
        assert!(variables_defined_later(&ps, ps.features.len()).is_empty());
        // Editing the first Variable: nothing is defined yet.
        assert_eq!(variables_defined_later(&ps, 0).len(), 3);
    }
}
