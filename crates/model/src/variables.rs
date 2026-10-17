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
