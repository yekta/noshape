//! Autofill dropdown for numeric fields (Onshape's quantity autocomplete,
//! live "skb (10-25) quantity autocomplete"): typing `#` lists the
//! variables in scope and "New variable…"; typing a word lists the matching
//! variables, functions (with their argument placeholders), units of the
//! field's kind and constants. ↑/↓ move, Enter / Tab / a click insert, Esc
//! closes. The `#` matching is [`noshape_units::complete`]; this is the
//! word matching, the shared UI state and the list.

use crate::theme::p;
use gpui_kit::component::input::InputState;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_units::Dim;
use noshape_units::complete::variable_completion;
use std::collections::BTreeMap;
use std::ops::Range;

/// Most entries shown at once.
const MAX_ROWS: usize = 8;

/// Live's functions and their argument placeholders, in its order.
const FUNCTIONS: &[(&str, &[&str])] = &[
    ("abs", &["x"]),
    ("acos", &["x"]),
    ("asin", &["x"]),
    ("atan", &["x"]),
    ("atan2", &["y", "x"]),
    ("ceil", &["x"]),
    ("ceil", &["x", "unit"]),
    ("clamp", &["x", "min", "max"]),
    ("cos", &["angle"]),
    ("exp", &["x"]),
    ("floor", &["x"]),
    ("floor", &["x", "unit"]),
    ("hypot", &["x", "y"]),
    ("isInteger", &["x"]),
    ("log", &["x"]),
    ("log10", &["x"]),
    ("max", &["x", "y"]),
    ("min", &["x", "y"]),
    ("round", &["x"]),
    ("round", &["x", "unit"]),
    ("roundToPrecision", &["x", "precision"]),
    ("sin", &["angle"]),
    ("sqrt", &["x"]),
    ("tan", &["angle"]),
    ("tolerantEquals", &["x", "y"]),
];

const CONSTANTS: &[&str] = &["PI", "true", "false"];

/// Units offered for a field of this kind (live: a length field lists
/// cm, ft, in, m, mm, yd; an angle field deg, rad).
fn units_for(dim: Option<Dim>) -> &'static [&'static str] {
    match dim {
        Some(d) if d == Dim::LENGTH => &["cm", "ft", "in", "m", "mm", "yd"],
        Some(d) if d == Dim::ANGLE => &["deg", "rad"],
        _ => &[],
    }
}

/// One row of the list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Entry {
    /// A variable, inserted as `#name`.
    Variable(String),
    /// A function: name and argument placeholders.
    Function(&'static str, &'static [&'static str]),
    /// A unit, inserted as is.
    Unit(&'static str),
    /// A constant (PI, true, false), inserted as is.
    Constant(&'static str),
}

impl Entry {
    fn name(&self) -> &str {
        match self {
            Entry::Variable(n) => n,
            Entry::Function(n, _) | Entry::Unit(n) | Entry::Constant(n) => n,
        }
    }

    /// The inserted text and the cursor offset in it (live: `clamp(, , )`
    /// with the cursor after the parenthesis).
    pub fn insertion(&self) -> (String, usize) {
        match self {
            Entry::Variable(n) => (format!("#{n}"), n.len() + 1),
            Entry::Function(n, args) => (format!("{n}({})", vec![""; args.len()].join(", ")), n.len() + 1),
            Entry::Unit(n) | Entry::Constant(n) => (n.to_string(), n.len()),
        }
    }
}

/// Where `needle` (lower case) matches `name`: 0 at its start, 1 at a
/// camelCase word ("p" → roundToPrecision), none elsewhere.
fn match_rank(name: &str, needle: &str) -> Option<u8> {
    let lower = name.to_ascii_lowercase();
    if lower.starts_with(needle) {
        return Some(0);
    }
    // A word starts at an upper-case letter after a lower-case one (not
    // inside "PI": live's `i` doesn't list it).
    let b = name.as_bytes();
    (1..b.len()).any(|i| b[i].is_ascii_uppercase() && b[i - 1].is_ascii_lowercase() && lower[i..].starts_with(needle)).then_some(1)
}

/// The word under the cursor (letters, digits, `_`, starting with a letter
/// or `_`, not after `#`) and the entries matching it, live's order:
/// matches at the start, then at a camelCase word, each alphabetical.
pub fn word_entries(text: &str, cursor: usize, variables: &[&str], dim: Option<Dim>) -> Option<(Range<usize>, Vec<Entry>)> {
    let mut cursor = cursor.min(text.len());
    while !text.is_char_boundary(cursor) {
        cursor -= 1;
    }
    let ident = |c: char| c.is_ascii_alphanumeric() || c == '_';
    let before = &text[..cursor];
    let mut start = before.char_indices().rev().take_while(|&(_, c)| ident(c)).last().map_or(cursor, |(i, _)| i);
    if before[..start].ends_with('#') {
        return None;
    }
    // "1in": the word starts after the number.
    start += before[start..].chars().take_while(char::is_ascii_digit).count();
    if start >= cursor {
        return None;
    }
    let end = cursor + text[cursor..].chars().take_while(|&c| ident(c)).map(char::len_utf8).sum::<usize>();
    let needle = before[start..].to_ascii_lowercase();
    let mut all: Vec<Entry> = variables.iter().map(|v| Entry::Variable(v.to_string())).collect();
    all.extend(FUNCTIONS.iter().map(|(n, a)| Entry::Function(n, a)));
    all.extend(units_for(dim).iter().map(|u| Entry::Unit(u)));
    all.extend(CONSTANTS.iter().map(|c| Entry::Constant(c)));
    let mut ranked: Vec<(u8, String, usize, Entry)> = all.into_iter().enumerate().filter_map(|(i, e)| Some((match_rank(e.name(), &needle)?, e.name().to_ascii_lowercase(), i, e))).collect();
    ranked.sort_by(|a, b| (a.0, &a.1, a.2).cmp(&(b.0, &b.1, b.2)));
    let entries: Vec<Entry> = ranked.into_iter().map(|r| r.3).collect();
    (!entries.is_empty()).then_some((start..end, entries))
}

/// An open list for one field.
pub struct VarCompletion {
    /// Which field (dialog parameter path).
    pub key: String,
    /// The text an entry replaces (`#` included for variables).
    pub range: Range<usize>,
    pub entries: Vec<Entry>,
    pub selected: usize,
    /// Typing `#…`: the list ends with "(x) New variable…" for this name
    /// (live part-studio/152; shown even when the name exists).
    pub new_variable: Option<String>,
}

impl VarCompletion {
    /// The list for `state`'s text and cursor, or `None`. `dim` is the
    /// field's kind (which units to offer).
    pub fn compute(key: &str, state: &InputState, variables: &BTreeMap<String, noshape_units::Quantity>, dim: Option<Dim>) -> Option<Self> {
        let text = state.value().to_string();
        let cursor = state.cursor();
        if let Some((range, name)) = fragment(&text, cursor) {
            let matches = variable_completion(&text, cursor, variables.keys().map(String::as_str)).map(|c| c.matches).unwrap_or_default();
            let new_variable = (name.is_empty() || noshape_model::variable_table::is_identifier(&name)).then_some(name);
            return Some(Self { key: key.to_string(), range, entries: matches.into_iter().map(Entry::Variable).collect(), selected: 0, new_variable });
        }
        let names: Vec<&str> = variables.keys().map(String::as_str).collect();
        let (range, entries) = word_entries(&text, cursor, &names, dim)?;
        Some(Self { key: key.to_string(), range, entries, selected: 0, new_variable: None })
    }

    /// Rows: the entries, then "New variable…".
    fn rows(&self) -> usize {
        self.entries.len().min(MAX_ROWS) + usize::from(self.new_variable.is_some())
    }

    /// Whether entry `index` (the selected one when `None`) is "New variable…".
    pub fn picks_new(&self, index: Option<usize>) -> bool {
        self.new_variable.is_some() && index.unwrap_or(self.selected) == self.entries.len().min(MAX_ROWS)
    }

    /// ↑ / ↓.
    pub fn move_selection(&mut self, delta: isize) {
        let n = self.rows() as isize;
        if n > 0 {
            self.selected = (self.selected as isize + delta).rem_euclid(n) as usize;
        }
    }

    /// Insert entry `index` (the selected one when `None`) into `state`.
    /// Returns the new text (the caller treats it as a user edit).
    pub fn accept(&self, index: Option<usize>, state: &Entity<InputState>, window: &mut Window, cx: &mut App) -> Option<String> {
        let entry = self.entries.get(index.unwrap_or(self.selected))?;
        let text = state.read(cx).value().to_string();
        let range = self.range.start.min(text.len())..self.range.end.min(text.len());
        let (insert, at) = entry.insertion();
        let new_text = format!("{}{}{}", &text[..range.start], insert, &text[range.end..]);
        let cursor = range.start + at;
        state.update(cx, |s, cx| {
            s.set_value(new_text.clone(), window, cx);
            s.set_selected_range(cursor..cursor, cx);
        });
        Some(new_text)
    }

    /// The dropdown, drawn on top of everything below the field. `on_pick`
    /// gets the clicked index. Live: white, 1 px #d7d7d7, 5 px corners,
    /// 31 px rows with a 15 px icon, 14 px text with grey placeholders, the
    /// selected row #b2ddf6.
    pub fn render(&self, variables: &BTreeMap<String, noshape_units::Quantity>, units: &noshape_units::UnitPrefs, on_pick: impl Fn(usize, &mut Window, &mut App) + 'static) -> AnyElement {
        let pal = p();
        let (text, faint, active) = (rgb(0x333333), rgb(0x666666), rgb(0xb2ddf6));
        let on_pick = std::rc::Rc::new(on_pick);
        let row = |id: SharedString, i: usize, icon: &str| {
            let on_pick = on_pick.clone();
            h_flex()
                .id(id)
                .h(px(31.))
                .p(px(4.))
                .gap(px(5.))
                .when(i == self.selected, |d| d.bg(active))
                .when(i != self.selected, |d| d.hover(|d| d.bg(pal.row_hover)))
                .cursor_pointer()
                .text_size(px(14.))
                .text_color(text)
                .child(crate::ui::glyph(icon, 15., text))
                .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                    cx.stop_propagation();
                    on_pick(i, window, cx);
                })
        };
        let rows = self.entries.iter().take(MAX_ROWS).enumerate().map(|(i, e)| {
            let id = SharedString::from(format!("var-complete-{i}"));
            match e {
                Entry::Variable(name) => {
                    let value = variables.get(name).map(|q| noshape_units::format_quantity(*q, units)).unwrap_or_default();
                    row(id, i, "variable").justify_between().child(div().flex_1().child(format!("#{name}"))).child(div().px(px(4.)).text_color(faint).child(value))
                }
                Entry::Function(name, args) => {
                    let mut r = row(id, i, "function").child(format!("{name}("));
                    for (k, a) in args.iter().enumerate() {
                        if k > 0 {
                            r = r.child(",");
                        }
                        r = r.child(div().text_color(faint).child(*a));
                    }
                    r.child(")")
                }
                Entry::Unit(name) => row(id, i, "dp-units").child(*name),
                Entry::Constant(name) => row(id, i, "function").child(*name),
            }
        });
        // Live 152: "(x) New variable…" (highlighted when selected).
        let new_row = self.new_variable.as_ref().map(|_| row("var-complete-new".into(), self.entries.len().min(MAX_ROWS), "variable").child("New variable…"));
        // Anchored at a zero-height box in the flow right below the field.
        let list = deferred(
            anchored().snap_to_window().child(
                v_flex()
                    .id("var-complete")
                    .min_w(px(154.))
                    .max_w(px(260.))
                    .p(px(1.))
                    .bg(rgb(0xffffff))
                    .border_1()
                    .border_color(rgb(0xd7d7d7))
                    .rounded(px(5.))
                    .shadow_md()
                    .children(rows)
                    .children(new_row),
            ),
        )
        .with_priority(4);
        div().relative().w_full().h_0().child(list).into_any_element()
    }
}

/// The `#identifier` under the cursor: its byte range (`#` included) and
/// the whole name.
fn fragment(text: &str, cursor: usize) -> Option<(Range<usize>, String)> {
    let mut cursor = cursor.min(text.len());
    while !text.is_char_boundary(cursor) {
        cursor -= 1;
    }
    let ident = |c: char| c.is_ascii_alphanumeric() || c == '_';
    let before = &text[..cursor];
    let start = before.char_indices().rev().take_while(|&(_, c)| ident(c)).last().map_or(cursor, |(i, _)| i);
    if !before[..start].ends_with('#') {
        return None;
    }
    let end = cursor + text[cursor..].chars().take_while(|&c| ident(c)).map(char::len_utf8).sum::<usize>();
    Some((start - 1..end, text[start..end].to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    // The glob import brings GPUI's `test` attribute; use the built-in one.
    use std::prelude::v1::test;

    fn shown(text: &str, dim: Option<Dim>, vars: &[&str]) -> Vec<String> {
        let Some((_, e)) = word_entries(text, text.len(), vars, dim) else { return Vec::new() };
        e.iter()
            .map(|e| match e {
                Entry::Variable(n) => format!("#{n}"),
                Entry::Function(n, a) => format!("{n}({})", a.join(",")),
                Entry::Unit(n) | Entry::Constant(n) => n.to_string(),
            })
            .collect()
    }

    /// The lists live shows in a length and an angle dimension box
    /// (behaviours "skb (10-25) quantity autocomplete").
    #[test]
    fn word_lists_match_live() {
        let len = Some(Dim::LENGTH);
        let w = ["w"];
        assert_eq!(shown("1 i", len, &w), ["in", "isInteger(x)"]);
        assert_eq!(shown("1 m", len, &w), ["m", "max(x,y)", "min(x,y)", "mm"]);
        assert!(shown("1 d", len, &w).is_empty());
        assert_eq!(shown("c", len, &w), ["ceil(x)", "ceil(x,unit)", "clamp(x,min,max)", "cm", "cos(angle)"]);
        assert_eq!(shown("a", len, &w), ["abs(x)", "acos(x)", "asin(x)", "atan(x)", "atan2(y,x)"]);
        assert_eq!(shown("f", len, &w), ["false", "floor(x)", "floor(x,unit)", "ft"]);
        assert_eq!(shown("r", len, &w), ["round(x)", "round(x,unit)", "roundToPrecision(x,precision)"]);
        assert_eq!(shown("p", len, &w), ["PI", "roundToPrecision(x,precision)"]);
        assert_eq!(shown("l", len, &w), ["log(x)", "log10(x)"]);
        assert_eq!(shown("e", len, &w), ["exp(x)", "tolerantEquals(x,y)"]);
        assert_eq!(shown("t", len, &w), ["tan(angle)", "tolerantEquals(x,y)", "true", "roundToPrecision(x,precision)"]);
        assert_eq!(shown("s", len, &w), ["sin(angle)", "sqrt(x)"]);
        assert_eq!(shown("h", len, &w), ["hypot(x,y)"]);
        assert_eq!(shown("w", len, &w), ["#w"]);
        assert_eq!(shown("y", len, &w), ["yd"]);
        assert_eq!(shown("ROU", len, &w), ["round(x)", "round(x,unit)", "roundToPrecision(x,precision)"]);
        assert_eq!(shown("cE", len, &w), ["ceil(x)", "ceil(x,unit)"]);
        assert_eq!(shown("2+r", len, &w), ["round(x)", "round(x,unit)", "roundToPrecision(x,precision)"]);
        for none in ["1 ", "b", "d", "u", "(1", "1 u"] {
            assert!(shown(none, len, &w).is_empty(), "{none}");
        }
        let ang = Some(Dim::ANGLE);
        assert_eq!(shown("d", ang, &w), ["deg"]);
        assert_eq!(shown("r", ang, &w), ["rad", "round(x)", "round(x,unit)", "roundToPrecision(x,precision)"]);
        assert_eq!(shown("c", ang, &w), ["ceil(x)", "ceil(x,unit)", "clamp(x,min,max)", "cos(angle)"]);
        assert!(word_entries("#w", 2, &w, len).is_none(), "`#` is the variable list");
    }

    /// Live: Enter on "sq" gives `sqrt()` with the cursor inside, "cl"
    /// `clamp(, , )` at 6, "1 i" `1 in`, "w" `#w`.
    #[test]
    fn insertions_match_live() {
        assert_eq!(Entry::Function("sqrt", &["x"]).insertion(), ("sqrt()".into(), 5));
        assert_eq!(Entry::Function("clamp", &["x", "min", "max"]).insertion(), ("clamp(, , )".into(), 6));
        assert_eq!(Entry::Unit("in").insertion(), ("in".into(), 2));
        assert_eq!(Entry::Variable("w".into()).insertion(), ("#w".into(), 2));
        let (r, _) = word_entries("1 i", 3, &[], Some(Dim::LENGTH)).unwrap();
        assert_eq!(r, 2..3);
        let (r, _) = word_entries("2*fl", 4, &[], Some(Dim::LENGTH)).unwrap();
        assert_eq!(r, 2..4);
    }
}
