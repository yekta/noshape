//! Onshape's rules for number / quantity fields (docs/handoff/input-behaviour.md;
//! the client's `BTQuantityParameter` / `validateSimpleExpression` /
//! `stepValue`, live behaviours.md "Input behaviour"), shared by every
//! numeric field:
//! - a commit (Enter, Tab, blur) rewrites a plain number with the workspace
//!   unit (`25` → `25 mm`, `5mm` → `5 mm`, `.5` → `0.5 mm`, `5.` → `5 mm`)
//!   and wraps a unitless expression in a dimensioned field (`10+5` →
//!   `(10+5) mm`); counts stay unitless;
//! - unfocused, a field shows the evaluated value in workspace units with
//!   the display decimals, trailing zeros dropped;
//! - ↑ / ↓ and the wheel (focused) step a plain value by the workspace
//!   unit's step (mm 1, cm 0.1, m 0.01, in 0.1, ft 0.01, yd 0.003, deg 1,
//!   rad 0.02, unitless 1), Shift ×10, Ctrl ×0.1, both ×1, counts ±1; a
//!   step that would leave the range does nothing.

use gpui_kit::{AppContext as _, SharedString};
use noshape_units::{AngleUnit, Dim, LengthUnit, UnitPrefs, VariableScope};

// The commit rule is shared with the model (sketch dimensions stored by
// any path, the agent API included, carry their unit).
pub(crate) use noshape_model::quantity_text::{commit_text, unit_word};
use noshape_model::quantity_text::simple;

/// The step of one ↑ / ↓ / wheel notch in the workspace unit of `dim`.
fn base_step(dim: Dim, prefs: &UnitPrefs) -> f64 {
    if dim == Dim::LENGTH {
        match prefs.length {
            LengthUnit::Millimeter => 1.,
            LengthUnit::Centimeter => 0.1,
            LengthUnit::Meter => 0.01,
            LengthUnit::Inch => 0.1,
            LengthUnit::Foot => 0.01,
            LengthUnit::Yard => 0.003,
            LengthUnit::Micrometer => 1.,
        }
    } else if dim == Dim::ANGLE {
        match prefs.angle {
            AngleUnit::Degree => 1.,
            AngleUnit::Radian => 0.02,
        }
    } else {
        1.
    }
}

/// The modifiers of a step.
#[derive(Clone, Copy, Default)]
pub(crate) struct StepMods {
    pub shift: bool,
    pub ctrl: bool,
}

/// What a field steps: its dimension, the workspace units, the variables,
/// whether it is a count, and its range in workspace units.
pub(crate) struct Stepping<'a> {
    pub dim: Dim,
    pub prefs: &'a UnitPrefs,
    pub scope: &'a dyn VariableScope,
    pub integer: bool,
    pub range: (Option<f64>, Option<f64>),
}

/// The field's text after one step up (`up`) or down, or None when the
/// value can't step: an expression, a value that doesn't evaluate, or a
/// step out of the range.
pub(crate) fn step_text(text: &str, up: bool, mods: StepMods, f: &Stepping) -> Option<String> {
    let Stepping { dim, prefs, scope, integer, range } = *f;
    simple(text)?;
    let q = noshape_units::evaluate_as(text, dim, prefs, scope).ok()?;
    let v = q.value / prefs.si_factor(dim);
    let decimals = if dim == Dim::ANGLE { prefs.angle_precision } else { prefs.length_precision };
    let step = if integer {
        1.
    } else {
        let mut s = base_step(dim, prefs);
        match (mods.shift, mods.ctrl) {
            (true, false) => s *= 10.,
            (false, true) if decimals >= 1 => s *= 0.1,
            _ => {}
        }
        s
    };
    let next = if up { v + step } else { v - step };
    if range.0.is_some_and(|m| next < m - 1e-9) || range.1.is_some_and(|m| next > m + 1e-9) {
        return None;
    }
    // Enough decimals for the step itself (0.003 yd, 0.1 mm with Ctrl).
    let step_decimals = (0..=6u8).find(|d| ((step * 10f64.powi(i32::from(*d))).round() - step * 10f64.powi(i32::from(*d))).abs() < 1e-9).unwrap_or(6);
    let num = noshape_units::format_number(next, step_decimals.max(decimals.min(6)));
    Some(match unit_word(dim, prefs) {
        Some(w) if !integer => format!("{num} {w}"),
        _ => num,
    })
}

/// The hover text of an invalid number field (live).
pub(crate) const INVALID_TIP: &str = "Enter a valid expression.";

/// Onshape's invalid number field (dialog fields, the sketch dimension box):
/// a pink `#f9ebec` fill, a 2 px `#bd3039` underline and the hover
/// "Enter a valid expression.". `el` is the field's box (with a bottom
/// border, positioned); unchanged when `invalid` is false.
pub(crate) fn invalid_field(el: gpui_kit::Stateful<gpui_kit::Div>, invalid: bool) -> gpui_kit::Stateful<gpui_kit::Div> {
    use gpui_kit::{ParentElement as _, StatefulInteractiveElement as _, Styled as _, div, px};
    if !invalid {
        return el;
    }
    let pal = crate::theme::p();
    el.relative()
        .bg(pal.error_fill)
        .border_b_1()
        .border_color(pal.error)
        .child(div().absolute().left_0().right_0().bottom_0().h(px(1.)).bg(pal.error))
        .tooltip(crate::ui::tooltip(INVALID_TIP))
}

/// The hover text of an invalid count field (live: Zebra stripes count).
pub(crate) const INVALID_INT_TIP: &str = "Enter an integer.";

/// What a [`QuantityInput`] holds and how it shows it.
#[derive(Clone)]
pub(crate) struct QuantitySpec {
    /// The field's dimension (`Dim::UNITLESS` for counts and plain numbers).
    pub dim: Dim,
    /// A count: whole numbers only, stepped by 1, never given a unit.
    pub integer: bool,
    /// Whether expressions (`1+1`, `#w`) are accepted. Live: dialog and
    /// panel quantities yes; the analysis tools' counts (Zebra stripes) no.
    pub expressions: bool,
    /// The allowed range in SI units (metres, radians), inclusive.
    pub range: (Option<f64>, Option<f64>),
    /// The field's label, for the hover "Depth: 32.1 mm" of an unfocused value.
    pub label: Option<SharedString>,
    /// The invalid hover text when it isn't "Enter a valid expression.".
    pub invalid_tip: Option<&'static str>,
    /// Text size in px (live dialog fields 13.33).
    pub text_px: f32,
    /// Whether focusing selects all (live number fields) or leaves the
    /// caret (live click-to-edit bounds: the caret at the end).
    pub select_on_focus: bool,
}

impl QuantitySpec {
    /// A length or angle (or other dimensioned) quantity.
    pub fn quantity(dim: Dim) -> Self {
        Self { dim, integer: false, expressions: true, range: (None, None), label: None, invalid_tip: None, text_px: 13.33, select_on_focus: true }
    }

    /// A count (whole number, unitless).
    pub fn count() -> Self {
        Self { dim: Dim::UNITLESS, integer: true, ..Self::quantity(Dim::UNITLESS) }
    }

    pub fn range(mut self, min: Option<f64>, max: Option<f64>) -> Self {
        self.range = (min, max);
        self
    }

    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn no_expressions(mut self) -> Self {
        self.expressions = false;
        self
    }

    pub fn text_px(mut self, px: f32) -> Self {
        self.text_px = px;
        self
    }

    pub fn caret_on_focus(mut self) -> Self {
        self.select_on_focus = false;
        self
    }
}

/// How a [`QuantityInput`] commit happened.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum CommitVia {
    Enter,
    /// Tab or click-away.
    Blur,
    /// ↑ / ↓ / wheel.
    Step,
}

/// What a [`QuantityInput`] tells its owner.
pub(crate) enum QuantityEvent {
    /// The text was edited (each keystroke); nothing committed yet.
    Changed,
    /// The text was committed and rewritten by Onshape's rules; read
    /// [`QuantityInput::value`] (None: invalid, the field shows it).
    Committed(CommitVia),
    /// Enter with nothing new to apply (or in a count, after its commit):
    /// owners that accept their dialog on Enter do it here.
    Enter,
    /// Esc in the field (after the kit's revert, if there was an edit).
    Escape,
}

/// Onshape's number field (`os-param-number`) as one reusable view, for every
/// numeric field outside the feature dialogs (which wire the same rules
/// themselves, with their autofill and Create variable popup):
/// - first click selects all; Esc reverts to the text it had when focused
///   and blurs (the input kit's defaults);
/// - a commit (Enter, Tab, click-away) rewrites the text by `commit_text`
///   (`5` → `5 deg`, `1+1` → `(1+1) deg`); Enter re-selects it;
/// - unfocused, the value shows evaluated in workspace units ("15 mm" for
///   `10+5`), its hover "Label: 15 mm"; focusing shows the text again;
/// - a committed value that doesn't evaluate (or isn't a whole number in a
///   count, or is out of range) is pink with a red underline and the hover
///   "Enter a valid expression." / "Enter an integer.";
/// - ↑ / ↓ step a plain value (Shift ×10, Ctrl ×0.1, counts ±1), the wheel
///   too while focused;
/// - a 1 px `#d7d7d7` underline, blue on hover, 2 px blue while focused.
pub(crate) struct QuantityInput {
    state: gpui_kit::Entity<gpui_kit::component::input::InputState>,
    spec: QuantitySpec,
    units: UnitPrefs,
    variables: std::collections::BTreeMap<String, noshape_units::Quantity>,
    /// The text as last committed (what `value` evaluates and the invalid
    /// state reflects).
    committed: String,
    _subs: Vec<gpui_kit::Subscription>,
}

impl gpui_kit::EventEmitter<QuantityEvent> for QuantityInput {}

impl QuantityInput {
    pub fn new(text: impl Into<String>, spec: QuantitySpec, units: UnitPrefs, window: &mut gpui_kit::Window, cx: &mut gpui_kit::Context<Self>) -> Self {
        use gpui_kit::component::input::{InputEvent, InputState};
        let text: String = text.into();
        let state = cx.new(|cx| InputState::new(window, cx).default_value(text.clone()).select_on_focus(spec.select_on_focus));
        let sub = cx.subscribe_in(&state, window, |this: &mut Self, state, ev: &InputEvent, window, cx| match ev {
            InputEvent::Change => cx.emit(QuantityEvent::Changed),
            InputEvent::PressEnter { .. } => {
                let changed = this.commit(CommitVia::Enter, window, cx);
                state.update(cx, |s, cx| s.select_all(window, cx));
                if !changed || this.spec.integer {
                    cx.emit(QuantityEvent::Enter);
                }
            }
            InputEvent::Focus => cx.notify(),
            InputEvent::Blur => {
                this.commit(CommitVia::Blur, window, cx);
                cx.notify();
            }
        });
        let mut this = Self { state, spec, units, variables: Default::default(), committed: String::new(), _subs: vec![sub] };
        this.committed = this.rewrite(&text);
        if this.committed != text {
            let c = this.committed.clone();
            this.state.update(cx, |s, cx| s.set_value(c, window, cx));
        }
        this
    }

    /// The committed value in SI units (metres, radians; a count as is), or
    /// None when it is invalid.
    pub fn value(&self) -> Option<f64> {
        self.evaluate(&self.committed).ok()
    }

    /// The committed text (what an owner stores as the expression).
    pub fn text(&self) -> &str {
        &self.committed
    }

    /// Replace the text (committed as typed, rewritten by Onshape's rules),
    /// e.g. when a dialog opens on an existing feature.
    pub fn set_text(&mut self, text: impl Into<String>, window: &mut gpui_kit::Window, cx: &mut gpui_kit::Context<Self>) {
        let text = self.rewrite(&text.into());
        self.committed = text.clone();
        self.state.update(cx, |s, cx| s.set_value(text, window, cx));
        cx.notify();
    }

    pub fn set_variables(&mut self, variables: std::collections::BTreeMap<String, noshape_units::Quantity>, cx: &mut gpui_kit::Context<Self>) {
        if variables != self.variables {
            self.variables = variables;
            cx.notify();
        }
    }

    /// The input inside (focus, selection).
    pub fn state(&self) -> &gpui_kit::Entity<gpui_kit::component::input::InputState> {
        &self.state
    }

    fn dim(&self) -> Dim {
        if self.spec.integer { Dim::UNITLESS } else { self.spec.dim }
    }

    /// Onshape's commit rewrite of `text` (an empty field keeps its last
    /// committed text, live).
    fn rewrite(&self, text: &str) -> String {
        if text.trim().is_empty() {
            return self.committed.clone();
        }
        commit_text(text, self.dim(), &self.units, &self.variables)
    }

    /// `text` evaluated in SI units, or the hover text saying why it can't be.
    fn evaluate(&self, text: &str) -> Result<f64, &'static str> {
        let tip = self.spec.invalid_tip.unwrap_or(if self.spec.integer && !self.spec.expressions { INVALID_INT_TIP } else { INVALID_TIP });
        if !self.spec.expressions && simple(text).is_none() {
            return Err(tip);
        }
        let q = noshape_units::evaluate_as(text, self.dim(), &self.units, &self.variables).map_err(|_| tip)?;
        let v = q.value;
        if self.spec.integer && (v - v.round()).abs() > 1e-9 {
            return Err(self.spec.invalid_tip.unwrap_or(INVALID_INT_TIP));
        }
        let (lo, hi) = self.spec.range;
        if lo.is_some_and(|m| v < m - 1e-12) || hi.is_some_and(|m| v > m + 1e-12) {
            return Err(tip);
        }
        Ok(v)
    }

    /// Commit the input's text: rewrite it, remember it, tell the owner.
    /// True when the committed text changed.
    fn commit(&mut self, via: CommitVia, window: &mut gpui_kit::Window, cx: &mut gpui_kit::Context<Self>) -> bool {
        let text = self.state.read(cx).value().to_string();
        let next = self.rewrite(&text);
        if next != text {
            self.state.update(cx, |s, cx| s.set_value(next.clone(), window, cx));
        }
        let changed = next != self.committed;
        self.committed = next;
        cx.emit(QuantityEvent::Committed(via));
        cx.notify();
        changed
    }

    /// One ↑ / ↓ / wheel step of a plain committed value (committed and
    /// selected). False when it can't step.
    fn step(&mut self, up: bool, mods: StepMods, window: &mut gpui_kit::Window, cx: &mut gpui_kit::Context<Self>) -> bool {
        let text = self.state.read(cx).value().to_string();
        if text != self.committed {
            return false;
        }
        let dim = self.dim();
        let f = self.units.si_factor(dim);
        let range = (self.spec.range.0.map(|v| v / f), self.spec.range.1.map(|v| v / f));
        let stepping = Stepping { dim, prefs: &self.units, scope: &self.variables, integer: self.spec.integer, range };
        let Some(next) = step_text(&text, up, mods, &stepping) else { return false };
        self.state.update(cx, |s, cx| {
            s.set_value(next.clone(), window, cx);
            s.select_all(window, cx);
        });
        self.committed = next;
        cx.emit(QuantityEvent::Committed(CommitVia::Step));
        cx.notify();
        true
    }
}

impl gpui_kit::Render for QuantityInput {
    fn render(&mut self, window: &mut gpui_kit::Window, cx: &mut gpui_kit::Context<Self>) -> impl gpui_kit::IntoElement {
        use gpui_kit::base::actions::{SelectDown, SelectUp};
        use gpui_kit::component::Sizable as _;
        use gpui_kit::component::input::{Escape, Input, MoveDown, MoveUp};
        use gpui_kit::prelude::FluentBuilder as _;
        use gpui_kit::*;
        let pal = crate::theme::p();
        let focused = self.state.read(cx).focus_handle(cx).is_focused(window);
        let text = self.state.read(cx).value().to_string();
        let invalid_tip = (!self.committed.trim().is_empty()).then(|| self.evaluate(&self.committed).err()).flatten();
        let invalid = invalid_tip.is_some() && text == self.committed;
        // Unfocused, a valid value shows evaluated in workspace units.
        let shown = (!focused && !invalid && !self.spec.integer)
            .then(|| noshape_units::evaluate_as(&self.committed, self.spec.dim, &self.units, &self.variables).ok())
            .flatten()
            .map(|q| noshape_units::format_quantity(q, &self.units).replace('°', " deg"))
            .filter(|c| c.replace(' ', "") != self.committed.trim().replace(' ', ""));
        let tsize = px(self.spec.text_px);
        let st_click = self.state.clone();
        let label = self.spec.label.clone();
        let shift = StepMods { shift: true, ctrl: false };
        let field = div()
            .id("qty-field")
            .w_full()
            .relative()
            .border_b_1()
            .border_color(if invalid { pal.error } else { pal.border })
            .hover(|d| d.border_color(pal.accent))
            // Live: the focused field has a 2 px blue underline (a 1 px
            // border plus a 1 px line above it).
            .when(focused && !invalid, |d| d.border_color(pal.accent).child(div().absolute().left_0().right_0().bottom_0().h(px(1.)).bg(pal.accent)))
            .child(div().when(shown.is_some(), |d| d.opacity(0.)).child(Input::new(&self.state).small().appearance(false).text_size(tsize).px(px(3.)).text_align(TextAlign::Right)))
            .when_some(shown, |d, c| {
                let tip = label.as_ref().map(|l| format!("{l}: {c}"));
                d.child(
                    div()
                        .id("qty-shown")
                        .when_some(tip, |d, t| d.tooltip(crate::ui::tooltip(t)))
                        .absolute()
                        .inset_0()
                        .px(px(3.))
                        .flex()
                        .items_center()
                        .justify_end()
                        .text_size(tsize)
                        .text_color(pal.text)
                        .cursor_text()
                        .child(c)
                        .on_mouse_down(MouseButton::Left, move |_, window, cx| st_click.update(cx, |s, cx| s.focus(window, cx))),
                )
            });
        let field = match invalid_tip.filter(|_| invalid) {
            Some(tip) => field.relative().bg(pal.error_fill).child(div().absolute().left_0().right_0().bottom_0().h(px(1.)).bg(pal.error)).tooltip(crate::ui::tooltip(tip)),
            None => field,
        };
        div()
            .id("qty")
            .w_full()
            .child(gpui_kit::base::input::input_frame(field))
            // Esc: the kit reverts (and blurs); the owner hears of it after.
            .capture_action(cx.listener(|_, _: &Escape, window, cx| {
                cx.defer_in(window, |_, _, cx| cx.emit(QuantityEvent::Escape));
            }))
            // In a dialog that binds Esc to Cancel (feature and mate dialogs:
            // "FeatureDialog > Input"), an edited field reverts first and
            // keeps the key; the next Esc cancels the dialog (live).
            .capture_action(cx.listener(|this, _: &crate::actions::Cancel, window, cx| {
                if this.state.update(cx, |s, cx| s.revert_escape(window, cx)) {
                    cx.stop_propagation();
                    cx.emit(QuantityEvent::Escape);
                }
            }))
            // ↑ / ↓ step a plain committed value; Shift ×10 (the input's
            // select-up / -down), Ctrl ×0.1 (no input binding).
            .capture_action(cx.listener(move |this, _: &SelectUp, window, cx| {
                if this.step(true, shift, window, cx) {
                    cx.stop_propagation();
                }
            }))
            .capture_action(cx.listener(move |this, _: &SelectDown, window, cx| {
                if this.step(false, shift, window, cx) {
                    cx.stop_propagation();
                }
            }))
            .capture_key_down(cx.listener(|this, e: &KeyDownEvent, window, cx| {
                let k = &e.keystroke;
                if (k.key == "up" || k.key == "down") && k.modifiers.control && !k.modifiers.alt && this.step(k.key == "up", StepMods { shift: k.modifiers.shift, ctrl: true }, window, cx) {
                    cx.stop_propagation();
                }
            }))
            .capture_action(cx.listener(|this, _: &MoveUp, window, cx| {
                let m = window.modifiers();
                if this.step(true, StepMods { shift: m.shift, ctrl: m.control || m.platform }, window, cx) {
                    cx.stop_propagation();
                }
            }))
            .capture_action(cx.listener(|this, _: &MoveDown, window, cx| {
                let m = window.modifiers();
                if this.step(false, StepMods { shift: m.shift, ctrl: m.control || m.platform }, window, cx) {
                    cx.stop_propagation();
                }
            }))
            // The wheel steps like ↑ / ↓, only while focused (live).
            .on_scroll_wheel(cx.listener(|this, e: &ScrollWheelEvent, window, cx| {
                if !this.state.read(cx).focus_handle(cx).is_focused(window) {
                    return;
                }
                let dy = match e.delta {
                    ScrollDelta::Lines(l) => f64::from(l.y),
                    ScrollDelta::Pixels(p) => f64::from(f32::from(p.y)),
                };
                if dy.abs() < 1e-6 {
                    return;
                }
                cx.stop_propagation();
                this.step(dy > 0., StepMods { shift: e.modifiers.shift, ctrl: e.modifiers.control || e.modifiers.platform }, window, cx);
            }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use noshape_units::EmptyScope;

    fn mm() -> UnitPrefs {
        UnitPrefs::default()
    }

    #[test]
    fn commit_appends_units_like_onshape() {
        let p = mm();
        assert_eq!(commit_text("25", Dim::LENGTH, &p, &EmptyScope), "25 mm");
        assert_eq!(commit_text("5mm", Dim::LENGTH, &p, &EmptyScope), "5 mm");
        assert_eq!(commit_text(".5", Dim::LENGTH, &p, &EmptyScope), "0.5 mm");
        assert_eq!(commit_text("5.", Dim::LENGTH, &p, &EmptyScope), "5 mm");
        assert_eq!(commit_text("2 in", Dim::LENGTH, &p, &EmptyScope), "2 in");
        assert_eq!(commit_text("5", Dim::ANGLE, &p, &EmptyScope), "5 deg");
        assert_eq!(commit_text("10+5", Dim::LENGTH, &p, &EmptyScope), "(10+5) mm");
        assert_eq!(commit_text("1+2", Dim::ANGLE, &p, &EmptyScope), "(1+2) deg");
        assert_eq!(commit_text("10 mm + 5 mm", Dim::LENGTH, &p, &EmptyScope), "10 mm + 5 mm");
        assert_eq!(commit_text("1+2", Dim::UNITLESS, &p, &EmptyScope), "1+2");
        assert_eq!(commit_text("5", Dim::UNITLESS, &p, &EmptyScope), "5");
        assert_eq!(commit_text("abc", Dim::LENGTH, &p, &EmptyScope), "abc");
        assert_eq!(commit_text("  ", Dim::LENGTH, &p, &EmptyScope), "");
    }

    #[test]
    fn steps_like_onshape() {
        let p = mm();
        let len = Stepping { dim: Dim::LENGTH, prefs: &p, scope: &EmptyScope, integer: false, range: (None, None) };
        let s = |t: &str, up, shift, ctrl| step_text(t, up, StepMods { shift, ctrl }, &len);
        assert_eq!(s("10 mm", true, false, false).as_deref(), Some("11 mm"));
        assert_eq!(s("10 mm", false, true, false).as_deref(), Some("0 mm"));
        assert_eq!(s("10 mm", true, false, true).as_deref(), Some("10.1 mm"));
        assert_eq!(s("10 mm", true, true, true).as_deref(), Some("11 mm"));
        assert_eq!(s("10+5", true, false, false), None);
        let a = step_text("5 deg", true, StepMods { shift: true, ctrl: false }, &Stepping { dim: Dim::ANGLE, prefs: &p, scope: &EmptyScope, integer: false, range: (None, None) });
        assert_eq!(a.as_deref(), Some("15 deg"));
        let count = Stepping { dim: Dim::UNITLESS, prefs: &p, scope: &EmptyScope, integer: true, range: (Some(1.), None) };
        assert_eq!(step_text("2", true, StepMods { shift: true, ctrl: false }, &count).as_deref(), Some("3"));
        assert_eq!(step_text("1", false, StepMods::default(), &count), None);
    }
}
