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
