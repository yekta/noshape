//! The Feature Studio editor's line commands, as Ace's defaults in live's editor (live2 feel/studios fs0.json):
//! toggle comment, remove line, duplicate, move and copy lines. Each gives the text range it replaces, the new text
//! for it and the selection after, so the editor applies it as one undo step.

use std::ops::Range;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LineOp {
    ToggleComment,
    RemoveLine,
    Duplicate,
    MoveUp,
    MoveDown,
    CopyUp,
    CopyDown,
}

/// An edit: replace `range` of the old text with `text`; then select `sel` (in the new text).
#[derive(Debug, PartialEq)]
pub struct LineEdit {
    pub range: Range<usize>,
    pub text: String,
    pub sel: Range<usize>,
}

/// The whole lines under `sel`, without the last line break: a selection ending at a line's start leaves that line
/// out (Ace).
fn block(text: &str, sel: &Range<usize>) -> Range<usize> {
    let start = text[..sel.start].rfind('\n').map_or(0, |i| i + 1);
    let end_at = if sel.end > sel.start && sel.end > start && text[..sel.end].ends_with('\n') { sel.end - 1 } else { sel.end };
    let end = text[end_at..].find('\n').map_or(text.len(), |i| end_at + i);
    start..end.max(start)
}

pub fn apply(text: &str, sel: Range<usize>, op: LineOp) -> Option<LineEdit> {
    let b = block(text, &sel);
    let lines = &text[b.clone()];
    match op {
        LineOp::ToggleComment => Some(toggle_comment(text, b, sel)),
        // With its line break: the one after it (the caret at the next line's start), on the last line the one before.
        LineOp::RemoveLine if b.end < text.len() => Some(LineEdit { range: b.start..b.end + 1, text: String::new(), sel: b.start..b.start }),
        LineOp::RemoveLine => {
            let from = b.start.saturating_sub(1);
            let prev = text[..from].rfind('\n').map_or(0, |i| i + 1);
            Some(LineEdit { range: from..b.end, text: String::new(), sel: prev..prev })
        }
        LineOp::Duplicate if sel.is_empty() => Some(LineEdit { range: b.end..b.end, text: format!("\n{lines}"), sel }),
        LineOp::Duplicate => {
            let copy = text[sel.clone()].to_string();
            let n = copy.len();
            Some(LineEdit { range: sel.end..sel.end, text: copy, sel: sel.end..sel.end + n })
        }
        LineOp::MoveUp => {
            let above = text[..b.start.checked_sub(1)?].rfind('\n').map_or(0, |i| i + 1);
            let prev = &text[above..b.start - 1];
            let shift = b.start - above;
            Some(LineEdit { range: above..b.end, text: format!("{lines}\n{prev}"), sel: sel.start - shift..sel.end - shift })
        }
        LineOp::MoveDown => {
            if b.end >= text.len() {
                return None;
            }
            let below_end = text[b.end + 1..].find('\n').map_or(text.len(), |i| b.end + 1 + i);
            let next = &text[b.end + 1..below_end];
            let shift = next.len() + 1;
            Some(LineEdit { range: b.start..below_end, text: format!("{next}\n{lines}"), sel: sel.start + shift..sel.end + shift })
        }
        // The copy goes above (the selection stays where it was, on the copy) or below (the selection moves to it).
        LineOp::CopyUp => Some(LineEdit { range: b.start..b.start, text: format!("{lines}\n"), sel }),
        LineOp::CopyDown => {
            let shift = lines.len() + 1;
            Some(LineEdit { range: b.end..b.end, text: format!("\n{lines}"), sel: sel.start + shift..sel.end + shift })
        }
    }
}

/// `// ` at the lines' least indentation (blank lines left alone); taken away again when every other line has it.
fn toggle_comment(text: &str, b: Range<usize>, sel: Range<usize>) -> LineEdit {
    let lines: Vec<&str> = text[b.clone()].split('\n').collect();
    let indent = |l: &str| l.len() - l.trim_start().len();
    let filled: Vec<&&str> = lines.iter().filter(|l| !l.trim().is_empty()).collect();
    let col = filled.iter().map(|l| indent(l)).min().unwrap_or(0);
    let commented = !filled.is_empty() && filled.iter().all(|l| l.trim_start().starts_with("//"));
    let mut out = Vec::new();
    // (where, bytes added there: negative when taken away)
    let mut deltas: Vec<(usize, isize)> = Vec::new();
    let mut at = b.start;
    for l in &lines {
        if l.trim().is_empty() {
            out.push(l.to_string());
        } else if commented {
            let i = indent(l);
            let n = if l[i..].starts_with("// ") { 3 } else { 2 };
            out.push(format!("{}{}", &l[..i], &l[i + n..]));
            deltas.push((at + i, -(n as isize)));
        } else {
            out.push(format!("{}// {}", &l[..col], &l[col..]));
            deltas.push((at + col, 3));
        }
        at += l.len() + 1;
    }
    let map = |p: usize| {
        let mut q = p as isize;
        for &(pos, d) in &deltas {
            let gone = (-d).max(0) as usize;
            if (d > 0 && p >= pos) || (d < 0 && p >= pos + gone) {
                q += d;
            } else if d < 0 && p > pos {
                q -= (p - pos) as isize;
            }
        }
        q.max(0) as usize
    };
    LineEdit { range: b, text: out.join("\n"), sel: map(sel.start)..map(sel.end) }
}

/// Byte offsets of the brackets outside strings and comments, each with its kind.
fn code_brackets(text: &str) -> Vec<(usize, u8)> {
    let b = text.as_bytes();
    let (mut i, mut out) = (0, Vec::new());
    while i < b.len() {
        match b[i] {
            b'/' if b.get(i + 1) == Some(&b'/') => i = text[i..].find('\n').map_or(b.len(), |n| i + n),
            b'/' if b.get(i + 1) == Some(&b'*') => i = text[i + 2..].find("*/").map_or(b.len(), |n| i + n + 4),
            q @ (b'"' | b'\'') => {
                i += 1;
                while i < b.len() && b[i] != q {
                    i += if b[i] == b'\\' { 2 } else { 1 };
                }
                i += 1;
            }
            c @ (b'(' | b')' | b'[' | b']' | b'{' | b'}') => {
                out.push((i, c));
                i += 1;
            }
            _ => i += 1,
        }
    }
    out
}

/// Ctrl+\ (Ace's jumptomatching): with a bracket right after the caret, the caret goes before its partner; else with
/// one right before it, after its partner. Brackets in strings and comments don't count.
pub fn matching_bracket(text: &str, at: usize) -> Option<usize> {
    let brackets = code_brackets(text);
    let partner = |k: usize| -> Option<usize> {
        let (_, c) = brackets[k];
        let (open, close) = match c {
            b'(' | b')' => (b'(', b')'),
            b'[' | b']' => (b'[', b']'),
            _ => (b'{', b'}'),
        };
        let mut depth = 0i32;
        if c == open {
            for (j, &(_, d)) in brackets.iter().enumerate().skip(k) {
                depth += if d == open { 1 } else if d == close { -1 } else { 0 };
                if depth == 0 {
                    return Some(j);
                }
            }
        } else {
            for j in (0..=k).rev() {
                let d = brackets[j].1;
                depth += if d == close { 1 } else if d == open { -1 } else { 0 };
                if depth == 0 {
                    return Some(j);
                }
            }
        }
        None
    };
    if let Some(k) = brackets.iter().position(|(p, _)| *p == at) {
        return partner(k).map(|j| brackets[j].0);
    }
    let k = brackets.iter().position(|(p, _)| *p + 1 == at)?;
    partner(k).map(|j| brackets[j].0 + 1)
}

/// The next (`back`: previous) case-insensitive occurrence of `query` from `at`, wrapping round the text (Ace's
/// findnext / findprevious).
pub fn find_from(text: &str, query: &str, at: usize, back: bool) -> Option<Range<usize>> {
    if query.is_empty() {
        return None;
    }
    let (t, q) = (text.to_lowercase(), query.to_lowercase());
    if t.len() != text.len() {
        return None;
    }
    let hits: Vec<usize> = t.match_indices(&q).map(|(i, _)| i).collect();
    let start = if back { hits.iter().rev().find(|&&i| i + q.len() <= at).or(hits.last()) } else { hits.iter().find(|&&i| i >= at).or(hits.first()) };
    start.map(|&i| i..i + q.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(text: &str, sel: Range<usize>, op: LineOp) -> (String, Range<usize>) {
        let e = apply(text, sel, op).expect("an edit");
        let mut t = text.to_string();
        t.replace_range(e.range, &e.text);
        (t, e.sel)
    }

    /// Ctrl+/ adds `// ` at the line start with a space, again takes it away (live2 studios.json commentToggle);
    /// several lines at their least indentation.
    #[::core::prelude::v1::test]
    fn ctrl_slash_toggles_line_comments() {
        let (t, s) = run("a\nfoo();\nb", 4..4, LineOp::ToggleComment);
        assert_eq!((t.as_str(), s.clone()), ("a\n// foo();\nb", 7..7));
        let (t, s) = run(&t, s, LineOp::ToggleComment);
        assert_eq!((t.as_str(), s), ("a\nfoo();\nb", 4..4));
        let (t, _) = run("{\n    x;\n\n        y;\n}", 2..17, LineOp::ToggleComment);
        assert_eq!(t, "{\n    // x;\n\n    //     y;\n}");
        let (t, _) = run(&t, 2..20, LineOp::ToggleComment);
        assert_eq!(t, "{\n    x;\n\n        y;\n}");
    }

    #[::core::prelude::v1::test]
    fn remove_and_duplicate_lines() {
        assert_eq!(run("a\nbb\nc", 3..3, LineOp::RemoveLine), ("a\nc".into(), 2..2));
        assert_eq!(run("a\nbb", 3..3, LineOp::RemoveLine), ("a".into(), 0..0), "the last line takes the break before it");
        assert_eq!(run("a\nbb\nc", 3..3, LineOp::Duplicate), ("a\nbb\nbb\nc".into(), 3..3), "an empty selection: its line");
        assert_eq!(run("a\nbb\nc", 2..4, LineOp::Duplicate), ("a\nbbbb\nc".into(), 4..6), "a selection: right after it, selected");
    }

    /// Alt+↑ / Alt+↓ move the lines, Alt+Shift+↑ / ↓ copy them; a selection ending at a line start leaves that line.
    #[::core::prelude::v1::test]
    fn move_and_copy_lines() {
        assert_eq!(run("a\nbb\nc", 3..3, LineOp::MoveUp), ("bb\na\nc".into(), 1..1));
        assert_eq!(run("a\nbb\nc", 3..3, LineOp::MoveDown), ("a\nc\nbb".into(), 5..5));
        assert!(apply("a\nbb", 0..0, LineOp::MoveUp).is_none() && apply("a\nbb", 3..3, LineOp::MoveDown).is_none());
        assert_eq!(run("a\nbb\nc", 0..5, LineOp::MoveDown), ("c\na\nbb".into(), 2..7), "two lines (the selection ends at c's start)");
        assert_eq!(run("a\nbb\nc", 3..3, LineOp::CopyUp), ("a\nbb\nbb\nc".into(), 3..3));
        assert_eq!(run("a\nbb\nc", 3..3, LineOp::CopyDown), ("a\nbb\nbb\nc".into(), 6..6));
    }

    /// Ctrl+\\ jumps between a bracket and its partner, past nested ones and those in strings or comments.
    #[::core::prelude::v1::test]
    fn ctrl_backslash_jumps_to_the_matching_bracket() {
        let t = "f(a, [b], \")\") { // }\n}";
        assert_eq!(matching_bracket(t, 1), Some(13), "before ( : before its )");
        assert_eq!(matching_bracket(t, 14), Some(2), "after ) : after its (");
        assert_eq!(matching_bracket(t, 15), Some(22), "the brace past the comment's");
        assert_eq!(matching_bracket(t, 3), None);
    }

    /// Ctrl+K / Ctrl+Shift+K: the next / previous occurrence from the caret, wrapping, any case.
    #[::core::prelude::v1::test]
    fn find_next_and_previous_wrap() {
        let t = "ab Ab x ab";
        assert_eq!(find_from(t, "ab", 1, false), Some(3..5));
        assert_eq!(find_from(t, "ab", 9, false), Some(0..2), "wraps to the top");
        assert_eq!(find_from(t, "ab", 3, true), Some(0..2));
        assert_eq!(find_from(t, "ab", 0, true), Some(8..10), "wraps to the end");
        assert_eq!(find_from(t, "zz", 0, false), None);
    }
}
