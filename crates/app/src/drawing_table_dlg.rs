//! Drawing editor: Onshape's BOM table and Cut list table dialogs (live
//! drawing/188, help cutlist-table-01): Insert + the source (an assembly
//! for BOMs, a Part Studio with frames for cut lists, from the drawing's
//! views), Template "Follow assembly" and BOM type (BOM only), Order, and
//! Select fixed corner. The table's outline follows the cursor; a click
//! places it.

use crate::drawing_view::*;
use crate::os_ui::OsDropdownExt as _;
use crate::theme::p;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::drawing::views::{self, SourceKey};
use noshape_model::drawing::{TableCorner, TableKind, ViewId};

const CORNERS: [TableCorner; 4] = [TableCorner::TopLeft, TableCorner::TopRight, TableCorner::BottomLeft, TableCorner::BottomRight];

/// BOM / cut list table dialog state.
pub struct TableDlg {
    pub kind: TableTool,
    /// The view whose source the table lists.
    pub view: Option<ViewId>,
    pub structured: bool,
    pub bottom_to_top: bool,
    pub corner: TableCorner,
}

impl DrawingEditor {
    /// Views of the current sheet whose source suits the table (assemblies
    /// for BOMs, Part Studios for cut lists), one per source, with names.
    fn table_sources(&self, kind: TableTool) -> Vec<(ViewId, String)> {
        let sheet = self.current_sheet();
        let mut seen: Vec<SourceKey> = Vec::new();
        let mut out = Vec::new();
        for v in &sheet.views {
            let Some((key, _)) = views::view_source(sheet, v.id) else { continue };
            let Some(src) = self.sources.get(&key) else { continue };
            let fits = match kind {
                TableTool::Bom => src.assembly,
                TableTool::CutList => !src.assembly,
                _ => false,
            };
            if fits && !seen.contains(&key) {
                seen.push(key);
                out.push((v.id, src.name.clone()));
            }
        }
        out
    }

    /// Create / drop the dialog with the BOM table and Cut list table tools.
    pub fn sync_table_dlg(&mut self) {
        let kind = match &self.tool {
            Tool::Table { kind: k @ (TableTool::Bom | TableTool::CutList), .. } => Some(*k),
            _ => None,
        };
        match kind {
            Some(k) if self.table_dlg.as_ref().is_none_or(|d| d.kind != k) => {
                // The selected view (if it fits), else the first fitting one.
                let sources = self.table_sources(k);
                let preferred = match &self.tool {
                    Tool::Table { view: Some(v), .. } => sources.iter().find(|(x, _)| x == v).map(|s| s.0),
                    _ => None,
                };
                let view = preferred.or_else(|| sources.first().map(|s| s.0));
                self.table_dlg = Some(TableDlg { kind: k, view, structured: false, bottom_to_top: false, corner: TableCorner::TopRight });
            }
            Some(_) => {}
            None => self.table_dlg = None,
        }
    }

    /// A click with the dialog open: place the table there. True when used.
    pub fn table_dlg_click(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(d) = &self.table_dlg else { return false };
        let Some(view) = d.view else { return true };
        let c = self.cursor;
        let id = self.current_sheet().next_table_id();
        let (kind, name) = match d.kind {
            TableTool::Bom => (TableKind::Bom { view, structured: d.structured, bottom_to_top: d.bottom_to_top, overrides: Vec::new() }, "BOM table"),
            _ => (TableKind::CutList { view, bottom_to_top: d.bottom_to_top }, "Cut list table"),
        };
        let corner = d.corner;
        self.edit_sheet(name, cx, |s| {
            let mut t = noshape_model::drawing::Table::new(id, kind, c);
            t.corner = corner;
            s.tables.push(t);
        });
        self.table_dlg = None;
        self.tool = Tool::Select;
        self.selection = vec![Sel::Table(id)];
        cx.notify();
        true
    }

    pub fn render_table_dlg(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let d = self.table_dlg.as_ref()?;
        let pal = p();
        let ink = gpui_kit::rgb(0x333333);
        let ed = cx.entity();
        let bom = d.kind == TableTool::Bom;
        let sources = self.table_sources(d.kind);
        let source_label = sources.iter().find(|s| Some(s.0) == d.view).map(|s| s.1.clone()).unwrap_or_default();
        type Set = std::rc::Rc<dyn Fn(&mut TableDlg)>;
        let select = |id: &'static str, label: String, w: f32, items: Vec<(String, bool, Set)>| {
            let ed = ed.clone();
            Button::new(id).label(label).ghost().xsmall().w(px(w)).dropdown_caret(true).border_b_1().border_color(gpui_kit::rgb(0xdddddd)).rounded(px(0.)).os_dropdown_menu(SharedString::from(format!("{id}-dd")), true, move |mut m, _, _| {
                for (l, on, f) in &items {
                    let (e, f) = (ed.clone(), f.clone());
                    m = m.item(crate::os_menu::OsMenuItem::new(l.clone()).checked(*on).on_click(move |_, _, cx| {
                        e.update(cx, |t, cx| {
                            if let Some(d) = &mut t.table_dlg {
                                f(d);
                            }
                            cx.notify();
                        })
                    }));
                }
                m
            })
        };
        let source_items: Vec<(String, bool, Set)> = sources.iter().map(|(v, n)| {
            let v = *v;
            (n.clone(), Some(v) == d.view, std::rc::Rc::new(move |d: &mut TableDlg| d.view = Some(v)) as Set)
        }).collect();
        let types: Vec<(String, bool, Set)> = vec![
            ("Flattened".into(), !d.structured, std::rc::Rc::new(|d: &mut TableDlg| d.structured = false) as Set),
            ("Structured".into(), d.structured, std::rc::Rc::new(|d: &mut TableDlg| d.structured = true) as Set),
        ];
        let orders: Vec<(String, bool, Set)> = vec![
            ("Top to bottom".into(), !d.bottom_to_top, std::rc::Rc::new(|d: &mut TableDlg| d.bottom_to_top = false) as Set),
            ("Bottom to top".into(), d.bottom_to_top, std::rc::Rc::new(|d: &mut TableDlg| d.bottom_to_top = true) as Set),
        ];
        let row = |label: &'static str, body: AnyElement| h_flex().h(px(28.)).px(px(8.)).child(div().w(px(70.)).text_size(px(11.)).child(label)).child(body).into_any_element();
        let corner = d.corner;
        let corner_btn = |k: TableCorner, i: usize, cx: &mut Context<Self>| {
            let on = corner == k;
            let (top, left) = (matches!(k, TableCorner::TopLeft | TableCorner::TopRight), matches!(k, TableCorner::TopLeft | TableCorner::BottomLeft));
            div()
                .id(("table-corner", i))
                .size(px(24.))
                .relative()
                .cursor_pointer()
                .border_1()
                .border_color(gpui_kit::rgb(0x909090))
                .when(on, |e| e.border_b_2().border_color(pal.accent))
                .child(crate::ui::glyph("table", 20., ink))
                .child(div().absolute().size(px(4.)).rounded_full().bg(ink).when(top, |e| e.top(px(1.))).when(!top, |e| e.bottom(px(1.))).when(left, |e| e.left(px(1.))).when(!left, |e| e.right(px(1.))))
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    if let Some(d) = &mut this.table_dlg {
                        d.corner = k;
                    }
                    cx.notify();
                }))
        };
        let corners: Vec<AnyElement> = CORNERS.iter().enumerate().map(|(i, k)| corner_btn(*k, i, cx).into_any_element()).collect();
        Some(
            v_flex()
                .id("dr-table-dlg")
                .absolute()
                .top(px(DIALOG_TOP))
                .left(px(self.dialog_left()))
                .w(px(210.))
                .pb(px(6.))
                .bg(pal.dialog_bg)
                .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(5.)), blur_radius: px(10.), spread_radius: px(0.), inset: false }])
                .text_color(ink)
                .text_size(px(12.))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    h_flex()
                        .h(px(28.))
                        .pl(px(11.))
                        .child(div().flex_1().text_size(px(13.)).font_weight(FontWeight::MEDIUM).child(if bom { "BOM table" } else { "Cut list table" }))
                        .child(div().id("table-dlg-close").w(px(28.)).h(px(27.)).flex().items_center().justify_center().cursor_pointer().child(crate::ui::glyph("close", 14., pal.cancel)).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.table_dlg = None;
                            this.tool = Tool::Select;
                            cx.notify();
                        }))),
                )
                .child(
                    h_flex()
                        .px(px(4.))
                        .gap(px(4.))
                        // Insert lists the drawing's suitable sources, like the select.
                        .child(select("table-dlg-insert", "Insert".into(), 64., source_items.clone()))
                        .child(select("table-dlg-source", source_label, 120., source_items)),
                )
                .when(bom, |v| v.child(row("Template", div().w(px(120.)).text_size(px(11.)).child("Follow assembly").into_any_element())).child(row("BOM type", select("table-dlg-type", if d.structured { "Structured".into() } else { "Flattened".into() }, 120., types).into_any_element())))
                .child(row("Order", select("table-dlg-order", if d.bottom_to_top { "Bottom to top".into() } else { "Top to bottom".into() }, 120., orders).into_any_element()))
                .child(div().px(px(8.)).pt(px(4.)).text_size(px(11.)).child("Select fixed corner"))
                .child(h_flex().px(px(8.)).pt(px(4.)).gap(px(26.)).children(corners))
                .into_any_element(),
        )
    }
}
