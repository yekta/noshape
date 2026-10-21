//! Three-way merge of tabs (Onshape's "Merge changes" for a tab).
//!
//! Part Studios merge feature-by-feature (matched by feature id): changes
//! made on only one side are applied; features changed on both sides keep
//! "ours" and are reported as conflicts. Assemblies merge item by item
//! ([`merge_json`]). Other tab kinds merge whole-tab (one-sided changes
//! win, otherwise ours).

use crate::document::{Tab, TabContent};
use crate::partstudio::{Feature, PartStudio};
use crate::reference::FeatureId;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Default, Clone, PartialEq)]
pub struct MergeReport {
    pub conflicts: Vec<FeatureId>,
    pub added: Vec<FeatureId>,
    pub removed: Vec<FeatureId>,
    pub changed: Vec<FeatureId>,
}

pub fn merge_part_studio(base: &PartStudio, ours: &PartStudio, theirs: &PartStudio) -> (PartStudio, MergeReport) {
    let mut report = MergeReport::default();
    let index = |ps: &PartStudio| -> HashMap<FeatureId, Feature> { ps.features.iter().map(|f| (f.id, f.clone())).collect() };
    let b = index(base);
    let o = index(ours);
    let t = index(theirs);
    let mut result = ours.clone();

    // Changes and deletions coming from theirs.
    for (id, bf) in &b {
        let of = o.get(id);
        let tf = t.get(id);
        match (of, tf) {
            (Some(of), Some(tf)) if tf != bf => {
                if of == bf {
                    if let Some(slot) = result.features.iter_mut().find(|f| f.id == *id) {
                        *slot = tf.clone();
                    }
                    report.changed.push(*id);
                } else if of != tf {
                    report.conflicts.push(*id);
                }
            }
            (Some(of), None) => {
                if of == bf {
                    result.features.retain(|f| f.id != *id);
                    report.removed.push(*id);
                } else {
                    report.conflicts.push(*id);
                }
            }
            _ => {}
        }
    }
    // Additions from theirs: insert after the nearest preceding feature that exists in the result.
    let base_ids: HashSet<FeatureId> = b.keys().copied().collect();
    for (i, tf) in theirs.features.iter().enumerate() {
        if base_ids.contains(&tf.id) || o.contains_key(&tf.id) {
            continue;
        }
        let mut insert_at = 0;
        for prev in theirs.features[..i].iter().rev() {
            if let Some(p) = result.features.iter().position(|f| f.id == prev.id) {
                insert_at = p + 1;
                break;
            }
        }
        result.features.insert(insert_at.min(result.features.len()), tf.clone());
        report.added.push(tf.id);
    }
    // Configuration / properties: take theirs if only they changed.
    if ours.configuration == base.configuration && theirs.configuration != base.configuration {
        result.configuration = theirs.configuration.clone();
    }
    if ours.part_properties == base.part_properties && theirs.part_properties != base.part_properties {
        result.part_properties = theirs.part_properties.clone();
    }
    if ours.property_tables == base.property_tables && theirs.property_tables != base.property_tables {
        result.property_tables = theirs.property_tables.clone();
    }
    if ours.annotations == base.annotations && theirs.annotations != base.annotations {
        result.annotations = theirs.annotations.clone();
    }
    if ours.default_tolerances == base.default_tolerances && theirs.default_tolerances != base.default_tolerances {
        result.default_tolerances = theirs.default_tolerances.clone();
    }
    result.rollback = None;
    (result, report)
}

/// Merge serialized tabs; returns merged bytes (None = deleted).
pub fn merge_tab_blobs(base: Option<&[u8]>, ours: Option<&[u8]>, theirs: Option<&[u8]>) -> Option<Vec<u8>> {
    let parse = |b: Option<&[u8]>| {
        let mut tab = b.and_then(|b| serde_json::from_slice::<Tab>(b).ok())?;
        if let TabContent::PartStudio(ps) = &mut tab.content {
            ps.upgrade_saved_params();
        }
        Some(tab)
    };
    match (parse(base), parse(ours), parse(theirs)) {
        (Some(b), Some(mut o), Some(t)) => {
            if let (TabContent::PartStudio(bp), TabContent::PartStudio(op), TabContent::PartStudio(tp)) = (&b.content, &o.content, &t.content) {
                let (merged, _) = merge_part_studio(bp, op, tp);
                o.content = TabContent::PartStudio(merged);
                if o.name == b.name {
                    o.name = t.name.clone();
                }
                return serde_json::to_vec(&o).ok();
            }
            // Assemblies merge item by item (instances, mates, connectors, …
            // matched by id; other fields one by one).
            if let (TabContent::Assembly(_), TabContent::Assembly(_), TabContent::Assembly(_)) = (&b.content, &o.content, &t.content) {
                let (Ok(bv), Ok(ov), Ok(tv)) = (serde_json::to_value(&b.content), serde_json::to_value(&o.content), serde_json::to_value(&t.content)) else { return ours.map(|x| x.to_vec()) };
                if let Ok(merged) = serde_json::from_value::<TabContent>(merge_json(&bv, &ov, &tv)) {
                    o.content = merged;
                }
                if o.name == b.name {
                    o.name = t.name.clone();
                }
                return serde_json::to_vec(&o).ok();
            }
            // Feature Studios merge as a whole: take theirs if only they changed.
            if let (TabContent::FeatureStudio(bf), TabContent::FeatureStudio(of), TabContent::FeatureStudio(tf)) = (&b.content, &o.content, &t.content) {
                if of == bf && tf != bf {
                    o.content = TabContent::FeatureStudio(tf.clone());
                }
                if o.name == b.name {
                    o.name = t.name.clone();
                }
                return serde_json::to_vec(&o).ok();
            }
            ours.map(|x| x.to_vec())
        }
        _ => ours.map(|x| x.to_vec()),
    }
}

/// Three-way merge of JSON values: a side's change wins over an unchanged
/// other side; objects merge key by key; lists of objects that all carry
/// an `"id"` merge item by item (additions from theirs after the item they
/// follow there, deletions on one side applied when the other side left
/// the item alone); anything changed differently on both sides keeps ours.
pub fn merge_json(base: &serde_json::Value, ours: &serde_json::Value, theirs: &serde_json::Value) -> serde_json::Value {
    use serde_json::Value;
    if ours == theirs || theirs == base {
        return ours.clone();
    }
    if ours == base {
        return theirs.clone();
    }
    match (base, ours, theirs) {
        (Value::Object(b), Value::Object(o), Value::Object(t)) => {
            let mut out = o.clone();
            for (k, tv) in t {
                match (b.get(k), o.get(k)) {
                    (Some(bv), Some(ov)) => {
                        out.insert(k.clone(), merge_json(bv, ov, tv));
                    }
                    // Added by theirs only.
                    (None, None) => {
                        out.insert(k.clone(), tv.clone());
                    }
                    // Deleted by ours: stays deleted unless theirs changed it.
                    (Some(bv), None) if bv != tv => {
                        out.insert(k.clone(), tv.clone());
                    }
                    _ => {}
                }
            }
            // Deleted by theirs, untouched by ours.
            for (k, bv) in b {
                if !t.contains_key(k) && o.get(k) == Some(bv) {
                    out.remove(k);
                }
            }
            Value::Object(out)
        }
        (Value::Array(b), Value::Array(o), Value::Array(t)) => {
            let id = |v: &Value| v.get("id").map(|i| i.to_string());
            let keyed = |a: &Vec<Value>| a.iter().all(|v| id(v).is_some());
            if !(keyed(b) && keyed(o) && keyed(t)) || (o.is_empty() && t.is_empty()) {
                return ours.clone();
            }
            let find = |a: &Vec<Value>, k: &str| a.iter().find(|v| id(v).as_deref() == Some(k)).cloned();
            let mut out: Vec<Value> = Vec::with_capacity(o.len().max(t.len()));
            for ov in o {
                let k = id(ov).unwrap_or_default();
                match (find(b, &k), find(t, &k)) {
                    (Some(bv), Some(tv)) => out.push(merge_json(&bv, ov, &tv)),
                    // Deleted by theirs: gone unless ours changed it.
                    (Some(bv), None) if bv == *ov => {}
                    _ => out.push(ov.clone()),
                }
            }
            for (i, tv) in t.iter().enumerate() {
                let k = id(tv).unwrap_or_default();
                if find(b, &k).is_some() || find(o, &k).is_some() {
                    continue;
                }
                let at = t[..i].iter().rev().find_map(|p| out.iter().position(|x| id(x) == id(p))).map_or(0, |p| p + 1);
                out.insert(at.min(out.len()), tv.clone());
            }
            Value::Array(out)
        }
        _ => ours.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use noshape_units::UnitPrefs;

    #[test]
    fn json_merges_items_by_id() {
        use serde_json::json;
        let base = json!({"instances": [{"id": 1, "x": 0}, {"id": 2, "x": 0}], "mates": [], "name": "a"});
        let ours = json!({"instances": [{"id": 1, "x": 5}, {"id": 2, "x": 0}, {"id": 3, "x": 0}], "mates": [], "name": "a"});
        let theirs = json!({"instances": [{"id": 1, "x": 0}, {"id": 4, "x": 1}], "mates": [{"id": 9}], "name": "b"});
        let m = merge_json(&base, &ours, &theirs);
        // Ours' edit of 1 and addition of 3 kept, theirs' deletion of 2 and
        // additions of 4 and the mate taken, theirs' rename taken.
        assert_eq!(m, json!({"instances": [{"id": 1, "x": 5}, {"id": 4, "x": 1}, {"id": 3, "x": 0}], "mates": [{"id": 9}], "name": "b"}));
        // Both changed the same value: ours.
        assert_eq!(merge_json(&json!(1), &json!(2), &json!(3)), json!(2));
    }

    fn studio_with(n: usize) -> PartStudio {
        let mut ps = PartStudio::new();
        for _ in 0..n {
            let f = ps.new_feature("extrude", &UnitPrefs::default()).unwrap();
            ps.features.push(f);
        }
        ps
    }

    #[test]
    fn one_sided_edits_merge() {
        let base = studio_with(2);
        let mut ours = base.clone();
        let mut theirs = base.clone();
        ours.features[0].name = "Ours".into();
        theirs.features[1].name = "Theirs".into();
        let added = theirs.new_feature("fillet", &UnitPrefs::default()).unwrap();
        theirs.features.push(added.clone());
        let (m, r) = merge_part_studio(&base, &ours, &theirs);
        assert_eq!(m.features.len(), 3);
        assert_eq!(m.features[0].name, "Ours");
        assert_eq!(m.features[1].name, "Theirs");
        assert_eq!(m.features[2].id, added.id);
        assert!(r.conflicts.is_empty());
    }

    #[test]
    fn both_sided_edit_is_conflict_keeping_ours() {
        let base = studio_with(1);
        let mut ours = base.clone();
        let mut theirs = base.clone();
        ours.features[0].name = "A".into();
        theirs.features[0].name = "B".into();
        let (m, r) = merge_part_studio(&base, &ours, &theirs);
        assert_eq!(m.features[0].name, "A");
        assert_eq!(r.conflicts.len(), 1);
    }
}
