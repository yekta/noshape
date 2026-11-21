//! The Merge dialog's Tabs table (Versions and history › Merge into current workspace…, help "Merging"): each tab
//! changed since the merge base, its change in the target and in the source, the strategies it offers and the one it
//! gets under the overall strategy. The dialog (history-ui merge_dialog) and history_merge share it.

use crate::session::Session;
use anyhow::Result;
use noshape_model::document::{Document, Tab, TabContent};
use noshape_store::{MergePick, MicroversionId};
use std::collections::BTreeMap;

/// The merge dialog's warning (and history_merge's refusal) when the target workspace is protected and has changes of
/// its own.
pub const PROTECTED_NOTICE: &str = "Workspace protection has been enabled. Merges are allowed only when the source workspace has all the changes from the protected workspace. Merge the protected workspace into the source workspace first and then retry.";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Strategy {
    Keep,
    Merge,
    Replace,
}

impl Strategy {
    pub fn pick(self) -> MergePick {
        match self {
            Strategy::Keep => MergePick::Ours,
            Strategy::Merge => MergePick::Auto,
            Strategy::Replace => MergePick::Theirs,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
    None,
    Edited,
    New,
    Deleted,
}

impl Change {
    fn of(base: Option<&Vec<u8>>, side: Option<&Vec<u8>>) -> Self {
        match (base, side) {
            (None, None) => Change::None,
            (None, Some(_)) => Change::New,
            (Some(_), None) => Change::Deleted,
            (Some(a), Some(b)) if a == b => Change::None,
            _ => Change::Edited,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Change::None => "No changes",
            Change::Edited => "Edited",
            Change::New => "New tab",
            Change::Deleted => "Deleted",
        }
    }
}

/// One changed tab.
pub struct MergeTab {
    /// Its blob name ("tab/<id>").
    pub blob: String,
    pub tab: Tab,
    pub ours: Change,
    pub theirs: Change,
    /// "Merge changes from both" is offered: an edited Part Studio or Assembly (help: not drawings, not created /
    /// deleted tabs).
    pub mergeable: bool,
    /// Changes in separate branches can't be merged for this tab type (ⓘ).
    pub unmergeable_type: bool,
}

impl MergeTab {
    pub fn options(&self) -> &'static [Strategy] {
        if self.mergeable { &[Strategy::Keep, Strategy::Merge, Strategy::Replace] } else { &[Strategy::Keep, Strategy::Replace] }
    }

    /// The tab's strategy under the overall one (help merge-dialog-full-01: with "Merge changes", a tab changed only
    /// in the source is replaced, one changed only in the target kept, one edited on both sides merged).
    pub fn default_for(&self, overall: Strategy) -> Strategy {
        match overall {
            Strategy::Merge if self.mergeable && self.ours == Change::Edited && self.theirs == Change::Edited => Strategy::Merge,
            Strategy::Merge if self.theirs != Change::None => Strategy::Replace,
            Strategy::Merge => Strategy::Keep,
            s => s,
        }
    }
}

pub struct MergeTabs {
    /// Restore to <workspace>… (live 2351): the workspace against a history entry, Keep or Replace per tab.
    pub restore: bool,
    pub base: MicroversionId,
    pub changed: Vec<MergeTab>,
    pub unchanged: Vec<Tab>,
}

impl MergeTabs {
    pub fn opening(&self) -> (Strategy, bool) {
        opening(self.restore, self.changed.iter())
    }
}

/// The overall strategy the dialog opens on and whether "Merge changes from both" is offered: it is when both
/// workspaces changed something (live session-studio-1119: B1 alone changed, only Keep and Replace); a target that
/// changed nothing opens on Replace, a restore too.
pub fn opening<'a>(restore: bool, changed: impl Iterator<Item = &'a MergeTab> + Clone) -> (Strategy, bool) {
    let target_changed = changed.clone().any(|r| r.ours != Change::None);
    let source_changed = changed.clone().any(|r| r.theirs != Change::None);
    let merge_offered = !restore && target_changed && source_changed && changed.clone().any(|r| r.mergeable);
    let overall = if restore || (source_changed && !target_changed) {
        Strategy::Replace
    } else if merge_offered {
        Strategy::Merge
    } else {
        Strategy::Keep
    };
    (overall, merge_offered)
}

/// The tabs of the current workspace against `from` (a workspace head or a history entry), in the target's tab order
/// then the tabs only in the source; `restore` compares with `from` itself ("Changes in Main" since then).
pub fn merge_tabs(s: &Session, from: MicroversionId, restore: bool) -> Result<MergeTabs> {
    let store = &s.store;
    let ours = s.head()?;
    let base = if restore { from } else { store.merge_base(ours, from)?.ok_or_else(|| anyhow::anyhow!("These workspaces share no history."))? };
    let (bb, ob, tb) = (store.read_all(base)?, store.read_all(ours)?, store.read_all(from)?);
    let tab_of = |m: &BTreeMap<String, Vec<u8>>, n: &str| m.get(n).and_then(|b| serde_json::from_slice::<Tab>(b).ok());
    // The target's tab order as its tab bar shows it (live 49-merge-dialog: Part Studio 1 / Variable Studio 1 /
    // Assembly 1), then tabs only in the source.
    let current = &s.doc;
    let mut names: Vec<String> = current.ordered_tabs().iter().map(|t| Document::blob_name(t.id)).collect();
    for n in bb.keys().chain(ob.keys()).chain(tb.keys()).filter(|n| n.starts_with("tab/")) {
        if !names.contains(n) {
            names.push(n.clone());
        }
    }
    // The tabs using a Variable Studio changed on a side count as edited there too (live session-studio-1119: width
    // changed in B1's Variable Studio 1, Part Studio 1 and Assembly 1 "Edited" in B1 as well).
    let vs_users = |side: &BTreeMap<String, Vec<u8>>| -> Vec<String> {
        current
            .tabs
            .iter()
            .filter(|t| matches!(t.content, TabContent::VariableStudio(_)))
            .filter(|t| {
                let n = Document::blob_name(t.id);
                Change::of(bb.get(&n), side.get(&n)) == Change::Edited
            })
            .flat_map(|t| current.tab_users(t.id))
            .map(Document::blob_name)
            .collect()
    };
    let (ours_vs, theirs_vs) = (vs_users(&ob), vs_users(&tb));
    let via = |c: Change, users: &[String], n: &str| if c == Change::None && users.iter().any(|u| u == n) { Change::Edited } else { c };
    let (mut changed, mut unchanged) = (Vec::new(), Vec::new());
    for n in names {
        let Some(tab) = tab_of(&ob, &n).or_else(|| tab_of(&tb, &n)).or_else(|| tab_of(&bb, &n)) else { continue };
        let (oc, tc) = (via(Change::of(bb.get(&n), ob.get(&n)), &ours_vs, &n), via(Change::of(bb.get(&n), tb.get(&n)), &theirs_vs, &n));
        if oc == Change::None && tc == Change::None {
            unchanged.push(tab);
            continue;
        }
        let drawing = matches!(tab.content, TabContent::Drawing(_));
        let kind_merges = matches!(tab.content, TabContent::PartStudio(_) | TabContent::Assembly(_));
        let both = oc == Change::Edited && tc == Change::Edited;
        // Live (qa5 DOC2, mg-903): an edited Part Studio / Assembly offers "Merge changes from both" (not a created or
        // deleted tab).
        let edited_only = [oc, tc].iter().all(|c| matches!(c, Change::None | Change::Edited));
        changed.push(MergeTab { blob: n, tab, ours: oc, theirs: tc, mergeable: kind_merges && edited_only && !restore, unmergeable_type: drawing && both });
    }
    Ok(MergeTabs { restore, base, changed, unchanged })
}
