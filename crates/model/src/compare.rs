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

/// One parameter of a feature in a Part Studio compare: its label and the
/// value text on each side (None: the side lacks the parameter).
#[derive(Clone, Debug, PartialEq)]
pub struct ParamCompare {
    pub label: String,
    pub base: Option<String>,
    pub target: Option<String>,
    /// How the feature dialog shows it (the compare panel draws read-only
    /// dialogs side by side, live cp-921).
    pub shape: ParamShape,
}

/// A parameter's control in a feature dialog.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParamShape {
    /// The feature's name (the dialog header).
    Name,
    /// Suppression (no dialog control; a checkbox row here).
    Bool,
    Query,
    Enum,
    Number,
    Text,
    Other,
}

impl ParamCompare {
    pub fn differs(&self) -> bool {
        self.base != self.target
    }
}

/// One feature of a Part Studio compare (Onshape's Compare panel ›
/// Features), matched by id: `status` None = the same on both sides.
#[derive(Clone, Debug, PartialEq)]
pub struct FeatureCompare {
    pub id: crate::reference::FeatureId,
    pub name: String,
    pub type_id: String,
    pub status: Option<ChangeKind>,
    pub params: Vec<ParamCompare>,
}

/// A selection as the dialog's query box names it ("Edge of Extrude 1").
fn reference_label(r: &crate::reference::Reference, studio: &PartStudio) -> String {
    use crate::reference::{DatumPlane, Reference, TopoName};
    let fname = |id: crate::reference::FeatureId| studio.feature(id).map(|f| f.name.clone()).unwrap_or_else(|| "deleted feature".into());
    let creator = |n: &TopoName| match n {
        TopoName::Edge { faces, .. } => faces[1].creator().or_else(|| faces[0].creator()),
        TopoName::Boundary { face, .. } => face.creator(),
        other => other.creator(),
    };
    match r {
        Reference::Origin => "Origin".into(),
        Reference::Plane(DatumPlane::Top) => "Top plane".into(),
        Reference::Plane(DatumPlane::Front) => "Front plane".into(),
        Reference::Plane(DatumPlane::Right) => "Right plane".into(),
        Reference::Face(n) => format!("Face of {}", n.creator().map(fname).unwrap_or_else(|| "part".into())),
        Reference::Edge(n) => format!("Edge of {}", creator(n).map(fname).unwrap_or_else(|| "part".into())),
        Reference::Vertex(_) => "Vertex".into(),
        Reference::Body(_) => "Part".into(),
        Reference::SketchEntity { sketch, .. } | Reference::SketchRegion { sketch, .. } | Reference::Sketch(sketch) => fname(*sketch),
        Reference::FeaturePlane(f) | Reference::Feature(f) | Reference::MateConnector { feature: f, .. } => fname(*f),
        Reference::Implicit { .. } => "Mate connector".into(),
        Reference::QueryVariable(name) => format!("#{name}"),
    }
}

fn shape_of(v: &ParamValue) -> ParamShape {
    match v {
        ParamValue::Bool(_) => ParamShape::Bool,
        ParamValue::Enum(_) => ParamShape::Enum,
        ParamValue::Expr(_) => ParamShape::Number,
        ParamValue::Text(_) => ParamShape::Text,
        ParamValue::Query(_) => ParamShape::Query,
        _ => ParamShape::Other,
    }
}

/// A parameter value as the compare panel shows it.
fn param_text(v: &ParamValue, spec: Option<&crate::params::ParamSpec>, studio: &PartStudio) -> String {
    match v {
        ParamValue::Bool(b) => if *b { "true" } else { "false" }.into(),
        ParamValue::Enum(e) => match spec.map(|s| &s.kind) {
            Some(crate::params::ParamKind::Enum { options, .. }) => options.iter().find(|o| o.0 == e).map(|o| o.1.to_string()).unwrap_or_else(|| e.clone()),
            _ => e.clone(),
        },
        ParamValue::Expr(x) => x.text.clone(),
        ParamValue::Text(t) => t.clone(),
        ParamValue::Query(q) => q.iter().map(|r| reference_label(r, studio)).collect::<Vec<_>>().join(", "),
        ParamValue::Array(a) => format!("{} items", a.len()),
        ParamValue::Sketch(_) => "Sketch".into(),
        ParamValue::Lookup(l) => l.join(" › "),
    }
}

/// The features of two Part Studio states side by side: `base`'s order,
/// features only in `target` after the feature they follow there.
pub fn compare_features(base: &PartStudio, target: &PartStudio) -> Vec<FeatureCompare> {
    let row = |a: Option<&Feature>, b: Option<&Feature>| -> FeatureCompare {
        let f = b.or(a).expect("one side");
        let ty = f.feature_type();
        let mut ids: Vec<&str> = Vec::new();
        for side in [a, b].into_iter().flatten() {
            for id in side.params.0.keys() {
                if !ids.contains(&id.as_str()) {
                    ids.push(id);
                }
            }
        }
        // The feature type's order, and only what its dialog shows on either
        // side (live cp-921: the read-only dialogs).
        if let Some(t) = ty {
            ids.retain(|id| t.params.iter().any(|s| s.id == *id && [a, b].into_iter().flatten().any(|x| s.is_visible(&x.params))));
            ids.sort_by_key(|id| t.params.iter().position(|s| s.id == *id).unwrap_or(usize::MAX));
        }
        // The feature's own name and suppression first, then its parameters.
        let own = |label: &str, shape: ParamShape, f: fn(&Feature) -> String| ParamCompare { label: label.to_string(), base: a.map(f), target: b.map(f), shape };
        let head = [own("Name", ParamShape::Name, |x| x.name.clone()), own("Suppressed", ParamShape::Bool, |x| x.suppressed.to_string())];
        let params = head
            .into_iter()
            .chain(ids.into_iter()
            .map(|id| {
                let spec = ty.and_then(|t| t.params.iter().find(|s| s.id == id));
                let text = |side: Option<&Feature>, studio: &PartStudio| side.and_then(|x| x.params.get(id)).map(|v| param_text(v, spec, studio));
                let shape = [a, b].into_iter().flatten().find_map(|x| x.params.get(id)).map(shape_of).unwrap_or(ParamShape::Other);
                ParamCompare { label: spec.map(|s| s.label.to_string()).unwrap_or_else(|| id.to_string()), base: text(a, base), target: text(b, target), shape }
            }))
            .collect();
        let status = match (a, b) {
            (Some(_), None) => Some(ChangeKind::Removed),
            (None, Some(_)) => Some(ChangeKind::Added),
            (Some(x), Some(y)) if x != y => Some(ChangeKind::Modified),
            _ => None,
        };
        FeatureCompare { id: f.id, name: f.name.clone(), type_id: f.type_id.clone(), status, params }
    };
    let mut out: Vec<FeatureCompare> = base.features.iter().map(|f| row(Some(f), target.feature(f.id))).collect();
    for (k, f) in target.features.iter().enumerate() {
        if base.feature(f.id).is_some() {
            continue;
        }
        let after = target.features[..k].iter().rev().find_map(|p| out.iter().position(|r| r.id == p.id));
        out.insert(after.map_or(0, |i| i + 1), row(None, Some(f)));
    }
    out
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
    fn features_side_by_side() {
        let base = crate::demo::demo_document();
        let mut target = base.clone();
        let units = UnitPrefs::default();
        let tab = target.tabs.iter().position(|t| matches!(t.content, TabContent::PartStudio(_))).unwrap();
        let TabContent::PartStudio(ps) = &mut target.tabs[tab].content else { unreachable!() };
        let ex = ps.features.iter_mut().find(|f| f.type_id == "extrude").unwrap();
        ex.params.set("depth", ParamValue::Expr(ExprValue::new("30 mm")));
        let f = ps.new_feature("chamfer", &units).unwrap();
        ps.features.push(f);
        let (TabContent::PartStudio(a), TabContent::PartStudio(b)) = (&base.tabs[tab].content, &target.tabs[tab].content) else { unreachable!() };
        let rows = compare_features(a, b);
        assert_eq!(rows.len(), a.features.len() + 1);
        let ex = rows.iter().find(|r| r.type_id == "extrude").unwrap();
        assert_eq!(ex.status, Some(ChangeKind::Modified));
        let depth = ex.params.iter().find(|p| p.target.as_deref() == Some("30 mm")).unwrap();
        assert!(depth.differs());
        assert_eq!(rows.last().unwrap().status, Some(ChangeKind::Added));
        // Suppression shows as its own row.
        assert!(ex.params.iter().any(|p| p.label == "Suppressed" && !p.differs()));
        assert!(rows.iter().filter(|r| r.type_id != "extrude" && r.type_id != "chamfer").all(|r| r.status.is_none()));
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
