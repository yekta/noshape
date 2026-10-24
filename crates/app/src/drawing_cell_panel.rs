//! Drawing editor: Onshape's floating "Cell" panel for selected cells of a
//! general table (live drawing/1143–1148, dom/drawing-custom-table-cell-panel.txt,
//! behaviours.md "Note panel popups and values" (6)): 590 px, "Cell" with ✕
//! only. Row 1: Insert row above / below, Insert column left / right, Remove
//! rows, Remove columns, Duplicate cells ▾ | Size columns equally, Size rows
//! equally, Merge cells ▾, Unmerge cells (the last four disabled for one
//! cell). Row 2: Font, Text height, B I U S, Line spacing (disabled),
//! Horizontal alignment ▾ (Left, Center, Right), Vertical alignment ▾ (Top,
//! Middle, Bottom). Changes apply at once.

use crate::drawing_cells::CellSel;
use crate::drawing_view::*;
use crate::os_ui::OsDropdownExt as _;
use crate::theme::p;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::drawing::sheet::{DRAWING_DEFAULT_FONT, FONT_FAMILIES};
use noshape_model::drawing::{CellAlign, CellFormat, CellVAlign, Table, TableKind};

/// The open Cell panel: the cells it was opened for and its Text height box.
pub struct CellPanel {
    pub sel: CellSel,
    pub height: Entity<InputState>,
}

type TableEdit = std::rc::Rc<dyn Fn(&mut Table)>;
type FormatEdit = std::rc::Rc<dyn Fn(&mut CellFormat)>;

impl DrawingEditor {
    /// The general table cells the Cell panel shows for (no other panel or
    /// tool open).
    fn cell_panel_sel(&self) -> Option<CellSel> {
        let cs = self.cell_sel?;
        let t = self.current_sheet().table(cs.table)?;
        (matches!(t.kind, TableKind::General { .. }) && self.panel.is_none() && matches!(self.tool, Tool::Select)).then_some(cs)
    }

    /// Create / drop the panel's state as the cell selection changes (from render).
    pub fn sync_cell_panel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let want = self.cell_panel_sel();
        if want == self.cell_panel.as_ref().map(|p| p.sel) {
            return;
        }
        self.cell_panel = want.and_then(|sel| {
            let t = self.current_sheet().table(sel.table)?;
            let f = t.shown_cell_format(sel.a.0.min(sel.b.0), sel.a.1.min(sel.b.1));
            let h = f.height.filter(|v| *v > 0.0).unwrap_or(t.text_height);
            let text = crate::drawing_note::note_height_text(h, self.drawing.units_mm);
            let height = cx.new(|cx| InputState::new(window, cx).select_on_focus(false).revert_on_escape(false).default_value(text));
            let sub = cx.subscribe_in(&height, window, |this: &mut Self, st, ev: &InputEvent, _, cx| {
                if let InputEvent::PressEnter { .. } = ev
                    && let Some(mm) = crate::drawing_note::parse_note_height(&st.read(cx).value(), this.drawing.units_mm)
                {
                    this.edit_cell_formats("Text height", std::rc::Rc::new(move |f: &mut CellFormat| f.height = Some(mm)), cx);
                }
            });
            self._subs.push(sub);
            Some(CellPanel { sel, height })
        });
    }

    /// Apply `f` to the format of every selected cell.
    fn edit_cell_formats(&mut self, label: &str, f: FormatEdit, cx: &mut Context<Self>) {
        let Some(cs) = self.cell_panel_sel() else { return };
        self.edit_table(cs.table, label, cx, move |t| {
            for r in cs.rows() {
                for c in cs.cols() {
                    let mut fmt = t.shown_cell_format(r, c);
                    f(&mut fmt);
                    t.set_cell_format(r, c, fmt);
                }
            }
        });
        cx.notify();
    }

    /// A table edit from the panel's first row. The selected cells stay
    /// selected, moved by `shift` (rows, columns) when cells were inserted
    /// before them; None: they were removed.
    fn cell_table_edit(&mut self, label: &str, f: TableEdit, shift: Option<(usize, usize)>, cx: &mut Context<Self>) {
        let Some(cs) = self.cell_panel_sel() else { return };
        self.cell_sel = shift.map(|(dr, dc)| CellSel { table: cs.table, a: (cs.a.0 + dr, cs.a.1 + dc), b: (cs.b.0 + dr, cs.b.1 + dc) });
        self.edit_table(cs.table, label, cx, move |t| f(t));
        cx.notify();
    }

    /// The floating Cell panel above the table.
    pub fn render_cell_panel(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let cs = self.cell_panel_sel()?;
        let height = self.cell_panel.as_ref().filter(|p| p.sel == cs)?.height.clone();
        let t = self.current_sheet().table(cs.table)?.clone();
        let l = self.built.as_ref()?.table_layout(cs.table)?.clone();
        let pal = p();
        let (r0, r1, c0, c1) = (*cs.rows().start(), *cs.rows().end(), *cs.cols().start(), *cs.cols().end());
        let single = r0 == r1 && c0 == c1;
        let first = t.shown_cell_format(r0, c0);
        let all = |p: &dyn Fn(&CellFormat) -> bool| cs.rows().all(|r| cs.cols().all(|c| p(&t.shown_cell_format(r, c))));
        let (bold, italic, underline, strike) = (all(&|f| f.bold), all(&|f| f.italic), all(&|f| f.underline), all(&|f| f.strikethrough));
        // Live 1143: the panel's left edge by the table's, above it.
        let tl = self.sheet_to_local([l.min()[0], l.max()[1]]);
        let bottom = (self.size_px()[1] - tl[1] + 60.0) as f32;
        let left = (tl[0] - 7.0).max(4.0) as f32;
        let ed = cx.entity();

        let icon_btn = |id: &'static str, icon: &'static str, tip: &'static str, on: bool, enabled: bool, cx: &App| {
            div()
                .id(id)
                .size(px(30.))
                .p(px(1.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(2.))
                .border_1()
                .border_color(if on { Hsla::from(gpui_kit::rgb(0x9aa5af)) } else { gpui_kit::transparent_black() })
                .when(on, |d| d.bg(gpui_kit::rgb(0xcfdbe5)))
                .opacity(if enabled { 0.95 } else { 0.3 })
                .when(enabled && !on, |d| d.cursor_pointer().hover(|d| d.bg(pal.row_hover)))
                .tooltip(crate::ui::tooltip(tip))
                .child(crate::assets::icon(icon, 20., cx))
        };
        let popup_btn = |id: &'static str, icon: &'static str, tip: &'static str, enabled: bool, cx: &App| {
            div()
                .id(id)
                .w(px(45.))
                .h(px(30.))
                .p(px(1.))
                .flex()
                .items_center()
                .rounded(px(2.))
                .opacity(if enabled { 0.95 } else { 0.3 })
                .when(enabled, |d| d.cursor_pointer().hover(|d| d.bg(pal.row_hover)))
                .tooltip(crate::ui::tooltip(tip))
                .child(div().size(px(28.)).flex().items_center().justify_center().child(crate::assets::icon(icon, 20., cx)))
                .child(crate::ui::glyph("drawing-tool-caret", 12., gpui_kit::rgb(0x000000)))
        };
        let table_op = |label: &'static str, f: TableEdit, shift: Option<(usize, usize)>| {
            let ed = ed.clone();
            move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                let f = f.clone();
                ed.update(cx, |t, cx| t.cell_table_edit(label, f, shift, cx));
            }
        };
        let menu = |items: Vec<(&'static str, bool, bool, TableEdit)>| {
            let ed = ed.clone();
            move |mut m: crate::os_menu::OsMenu, _: &mut Window, _: &mut Context<crate::os_menu::OsMenu>| {
                for (label, on, enabled, f) in &items {
                    let (e, f, label) = (ed.clone(), f.clone(), *label);
                    m = m.item(crate::os_menu::OsMenuItem::new(label).checked(*on).disabled(!*enabled).on_click(move |_, _, cx| {
                        let f = f.clone();
                        e.update(cx, |t, cx| t.cell_table_edit(label, f, Some((0, 0)), cx))
                    }));
                }
                m
            }
        };
        let fmt_menu = |items: Vec<(String, bool, FormatEdit)>| {
            let ed = ed.clone();
            move |mut m: crate::os_menu::OsMenu, _: &mut Window, _: &mut Context<crate::os_menu::OsMenu>| {
                for (label, on, f) in &items {
                    let (e, f) = (ed.clone(), f.clone());
                    m = m.item(crate::os_menu::OsMenuItem::new(label.clone()).checked(*on).on_click(move |_, _, cx| {
                        let f = f.clone();
                        e.update(cx, |t, cx| t.edit_cell_formats("Format cells", f, cx))
                    }));
                }
                m
            }
        };

        // Row 1: rows / columns.
        let widths: Vec<f64> = l.xs.windows(2).map(|w| w[1] - w[0]).collect();
        let merged = t.merge_at(r0, c0).is_some();
        let row1 = h_flex()
            .h(px(30.))
            .pl(px(1.))
            .gap(px(2.))
            .child(icon_btn("cp-row-above", "cell-insert-row-above", "Insert row above", false, true, cx).on_click(table_op("Insert row above", std::rc::Rc::new(move |t: &mut Table| t.insert_row(r0)), Some((1, 0)))))
            .child(icon_btn("cp-row-below", "cell-insert-row-below", "Insert row below", false, true, cx).on_click(table_op("Insert row below", std::rc::Rc::new(move |t: &mut Table| t.insert_row(r1 + 1)), Some((0, 0)))))
            .child(icon_btn("cp-col-left", "cell-insert-col-left", "Insert column left", false, true, cx).on_click(table_op("Insert column left", std::rc::Rc::new(move |t: &mut Table| t.insert_col(c0)), Some((0, 1)))))
            .child(icon_btn("cp-col-right", "cell-insert-col-right", "Insert column right", false, true, cx).on_click(table_op("Insert column right", std::rc::Rc::new(move |t: &mut Table| t.insert_col(c1 + 1)), Some((0, 0)))))
            .child(icon_btn("cp-rm-rows", "cell-remove-rows", "Remove rows", false, true, cx).on_click(table_op(
                "Remove rows",
                std::rc::Rc::new(move |t: &mut Table| {
                    for r in (r0..=r1).rev() {
                        t.delete_row(r);
                    }
                }),
                None,
            )))
            .child(icon_btn("cp-rm-cols", "cell-remove-cols", "Remove columns", false, true, cx).on_click(table_op(
                "Remove columns",
                std::rc::Rc::new(move |t: &mut Table| {
                    for c in (c0..=c1).rev() {
                        t.delete_col(c);
                    }
                }),
                None,
            )))
            .child(popup_btn("cp-dup", "cell-duplicate", "Duplicate cells", true, cx).os_dropdown_menu(
                "cp-dup-dd",
                false,
                menu(vec![
                    ("Columns", false, true, std::rc::Rc::new(move |t: &mut Table| t.duplicate_cols(c0, c1)) as TableEdit),
                    ("Rows", false, true, std::rc::Rc::new(move |t: &mut Table| t.duplicate_rows(r0, r1)) as TableEdit),
                ]),
            ))
            // Live 1143: a gap (no rule) before the size / merge group.
            .child(div().w(px(6.)))
            .child({
                let b = icon_btn("cp-eq-cols", "cell-size-cols-equal", "Size columns equally", false, !single, cx);
                if single {
                    b.into_any_element()
                } else {
                    let w = widths.clone();
                    b.on_click(table_op(
                        "Size columns equally",
                        std::rc::Rc::new(move |t: &mut Table| {
                            let mut all = w.clone();
                            let avg = all[c0..=c1.min(all.len() - 1)].iter().sum::<f64>() / (c1 - c0 + 1) as f64;
                            for x in all.iter_mut().take(c1 + 1).skip(c0) {
                                *x = avg;
                            }
                            t.col_widths = all;
                        }),
                        Some((0, 0)),
                    ))
                    .into_any_element()
                }
            })
            .child({
                let b = icon_btn("cp-eq-rows", "cell-size-rows-equal", "Size rows equally", false, !single, cx);
                if single {
                    b.into_any_element()
                } else {
                    let tallest = l.ys.windows(2).skip(l.title as usize).map(|w| w[0] - w[1]).fold(0.0, f64::max);
                    b.on_click(table_op("Size rows equally", std::rc::Rc::new(move |t: &mut Table| t.row_height = tallest), Some((0, 0)))).into_any_element()
                }
            })
            .child({
                let b = popup_btn("cp-merge", "cell-merge", "Merge cells", !single, cx);
                if single {
                    b.into_any_element()
                } else {
                    b.os_dropdown_menu(
                        "cp-merge-dd",
                        false,
                        menu(vec![
                            (
                                "All",
                                false,
                                true,
                                std::rc::Rc::new(move |t: &mut Table| {
                                    t.merge_cells((r0, c0), (r1, c1));
                                }) as TableEdit,
                            ),
                            (
                                "Horizontal",
                                false,
                                true,
                                std::rc::Rc::new(move |t: &mut Table| {
                                    for r in r0..=r1 {
                                        t.merge_cells((r, c0), (r, c1));
                                    }
                                }) as TableEdit,
                            ),
                            (
                                "Vertical",
                                false,
                                true,
                                std::rc::Rc::new(move |t: &mut Table| {
                                    for c in c0..=c1 {
                                        t.merge_cells((r0, c), (r1, c));
                                    }
                                }) as TableEdit,
                            ),
                        ]),
                    )
                    .into_any_element()
                }
            })
            .child({
                let b = icon_btn("cp-unmerge", "cell-unmerge", "Unmerge cells", false, merged, cx);
                if merged {
                    b.on_click(table_op(
                        "Unmerge cells",
                        std::rc::Rc::new(move |t: &mut Table| {
                            t.unmerge(r0, c0);
                        }),
                        Some((0, 0)),
                    ))
                    .into_any_element()
                } else {
                    b.into_any_element()
                }
            });

        // Row 2: font and text.
        let field = |w: f32| div().w(px(w)).h(px(24.)).bg(crate::theme::light().panel).border_1().border_color(gpui_kit::rgb(0xd3d7d9)).rounded(px(2.));
        let family = first.font.clone().unwrap_or_else(|| DRAWING_DEFAULT_FONT.to_string());
        let fonts: Vec<(String, bool, FormatEdit)> = FONT_FAMILIES
            .iter()
            .map(|&fam| {
                let set = (fam != DRAWING_DEFAULT_FONT).then(|| fam.to_string());
                (fam.to_string(), family == fam, std::rc::Rc::new(move |f: &mut CellFormat| f.font = set.clone()) as FormatEdit)
            })
            .collect();
        let font_box = field(238.)
            .id("cp-font")
            .pl(px(6.))
            .pr(px(6.))
            .flex()
            .items_center()
            .cursor_pointer()
            .text_size(px(13.33))
            .text_color(gpui_kit::rgb(0x555555))
            .tooltip(crate::ui::tooltip("Font"))
            .child(div().flex_1().whitespace_nowrap().child(family.clone()))
            .child(crate::ui::glyph("ui-caret-down", 9.6, gpui_kit::rgb(0x555555)));
        let toggle = |id: &'static str, icon: &'static str, tip: &'static str, on: bool, f: fn(&mut CellFormat, bool), cx: &mut Context<Self>| {
            icon_btn(id, icon, tip, on, true, cx).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.edit_cell_formats(tip, std::rc::Rc::new(move |fm: &mut CellFormat| f(fm, !on)), cx)))
        };
        let aligns = [(CellAlign::Left, "Left", "note-align-left"), (CellAlign::Center, "Center", "note-align-center"), (CellAlign::Right, "Right", "note-align-right")];
        let align_icon = aligns.iter().find(|a| a.0 == first.align).map(|a| a.2).unwrap_or("note-align-left");
        let align: Vec<(String, bool, FormatEdit)> = aligns.iter().map(|&(a, l, _)| (l.to_string(), first.align == a, std::rc::Rc::new(move |f: &mut CellFormat| f.align = a) as FormatEdit)).collect();
        let valigns = [(CellVAlign::Top, "Top", "note-valign-top"), (CellVAlign::Middle, "Middle", "note-valign-middle"), (CellVAlign::Bottom, "Bottom", "note-valign-bottom")];
        let valign_icon = valigns.iter().find(|v| v.0 == first.valign).map(|v| v.2).unwrap_or("note-valign-middle");
        let valign: Vec<(String, bool, FormatEdit)> = valigns.iter().map(|&(v, l, _)| (l.to_string(), first.valign == v, std::rc::Rc::new(move |f: &mut CellFormat| f.valign = v) as FormatEdit)).collect();
        let row2 = h_flex()
            .h(px(30.))
            .ml(px(-1.))
            .gap(px(1.))
            .child(font_box.os_dropdown_menu("cp-font-dd", true, fmt_menu(fonts)))
            .child(field(75.).flex().items_center().pr(px(4.)).child(div().flex_1().child(Input::new(&height).appearance(false).text_size(px(13.33)).text_color(gpui_kit::rgb(0x555555)).xsmall())).child(crate::ui::glyph("ui-caret-down", 9.6, gpui_kit::rgb(0x555555))))
            .child(toggle("cp-b", "note-bold", "Bold", bold, |f, v| f.bold = v, cx))
            .child(toggle("cp-i", "note-italic", "Italic", italic, |f, v| f.italic = v, cx))
            .child(toggle("cp-u", "note-underline", "Underline", underline, |f, v| f.underline = v, cx))
            .child(toggle("cp-s", "note-strike", "Strikethrough", strike, |f, v| f.strikethrough = v, cx))
            .child(popup_btn("cp-spacing", "note-line-spacing", "Line spacing", false, cx))
            .child(popup_btn("cp-align", align_icon, "Horizontal alignment", true, cx).os_dropdown_menu("cp-align-dd", false, fmt_menu(align)))
            .child(popup_btn("cp-valign", valign_icon, "Vertical alignment", true, cx).os_dropdown_menu("cp-valign-dd", false, fmt_menu(valign)));

        Some(
            v_flex()
                .id("dr-cell-panel")
                .absolute()
                .left(px(left))
                .bottom(px(bottom))
                .w(px(590.))
                .bg(pal.dialog_bg)
                .shadow(crate::drawing_chrome::dialog_shadow())
                .text_color(gpui_kit::rgb(0x333333))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    h_flex()
                        .h(px(28.))
                        .pl(px(11.))
                        .border_b_1()
                        .border_color(gpui_kit::rgb(0xd3d7d9))
                        .child(div().flex_1().text_size(px(13.)).font_weight(FontWeight::MEDIUM).child("Cell"))
                        .child(div().id("cp-close").w(px(28.)).h(px(27.)).flex().items_center().justify_center().cursor_pointer().child(crate::drawing_chrome::cancel_x(cx)).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.cell_sel = None;
                            cx.notify();
                        }))),
                )
                .child(v_flex().p(px(4.)).text_size(px(12.)).child(row1).child(row2))
                .into_any_element(),
        )
    }
}
