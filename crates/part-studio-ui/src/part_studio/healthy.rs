//! A broken pick's "Edit healthy moment" (live 1993 (b), help "Repairing"): the document's recent history, newest
//! first, is searched for the last moment where the pick held (its creating feature there, unsuppressed, before the
//! feature); that moment opens in the Repair panel beside the Part Studio. None in the last [`MOMENTS`]: "Checked 50
//! recent document history moments - no healthy state found." and the creator's row selected.

use super::PartStudioView;
use gpui_kit::*;
use noshape_model::document::{Document, TabContent, TabId};
use noshape_model::reference::{FeatureId, Reference};
use noshape_store::{BranchId, MicroversionId};
use std::path::PathBuf;

/// How many moments before the current one are checked.
const MOMENTS: usize = 50;

/// The search's outcome: the healthy moment (and its Repair label), or how many moments were checked.
enum Found {
    Moment(MicroversionId, String),
    None(usize),
}

impl PartStudioView {
    pub(super) fn edit_healthy_moment(&mut self, feature: FeatureId, pick: Reference, cx: &mut Context<Self>) {
        let (path, branch) = {
            let s = &self.doc.read(cx).session;
            (s.store.path().to_path_buf(), s.branch)
        };
        let tab = self.tab;
        self.message = Some((noshape_model::display::FeatureStatus::Info, "Checking recent document history moments…".into()));
        cx.notify();
        let search = cx.background_spawn({
            let pick = pick.clone();
            async move { search(path, branch, tab, feature, &pick) }
        });
        cx.spawn(async move |this, cx| {
            let found = search.await;
            let _ = this.update(cx, |this: &mut Self, cx| {
                match found {
                    Ok(Found::Moment(mv, label)) => {
                        this.message = None;
                        if let Err(e) = crate::repair_mode::open(&this.doc, mv, label, cx) {
                            this.message = Some((noshape_model::display::FeatureStatus::Error, format!("{e:#}")));
                        }
                    }
                    Ok(Found::None(n)) => {
                        this.message = Some((noshape_model::display::FeatureStatus::Info, format!("Checked {n} recent document history moments - no healthy state found.")));
                        // Live: the pick's creating feature's row is selected.
                        if let Some(f) = this.studio(cx).and_then(|s| creator(s, &pick)).filter(|f| this.studio(cx).is_some_and(|s| s.feature(*f).is_some())) {
                            this.selected_features = vec![f];
                        }
                    }
                    Err(e) => this.message = Some((noshape_model::display::FeatureStatus::Error, format!("{e:#}"))),
                }
                cx.notify();
            });
        })
        .detach();
    }
}

/// The feature that makes `r` in `ps`.
fn creator(ps: &noshape_model::partstudio::PartStudio, r: &Reference) -> Option<FeatureId> {
    match r {
        Reference::Edge(n) | Reference::Vertex(n) => noshape_ops::studio::edge_creator(n, ps),
        r => r.depends_on(),
    }
}

/// Whether `pick` holds for `feature` of tab `tab` in `doc`.
fn healthy(doc: &Document, tab: TabId, feature: FeatureId, pick: &Reference) -> bool {
    let Some(TabContent::PartStudio(ps)) = doc.tabs.iter().find(|t| t.id == tab).map(|t| &t.content) else { return false };
    let Some(at) = ps.features.iter().position(|f| f.id == feature && !f.suppressed) else { return false };
    let missing = noshape_ops::studio::missing_reference(doc, ps);
    !missing(pick) && !super::later_reference(ps, at)(pick)
}

/// The newest moment before the current one where `pick` held, on the document's own read-only connection.
fn search(path: PathBuf, branch: BranchId, tab: TabId, feature: FeatureId, pick: &Reference) -> anyhow::Result<Found> {
    let store = noshape_store::Document::open_read_only(&path)?;
    let moments = store.history_from(store.head(branch)?, Some(MOMENTS + 1))?;
    let branch_name = store.branch(branch).map(|b| b.name).unwrap_or_else(|_| "Main".into());
    let mut checked = 0;
    for m in moments.iter().skip(1) {
        checked += 1;
        let Ok(doc) = store.read_all(m.id).map_err(anyhow::Error::from).and_then(|b| Document::from_blobs(&b).map_err(anyhow::Error::from)) else { continue };
        if healthy(&doc, tab, feature, pick) {
            return Ok(Found::Moment(m.id, format!("{branch_name} :: {}", m.description)));
        }
    }
    Ok(Found::None(checked))
}
