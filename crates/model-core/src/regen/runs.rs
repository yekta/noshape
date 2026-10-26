//! The regenerations a view has running (the app's Part Studio, live
//! behaviours.md "Pending state"): the newest edit wins. A running
//! regeneration whose features a newer request changes is aborted; one the
//! newer request only extends (✓ after the preview, the same features up to
//! the edited one) finishes its features into the cache and skips its
//! display.
//!
//! Every run that may still hold the Regenerator is kept until it returns
//! (until no one but [`Runs`] holds its [`RegenCancel`]):
//! a superseded preview still computing the feature is aborted too when a
//! later edit (deleting the feature) no longer extends it, so the next
//! regeneration never waits for it.

use super::RegenCancel;
use crate::configuration::ConfigValues;
use crate::partstudio::PartStudio;
use crate::reference::FeatureId;
use noshape_units::Quantity;
use std::collections::BTreeMap;
use std::sync::Arc;

/// What a regeneration computes, to tell whether a newer request makes it
/// stale. The regeneration holds a clone of `cancel` while it runs.
pub struct Run {
    pub cancel: RegenCancel,
    pub studio: Arc<PartStudio>,
    pub upto: Option<usize>,
    pub config: ConfigValues,
    pub doc_vars: BTreeMap<String, Quantity>,
    pub editing_sketch: Option<FeatureId>,
}

impl Run {
    /// `newer` evaluates the same features as this run (and maybe more after
    /// them), so this run's cached feature results stay useful.
    pub fn extended_by(&self, newer: &Run) -> bool {
        let n = self.upto.unwrap_or_else(|| self.studio.rollback_index()).min(self.studio.features.len());
        self.config == newer.config
            && self.doc_vars == newer.doc_vars
            && self.editing_sketch == newer.editing_sketch
            && self.studio.configuration == newer.studio.configuration
            && self.studio.contexts == newer.studio.contexts
            && newer.studio.features.len() >= n
            && self.studio.features[..n] == newer.studio.features[..n]
    }
}

/// The runs not known to have returned yet.
#[derive(Default)]
pub struct Runs(Vec<Run>);

impl Runs {
    /// A newer request starts: every run it doesn't extend is aborted (also
    /// one superseded earlier, still computing); the ones it extends are
    /// superseded and kept until they return.
    pub fn start(&mut self, run: Run) {
        self.0.retain(|old| {
            if !old.cancel.held_elsewhere() {
                false
            } else if old.extended_by(&run) {
                old.cancel.supersede();
                true
            } else {
                old.cancel.abort();
                false
            }
        });
        self.0.push(run);
    }

    /// The runs that may still be computing.
    pub fn running(&self) -> usize {
        self.0.iter().filter(|r| r.cancel.held_elsewhere()).count()
    }
}
