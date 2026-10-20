//! Drawing editor: Onshape's right-side panel strip (live, drawing/09 and
//! /18: four 32 px buttons at the right edge, moved to the open panel's left
//! edge): Inspection table, Styles, Drawing properties, MBD status. The
//! Inspection table lists datums and inspection items (View: All sheets /
//! Current sheet; hovering a row cross-highlights; right-click an empty
//! Item cell > Assign number; double-click a number to edit it; ⋯ > Export
//! to CSV).

use crate::drawing_view::*;
use crate::theme::p;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use crate::os_ui::OsDropdownExt as _;
use gpui_kit::component::{Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::drawing::views::SourceKey;
use noshape_model::drawing::{AnnotationKind, ViewKind};

/// Panels docked at the right (Drawing properties keeps its own state).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RightPanel {
    InspectionTable,
    Styles,
    MbdStatus,
}

/// Inspection table state: View = all sheets (default) or the current
/// sheet; the number being edited.
#[derive(Default)]
pub struct InspectionTableUi {
    pub current_sheet_only: bool,
    /// The number being edited: (sheet index, annotation id).
    pub editing: Option<((usize, u64), Entity<InputState>, Subscription)>,
    /// The hovered row (sheet index, annotation id).
    pub hover: Option<(usize, u64)>,
    /// Collapsed sections (live: Datums, Characteristics).
    pub datums_closed: bool,
    pub chars_closed: bool,
}

/// Set the inspection number of annotation `id` on sheet `si`.
fn set_number(d: &mut noshape_model::drawing::Drawing, (si, id): (usize, u64), n: Option<u32>) {
    if let Some(a) = d.sheets.get_mut(si).and_then(|s| s.annotations.iter_mut().find(|a| a.id == id)) {
        a.inspection = n;
    }
}

/// Panel widths (Drawing properties: 369 px live).
const PANEL_W: f32 = 370.;

impl DrawingEditor {
    fn toggle_right_panel(&mut self, panel: Option<RightPanel>, cx: &mut Context<Self>) {
        match panel {
            // Drawing properties.
            None => {
                self.drawing_props.open = !self.drawing_props.open;
                self.right_panel = None;
            }
            Some(p) => {
                self.right_panel = if self.right_panel == Some(p) { None } else { Some(p) };
                self.drawing_props.open = false;
            }
        }
        self.panel_highlight = None;
        self.insp_table.hover = None;
        cx.notify();
    }

    pub fn render_panel_strip(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let open = self.drawing_props.open || self.right_panel.is_some();
        let btn = |id: &'static str, icon: &'static str, tip: &'static str, panel: Option<RightPanel>, on: bool, cx: &mut Context<Self>| {
            div()
                .id(id)
                .relative()
                .w(px(33.))
                .h(px(34.))
                .mt(px(-1.))
                .flex()
                .items_center()
                .justify_center()
                .bg(if on { pal.row_hover } else { pal.panel })
                .hover(|d| d.bg(pal.panel_hover))
                .cursor_pointer()
                .border_1()
                .border_color(pal.border_strong)
                .child(crate::assets::icon(icon, 20., cx))
                .tooltip(crate::ui::tooltip(tip))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.toggle_right_panel(panel, cx)))
        };
        let rp = self.right_panel;
        v_flex()
            .id("dr-panel-strip")
            .absolute()
            .top(relative(0.46))
            .right(px(if open { PANEL_W - 1. } else { -1. }))
            .occlude()
            // Live order (drawing/09).
            .child(btn("drp-insp", "inspection-table", "Inspection table", Some(RightPanel::InspectionTable), rp == Some(RightPanel::InspectionTable), cx))
            .child(btn("drp-styles", "drawing-styles", "Styles", Some(RightPanel::Styles), rp == Some(RightPanel::Styles), cx))
            .child(btn("drp-props", "drawing-props", "Drawing properties", None, self.drawing_props.open, cx))
            .child(btn("drp-mbd", "mbd-status", "MBD status", Some(RightPanel::MbdStatus), rp == Some(RightPanel::MbdStatus), cx))
            .into_any_element()
    }

    /// Double-click a number in the Inspection table: edit it in place.
    fn edit_table_number(&mut self, key: (usize, u64), n: String, window: &mut Window, cx: &mut Context<Self>) {
        let st = cx.new(|cx| InputState::new(window, cx).default_value(n));
        st.update(cx, |s, cx| s.focus(window, cx));
        let sub = cx.subscribe(&st, move |this: &mut Self, st, ev: &InputEvent, cx| {
            if !matches!(ev, InputEvent::PressEnter { .. } | InputEvent::Blur) {
                return;
            }
            let v = st.read(cx).value().trim().parse::<u32>().ok();
            this.insp_table.editing = None;
            if let Some(v) = v {
                this.edit("Edit inspection item", cx, |d| set_number(d, key, Some(v)));
            }
            cx.notify();
        });
        self.insp_table.editing = Some((key, st, sub));
        cx.notify();
    }

    /// The Inspection table (live drawing/95, 98, dom/drawing-panel-inspection-table.txt): "View:" with a
    /// bordered select (All sheets / Current sheet) and ⋯ (Export to CSV); the collapsible Datums section
    /// (its info box while there are none) and Characteristics: Item, Sheet, Type, Units, Nominal value,
    /// Tolerance, Upper limit, Lower limit, Nominal & Tolerance (the item bold, "--" when unnumbered, the
    /// other cells read-only grey), scrolling sideways.
    fn render_inspection_table(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let only = self.insp_table.current_sheet_only;
        let sheet = self.sheet + 1;
        let is_datum = |r: &noshape_model::drawing::inspection::InspectionRow| {
            self.drawing.sheets.get(r.sheet - 1).and_then(|s| s.annotations.iter().find(|a| a.id == r.id)).is_some_and(|a| matches!(a.kind, AnnotationKind::Datum { .. } | AnnotationKind::DatumTarget { .. }))
        };
        let mut all = self.inspection_rows(true);
        all.retain(|r| !only || r.sheet == sheet);
        let (datums, chars): (Vec<_>, Vec<_>) = all.into_iter().partition(|r| is_datum(r));
        let ed = cx.entity();
        let entity = ed.downgrade();
        let view = crate::os_ui::select_field("it-view", vec!["All sheets".into(), "Current sheet".into()], usize::from(only), move |i, _, cx| {
            let _ = entity.update(cx, |t: &mut Self, cx| {
                t.insp_table.current_sheet_only = i == 1;
                cx.notify();
            });
        })
        .w(px(130.))
        .h(px(26.))
        .text_size(px(13.));
        let ed3 = ed.clone();
        let more = Button::new("it-more").ghost().small().child(crate::ui::glyph("more", 14., pal.text_muted)).os_dropdown_menu("it-more-dd", false, move |m, _, _| {
            let e = ed3.clone();
            m.item(crate::os_menu::OsMenuItem::new("Export to CSV").on_click(move |_, w, cx| e.update(cx, |t, cx| t.export_inspection_list(false, w, cx))))
        });
        let ink = gpui_kit::rgb(0x333333);
        let rule = gpui_kit::hsla(0., 0., 0., 0.1);
        let section = |id: &'static str, label: &'static str, closed: bool, cx: &mut Context<Self>| {
            h_flex()
                .id(id)
                .h(px(28.))
                .px(px(6.))
                .gap(px(2.))
                .bg(gpui_kit::rgb(0xebebeb))
                .border_y_1()
                .border_color(rule)
                .cursor_pointer()
                .child(crate::ui::glyph(if closed { "expander-right" } else { "expander-down" }, 12., ink))
                .child(div().text_size(px(16.)).font_weight(FontWeight::BOLD).text_color(ink).child(label))
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    if id == "it-datums" {
                        this.insp_table.datums_closed = !this.insp_table.datums_closed;
                    } else {
                        this.insp_table.chars_closed = !this.insp_table.chars_closed;
                    }
                    cx.notify();
                }))
        };
        const COLS: [(&str, f32); 9] = [("Item", 87.), ("Sheet", 147.), ("Type", 147.), ("Units", 147.), ("Nominal value", 147.), ("Tolerance", 147.), ("Upper limit", 147.), ("Lower limit", 147.), ("Nominal & Tolerance", 147.)];
        let cell = |text: SharedString, w: f32, grey: bool| div().w(px(w)).h(px(34.)).flex_none().px(px(3.)).flex().items_center().overflow_hidden().whitespace_nowrap().text_ellipsis().border_r_1().border_color(rule).when(grey, |d| d.bg(gpui_kit::rgb(0xf6f6f6))).child(text);
        let sheet_name = |n: usize| format!("{} ({n})", self.drawing.sheets.get(n - 1).map(|s| s.name.as_str()).unwrap_or(""));
        let table = |rows: &[noshape_model::drawing::inspection::InspectionRow], key: &'static str, cx: &mut Context<Self>| -> AnyElement {
            let mut header = h_flex().h(px(34.)).bg(gpui_kit::rgb(0xebebeb)).border_b_1().border_color(gpui_kit::rgb(0xdddddd)).text_size(px(14.)).font_weight(FontWeight::BOLD).text_color(ink);
            for (c, w) in COLS {
                header = header.child(cell(c.into(), w, false));
            }
            let mut list = v_flex().text_size(px(14.)).text_color(ink);
            let editing = self.insp_table.editing.as_ref().map(|e| (e.0, e.1.clone()));
            let hl = self.insp_table.hover;
            let cur = self.sheet;
            for (i, r) in rows.iter().enumerate() {
                let id = r.id;
                let rk = (r.sheet - 1, id);
                let on = hl == Some(rk);
                let number = r.number.clone();
                let item: AnyElement = match &editing {
                    Some((eid, st)) if *eid == rk => div().w(px(COLS[0].1)).flex_none().child(Input::new(st).xsmall()).into_any_element(),
                    _ => {
                        let empty = number.is_empty();
                        let n2 = number.clone();
                        cell(if empty { "--".into() } else { number.clone().into() }, COLS[0].1, false)
                            .id((key, i))
                            .font_weight(FontWeight::BOLD)
                            .on_click(cx.listener(move |this, e: &ClickEvent, w, cx| {
                                if e.click_count() >= 2 && !n2.is_empty() && !n2.contains('.') {
                                    this.edit_table_number(rk, n2.clone(), w, cx);
                                }
                            }))
                            .when(empty, |d| {
                                d.on_mouse_down(
                                    MouseButton::Right,
                                    cx.listener(move |_, e: &MouseDownEvent, window, cx| {
                                        cx.stop_propagation();
                                        let entries = vec![MenuEntry::item("Assign number", move |t, _, cx| {
                                            let n = t.drawing.next_inspection_number();
                                            t.edit("Assign number", cx, |d| set_number(d, rk, Some(n)));
                                        })];
                                        let ed = cx.entity();
                                        let menu = crate::os_menu::OsMenu::build(window, cx, move |m, window, cx| fill_os_menu(m, &entries, &ed, window, cx));
                                        crate::os_menu::open(e.position, menu, window, cx);
                                    }),
                                )
                            })
                            .into_any_element()
                    }
                };
                let both = if r.tolerance.is_empty() { r.nominal.clone() } else { format!("{} {}", r.nominal, r.tolerance) };
                list = list.child(
                    h_flex()
                        .id((if key == "it-d" { "it-drow" } else { "it-crow" }, i))
                        .border_t_1()
                        .border_color(rule)
                        .when(on, |d| d.bg(gpui_kit::rgb(0xdbe9fb)).text_color(pal.accent))
                        // Cross-highlight: the row and its item on the sheet.
                        .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                            if *hovered || this.insp_table.hover == Some(rk) {
                                this.insp_table.hover = hovered.then_some(rk);
                                this.panel_highlight = (*hovered && rk.0 == cur).then_some(Sel::Annotation(id));
                                this.redraw(cx);
                            }
                        }))
                        .child(item)
                        .child(cell(sheet_name(r.sheet).into(), COLS[1].1, true))
                        .child(cell(r.kind.clone().into(), COLS[2].1, true))
                        .child(cell(r.unit.clone().into(), COLS[3].1, true))
                        .child(cell(r.nominal.clone().into(), COLS[4].1, true))
                        .child(cell(r.tolerance.clone().into(), COLS[5].1, true))
                        .child(cell(r.upper.clone().into(), COLS[6].1, true))
                        .child(cell(r.lower.clone().into(), COLS[7].1, true))
                        .child(cell(both.into(), COLS[8].1, true)),
                );
            }
            let width: f32 = COLS.iter().map(|c| c.1).sum();
            div().id(key).mx(px(1.)).overflow_x_scroll().border_1().border_color(gpui_kit::rgb(0xdddddd)).child(v_flex().min_w(px(width)).child(header).child(list)).into_any_element()
        };
        let info = v_flex()
            .m(px(5.))
            .p(px(15.))
            .gap(px(4.))
            .rounded(px(4.))
            .bg(gpui_kit::rgb(0xd9edf7))
            .border_1()
            .border_color(gpui_kit::rgb(0xbce8f1))
            .text_size(px(14.))
            .line_height(px(20.))
            .text_color(gpui_kit::rgb(0x31708f))
            .child(div().font_weight(FontWeight::BOLD).child("Add datums or datum targets to build an inspection table"))
            .child(div().child("An inspection table reveals all the datums and datum targets within the drawing. Add datums or datum targets to see the table by the datums name."));
        let (dc, cc) = (self.insp_table.datums_closed, self.insp_table.chars_closed);
        let datums_body = if datums.is_empty() { info.into_any_element() } else { table(&datums, "it-d", cx) };
        let chars_body = table(&chars, "it-c", cx);
        v_flex()
            .size_full()
            .child(h_flex().h(px(36.)).mt(px(5.)).px(px(7.)).gap(px(5.)).child(div().text_size(px(12.)).text_color(gpui_kit::rgb(0x666666)).child("View:")).child(view).child(div().flex_1()).child(more))
            .child(
                v_flex()
                    .id("it-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .border_b_1()
                    .border_color(gpui_kit::rgb(0xcccccc))
                    .child(section("it-datums", "Datums", dc, cx))
                    .when(!dc, |e| e.child(datums_body))
                    .child(section("it-chars", "Characteristics", cc, cx))
                    .when(!cc, |e| e.child(chars_body)),
            )
            .into_any_element()
    }

    /// MBD status (live drawing/95, dom/drawing-panel-mbd-status.txt):
    /// "Model:" (Current sheet / All models / the referenced parts), the
    /// Datums and Characteristics sections and, while the referenced models
    /// have no MBD annotations, the blue info box.
    fn render_mbd_status(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let section = |label: &'static str| {
            h_flex()
                .h(px(28.))
                .px(px(6.))
                .gap(px(2.))
                .bg(gpui_kit::rgb(0xebebeb))
                .border_y_1()
                .border_color(gpui_kit::hsla(0., 0., 0., 0.1))
                .child(crate::ui::glyph("expander-down", 12., gpui_kit::rgb(0x333333)))
                .child(div().text_size(px(16.)).font_weight(FontWeight::BOLD).text_color(gpui_kit::rgb(0x333333)).child(label))
        };
        let mut models: Vec<String> = vec!["Current sheet".into(), "All models".into()];
        // The referenced parts (or Part Studios / assemblies shown whole).
        for v in self.drawing.sheets.iter().flat_map(|s| &s.views) {
            let ViewKind::Base { source, part, .. } = &v.kind else { continue };
            let name = match part {
                Some(b) => self.sources.get(&SourceKey::studio(*source)).and_then(|s| s.context.bodies.iter().find(|x| &x.id == b).map(|x| x.name.clone())),
                None => self.doc.read(cx).session.doc.tab(*source).map(|t| t.name.clone()),
            };
            if let Some(name) = name
                && !models.contains(&name)
            {
                models.push(name);
            }
        }
        let current = self.mbd_model.min(models.len() - 1);
        let entity = cx.entity().downgrade();
        let model = crate::os_ui::select_field("mbd-model", models, current, move |i, _, cx| {
            let _ = entity.update(cx, |t: &mut Self, cx| {
                t.mbd_model = i;
                cx.notify();
            });
        })
        .w(px(130.))
        .h(px(26.))
        .text_size(px(13.));
        v_flex()
            .size_full()
            .child(h_flex().h(px(36.)).mt(px(5.)).px(px(7.)).gap(px(5.)).child(div().text_size(px(12.)).text_color(gpui_kit::rgb(0x666666)).child("Model:")).child(model))
            .child(section("Datums").mt(px(5.)))
            .child(section("Characteristics").mt(px(5.)))
            .child(
                v_flex()
                    .m(px(5.))
                    .p(px(15.))
                    .gap(px(4.))
                    .rounded(px(4.))
                    .bg(gpui_kit::rgb(0xd9edf7))
                    .border_1()
                    .border_color(gpui_kit::rgb(0xbce8f1))
                    .text_size(px(14.))
                    .line_height(px(20.))
                    .text_color(gpui_kit::rgb(0x31708f))
                    .child(div().font_weight(FontWeight::BOLD).child("Add dimensions and annotations to build an MBD status table."))
                    .child(div().child("An MBD status table reveals all the MBD requirements within the document.")),
            )
            .into_any_element()
    }

    pub fn render_right_panel(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let panel = self.right_panel?;
        let pal = p();
        let title = match panel {
            RightPanel::InspectionTable => "Inspection table",
            RightPanel::Styles => "Styles",
            RightPanel::MbdStatus => "MBD status",
        };
        let body = match panel {
            RightPanel::InspectionTable => self.render_inspection_table(cx),
            RightPanel::Styles => self.render_styles_panel(window, cx),
            RightPanel::MbdStatus => self.render_mbd_status(cx),
        };
        Some(
            v_flex()
                .id("dr-right-panel")
                .absolute()
                .top_0()
                .right_0()
                .bottom_0()
                .w(px(PANEL_W))
                .bg(gpui_kit::rgb(0xfafafa))
                .border_l_1()
                .border_color(pal.border)
                .text_color(pal.text)
                .occlude()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                // Live dom/drawing-panel-*.txt: a 34 px header, the title
                // centred (18.67 px / 500, #1b5faa) over a 2 px #1b5faa
                // underline; no close button (the strip icon closes it).
                .child(
                    h_flex()
                        .h(px(34.))
                        .flex_none()
                        .border_b(px(2.))
                        .border_color(gpui_kit::rgb(0x1b5faa))
                        .child(div().flex_1().text_center().text_size(px(18.67)).font_weight(FontWeight::MEDIUM).text_color(gpui_kit::rgb(0x1b5faa)).child(title)),
                )
                .child(div().flex_1().min_h_0().child(body))
                .into_any_element(),
        )
    }
}
