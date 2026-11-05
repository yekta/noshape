//! Drawing editor: Onshape's Hole table dialog (live behaviours 1983):
//! ☑ Show location data, Location precision ▾ (0 … 0.123 (Drawing) …),
//! the view and its origin, Sheet, Order ▾ and Select fixed corner. Pick
//! the view, then the origin (a vertex or circle center); a click places
//! the table with these choices.

use crate::drawing_view::*;
use crate::os_ui::OsDropdownExt as _;
use crate::theme::p;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::drawing::{TableCorner, TableKind, ViewId};

const CORNERS: [TableCorner; 4] = [TableCorner::TopLeft, TableCorner::TopRight, TableCorner::BottomLeft, TableCorner::BottomRight];

/// Location precision choices (live's list); None = "(Drawing)".
const PRECISIONS: [(&str, Option<u8>); 8] = [("0", Some(0)), ("0.1", Some(1)), ("0.12", Some(2)), ("0.123", Some(3)), ("0.123 (Drawing)", None), ("0.1234", Some(4)), ("0.12345", Some(5)), ("0.123456", Some(6))];

/// The Hole table dialog's choices.
pub struct HoleTableDlg {
    pub show_location: bool,
    pub precision: Option<u8>,
    pub bottom_to_top: bool,
    /// Live: the 2nd corner icon is pressed at first.
    pub corner: TableCorner,
    /// Hole table properties… of this table (None: inserting one).
    pub table: Option<u64>,
    /// The X / Y flip buttons.
    pub flip_x: bool,
    pub flip_y: bool,
    /// Holes to exclude (view coordinates); `picking`: its field takes
    /// clicks on hole circles.
    pub exclude: Vec<[f64; 2]>,
    pub picking: bool,
    /// X direction (view coordinates; None: the view's) and whether its
    /// field takes the next straight edge clicked.
    pub x_dir: Option<[f64; 2]>,
    pub x_picking: bool,
    /// Y direction takes the next straight edge instead (live request
    /// 1682: the X axis then a quarter turn clockwise from it); `y_set`:
    /// the axes came from the Y field (the X field is hidden, and the other
    /// way round).
    pub y_picking: bool,
    pub y_set: bool,
    /// Further views of the same part (live request 1744: the Views box
    /// clicked again takes another view; each its own origin) and whether
    /// the next click picks one.
    pub more: Vec<(ViewId, Option<noshape_scene::drawing_tools::PickedPoint>)>,
    pub adding_view: bool,
}

impl Default for HoleTableDlg {
    fn default() -> Self {
        Self { show_location: true, precision: None, bottom_to_top: false, corner: TableCorner::TopRight, table: None, flip_x: false, flip_y: false, exclude: Vec::new(), picking: false, x_dir: None, x_picking: false, y_picking: false, y_set: false, more: Vec::new(), adding_view: false }
    }
}

impl DrawingEditor {
    /// Open / drop the dialog with the Hole table tool.
    pub fn sync_hole_table_dlg(&mut self) {
        // Hole table properties… stays open whatever the tool.
        if self.hole_dlg.as_ref().is_some_and(|d| d.table.is_some()) {
            return;
        }
        let on = matches!(self.tool, Tool::Table { kind: TableTool::Hole, .. });
        match (on, self.hole_dlg.is_some()) {
            (true, false) => self.hole_dlg = Some(HoleTableDlg::default()),
            (false, true) => self.hole_dlg = None,
            _ => {}
        }
    }

    /// Table menu › Hole table properties…: the dialog with the table's
    /// choices; ✓ applies them (one edit), ✕ leaves the table as it was.
    pub fn open_hole_table_props(&mut self, id: u64, cx: &mut Context<Self>) {
        let Some(t) = self.current_sheet().table(id) else { return };
        let TableKind::Hole { show_location, precision, bottom_to_top, flip_x, flip_y, ref exclude, x_dir, .. } = t.kind else { return };
        self.hole_dlg = Some(HoleTableDlg { show_location, precision, bottom_to_top, corner: t.corner, table: Some(id), flip_x, flip_y, exclude: exclude.clone(), picking: false, x_dir, x_picking: false, y_picking: false, y_set: false, more: Vec::new(), adding_view: false });
        cx.notify();
    }

    fn close_hole_table_props(&mut self, accept: bool, cx: &mut Context<Self>) {
        let Some(d) = self.hole_dlg.take() else { return };
        // The table's columns may change: no stale cell outline (qa
        // HOLE-TABLE-MENU (2)).
        self.cell_sel = None;
        if let (true, Some(id)) = (accept, d.table) {
            let (sl, pr, bt, corner, fx, fy) = (d.show_location, d.precision, d.bottom_to_top, d.corner, d.flip_x, d.flip_y);
            let (excluded, xd) = (d.exclude.clone(), d.x_dir);
            self.edit_table(id, "Hole table properties", cx, move |t| {
                if let TableKind::Hole { show_location, precision, bottom_to_top, flip_x, flip_y, exclude, x_dir, .. } = &mut t.kind {
                    (*show_location, *precision, *bottom_to_top, *flip_x, *flip_y) = (sl, pr, bt, fx, fy);
                    *exclude = excluded;
                    *x_dir = xd;
                }
                t.corner = corner;
            });
        }
        cx.notify();
    }

    /// A hole table just made by the tool takes the dialog's choices.
    pub fn apply_hole_table_dlg(&self, table: &mut noshape_model::drawing::Table) {
        let Some(d) = &self.hole_dlg else { return };
        if let TableKind::Hole { show_location, precision, bottom_to_top, flip_x, flip_y, exclude, x_dir, .. } = &mut table.kind {
            (*show_location, *precision, *bottom_to_top, *flip_x, *flip_y) = (d.show_location, d.precision, d.bottom_to_top, d.flip_x, d.flip_y);
            exclude.clone_from(&d.exclude);
            *x_dir = d.x_dir;
        }
        if let TableKind::Hole { more, .. } = &mut table.kind {
            *more = d.more.iter().filter_map(|(v, o)| o.as_ref().map(|o| noshape_model::drawing::HoleView { view: *v, origin: o.p, origin_ref: o.r.clone(), flip_x: false, flip_y: false, exclude: Vec::new(), x_dir: None })).collect();
        }
        table.corner = d.corner;
    }

    /// Every view of the table being inserted has its origin (live 1744: ✓
    /// waits for all of them).
    pub fn hole_origins_done(&self) -> bool {
        matches!(self.tool, Tool::Table { kind: TableTool::Hole, origin: Some(_), .. }) && self.hole_dlg.as_ref().is_none_or(|d| !d.adding_view && d.more.iter().all(|m| m.1.is_some()))
    }

    /// A click for a further view (after the Views box) or its origin; true
    /// when used. Only views of the first view's part are taken (live 1744:
    /// others aren't pickable).
    pub fn hole_more_click(&mut self, c: [f64; 2], cx: &mut Context<Self>) -> bool {
        let Tool::Table { kind: TableTool::Hole, view: Some(first), .. } = self.tool else { return false };
        let (adding, pending) = match &self.hole_dlg {
            Some(d) => (d.adding_view, d.more.iter().position(|m| m.1.is_none())),
            None => return false,
        };
        if adding {
            let sheet = self.current_sheet();
            let part = noshape_model::drawing::views::view_source(sheet, first);
            let taken: Vec<ViewId> = self.hole_dlg.as_ref().map(|d| d.more.iter().map(|m| m.0).collect()).unwrap_or_default();
            let v = self.view_at(c).filter(|v| *v != first && !taken.contains(v) && part.is_some() && noshape_model::drawing::views::view_source(sheet, *v) == part);
            if let (Some(v), Some(d)) = (v, &mut self.hole_dlg) {
                d.more.push((v, None));
                d.adding_view = false;
            }
            cx.notify();
            return true;
        }
        if let Some(i) = pending {
            let want = self.hole_dlg.as_ref().map(|d| d.more[i].0);
            if let (Some(p), Some(d)) = (self.snap_at(c).filter(|p| Some(p.view) == want), &mut self.hole_dlg) {
                d.more[i].1 = Some(p);
            }
            cx.notify();
            return true;
        }
        false
    }

    /// Place the hole table being inserted at the pointer's last spot (a
    /// click on the sheet, or ✓).
    pub fn place_hole_table(&mut self, cx: &mut Context<Self>) {
        let Tool::Table { kind: TableTool::Hole, view: Some(v), origin: Some(o) } = self.tool.clone() else { return };
        if !self.hole_origins_done() {
            return;
        }
        let id = self.current_sheet().next_table_id();
        let (mut table, name) = noshape_scene::drawing_tools::view_table(id, TableTool::Hole, v, Some(o), self.cursor);
        self.apply_hole_table_dlg(&mut table);
        self.edit_sheet(name, cx, |s| s.tables.push(table));
        self.hole_dlg = None;
        self.tool = Tool::Select;
        self.selection = vec![Sel::Table(id)];
        cx.notify();
    }

    /// The hole table being placed, at the pointer (live 1985: the table,
    /// the view's tags and the origin arrows follow it before the click).
    pub fn hole_table_preview(&self) -> Option<noshape_model::drawing::Table> {
        let Tool::Table { kind: TableTool::Hole, view: Some(v), origin: Some(o) } = &self.tool else { return None };
        if self.hole_dlg.as_ref().is_some_and(|d| d.picking || d.x_picking || d.y_picking) || !self.hole_origins_done() {
            return None;
        }
        let id = self.current_sheet().next_table_id();
        let (mut t, _) = noshape_scene::drawing_tools::view_table(id, TableTool::Hole, *v, Some(o.clone()), self.cursor);
        self.apply_hole_table_dlg(&mut t);
        Some(t)
    }

    /// A click while X direction takes a pick: a straight edge of the
    /// table's view sets the table's X axis along it (live 1985). True when
    /// used.
    /// The direction of view `v`'s straight model edge under the pointer
    /// (pointing right, or up when vertical), by geometry (with the table
    /// and datum tools the hover is the view, not its edge).
    pub fn straight_edge_dir_near(&self, v: noshape_model::drawing::ViewId) -> Option<[f64; 2]> {
        let tol = self.tol_mm(5.0);
        let g = self.geoms.get(&v)?;
        let pl = self.placement(v)?;
        let c = self.cursor;
        let near = |e: &noshape_model::drawing::views::ViewEdge| e.points.windows(2).any(|w| noshape_scene::drawing_tools::seg_dist(c, pl.to_sheet(w[0]), pl.to_sheet(w[1])) < tol);
        let me = g.edges.iter().filter(|e| e.visible && near(e)).find_map(|e| g.model_edges.get(e.model? as usize))?;
        let noshape_model::drawing::views::EdgeGeom::Line { a, b } = me.geom else { return None };
        let l = (b[0] - a[0]).hypot(b[1] - a[1]);
        (l > 1e-9).then(|| {
            let u = [(b[0] - a[0]) / l, (b[1] - a[1]) / l];
            if u[0] < -1e-9 || (u[0].abs() <= 1e-9 && u[1] < 0.0) { [-u[0], -u[1]] } else { u }
        })
    }

    pub fn hole_x_dir_click(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(d) = self.hole_dlg.as_ref().filter(|d| d.x_picking || d.y_picking) else { return false };
        let v = match (&self.tool, d.table.and_then(|id| self.current_sheet().table(id))) {
            (_, Some(t)) => t.view(),
            (Tool::Table { kind: TableTool::Hole, view, .. }, None) => *view,
            _ => None,
        };
        let Some(v) = v else { return false };
        let dir = self.straight_edge_dir_near(v);
        if let (Some(dir), Some(d)) = (dir, &mut self.hole_dlg) {
            // A Y edge: X a quarter turn clockwise from it (live 1682: Y
            // along the top edge → X down).
            d.y_set = d.y_picking;
            d.x_dir = Some(if d.y_picking { [dir[1], -dir[0]] } else { dir });
            (d.x_picking, d.y_picking) = (false, false);
        }
        self.update_preview(cx);
        cx.notify();
        true
    }

    /// A click while Holes to exclude takes picks (inserting, or Hole table
    /// properties…): a hole circle of the table's view toggles. True when
    /// used.
    pub fn hole_exclude_click(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(d) = self.hole_dlg.as_ref().filter(|d| d.picking) else { return false };
        let v = match (&self.tool, d.table.and_then(|id| self.current_sheet().table(id))) {
            (_, Some(t)) => t.view(),
            (Tool::Table { kind: TableTool::Hole, view, .. }, None) => *view,
            _ => None,
        };
        let Some(v) = v else { return false };
        if let Some(noshape_scene::drawing_tools::Pick::Circle { view, center, .. }) = self.pk().circle_pick(self.cursor)
            && view == v
            && let Some(d) = &mut self.hole_dlg
        {
            match d.exclude.iter().position(|e| (e[0] - center[0]).hypot(e[1] - center[1]) < 1e-7) {
                Some(i) => {
                    d.exclude.remove(i);
                }
                None => d.exclude.push(center),
            }
        }
        cx.notify();
        true
    }

    pub fn render_hole_table_dlg(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let d = self.hole_dlg.as_ref()?;
        let pal = p();
        let ink = gpui_kit::rgb(0x333333);
        let ed = cx.entity();
        let editing = d.table;
        let (view, origin) = match (&self.tool, editing.and_then(|id| self.current_sheet().table(id))) {
            (_, Some(t)) => (t.view(), true),
            (Tool::Table { view, .. }, None) => (*view, self.hole_origins_done()),
            _ => (None, false),
        };
        let first_origin = matches!(self.tool, Tool::Table { origin: Some(_), .. }) || editing.is_some();
        let view_label = |v: ViewId| self.current_sheet().view(v).map(|x| x.name.clone().unwrap_or_else(|| crate::drawing_props::default_view_name(&self.drawing, self.current_sheet(), x))).unwrap_or_default();
        let more: Vec<(ViewId, String, bool)> = d.more.iter().map(|(v, o)| (*v, view_label(*v), o.is_some())).collect();
        let view_name = view.and_then(|v| self.current_sheet().view(v)).map(|v| v.name.clone().unwrap_or_else(|| crate::drawing_props::default_view_name(&self.drawing, self.current_sheet(), v)));
        type Set = std::rc::Rc<dyn Fn(&mut HoleTableDlg)>;
        let select = |id: &'static str, label: String, w: f32, items: Vec<(String, bool, Set)>| {
            let ed = ed.clone();
            Button::new(id).label(label).ghost().xsmall().w(px(w)).dropdown_caret(true).border_b_1().border_color(gpui_kit::rgb(0xdddddd)).rounded(px(0.)).os_dropdown_menu(SharedString::from(format!("{id}-dd")), true, move |mut m, _, _| {
                for (l, on, f) in &items {
                    let (e, f) = (ed.clone(), f.clone());
                    m = m.item(crate::os_menu::OsMenuItem::new(l.clone()).checked(*on).on_click(move |_, _, cx| {
                        e.update(cx, |t, cx| {
                            if let Some(d) = &mut t.hole_dlg {
                                f(d);
                            }
                            cx.notify();
                        })
                    }));
                }
                m
            })
        };
        // "Location precision" on one line (it wrapped at 70 px, walk 10-28).
        let row = |label: &'static str, body: AnyElement| h_flex().h(px(28.)).px(px(8.)).child(div().w(px(98.)).flex_none().text_size(px(11.)).child(label)).child(body).into_any_element();
        let precisions: Vec<(String, bool, Set)> = PRECISIONS.iter().map(|&(l, p)| (l.to_string(), d.precision == p, std::rc::Rc::new(move |d: &mut HoleTableDlg| d.precision = p) as Set)).collect();
        let precision_label = PRECISIONS.iter().find(|(_, p)| *p == d.precision).map(|(l, _)| l.to_string()).unwrap_or_default();
        // Sheet ▾ (live drawing-hole-table-dialog): a placed table moves to
        // the sheet chosen (as Move to sheet…); a new one goes on this sheet.
        let sheet_select = {
            let names: Vec<String> = self.drawing.sheets.iter().map(|s| s.name.clone()).collect();
            let (cur, table, ed) = (self.sheet, d.table, ed.clone());
            Button::new("hole-table-sheet")
                .label(names.get(cur).cloned().unwrap_or_default())
                .ghost()
                .xsmall()
                .w(px(112.))
                .dropdown_caret(true)
                .border_b_1()
                .border_color(gpui_kit::rgb(0xdddddd))
                .rounded(px(0.))
                .os_dropdown_menu("hole-table-sheet-dd", true, move |mut m, _, _| {
                    for (i, n) in names.iter().enumerate() {
                        let e = ed.clone();
                        m = m.item(crate::os_menu::OsMenuItem::new(n.clone()).checked(i == cur).disabled(table.is_none() && i != cur).on_click(move |_, _, cx| {
                            e.update(cx, |t, cx| {
                                if let (Some(id), true) = (table, i != cur) {
                                    t.close_hole_table_props(true, cx);
                                    t.move_table_to_sheet(id, Some(i), cx);
                                }
                            })
                        }));
                    }
                    m
                })
                .into_any_element()
        };
        let orders: Vec<(String, bool, Set)> = vec![
            ("Top to bottom".into(), !d.bottom_to_top, std::rc::Rc::new(|d: &mut HoleTableDlg| d.bottom_to_top = false) as Set),
            ("Bottom to top".into(), d.bottom_to_top, std::rc::Rc::new(|d: &mut HoleTableDlg| d.bottom_to_top = true) as Set),
        ];
        let corner = d.corner;
        let corner_btn = |k: TableCorner, i: usize, cx: &mut Context<Self>| {
            let on = corner == k;
            let (top, left) = (matches!(k, TableCorner::TopLeft | TableCorner::TopRight), matches!(k, TableCorner::TopLeft | TableCorner::BottomLeft));
            div()
                .id(("hole-table-corner", i))
                .size(px(24.))
                .relative()
                .cursor_pointer()
                .border_1()
                .border_color(gpui_kit::rgb(0x909090))
                .when(on, |e| e.border_b_2().border_color(pal.accent))
                .child(crate::ui::glyph("table", 20., ink))
                .child(div().absolute().size(px(4.)).rounded_full().bg(ink).when(top, |e| e.top(px(1.))).when(!top, |e| e.bottom(px(1.))).when(left, |e| e.left(px(1.))).when(!left, |e| e.right(px(1.))))
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    if let Some(d) = &mut this.hole_dlg {
                        d.corner = k;
                    }
                    cx.notify();
                }))
        };
        let corners: Vec<AnyElement> = CORNERS.iter().enumerate().map(|(i, k)| corner_btn(*k, i, cx).into_any_element()).collect();
        let show = d.show_location;
        // The view field: "Top" once picked (live "Top ×"), then the origin.
        let field = |text: String, active: bool| {
            div().mx(px(8.)).my(px(2.)).px(px(6.)).h(px(24.)).flex().items_center().text_size(px(12.)).when(active, |d| d.bg(gpui_kit::rgb(0xdbf1fa))).child(text).into_any_element()
        };
        crate::ui_script::note_dialog(|| {
            // Every text shown, in reading order (README "### Drawings"; live 2131): the selects' options, a picked
            // view's and origin's chips with their ×.
            let mut text: Vec<String> = vec![if editing.is_some() { "Hole table properties" } else { "Hole table" }.into(), "Show location data".into(), "Location precision".into()];
            text.extend(PRECISIONS.iter().map(|(l, _)| l.to_string()));
            text.push("Views".into());
            if let Some(name) = &view_name {
                text.extend([name.clone(), "×".into(), "Origin".into()]);
                if first_origin {
                    text.extend(["Point".into(), "×".into()]);
                }
                text.extend(["X direction", "Y direction", "Holes to exclude", "X", "Y"].map(String::from));
            }
            text.push("Sheet".into());
            text.extend(self.drawing.sheets.iter().map(|s| s.name.clone()));
            text.extend(["Order", "Top to bottom", "Bottom to top", "Select fixed corner"].map(String::from));
            serde_json::json!({ "kind": "drawing-panel", "title": text[0], "text": text })
        });
        Some(
            v_flex()
                .id("dr-hole-table-dlg")
                .absolute()
                .top(px(DIALOG_TOP))
                .left(px(self.dialog_left()))
                .w(px(230.))
                .pb(px(6.))
                .bg(pal.dialog_bg)
                .shadow(crate::drawing_chrome::dialog_shadow())
                .text_color(ink)
                .text_size(px(12.))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    h_flex()
                        .h(px(28.))
                        .pl(px(11.))
                        .child(div().flex_1().text_size(px(13.)).font_weight(FontWeight::MEDIUM).child(if editing.is_some() { "Hole table properties" } else { "Hole table" }))
                        // Live: ✓ stays disabled until an origin is picked (a click places the table).
                        .child(
                            div()
                                .id("hole-table-ok")
                                .w(px(28.))
                                .h(px(27.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .bg(pal.accept)
                                .when(!origin, |d| d.opacity(0.3))
                                .child(crate::drawing_chrome::ok_check(16.))
                                .when(editing.is_some(), |d| d.cursor_pointer().on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.close_hole_table_props(true, cx))))
                                // Inserting (live request 1682): ✓ places the table with its
                                // fixed corner at the pointer's last spot on the sheet.
                                .when(editing.is_none() && origin, |d| d.cursor_pointer().on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.place_hole_table(cx)))),
                        )
                        .child(div().id("hole-table-close").w(px(28.)).h(px(27.)).flex().items_center().justify_center().cursor_pointer().child(crate::drawing_chrome::cancel_x(cx)).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            if editing.is_some() {
                                this.close_hole_table_props(false, cx);
                            } else {
                                this.tool = Tool::Select;
                                this.hole_dlg = None;
                                cx.notify();
                            }
                        }))),
                )
                .child(
                    h_flex().px(px(8.)).h(px(26.)).gap(px(6.)).child(crate::drawing_chrome::DrCheckbox::new("hole-table-loc").label("Show location data").checked(show).on_click(cx.listener(|this, on: &bool, _, cx| {
                        if let Some(d) = &mut this.hole_dlg {
                            d.show_location = *on;
                        }
                        cx.notify();
                    }))),
                )
                .when(show, |v| v.child(row("Location precision", select("hole-table-precision", precision_label, 112., precisions).into_any_element())))
                // Live drawing-hole-table-dialog: the Views field's tooltip.
                // A click on it with a view picked takes another view of the
                // same part (live request 1744).
                .child(
                    div()
                        .id("hole-table-views")
                        .tooltip(crate::ui::tooltip("Select views of a common singular part"))
                        .child(field(view_name.clone().unwrap_or_else(|| "Views".into()), view.is_none() || d.adding_view))
                        .when(view.is_some() && editing.is_none(), |e| {
                            e.cursor_pointer().on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                if let Some(d) = &mut this.hole_dlg {
                                    d.adding_view = !d.adding_view;
                                }
                                cx.notify();
                            }))
                        }),
                )
                .when(view.is_some(), |v| v.child(field(if first_origin { "Origin: Point".into() } else { "Origin: a vertex or circle center".into() }, !first_origin)))
                // Live 1985 / 1682: X direction and Y direction, each a
                // straight edge of the view; setting one hides the other (the
                // other axis is square to it); a click on a set one clears it.
                .when(view.is_some(), |v| {
                    let axis = |id: &'static str, label: &'static str, y: bool, cx: &mut Context<Self>| {
                        let on = if y { d.y_picking } else { d.x_picking };
                        let set = d.x_dir.is_some() && d.y_set == y;
                        div()
                            .id(id)
                            .mx(px(8.))
                            .my(px(2.))
                            .px(px(6.))
                            .h(px(24.))
                            .flex()
                            .items_center()
                            .text_size(px(12.))
                            .border_1()
                            .border_color(if on { pal.accent } else { gpui_kit::rgb(0xd3d7d9) })
                            .when(on, |d| d.bg(gpui_kit::rgb(0xdbf1fa)))
                            .cursor_pointer()
                            .child(if set { format!("{label}: Edge ×") } else { label.to_string() })
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                if let Some(d) = &mut this.hole_dlg {
                                    if set {
                                        (d.x_dir, d.y_set) = (None, false);
                                    } else if y {
                                        (d.y_picking, d.x_picking) = (!d.y_picking, false);
                                    } else {
                                        (d.x_picking, d.y_picking) = (!d.x_picking, false);
                                    }
                                    d.picking = false;
                                }
                                this.update_preview(cx);
                                cx.notify();
                            }))
                            .into_any_element()
                    };
                    let v = if d.x_dir.is_some() && d.y_set { v } else { v.child(axis("hole-table-x-dir", "X direction", false, cx)) };
                    if d.x_dir.is_some() && !d.y_set { v } else { v.child(axis("hole-table-y-dir", "Y direction", true, cx)) }
                })
                // Live 1609: the X / Y flip icons and Holes to exclude.
                .when(view.is_some(), |v| {
                    let flip = |id: &'static str, label: &'static str, on: bool, f: fn(&mut HoleTableDlg), cx: &mut Context<Self>| {
                        div()
                            .id(id)
                            .px(px(6.))
                            .h(px(22.))
                            .flex()
                            .items_center()
                            .border_1()
                            .border_color(gpui_kit::rgb(0xd3d7d9))
                            .when(on, |d| d.bg(gpui_kit::rgb(0xcfdbe5)))
                            .cursor_pointer()
                            .child(label)
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                if let Some(d) = &mut this.hole_dlg {
                                    f(d);
                                }
                                cx.notify();
                            }))
                    };
                    v.child(h_flex().px(px(8.)).h(px(28.)).gap(px(6.)).child(flip("hole-table-flip-x", "Flip X", d.flip_x, |d| d.flip_x = !d.flip_x, cx)).child(flip("hole-table-flip-y", "Flip Y", d.flip_y, |d| d.flip_y = !d.flip_y, cx)))
                })
                .when(view.is_some(), |v| {
                    let n = d.exclude.len();
                    let on = d.picking;
                    v.child(
                        div()
                            .id("hole-table-exclude")
                            .mx(px(8.))
                            .my(px(2.))
                            .px(px(6.))
                            .h(px(24.))
                            .flex()
                            .items_center()
                            .text_size(px(12.))
                            .border_1()
                            .border_color(if on { pal.accent } else { gpui_kit::rgb(0xd3d7d9) })
                            .when(on, |d| d.bg(gpui_kit::rgb(0xdbf1fa)))
                            .cursor_pointer()
                            .child(if n == 0 { "Holes to exclude".to_string() } else { format!("Holes to exclude: {n}") })
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                if let Some(d) = &mut this.hole_dlg {
                                    d.picking = !d.picking;
                                }
                                cx.notify();
                            })),
                    )
                })
                // The further views: their name (× drops it) and Origin.
                .children(more.into_iter().enumerate().map(|(i, (_, name, set))| {
                    v_flex()
                        .child(
                            h_flex()
                                .child(div().flex_1().child(field(name, false)))
                                .child(div().id(SharedString::from(format!("hole-table-more-x-{i}"))).px(px(8.)).cursor_pointer().child("×").on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    if let Some(d) = &mut this.hole_dlg {
                                        d.more.remove(i);
                                    }
                                    cx.notify();
                                }))),
                        )
                        .child(field(if set { "Origin: Point".into() } else { "Origin: a vertex or circle center".into() }, !set))
                }))
                .child(row("Sheet", sheet_select))
                .child(row("Order", select("hole-table-order", if d.bottom_to_top { "Bottom to top".into() } else { "Top to bottom".into() }, 112., orders).into_any_element()))
                .child(div().px(px(8.)).pt(px(4.)).text_size(px(11.)).child("Select fixed corner"))
                .child(h_flex().px(px(8.)).pt(px(4.)).gap(px(26.)).children(corners))
                .into_any_element(),
        )
    }
}
