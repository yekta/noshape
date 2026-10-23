//! The BOM Table panel (right strip › "BOM Table"; live assembly/181–187,
//! dom/assembly-panel-bom.txt, assembly-bom-menus.txt): the 712 px docked
//! pane titled "Bill of materials", a `#eaeaea` header row with "BOM type:"
//! ▾ Flattened / Structured, "Add column ▾" and ⋯ (Copy table, Show
//! excluded/suppressed, Show top level assembly row | Export to CSV), and
//! the table (frozen bold Item column on `#eaeaea`, 140 x 34 cells, 1 px
//! `#aaa` grid). Header right-click: Move left, Move right, Remove column;
//! row right-click: Switch to Part Studio. Editable cells write the part's
//! Properties (model: `noshape_model::assembly::bom`).

use super::*;
use gpui_kit::component::input::InputEvent;
use std::cell::Cell;
use std::rc::Rc;
use noshape_model::assembly::bom::{self, BomCell, BomColumn, BomLine, BomSource, BomType, CellEdit};

/// Column width (live: every column 140 px).
const COL_W: f32 = 140.;
const ROW_H: f32 = 34.;

/// A BOM cell being typed in: (row, column).
pub(crate) struct BomEdit {
    pub row: usize,
    pub col: BomColumn,
    pub input: Entity<InputState>,
    _sub: Subscription,
}

impl AssemblyView {
    fn bom_settings(&self, cx: &App) -> bom::BomSettings {
        self.assembly(cx).map(|a| a.bom.clone()).unwrap_or_default()
    }

    fn bom_lines(&self, cx: &App) -> Vec<BomLine> {
        let s = self.bom_settings(cx);
        bom::lines(&self.doc.read(cx).session.doc, self.tab, &self.result, &s)
    }

    /// Change the panel settings (undoable, like Onshape's table settings).
    fn edit_bom(&mut self, desc: &str, cx: &mut Context<Self>, f: impl FnOnce(&mut bom::BomSettings)) {
        self.edit(desc, cx, |a| f(&mut a.bom));
        cx.notify();
    }

    /// Write a cell into the part's / subassembly's Properties.
    fn set_bom_cell(&mut self, line: &BomLine, col: BomColumn, value: String, cx: &mut Context<Self>) {
        let mut probe = self.doc.read(cx).session.doc.clone();
        if !bom::set_cell(&mut probe, line, col, &value) {
            return;
        }
        let line = line.clone();
        // Live (qa5 pass 7): BOM cell edits are not undone by Ctrl+Z.
        self.doc.update(cx, |d, cx| {
            d.edit_document_not_undoable(&format!("Edit {}", col.label()), cx, |doc| {
                bom::set_cell(doc, &line, col, &value);
            })
        });
        cx.notify();
    }

    fn edit_bom_cell(&mut self, row: usize, col: BomColumn, current: String, window: &mut Window, cx: &mut Context<Self>) {
        self.edit_bom_cell_with(row, col, current, true, window, cx);
    }

    /// Edit a cell starting from `text`, selected (double-click) or with the
    /// caret after it (typing into the selected cell).
    fn edit_bom_cell_with(&mut self, row: usize, col: BomColumn, text: String, select: bool, window: &mut Window, cx: &mut Context<Self>) {
        // Esc drops the edit (the view's Cancel), not back to the typed key.
        let input = cx.new(|cx| InputState::new(window, cx).revert_on_escape(false));
        input.update(cx, |s, cx| {
            let n = text.len();
            s.set_value(text, window, cx);
            s.focus(window, cx);
            if select {
                s.select_all(window, cx);
            } else {
                // qa5 A11: the caret after the typed key (a select-on-focus
                // made the next key replace it: "PN-002" gave "N-002").
                s.set_selected_range(n..n, cx);
            }
        });
        let sub = cx.subscribe_in(&input, window, move |this: &mut Self, _, ev: &InputEvent, window, cx| match ev {
            InputEvent::PressEnter { .. } => {
                this.commit_bom_cell_down(cx);
                window.focus(&this.focus, cx);
            }
            InputEvent::Blur => this.commit_bom_cell(cx),
            _ => {}
        });
        self.ui.bom_edit = Some(BomEdit { row, col, input, _sub: sub });
        cx.notify();
    }

    /// Typing into the selected cell starts editing it with the typed text
    /// (spreadsheet style, live; qa5 A11). True when the key was taken.
    pub(super) fn bom_type_into(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.panel != Some(SidePanel::Bom) || self.ui.bom_edit.is_some() || !self.focus.is_focused(window) {
            return false;
        }
        let Some((row, col)) = self.ui.bom_selected else { return false };
        let Some(line) = self.bom_lines(cx).get(row).cloned() else { return false };
        if !bom::editable(&line, col) || col.edit() != CellEdit::Text {
            return false;
        }
        self.edit_bom_cell_with(row, col, text.to_string(), false, window, cx);
        true
    }

    /// Delete on the selected cell clears it (spreadsheet style; qa5 A11).
    pub(super) fn bom_clear_selected(&mut self, window: &Window, cx: &mut Context<Self>) -> bool {
        if self.panel != Some(SidePanel::Bom) || self.ui.bom_edit.is_some() || !self.focus.is_focused(window) {
            return false;
        }
        let Some((row, col)) = self.ui.bom_selected else { return false };
        let Some(line) = self.bom_lines(cx).get(row).cloned() else { return false };
        if !bom::editable(&line, col) || col.edit() != CellEdit::Text {
            return false;
        }
        self.set_bom_cell(&line, col, String::new(), cx);
        true
    }

    fn commit_bom_cell(&mut self, cx: &mut Context<Self>) {
        let Some(e) = self.ui.bom_edit.take() else { return };
        let text = e.input.read(cx).value().trim().to_string();
        if let Some(line) = self.bom_lines(cx).get(e.row).cloned() {
            self.set_bom_cell(&line, e.col, text, cx);
        }
        cx.notify();
    }

    /// Enter: commit and select the cell one row down (spreadsheet style, live).
    fn commit_bom_cell_down(&mut self, cx: &mut Context<Self>) {
        let at = self.ui.bom_edit.as_ref().map(|e| (e.row, e.col));
        self.commit_bom_cell(cx);
        if let Some((row, col)) = at {
            let n = self.bom_lines(cx).len();
            self.ui.bom_selected = Some(((row + 1).min(n.saturating_sub(1)), col));
        }
    }

    /// Esc while typing: drop the edit.
    pub(super) fn cancel_bom_cell(&mut self, cx: &mut Context<Self>) -> bool {
        if self.ui.bom_edit.take().is_some() {
            cx.notify();
            return true;
        }
        false
    }

    /// ⋯ › Export to CSV: "<Assembly name> - BOM.csv" (save dialog).
    fn export_bom_csv(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (text, name) = {
            let doc = &self.doc.read(cx).session.doc;
            let s = self.bom_settings(cx);
            let lines = bom::lines(doc, self.tab, &self.result, &s);
            (bom::table_text(doc, &self.result, &lines, &s, true), doc.tab(self.tab).map(|t| t.name.clone()).unwrap_or_default())
        };
        crate::files::request_save_path("Export to CSV", bom::csv_file_name(&name), "csv", window, cx, move |path, _| {
            if let Err(e) = std::fs::write(&path, text) {
                log::error!("BOM export to {}: {e}", path.display());
            }
        });
    }

    /// ⋯ › Copy table: tab separated, pastes into spreadsheets.
    fn copy_bom_table(&mut self, cx: &mut Context<Self>) {
        let text = {
            let doc = &self.doc.read(cx).session.doc;
            let s = self.bom_settings(cx);
            let lines = bom::lines(doc, self.tab, &self.result, &s);
            bom::table_text(doc, &self.result, &lines, &s, false)
        };
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        // Live: a green toast names the BOM type.
        let kind = self.bom_settings(cx).bom_type.label();
        self.ui.np_toast = Some(format!("{kind} BOM table copied to clipboard."));
        cx.notify();
    }

    fn bom_more_menu(&mut self, at: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        let s = self.bom_settings(cx);
        let view = cx.entity();
        let menu = crate::os_menu::OsMenu::build(window, cx, move |m, _, _| {
            let it = |label: &str, f: MenuAct| {
                let v = view.clone();
                crate::os_menu::OsMenuItem::new(label.to_string()).on_click(move |_, window, cx| v.update(cx, |this, cx| f(this, window, cx)))
            };
            let structured = s.bom_type == BomType::Structured;
            let mut m = m
                .list_no_icons()
                .item(it("Copy table", Rc::new(|this, _, cx| this.copy_bom_table(cx))))
                .item(it(
                    if s.show_excluded { "Hide excluded/suppressed" } else { "Show excluded/suppressed" },
                    Rc::new(|this, _, cx| this.edit_bom("Show excluded/suppressed", cx, |b| b.show_excluded = !b.show_excluded)),
                ));
            // Structured only: Collapse all / Expand all (live 3).
            if structured {
                m = m
                    .item(it("Collapse all", Rc::new(|this, _, cx| {
                        this.ui.bom_expanded.clear();
                        cx.notify();
                    })))
                    .item(it("Expand all", Rc::new(|this, _, cx| {
                        let all: Vec<String> = this.bom_lines(cx).into_iter().filter(|l| l.has_children).filter_map(|l| l.item).collect();
                        this.ui.bom_expanded.extend(all);
                        cx.notify();
                    })));
            }
            m.item(it(
                if s.show_top_level { "Hide top level assembly row" } else { "Show top level assembly row" },
                Rc::new(|this, _, cx| this.edit_bom("Show top level assembly row", cx, |b| b.show_top_level = !b.show_top_level)),
            ))
            .separator()
            .item(it("Export to CSV", Rc::new(|this, window, cx| this.export_bom_csv(window, cx))))
        });
        crate::os_menu::open_anchored(at, Anchor::TopRight, menu, window, cx);
    }

    fn bom_add_column_menu(&mut self, at: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        let s = self.bom_settings(cx);
        let view = cx.entity();
        let menu = crate::os_menu::OsMenu::build(window, cx, move |mut m, _, _| {
            m = m.list_no_icons();
            for c in BomColumn::ADDABLE.into_iter().filter(|c| !s.columns.contains(c)) {
                let v = view.clone();
                m = m.item(crate::os_menu::OsMenuItem::new(c.label()).on_click(move |_, _, cx| v.update(cx, |this, cx| this.edit_bom(&format!("Add column {}", c.label()), cx, |b| b.add_column(c)))));
            }
            m
        });
        crate::os_menu::open_anchored(at, Anchor::TopLeft, menu, window, cx);
    }

    fn bom_header_menu(&self, col: BomColumn, s: &bom::BomSettings, view: Entity<Self>) -> impl Fn(crate::os_menu::OsMenu, &mut Window, &mut Context<crate::os_menu::OsMenu>) -> crate::os_menu::OsMenu + 'static {
        let (left, right, remove) = (s.can_move(col, -1), s.can_move(col, 1), !col.frozen());
        move |m, _, _| {
            let it = |label: &'static str, on: bool, f: fn(&mut bom::BomSettings, BomColumn)| {
                let v = view.clone();
                crate::os_menu::OsMenuItem::new(label).disabled(!on).on_click(move |_, _, cx| v.update(cx, |this, cx| this.edit_bom(label, cx, |b| f(b, col))))
            };
            m.list_no_icons()
                .item(it("Move left", left, |b, c| b.move_column(c, -1)))
                .item(it("Move right", right, |b, c| b.move_column(c, 1)))
                .item(it("Remove column", remove, |b, c| b.remove_column(c)))
        }
    }

    /// The docked BOM Table pane.
    pub(super) fn render_bom_panel(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let border = rgb(0xaaaaaa);
        let head_bg = rgb(0xeaeaea);
        let s = self.bom_settings(cx);
        let lines = self.bom_lines(cx);
        let view = cx.entity();
        // Column widths: live's grips (`ui::col_grip`, 140 px until dragged).
        let col_w = |c: &BomColumn| crate::ui::col_w("bom", &format!("{c:?}"), COL_W);
        let cell = |w: f32| div().w(px(w)).h(px(ROW_H)).flex_none().flex().items_center().px(px(3.)).border_r_1().border_b_1().border_color(border).text_size(px(14.));

        // Header row: BOM type ▾ | Add column ▾ ⋯.
        let v = view.clone();
        let type_ix = BomType::ALL.iter().position(|t| *t == s.bom_type).unwrap_or(0);
        let type_select = crate::os_ui::select_field("bom-type", BomType::ALL.iter().map(|t| t.label().to_string()).collect(), type_ix, move |i, _, cx| {
            v.update(cx, |this, cx| this.edit_bom("BOM type", cx, |b| b.bom_type = BomType::ALL[i]));
        })
        .h(px(26.))
        .w(px(120.))
        .pl(px(12.));
        let add_bounds: Rc<Cell<Bounds<Pixels>>> = Default::default();
        let more_bounds: Rc<Cell<Bounds<Pixels>>> = Default::default();
        let (ab, mb) = (add_bounds.clone(), more_bounds.clone());
        let header = h_flex()
            .h(px(35.))
            .flex_none()
            .px(px(6.))
            .gap(px(1.))
            .bg(head_bg)
            .border_1()
            .border_color(border)
            .text_size(px(14.))
            .text_color(rgb(0x333333))
            .child(div().pr(px(10.)).child("BOM type:"))
            .child(type_select)
            .child(div().flex_1())
            .child(
                h_flex()
                    .id("bom-add-column")
                    .relative()
                    .h(px(23.))
                    .px(px(10.))
                    .gap(px(8.))
                    .rounded(px(3.5))
                    .border_1()
                    .border_color(border)
                    .bg(rgb(0xffffff))
                    .text_size(px(13.))
                    .cursor_pointer()
                    .tooltip(crate::ui::tooltip("Add columns to the BOM table"))
                    .child(canvas(move |b, _, _| ab.set(b), |_, _, _, _| {}).absolute().top_0().left_0().size_full())
                    .child("Add column")
                    .child(crate::os_ui::glyph(crate::os_ui::GLYPH_CARET, 8., rgb(0x333333)))
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        let b = add_bounds.get();
                        this.bom_add_column_menu(point(b.origin.x, b.origin.y + b.size.height), window, cx);
                    })),
            )
            .child(
                div()
                    .id("bom-more")
                    .relative()
                    .ml(px(4.))
                    .w(px(25.))
                    .h(px(22.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .child(canvas(move |b, _, _| mb.set(b), |_, _, _, _| {}).absolute().top_0().left_0().size_full())
                    .child(crate::assets::icon("more", 20., cx))
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        let b = more_bounds.get();
                        this.bom_more_menu(point(b.origin.x + b.size.width, b.origin.y + b.size.height), window, cx);
                    })),
            );

        // Rows: Structured subassemblies are collapsed until expanded
        // (double-click the row or ⋯ Expand all; the chevron is part of the
        // Item text, no indentation, live 357).
        let doc = self.doc.read(cx).session.doc.clone();
        let editing = self.ui.bom_edit.as_ref().map(|e| (e.row, e.col, e.input.clone()));
        let selected = self.ui.bom_selected;
        let expanded = self.ui.bom_expanded.clone();
        let structured = s.bom_type == BomType::Structured;
        let hidden_by_parent = |item: &str| {
            let mut parts: Vec<&str> = item.split('.').collect();
            while parts.len() > 1 {
                parts.pop();
                if !expanded.contains(&parts.join(".")) {
                    return true;
                }
            }
            false
        };
        // Selected cell: its column header and its row's Item go #d7d7d7.
        let sel_col = selected.map(|(_, c)| c);
        let mut head = h_flex().items_start().border_l_1().border_color(border).bg(head_bg).font_weight(FontWeight::BOLD);
        for c in &s.columns {
            let menu = self.bom_header_menu(*c, &s, view.clone());
            let label = div().truncate().child(c.label());
            // Mass carries Onshape's italic ƒ (computed) marker.
            let label = if *c == BomColumn::Mass { h_flex().gap(px(3.)).child(label).child(div().italic().font_weight(FontWeight::NORMAL).child("ƒ")).into_any_element() } else { label.into_any_element() };
            head = head.child(
                cell(col_w(c))
                    .id(SharedString::from(format!("bom-h-{c:?}")))
                    .relative()
                    .child(crate::ui::col_grip("bom", &format!("{c:?}"), COL_W))
                    .justify_center()
                    .when(sel_col == Some(*c), |d| d.bg(rgb(0xd7d7d7)))
                    .tooltip(crate::ui::tooltip(c.label()))
                    .child(label)
                    .os_context_menu(menu),
            );
        }
        let mut rows = v_flex().border_l_1().border_color(border);
        for (r, line) in lines.iter().enumerate() {
            if structured && line.depth > 0 && line.item.as_deref().is_some_and(&hidden_by_parent) {
                continue;
            }
            let item_key = line.item.clone().unwrap_or_default();
            let mut row = h_flex();
            for c in &s.columns {
                let content = bom::cell(&doc, &self.result, line, *c);
                let can_edit = bom::editable(line, *c);
                let frozen = c.frozen();
                let is_sel = selected == Some((r, *c));
                let missing = bom::is_missing_value(&content, *c);
                let mut el = cell(col_w(c))
                    .id(SharedString::from(format!("bom-{r}-{c:?}")))
                    .relative()
                    .when(frozen, |d| d.bg(if selected.is_some_and(|(sr, _)| sr == r) { rgb(0xd7d7d7) } else { head_bg }).font_weight(FontWeight::BOLD))
                    .when(!frozen && !can_edit, |d| d.bg(rgb(0xf5f5f5)))
                    .when(missing, |d| d.bg(rgb(0xf9ebec)))
                    .when(line.top_level, |d| d.font_weight(FontWeight::BOLD))
                    .when(is_sel, |d| d.border_2().border_color(rgb(0x1651b0)));
                if let Some((_, _, st)) = editing.as_ref().filter(|(er, ec, _)| *er == r && ec == c) {
                    rows_cell_input(&mut el, st);
                    row = row.child(el.border_2().border_color(rgb(0x1651b0)));
                    continue;
                }
                el = match &content {
                    BomCell::Text(t) if frozen && structured && line.has_children => {
                        let open = expanded.contains(&item_key);
                        el.child(div().truncate().child(format!("{} {t}", if open { "⌄" } else { "›" })))
                    }
                    BomCell::Text(t) if c.edit() == CellEdit::Choice && can_edit => el.child(h_flex().w_full().justify_between().child(div().truncate().child(t.clone())).child(crate::os_ui::glyph(crate::os_ui::GLYPH_CARET, 8., rgb(0x333333)))),
                    BomCell::Text(t) => el.child(div().truncate().child(t.clone())),
                    BomCell::Check(b) => el.justify_center().child(crate::os_ui::checkbox(SharedString::from(format!("bom-chk-{r}-{c:?}")), "", *b)),
                    BomCell::Color(col) => el.child(div().size(px(16.)).border_1().border_color(border).bg(gpui_kit::Rgba { r: srgb(col[0]), g: srgb(col[1]), b: srgb(col[2]), a: 1.0 })),
                };
                let (line2, c2) = (line.clone(), *c);
                // Click selects the cell; double-click edits (live 4).
                el = match (can_edit, c.edit()) {
                    (true, CellEdit::Text) => {
                        let current = content.text();
                        el.cursor_text().on_click(cx.listener(move |this, ev: &ClickEvent, window, cx| {
                            this.ui.bom_selected = Some((r, c2));
                            if ev.click_count() >= 2 {
                                this.edit_bom_cell(r, c2, current.clone(), window, cx);
                            }
                            cx.notify();
                        }))
                    }
                    (true, CellEdit::Check) => {
                        let on = matches!(content, BomCell::Check(true));
                        el.cursor_pointer().on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.ui.bom_selected = Some((r, c2));
                            this.set_bom_cell(&line2, c2, (!on).to_string(), cx);
                        }))
                    }
                    (true, CellEdit::Choice) => {
                        let current = content.text();
                        let v = view.clone();
                        el.cursor_pointer().on_mouse_down(MouseButton::Left, move |e: &MouseDownEvent, window, cx| {
                            let (v, line, current) = (v.clone(), line2.clone(), current.clone());
                            v.update(cx, |this, cx| {
                                this.ui.bom_selected = Some((r, c2));
                                cx.notify();
                            });
                            let menu = crate::os_menu::OsMenu::build(window, cx, move |mut m, _, _| {
                                m = m.select_list(COL_W);
                                for o in c2.choices() {
                                    let (v, line) = (v.clone(), line.clone());
                                    m = m.item(crate::os_menu::OsMenuItem::new(if o.is_empty() { " " } else { o }).checked(o == current).on_click(move |_, _, cx| v.update(cx, |this, cx| this.set_bom_cell(&line, c2, o.to_string(), cx))));
                                }
                                m
                            });
                            crate::os_menu::open(e.position, menu, window, cx);
                        })
                    }
                    _ => el.on_click(cx.listener(move |this, ev: &ClickEvent, _, cx| {
                        this.ui.bom_selected = Some((r, c2));
                        // Double-click a subassembly row: expand / collapse it.
                        if ev.click_count() >= 2 && structured && line2.has_children
                            && let Some(k) = line2.item.clone()
                            && !this.ui.bom_expanded.remove(&k)
                        {
                            this.ui.bom_expanded.insert(k);
                        }
                        cx.notify();
                    })),
                };
                row = row.child(el);
            }
            // Row right-click: Switch to Part Studio (this document's parts).
            let row_el = div().id(SharedString::from(format!("bom-row-{r}"))).child(row);
            let row_el = match &line.source {
                BomSource::Part { studio, .. } => {
                    let (studio, v) = (*studio, view.clone());
                    row_el.os_context_menu(move |m, _, _| {
                        let v = v.clone();
                        m.list_no_icons().item(crate::os_menu::OsMenuItem::new("Switch to Part Studio").on_click(move |_, _, cx| v.update(cx, |_, cx| cx.emit(AssemblyViewEvent::ActivateTab(studio)))))
                    })
                }
                _ => row_el,
            };
            rows = rows.child(row_el);
        }

        crate::ui::right_resizable(v_flex().id("bom-pane").relative(), 0.)
            .w(px(crate::ui::right_panel_w()))
            // Within its dock (at most half the graphics area): a fixed 712 px
            // ran under the panel strip and off the window ("Add columns" cut).
            .max_w_full()
            .h_full()
            .occlude()
            .bg(rgb(0xffffff))
            .text_color(rgb(0x333333))
            .child(
                div().pt(px(4.)).child(
                    div()
                        .h(px(26.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .border_b_2()
                        .border_color(rgb(0x1651b0))
                        .text_size(px(13.33))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(rgb(0x1651b0))
                        .child("Bill of materials"),
                ),
            )
            .child(div().pt(px(4.)).child(header))
            // Live: a horizontal scrollbar once the columns are wider than the pane.
            .child(v_flex().id("bom-table").flex_1().min_h_0().overflow_scroll().child(v_flex().items_start().child(head).child(rows)))
            .child(
                h_flex()
                    .h(px(40.))
                    .flex_none()
                    .bg(rgb(0xeeeeee))
                    .border_t_1()
                    .border_l_1()
                    .border_color(rgb(0xcccccc))
                    .child(div().flex_1())
                    .child(div().pr(px(10.)).child(div().size(px(14.)).rounded_full().flex().items_center().justify_center().text_size(px(11.)).font_weight(FontWeight::BOLD).child(crate::ui::glyph("help-dot", 15.6, crate::theme::help_dot(0xaaaaaa))))),
            )
            .text_color(pal.text)
            .into_any_element()
    }
}

/// The typed cell: a bare input filling it.
fn rows_cell_input(el: &mut Stateful<Div>, st: &Entity<InputState>) {
    let taken = std::mem::replace(el, div().id("bom-placeholder"));
    *el = taken.child(Input::new(st).appearance(false).text_size(px(13.33)).pl(px(3.)));
}

/// Linear → sRGB channel (swatches).
fn srgb(v: f32) -> f32 {
    let v = v.clamp(0.0, 1.0);
    if v <= 0.003_130_8 { v * 12.92 } else { 1.055 * v.powf(1.0 / 2.4) - 0.055 }
}

/// The text a keystroke types into a selected BOM cell: a printable
/// character without Ctrl / Alt / Cmd / Fn (those stay shortcuts).
pub(crate) fn typed_text(k: &gpui_kit::Keystroke) -> Option<String> {
    if k.modifiers.control || k.modifiers.alt || k.modifiers.platform || k.modifiers.function {
        return None;
    }
    k.key_char.clone().filter(|t| !t.is_empty() && !t.chars().any(char::is_control))
}

#[cfg(test)]
mod tests {
    use super::typed_text;

    #[test]
    fn typing_into_a_bom_cell_takes_printable_keys_only() {
        // What the platform reports for a key: modifiers, key, typed character.
        let k = |mods: &str, key: &str, ch: Option<&str>| {
            let mut s = gpui_kit::Keystroke::parse(&if mods.is_empty() { key.to_string() } else { format!("{mods}-{key}") }).unwrap();
            s.key_char = ch.map(String::from);
            s
        };
        assert_eq!(typed_text(&k("", "p", Some("p"))).as_deref(), Some("p"));
        assert_eq!(typed_text(&k("shift", "p", Some("P"))).as_deref(), Some("P"));
        assert_eq!(typed_text(&k("", "space", Some(" "))).as_deref(), Some(" "));
        assert_eq!(typed_text(&k("ctrl", "z", Some("z"))), None);
        assert_eq!(typed_text(&k("cmd", "c", Some("c"))), None);
        assert_eq!(typed_text(&k("", "escape", None)), None);
        assert_eq!(typed_text(&k("", "enter", Some("\n"))), None);
    }
}
