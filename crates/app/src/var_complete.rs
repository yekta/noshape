//! `#` variable autofill dropdown for numeric fields (Onshape: typing `#`
//! lists the variables in scope; typing filters, ↑/↓ move, Enter or a click
//! inserts, Esc closes). The matching logic is
//! [`noshape_units::complete`]; this is the shared UI state and list.

use crate::theme::p;
use gpui_kit::component::input::InputState;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_units::complete::{Completion, apply_completion, variable_completion};
use std::collections::BTreeMap;

/// Most entries shown at once.
const MAX_ROWS: usize = 8;

/// An open list for one field.
pub struct VarCompletion {
    /// Which field (dialog parameter path).
    pub key: String,
    pub completion: Completion,
    pub selected: usize,
    /// The typed `#name` when no variable has it: the list ends with
    /// "(x) New variable…" (live part-studio/152).
    pub new_variable: Option<String>,
}

impl VarCompletion {
    /// The list for `state`'s text and cursor, or `None`.
    pub fn compute(key: &str, state: &InputState, variables: &BTreeMap<String, noshape_units::Quantity>) -> Option<Self> {
        let text = state.value().to_string();
        let cursor = state.cursor();
        let (range, prefix, name) = fragment(&text, cursor)?;
        let new_variable = (!name.is_empty() && noshape_model::variable_table::is_identifier(&name) && !variables.contains_key(&name)).then_some(name);
        let c = variable_completion(&text, cursor, variables.keys().map(String::as_str)).or_else(|| new_variable.as_ref().map(|_| Completion { range, prefix, matches: Vec::new() }))?;
        Some(Self { key: key.to_string(), completion: c, selected: 0, new_variable })
    }

    /// Rows: the matches, then "New variable…".
    fn rows(&self) -> usize {
        self.completion.matches.len().min(MAX_ROWS) + usize::from(self.new_variable.is_some())
    }

    /// Whether entry `index` (the selected one when `None`) is "New variable…".
    pub fn picks_new(&self, index: Option<usize>) -> bool {
        self.new_variable.is_some() && index.unwrap_or(self.selected) == self.completion.matches.len().min(MAX_ROWS)
    }

    /// ↑ / ↓.
    pub fn move_selection(&mut self, delta: isize) {
        let n = self.rows() as isize;
        if n > 0 {
            self.selected = (self.selected as isize + delta).rem_euclid(n) as usize;
        }
    }

    /// Insert match `index` (the selected one when `None`) into `state`.
    /// Returns the new text (the caller treats it as a user edit).
    pub fn accept(&self, index: Option<usize>, state: &Entity<InputState>, window: &mut Window, cx: &mut App) -> Option<String> {
        let name = self.completion.matches.get(index.unwrap_or(self.selected))?;
        let text = state.read(cx).value().to_string();
        let (new_text, cursor) = apply_completion(&text, &self.completion, name);
        state.update(cx, |s, cx| {
            s.set_value(new_text.clone(), window, cx);
            s.set_selected_range(cursor..cursor, cx);
        });
        Some(new_text)
    }

    /// The dropdown, drawn on top of everything below the field. `on_pick`
    /// gets the clicked index.
    pub fn render(&self, variables: &BTreeMap<String, noshape_units::Quantity>, units: &noshape_units::UnitPrefs, on_pick: impl Fn(usize, &mut Window, &mut App) + 'static) -> AnyElement {
        let pal = p();
        let on_pick = std::rc::Rc::new(on_pick);
        let rows = self.completion.matches.iter().take(MAX_ROWS).enumerate().map(|(i, name)| {
            let value = variables.get(name).map(|q| noshape_units::format_quantity(*q, units)).unwrap_or_default();
            let on_pick = on_pick.clone();
            h_flex()
                .id(SharedString::from(format!("var-complete-{i}")))
                .gap_3()
                .px_2()
                .py(px(3.))
                .justify_between()
                .when(i == self.selected, |d| d.bg(pal.accent_soft))
                .hover(|d| d.bg(pal.row_hover))
                .cursor_pointer()
                .child(div().text_size(px(12.)).text_color(pal.text).child(format!("#{name}")))
                .child(div().text_size(px(11.)).text_color(pal.text_faint).child(value))
                .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                    cx.stop_propagation();
                    on_pick(i, window, cx);
                })
        });
        // Live 152: "(x) New variable…" (highlighted when selected).
        let new_row = self.new_variable.as_ref().map(|_| {
            let i = self.completion.matches.len().min(MAX_ROWS);
            let on_pick = on_pick.clone();
            h_flex()
                .id("var-complete-new")
                .gap(px(6.))
                .px_2()
                .py(px(3.))
                .when(i == self.selected, |d| d.bg(pal.accent_soft))
                .hover(|d| d.bg(pal.row_hover))
                .cursor_pointer()
                .child(crate::ui::glyph("variable", 14., pal.text))
                .child(div().text_size(px(13.)).text_color(pal.text).child("New variable…"))
                .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                    cx.stop_propagation();
                    on_pick(i, window, cx);
                })
        });
        // Anchored at a zero-height box in the flow right below the field.
        let list = deferred(
            anchored().snap_to_window().child(
                v_flex()
                    .id("var-complete")
                    .min_w(px(160.))
                    .max_w(px(260.))
                    .py_1()
                    .bg(pal.panel_raised)
                    .border_1()
                    .border_color(pal.menu_outline)
                    .shadow_md()
                    .children(rows)
                    .children(new_row),
            ),
        )
        .with_priority(4);
        div().relative().w_full().h_0().child(list).into_any_element()
    }
}

/// The `#identifier` under the cursor: its byte range (`#` included), the
/// part before the cursor, and the whole name.
fn fragment(text: &str, cursor: usize) -> Option<(std::ops::Range<usize>, String, String)> {
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
    Some((start - 1..end, text[start..cursor].to_string(), text[start..end].to_string()))
}
