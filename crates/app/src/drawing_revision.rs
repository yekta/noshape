//! Drawing editor: Onshape's Revision table dialog (while the Revision
//! table tool is active, and from "Revision table properties…"):
//! Multi-sheet (Linked / See sheet 1), Visible rows, Order (Top to bottom /
//! Bottom to top), "Hide revision callouts no longer shown in the table" and
//! the fixed corner; then a click places the table. Rows come from the
//! document's release log ([`revisions`]).

use crate::drawing_view::*;
use crate::os_ui::OsDropdownExt as _;
use crate::theme::p;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::drawing::tables::RevisionRow;
use noshape_model::drawing::build::PickTarget;
use noshape_model::drawing::{Annotation, AnnotationKind, Table, TableCorner, TableKind};

/// Revision table dialog state.
pub struct RevisionDlg {
    /// The table being edited (None while inserting one).
    pub table: Option<u64>,
    /// The table's settings before editing (restored by ✕).
    original: Option<(TableKind, TableCorner)>,
    pub see_sheet1: bool,
    pub visible_rows: u32,
    pub newest_top: bool,
    pub hide_callouts: bool,
    pub corner: TableCorner,
    rows_input: Entity<InputState>,
    _sub: Subscription,
}

/// The drawing's revisions (oldest first) from the document's release log
/// (store meta "releases", kept by the release workflow).
pub fn revisions(doc: &crate::doc_model::DocModel, tab: noshape_model::document::TabId) -> Vec<RevisionRow> {
    noshape_model::drawing::tables::revision_rows(&crate::release_dialog::release_log(doc), tab)
}

const CORNERS: [(TableCorner, &str); 4] = [(TableCorner::TopLeft, "Top left"), (TableCorner::TopRight, "Top right"), (TableCorner::BottomLeft, "Bottom left"), (TableCorner::BottomRight, "Bottom right")];

impl DrawingEditor {
    fn new_revision_dlg(&mut self, table: Option<u64>, window: &mut Window, cx: &mut Context<Self>) -> RevisionDlg {
        let t = table.and_then(|id| self.drawing.sheets[self.sheet].table(id).cloned());
        let original = t.as_ref().map(|t| (t.kind.clone(), t.corner));
        let (see_sheet1, visible_rows, newest_top, hide_callouts) = match t.as_ref().map(|t| &t.kind) {
            Some(TableKind::Revision { see_sheet1, visible_rows, newest_top, hide_callouts, .. }) => (*see_sheet1, *visible_rows, *newest_top, *hide_callouts),
            _ => (false, 5, false, false),
        };
        let corner = t.map(|t| t.corner).unwrap_or(TableCorner::TopRight);
        let st = cx.new(|cx| InputState::new(window, cx).default_value(visible_rows.to_string()));
        let sub = cx.subscribe(&st, move |this: &mut Self, st, ev: &InputEvent, cx| {
            if matches!(ev, InputEvent::Change)
                && let Ok(n) = st.read(cx).value().trim().parse::<u32>()
                && (1..=100).contains(&n)
                && let Some(d) = &mut this.revision_dlg
            {
                d.visible_rows = n;
                this.apply_revision_dlg(cx);
            }
        });
        RevisionDlg { table, original, see_sheet1, visible_rows, newest_top, hide_callouts, corner, rows_input: st, _sub: sub }
    }

    /// Create / drop the insert dialog with the Revision table tool (called
    /// from render).
    pub fn sync_revision_dlg(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let active = matches!(self.tool, Tool::Table { kind: TableTool::Revision, .. });
        let inserting = self.revision_dlg.as_ref().is_some_and(|d| d.table.is_none());
        if active && self.revision_dlg.is_none() {
            self.revision_dlg = Some(self.new_revision_dlg(None, window, cx));
        } else if !active && inserting {
            self.revision_dlg = None;
        }
    }

    /// Right-click › Revision table properties…
    pub fn open_revision_props(&mut self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        self.tool = Tool::Select;
        self.revision_dlg = Some(self.new_revision_dlg(Some(id), window, cx));
        cx.notify();
    }

    /// The dialog's settings as a table kind (keeping edited texts).
    fn revision_kind(d: &RevisionDlg, old: Option<&TableKind>) -> TableKind {
        let mut k = old.cloned().filter(|k| matches!(k, TableKind::Revision { .. })).unwrap_or_else(TableKind::revision);
        if let TableKind::Revision { see_sheet1, visible_rows, newest_top, hide_callouts, .. } = &mut k {
            (*see_sheet1, *visible_rows, *newest_top, *hide_callouts) = (d.see_sheet1, d.visible_rows, d.newest_top, d.hide_callouts);
        }
        k
    }

    /// Editing a table: apply the dialog live.
    fn apply_revision_dlg(&mut self, cx: &mut Context<Self>) {
        if let Some(d) = &self.revision_dlg
            && let Some(id) = d.table
        {
            let (kind, corner) = (Self::revision_kind(d, d.original.as_ref().map(|o| &o.0)), d.corner);
            self.edit_table(id, "Revision table properties", cx, |t| {
                t.kind = kind;
                t.corner = corner;
            });
        }
        cx.notify();
    }

    /// Revision table tool click: place the table with its fixed corner at `c`.
    pub fn place_revision_table(&mut self, c: [f64; 2], cx: &mut Context<Self>) {
        let Some(d) = &self.revision_dlg else { return };
        let id = self.drawing.sheets[self.sheet].next_table_id();
        let mut t = Table::new(id, Self::revision_kind(d, None), c);
        t.corner = d.corner;
        self.edit_sheet("Revision table", cx, |s| s.tables.push(t));
        self.revision_dlg = None;
        self.tool = Tool::Select;
        self.selection = vec![Sel::Table(id)];
        cx.notify();
    }

    /// The revision new callouts get: the latest (pending first), else "A".
    pub fn latest_revision(&self) -> String {
        self.info.revisions.last().map(|r| r.revision.clone()).unwrap_or_else(|| noshape_model::release::next_label(None))
    }

    pub fn place_revision_callout(&mut self, revision: String, c: [f64; 2], cx: &mut Context<Self>) {
        let id = self.drawing.sheets[self.sheet].next_annotation_id();
        self.edit_sheet("Revision callout", cx, |s| s.annotations.push(Annotation { id, kind: AnnotationKind::RevisionCallout { anchor: c, revision, group: None }, position: c, ..Default::default() }));
        self.tool = Tool::Select;
        self.selection = vec![Sel::Annotation(id)];
        cx.notify();
    }

    /// Dropping a revision callout onto a dimension's text groups it with
    /// the dimension: it then moves with it (its anchor becomes the offset
    /// from the upper right of the dimension's text).
    pub fn group_callout_on_drop(&self, orig: &Annotation, p: &mut noshape_model::drawing::Drawing) {
        let balloon = matches!(orig.kind, AnnotationKind::Balloon { .. }) && orig.group.is_none();
        if !balloon && !matches!(orig.kind, AnnotationKind::RevisionCallout { group: None, .. }) {
            return;
        }
        let Some(b) = &self.built else { return };
        let Some(sheet) = p.sheets.get_mut(self.sheet) else { return };
        let Some(at) = sheet.annotations.iter().find(|a| a.id == orig.id).and_then(|a| match a.kind {
            AnnotationKind::RevisionCallout { anchor, .. } => Some(anchor),
            AnnotationKind::Balloon { .. } => Some(a.position),
            _ => None,
        }) else {
            return;
        };
        // The dimension whose text box (2 mm margin) contains the drop point.
        let hit = sheet.annotations.iter().filter(|a| a.kind.is_dimension()).find_map(|d| {
            let picks: Vec<u32> = b.picks.iter().enumerate().filter(|(_, t)| matches!(t, PickTarget::Annotation(x) if *x == d.id)).map(|(i, _)| i as u32).collect();
            let pts: Vec<[f64; 2]> = b.geometry.texts.iter().filter(|t| picks.contains(&t.pick)).flat_map(noshape_model::drawing::font::text_box).collect();
            if pts.is_empty() {
                return None;
            }
            let (x0, x1) = (pts.iter().map(|q| q[0]).fold(f64::MAX, f64::min), pts.iter().map(|q| q[0]).fold(f64::MIN, f64::max));
            let (y0, y1) = (pts.iter().map(|q| q[1]).fold(f64::MAX, f64::min), pts.iter().map(|q| q[1]).fold(f64::MIN, f64::max));
            (at[0] >= x0 - 2.0 && at[0] <= x1 + 2.0 && at[1] >= y0 - 2.0 && at[1] <= y1 + 2.0).then_some((d.id, [x1, y1]))
        });
        let Some((dim, corner)) = hit else { return };
        if balloon {
            // A callout: its position becomes the offset from the dimension.
            if let Some(a) = sheet.annotations.iter_mut().find(|a| a.id == orig.id) {
                a.position = [at[0] - corner[0], at[1] - corner[1]];
                a.group = Some(dim);
            }
            return;
        }
        if let Some(AnnotationKind::RevisionCallout { anchor, group, .. }) = sheet.annotations.iter_mut().find(|a| a.id == orig.id).map(|a| &mut a.kind) {
            // It stays where it was dropped, now relative to the dimension.
            *anchor = [at[0] - corner[0], at[1] - corner[1]];
            *group = Some(dim);
        }
    }

    /// Right-click on a revision table (`cell`: the cell under the cursor).
    pub fn revision_table_entries(&self, id: u64, cell: Option<(usize, usize)>) -> Vec<MenuEntry> {
        let mut out = vec![MenuEntry::item("Revision table properties...", move |t, w, cx| t.open_revision_props(id, w, cx))];
        let Some(t) = self.drawing.sheets[self.sheet].table(id) else { return out };
        // The revision of the row under the cursor (row 0 is the header).
        let data = noshape_model::drawing::tables::table_data_with(&self.drawing, t, None, None, None, &self.info, &|s| s.to_string());
        let row_rev = cell.and_then(|(r, _)| r.checked_sub(1)).and_then(|r| data.rows.get(r)).map(|r| r[0].trim_end_matches('*').to_string());
        let rev = row_rev.clone().unwrap_or_else(|| self.latest_revision());
        out.push(MenuEntry::item("Add revision callout", move |t, _, cx| t.set_tool(Tool::RevisionCallout { revision: rev.clone() }, cx)));
        if let (Some(rev), TableKind::Revision { hidden, .. }) = (row_rev, &t.kind) {
            let is_hidden = hidden.contains(&rev);
            let label = if is_hidden { "Show revision callouts" } else { "Hide revision callouts" };
            out.push(MenuEntry::item(label, move |t, _, cx| {
                let rev = rev.clone();
                t.edit_table(id, label, cx, |t| {
                    if let TableKind::Revision { hidden, .. } = &mut t.kind {
                        if is_hidden {
                            hidden.retain(|h| *h != rev);
                        } else {
                            hidden.push(rev);
                        }
                    }
                })
            }));
        }
        out.push(MenuEntry::Separator);
        out
    }

    /// Double-click on a revision table: edit the title (`row` = `usize::MAX`),
    /// a column header (row 0) or a revision's description (its row).
    pub fn open_revision_cell(&mut self, t: &Table, window: &mut Window, cx: &mut Context<Self>) {
        let Some(l) = self.built.as_ref().and_then(|b| b.table_layout(t.id)).cloned() else { return };
        let data = noshape_model::drawing::tables::table_data_with(&self.drawing, t, None, None, None, &self.info, &|s| s.to_string());
        let in_title = l.title && self.cursor[1] <= l.ys[0] && self.cursor[1] >= l.ys[1];
        let (row, col, text) = match (in_title, l.cell_at(self.cursor)) {
            (true, _) => (usize::MAX, 0, data.title.clone().unwrap_or_default()),
            (false, Some((0, c))) => (0, c, data.header.as_ref().and_then(|h| h.get(c)).cloned().unwrap_or_default()),
            (false, Some((r, _))) => (r, 1, data.rows.get(r - 1).and_then(|x| x.get(1)).cloned().unwrap_or_default()),
            (false, None) => return,
        };
        let input = self.textarea(&text, 2, window, cx);
        self.panel = Some(Panel::Cell { table: t.id, row, col, input, note: None });
        cx.notify();
    }

    /// Apply a revision table cell edit (see [`Self::open_revision_cell`]).
    pub fn apply_revision_cell(&mut self, table: u64, row: usize, col: usize, text: String, cx: &mut Context<Self>) {
        // The row's revision (descriptions are kept per revision).
        let rev = self.drawing.sheets[self.sheet].table(table).and_then(|t| {
            let data = noshape_model::drawing::tables::table_data_with(&self.drawing, t, None, None, None, &self.info, &|s| s.to_string());
            data.rows.get(row.wrapping_sub(1)).map(|r| r[0].trim_end_matches('*').to_string())
        });
        self.edit_table(table, "Edit revision table", cx, |t| {
            let TableKind::Revision { title, headers, descriptions, .. } = &mut t.kind else { return };
            match row {
                usize::MAX => *title = Some(text),
                0 => {
                    if headers.len() <= col {
                        headers.resize(col + 1, None);
                    }
                    headers[col] = Some(text);
                }
                _ => {
                    if let Some(rev) = rev {
                        descriptions.retain(|(r, _)| *r != rev);
                        descriptions.push((rev, text));
                    }
                }
            }
            t.col_widths.clear();
        });
    }

    fn close_revision_dlg(&mut self, accept: bool, cx: &mut Context<Self>) {
        let Some(d) = self.revision_dlg.take() else { return };
        match (d.table, d.original) {
            (Some(id), Some((kind, corner))) if !accept => self.edit_table(id, "Revision table properties", cx, |t| {
                t.kind = kind;
                t.corner = corner;
            }),
            (None, _) => self.tool = Tool::Select,
            _ => {}
        }
        cx.notify();
    }

    pub fn render_revision_dlg(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let d = self.revision_dlg.as_ref()?;
        let pal = p();
        let ed = cx.entity();
        let select = |id: &'static str, items: [(&'static str, bool); 2], set: fn(&mut RevisionDlg, usize)| {
            let ed = ed.clone();
            let label = items.iter().find(|i| i.1).map(|i| i.0).unwrap_or(items[0].0);
            Button::new(id).label(label).ghost().small().w(px(130.)).dropdown_caret(true).os_dropdown_menu(SharedString::from(format!("{id}-dd")), true, move |mut m, _, _| {
                for (i, (l, on)) in items.into_iter().enumerate() {
                    let e = ed.clone();
                    m = m.item(crate::os_menu::OsMenuItem::new(l).checked(on).on_click(move |_, _, cx| {
                        e.update(cx, |t, cx| {
                            if let Some(d) = &mut t.revision_dlg {
                                set(d, i);
                            }
                            t.apply_revision_dlg(cx);
                        })
                    }));
                }
                m
            })
        };
        let multi = select("rev-multi", [("Linked", !d.see_sheet1), ("See sheet 1", d.see_sheet1)], |d, i| d.see_sheet1 = i == 1);
        let order = select("rev-order", [("Top to bottom", !d.newest_top), ("Bottom to top", d.newest_top)], |d, i| d.newest_top = i == 1);
        let row = |label: &'static str, body: AnyElement| h_flex().h(px(30.)).px(px(6.)).child(div().w(px(90.)).text_size(px(12.)).child(label)).child(body).into_any_element();
        let corner = d.corner;
        let corner_btn = |k: TableCorner, name: &'static str, cx: &mut Context<Self>| {
            let on = corner == k;
            let (top, left) = (matches!(k, TableCorner::TopLeft | TableCorner::TopRight), matches!(k, TableCorner::TopLeft | TableCorner::BottomLeft));
            div()
                .id(SharedString::from(format!("rev-corner-{name}")))
                .size(px(24.))
                .relative()
                .cursor_pointer()
                .border_1()
                .border_color(if on { pal.accent } else { gpui_kit::rgb(0xb0b0b0) })
                .when(on, |e| e.bg(gpui_kit::rgb(0xe3f0fb)))
                .child(div().absolute().size(px(6.)).bg(if on { pal.accent } else { gpui_kit::rgb(0x808080) }).when(top, |e| e.top(px(2.))).when(!top, |e| e.bottom(px(2.))).when(left, |e| e.left(px(2.))).when(!left, |e| e.right(px(2.))))
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    if let Some(d) = &mut this.revision_dlg {
                        d.corner = k;
                    }
                    this.apply_revision_dlg(cx);
                }))
        };
        let corners: Vec<AnyElement> = CORNERS.iter().map(|(k, n)| corner_btn(*k, n, cx).into_any_element()).collect();
        let hide = d.hide_callouts;
        let inserting = d.table.is_none();
        let header_btn = |id: &'static str, accept: bool, cx: &mut Context<Self>| {
            div()
                .id(id)
                .w(px(28.))
                .h(px(27.))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .when(accept, |e| e.bg(gpui_kit::rgb(0x009500)).child(crate::ui::glyph("check", 18., gpui_kit::white())))
                .when(!accept, |e| e.child(crate::drawing_chrome::cancel_x(cx)))
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.close_revision_dlg(accept, cx)))
        };
        Some(
            v_flex()
                .id("dr-revision-dlg")
                .absolute()
                .top(px(DIALOG_TOP))
                .left(px(self.dialog_left()))
                .w(px(250.))
                .bg(pal.dialog_bg)
                .shadow(crate::drawing_chrome::dialog_shadow())
                .text_color(pal.text)
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    h_flex()
                        .h(px(28.))
                        .pl(px(11.))
                        .border_b_1()
                        .border_color(gpui_kit::rgb(0xd3d7d9))
                        .child(div().flex_1().text_size(px(13.)).font_weight(FontWeight::MEDIUM).child("Revision table"))
                        .when(!inserting, |h| h.child(header_btn("rev-ok", true, cx)))
                        .child(header_btn("rev-cancel", false, cx)),
                )
                .child(
                    v_flex()
                        .p(px(3.))
                        .child(row("Multi-sheet", multi.into_any_element()))
                        .child(row("Visible rows", div().w(px(130.)).child(Input::new(&d.rows_input).small()).into_any_element()))
                        .child(row("Order", order.into_any_element()))
                        .child(
                            div().px(px(6.)).py(px(4.)).child(Checkbox::new("rev-hide").label("Hide revision callouts no longer shown in the table").small().checked(hide).on_click(cx.listener(|this, v: &bool, _, cx| {
                                if let Some(d) = &mut this.revision_dlg {
                                    d.hide_callouts = *v;
                                }
                                this.apply_revision_dlg(cx);
                            }))),
                        )
                        .child(row("Fixed corner", h_flex().gap(px(4.)).children(corners).into_any_element()))
                        .when(inserting, |v| v.child(div().px(px(6.)).py(px(4.)).text_size(px(12.)).text_color(pal.text_muted).child("Click in the drawing to place the table"))),
                )
                .into_any_element(),
        )
    }
}
