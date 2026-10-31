//! The regenerations a view has running (the app's Part Studio, live
//! behaviours.md "Pending state"): the newest edit wins. A running
//! regeneration whose features a newer request changes, or that goes past
//! the features a newer request needs (the rollback bar moved above the
//! running feature), is aborted; one the newer request only extends (✓
//! after the preview, the same features up to the edited one) finishes its
//! features into the cache and skips its display.
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
    /// How many features the run evaluates.
    fn evaluated(&self) -> usize {
        self.upto.unwrap_or_else(|| self.studio.rollback_index()).min(self.studio.features.len())
    }

    /// `newer` evaluates the same features as this run (and maybe more after
    /// them), so this run's cached feature results stay useful. A newer run
    /// that stops earlier doesn't: the features it needs are cached already
    /// or will be computed by it (qa REGEN-NO-CANCEL: a rollback above a
    /// running fillet waited for its 30 s budget).
    pub fn extended_by(&self, newer: &Run) -> bool {
        let n = self.evaluated();
        newer.evaluated() >= n
            && self.config == newer.config
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

    /// Abort every run (a cancelled edit whose last result is shown again:
    /// none of them is wanted).
    pub fn abort_all(&mut self) {
        for r in self.0.drain(..) {
            r.cancel.abort();
        }
    }

    /// The runs that may still be computing.
    pub fn running(&self) -> usize {
        self.0.iter().filter(|r| r.cancel.held_elsewhere()).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(studio: &Arc<PartStudio>, upto: Option<usize>) -> (Run, RegenCancel) {
        let cancel = RegenCancel::default();
        // The worker's handle: the run counts as computing.
        let held = cancel.clone();
        (Run { cancel, studio: studio.clone(), upto, config: ConfigValues::default(), doc_vars: BTreeMap::new(), editing_sketch: None }, held)
    }

    #[test]
    fn a_run_is_kept_only_while_the_newer_one_needs_all_of_it() {
        let studio = Arc::new(crate::demo::plate_studio());
        let n = studio.features.len();
        assert!(n >= 3);
        // ✓ after the dialog's preview: the preview goes on into the cache.
        let mut runs = Runs::default();
        let (preview, preview_cancel) = run(&studio, Some(n - 1));
        runs.start(preview);
        let (accepted, accepted_cancel) = run(&studio, None);
        runs.start(accepted);
        assert!(preview_cancel.superseded() && !preview_cancel.aborted());
        // The rollback bar above the running feature: the full run stops.
        let mut rolled = (*studio).clone();
        rolled.rollback = Some(1);
        let (back, back_cancel) = run(&Arc::new(rolled), None);
        runs.start(back);
        assert!(accepted_cancel.aborted() && preview_cancel.aborted() && !back_cancel.superseded());
        // A dialog previewing an earlier feature stops a full run too.
        let (full, full_cancel) = run(&studio, None);
        runs.start(full);
        let (earlier, _) = run(&studio, Some(1));
        runs.start(earlier);
        assert!(full_cancel.aborted());
    }
}
