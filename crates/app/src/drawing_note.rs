//! Drawing editor: Onshape's floating Note panel (live drawing/225,
//! dom/drawing-note-edit-dialog.txt, 394 px wide): Font and Text height
//! (becomes the new Notes default); Bold, Italic, Underline, Strikethrough,
//! Color, then the Line spacing (1.0–3.0, space before / after paragraphs),
//! Horizontal alignment (disabled with leaders), List, Vertical alignment
//! (disabled with leaders) and Note order popups; Insert symbol, Insert
//! drawing property, Insert sheet reference property, Text format and Date
//! time format (enabled with the caret in a property); Flag notes with the
//! border shape and size.

use crate::drawing_view::*;
use crate::os_ui::OsDropdownExt as _;
use crate::theme::p;
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::Input;
use gpui_kit::component::{Disableable as _, Sizable as _, h_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::drawing::format::{DATE_FORMATS, DRAWING_FIELDS, SHEET_REF_FIELDS, TEXT_FORMATS, is_date_field, is_field};
use noshape_model::drawing::note::{FlagShape, NoteAlign, NoteList, NoteStyle, NoteVAlign};
use noshape_model::drawing::sheet::{FONT_FAMILIES, TextFont};

type Set = std::rc::Rc<dyn Fn(&mut NoteStyle)>;

/// A popup entry: label, icon, checked, action (None: a separator).
type Entry = Option<(String, Option<&'static str>, bool, Set)>;

const SPACINGS: [f64; 5] = [1.0, 1.5, 2.0, 2.5, 3.0];

/// Note symbols (live: Insert symbol, one column, in this order).
const SYMBOLS: [&str; 14] = ["°", "Ø", "℄", "⌵", "↧", "⌴", "□", "⌒", "±", "Ⓤ", "Ⓘ", "✟", "✝", "▷"];

/// The `{key|format}` property token ending right at byte offset `at` (live:
/// Text format / Date time format apply just after an inserted property).
fn field_at(text: &str, at: usize) -> Option<(std::ops::Range<usize>, String)> {
    let at = at.min(text.len());
    let before = text[..at].strip_suffix('}')?;
    let open = before.rfind('{')?;
    let inner = &before[open + 1..];
    let key = inner.split('|').next().unwrap_or("").to_string();
    is_field(&key).then_some((open..at, key))
}

/// The Note panel's Text height: four decimals in the drawing's units, no
/// unit (live: "3.5000" in a mm drawing, "0.1200" in an inch one).
pub fn note_height_text(mm: f64, units_mm: bool) -> String {
    format!("{:.4}", if units_mm { mm } else { mm / 25.4 })
}

/// A typed Text height (drawing units, or with an explicit mm / in) in mm.
pub fn parse_note_height(s: &str, units_mm: bool) -> Option<f64> {
    let t = s.trim();
    let (num, mm_per) = if let Some(n) = t.strip_suffix("mm") {
        (n, 1.0)
    } else if let Some(n) = t.strip_suffix("in").or_else(|| t.strip_suffix('"')) {
        (n, 25.4)
    } else {
        (t, if units_mm { 1.0 } else { 25.4 })
    };
    num.trim().parse::<f64>().ok().map(|v| v * mm_per).filter(|v| *v > 0.1)
}

/// Index of the list item (non-empty paragraph) holding byte offset `at`.
fn item_at(text: &str, at: usize) -> Option<usize> {
    let para = text[..at.min(text.len())].matches('\n').count();
    let lines: Vec<&str> = text.split('\n').collect();
    lines.get(para).filter(|l| !l.trim().is_empty())?;
    Some(lines[..para].iter().filter(|l| !l.trim().is_empty()).count())
}

impl DrawingEditor {
    /// Change the open note editor's style.
    fn note_style(&mut self, f: &dyn Fn(&mut NoteStyle), cx: &mut Context<Self>) {
        match &mut self.panel {
            Some(Panel::Note { style, .. }) => f(style),
            Some(Panel::Cell { note: Some(n), .. }) => f(&mut n.style),
            _ => {}
        }
        cx.notify();
    }

    /// The Note panel's rows (live 225; for a general table cell live 743–750:
    /// no Note order and no Flag notes, Line spacing and List disabled).
    pub fn note_panel_rows(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let (input, height, st, cell) = match &self.panel {
            Some(Panel::Note { input, height, style, .. }) => (input.clone(), height.clone(), style.clone(), false),
            Some(Panel::Cell { input, note: Some(n), .. }) => (input.clone(), n.height.clone(), n.style.clone(), true),
            _ => return Vec::new(),
        };
        let pal = p();
        let ed = cx.entity();
        let text = input.read(cx).value().to_string();
        let cursor = input.read(cx).cursor();
        let menu = |items: Vec<Entry>| {
            let ed = ed.clone();
            move |mut m: crate::os_menu::OsMenu, _: &mut Window, _: &mut Context<crate::os_menu::OsMenu>| {
                for it in &items {
                    let Some((l, icon, on, f)) = it else {
                        m = m.separator();
                        continue;
                    };
                    let (e, f) = (ed.clone(), f.clone());
                    let mut item = crate::os_menu::OsMenuItem::new(l.clone()).checked(*on).on_click(move |_, _, cx| e.update(cx, |t, cx| t.note_style(&*f, cx)));
                    if let Some(i) = icon {
                        item = item.icon_name(i);
                    }
                    m = m.item(item);
                }
                m
            }
        };
        // 30 × 30 toolbar buttons (28 px image, 4 px padding).
        let button = |id: &'static str, icon: &'static str, tip: &'static str, on: bool, cx: &App| {
            div()
                .id(id)
                .size(px(30.))
                .p(px(1.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(2.))
                .opacity(0.95)
                .cursor_pointer()
                // Live 1140: pressed = #cfdbe5 with a 1 px #9aa5af border.
                .border_1()
                .border_color(if on { Hsla::from(gpui_kit::rgb(0x9aa5af)) } else { gpui_kit::transparent_black() })
                .when(on, |d| d.bg(gpui_kit::rgb(0xcfdbe5)))
                .when(!on, |d| d.hover(|d| d.bg(pal.row_hover)))
                .tooltip(crate::ui::tooltip(tip))
                .child(crate::assets::icon(icon, 20., cx))
        };
        // 45 × 30 popup buttons: the icon and a 8 × 4 caret.
        let popup = |id: &'static str, icon: &'static str, tip: &'static str, enabled: bool, cx: &App| {
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
        // Popup buttons open a menu under them; the Font / Flag selects open a
        // list as wide as the box (`select`).
        let with_menu = |b: Stateful<Div>, id: &'static str, enabled: bool, select: bool, items: Vec<Entry>| -> AnyElement {
            if enabled { b.os_dropdown_menu(SharedString::from(format!("{id}-dd")), select, menu(items)).into_any_element() } else { b.into_any_element() }
        };
        let entry = |l: &str, icon: Option<&'static str>, on: bool, f: Set| -> Entry { Some((l.to_string(), icon, on, f)) };
        let free = st.leaders.is_empty();
        let (bold, italic) = st.font.as_ref().map(|f| (f.bold, f.italic)).unwrap_or((false, false));
        fn font_mut(s: &mut NoteStyle) -> &mut TextFont {
            s.font.get_or_insert_with(|| TextFont { family: FONT_FAMILIES[0].into(), bold: false, italic: false })
        }

        // Row 1: Font (238 × 24) and Text height (75 × 24).
        let font_label = match &st.font {
            Some(f) if f.family != FONT_FAMILIES[0] => f.family.clone(),
            _ => "Noto Sans ( Note )".to_string(),
        };
        let mut fonts: Vec<Entry> = vec![entry("Noto Sans ( Note )", None, st.font.as_ref().is_none_or(|f| f.family == FONT_FAMILIES[0]), std::rc::Rc::new(|s: &mut NoteStyle| match &mut s.font {
            Some(f) if f.bold || f.italic => f.family = FONT_FAMILIES[0].into(),
            _ => s.font = None,
        }))];
        for fam in FONT_FAMILIES.iter().skip(1).copied() {
            fonts.push(entry(fam, None, st.font.as_ref().is_some_and(|f| f.family == fam), std::rc::Rc::new(move |s: &mut NoteStyle| {
                let (b, i) = s.font.as_ref().map(|f| (f.bold, f.italic)).unwrap_or((false, false));
                s.font = Some(TextFont { family: fam.into(), bold: b, italic: i });
            })));
        }
        let field = |w: f32| div().w(px(w)).h(px(24.)).bg(crate::theme::light().panel).border_1().border_color(gpui_kit::rgb(0xd3d7d9)).rounded(px(2.));
        let font_box = field(238.)
            .id("note-font")
            .pl(px(6.))
            .pr(px(6.))
            .flex()
            .items_center()
            .cursor_pointer()
            .text_size(px(13.33))
            .text_color(gpui_kit::rgb(0x555555))
            .tooltip(crate::ui::tooltip("Font"))
            .child(div().flex_1().overflow_hidden().whitespace_nowrap().child(font_label))
            .child(crate::ui::glyph("ui-select-caret", 8., gpui_kit::rgb(0x777777)));
        let row1 = h_flex()
            .h(px(30.))
            .ml(px(-1.))
            .gap(px(1.))
            .child(with_menu(font_box, "note-font", true, true, fonts))
            .child(field(75.).flex().items_center().pr(px(6.)).child(div().flex_1().child(Input::new(&height).appearance(false).text_size(px(13.33)).text_color(gpui_kit::rgb(0x555555)).xsmall())).child(crate::ui::glyph("ui-select-caret", 8., gpui_kit::rgb(0x777777))));

        // Row 2: B I U S, Color, Line spacing, Horizontal alignment, List,
        // Vertical alignment, Note order.
        let toggle = |id: &'static str, icon: &'static str, tip: &'static str, on: bool, f: fn(&mut NoteStyle), cx: &mut Context<Self>| button(id, icon, tip, on, cx).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.note_style(&f, cx)));
        let swatch = match st.color {
            Some([r, g, b]) => gpui_kit::rgb(((r as u32) << 16) | ((g as u32) << 8) | b as u32),
            None => gpui_kit::rgb(0x000000),
        };
        let color = div().id("note-color").size(px(23.)).mx(px(1.)).bg(swatch).cursor_pointer().tooltip(crate::ui::tooltip("Color"));
        let mut spacing: Vec<Entry> = SPACINGS.iter().map(|&v| entry(&format!("{v:.1}"), None, (st.line_spacing - v).abs() < 1e-9, std::rc::Rc::new(move |s: &mut NoteStyle| s.line_spacing = v))).collect();
        spacing.push(None);
        spacing.push(entry(if st.space_before { "Remove space before paragraph" } else { "Add space before paragraph" }, None, false, std::rc::Rc::new(|s: &mut NoteStyle| s.space_before = !s.space_before)));
        spacing.push(entry(if st.space_after { "Remove space after paragraph" } else { "Add space after paragraph" }, None, false, std::rc::Rc::new(|s: &mut NoteStyle| s.space_after = !s.space_after)));
        let aligns = [(NoteAlign::Left, "Left", "note-align-left"), (NoteAlign::Center, "Center", "note-align-center"), (NoteAlign::Right, "Right", "note-align-right"), (NoteAlign::Justified, "Justify", "note-align-justify")];
        let aligns = &aligns[..if cell { 3 } else { 4 }];
        let align_icon = aligns.iter().find(|a| a.0 == st.align).map(|a| a.2).unwrap_or("note-align-left");
        let align: Vec<Entry> = aligns.iter().map(|&(a, l, _)| entry(l, None, st.align == a, std::rc::Rc::new(move |s: &mut NoteStyle| s.align = a))).collect();
        // Live (ui/icons2-note-panel, -list-numbers-*): the List button keeps the
        // bulleted list icon whichever list is picked.
        let list_icon = |_: NoteList| "note-list-bullets";
        // Live: Numbers, Bullets, Uppercase / Lowercase letters (no "None");
        // picking the current one again ends the list. The alignment, list and
        // order menus are text only, with a check on the current option
        // (ui/icons2-note-menu-*).
        let lists: Vec<Entry> = NoteList::ALL.iter().skip(1).map(|&l| entry(l.name(), None, st.list == l, std::rc::Rc::new(move |s: &mut NoteStyle| s.list = if s.list == l { NoteList::None } else { l }))).collect();
        let valigns = [(NoteVAlign::Top, "Top", "note-valign-top"), (NoteVAlign::Middle, "Middle", "note-valign-middle"), (NoteVAlign::Bottom, "Bottom", "note-valign-bottom")];
        let valign_icon = if free { valigns.iter().find(|v| v.0 == st.valign).map(|v| v.2).unwrap_or("note-valign-top") } else { "note-valign-middle" };
        let valign: Vec<Entry> = valigns.iter().map(|&(v, l, _)| entry(l, None, st.valign == v, std::rc::Rc::new(move |s: &mut NoteStyle| s.valign = v))).collect();
        let order: Vec<Entry> = vec![
            entry("Bottom to top", None, st.bottom_to_top, std::rc::Rc::new(|s: &mut NoteStyle| s.bottom_to_top = true)),
            entry("Top to bottom", None, !st.bottom_to_top, std::rc::Rc::new(|s: &mut NoteStyle| s.bottom_to_top = false)),
        ];
        let row2 = h_flex()
            .h(px(30.))
            .pl(px(1.))
            .gap(px(2.))
            .child(toggle("note-b", "note-bold", "Bold", bold, |s| {
                let f = font_mut(s);
                f.bold = !f.bold;
            }, cx))
            .child(toggle("note-i", "note-italic", "Italic", italic, |s| {
                let f = font_mut(s);
                f.italic = !f.italic;
            }, cx))
            .child(toggle("note-u", "note-underline", "Underline", st.underline, |s| s.underline = !s.underline, cx))
            .child(toggle("note-s", "note-strike", "Strikethrough", st.strikethrough, |s| s.strikethrough = !s.strikethrough, cx))
            // Live 1137: opens the "Note color" panel under it.
            .child(color.on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.open_note_color(window, cx))))
            .child(with_menu(popup("note-spacing", "note-line-spacing", "Line spacing", !cell, cx), "note-spacing", !cell, false, spacing))
            .child(with_menu(popup("note-align", align_icon, "Horizontal alignment", free, cx), "note-align", free, false, align))
            .child(with_menu(popup("note-list", list_icon(st.list), "List", !cell, cx), "note-list", !cell, false, lists))
            .child(with_menu(popup("note-valign", valign_icon, "Vertical alignment", free, cx), "note-valign", free, false, valign))
            .when(!cell, |r| r.child(with_menu(popup("note-order", if st.bottom_to_top { "note-order-up" } else { "note-order-down" }, "Note order", true, cx), "note-order", true, false, order)));

        // Row 3: Insert symbol, Insert drawing property, Insert sheet
        // reference property, Text format, Date time format.
        let insert_menu = |items: Vec<(String, String)>| {
            let inp = input.clone();
            move |mut m: crate::os_menu::OsMenu, _: &mut Window, _: &mut Context<crate::os_menu::OsMenu>| {
                for (label, token) in &items {
                    let (inp, token) = (inp.clone(), token.clone());
                    m = m.item(crate::os_menu::OsMenuItem::new(label.clone()).on_click(move |_, w, cx| inp.update(cx, |s, cx| s.insert(token.clone(), w, cx))));
                }
                m
            }
        };
        let symbols: Vec<(String, String)> = SYMBOLS.iter().map(|s| (s.to_string(), s.to_string())).collect();
        let token = |key: &str| format!("{{{key}}}");
        let drawing_props: Vec<(String, String)> = DRAWING_FIELDS.iter().map(|(k, n)| (n.to_string(), token(k))).collect();
        let sheet_props: Vec<(String, String)> = SHEET_REF_FIELDS.iter().map(|(k, n)| (n.to_string(), token(k))).collect();
        let symbol_btn = div()
            .id("note-symbol")
            .w(px(45.))
            .h(px(30.))
            .p(px(1.))
            .flex()
            .items_center()
            .rounded(px(2.))
            .opacity(0.95)
            .cursor_pointer()
            .hover(|d| d.bg(pal.row_hover))
            .tooltip(crate::ui::tooltip("Insert symbol"))
            .child(div().size(px(28.)).flex().items_center().justify_center().text_size(px(17.)).text_color(gpui_kit::rgb(0x333333)).child("Ø"))
            .child(crate::ui::glyph("drawing-tool-caret", 12., gpui_kit::rgb(0x000000)));
        // Text format / Date time format: for the property at the caret.
        let at = field_at(&text, cursor);
        let is_date = at.as_ref().is_some_and(|(_, k)| is_date_field(k));
        let reformat = |fmts: Vec<(String, String)>| {
            let (inp, range, key) = (input.clone(), at.as_ref().map(|a| a.0.clone()), at.as_ref().map(|a| a.1.clone()).unwrap_or_default());
            move |mut m: crate::os_menu::OsMenu, _: &mut Window, _: &mut Context<crate::os_menu::OsMenu>| {
                for (label, fmt) in &fmts {
                    let (inp, range, tok) = (inp.clone(), range.clone(), if fmt.is_empty() { format!("{{{key}}}") } else { format!("{{{key}|{fmt}}}") });
                    m = m.item(crate::os_menu::OsMenuItem::new(label.clone()).on_click(move |_, w, cx| {
                        let Some(r) = range.clone() else { return };
                        inp.update(cx, |s, cx| {
                            let mut v = s.value().to_string();
                            if r.end <= v.len() {
                                v.replace_range(r.clone(), &tok);
                                s.set_value(v, w, cx);
                            }
                        })
                    }));
                }
                m
            }
        };
        let text_formats: Vec<(String, String)> = TEXT_FORMATS.iter().map(|(f, n)| (n.to_string(), f.to_string())).collect();
        let date_formats: Vec<(String, String)> = DATE_FORMATS.iter().map(|f| (f.to_string(), f.to_string())).collect();
        let tf_on = at.is_some() && !is_date;
        let tf_btn = popup("note-tf", "note-text-format", "Text format", tf_on, cx);
        let df_btn = popup("note-df", "note-date", "Date time format", is_date, cx);
        let row3 = h_flex()
            .h(px(30.))
            .pl(px(1.))
            .gap(px(2.))
            .child(symbol_btn.os_dropdown_menu("note-symbol-dd", false, insert_menu(symbols)))
            .child(popup("note-dprop", "note-drawing-prop", "Insert drawing property", true, cx).os_dropdown_menu("note-dprop-dd", false, insert_menu(drawing_props)))
            .child(popup("note-sprop", "note-sheet-prop", "Insert sheet reference property", true, cx).os_dropdown_menu("note-sprop-dd", false, insert_menu(sheet_props)))
            .child(if tf_on { tf_btn.os_dropdown_menu("note-tf-dd", false, reformat(text_formats)).into_any_element() } else { tf_btn.into_any_element() })
            .child(if is_date { df_btn.os_dropdown_menu("note-df-dd", false, reformat(date_formats)).into_any_element() } else { df_btn.into_any_element() });

        // Row 4: Flag notes (list items only) with the border shape and size.
        let item = if st.list != NoteList::None { item_at(&text, cursor) } else { None };
        let flagged = item.is_some_and(|i| st.flags.contains(&i));
        let default_shape = self.drawing.props.flag_shape;
        let flag_on = flagged;
        // Unflagged notes show the drawing's shape ("Flag (Drawing)").
        let shown_shape = if st.flags.is_empty() { default_shape } else { st.flag_shape };
        let shape_label = if shown_shape == default_shape { format!("{} (Drawing)", shown_shape.name()) } else { shown_shape.name().to_string() };
        let shapes: Vec<Entry> = FlagShape::ALL.iter().map(|&k| entry(k.name(), None, st.flag_shape == k, std::rc::Rc::new(move |s: &mut NoteStyle| s.flag_shape = k))).collect();
        let sizes: Vec<Entry> = (1..=4u8).map(|n| entry(&format!("{n} Character{}", if n > 1 { "s" } else { "" }), None, st.flag_chars == n, std::rc::Rc::new(move |s: &mut NoteStyle| s.flag_chars = n))).collect();
        let flag_select = |id: &'static str, w: f32, label: String| {
            div()
                .id(id)
                .w(px(w))
                .h(px(26.))
                .pl(px(6.))
                .pr(px(4.))
                .flex()
                .items_center()
                .bg(crate::theme::light().panel)
                .border_b_1()
                .border_color(gpui_kit::rgb(0xdddddd))
                .text_size(px(13.33))
                .text_color(gpui_kit::rgb(0x555555))
                .opacity(if flag_on { 1.0 } else { 0.5 })
                .when(flag_on, |d| d.cursor_pointer())
                .child(div().flex_1().whitespace_nowrap().child(label))
                .child(crate::ui::glyph("ui-select-caret", 8., gpui_kit::rgb(0x777777)))
        };
        let flag_box = Checkbox::new("note-flag").label("Flag notes").small().checked(flagged).disabled(item.is_none()).on_click(cx.listener(move |this, v: &bool, _, cx| {
            let (v, Some(i)) = (*v, item) else { return };
            this.note_style(&move |s: &mut NoteStyle| {
                s.flags.retain(|x| *x != i);
                if v {
                    if s.flags.is_empty() {
                        s.flag_shape = default_shape;
                    }
                    s.flags.push(i);
                }
            }, cx)
        }));
        let shape_sel = flag_select("note-flag-shape", 166., shape_label);
        let size_sel = flag_select("note-flag-size", 105., format!("{} Character{}", st.flag_chars, if st.flag_chars > 1 { "s" } else { "" }));
        let row4 = h_flex()
            .h(px(28.))
            .child(div().w(px(85.)).pl(px(4.)).text_size(px(12.)).when(item.is_none(), |d| d.opacity(0.5)).child(flag_box))
            .child(div().w(px(14.)))
            .child(with_menu(shape_sel, "note-flag-shape", flag_on, true, shapes))
            .child(div().w(px(15.)))
            .child(with_menu(size_sel, "note-flag-size", flag_on, true, sizes));

        // The text and its ruler are edited in place on the sheet
        // (`render_note_inplace`).
        let mut rows = vec![row1.into_any_element(), row2.into_any_element(), row3.into_any_element()];
        if !cell {
            rows.push(row4.into_any_element());
        }
        rows
    }
}
