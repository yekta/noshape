//! Feature Studio "Monitor ▾ › Profile <studio>" (live dom/fs-profile.txt):
//! regenerates the Part Studio from scratch with FeatureScript line timing
//! on, then every Feature Studio of the document shows a badge at the
//! editor's top-left ("288ms", hover "288ms to fully regenerate <studio>")
//! and an 8 × 16 heat bar beside each line that ran ("<1ms" or "N ms" on
//! hover). Commit re-profiles; uncommitted edits grey the bars.

use crate::doc_model::DocModel;
use gpui_kit::{App, Entity, EntityId, Global, Hsla, Rgba};
use noshape_model::document::{TabContent, TabId};
use std::collections::{BTreeMap, HashMap};
use std::time::{Duration, Instant};

/// The last profile of a Part Studio (or Variable Studio).
#[derive(Clone, Debug)]
pub struct Profile {
    pub studio_name: String,
    /// Time to fully regenerate the studio.
    pub total: Duration,
    /// Time per line, by Feature Studio.
    pub lines: HashMap<TabId, BTreeMap<u32, Duration>>,
}

#[derive(Default)]
struct State {
    /// Studio being profiled (active), and its last result.
    studio: Option<TabId>,
    result: Option<Profile>,
    running: bool,
}

#[derive(Default)]
struct Profiles(HashMap<EntityId, State>);

impl Global for Profiles {}

/// The studio being profiled in a document, if profiling is on.
pub fn profiling(doc: &Entity<DocModel>, cx: &App) -> Option<TabId> {
    cx.try_global::<Profiles>().and_then(|p| p.0.get(&doc.entity_id())).and_then(|s| s.studio)
}

/// The last profile shown in a document's Feature Studios.
pub fn result(doc: &Entity<DocModel>, cx: &App) -> Option<Profile> {
    let s = cx.try_global::<Profiles>()?.0.get(&doc.entity_id())?;
    s.studio?;
    s.result.clone()
}

/// Stop profiling (live: a click on the active "Profiling <studio>" button).
pub fn stop(doc: &Entity<DocModel>, cx: &mut App) {
    let id = doc.entity_id();
    if let Some(s) = cx.default_global::<Profiles>().0.get_mut(&id) {
        s.studio = None;
        s.result = None;
    }
    cx.refresh_windows();
}

/// Profile `studio` now (and again after each commit while it stays on).
pub fn start(doc: &Entity<DocModel>, studio: TabId, cx: &mut App) {
    let id = doc.entity_id();
    {
        let s = cx.default_global::<Profiles>().0.entry(id).or_default();
        s.studio = Some(studio);
        if s.running {
            return;
        }
        s.running = true;
    }
    let document = doc.read(cx).session.doc.clone();
    let derived = doc.read(cx).derived_provider();
    let doc_entity = doc.clone();
    cx.spawn(async move |cx| {
        let result = cx.background_executor().spawn(async move { run(&document, studio, derived) }).await;
        cx.update(|cx| {
            if let Some(s) = cx.default_global::<Profiles>().0.get_mut(&doc_entity.entity_id()) {
                s.running = false;
                if s.studio == Some(studio) {
                    s.result = result;
                }
            }
            cx.refresh_windows();
        });
    })
    .detach();
    cx.refresh_windows();
}

/// After a Feature Studio commit: profile again if profiling is on.
pub fn committed(doc: &Entity<DocModel>, cx: &mut App) {
    if let Some(studio) = profiling(doc, cx) {
        start(doc, studio, cx);
    }
}

/// A full regeneration of `studio` with line timing.
fn run(doc: &noshape_model::Document, studio: TabId, derived: std::sync::Arc<crate::derived::DocStudios>) -> Option<Profile> {
    use noshape_model::regen::{RegenOptions, Regenerator};
    let tab = doc.tab(studio)?;
    let name = tab.name.clone();
    noshape_model::custom::run::set_profiling(true);
    let start = Instant::now();
    match &tab.content {
        TabContent::PartStudio(ps) => {
            let opts = RegenOptions {
                document_variables: noshape_model::variables::element_variables(doc, studio),
                derived: Some(derived),
                custom_features: Some(noshape_model::custom::DocumentScripts::from_document(doc)),
                ..Default::default()
            };
            let _ = Regenerator::new().regenerate(ps, &ps.configuration.defaults(), &doc.settings.units, &opts);
        }
        TabContent::VariableStudio(_) => {
            let _ = noshape_model::variables::element_variables(doc, studio);
        }
        _ => {
            noshape_model::custom::run::set_profiling(false);
            return None;
        }
    }
    let total = start.elapsed();
    noshape_model::custom::run::set_profiling(false);
    let lines = noshape_model::custom::run::profile(doc, studio);
    Some(Profile { studio_name: name, total, lines })
}

/// "<1ms" or whole milliseconds ("15ms").
pub fn label(t: Duration) -> String {
    let ms = t.as_secs_f64() * 1000.0;
    if ms < 1.0 { "<1ms".to_string() } else { format!("{}ms", ms.round() as u64) }
}

/// Heat colour: white through yellow to orange. Fitted to live's samples
/// (dom/fs-profile.txt: <1ms ≈ (255,254,248), 1ms (255,245,103), 4ms
/// (255,212,7), 15ms (255,146,0), 23ms (255,125,0)); the badge uses the
/// same scale at 1/170 (288ms → (255,240,36)).
pub fn heat(ms: f64) -> Hsla {
    let g = 255.0 - 165.0 * (1.0 - (-ms / 15.0).exp());
    let b = 255.0 * (-ms).exp();
    Rgba { r: 1.0, g: (g / 255.0) as f32, b: (b / 255.0) as f32, a: 1.0 }.into()
}

pub fn badge_heat(total: Duration) -> Hsla {
    heat(total.as_secs_f64() * 1000.0 / 170.0)
}
