//! Fabrication panels: the sheet metal flat patterns (unfolded in the
//! background for the Sheet metal table and flat view) and the frame cut list.

use super::PartStudioView;
use crate::theme::p;
use gpui_kit::component::v_flex;
use gpui_kit::*;
use noshape_model::custom::table::{CustomTable, TableCell, TableColumn, TableRow};
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
        // Live (behaviours "psui (2220)", rn-1222-1223 sticky-800.png): a table per Cut list feature, named as it, of
        // its composite part's members (with no Cut list feature yet, one over every frame member).
        let mut lists: Vec<(String, Option<Vec<BodyId>>)> = self
            .studio(cx)
            .map(|s| {
                s.features
                    .iter()
                    .filter(|f| f.type_id == "cutlist" && !f.suppressed)
                    .filter_map(|f| Some((f.name.clone(), Some(ctx.bodies.iter().find(|b| b.id.feature == f.id)?.composite.as_ref()?.members.clone()))))
                    .collect()
            })
            .unwrap_or_default();
        if lists.is_empty() {
            lists.push(("Cut list".into(), None));
        }
        let units = self.doc.read(cx).session.doc.settings.units;
        let tables: Vec<CustomTable> = lists
            .into_iter()
            .filter_map(|(title, members)| {
                let rows = noshape_model::regen::features::frames::cut_list(&ctx, members.as_deref());
                (!rows.is_empty()).then(|| cut_list_table(title, rows, &units))
            })
            .collect();
        if tables.is_empty() {
            return div().text_xs().text_color(pal.text_faint).child("No frame members. Create a Frame feature to get a cut list.").into_any_element();
        }
        v_flex().mx(px(-5.)).mt(px(-5.)).children(tables.iter().enumerate().map(|(i, t)| self.table_section(i, t, cx))).into_any_element()
    }
}

/// A cut list as the tables panel shows it: Item | Qty | Standard | Description | Length | Angle 1 | Angle 2 (live
/// behaviours "psui (2220)": "99.698 mm", "0 deg"), each row lighting its members.
fn cut_list_table(title: String, rows: Vec<noshape_model::regen::features::frames::CutListRow>, units: &noshape_units::UnitPrefs) -> CustomTable {
    let q = |v: f64, dim| noshape_units::format_quantity(noshape_units::Quantity::new(v, dim), units);
    let or_dash = |v: Option<String>| v.unwrap_or_else(|| "—".into());
    let columns = ["Item", "Qty", "Standard", "Description", "Length", "Angle 1", "Angle 2"];
    CustomTable {
        title,
        columns: columns.iter().map(|c| TableColumn { id: c.to_string(), name: c.to_string(), alignment: None }).collect(),
        rows: rows
            .into_iter()
            .map(|r| {
                let values = [
                    r.item.to_string(),
                    r.qty.to_string(),
                    r.standard,
                    r.description,
                    or_dash(r.length.map(|l| q(l, noshape_units::Dim::LENGTH))),
                    or_dash(r.angle1.map(|a| q(a, noshape_units::Dim::ANGLE))),
                    or_dash(r.angle2.map(|a| q(a, noshape_units::Dim::ANGLE))),
                ];
                TableRow {
                    cells: columns.iter().zip(values).map(|(c, text)| (c.to_string(), TableCell { text, ..Default::default() })).collect(),
                    entities: r.bodies.into_iter().map(noshape_model::reference::Reference::Body).collect(),
                }
            })
            .collect(),
        ..Default::default()
    }
}
