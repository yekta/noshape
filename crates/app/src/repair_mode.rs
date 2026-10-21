//! Versions and history › Repair / View in repair (help "Repairing", live
//! document/rp-900–914, dom/ui2-repair-panel.html): a Part Studio shown as
//! it was at a history entry, in a 567 px panel docked right of the
//! graphics, to repair references broken since.
//! - the history header's wrench "Repair" toggles a picking mode (banner
//!   "Select a history entry to repair this Part Studio"); the next entry
//!   clicked opens in Repair ([`picking`], [`toggle_picking`]);
//! - change and version rows have "View in repair" ([`open`]);
//! - the panel ([`render_panel`]): blue "Repair" title, a pill "Viewing
//!   <entry>" with ↗ and ?, the view-only graphics (perf's second view of
//!   [`document`], following the main camera while [`synchronized`]), and
//!   the "Synchronize view" toggle at the bottom left;
//! - the Part Studio's right strip gains the wrench "Repair panel", which
//!   closes it ([`close`]).
//!
//! The state is per document window (a global keyed by the DocModel).

use crate::doc_model::DocModel;
use crate::theme::p;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::document::Document;
use noshape_store::MicroversionId;
use std::collections::HashMap;
use std::sync::Arc;

/// The panel's width (live rp-906).
pub const PANEL_W: f32 = 567.;

pub struct RepairSession {
    #[allow(dead_code)] // Wired in next step: perf's second view keys its cached scene on it.
    pub mv: MicroversionId,
    /// "V2", or "Main :: Insert new tab : ui iso" for a change.
    pub label: String,
    pub doc: Arc<Document>,
    pub synchronized: bool,
}

#[derive(Default)]
struct State {
    sessions: HashMap<EntityId, RepairSession>,
    picking: HashMap<EntityId, bool>,
}
impl Global for State {}

fn changed(doc: &Entity<DocModel>, cx: &mut App) {
    doc.update(cx, |_, cx| cx.notify());
    cx.refresh_windows();
}

/// View in repair: the document at `mv` in the Repair panel.
pub fn open(doc: &Entity<DocModel>, mv: MicroversionId, label: String, cx: &mut App) -> anyhow::Result<()> {
    let at = {
        let s = &doc.read(cx).session;
        Document::from_blobs(&s.store.read_all(mv)?)?
    };
    let st = cx.default_global::<State>();
    st.picking.remove(&doc.entity_id());
    st.sessions.insert(doc.entity_id(), RepairSession { mv, label, doc: Arc::new(at), synchronized: true });
    changed(doc, cx);
    Ok(())
}

/// The strip's "Repair panel" wrench: close the panel.
pub fn close(doc: &Entity<DocModel>, cx: &mut App) {
    if cx.default_global::<State>().sessions.remove(&doc.entity_id()).is_some() {
        changed(doc, cx);
    }
}

pub fn is_open(doc: &Entity<DocModel>, cx: &App) -> bool {
    cx.try_global::<State>().is_some_and(|s| s.sessions.contains_key(&doc.entity_id()))
}

/// The history header's wrench: picking a history entry to repair from.
pub fn picking(doc: &Entity<DocModel>, cx: &App) -> bool {
    cx.try_global::<State>().and_then(|s| s.picking.get(&doc.entity_id())).copied().unwrap_or(false)
}

pub fn toggle_picking(doc: &Entity<DocModel>, cx: &mut App) {
    let st = cx.default_global::<State>();
    let on = st.picking.get(&doc.entity_id()).copied().unwrap_or(false);
    st.picking.insert(doc.entity_id(), !on);
    changed(doc, cx);
}

/// The document as it was at the repaired entry (for the second view).
#[allow(dead_code)] // Wired in next step: perf's second view of the Repair panel reads it.
pub fn document(doc: &Entity<DocModel>, cx: &App) -> Option<Arc<Document>> {
    cx.try_global::<State>().and_then(|s| s.sessions.get(&doc.entity_id())).map(|s| s.doc.clone())
}

/// Whether the repair view follows the main camera ("Synchronize view", on
/// by default).
#[allow(dead_code)] // Wired in next step: perf's second view of the Repair panel reads it.
pub fn synchronized(doc: &Entity<DocModel>, cx: &App) -> bool {
    cx.try_global::<State>().and_then(|s| s.sessions.get(&doc.entity_id())).is_some_and(|s| s.synchronized)
}

/// The panel's chrome (live rp-906): title, the "Viewing …" pill and the
/// sync toggle around `graphics` (perf's view-only second view).
pub fn render_panel(doc: &Entity<DocModel>, graphics: Option<AnyElement>, cx: &App) -> Option<AnyElement> {
    let s = cx.try_global::<State>()?.sessions.get(&doc.entity_id())?;
    let pal = p();
    let sync = s.synchronized;
    let d = doc.clone();
    Some(
        v_flex()
            .w(px(PANEL_W))
            .h_full()
            .flex_none()
            .relative()
            .bg(rgb(0xffffff))
            .border_l_1()
            .border_color(rgb(0xd7d7d7))
            .child(
                div()
                    .h(px(28.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .border_b_2()
                    .border_color(rgb(0x1651b0))
                    .text_size(px(14.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(0x1651b0))
                    .child("Repair"),
            )
            .child(
                div().flex_1().min_h_0().relative().children(graphics).child(
                    div().absolute().top(px(6.)).left_0().right_0().flex().justify_center().child(
                        h_flex()
                            .h(px(30.))
                            .px(px(20.))
                            .gap(px(10.))
                            .bg(rgb(0xffffff))
                            .border_1()
                            .border_color(rgb(0xcccccc))
                            .rounded_b(px(6.))
                            .text_size(px(14.))
                            .text_color(pal.text)
                            .child(format!("Viewing {}", s.label))
                            .child(div().id("repair-new-tab").size(px(18.)).cursor_pointer().tooltip(crate::ui::tooltip("Open in new browser tab")).child(crate::assets::icon("open-external", 18., cx)))
                            .child(div().size(px(20.)).child(crate::assets::icon("help-filled", 20., cx))),
                    ),
                ),
            )
            // "Synchronize view": a round toggle at the bottom left, on by default.
            .child(
                div()
                    .id("repair-sync")
                    .absolute()
                    .left(px(10.))
                    .bottom(px(8.))
                    .size(px(30.))
                    .rounded_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .when(sync, |d| d.bg(rgb(0xeaeaea)))
                    .hover(|d| d.bg(rgb(0xeaeaea)))
                    .tooltip(crate::ui::tooltip("Synchronize view"))
                    .child(crate::assets::icon("sync-view", 20., cx))
                    .on_click(move |_, _, cx| {
                        if let Some(s) = cx.default_global::<State>().sessions.get_mut(&d.entity_id()) {
                            s.synchronized = !s.synchronized;
                        }
                        changed(&d, cx);
                    }),
            )
            .into_any_element(),
    )
}
