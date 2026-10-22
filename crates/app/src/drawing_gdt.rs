//! Drawing editor: Onshape's Geometric tolerance dialog (live
//! drawing/200–215, dom/drawing-gdt-*): Upper text; the label row Prefix ·
//! Type · Tolerance · Datums · Suffix; one frame row per frame (prefix box,
//! Type dropdown, the tolerance box with its before / value / after slots,
//! three datum boxes, suffix box); the button row [+] Add frame, Leader ▾
//! (Leader, All around, All over) and Ø (Show/hide symbols: the palette,
//! whose symbols are enabled per focused slot); Lower text. With the tool
//! active, clicks place frames (the dialog stays open; Esc closes it);
//! Edit… / double-click edits one (✓ applies).

use crate::drawing_view::*;
use crate::os_ui::OsDropdownExt as _;
use crate::theme::p;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{Disableable as _, Selectable as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::drawing::{AnnotationKind, GdtExtra, GdtLeader};

/// The Type list (glyph, name) in Onshape's order.
pub use noshape_scene::drawing_tools::GDT_TYPES;

/// The symbol palette (glyph, tooltip) in Onshape's order.
const PALETTE: [(&str, &str); 28] = [
    ("Ø", "Specify diameter"),
    ("SØ", "Spherical diameter"),
    ("SR", "Spherical radius"),
    ("Ⓜ", "Maximum material condition"),
    ("Ⓛ", "Least material condition"),
    ("Ⓕ", "Free state"),
    ("Ⓣ", "Tangent plane"),
    ("Ⓢ", "Regardless of feature size"),
    ("Ⓟ", "Projected tolerance zone"),
    ("Ⓤ", "Unequally disposed profile"),
    ("CR", "Controlled radius"),
    ("R", "Radius"),
    ("CF", "Continuous feature"),
    ("ST", "Statistical tolerance"),
    ("□", "Square"),
    ("Ⓘ", "Independency"),
    ("▷", "Translation"),
    ("SF", "Spot face"),
    ("↔", "Between"),
    ("⟼", "From to"),
    ("⟷", "From to"),
    ("°", "Degree"),
    ("℄", "Centerline"),
    ("⌵", "Countersink"),
    ("⌴", "Counterbore"),
    ("↧", "Depth"),
    ("⌒", "Arc length"),
    ("±", "Plus/minus"),
];

/// Slots of a frame row, in [`GdtRow::inputs`] order.
const PREFIX: usize = 0;
const BEFORE: usize = 1;
const VALUE: usize = 2;
const AFTER: usize = 3;
const DATUM0: usize = 4;
const SUFFIX: usize = 7;

/// One frame row of the dialog: its Type (index into [`GDT_TYPES`], or
/// None = Composite) and its boxes.
pub struct GdtRow {
    pub symbol: Option<usize>,
    inputs: Vec<Entity<InputState>>,
}

/// Geometric tolerance dialog state.
pub struct GdtDlg {
    /// Editing this frame (else the tool's settings for new frames).
    pub edit: Option<u64>,
    pub rows: Vec<GdtRow>,
    upper: Entity<InputState>,
    lower: Entity<InputState>,
    pub leader: GdtLeader,
    pub symbols: bool,
    /// The focused slot (row, slot; usize::MAX rows = upper / lower text).
    focus: (usize, usize),
}

/// Split a tolerance cell "Ø0.1Ⓜ" into before / value / after.
fn split_tolerance(cell: &str) -> (String, String, String) {
    let before: String = ["SØ", "Ø", "⌀"].iter().find(|p| cell.starts_with(**p)).map(|p| p.to_string()).unwrap_or_default();
    let rest = &cell[before.len()..];
    let after: String = rest.chars().rev().take_while(|c| "ⓂⓁⒻⓅⓈⓉⓊ".contains(*c)).collect::<Vec<_>>().into_iter().rev().collect();
    (before, rest[..rest.len() - after.len()].to_string(), after)
}

/// Symbols allowed in a slot (Onshape enables the rest only in texts).
fn allowed(slot: usize, sym: &str) -> bool {
    match slot {
        BEFORE => matches!(sym, "Ø" | "SØ"),
        VALUE => false,
        AFTER => matches!(sym, "Ⓜ" | "Ⓛ" | "Ⓕ" | "Ⓟ"),
        s if (DATUM0..DATUM0 + 3).contains(&s) => matches!(sym, "Ⓜ" | "Ⓛ" | "Ⓕ"),
        _ => true,
    }
}

impl DrawingEditor {
    fn gdt_row(&mut self, symbol: Option<usize>, cells: &[String], prefix: &str, suffix: &str, window: &mut Window, cx: &mut Context<Self>) -> GdtRow {
        let (before, value, after) = split_tolerance(cells.get(1).map(String::as_str).unwrap_or(""));
        let vals = [prefix.to_string(), before, value, after, cells.get(2).cloned().unwrap_or_default(), cells.get(3).cloned().unwrap_or_default(), cells.get(4).cloned().unwrap_or_default(), suffix.to_string()];
        let inputs = vals.into_iter().map(|v| cx.new(|cx| InputState::new(window, cx).select_on_focus(false).revert_on_escape(false).default_value(v))).collect();
        GdtRow { symbol, inputs }
    }

    fn new_gdt_dlg(&mut self, edit: Option<u64>, frame: &[String], extra: &GdtExtra, window: &mut Window, cx: &mut Context<Self>) -> GdtDlg {
        let sym = |c: Option<&String>| c.and_then(|s| GDT_TYPES.iter().position(|(g, _)| g == s));
        let first_sym = sym(frame.first()).or(Some(0));
        let mut rows = vec![self.gdt_row(first_sym, frame, extra.prefix.first().map(String::as_str).unwrap_or(""), extra.suffix.first().map(String::as_str).unwrap_or(""), window, cx)];
        for (i, r) in extra.rows.iter().enumerate() {
            let row = self.gdt_row(sym(r.first()), r, extra.prefix.get(i + 1).map(String::as_str).unwrap_or(""), extra.suffix.get(i + 1).map(String::as_str).unwrap_or(""), window, cx);
            rows.push(row);
        }
        let upper = cx.new(|cx| InputState::new(window, cx).select_on_focus(false).revert_on_escape(false).placeholder_style(gpui_kit::base::input::PlaceholderStyle { italic: true, weight: Some(gpui_kit::FontWeight::NORMAL), color: Some(gpui_kit::rgb(0x707070).into()) }).default_value(extra.upper.clone()));
        let lower = cx.new(|cx| InputState::new(window, cx).select_on_focus(false).revert_on_escape(false).placeholder_style(gpui_kit::base::input::PlaceholderStyle { italic: true, weight: Some(gpui_kit::FontWeight::NORMAL), color: Some(gpui_kit::rgb(0x707070).into()) }).default_value(extra.lower.clone()));
        // The Tolerance field takes the keys at once (qa5 D7: it looked
        // focused but typing went to the canvas). Deferred: the dialog is
        // created while rendering.
        let tol = rows[0].inputs[VALUE].downgrade();
        window.defer(cx, move |window, cx| {
            if let Some(st) = tol.upgrade() {
                st.update(cx, |s, cx| s.focus(window, cx));
            }
        });
        GdtDlg { edit, rows, upper, lower, leader: extra.leader, symbols: false, focus: (0, VALUE) }
    }

    /// Create / drop the dialog with the Geometric tolerance tool.
    pub fn sync_gdt_dlg(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let active = matches!(self.tool, Tool::Gdt);
        let placing = self.gdt_dlg.as_ref().is_some_and(|d| d.edit.is_none());
        if active && self.gdt_dlg.is_none() {
            self.gdt_dlg = Some(self.new_gdt_dlg(None, &[], &GdtExtra::default(), window, cx));
        } else if !active && placing {
            self.gdt_dlg = None;
        }
        self.limit_gdt_texts(window, cx);
    }

    /// The help's limits (H:geometric_tolerance): Upper text 50 characters,
    /// Lower text 200, Prefix / Suffix 20 each, up to 7 symbols on each side
    /// of the tolerance.
    fn limit_gdt_texts(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(d) = &self.gdt_dlg else { return };
        let mut boxes: Vec<(Entity<InputState>, usize)> = vec![(d.upper.clone(), 50), (d.lower.clone(), 200)];
        for r in &d.rows {
            boxes.extend([(r.inputs[PREFIX].clone(), 20), (r.inputs[SUFFIX].clone(), 20), (r.inputs[BEFORE].clone(), 7), (r.inputs[AFTER].clone(), 7)]);
        }
        for (st, max) in boxes {
            let v = st.read(cx).value().to_string();
            if v.chars().count() > max {
                let cut: String = v.chars().take(max).collect();
                st.update(cx, |s, cx| s.set_value(cut, window, cx));
            }
        }
    }

    /// Edit… / double-click on a geometric tolerance.
    pub fn open_gdt_edit(&mut self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        let Some((frame, extra)) = self.current_sheet().annotations.iter().find(|a| a.id == id).and_then(|a| match &a.kind {
            AnnotationKind::Gdt { frame, extra, .. } => Some((frame.clone(), (**extra).clone())),
            _ => None,
        }) else {
            return;
        };
        self.tool = Tool::Select;
        self.gdt_dlg = Some(self.new_gdt_dlg(Some(id), &frame, &extra, window, cx));
        cx.notify();
    }

    /// The dialog's frame (first row) and extra, None when nothing is entered.
    fn gdt_values(&self, cx: &App) -> Option<(Vec<String>, GdtExtra)> {
        let d = self.gdt_dlg.as_ref()?;
        let v = |e: &Entity<InputState>| e.read(cx).value().to_string();
        let rows: Vec<noshape_scene::drawing_tools::GdtRowValues> = d
            .rows
            .iter()
            .map(|r| noshape_scene::drawing_tools::GdtRowValues {
                symbol: r.symbol,
                prefix: v(&r.inputs[PREFIX]),
                before: v(&r.inputs[BEFORE]),
                value: v(&r.inputs[VALUE]),
                after: v(&r.inputs[AFTER]),
                datums: [0, 1, 2].map(|k| v(&r.inputs[DATUM0 + k])),
                suffix: v(&r.inputs[SUFFIX]),
            })
            .collect();
        noshape_scene::drawing_tools::gdt_values(&rows, &v(&d.upper), &v(&d.lower), d.leader)
    }

    /// A click with the tool: place a frame there (the dialog stays open).
    pub fn gdt_click(&mut self, cx: &mut Context<Self>) -> bool {
        if !matches!(self.tool, Tool::Gdt) {
            return false;
        }
        let Some((frame, extra)) = self.gdt_values(cx) else { return true };
        let c = self.cursor;
        let id = self.current_sheet().next_annotation_id();
        self.edit_sheet("Geometric tolerance", cx, |s| s.annotations.push(noshape_scene::drawing_tools::gdt(id, c, frame, extra)));
        cx.notify();
        true
    }

    pub fn close_gdt_dlg(&mut self, ok: bool, cx: &mut Context<Self>) {
        let values = self.gdt_values(cx);
        let Some(d) = self.gdt_dlg.take() else { return };
        if ok
            && let (Some(id), Some((frame, extra))) = (d.edit, values)
        {
            self.edit_sheet("Edit geometric tolerance", cx, |s| {
                if let Some(AnnotationKind::Gdt { frame: f, extra: e, .. }) = s.annotations.iter_mut().find(|a| a.id == id).map(|a| &mut a.kind) {
                    *f = frame;
                    **e = extra;
                }
            });
        }
        if d.edit.is_none() {
            self.tool = Tool::Select;
        }
        cx.notify();
    }

    fn gdt_insert(&mut self, sym: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(d) = &self.gdt_dlg else { return };
        let (row, slot) = d.focus;
        let input = match row {
            usize::MAX if slot == 0 => d.upper.clone(),
            usize::MAX => d.lower.clone(),
            r => match d.rows.get(r) {
                Some(r) => r.inputs[slot].clone(),
                None => return,
            },
        };
        let sym = sym.to_string();
        input.update(cx, |s, cx| {
            let v = format!("{}{sym}", s.value());
            s.set_value(v, window, cx);
        });
        cx.notify();
    }

    pub fn render_gdt_dlg(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let entered = self.gdt_values(cx).is_some();
        let d = self.gdt_dlg.as_ref()?;
        let pal = p();
        let ink = gpui_kit::rgb(0x333333);
        let ed = cx.entity();
        let focus = d.focus;
        let leader = d.leader;
        let show_symbols = d.symbols;
        let frames = d.rows.len();
        let (upper, lower) = (d.upper.clone(), d.lower.clone());
        let rows: Vec<(Option<usize>, Vec<Entity<InputState>>)> = d.rows.iter().map(|r| (r.symbol, r.inputs.clone())).collect();
        // A slot (live dom drawing-gdt-dialog-empty): a borderless input over a
        // `#999` underline; it records the focused slot (for the symbol palette).
        let slot_box = |row: usize, slot: usize, input: &Entity<InputState>, w: f32, cx: &mut Context<Self>| {
            div()
                .w(px(w))
                .h(px(24.))
                .border_b_1()
                .border_color(gpui_kit::rgb(0x999999))
                .text_size(px(16.))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _: &MouseDownEvent, _, cx| {
                        if let Some(d) = &mut this.gdt_dlg {
                            d.focus = (row, slot);
                        }
                        cx.notify();
                    }),
                )
                .child(Input::new(input).text_size(px(21.)).appearance(false).small().text_color(gpui_kit::rgb(0x555555)).px(px(0.)))
        };
        // A 69 x 34 cell: 1 px `#ccc`, radius 4, the slot 6 px in.
        let cell = |row: usize, slot: usize, input: &Entity<InputState>, cx: &mut Context<Self>| div().w(px(69.)).h(px(34.)).flex_none().px(px(6.)).pt(px(2.)).border_1().border_color(gpui_kit::rgb(0xcccccc)).rounded(px(4.)).bg(gpui_kit::white()).child(slot_box(row, slot, input, 55., cx));
        // Label columns as live: Prefix 87, Type 67, Tolerance 239, Datums 245, Suffix.
        let label_row = h_flex().h(px(20.)).mt(px(3.)).text_size(px(14.)).child(div().w(px(87.)).child("Prefix")).child(div().w(px(67.)).child("Type")).child(div().w(px(239.)).child("Tolerance")).child(div().w(px(245.)).child("Datums")).child(div().child("Suffix"));
        let sep = || div().w(px(1.)).h(px(34.)).mx(px(8.)).flex_none().bg(gpui_kit::rgb(0xdddddd));
        let mut frame_rows = v_flex().mt(px(6.)).gap(px(6.));
        for (ri, (symbol, inputs)) in rows.iter().enumerate() {
            // Type: the 14 types; rows after the first start with Composite.
            let ed2 = ed.clone();
            let cur = *symbol;
            let label = match cur {
                Some(i) => GDT_TYPES[i].0.to_string(),
                None => "⌖⌖".to_string(),
            };
            // Live: a 50 x 30 white button, 1 px `#ccc`, radius 4: the symbol and a caret.
            let type_btn = Button::new(("gdt-type", ri)).child(div().text_size(px(18.)).child(label)).ghost().small().w(px(50.)).h(px(30.)).text_size(px(16.)).border_1().border_color(gpui_kit::rgb(0xcccccc)).rounded(px(4.)).bg(gpui_kit::white()).dropdown_caret(true).tooltip(match cur {
                Some(i) => GDT_TYPES[i].1,
                None => "Composite",
            });
            let type_btn = type_btn.os_dropdown_menu(SharedString::from(format!("gdt-type-{ri}-dd")), false, move |mut m, _, _| {
                let mut items: Vec<(Option<usize>, String)> = Vec::new();
                if ri > 0 {
                    items.push((None, "Composite".into()));
                }
                items.extend(GDT_TYPES.iter().enumerate().map(|(i, (g, n))| (Some(i), format!("{g}  {n}"))));
                for (v, l) in items {
                    let e = ed2.clone();
                    m = m.item(crate::os_menu::OsMenuItem::new(l).checked(v == cur).on_click(move |_, _, cx| {
                        e.update(cx, |t, cx| {
                            if let Some(r) = t.gdt_dlg.as_mut().and_then(|d| d.rows.get_mut(ri)) {
                                r.symbol = v;
                            }
                            cx.notify();
                        })
                    }));
                }
                m
            });
            // Right-click a later row: Move up / Move down / Remove.
            let n = rows.len();
            let row_menu = cx.listener(move |_, e: &MouseDownEvent, window, cx| {
                if ri == 0 {
                    return;
                }
                cx.stop_propagation();
                let mut entries: Vec<MenuEntry> = Vec::new();
                if ri > 1 {
                    entries.push(MenuEntry::item("Move up", move |t, _, cx| {
                        if let Some(d) = &mut t.gdt_dlg {
                            d.rows.swap(ri, ri - 1);
                        }
                        cx.notify();
                    }));
                }
                if ri + 1 < n {
                    entries.push(MenuEntry::item("Move down", move |t, _, cx| {
                        if let Some(d) = &mut t.gdt_dlg {
                            d.rows.swap(ri, ri + 1);
                        }
                        cx.notify();
                    }));
                }
                entries.push(MenuEntry::item("Remove", move |t, _, cx| {
                    if let Some(d) = &mut t.gdt_dlg
                        && ri < d.rows.len()
                    {
                        d.rows.remove(ri);
                    }
                    cx.notify();
                }));
                let ed = cx.entity();
                let menu = crate::os_menu::OsMenu::build(window, cx, move |m, window, cx| fill_os_menu(m, &entries, &ed, window, cx));
                crate::os_menu::open(e.position, menu, window, cx);
            });
            frame_rows = frame_rows.child(
                h_flex()
                    .pl(px(1.))
                    .on_mouse_down(MouseButton::Right, row_menu)
                    .child(cell(ri, PREFIX, &inputs[PREFIX], cx))
                    .child(sep())
                    .child(type_btn)
                    .child(div().w(px(16.)))
                    .child({
                        // The tolerance cell: 221 x 34, the active one 2 px `#abcbff`.
                        let active = focus.0 == ri && (BEFORE..=AFTER).contains(&focus.1);
                        h_flex()
                            .w(px(221.))
                            .h(px(34.))
                            .flex_none()
                            .px(px(6.))
                            .pt(px(2.))
                            .rounded(px(4.))
                            .bg(gpui_kit::white())
                            .when(active, |d| d.border_2().border_color(gpui_kit::rgb(0xabcbff)))
                            .when(!active, |d| d.border_1().border_color(gpui_kit::rgb(0xcccccc)))
                            .child(slot_box(ri, BEFORE, &inputs[BEFORE], 69., cx))
                            .child(div().w(px(6.)))
                            .child(slot_box(ri, VALUE, &inputs[VALUE], 49., cx))
                            .child(div().w(px(6.)))
                            .child(slot_box(ri, AFTER, &inputs[AFTER], 69., cx))
                    })
                    .child(sep())
                    .child(cell(ri, DATUM0, &inputs[DATUM0], cx))
                    .child(div().w(px(10.)))
                    .child(cell(ri, DATUM0 + 1, &inputs[DATUM0 + 1], cx))
                    .child(div().w(px(10.)))
                    .child(cell(ri, DATUM0 + 2, &inputs[DATUM0 + 2], cx))
                    .child(sep())
                    .child(cell(ri, SUFFIX, &inputs[SUFFIX], cx)),
            );
        }
        let ed3 = ed.clone();
        let leader_btn = Button::new("gdt-leader").child(div().text_size(px(16.)).child("↙")).ghost().small().w(px(50.)).h(px(30.)).text_size(px(16.)).border_1().border_color(gpui_kit::rgb(0xcccccc)).rounded(px(4.)).bg(gpui_kit::white()).dropdown_caret(true).tooltip("Leader").os_dropdown_menu("gdt-leader-dd", false, move |mut m, _, _| {
            for (l, v) in [("Leader", GdtLeader::Leader), ("All around", GdtLeader::AllAround), ("All over", GdtLeader::AllOver)] {
                let e = ed3.clone();
                m = m.item(crate::os_menu::OsMenuItem::new(l).checked(leader == v).on_click(move |_, _, cx| {
                    e.update(cx, |t, cx| {
                        if let Some(d) = &mut t.gdt_dlg {
                            d.leader = v;
                        }
                        cx.notify();
                    })
                }));
            }
            m
        });
        let palette = show_symbols.then(|| {
            let mut grid = h_flex().flex_wrap().gap(px(2.));
            for (i, (sym, tip)) in PALETTE.iter().enumerate() {
                let enabled = if focus.0 == usize::MAX || focus.1 == PREFIX || focus.1 == SUFFIX { true } else { allowed(focus.1, sym) };
                grid = grid.child(
                    div()
                        .id(("gdt-sym", i))
                        .size(px(30.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .border_1()
                        .border_color(gpui_kit::rgb(0xcccccc))
                        .text_size(px(14.))
                        .tooltip(crate::ui::tooltip(*tip))
                        .when(!enabled, |d| d.opacity(0.3))
                        .when(enabled, |d| d.cursor_pointer().on_click(cx.listener(move |this, _: &ClickEvent, window, cx| this.gdt_insert(sym, window, cx))))
                        .child(*sym),
                );
            }
            grid
        });
        // Upper / Lower text: a 28 px underlined field, its placeholder italic
        // `#999` (live 200).
        let text_box = |which: usize, input: &Entity<InputState>, placeholder: &'static str, cx: &mut Context<Self>| {
            div()
                .relative()
                .h(px(28.))
                .border_b_1()
                .border_color(gpui_kit::rgb(0x999999))
                .text_size(px(14.))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _: &MouseDownEvent, _, cx| {
                        if let Some(d) = &mut this.gdt_dlg {
                            d.focus = (usize::MAX, which);
                        }
                        cx.notify();
                    }),
                )
                .child(crate::ui::input_with_placeholder(input, placeholder, cx).appearance(false).small().text_size(px(21.)).text_color(gpui_kit::rgb(0x555555)).pl(px(0.)))
        };
        // Live: ✓ is enabled once something is entered (placing: it ends the tool).
        let can_accept = entered;
        Some(
            v_flex()
                .id("dr-gdt-dlg")
                .absolute()
                .top(px(DIALOG_TOP))
                .left(px(self.dialog_left()))
                .w(px(726.))
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
                        .border_b_1()
                        .border_color(gpui_kit::rgb(0xd3d7d9))
                        .child(div().flex_1().text_size(px(13.)).font_weight(FontWeight::MEDIUM).child("Geometric tolerance"))
                        .child(div().id("gdt-ok").w(px(28.)).h(px(27.)).flex().items_center().justify_center().bg(gpui_kit::rgb(0x009500)).when(!can_accept, |d| d.bg(gpui_kit::rgb(crate::drawing_chrome::OK_DISABLED))).when(can_accept, |d| d.cursor_pointer().on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.close_gdt_dlg(true, cx)))).child(crate::ui::glyph("check", 18., gpui_kit::white())))
                        .child(div().id("gdt-cancel").w(px(28.)).h(px(27.)).flex().items_center().justify_center().cursor_pointer().child(crate::drawing_chrome::cancel_x(cx)).on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.close_gdt_dlg(false, cx)))),
                )
                .child(
                    v_flex()
                        .p(px(9.))
                        .child(text_box(0, &upper, "Upper text", cx))
                        .child(label_row)
                        .child(frame_rows)
                        .child(
                    h_flex()
                        .mt(px(11.))
                        .pl(px(1.))
                        .pb(px(10.))
                        .gap(px(6.))
                        .border_b_1()
                        .border_color(gpui_kit::rgb(0xdddddd))
                        // Up to 5 frames (help).
                        .child(Button::new("gdt-add").child(div().text_size(px(20.)).child("+")).ghost().small().w(px(50.)).h(px(30.)).text_size(px(20.)).border_1().border_color(gpui_kit::rgb(0xcccccc)).rounded(px(4.)).tooltip("Add frame").disabled(frames >= 5).on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                            let row = this.gdt_row(None, &[], "", "", window, cx);
                            if let Some(d) = &mut this.gdt_dlg {
                                d.rows.push(row);
                            }
                            cx.notify();
                        })))
                        .child(leader_btn)
                        .child(Button::new("gdt-symbols").child(div().text_size(px(16.)).child("Ø")).ghost().small().w(px(50.)).h(px(30.)).text_size(px(16.)).border_1().border_color(gpui_kit::rgb(0xcccccc)).rounded(px(4.)).tooltip("Show/hide symbols").when(show_symbols, |b| b.selected(true)).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            if let Some(d) = &mut this.gdt_dlg {
                                d.symbols = !d.symbols;
                            }
                            cx.notify();
                        }))),
                )
                        .children(palette.map(|p| p.mt(px(6.))))
                        .child(div().mt(px(6.)).child(text_box(1, &lower, "Lower text", cx))),
                )
                .into_any_element(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::split_tolerance;

    #[test]
    fn tolerance_slots() {
        assert_eq!(split_tolerance("Ø0.1Ⓜ"), ("Ø".into(), "0.1".into(), "Ⓜ".into()));
        assert_eq!(split_tolerance("0.05"), (String::new(), "0.05".into(), String::new()));
        assert_eq!(split_tolerance("SØ0.2ⓁⓅ"), ("SØ".into(), "0.2".into(), "ⓁⓅ".into()));
    }
}
