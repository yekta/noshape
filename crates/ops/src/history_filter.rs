//! The Versions and history panel's search ▾ (live documents/117–119):
//! who made a change, which tab or kinds of tab it changed, and when. The
//! app's panel and history_list both filter with it.

use noshape_model::document::{Document, TabContent, TabId};
use noshape_store::MicroversionInfo;

/// "Modified by".
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ModifiedBy {
    #[default]
    Any,
    OnlyMe,
    NotMe,
}

impl ModifiedBy {
    pub const ALL: [ModifiedBy; 3] = [ModifiedBy::Any, ModifiedBy::OnlyMe, ModifiedBy::NotMe];

    pub fn label(self) -> &'static str {
        match self {
            ModifiedBy::Any => "Any",
            ModifiedBy::OnlyMe => "Only me",
            ModifiedBy::NotMe => "Not me",
        }
    }
}

/// The filter's tab kinds, in Onshape's order.
pub const KINDS: [&str; 5] = ["partstudio", "assembly", "drawing", "featurestudio", "file"];

/// A tab's kind for the filter ("" for kinds it has no toggle for).
pub fn filter_kind(c: &TabContent) -> &'static str {
    match c {
        TabContent::PartStudio(_) => "partstudio",
        TabContent::Assembly(_) => "assembly",
        TabContent::Drawing(_) => "drawing",
        TabContent::FeatureStudio(_) => "featurestudio",
        TabContent::File(_) => "file",
        TabContent::VariableStudio(_) | TabContent::MaterialLibrary(_) => "",
    }
}

/// Is `author` the local user (Preferences name, OS full name or login)?
pub fn is_me(author: &str) -> bool {
    !author.is_empty() && [noshape_store::local_user_name(), noshape_store::os_user_name(), noshape_store::os_login()].iter().any(|n| n == author)
}

/// The applied filter: tab, tab kinds, author, time range (ms).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HistoryFilter {
    pub tab: Option<TabId>,
    pub kinds: Vec<&'static str>,
    pub by: ModifiedBy,
    pub from: Option<i64>,
    pub to: Option<i64>,
}

impl HistoryFilter {
    pub fn is_active(&self) -> bool {
        *self != HistoryFilter::default()
    }

    /// Does change `m` pass? `changed_tabs`: the tabs it changed (asked only
    /// when a tab or kind is filtered: it diffs the store).
    pub fn matches(&self, m: &MicroversionInfo, doc: &Document, changed_tabs: impl FnOnce() -> Vec<TabId>) -> bool {
        let mine = is_me(&m.author);
        if (self.by == ModifiedBy::OnlyMe && !mine) || (self.by == ModifiedBy::NotMe && mine) {
            return false;
        }
        if self.from.is_some_and(|t| m.created < t) || self.to.is_some_and(|t| m.created > t) {
            return false;
        }
        if self.tab.is_none() && self.kinds.is_empty() {
            return true;
        }
        changed_tabs().iter().any(|t| self.tab.is_none_or(|ft| ft == *t) && (self.kinds.is_empty() || doc.tab(*t).is_some_and(|tab| self.kinds.contains(&filter_kind(&tab.content)))))
    }
}

/// The tabs change `m` changed (against its first parent).
pub fn changed_tabs(store: &noshape_store::Document, m: &MicroversionInfo) -> Vec<TabId> {
    let Some(parent) = m.parents.first() else { return Vec::new() };
    store.diff(*parent, m.id).unwrap_or_default().iter().filter_map(|c| c.name.strip_prefix("tab/")).filter_map(|h| u64::from_str_radix(h, 16).ok()).map(TabId).collect()
}
