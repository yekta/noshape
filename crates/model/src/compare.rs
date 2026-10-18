//! Compare two document states (Onshape's "Compare" in the version graph):
//! which tabs were added, removed or renamed, and for each changed tab what
//! changed — features, parameters, suppression, order, configurations,
//! assembly instances and mates, variables.

use crate::document::{Document, TabContent};
use crate::params::ParamValue;
use crate::partstudio::{Feature, PartStudio};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq)]
pub enum ChangeKind {
    Added,
    Removed,
    Modified,
}

/// One line of a comparison.
#[derive(Clone, Debug, PartialEq)]
pub struct Change {
    pub kind: ChangeKind,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TabDiff {
    /// Tab name in the newer state (older if removed).
    pub tab: String,
    pub kind_name: &'static str,
    pub status: ChangeKind,
    pub changes: Vec<Change>,
}

fn change(kind: ChangeKind, text: impl Into<String>) -> Change {
    Change { kind, text: text.into() }
}

/// Differences going from `old` to `new`. Unchanged tabs are omitted.
pub fn compare_documents(old: &Document, new: &Document) -> Vec<TabDiff> {
    let mut out = Vec::new();
    for t in &new.tabs {
        match old.tabs.iter().find(|o| o.id == t.id) {
            None => out.push(TabDiff { tab: t.name.clone(), kind_name: t.content.kind_name(), status: ChangeKind::Added, changes: vec![] }),
            Some(o) => {
                let mut changes = Vec::new();
                if o.name != t.name {
                    changes.push(change(ChangeKind::Modified, format!("Renamed from \"{}\"", o.name)));
                }
                changes.extend(compare_content(&o.content, &t.content));
                if !changes.is_empty() {
                    out.push(TabDiff { tab: t.name.clone(), kind_name: t.content.kind_name(), status: ChangeKind::Modified, changes });
                }
            }
        }
    }
    for o in &old.tabs {
        if !new.tabs.iter().any(|t| t.id == o.id) {
            out.push(TabDiff { tab: o.name.clone(), kind_name: o.content.kind_name(), status: ChangeKind::Removed, changes: vec![] });
        }
    }
    if old.settings.units != new.settings.units {
        out.push(TabDiff { tab: "Document".into(), kind_name: "Settings", status: ChangeKind::Modified, changes: vec![change(ChangeKind::Modified, "Units changed")] });
    }
    out
}

fn compare_content(old: &TabContent, new: &TabContent) -> Vec<Change> {
    match (old, new) {
        (TabContent::PartStudio(a), TabContent::PartStudio(b)) => compare_studios(a, b),
        _ => {
            // Generic structural comparison by serialized form: named lists
            // (instances, mates, variables, sheets...) are compared by id/name.
            let (Ok(a), Ok(b)) = (serde_json::to_value(old), serde_json::to_value(new)) else { return vec![] };
            if a == b {
                return vec![];
            }
            let mut out = Vec::new();
            compare_values("", inner(&a), inner(&b), &mut out);
            if out.is_empty() {
                out.push(change(ChangeKind::Modified, "Content changed"));
            }
            out
        }
    }
}

/// Strip the enum tag (`{"Assembly": {...}}` → `{...}`).
fn inner(v: &Value) -> &Value {
    match v {
        Value::Object(m) if m.len() == 1 => m.values().next().unwrap_or(v),
        _ => v,
    }
}

fn item_key(v: &Value) -> Option<String> {
    let o = v.as_object()?;
    o.get("id").map(|i| i.to_string()).or_else(|| o.get("name").map(|n| n.to_string()))
}

fn item_label(v: &Value, list: &str) -> String {
    let singular = list.trim_end_matches('s').replace('_', " ");
    match v.get("name").and_then(|n| n.as_str()) {
        Some(n) if !n.is_empty() => format!("{singular} \"{n}\""),
        _ => singular,
    }
}

fn compare_values(path: &str, a: &Value, b: &Value, out: &mut Vec<Change>) {
    let (Some(ao), Some(bo)) = (a.as_object(), b.as_object()) else {
        if a != b {
            out.push(change(ChangeKind::Modified, if path.is_empty() { "Content changed".to_string() } else { format!("{} changed", path.replace('_', " ")) }));
        }
        return;
    };
    for (k, bv) in bo {
        let Some(av) = ao.get(k) else { continue };
        if av == bv {
            continue;
        }
        match (av.as_array(), bv.as_array()) {
            (Some(aa), Some(ba)) if aa.iter().chain(ba).all(|x| item_key(x).is_some()) => {
                for x in ba {
                    match aa.iter().find(|y| item_key(y) == item_key(x)) {
                        None => out.push(change(ChangeKind::Added, format!("Added {}", item_label(x, k)))),
                        Some(y) if y != x => out.push(change(ChangeKind::Modified, format!("Modified {}", item_label(x, k)))),
                        _ => {}
                    }
                }
                for y in aa {
                    if !ba.iter().any(|x| item_key(x) == item_key(y)) {
                        out.push(change(ChangeKind::Removed, format!("Removed {}", item_label(y, k))));
                    }
                }
            }
            _ => out.push(change(ChangeKind::Modified, format!("{} changed", k.replace('_', " ")))),
        }
    }
}

fn compare_studios(a: &PartStudio, b: &PartStudio) -> Vec<Change> {
    let mut out = Vec::new();
    for f in &b.features {
        match a.feature(f.id) {
            None => out.push(change(ChangeKind::Added, format!("Added feature \"{}\"", f.name))),
            Some(o) => {
                let details = feature_changes(o, f);
                if !details.is_empty() {
                    out.push(change(ChangeKind::Modified, format!("\"{}\": {}", f.name, details.join(", "))));
                }
            }
        }
    }
    for o in &a.features {
        if b.feature(o.id).is_none() {
            out.push(change(ChangeKind::Removed, format!("Deleted feature \"{}\"", o.name)));
        }
    }
    // Reordering among features present in both states.
    let common_a: Vec<_> = a.features.iter().filter(|f| b.feature(f.id).is_some()).map(|f| f.id).collect();
    let common_b: Vec<_> = b.features.iter().filter(|f| a.feature(f.id).is_some()).map(|f| f.id).collect();
    if common_a != common_b {
        out.push(change(ChangeKind::Modified, "Features reordered"));
    }
    // A bar at the end of the list counts as "no rollback" (adding a
    // feature changes the end index without moving the bar).
    let bar = |s: &crate::partstudio::PartStudio| Some(s.rollback_index()).filter(|&i| i < s.features.len());
    if bar(a) != bar(b) {
        out.push(change(ChangeKind::Modified, "Rollback bar moved"));
    }
    if a.configuration != b.configuration {
        out.push(change(ChangeKind::Modified, "Configuration changed"));
    }
    if a.part_properties != b.part_properties {
        out.push(change(ChangeKind::Modified, "Part properties changed"));
    }
    if a.folders != b.folders {
        out.push(change(ChangeKind::Modified, "Folders changed"));
    }
    out
}

fn feature_changes(a: &Feature, b: &Feature) -> Vec<String> {
    let mut d = Vec::new();
    if a.name != b.name {
        d.push(format!("renamed from \"{}\"", a.name));
    }
    if a.suppressed != b.suppressed {
        d.push(if b.suppressed { "suppressed".into() } else { "unsuppressed".into() });
    }
    let label = |id: &str| -> String {
        b.feature_type().and_then(|t| t.params.iter().find(|s| s.id == id)).map(|s| s.label.to_string()).unwrap_or_else(|| id.to_string())
    };
    for (id, bv) in &b.params.0 {
        let av = a.params.get(id);
        if av == Some(bv) {
            continue;
        }
        let text = match (av, bv) {
            (Some(ParamValue::Expr(x)), ParamValue::Expr(y)) if x.text != y.text => format!("{} {} → {}", label(id), x.text, y.text),
            (Some(ParamValue::Expr(_)), ParamValue::Expr(_)) => continue,
            (Some(ParamValue::Enum(x)), ParamValue::Enum(y)) => format!("{} {} → {}", label(id), x.to_lowercase(), y.to_lowercase()),
            (Some(ParamValue::Bool(_)), ParamValue::Bool(y)) => format!("{} {}", label(id), if *y { "on" } else { "off" }),
            (Some(ParamValue::Query(_)), ParamValue::Query(_)) => format!("{} selection changed", label(id)),
            (Some(ParamValue::Sketch(_)), ParamValue::Sketch(_)) => "sketch edited".into(),
            _ => format!("{} changed", label(id)),
        };
        d.push(text);
    }
    d
}

#[cfg(test)]
mod tests {
    use super::*;
    use noshape_units::{ExprValue, UnitPrefs};

    #[test]
    fn studio_feature_diff() {
        let mut old = crate::demo::demo_document();
        let mut new = old.clone();
        let units = UnitPrefs::default();
        let tab = new.tabs.iter().position(|t| matches!(t.content, TabContent::PartStudio(_))).unwrap();
        let TabContent::PartStudio(ps) = &mut new.tabs[tab].content else { unreachable!() };
        let ex = ps.features.iter_mut().find(|f| f.type_id == "extrude").unwrap();
        ex.params.set("depth", ParamValue::Expr(ExprValue::new("30 mm")));
        ex.suppressed = true;
        let f = ps.new_feature("chamfer", &units).unwrap();
        ps.features.push(f);
        old.tabs.retain(|t| t.name != "Removed");
        let diff = compare_documents(&old, &new);
        assert_eq!(diff.len(), 1, "{diff:?}");
        let texts: Vec<&str> = diff[0].changes.iter().map(|c| c.text.as_str()).collect();
        assert!(texts.iter().any(|t| t.contains("→ 30 mm") && t.contains("suppressed")), "{texts:?}");
        assert!(texts.iter().any(|t| t.starts_with("Added feature \"Chamfer")), "{texts:?}");
        assert!(compare_documents(&new, &new).is_empty());
        assert!(!texts.iter().any(|t| t.contains("Rollback")), "bar at the end did not move: {texts:?}");
    }

    #[test]
    fn rollback_moves_are_reported() {
        let old = crate::demo::demo_document();
        let mut new = old.clone();
        let tab = new.tabs.iter().position(|t| matches!(t.content, TabContent::PartStudio(_))).unwrap();
        let TabContent::PartStudio(ps) = &mut new.tabs[tab].content else { unreachable!() };
        ps.rollback = Some(1);
        let diff = compare_documents(&old, &new);
        assert!(diff.iter().flat_map(|d| &d.changes).any(|c| c.text == "Rollback bar moved"), "{diff:?}");
        // Explicitly at the end is the same as no rollback.
        let TabContent::PartStudio(ps) = &mut new.tabs[tab].content else { unreachable!() };
        ps.rollback = Some(ps.features.len());
        assert!(compare_documents(&old, &new).is_empty());
    }
}
