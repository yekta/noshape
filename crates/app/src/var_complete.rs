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
}

impl VarCompletion {
    /// The list for `state`'s text and cursor, or `None`.
    pub fn compute(key: &str, state: &InputState, variables: &BTreeMap<String, noshape_units::Quantity>) -> Option<Self> {
        let text = state.value().to_string();
        let c = variable_completion(&text, state.cursor(), variables.keys().map(String::as_str))?;
        Some(Self { key: key.to_string(), completion: c, selected: 0 })
    }

    /// ↑ / ↓.
    pub fn move_selection(&mut self, delta: isize) {
        let n = self.completion.matches.len().min(MAX_ROWS) as isize;
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
                    .children(rows),
            ),
        )
        .with_priority(4);
        div().relative().w_full().h_0().child(list).into_any_element()
    }
}
