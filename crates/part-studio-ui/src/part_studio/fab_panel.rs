//! Fabrication panels: the sheet metal flat patterns (unfolded in the
//! background for the Sheet metal table and flat view) and the frame cut list.

use super::PartStudioView;
use crate::theme::p;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::*;
use noshape_model::reference::BodyId;
use noshape_model::regen::context::Context as RegenContext;
use noshape_model::sheetmetal::FlatPattern;
use std::sync::Arc;

/// Flat patterns of the sheet metal parts of one regeneration result.
pub(super) struct FlatCache {
    /// Identity of the regeneration context the patterns came from.
    context: usize,
    pub parts: Vec<(BodyId, Result<Arc<FlatPattern>, String>)>,
}

fn context_key(c: &Arc<RegenContext>) -> usize {
    Arc::as_ptr(c) as usize
}

impl PartStudioView {
    pub(super) fn has_sheet_metal(&self) -> bool {
        self.context.as_ref().is_some_and(|c| !c.sheet_metal.is_empty())
    }

    /// Whether the studio has a hole (an unsuppressed Hole made one): live shows the Hole table button only then
    /// (HOLE-TABLE-BUTTON).
    pub(super) fn has_holes(&self) -> bool {
        self.context.as_ref().is_some_and(|c| !c.holes.is_empty())
    }

    pub(super) fn has_frames(&self) -> bool {
        self.context.as_ref().is_some_and(|c| !c.frame_members.is_empty())
    }

    /// Unfold sheet metal parts in the background when the model changed.
    pub(super) fn ensure_flats(&mut self, cx: &mut Context<Self>) {
        let Some(ctx) = self.context.clone() else { return };
        let key = context_key(&ctx);
        if self.flat_cache.as_ref().is_some_and(|c| c.context == key) || self.flat_task.as_ref().is_some_and(|(k, _)| *k == key) {
            return;
        }
        let task = cx.spawn(async move |this, cx| {
            let ctx2 = ctx.clone();
            let parts = cx
                .background_spawn(async move {
                    ctx2.sheet_metal.keys().map(|id| (id.clone(), noshape_model::sheetmetal::flat_pattern_for(&ctx2, id).map(Arc::new))).collect::<Vec<_>>()
                })
                .await;
            let _ = this.update(cx, |this: &mut Self, cx| {
                this.flat_cache = Some(FlatCache { context: key, parts });
                this.flat_task = None;
                cx.notify();
            });
        });
        self.flat_task = Some((key, task));
    }

    pub(super) fn render_cut_list_panel(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let Some(ctx) = self.context.clone() else { return div().into_any_element() };
        let rows = noshape_model::regen::features::frames::cut_list(&ctx, None);
        if rows.is_empty() {
            return div().text_xs().text_color(pal.text_faint).child("No frame members. Create a Frame feature to get a cut list.").into_any_element();
        }
        let units = self.doc.read(cx).session.doc.settings.units;
        let len = |v: f64| noshape_units::format_quantity(noshape_units::Quantity::new(v, noshape_units::Dim::LENGTH), &units);
        let ang = |a: Option<f64>| a.map(|a| format!("{}°", noshape_units::to_fixed(a.to_degrees(), 1))).unwrap_or_else(|| "—".into());
        let header = h_flex()
            .text_xs()
            .text_color(pal.text_faint)
            .child(div().w(px(22.)).child("#"))
            .child(div().w(px(28.)).child("Qty"))
            .child(div().flex_1().child("Profile"))
            .child(div().w(px(70.)).child("Length"))
            .child(div().w(px(80.)).child("Angles"));
        let mut out = vec![header.into_any_element()];
        let total: f64 = rows.iter().filter_map(|r| r.length.map(|l| l * r.qty as f64)).sum();
        for r in rows {
            let bodies = r.bodies.clone();
            out.push(
                h_flex()
                    .id(SharedString::from(format!("cut-{}", r.item)))
                    .py_0p5()
                    .rounded_sm()
                    .text_xs()
                    .text_color(pal.text)
                    .hover(|d| d.bg(pal.panel_hover))
                    .child(div().w(px(22.)).text_color(pal.text_muted).child(r.item.to_string()))
                    .child(div().w(px(28.)).child(r.qty.to_string()))
                    .child(div().flex_1().truncate().child(if r.description.is_empty() { r.standard.clone() } else { format!("{} {}", r.standard, r.description) }))
                    .child(div().w(px(70.)).child(r.length.map(len).unwrap_or_else(|| "—".into())))
                    .child(div().w(px(80.)).child(format!("{} / {}", ang(r.angle1), ang(r.angle2))))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        // Select the members of this row.
                        this.selection = bodies.iter().cloned().map(noshape_model::reference::Reference::Body).collect();
                        this.refresh_scene(false, cx);
                        cx.notify();
                    }))
                    .into_any_element(),
            );
        }
        out.push(h_flex().pt_1().justify_between().text_xs().child(div().text_color(pal.text_muted).child("Total length")).child(div().child(len(total))).into_any_element());
        v_flex().gap_px().children(out).into_any_element()
    }
}
