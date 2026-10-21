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
