//! Slow regenerations (live behaviours.md "Pending state", a 1000-instance
//! Linear pattern):
//! - while a preview computes, the dialog, feature list and cursor don't
//!   change and the previous result stays shown as it is;
//! - after 1.2 s Onshape puts its working overlay over the graphics: a spinner
//!   at the bottom centre ("Opening <feature>…" under it while a dialog
//!   opens), until the final geometry is in;
//! - ✓ closes the dialog at once and the final regeneration runs under that
//!   overlay; Esc doesn't cancel it.
//!
//! Internally the newest edit wins: a running regeneration whose features a
//! newer request changes is aborted; one the newer request only extends
//! (✓ after the preview, same features up to the edited one) finishes its
//! features into the cache and skips its display.

use super::PartStudioView;
use gpui_kit::*;
use noshape_model::configuration::ConfigValues;
use noshape_model::partstudio::PartStudio;
use noshape_model::reference::FeatureId;
use noshape_model::regen::RegenCancel;
use noshape_units::Quantity;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Live: the working overlay appears 1.2 s after the edit.
pub(super) const OVERLAY_DELAY: Duration = Duration::from_millis(1200);

/// A regeneration whose result isn't shown yet.
pub(super) struct Pending {
    pub seq: u64,
    pub since: Instant,
    /// "Opening <feature>…" (an edit dialog opening on a slow model).
    pub opening: Option<String>,
}

/// What the running regeneration computes, to tell whether a newer request
/// makes it stale.
pub(super) struct InFlight {
    pub cancel: RegenCancel,
    pub studio: Arc<PartStudio>,
    pub upto: Option<usize>,
    pub config: ConfigValues,
    pub doc_vars: BTreeMap<String, Quantity>,
    pub editing_sketch: Option<FeatureId>,
}

impl InFlight {
    /// The newer request evaluates the same features as this one (and maybe
    /// more after them), so this run's cached feature results stay useful.
    pub fn extended_by(&self, studio: &PartStudio, config: &ConfigValues, doc_vars: &BTreeMap<String, Quantity>, editing_sketch: Option<FeatureId>) -> bool {
        let n = self.upto.unwrap_or_else(|| self.studio.rollback_index()).min(self.studio.features.len());
        self.config == *config
            && self.doc_vars == *doc_vars
            && self.editing_sketch == editing_sketch
            && self.studio.configuration == studio.configuration
            && self.studio.contexts == studio.contexts
            && studio.features.len() >= n
            && self.studio.features[..n] == studio.features[..n]
    }

    /// Stop it for a newer request: abort, or let it finish its features
    /// when the newer one only extends it.
    pub fn retire(self, studio: &PartStudio, config: &ConfigValues, doc_vars: &BTreeMap<String, Quantity>, editing_sketch: Option<FeatureId>) {
        if self.extended_by(studio, config, doc_vars, editing_sketch) {
            self.cancel.supersede();
        } else {
            self.cancel.abort();
        }
    }
}

/// Onshape's medium spinner (`os-spinner-medium`, 41 px box): a light grey
/// ring with a blue quarter arc turning (live pend-accept1000b-spinner). A
/// view of its own: its animation frames re-render only it, not the Part
/// Studio.
pub(super) struct Spinner;

impl Render for Spinner {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        spinner()
    }
}

fn spinner() -> impl IntoElement {
    div().size(px(41.)).flex_none().with_animation("pending-spinner", Animation::new(Duration::from_millis(900)).repeat(), |d, t| {
        d.child(
            canvas(
                |_, _, _| {},
                move |bounds, _, window, _| {
                    let c = bounds.center();
                    let r = 13.5_f32;
                    let ring = |from: f32, to: f32, color: Rgba, window: &mut Window| {
                        let steps = 48;
                        let mut pb = PathBuilder::stroke(px(3.));
                        for i in 0..=steps {
                            let a = from + (to - from) * i as f32 / steps as f32;
                            let p = point(c.x + px(r * a.cos()), c.y + px(r * a.sin()));
                            if i == 0 { pb.move_to(p) } else { pb.line_to(p) }
                        }
                        if let Ok(path) = pb.build() {
                            window.paint_path(path, color);
                        }
                    };
                    let tau = std::f32::consts::TAU;
                    ring(0., tau, rgb(0xdddddd), window);
                    let start = t * tau - tau / 4.;
                    ring(start, start + tau / 4., rgb(0x1651b0), window);
                },
            )
            .size_full(),
        )
    })
}

impl PartStudioView {
    /// A regeneration started: pending from now (or still pending from an
    /// earlier edit), with the overlay timer.
    pub(super) fn begin_pending(&mut self, seq: u64, window: &mut Window, cx: &mut Context<Self>) {
        let since = self.pending.as_ref().map(|p| p.since).unwrap_or_else(Instant::now);
        let opening = self.pending.as_mut().and_then(|p| p.opening.take());
        self.pending = Some(Pending { seq, since, opening });
        let wait = OVERLAY_DELAY.saturating_sub(since.elapsed());
        self.pending_timer = Some(cx.spawn_in(window, async move |this, cx| {
            cx.background_executor().timer(wait).await;
            let _ = this.update(cx, |_, cx| cx.notify());
        }));
    }

    /// The next regeneration opens `name`'s dialog.
    pub(super) fn pending_opening(&mut self, name: String) {
        match &mut self.pending {
            Some(p) => p.opening = Some(name),
            None => self.pending = Some(Pending { seq: 0, since: Instant::now(), opening: Some(name) }),
        }
    }

    /// The regeneration `seq` is fully shown.
    pub(super) fn end_pending(&mut self, seq: u64, cx: &mut Context<Self>) {
        if self.pending.as_ref().is_some_and(|p| p.seq == seq) {
            let shown = self.pending.take().is_some_and(|p| p.since.elapsed() >= OVERLAY_DELAY);
            self.pending_timer = None;
            if shown {
                cx.notify();
            }
        }
    }

    /// The working overlay over the graphics (live: inserted at +1.2 s,
    /// spinner at the bottom centre, text under it while a dialog opens).
    pub(super) fn render_pending_overlay(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let p = self.pending.as_ref().filter(|p| p.since.elapsed() >= OVERLAY_DELAY)?;
        let pal = crate::theme::p();
        // Live (1879×973 graphics): the 41 px spinner box ends 85 px above
        // the bottom; the text is centred 49 px under the spinner's centre.
        let row = |bottom: f32| div().absolute().left_0().right_0().bottom(px(bottom)).flex().justify_center();
        let spinner = self.pending_spinner.get_or_insert_with(|| cx.new(|_| Spinner)).clone();
        let mut layer = div().absolute().top_0().left_0().size_full().child(row(85.).child(spinner));
        if let Some(name) = p.opening.clone() {
            layer = layer.child(row(47.).child(div().h(px(18.)).text_size(px(14.)).text_color(pal.text).child(format!("Opening {name}…"))));
        }
        Some(layer.into_any_element())
    }
}
