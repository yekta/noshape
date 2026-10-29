//! Drawing editor: the Table tool's "Table" dialog (live drawing/530–533,
//! dom/drawing-table-*): Rows 2, Columns 2, ☐ Title row, ☐ Header row and
//! Select fixed corner; the table's outline follows the pointer, a click
//! positions it, and only ✓ creates it (✗ / Esc discards).

use crate::drawing_view::*;
use crate::theme::p;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::drawing::{Table, TableCorner};

const CORNERS: [TableCorner; 4] = [TableCorner::TopLeft, TableCorner::TopRight, TableCorner::BottomLeft, TableCorner::BottomRight];

/// The open Table dialog.
pub struct TableToolDlg {
    pub rows: Entity<InputState>,
    pub cols: Entity<InputState>,
    pub title_row: bool,
    pub header_row: bool,
    pub corner: TableCorner,
    /// Where a click put the table (its fixed corner).
    pub placed: Option<[f64; 2]>,
}

impl DrawingEditor {
    /// Create / drop the dialog with the Table tool (from render).
    pub fn sync_table_tool(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let active = matches!(self.tool, Tool::Table { kind: TableTool::General, .. });
        if active && self.table_tool.is_none() {
            let rows = cx.new(|cx| InputState::new(window, cx).select_on_focus(false).revert_on_escape(false).default_value("2"));
            let cols = cx.new(|cx| InputState::new(window, cx).select_on_focus(false).revert_on_escape(false).default_value("2"));
            // Live 530: Rows has the focus with its value selected.
            rows.update(cx, |s, cx| {
                s.focus(window, cx);
                s.select_all(window, cx);
            });
            self.table_tool = Some(TableToolDlg { rows, cols, title_row: false, header_row: false, corner: TableCorner::TopRight, placed: None });
        } else if !active {
            self.table_tool = None;
        }
    }

    /// Rows and columns typed in the dialog.
    fn table_tool_size(&self, cx: &App) -> Option<(usize, usize)> {
        let d = self.table_tool.as_ref()?;
        Some(noshape_scene::drawing_tools::table_size(&d.rows.read(cx).value(), &d.cols.read(cx).value()))
    }

    /// The table the dialog would create, its fixed corner at `at`.
    fn table_tool_table(&self, at: [f64; 2], cx: &App) -> Option<Table> {
        let d = self.table_tool.as_ref()?;
        let (r, c) = self.table_tool_size(cx)?;
        Some(noshape_scene::drawing_tools::general_table(r, c, d.title_row, d.header_row, d.corner, at))
    }

    /// The ghost outline at the placed point or the pointer.
    pub fn table_tool_overlay(&self, overlay: &mut crate::drawing_scene::Overlay, cx: &App) {
        let Some(d) = &self.table_tool else { return };
        let at = d.placed.unwrap_or(self.cursor);
        let Some(t) = self.table_tool_table(at, cx) else { return };
        let data = noshape_model::drawing::tables::table_data(&self.drawing, &t, None, None, None, &[], &|s| s.to_string());
        let l = noshape_model::drawing::tables::layout(&t, &data);
        overlay.rect(l.min(), l.max(), true);
    }

    /// A sheet click with the dialog open: position the table.
    pub fn table_tool_click(&mut self, cx: &mut Context<Self>) -> bool {
        let c = self.cursor;
        let Some(d) = &mut self.table_tool else { return false };
        d.placed = Some(c);
        cx.notify();
        true
    }

    fn close_table_tool(&mut self, accept: bool, window: &mut Window, cx: &mut Context<Self>) {
        let placed = self.table_tool.as_ref().and_then(|d| d.placed);
        if accept && let Some(at) = placed && let Some(mut t) = self.table_tool_table(at, cx) {
            let id = self.current_sheet().next_table_id();
            t.id = id;
            self.edit_sheet("Table", cx, |s| s.tables.push(t));
            // Live 743: ✓ starts typing in the first cell; the table is not selected.
            self.selection.clear();
            let input = self.textarea("", 1, window, cx);
            let note = self.cell_note(id, 0, 0, window, cx);
            self.panel = Some(Panel::Cell { table: id, row: 0, col: 0, input, note });
        }
        self.table_tool = None;
        self.tool = Tool::Select;
        // The ghost outline goes with it (qa DWG-TABLE-ESC-GHOST).
        self.update_preview(cx);
        cx.notify();
    }

    pub fn render_table_tool(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let d = self.table_tool.as_ref()?;
        let pal = p();
        let ink = gpui_kit::rgb(0x333333);
        let corner = d.corner;
        let corners: Vec<AnyElement> = CORNERS
            .iter()
            .enumerate()
            .map(|(i, k)| {
                let k = *k;
                let on = corner == k;
                let (top, left) = (matches!(k, TableCorner::TopLeft | TableCorner::TopRight), matches!(k, TableCorner::TopLeft | TableCorner::BottomLeft));
                // Live drawing/530: as Table properties' corner buttons.
                div()
                    .id(("tt-corner", i))
                    .size(px(26.))
                    .p(px(2.))
                    .relative()
                    .cursor_pointer()
                    .border_1()
                    .border_color(gpui_kit::rgb(0xd3d7d9))
                    .when(on, |e| e.bg(gpui_kit::rgb(0xe6e6e6)).border_color(gpui_kit::rgb(0xcccccc)).child(div().absolute().left(px(-1.)).right(px(-1.)).bottom(px(-1.)).h(px(2.)).bg(pal.accent)))
                    .child(crate::ui::glyph("table", 20., ink))
                    .child(div().absolute().size(px(4.)).rounded_full().bg(ink).when(top, |e| e.top(px(2.))).when(!top, |e| e.bottom(px(2.))).when(left, |e| e.left(px(2.))).when(!left, |e| e.right(px(2.))))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        if let Some(d) = &mut this.table_tool {
                            d.corner = k;
                        }
                        cx.notify();
                    }))
                    .into_any_element()
            })
            .collect();
        // Live 530 / 740: 28 px rows, 12 px labels 10 px in, borderless 121 px
        // fields on a `#ddd` underline from x 77.
        let row = |label: &'static str, input: &Entity<InputState>| {
            h_flex()
                .h(px(28.))
                .pl(px(10.))
                .child(div().w(px(67.)).text_size(px(12.)).child(label))
                .child(div().w(px(121.)).h(px(26.)).flex().items_end().border_b_1().border_color(gpui_kit::rgb(0xdddddd)).text_size(px(13.33)).child(Input::new(input).text_size(px(13.33)).text_color(gpui_kit::rgb(0x555555)).appearance(false).small()))
                .into_any_element()
        };
        let (title_row, header_row) = (d.title_row, d.header_row);
        let placed = d.placed.is_some();
        Some(
            v_flex()
                .id("dr-table-tool")
                // Esc (Rows has the focus): closed without placing (live
                // request 1566, qa DWG-PANEL-ESC-STACK).
                .capture_key_down(cx.listener(|this, e: &KeyDownEvent, window, cx| {
                    if e.keystroke.key == "escape" {
                        cx.stop_propagation();
                        this.close_table_tool(false, window, cx);
                    }
                }))
                .absolute()
                .top(px(DIALOG_TOP))
                .left(px(self.dialog_left()))
                .w(px(200.))
                .pb(px(3.))
                .bg(pal.dialog_bg)
                .shadow(crate::drawing_chrome::dialog_shadow())
                .text_color(ink)
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    h_flex()
                        .h(px(28.))
                        .pl(px(11.))
                        .border_b_1()
                        .border_color(gpui_kit::rgb(0xd3d7d9))
                        .child(div().flex_1().text_size(px(13.)).font_weight(FontWeight::MEDIUM).child("Table"))
                        .child(div().id("tt-ok").w(px(28.)).h(px(27.)).flex().items_center().justify_center().bg(if placed { pal.accept } else { gpui_kit::rgb(crate::drawing_chrome::OK_DISABLED) }).when(placed, |e| e.cursor_pointer()).child(crate::ui::glyph("check", 18., gpui_kit::white())).on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                            // ✓ stays disabled until the table is placed (live 530).
                            if placed {
                                this.close_table_tool(true, window, cx)
                            }
                        })))
                        .child(div().id("tt-close").w(px(28.)).h(px(27.)).flex().items_center().justify_center().cursor_pointer().child(crate::drawing_chrome::cancel_x(cx)).on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.close_table_tool(false, window, cx)))),
                )
                .child(div().h(px(4.)))
                .child(row("Rows", &d.rows))
                .child(row("Columns", &d.cols))
                .child(div().pl(px(9.)).h(px(26.)).flex().items_center().child(crate::drawing_chrome::DrCheckbox::new("tt-title").label("Title row").checked(title_row).on_click(cx.listener(|this, v: &bool, _, cx| {
                    if let Some(d) = &mut this.table_tool {
                        d.title_row = *v;
                    }
                    cx.notify();
                }))))
                .child(div().pl(px(9.)).h(px(26.)).flex().items_center().child(crate::drawing_chrome::DrCheckbox::new("tt-header").label("Header row").checked(header_row).on_click(cx.listener(|this, v: &bool, _, cx| {
                    if let Some(d) = &mut this.table_tool {
                        d.header_row = *v;
                    }
                    cx.notify();
                }))))
                .child(div().pl(px(11.)).pt(px(4.)).h(px(24.)).text_size(px(12.)).child("Select fixed corner"))
                .child(h_flex().pl(px(16.)).pt(px(6.)).gap(px(24.)).children(corners))
                .into_any_element(),
        )
    }
}
