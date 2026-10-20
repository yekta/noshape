//! Drawing editor: small dialogs from the right-click menus (live
//! behaviours.md "Move to sheet…", "General table"): "Move to sheet" (a
//! floating dialog at the click point: Sheet ▾ preselected to the current
//! sheet, ✓ moves the object there, switches to that sheet and keeps it
//! selected) and the general table's "Table properties" (only Select fixed
//! corner); and the menu of a multi-selection (Copy, Paste as table, Clear
//! selection, Zoom to fit, Delete).

use crate::drawing_view::*;
use crate::os_ui::OsDropdownExt as _;
use crate::theme::p;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::drawing::{TableCorner, ViewId};

/// What "Move to sheet" moves.
#[derive(Clone, Copy, PartialEq)]
pub enum MoveTarget {
    View(ViewId),
    Table(u64),
}

/// The open "Move to sheet" dialog.
pub struct MoveSheetDlg {
    pub target: MoveTarget,
    /// Where the menu was opened (editor px).
    pub at: [f64; 2],
    pub sheet: usize,
}

/// The open general "Table properties" dialog.
pub struct TablePropsDlg {
    pub table: u64,
    original: TableCorner,
}

const CORNERS: [TableCorner; 4] = [TableCorner::TopLeft, TableCorner::TopRight, TableCorner::BottomLeft, TableCorner::BottomRight];

impl DrawingEditor {
    pub fn open_move_sheet(&mut self, target: MoveTarget, cx: &mut Context<Self>) {
        self.move_sheet = Some(MoveSheetDlg { target, at: self.menu_at, sheet: self.sheet });
        cx.notify();
    }

    fn accept_move_sheet(&mut self, cx: &mut Context<Self>) {
        let Some(d) = self.move_sheet.take() else { return };
        if d.sheet != self.sheet {
            match d.target {
                MoveTarget::View(id) => self.move_selected_views(id, Some(d.sheet), cx),
                MoveTarget::Table(id) => self.move_table_to_sheet(id, Some(d.sheet), cx),
            }
        }
        cx.notify();
    }

    /// Right-click with several entities selected (live drawing/291).
    pub fn multi_selection_entries(&self) -> Vec<MenuEntry> {
        vec![
            MenuEntry::item("Copy", |t, _, cx| t.copy_views(None, cx)),
            MenuEntry::item("Paste as table", |t, _, cx| {
                t.start_paste_table(cx);
            }),
            MenuEntry::Separator,
            MenuEntry::item("Clear selection", |t, _, cx| {
                t.selection.clear();
                t.redraw(cx);
            }),
            MenuEntry::item("Zoom to fit", |t, _, cx| t.zoom_to_fit(cx)),
            MenuEntry::item("Delete", |t, _, cx| t.delete_selection(cx)),
        ]
    }

    pub fn open_table_props_corner(&mut self, id: u64, cx: &mut Context<Self>) {
        let Some(t) = self.current_sheet().table(id) else { return };
        self.table_props = Some(TablePropsDlg { table: id, original: t.corner });
        cx.notify();
    }

    fn set_table_corner(&mut self, id: u64, k: TableCorner, cx: &mut Context<Self>) {
        // Keep the table where it is: the anchor moves to the new corner.
        let l = self.built.as_ref().and_then(|b| b.table_layout(id)).cloned();
        self.edit_table(id, "Table fixed corner", cx, |t| {
            if let Some(l) = &l {
                let (lo, hi) = (l.min(), l.max());
                t.position = match k {
                    TableCorner::TopLeft => [lo[0], hi[1]],
                    TableCorner::TopRight => [hi[0], hi[1]],
                    TableCorner::BottomLeft => [lo[0], lo[1]],
                    TableCorner::BottomRight => [hi[0], lo[1]],
                };
            }
            t.corner = k;
        });
    }

    pub fn close_table_props(&mut self, accept: bool, cx: &mut Context<Self>) {
        let Some(d) = self.table_props.take() else { return };
        if !accept {
            self.set_table_corner(d.table, d.original, cx);
        }
        cx.notify();
    }

    fn dialog_header(title: &'static str, ok: impl Fn(&mut Self, &mut Context<Self>) + 'static, cancel: impl Fn(&mut Self, &mut Context<Self>) + 'static, ids: (&'static str, &'static str), cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        h_flex()
            .h(px(28.))
            .pl(px(11.))
            .border_b_1()
            .border_color(gpui_kit::rgb(0xd3d7d9))
            .child(div().flex_1().text_size(px(13.)).font_weight(FontWeight::MEDIUM).child(title))
            .child(div().id(ids.0).w(px(28.)).h(px(27.)).flex().items_center().justify_center().bg(pal.accept).cursor_pointer().child(crate::ui::glyph("check", 18., gpui_kit::white())).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| ok(this, cx))))
            .child(div().id(ids.1).w(px(28.)).h(px(27.)).flex().items_center().justify_center().cursor_pointer().child(crate::drawing_chrome::cancel_x(cx)).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| cancel(this, cx))))
            .into_any_element()
    }

    pub fn render_move_sheet(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let d = self.move_sheet.as_ref()?;
        let pal = p();
        let ed = cx.entity();
        let names: Vec<String> = self.drawing.sheets.iter().map(|s| s.name.clone()).collect();
        let cur = names.get(d.sheet).cloned().unwrap_or_default();
        let at = d.at;
        let sheet = Button::new("ms-sheet").label(cur).ghost().xsmall().w(px(130.)).dropdown_caret(true).border_b_1().border_color(gpui_kit::rgb(0xdddddd)).rounded(px(0.)).os_dropdown_menu("ms-sheet-dd", true, move |mut m, _, _| {
            for (i, n) in names.iter().enumerate() {
                let e = ed.clone();
                m = m.item(crate::os_menu::OsMenuItem::new(n.clone()).on_click(move |_, _, cx| {
                    e.update(cx, |t, cx| {
                        if let Some(d) = &mut t.move_sheet {
                            d.sheet = i;
                        }
                        cx.notify();
                    })
                }));
            }
            m
        });
        Some(
            v_flex()
                .id("dr-move-sheet")
                .absolute()
                .left(px(at[0] as f32))
                .top(px(at[1] as f32))
                .w(px(220.))
                .pb(px(6.))
                .bg(pal.dialog_bg)
                .shadow(crate::drawing_chrome::dialog_shadow())
                .text_color(gpui_kit::rgb(0x333333))
                .text_size(px(12.))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(Self::dialog_header("Move to sheet", |t, cx| t.accept_move_sheet(cx), |t, cx| {
                    t.move_sheet = None;
                    cx.notify();
                }, ("ms-ok", "ms-close"), cx))
                .child(h_flex().h(px(30.)).px(px(10.)).gap(px(8.)).child(div().w(px(50.)).child("Sheet")).child(sheet))
                .into_any_element(),
        )
    }

    pub fn render_table_props(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let d = self.table_props.as_ref()?;
        let pal = p();
        let ink = gpui_kit::rgb(0x333333);
        let id = d.table;
        let corner = self.current_sheet().table(id).map(|t| t.corner).unwrap_or(TableCorner::TopRight);
        let corners: Vec<AnyElement> = CORNERS
            .iter()
            .enumerate()
            .map(|(i, k)| {
                let k = *k;
                let on = corner == k;
                let (top, left) = (matches!(k, TableCorner::TopLeft | TableCorner::TopRight), matches!(k, TableCorner::TopLeft | TableCorner::BottomLeft));
                // Live drawing/550: 26 px, 1 px `#d3d7d9`; the current one
                // `#e6e6e6`, `#ccc` with a 2 px blue bottom rule.
                div()
                    .id(("tp-corner", i))
                    .size(px(26.))
                    .p(px(2.))
                    .relative()
                    .cursor_pointer()
                    .border_1()
                    .border_color(gpui_kit::rgb(0xd3d7d9))
                    .when(on, |e| e.bg(gpui_kit::rgb(0xe6e6e6)).border_color(gpui_kit::rgb(0xcccccc)).child(div().absolute().left(px(-1.)).right(px(-1.)).bottom(px(-1.)).h(px(2.)).bg(pal.accent)))
                    .child(crate::ui::glyph("table", 20., ink))
                    .child(div().absolute().size(px(4.)).rounded_full().bg(ink).when(top, |e| e.top(px(2.))).when(!top, |e| e.bottom(px(2.))).when(left, |e| e.left(px(2.))).when(!left, |e| e.right(px(2.))))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.set_table_corner(id, k, cx)))
                    .into_any_element()
            })
            .collect();
        Some(
            v_flex()
                .id("dr-table-props")
                .absolute()
                .top(px(DIALOG_TOP))
                .left(px(self.dialog_left()))
                .w(px(200.))
                .pb(px(3.))
                .bg(pal.dialog_bg)
                .shadow(crate::drawing_chrome::dialog_shadow())
                .text_color(ink)
                .text_size(px(12.))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(Self::dialog_header("Table properties", |t, cx| t.close_table_props(true, cx), |t, cx| t.close_table_props(false, cx), ("tp-ok", "tp-close"), cx))
                .child(div().pl(px(11.)).pt(px(14.)).h(px(34.)).text_size(px(13.33)).child("Select fixed corner"))
                .child(h_flex().pl(px(16.)).pt(px(6.)).gap(px(24.)).children(corners))
                .into_any_element(),
        )
    }
}
