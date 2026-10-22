//! Variable Studio table edits, shared by the app's Variable Studio
//! (`crates/app/src/variable_studio.rs`) and the agent API: the same edits
//! with the same history text. Rows are addressed by their index.

use noshape_model::variables::{VarKind, VariableDef, VariableStudio, VsField};
use noshape_units::{ExprValue, UnitPrefs};

/// The last row's "Name" cell: a Length variable set to 0 in the
/// document's length unit ("Add variable").
pub fn add_variable(s: &mut VariableStudio, name: &str, units: &UnitPrefs) {
    let name = name.trim().trim_start_matches('#').to_string();
    let unit = units.length.symbol();
    s.variables.push(VariableDef { name, expr: ExprValue::new(format!("0 {unit}")), kind: Some(VarKind::Length), description: String::new(), description_expr: false, import: None });
}

/// The Name cell ("Rename variable"): configured cells follow the name.
pub fn rename_variable(s: &mut VariableStudio, i: usize, name: &str) {
    if let Some(v) = s.variables.get_mut(i) {
        let (old, new) = (v.name.clone(), name.trim().trim_start_matches('#').to_string());
        v.name = new.clone();
        s.rename_configured(&old, &new);
    }
}

/// The Value cell ("Edit variable"): a bare number takes the workspace
/// unit of a Length / Angle variable; a configured value sets the shown
/// configuration's row.
pub fn set_value(s: &mut VariableStudio, i: usize, text: &str, units: &UnitPrefs) {
    let Some(v) = s.variables.get(i) else { return };
    let kind = match v.kind {
        Some(VarKind::Length) => "LENGTH",
        Some(VarKind::Angle) => "ANGLE",
        _ => "",
    };
    let text = ExprValue::new(noshape_model::variable_table::with_workspace_unit(kind, text, units)).text;
    let name = v.name.clone();
    if let Some(slot) = s.configured_text_mut(&name, VsField::Value) {
        *slot = text;
    } else if let Some(v) = s.variables.get_mut(i) {
        v.expr = ExprValue::new(text);
    }
}

/// The Description cell ("Edit variable description").
pub fn set_description(s: &mut VariableStudio, i: usize, text: &str) {
    let Some(name) = s.variables.get(i).map(|v| v.name.clone()) else { return };
    if let Some(slot) = s.configured_text_mut(&name, VsField::Description) {
        *slot = text.to_string();
    } else if let Some(v) = s.variables.get_mut(i) {
        v.set_description(text);
    }
}

/// The Type select ("Change variable type").
pub fn set_kind(s: &mut VariableStudio, i: usize, kind: VarKind) {
    if let Some(v) = s.variables.get_mut(i) {
        v.kind = Some(kind);
    }
}

/// "Move variable up" / "Move variable down".
pub fn move_description(down: bool) -> &'static str {
    if down { "Move variable down" } else { "Move variable up" }
}

/// The row menu's Move up / Move down.
pub fn move_variable(s: &mut VariableStudio, i: usize, down: bool) {
    let j = if down { i + 1 } else { i.wrapping_sub(1) };
    if i < s.variables.len() && j < s.variables.len() {
        s.variables.swap(i, j);
    }
}

/// The row menu's Delete ("Delete variable"): its configured cells go too.
pub fn delete_variable(s: &mut VariableStudio, i: usize) {
    if i < s.variables.len() {
        let v = s.variables.remove(i);
        s.configuration.configured.retain(|c| c.variable != v.name);
    }
}

/// The Type select's label.
pub fn kind_label(k: VarKind) -> &'static str {
    match k {
        VarKind::Length => "Length",
        VarKind::Angle => "Angle",
        VarKind::Number => "Number",
        VarKind::Any => "Any",
        VarKind::FromImport => "From import",
    }
}
