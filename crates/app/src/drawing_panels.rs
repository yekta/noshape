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

    fn render_inspection_table(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let only = self.insp_table.current_sheet_only;
        let sheet = self.sheet + 1;
        let numbered = self.inspection_rows(false);
        // Datums are listed too (without an item number unless assigned).
        let mut rows = numbered.clone();
        for r in self.inspection_rows(true) {
            let datum = self.drawing.sheets.get(r.sheet - 1).and_then(|s| s.annotations.iter().find(|a| a.id == r.id)).is_some_and(|a| matches!(a.kind, AnnotationKind::Datum { .. }));
            if datum && r.number.is_empty() {
                rows.push(r);
            }
        }
        rows.retain(|r| !only || r.sheet == sheet);
        let ed = cx.entity();
        let ed2 = ed.clone();
        let view = Button::new("it-view").label(if only { "Current sheet" } else { "All sheets" }).ghost().small().dropdown_caret(true).os_dropdown_menu("it-view-dd", true, move |m, _, _| {
            let (a, b) = (ed2.clone(), ed2.clone());
            m.item(crate::os_menu::OsMenuItem::new("All sheets").checked(!only).on_click(move |_, _, cx| {
                a.update(cx, |t, cx| {
                    t.insp_table.current_sheet_only = false;
                    cx.notify();
                })
            }))
            .item(crate::os_menu::OsMenuItem::new("Current sheet").checked(only).on_click(move |_, _, cx| {
                b.update(cx, |t, cx| {
                    t.insp_table.current_sheet_only = true;
                    cx.notify();
                })
            }))
        });
        let ed3 = ed.clone();
        let more = Button::new("it-more").ghost().small().child(crate::ui::glyph("more", 14., pal.text_muted)).os_dropdown_menu("it-more-dd", false, move |m, _, _| {
            let e = ed3.clone();
            m.item(crate::os_menu::OsMenuItem::new("Export to CSV").on_click(move |_, w, cx| e.update(cx, |t, cx| t.export_inspection_list(false, w, cx))))
        });
        const COLS: [(&str, f32); 7] = [("Item", 46.), ("Sheet", 44.), ("Type", 110.), ("Nominal", 60.), ("Tolerance", 66.), ("Upper", 50.), ("Lower", 50.)];
        let cell = |text: SharedString, w: f32| div().w(px(w)).flex_none().px(px(4.)).overflow_hidden().whitespace_nowrap().text_ellipsis().child(text);
        let mut header = h_flex().h(px(26.)).bg(gpui_kit::rgb(0xefefef)).border_b_1().border_color(pal.border).text_size(px(12.)).font_weight(FontWeight::SEMIBOLD);
        for (c, w) in COLS {
            header = header.child(cell(c.into(), w));
        }
        let mut list = v_flex().id("it-rows").flex_1().min_h_0().overflow_y_scroll().text_size(px(12.));
        let editing = self.insp_table.editing.as_ref().map(|e| (e.0, e.1.clone()));
        let hl = self.insp_table.hover;
        let cur = self.sheet;
        for (i, r) in rows.iter().enumerate() {
            let id = r.id;
            let key = (r.sheet - 1, id);
            let on = hl == Some(key);
            let number = r.number.clone();
            let item: AnyElement = match &editing {
                Some((eid, st)) if *eid == key => div().w(px(COLS[0].1)).flex_none().child(Input::new(st).xsmall()).into_any_element(),
                _ => {
                    let empty = number.is_empty();
                    let n2 = number.clone();
                    div()
                        .id(("it-num", i))
                        .w(px(COLS[0].1))
                        .h_full()
                        .flex_none()
                        .px(px(4.))
                        .flex()
                        .items_center()
                        .child(number.clone())
                        .on_click(cx.listener(move |this, e: &ClickEvent, w, cx| {
                            if e.click_count() >= 2 && !n2.is_empty() && !n2.contains('.') {
                                this.edit_table_number(key, n2.clone(), w, cx);
                            }
                        }))
                        .when(empty, |d| {
                            d.on_mouse_down(
                                MouseButton::Right,
                                cx.listener(move |_, e: &MouseDownEvent, window, cx| {
                                    cx.stop_propagation();
                                    let entries = vec![MenuEntry::item("Assign number", move |t, _, cx| {
                                        let n = t.drawing.next_inspection_number();
                                        t.edit("Assign number", cx, |d| set_number(d, key, Some(n)));
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
            list = list.child(
                h_flex()
                    .id(("it-row", i))
                    .h(px(26.))
                    .border_b_1()
                    .border_color(gpui_kit::rgb(0xe6e6e6))
                    .when(on, |d| d.bg(gpui_kit::rgb(0xdbe9fb)).text_color(pal.accent))
                    // Cross-highlight: the row and its item on the sheet.
                    .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                        if *hovered || this.insp_table.hover == Some(key) {
                            this.insp_table.hover = hovered.then_some(key);
                            this.panel_highlight = (*hovered && key.0 == cur).then_some(Sel::Annotation(id));
                            this.redraw(cx);
                        }
                    }))
                    .child(item)
                    .child(cell(r.sheet.to_string().into(), COLS[1].1))
                    .child(cell(r.kind.clone().into(), COLS[2].1))
                    .child(cell(r.nominal.clone().into(), COLS[3].1))
                    .child(cell(r.tolerance.clone().into(), COLS[4].1))
                    .child(cell(r.upper.clone().into(), COLS[5].1))
                    .child(cell(r.lower.clone().into(), COLS[6].1)),
            );
        }
        if rows.is_empty() {
            list = list.child(div().p(px(12.)).text_color(pal.text_muted).child("No inspection items or datums"));
        }
        v_flex()
            .size_full()
            .child(h_flex().h(px(34.)).px(px(8.)).gap(px(6.)).child(div().text_size(px(12.)).text_color(pal.text_muted).child("View")).child(view).child(div().flex_1()).child(more))
            .child(div().id("it-scroll-x").overflow_x_scroll().child(v_flex().min_w(px(426.)).child(header).child(list)))
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

    pub fn render_right_panel(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let panel = self.right_panel?;
        let pal = p();
        let title = match panel {
            RightPanel::InspectionTable => "Inspection table",
            RightPanel::Styles => "Styles",
            RightPanel::MbdStatus => "MBD status",
        };
        let body = match panel {
            RightPanel::InspectionTable => self.render_inspection_table(cx),
            RightPanel::Styles => self.render_styles_panel(cx),
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
