//! `#` variable autocomplete for numeric fields (Onshape's "variable
//! autofill"): typing `#` in a field lists the variables in scope, further
//! typing filters the list, and accepting an entry replaces the `#fragment`
//! with `#name`.

use std::ops::Range;

/// An open completion list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Completion {
    /// Byte range of the `#fragment` being completed (`#` included), up to
    /// the end of the identifier under the cursor.
    pub range: Range<usize>,
    /// What was typed after `#` (up to the cursor).
    pub prefix: String,
    /// Matching variable names: prefix matches first, then names containing
    /// the fragment; case-insensitive, alphabetical within each group.
    pub matches: Vec<String>,
}

fn is_ident(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// The completion for `text` with the cursor at byte offset `cursor`, or
/// `None` when the cursor is not right after `#` or a `#name` fragment, or
/// nothing matches.
pub fn variable_completion<'a>(text: &str, cursor: usize, names: impl IntoIterator<Item = &'a str>) -> Option<Completion> {
    let mut cursor = cursor.min(text.len());
    while !text.is_char_boundary(cursor) {
        cursor -= 1;
    }
    let before = &text[..cursor];
    let frag_start = before.char_indices().rev().take_while(|&(_, c)| is_ident(c)).last().map_or(cursor, |(i, _)| i);
    if !before[..frag_start].ends_with('#') {
        return None;
    }
    let hash = frag_start - 1;
    let end = cursor + text[cursor..].chars().take_while(|&c| is_ident(c)).map(char::len_utf8).sum::<usize>();
    let prefix = text[frag_start..cursor].to_string();
    let needle = prefix.to_ascii_lowercase();
    let mut starts: Vec<&str> = Vec::new();
    let mut contains: Vec<&str> = Vec::new();
    for n in names {
        let l = n.to_ascii_lowercase();
        if l.starts_with(&needle) {
            starts.push(n);
        } else if l.contains(&needle) {
            contains.push(n);
        }
    }
    let key = |n: &&str| (n.to_ascii_lowercase(), n.to_string());
    starts.sort_by_key(key);
    contains.sort_by_key(key);
    let mut matches: Vec<String> = starts.into_iter().chain(contains).map(str::to_string).collect();
    matches.dedup();
    if matches.is_empty() {
        return None;
    }
    Some(Completion { range: hash..end, prefix, matches })
}

/// `text` with the completion's fragment replaced by `#name`, and the byte
/// offset of the cursor after the inserted name.
pub fn apply_completion(text: &str, completion: &Completion, name: &str) -> (String, usize) {
    let r = completion.range.start.min(text.len())..completion.range.end.min(text.len());
    let mut out = String::with_capacity(text.len() + name.len() + 1);
    out.push_str(&text[..r.start]);
    out.push('#');
    out.push_str(name);
    let cursor = out.len();
    out.push_str(&text[r.end..]);
    (out, cursor)
}

#[cfg(test)]
mod tests {
    use super::*;

    const VARS: [&str; 4] = ["width", "wall", "height", "Thickness_w"];

    #[test]
    fn hash_lists_every_variable() {
        let c = variable_completion("#", 1, VARS).unwrap();
        assert_eq!(c.range, 0..1);
        assert_eq!(c.prefix, "");
        assert_eq!(c.matches, ["height", "Thickness_w", "wall", "width"]);
    }

    #[test]
    fn typing_filters_prefix_matches_first() {
        let c = variable_completion("2 * #w", 6, VARS).unwrap();
        assert_eq!(c.range, 4..6);
        assert_eq!(c.matches, ["wall", "width", "Thickness_w"]);
        let c = variable_completion("2 * #WI", 7, VARS).unwrap();
        assert_eq!(c.matches, ["width"]);
        assert!(variable_completion("#xyz", 4, VARS).is_none());
    }

    #[test]
    fn only_right_after_a_hash_fragment() {
        assert!(variable_completion("10 mm", 5, VARS).is_none());
        assert!(variable_completion("#width ", 7, VARS).is_none());
        assert!(variable_completion("#width + 2", 10, VARS).is_none());
        assert!(variable_completion("", 0, VARS).is_none());
        // Cursor beyond the end or inside a multi-byte char is clamped.
        assert!(variable_completion("10°#", 99, VARS).is_some());
        assert!(variable_completion("10°w", 5, VARS).is_none());
    }

    #[test]
    fn accepting_replaces_the_fragment() {
        let text = "#wi / 2 + 1 mm";
        let c = variable_completion(text, 3, VARS).unwrap();
        assert_eq!(apply_completion(text, &c, "width"), ("#width / 2 + 1 mm".to_string(), 6));
        // The whole identifier under the cursor is replaced.
        let text = "3 * #widxx";
        let c = variable_completion(text, 7, ["width"]).unwrap();
        assert_eq!(apply_completion(text, &c, "width"), ("3 * #width".to_string(), 10));
        let c = variable_completion("#", 1, VARS).unwrap();
        assert_eq!(apply_completion("#", &c, "wall"), ("#wall".to_string(), 5));
    }
}
