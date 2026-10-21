//! Onshape's commit rule for number / quantity text (live behaviours.md
//! "Input behaviour", "Sketch dimension display"): a plain number takes the
//! workspace unit (`25` → `25 mm`), a unitless expression in a dimensioned
//! field is wrapped (`10+5` → `(10+5) mm`), anything else is kept as typed.
//! Stored this way, a later change of the workspace units never changes
//! the geometry.

use noshape_units::{Dim, UnitPrefs, VariableScope};

/// The unit word typed after a number in a field of dimension `dim`
/// (`mm`, `deg`, `mm^2`), or None for unitless fields.
pub fn unit_word(dim: Dim, prefs: &UnitPrefs) -> Option<String> {
    if dim.is_unitless() {
        return None;
    }
    let part = |sym: &str, e: i8| if e == 1 { sym.to_string() } else { format!("{sym}^{e}") };
    let mut parts = Vec::new();
    if dim.length != 0 {
        parts.push(part(prefs.length.symbol(), dim.length));
    }
    if dim.angle != 0 {
        parts.push(part(prefs.angle.symbol(), dim.angle));
    }
    Some(parts.join("*"))
}

/// A plain number, optionally followed by a unit word: (number text, unit).
/// Onshape's `valueAndUnits` (with `missingLeadingZero` / `extraDecimal`
/// repaired): `-.5`, `5.`, `12.5 mm`, `3in`.
pub fn simple(text: &str) -> Option<(String, Option<String>)> {
    let t = text.trim();
    let split = t.char_indices().find(|(i, c)| !(c.is_ascii_digit() || *c == '.' || (*i == 0 && (*c == '-' || *c == '+')))).map_or(t.len(), |(i, _)| i);
    let (num, rest) = t.split_at(split);
    let unit = rest.trim();
    let digits = num.trim_start_matches(['-', '+']);
    if digits.is_empty() || digits == "." || digits.matches('.').count() > 1 {
        return None;
    }
    if !unit.is_empty() && !unit.chars().all(|c| c.is_ascii_alphabetic() || c == '^' || c.is_ascii_digit() || c == '*') {
        return None;
    }
    let sign = if num.starts_with('-') { "-" } else { "" };
    let mut n = digits.to_string();
    if n.starts_with('.') {
        n.insert(0, '0');
    }
    if n.ends_with('.') {
        n.pop();
    }
    Some((format!("{sign}{n}"), (!unit.is_empty()).then(|| unit.to_string())))
}

/// The text a commit leaves in a field of dimension `dim` (see the module
/// doc). Text that doesn't evaluate is left as typed (the field shows it
/// invalid).
pub fn commit_text(text: &str, dim: Dim, prefs: &UnitPrefs, scope: &dyn VariableScope) -> String {
    // A comma-decimal "1,5" (Preferences › Decimal format) is a plain number
    // too (as `variable_table::with_workspace_unit`).
    let text = noshape_units::delocalize(text);
    let t = text.trim();
    if t.is_empty() {
        return String::new();
    }
    let word = unit_word(dim, prefs);
    if let Some((num, unit)) = simple(t) {
        return match (unit, word) {
            (Some(u), _) => format!("{num} {u}"),
            (None, Some(w)) => format!("{num} {w}"),
            (None, None) => num,
        };
    }
    // An expression: one evaluating to a bare number gets the unit around it.
    match (word, noshape_units::evaluate(t, scope)) {
        (Some(w), Ok(q)) if q.is_unitless() => format!("({t}) {w}"),
        _ => t.to_string(),
    }
}

/// Unit kind of a sketch dimension's typed value.
fn sketch_dimension_dim(k: &noshape_sketch::ConstraintKind) -> Dim {
    use noshape_sketch::ConstraintKind as K;
    match k {
        K::Angle { .. } | K::ArcAngle { .. } => Dim::ANGLE,
        K::Rho { .. } => Dim::UNITLESS,
        _ => Dim::LENGTH,
    }
}

/// Before the workspace units change from `old`: sketch dimensions whose
/// text has no unit (`25`, `10+5`; older documents stored them so) get the
/// unit they were evaluated in, so the geometry stays the same (live adds
/// the unit when a value is committed, so in Onshape it never changes).
/// Returns the tabs that changed.
pub fn pin_sketch_units(doc: &mut crate::document::Document, old: &UnitPrefs) -> Vec<crate::document::TabId> {
    use crate::params::ParamValue;
    use crate::regen::features::sketch::CONFIGURED_DIMENSION_PREFIX;
    let scope = noshape_units::EmptyScope;
    let pin = |text: &mut String, dim: Dim| {
        let pinned = commit_text(text, dim, old, &scope);
        let changed = !pinned.is_empty() && pinned != *text;
        if changed {
            *text = pinned;
        }
        changed
    };
    let mut changed = Vec::new();
    for tab in &mut doc.tabs {
        let mut n = 0;
        let crate::document::TabContent::PartStudio(ps) = &mut tab.content else { continue };
        // Dimension kinds per (sketch, constraint), for the configured values.
        let mut kinds = std::collections::HashMap::new();
        for f in &mut ps.features {
            let Some(ParamValue::Sketch(data)) = f.params.0.get_mut("sketch") else { continue };
            n += pin_sketch_data(data, old);
            for c in data.sketch.constraints() {
                kinds.insert((f.id, c.id.0), sketch_dimension_dim(&c.kind));
            }
        }
        for cp in &mut ps.configuration.configured {
            let Some(cid) = cp.param.strip_prefix(CONFIGURED_DIMENSION_PREFIX).and_then(|c| c.parse::<u32>().ok()) else { continue };
            let Some(&dim) = kinds.get(&(cp.feature, cid)) else { continue };
            for v in cp.values.values_mut() {
                if let ParamValue::Expr(e) = v {
                    let mut t = e.text.clone();
                    if pin(&mut t, dim) {
                        *e = noshape_units::ExprValue::new(t);
                        n += 1;
                    }
                }
            }
        }
        if n > 0 {
            changed.push(tab.id);
        }
    }
    changed
}

/// [`pin_sketch_units`] for one sketch (also an open sketch's draft):
/// returns how many dimension texts got `old`'s unit.
pub fn pin_sketch_data(data: &mut crate::sketch_feature::SketchData, old: &UnitPrefs) -> usize {
    let mut n = 0;
    for (cid, e) in data.dimension_exprs.iter_mut() {
        let Some(c) = data.sketch.constraint(noshape_sketch::ConstraintId(*cid)) else { continue };
        let pinned = commit_text(&e.text, sketch_dimension_dim(&c.kind), old, &noshape_units::EmptyScope);
        if !pinned.is_empty() && pinned != e.text {
            *e = noshape_units::ExprValue::new(pinned);
            n += 1;
        }
    }
    n
}
