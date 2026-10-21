//! Preferences › Decimal format › "Use comma as decimal separator" (Onshape
//! help "Numeric fields", "Variables": with commas as decimal separators,
//! array items and arguments need a space after their comma, `[1, 4, 9][#i]`).
//!
//! Expressions are stored in the canonical period form, so documents read
//! the same whatever the setting: typed text is converted with
//! [`delocalize`] when it becomes an [`ExprValue`](crate::ExprValue), and
//! text shown to the user goes through [`localize`]. Quantities formatted
//! for the user ([`format_quantity`](crate::format_quantity)) use the comma
//! too; [`format_number`](crate::format_number) does not (drawings have
//! their own decimal separator).

use std::sync::atomic::{AtomicBool, Ordering};

static COMMA: AtomicBool = AtomicBool::new(false);

/// Sets the user's decimal separator (true: comma).
pub fn set_comma_decimal(on: bool) {
    COMMA.store(on, Ordering::Relaxed);
}

/// Whether the user writes decimals with a comma.
pub fn comma_decimal() -> bool {
    COMMA.load(Ordering::Relaxed)
}

/// Rewrites the separators between digits outside string literals.
fn rewrite(text: &str, f: impl Fn(char, &mut String)) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len() + 4);
    let mut in_string = false;
    for (i, &c) in chars.iter().enumerate() {
        if c == '"' {
            in_string = !in_string;
        }
        let between_digits = i > 0 && chars[i - 1].is_ascii_digit() && chars.get(i + 1).is_some_and(char::is_ascii_digit);
        if !in_string && between_digits && (c == '.' || c == ',') {
            f(c, &mut out);
        } else {
            out.push(c);
        }
    }
    out
}

/// Canonical expression text as the user reads it: decimal periods become
/// commas, and a comma between two digits (a list separator) gets the space
/// the comma form needs. Unchanged when the setting is off.
pub fn localize(text: &str) -> String {
    if !comma_decimal() {
        return text.to_string();
    }
    localize_always(text)
}

fn localize_always(text: &str) -> String {
    rewrite(text, |c, out| out.push_str(if c == '.' { "," } else { ", " }))
}

/// Typed expression text to the canonical form: a comma between two digits
/// is a decimal point. Unchanged when the setting is off.
pub fn delocalize(text: &str) -> String {
    if !comma_decimal() {
        return text.to_string();
    }
    delocalize_always(text)
}

fn delocalize_always(text: &str) -> String {
    rewrite(text, |c, out| out.push(if c == ',' { '.' } else { c }))
}

/// A formatted number (period form) in the user's separator.
pub(crate) fn localize_number(s: String) -> String {
    if comma_decimal() { s.replace('.', ",") } else { s }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comma_form_round_trips() {
        assert_eq!(localize_always("1.5 mm + #w * 2.25"), "1,5 mm + #w * 2,25");
        assert_eq!(delocalize_always("1,5 mm + #w * 2,25"), "1.5 mm + #w * 2.25");
        // A canonical list keeps its items: the comma form needs the space.
        assert_eq!(localize_always("[1,4.5][#i]"), "[1, 4,5][#i]");
        assert_eq!(delocalize_always("[1, 4,5][#i]"), "[1, 4.5][#i]");
        // String literals are left alone.
        assert_eq!(localize_always("\"v1.5\" ~ 2.5"), "\"v1.5\" ~ 2,5");
        assert_eq!(delocalize_always("max(1, 2)"), "max(1, 2)");
    }
}
