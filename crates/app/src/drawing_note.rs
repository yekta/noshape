//! Drawing editor: Onshape's note editor controls above the note text:
//! Font, Text height (becomes the new Notes default), Bold, Italic,
//! Underline, Strikethrough, Color; Line spacing (1.0–3.0) with space
//! before / after paragraphs; Horizontal alignment (left, center, right,
//! justified; disabled with leaders) and Vertical alignment (top, middle,
//! bottom); List (numbers, bullets, letters) with Note order and Flag notes.

use crate::drawing_drawprops::COLORS;
use crate::drawing_view::*;
use crate::os_ui::OsDropdownExt as _;
use crate::theme::p;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::Input;
use gpui_kit::component::{Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::drawing::note::{FlagShape, NoteAlign, NoteList, NoteStyle, NoteVAlign};
use noshape_model::drawing::sheet::{FONT_FAMILIES, TextFont};

type Set = std::rc::Rc<dyn Fn(&mut NoteStyle)>;

const SPACINGS: [f64; 5] = [1.0, 1.5, 2.0, 2.5, 3.0];

impl DrawingEditor {
    /// Change the open note editor's style.
    fn note_style(&mut self, f: &dyn Fn(&mut NoteStyle), cx: &mut Context<Self>) {
        if let Some(Panel::Note { style, .. }) = &mut self.panel {
            f(style);
        }
        cx.notify();
    }

    /// The note editor's rows (toolbar rows, text, list options).
    pub fn note_panel_rows(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let Some(Panel::Note { input, height, style, .. }) = &self.panel else { return Vec::new() };
        let (input, height, st) = (input.clone(), height.clone(), style.clone());
        let pal = p();
        let ed = cx.entity();
        let select = |id: &'static str, w: f32, label: String, items: Vec<(String, bool, Set)>| {
            let ed = ed.clone();
            Button::new(id).label(label).ghost().xsmall().w(px(w)).dropdown_caret(true).os_dropdown_menu(SharedString::from(format!("{id}-dd")), true, move |mut m, _, _| {
                for (l, on, f) in &items {
                    let (e, f) = (ed.clone(), f.clone());
                    m = m.item(crate::os_menu::OsMenuItem::new(l.clone()).checked(*on).on_click(move |_, _, cx| e.update(cx, |t, cx| t.note_style(&*f, cx))));
                }
                m
            })
        };
        let toggle = |id: &'static str, label: &'static str, tip: &'static str, on: bool, f: fn(&mut NoteStyle), enabled: bool, cx: &mut Context<Self>| {
            div()
                .id(id)
                .min_w(px(22.))
                .h(px(22.))
                .px(px(3.))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(12.))
                .rounded(px(2.))
                .when(on, |d| d.bg(gpui_kit::rgb(0xcfdbe5)))
                .when(!enabled, |d| d.opacity(0.35))
                .when(enabled, |d| d.cursor_pointer().hover(|d| d.bg(pal.row_hover)))
                .tooltip(crate::ui::tooltip(tip))
                .child(label)
                .when(enabled, |d| d.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.note_style(&f, cx))))
        };
        let font_family = st.font.as_ref().map(|f| f.family.clone()).unwrap_or_else(|| "Default".into());
        let mut fonts: Vec<(String, bool, Set)> = vec![("Default".into(), st.font.is_none(), std::rc::Rc::new(|s: &mut NoteStyle| s.font = None) as Set)];
        for fam in FONT_FAMILIES {
            fonts.push((fam.into(), st.font.as_ref().is_some_and(|f| f.family == fam), std::rc::Rc::new(move |s: &mut NoteStyle| {
                let (b, i) = s.font.as_ref().map(|f| (f.bold, f.italic)).unwrap_or((false, false));
                s.font = Some(TextFont { family: fam.into(), bold: b, italic: i });
            }) as Set));
        }
        let colors: Vec<(String, bool, Set)> = COLORS.iter().map(|&(n, c)| (n.to_string(), st.color == c, std::rc::Rc::new(move |s: &mut NoteStyle| s.color = c) as Set)).collect();
        let color_label = COLORS.iter().find(|c| c.1 == st.color).map(|c| c.0).unwrap_or("Custom").to_string();
        let (bold, italic) = st.font.as_ref().map(|f| (f.bold, f.italic)).unwrap_or((false, false));
        fn font_mut(s: &mut NoteStyle) -> &mut TextFont {
            s.font.get_or_insert_with(|| TextFont { family: FONT_FAMILIES[0].into(), bold: false, italic: false })
        }
        let spacing: Vec<(String, bool, Set)> = SPACINGS.iter().map(|&v| (format!("{v:.1}"), (st.line_spacing - v).abs() < 1e-9, std::rc::Rc::new(move |s: &mut NoteStyle| s.line_spacing = v) as Set)).collect();
        let lists: Vec<(String, bool, Set)> = NoteList::ALL.iter().map(|&l| (l.name().to_string(), st.list == l, std::rc::Rc::new(move |s: &mut NoteStyle| s.list = l) as Set)).collect();
        let order: Vec<(String, bool, Set)> = vec![
            ("Top to bottom".into(), !st.bottom_to_top, std::rc::Rc::new(|s: &mut NoteStyle| s.bottom_to_top = false) as Set),
            ("Bottom to top".into(), st.bottom_to_top, std::rc::Rc::new(|s: &mut NoteStyle| s.bottom_to_top = true) as Set),
        ];
        // Alignment is disabled for notes with leaders.
        let free = st.leaders.is_empty();
        let row1 = h_flex()
            .gap(px(3.))
            .child(select("note-font", 120., font_family, fonts))
            .child(div().w(px(70.)).child(Input::new(&height).text_size(px(13.33)).xsmall()))
            .child(toggle("note-b", "B", "Bold", bold, |s| {
                let f = font_mut(s);
                f.bold = !f.bold;
            }, true, cx))
            .child(toggle("note-i", "I", "Italic", italic, |s| {
                let f = font_mut(s);
                f.italic = !f.italic;
            }, true, cx))
            .child(toggle("note-u", "U", "Underline", st.underline, |s| s.underline = !s.underline, true, cx))
            .child(toggle("note-s", "S", "Strikethrough", st.strikethrough, |s| s.strikethrough = !s.strikethrough, true, cx))
            .child(select("note-color", 80., color_label, colors));
        let row2 = h_flex()
            .gap(px(3.))
            .child(div().text_size(px(11.)).text_color(pal.text_muted).child("Line spacing"))
            .child(select("note-spacing", 56., format!("{:.1}", st.line_spacing), spacing))
            .child(toggle("note-before", "¶↑", "Add space before paragraph", st.space_before, |s| s.space_before = !s.space_before, true, cx))
            .child(toggle("note-after", "¶↓", "Add space after paragraph", st.space_after, |s| s.space_after = !s.space_after, true, cx))
            .child(div().w(px(6.)))
            .child(toggle("note-al", "⯇", "Align left", st.align == NoteAlign::Left, |s| s.align = NoteAlign::Left, free, cx))
            .child(toggle("note-ac", "≡", "Align center", st.align == NoteAlign::Center, |s| s.align = NoteAlign::Center, free, cx))
            .child(toggle("note-ar", "⯈", "Align right", st.align == NoteAlign::Right, |s| s.align = NoteAlign::Right, free, cx))
            .child(toggle("note-aj", "☰", "Justified", st.align == NoteAlign::Justified, |s| s.align = NoteAlign::Justified, free, cx))
            .child(div().w(px(6.)))
            .child(toggle("note-vt", "⊤", "Top", st.valign == NoteVAlign::Top, |s| s.valign = NoteVAlign::Top, free, cx))
            .child(toggle("note-vm", "⊝", "Middle", st.valign == NoteVAlign::Middle, |s| s.valign = NoteVAlign::Middle, free, cx))
            .child(toggle("note-vb", "⊥", "Bottom", st.valign == NoteVAlign::Bottom, |s| s.valign = NoteVAlign::Bottom, free, cx));
        let row3 = h_flex()
            .gap(px(3.))
            .child(div().text_size(px(11.)).text_color(pal.text_muted).child("List"))
            .child(select("note-list", 130., st.list.name().into(), lists))
            .child(div().text_size(px(11.)).text_color(pal.text_muted).child("Note order"))
            .child(select("note-order", 110., if st.bottom_to_top { "Bottom to top".into() } else { "Top to bottom".into() }, order));
        // The text and its ruler are edited in place on the sheet
        // (`render_note_inplace`).
        let mut rows = vec![row1.into_any_element(), row2.into_any_element(), row3.into_any_element()];
        // Flag notes: per list item, with the flag type and size.
        if st.list != NoteList::None {
            let text = input.read(cx).value().to_string();
            let items: Vec<String> = text.split('\n').filter(|l| !l.trim().is_empty()).map(|l| l.to_string()).collect();
            let shapes: Vec<(String, bool, Set)> = FlagShape::ALL.iter().map(|&k| (k.name().to_string(), st.flag_shape == k, std::rc::Rc::new(move |s: &mut NoteStyle| s.flag_shape = k) as Set)).collect();
            // The first flag takes Drawing properties › Flag notes › Border shape.
            let default_shape = self.drawing.props.flag_shape;
            let sizes: Vec<(String, bool, Set)> = (1..=4u8).map(|n| (format!("{n} Character{}", if n > 1 { "s" } else { "" }), st.flag_chars == n, std::rc::Rc::new(move |s: &mut NoteStyle| s.flag_chars = n) as Set)).collect();
            rows.push(
                h_flex()
                    .gap(px(3.))
                    .child(div().text_size(px(11.)).text_color(pal.text_muted).child("Flag notes"))
                    .child(select("note-flag-shape", 90., st.flag_shape.name().into(), shapes))
                    .child(select("note-flag-size", 110., format!("{} Character{}", st.flag_chars, if st.flag_chars > 1 { "s" } else { "" }), sizes))
                    .into_any_element(),
            );
            let mut list = v_flex().gap(px(1.));
            for (i, item) in items.iter().enumerate() {
                let on = st.flags.contains(&i);
                list = list.child(Checkbox::new(("note-flag", i)).label(format!("{} {}", st.list.designator(i), item)).small().checked(on).on_click(cx.listener(move |this, v: &bool, _, cx| {
                    let v = *v;
                    this.note_style(&move |s: &mut NoteStyle| {
                        s.flags.retain(|x| *x != i);
                        if v {
                            if s.flags.is_empty() {
                                s.flag_shape = default_shape;
                            }
                            s.flags.push(i);
                        }
                    }, cx)
                })));
            }
            rows.push(list.into_any_element());
        }
        // Insert property, with its Text format / Date time format.
        {
            use noshape_model::drawing::format::{DATE_FORMATS, FIELDS, TEXT_FORMATS};
            let (tf, df) = self.note_field_fmt;
            let fmt_select = |id: &'static str, w: f32, label: String, items: Vec<(String, bool, (usize, usize))>| {
                let ed = ed.clone();
                Button::new(id).label(label).ghost().xsmall().w(px(w)).dropdown_caret(true).os_dropdown_menu(SharedString::from(format!("{id}-dd")), true, move |mut m, _, _| {
                    for (l, on, v) in &items {
                        let (e, v) = (ed.clone(), *v);
                        m = m.item(crate::os_menu::OsMenuItem::new(l.clone()).checked(*on).on_click(move |_, _, cx| {
                            e.update(cx, |t, cx| {
                                t.note_field_fmt = v;
                                cx.notify();
                            })
                        }));
                    }
                    m
                })
            };
            let tf_items = TEXT_FORMATS.iter().enumerate().map(|(i, (_, n))| (n.to_string(), i == tf, (i, df))).collect();
            let df_items = DATE_FORMATS.iter().enumerate().map(|(i, n)| (n.to_string(), i == df, (tf, i))).collect();
            let inp = input.clone();
            let props = Button::new("note-prop").label("Insert property").ghost().xsmall().w(px(120.)).dropdown_caret(true).os_dropdown_menu("note-prop-dd", true, move |mut m, _, _| {
                for (key, name) in FIELDS {
                    let inp = inp.clone();
                    let token = if *key == "date" { format!("{{date|{}}}", DATE_FORMATS[df]) } else if TEXT_FORMATS[tf].0.is_empty() { format!("{{{key}}}") } else { format!("{{{key}|{}}}", TEXT_FORMATS[tf].0) };
                    m = m.item(crate::os_menu::OsMenuItem::new(*name).on_click(move |_, w, cx| inp.update(cx, |s, cx| s.insert(token.clone(), w, cx))));
                }
                m
            });
            rows.push(
                h_flex()
                    .gap(px(3.))
                    .child(props)
                    .child(div().text_size(px(11.)).text_color(pal.text_muted).child("Text format"))
                    .child(fmt_select("note-tf", 110., TEXT_FORMATS[tf].1.into(), tf_items))
                    .child(div().text_size(px(11.)).text_color(pal.text_muted).child("Date"))
                    .child(fmt_select("note-df", 120., DATE_FORMATS[df].into(), df_items))
                    .into_any_element(),
            );
        }
        rows.push(div().text_xs().text_color(pal.text_faint).child("Fractions: type 1/2, 1#2 or 1^2 followed by a space").into_any_element());
        rows
    }
}
