//! Drawing editor: Onshape's floating Note panel (live drawing/225,
//! dom/drawing-note-edit-dialog.txt, 394 px wide): Font and Text height
//! (becomes the new Notes default); Bold, Italic, Underline, Strikethrough,
//! Color, then the Line spacing (1.0–3.0, space before / after paragraphs),
//! Horizontal alignment (disabled with leaders), List, Vertical alignment
//! (disabled with leaders) and Note order popups; Insert symbol, Insert
//! drawing property, Insert sheet reference property, Text format and Date
//! time format (enabled with the caret in a property); Flag notes with the
//! border shape and size.

use crate::drawing_drawprops::COLORS;
use crate::drawing_view::*;
use crate::os_ui::OsDropdownExt as _;
use crate::theme::p;
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::Input;
use gpui_kit::component::{Disableable as _, Sizable as _, h_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::drawing::format::{DATE_FORMATS, FIELDS, TEXT_FORMATS};
use noshape_model::drawing::note::{FlagShape, NoteAlign, NoteList, NoteStyle, NoteVAlign};
use noshape_model::drawing::sheet::{FONT_FAMILIES, TextFont};

type Set = std::rc::Rc<dyn Fn(&mut NoteStyle)>;

/// A popup entry: label, icon, checked, action (None: a separator).
type Entry = Option<(String, Option<&'static str>, bool, Set)>;

const SPACINGS: [f64; 5] = [1.0, 1.5, 2.0, 2.5, 3.0];

/// Note symbols (Onshape's Symbols dropdown, in its order).
const SYMBOLS: [(&str, &str); 14] = [
    ("°", "Degree"),
    ("⌀", "Diameter"),
    ("℄", "Centerline"),
    ("⌵", "Countersink"),
    ("↧", "Depth"),
    ("⌴", "Counterbore"),
    ("□", "Square"),
    ("⌒", "Arc length"),
    ("±", "Plus/Minus"),
    ("Ⓤ", "Unequally disposed profile"),
    ("Ⓘ", "Independency"),
    ("Ⓒ", "Continuous feature"),
    ("⌳", "Spot face"),
    ("▷", "Translation"),
];

/// The `{key|format}` property token around byte offset `at`: its range and key.
fn field_at(text: &str, at: usize) -> Option<(std::ops::Range<usize>, String)> {
    let at = at.min(text.len());
    let open = text[..at].rfind('{')?;
    if text[open..at].contains('}') {
        return None;
    }
    let close = at + text[at..].find('}')?;
    if text[at..close].contains('{') {
        return None;
    }
    let inner = &text[open + 1..close];
    let key = inner.split('|').next().unwrap_or("").to_string();
    FIELDS.iter().any(|(k, _)| *k == key).then_some((open..close + 1, key))
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
        if let Some(Panel::Note { style, .. }) = &mut self.panel {
            f(style);
        }
        cx.notify();
    }

    /// The Note panel's rows (live 225).
    pub fn note_panel_rows(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let Some(Panel::Note { input, height, style, .. }) = &self.panel else { return Vec::new() };
        let (input, height, st) = (input.clone(), height.clone(), style.clone());
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
                .when(on, |d| d.bg(gpui_kit::rgb(0xdbe1e6)))
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
                .child(crate::ui::glyph("ui-caret-down", 9.6, gpui_kit::rgb(0x000000)))
        };
        let with_menu = |b: Stateful<Div>, id: &'static str, enabled: bool, items: Vec<Entry>| -> AnyElement {
            if enabled { b.os_dropdown_menu(SharedString::from(format!("{id}-dd")), true, menu(items)).into_any_element() } else { b.into_any_element() }
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
        let field = |w: f32| div().w(px(w)).h(px(24.)).bg(gpui_kit::white()).border_1().border_color(gpui_kit::rgb(0xd3d7d9)).rounded(px(2.));
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
            .child(crate::ui::glyph("ui-caret-down", 9.6, gpui_kit::rgb(0x555555)));
        let row1 = h_flex()
            .h(px(30.))
            .ml(px(-1.))
            .gap(px(1.))
            .child(with_menu(font_box, "note-font", true, fonts))
            .child(field(75.).flex().items_center().pr(px(4.)).child(div().flex_1().child(Input::new(&height).appearance(false).text_size(px(13.33)).text_color(gpui_kit::rgb(0x555555)).xsmall())).child(crate::ui::glyph("ui-caret-down", 9.6, gpui_kit::rgb(0x555555))));

        // Row 2: B I U S, Color, Line spacing, Horizontal alignment, List,
        // Vertical alignment, Note order.
        let toggle = |id: &'static str, icon: &'static str, tip: &'static str, on: bool, f: fn(&mut NoteStyle), cx: &mut Context<Self>| button(id, icon, tip, on, cx).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.note_style(&f, cx)));
        let colors: Vec<Entry> = COLORS.iter().map(|&(n, c)| entry(n, None, st.color == c, std::rc::Rc::new(move |s: &mut NoteStyle| s.color = c))).collect();
        let swatch = match st.color {
            Some([r, g, b]) => gpui_kit::rgb(((r as u32) << 16) | ((g as u32) << 8) | b as u32),
            None => gpui_kit::rgb(0x000000),
        };
        let color = div().id("note-color").size(px(23.)).mx(px(1.)).bg(swatch).cursor_pointer().tooltip(crate::ui::tooltip("Color"));
        let mut spacing: Vec<Entry> = SPACINGS.iter().map(|&v| entry(&format!("{v:.1}"), None, (st.line_spacing - v).abs() < 1e-9, std::rc::Rc::new(move |s: &mut NoteStyle| s.line_spacing = v))).collect();
        spacing.push(None);
        spacing.push(entry(if st.space_before { "Remove space before paragraph" } else { "Add space before paragraph" }, None, false, std::rc::Rc::new(|s: &mut NoteStyle| s.space_before = !s.space_before)));
        spacing.push(entry(if st.space_after { "Remove space after paragraph" } else { "Add space after paragraph" }, None, false, std::rc::Rc::new(|s: &mut NoteStyle| s.space_after = !s.space_after)));
        let aligns = [(NoteAlign::Left, "Align left", "note-align-left"), (NoteAlign::Center, "Align center", "note-align-center"), (NoteAlign::Right, "Align right", "note-align-right"), (NoteAlign::Justified, "Justify", "note-align-justify")];
        let align_icon = aligns.iter().find(|a| a.0 == st.align).map(|a| a.2).unwrap_or("note-align-left");
        let align: Vec<Entry> = aligns.iter().map(|&(a, l, i)| entry(l, Some(i), st.align == a, std::rc::Rc::new(move |s: &mut NoteStyle| s.align = a))).collect();
        let list_icon = |l: NoteList| match l {
            NoteList::None => "note-list-none",
            NoteList::Numbers => "note-list-numbers",
            NoteList::Bullets => "note-list-bullets",
            NoteList::UpperLetters | NoteList::LowerLetters => "note-list-letters",
        };
        let lists: Vec<Entry> = NoteList::ALL.iter().map(|&l| entry(l.name(), Some(list_icon(l)), st.list == l, std::rc::Rc::new(move |s: &mut NoteStyle| s.list = l))).collect();
        let valigns = [(NoteVAlign::Top, "Top", "note-valign-top"), (NoteVAlign::Middle, "Middle", "note-valign-middle"), (NoteVAlign::Bottom, "Bottom", "note-valign-bottom")];
        let valign_icon = if free { valigns.iter().find(|v| v.0 == st.valign).map(|v| v.2).unwrap_or("note-valign-top") } else { "note-valign-middle" };
        let valign: Vec<Entry> = valigns.iter().map(|&(v, l, i)| entry(l, Some(i), st.valign == v, std::rc::Rc::new(move |s: &mut NoteStyle| s.valign = v))).collect();
        let order: Vec<Entry> = vec![
            entry("Top to bottom", Some("note-order-down"), !st.bottom_to_top, std::rc::Rc::new(|s: &mut NoteStyle| s.bottom_to_top = false)),
            entry("Bottom to top", Some("note-order-up"), st.bottom_to_top, std::rc::Rc::new(|s: &mut NoteStyle| s.bottom_to_top = true)),
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
            .child(color.os_dropdown_menu("note-color-dd", true, menu(colors)))
            .child(with_menu(popup("note-spacing", "note-line-spacing", "Line spacing", true, cx), "note-spacing", true, spacing))
            .child(with_menu(popup("note-align", align_icon, "Horizontal alignment", free, cx), "note-align", free, align))
            .child(with_menu(popup("note-list", list_icon(st.list), "List", true, cx), "note-list", true, lists))
            .child(with_menu(popup("note-valign", valign_icon, "Vertical alignment", free, cx), "note-valign", free, valign))
            .child(with_menu(popup("note-order", if st.bottom_to_top { "note-order-up" } else { "note-order-down" }, "Note order", true, cx), "note-order", true, order));

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
        let symbols: Vec<(String, String)> = SYMBOLS.iter().map(|(s, n)| (format!("{s}  {n}"), s.to_string())).collect();
        let token = |key: &str| if key == "date" || key.ends_with(".date") { format!("{{{key}|{}}}", DATE_FORMATS[0]) } else { format!("{{{key}}}") };
        let sheet_ref = |k: &str| k.starts_with("part.") || k.starts_with("model.");
        let drawing_props: Vec<(String, String)> = FIELDS.iter().filter(|(k, _)| !sheet_ref(k)).map(|(k, n)| (n.to_string(), token(k))).collect();
        let sheet_props: Vec<(String, String)> = FIELDS.iter().filter(|(k, _)| sheet_ref(k)).map(|(k, n)| (n.to_string(), token(k))).collect();
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
            .child(crate::ui::glyph("ui-caret-down", 9.6, gpui_kit::rgb(0x000000)));
        // Text format / Date time format: for the property at the caret.
        let at = field_at(&text, cursor);
        let is_date = at.as_ref().is_some_and(|(_, k)| k == "date" || k.ends_with(".date"));
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
            .child(symbol_btn.os_dropdown_menu("note-symbol-dd", true, insert_menu(symbols)))
            .child(popup("note-dprop", "note-drawing-prop", "Insert drawing property", true, cx).os_dropdown_menu("note-dprop-dd", true, insert_menu(drawing_props)))
            .child(popup("note-sprop", "note-sheet-prop", "Insert sheet reference property", true, cx).os_dropdown_menu("note-sprop-dd", true, insert_menu(sheet_props)))
            .child(if tf_on { tf_btn.os_dropdown_menu("note-tf-dd", true, reformat(text_formats)).into_any_element() } else { tf_btn.into_any_element() })
            .child(if is_date { df_btn.os_dropdown_menu("note-df-dd", true, reformat(date_formats)).into_any_element() } else { df_btn.into_any_element() });

        // Row 4: Flag notes (list items only) with the border shape and size.
        let item = if st.list != NoteList::None { item_at(&text, cursor) } else { None };
        let flagged = item.is_some_and(|i| st.flags.contains(&i));
        let default_shape = self.drawing.props.flag_shape;
        let flag_on = flagged;
        let shape_label = if st.flag_shape == default_shape { format!("{} (Drawing)", st.flag_shape.name()) } else { st.flag_shape.name().to_string() };
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
                .bg(gpui_kit::white())
                .border_b_1()
                .border_color(gpui_kit::rgb(0xdddddd))
                .text_size(px(13.33))
                .text_color(gpui_kit::rgb(0x555555))
                .opacity(if flag_on { 1.0 } else { 0.5 })
                .when(flag_on, |d| d.cursor_pointer())
                .child(div().flex_1().whitespace_nowrap().child(label))
                .child(crate::ui::glyph("ui-caret-down", 9.6, gpui_kit::rgb(0x555555)))
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
            .child(with_menu(shape_sel, "note-flag-shape", flag_on, shapes))
            .child(div().w(px(15.)))
            .child(with_menu(size_sel, "note-flag-size", flag_on, sizes));

        // The text and its ruler are edited in place on the sheet
        // (`render_note_inplace`).
        vec![row1.into_any_element(), row2.into_any_element(), row3.into_any_element(), row4.into_any_element()]
    }
}
