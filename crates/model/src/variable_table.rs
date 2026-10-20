//! The Part Studio's own Variable table (Help PartStudio__variable_table):
//! its rows are the studio's Variable features with valid names; the table
//! creates, renames and edits them. The inline "Create variable" popup of a
//! numeric field creates them too.

use crate::configuration::FS_KEYWORDS;
use crate::params::ParamValue;
use crate::partstudio::{Feature, PartStudio};
use crate::reference::FeatureId;
use noshape_units::{ExprValue, UnitPrefs};

/// One row of the table.
#[derive(Clone, Debug, PartialEq)]
pub struct VarRow {
    pub feature: FeatureId,
    pub name: String,
    /// `LENGTH`, `ANGLE`, `NUMBER` or `ANY`.
    pub kind: String,
    /// The value expression as typed.
    pub value: String,
    pub description: String,
}

/// Why a name was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NameError {
    /// Not a FeatureScript identifier (Help: a letter or underscore, then
    /// letters, digits or underscores, no spaces).
    Invalid,
    /// Another variable of the table has it ("the variable is not
    /// overwritten").
    Duplicate,
}

/// A FeatureScript identifier that isn't a reserved word.
pub fn is_identifier(s: &str) -> bool {
    let mut c = s.chars();
    c.next().is_some_and(|f| f.is_ascii_alphabetic() || f == '_') && c.all(|x| x.is_ascii_alphanumeric() || x == '_') && !FS_KEYWORDS.contains(&s)
}

/// The value parameter of a variable type.
pub fn value_param(kind: &str) -> &'static str {
    match kind {
        "ANGLE" => "angleValue",
        "NUMBER" => "numberValue",
        "ANY" => "anyValue",
        _ => "lengthValue",
    }
}

/// A new variable's value (Variable Studio, live document/10–16: Length,
/// 0 in the document unit).
pub fn default_value(kind: &str, units: &UnitPrefs) -> String {
    match kind {
        "LENGTH" => format!("0 {}", units.length.symbol()),
        "ANGLE" => format!("0 {}", units.angle.symbol()),
        _ => "0".into(),
    }
}

fn kind_of(f: &Feature) -> String {
    match f.params.get("variableType") {
        Some(ParamValue::Enum(k)) => k.clone(),
        _ => "LENGTH".into(),
    }
}

fn value_text(f: &Feature, kind: &str) -> String {
    match f.params.get(value_param(kind)) {
        Some(ParamValue::Expr(e)) => e.text.clone(),
        Some(ParamValue::Text(t)) => t.clone(),
        _ => String::new(),
    }
}

/// The table's rows, in feature order (Help: Variable features whose name
/// isn't a FeatureScript identifier don't appear).
pub fn rows(ps: &PartStudio) -> Vec<VarRow> {
    ps.features
        .iter()
        .filter(|f| f.type_id == "assignVariable")
        .filter_map(|f| {
            let name = f.params.text("name").trim().trim_start_matches('#').to_string();
            is_identifier(&name).then(|| {
                let kind = kind_of(f);
                VarRow { feature: f.id, value: value_text(f, &kind), description: f.params.text("description").to_string(), name, kind }
            })
        })
        .collect()
}

fn taken(ps: &PartStudio, name: &str, except: Option<FeatureId>) -> bool {
    rows(ps).iter().any(|r| r.name == name && Some(r.feature) != except)
}

/// Create a Variable feature `#name = value` at feature index `at` (the
/// inline popup: before the feature being edited) or, with `None`, at the
/// rollback bar (the table).
pub fn create(ps: &mut PartStudio, name: &str, kind: &str, value: &str, description: &str, at: Option<usize>, units: &UnitPrefs) -> Result<FeatureId, NameError> {
    let name = name.trim().trim_start_matches('#');
    if !is_identifier(name) {
        return Err(NameError::Invalid);
    }
    if taken(ps, name, None) {
        return Err(NameError::Duplicate);
    }
    let Some(mut f) = ps.new_feature("assignVariable", units) else { return Err(NameError::Invalid) };
    f.params.set("mode", ParamValue::Enum("ASSIGNED".into()));
    f.params.set("variableType", ParamValue::Enum(kind.to_string()));
    f.params.set("name", ParamValue::Text(name.to_string()));
    set_value_param(&mut f, kind, value);
    f.params.set("description", ParamValue::Text(description.to_string()));
    let id = f.id;
    match at {
        None => {
            ps.insert_at_rollback(f);
        }
        Some(i) => {
            let i = i.min(ps.features.len());
            ps.features.insert(i, f);
            if let Some(r) = ps.rollback
                && i < r
            {
                ps.rollback = Some(r + 1);
            }
        }
    }
    Ok(id)
}

fn set_value_param(f: &mut Feature, kind: &str, value: &str) {
    let p = value_param(kind);
    let v = if kind == "ANY" { ParamValue::Text(value.to_string()) } else { ParamValue::Expr(ExprValue::new(value)) };
    f.params.set(p, v);
}

/// `#old` → `#new` in an expression (whole names only).
pub fn rename_in_expr(text: &str, old: &str, new: &str) -> String {
    let needle = format!("#{old}");
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(i) = rest.find(&needle) {
        let after = &rest[i + needle.len()..];
        let whole = !after.starts_with(|c: char| c.is_ascii_alphanumeric() || c == '_');
        out.push_str(&rest[..i]);
        if whole {
            out.push('#');
            out.push_str(new);
        } else {
            out.push_str(&needle);
        }
        rest = after;
    }
    out.push_str(rest);
    out
}

/// Rename a variable; every feature (and configured value) that refers to
/// it follows (Help: "all Features that refer to the variable will be
/// updated").
pub fn rename(ps: &mut PartStudio, feature: FeatureId, new: &str) -> Result<(), NameError> {
    let new = new.trim().trim_start_matches('#');
    if !is_identifier(new) {
        return Err(NameError::Invalid);
    }
    if taken(ps, new, Some(feature)) {
        return Err(NameError::Duplicate);
    }
    let Some(f) = ps.feature(feature) else { return Ok(()) };
    let old = f.params.text("name").trim().trim_start_matches('#').to_string();
    let fix = |v: &mut ParamValue| {
        if let ParamValue::Expr(e) = v
            && e.text.contains(&format!("#{old}"))
        {
            *e = ExprValue::new(rename_in_expr(&e.text, &old, new));
        }
    };
    for f in &mut ps.features {
        for v in f.params.0.values_mut() {
            fix(v);
        }
        if let Some(e) = &mut f.suppression_expr {
            *e = rename_in_expr(e, &old, new);
        }
    }
    for c in &mut ps.configuration.configured {
        for v in c.values.values_mut() {
            fix(v);
        }
    }
    if let Some(f) = ps.features.iter_mut().find(|f| f.id == feature) {
        f.params.set("name", ParamValue::Text(new.to_string()));
    }
    Ok(())
}

/// Variable type ▾: the new type keeps the value text when it was already
/// set for that type, else gets the type's default.
pub fn set_kind(ps: &mut PartStudio, feature: FeatureId, kind: &str, units: &UnitPrefs) {
    if let Some(f) = ps.features.iter_mut().find(|f| f.id == feature) {
        f.params.set("variableType", ParamValue::Enum(kind.to_string()));
        if value_text(f, kind).is_empty() {
            set_value_param(f, kind, &default_value(kind, units));
        }
    }
}

/// The Value cell.
pub fn set_value(ps: &mut PartStudio, feature: FeatureId, value: &str) {
    if let Some(f) = ps.features.iter_mut().find(|f| f.id == feature) {
        let kind = kind_of(f);
        set_value_param(f, &kind, value);
    }
}

/// The Description cell.
pub fn set_description(ps: &mut PartStudio, feature: FeatureId, text: &str) {
    if let Some(f) = ps.features.iter_mut().find(|f| f.id == feature) {
        f.params.set("description", ParamValue::Text(text.to_string()));
    }
}

/// Row ⋯ Move up (-1) / down (+1): swap with the neighbouring variable row
/// in the feature list.
pub fn move_row(ps: &mut PartStudio, feature: FeatureId, by: i32) {
    let ids: Vec<FeatureId> = rows(ps).iter().map(|r| r.feature).collect();
    let Some(k) = ids.iter().position(|id| *id == feature) else { return };
    let j = k as i32 + by;
    if j < 0 || j as usize >= ids.len() {
        return;
    }
    let (Some(a), Some(b)) = (ps.index_of(ids[k]), ps.index_of(ids[j as usize])) else { return };
    ps.features.swap(a, b);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn studio() -> PartStudio {
        PartStudio::default()
    }

    fn mm() -> UnitPrefs {
        UnitPrefs::default()
    }

    #[test]
    fn create_from_the_table_and_before_a_feature() {
        let mut ps = studio();
        let a = create(&mut ps, "width", "LENGTH", &default_value("LENGTH", &mm()), "", None, &mm()).unwrap();
        assert_eq!(rows(&ps).len(), 1);
        assert_eq!(rows(&ps)[0].name, "width");
        assert_eq!(rows(&ps)[0].kind, "LENGTH");
        assert_eq!(rows(&ps)[0].value, format!("0 {}", mm().length.symbol()));
        // The inline popup places it before the feature being edited.
        let b = create(&mut ps, "height", "LENGTH", "2 mm", "", Some(0), &mm()).unwrap();
        assert_eq!(ps.features[0].id, b);
        assert_eq!(ps.features[1].id, a);
        assert_eq!(create(&mut ps, "width", "LENGTH", "1 mm", "", None, &mm()), Err(NameError::Duplicate));
        assert_eq!(create(&mut ps, "2wide", "LENGTH", "1 mm", "", None, &mm()), Err(NameError::Invalid));
        assert_eq!(create(&mut ps, "my var", "LENGTH", "1 mm", "", None, &mm()), Err(NameError::Invalid));
        assert_eq!(create(&mut ps, "default", "LENGTH", "1 mm", "", None, &mm()), Err(NameError::Invalid));
    }

    #[test]
    fn rename_updates_references() {
        let mut ps = studio();
        let a = create(&mut ps, "w", "LENGTH", "1 mm", "", None, &mm()).unwrap();
        let b = create(&mut ps, "h", "LENGTH", "#w * 2 + #wide", "", None, &mm()).unwrap();
        rename(&mut ps, a, "width").unwrap();
        let h = ps.feature(b).unwrap();
        assert_eq!(h.params.expr("lengthValue").unwrap().text, "#width * 2 + #wide");
        assert_eq!(rename(&mut ps, a, "h"), Err(NameError::Duplicate));
        set_kind(&mut ps, a, "NUMBER", &mm());
        assert_eq!(rows(&ps)[0].kind, "NUMBER");
        assert_eq!(rows(&ps)[0].value, "0");
        set_value(&mut ps, a, "5");
        assert_eq!(rows(&ps)[0].value, "5");
        move_row(&mut ps, a, 1);
        assert_eq!(rows(&ps)[0].name, "h");
    }
}
