//! Three-way merge of tabs (Onshape's "Merge changes" for a tab).
//!
//! Part Studios merge feature-by-feature (matched by feature id): changes
//! made on only one side are applied; features changed on both sides keep
//! "ours" and are reported as conflicts. Other tab kinds merge whole-tab
//! (one-sided changes win, otherwise ours).

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

#[cfg(test)]
mod tests {
    use super::*;
    use noshape_units::UnitPrefs;

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
