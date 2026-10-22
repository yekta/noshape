//! The drawing editor's own number widgets (live "Input behaviour outside
//! feature dialogs" › Drawing Wt widgets): unlike Onshape's other number
//! fields they put the caret at the click, ignore Esc and know no units or
//! expressions.
//! - Dropdowns (`XeDropdown`, Text height / Arrowhead ▾): four decimals in
//!   the drawing's units; only the leading number counts ("3 mm" → 3.0000,
//!   "1+1" → 1.0000); no number silently reverts; ▾ lists only the current
//!   value (live dom/drawing-wt-widgets).
//! - Spinboxes (`XeSpinBox`, Stack gap size, Text gap…): a number or
//!   nothing ("1 mm" is invalid: pale orange, hover "Must be a number");
//!   ↑ / ↓ and the spin arrows step by the field's step (0.05 in; mm:
//!   Stack gap 0.05, Text gap 0.1), clamped at 0.

/// Live's invalid spinbox background (rgba(221, 82, 40, 0.1)).
pub const INVALID_BG: u32 = 0xdd52281a;
/// The invalid spinbox's hover text.
pub const INVALID_TIP: &str = "Must be a number";

/// How a value shows: four decimals, no unit (the unit is a label beside).
pub fn show(v: f64) -> String {
    show_dec(v, 4)
}

/// A value with `decimals` decimals (table padding: 3).
pub fn show_dec(v: f64, decimals: usize) -> String {
    format!("{v:.decimals$}")
}

/// A dropdown's typed text: its leading number (sign, digits, one point),
/// whatever follows; None when it doesn't start with one (revert).
pub fn leading_number(text: &str) -> Option<f64> {
    let t = text.trim_start();
    let mut end = 0;
    let mut point = false;
    for (i, c) in t.char_indices() {
        match c {
            '-' | '+' if i == 0 => {}
            '0'..='9' => {}
            '.' if !point => point = true,
            _ => break,
        }
        end = i + c.len_utf8();
    }
    t[..end].parse::<f64>().ok()
}

/// A spinbox's typed text: a plain number only.
pub fn spin_number(text: &str) -> Option<f64> {
    let t = text.trim();
    (!t.is_empty() && t.chars().all(|c| c.is_ascii_digit() || matches!(c, '.' | '-' | '+'))).then(|| t.parse::<f64>().ok()).flatten()
}

/// One ↑ (`up`) / ↓ step of a spinbox showing `text` (an invalid text
/// steps from `current`), clamped at 0.
pub fn spin_step(text: &str, current: f64, up: bool, step: f64) -> f64 {
    let v = spin_number(text).unwrap_or(current);
    let next = if up { v + step } else { v - step };
    // Snap away float noise (0.1 + 0.05 = 0.15000000000000002).
    ((next * 1e9).round() / 1e9).max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The Stack gap spinbox's step (inch).
    const GAP_STEP: f64 = 0.05;

    /// Live, Drawing properties › Annotations and Sheet properties (ANSI A
    /// inch).
    #[test]
    fn dropdowns_and_spinboxes_read_text_like_live() {
        // Notes › Text height (dropdown).
        assert_eq!(leading_number("0.15").map(show).as_deref(), Some("0.1500"));
        assert_eq!(leading_number("3 mm").map(show).as_deref(), Some("3.0000"));
        assert_eq!(leading_number("1+1").map(show).as_deref(), Some("1.0000"));
        assert_eq!(leading_number("abc"), None);
        assert_eq!(leading_number(""), None);
        // Callouts › Stack gap size (spinbox).
        assert_eq!(spin_number("0.06").map(show).as_deref(), Some("0.0600"));
        assert_eq!(spin_number("1 mm"), None);
        assert_eq!(spin_number("1+1"), None);
        assert_eq!(spin_number("abc"), None);
        assert_eq!(show(spin_step("0.0500", 0.05, true, GAP_STEP)), "0.1000");
        assert_eq!(show(spin_step("0.0300", 0.03, false, GAP_STEP)), "0.0000");
        assert_eq!(show(spin_step("1 mm", 0.05, true, GAP_STEP)), "0.1000");
        // Sheet properties › Custom › width (`XeInput`, "11 in").
        let width = crate::drawing_sheetprops::commit_sheet_len;
        assert_eq!(width("12", "11 in", true), "12 in");
        assert_eq!(width("300 mm", "11 in", true), "300 mm");
        assert_eq!(width("1+1", "11 in", true), "1 in");
        assert_eq!(width("abc", "1 in", true), "1 in");
        assert_eq!(width("420", "297 mm", false), "420 mm");
    }
}
