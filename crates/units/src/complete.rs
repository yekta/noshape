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
    /// Matching variable names ([`words_match`]), alphabetical upper-cased (live: `_` after the letters).
    pub matches: Vec<String>,
}

/// A name's words for matching: split at `_` and where a lower-case letter or digit meets an upper-case one, lower
/// case.
fn words(name: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut prev: Option<char> = None;
    for c in name.chars() {
        let split = c == '_' || (c.is_ascii_uppercase() && prev.is_some_and(|p| p.is_ascii_lowercase() || p.is_ascii_digit()));
        if split && !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
        }
        if c != '_' {
            cur.push(c.to_ascii_lowercase());
        }
        prev = Some(c);
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// Onshape's variable match (live behaviours "Variable autocomplete in expression fields"): case-insensitive, the
/// typed text (its `#` and `_` ignored) read as prefixes of the name's words, in order, from any word, words skipped
/// as needed: "fancontrol" and "fcb" find fan_control_board_depth, "board" board_thickness too; not a plain substring
/// or subsequence ("fanco" keeps fan_blade_count and drops fan_clearance).
pub fn words_match(name: &str, typed: &str) -> bool {
    let needle: Vec<char> = typed.chars().filter(|c| *c != '#' && *c != '_').map(|c| c.to_ascii_lowercase()).collect();
    let words: Vec<Vec<char>> = words(name).into_iter().map(|w| w.chars().collect()).collect();
    // Whether needle[at..] is consumed by prefixes of words[from..], in order.
    fn rest(needle: &[char], words: &[Vec<char>]) -> bool {
        if needle.is_empty() {
            return true;
        }
        words.iter().enumerate().any(|(i, w)| {
            let common = w.iter().zip(needle).take_while(|(a, b)| a == b).count();
            (1..=common).rev().any(|k| rest(&needle[k..], &words[i + 1..]))
        })
    }
    rest(&needle, &words)
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
    let mut matches: Vec<String> = names.into_iter().filter(|n| words_match(n, &prefix)).map(str::to_string).collect();
    // Live sorts the names upper-cased: `_` after the letters (#fanControlBoardHeight before #fan_control_…).
    matches.sort_by_key(|n| (n.to_ascii_uppercase(), n.clone()));
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
    fn typing_filters_by_word_prefixes_alphabetically() {
        let c = variable_completion("2 * #w", 6, VARS).unwrap();
        assert_eq!(c.range, 4..6);
        assert_eq!(c.matches, ["Thickness_w", "wall", "width"]);
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

    /// Live behaviours "Variable autocomplete in expression fields" (11-13, the user's hat15 report): each probe's
    /// list, alphabetical and case-insensitive.
    #[test]
    fn variables_match_live_word_prefix_rule() {
        const LIVE: [&str; 17] = [
            "fan_control_board_depth", "fan_control_board_wall_length", "board_thickness", "Fan_Diameter", "fan_blade_count", "fan_hub_dia", "fan_rpm",
            "fan_mount_hole", "fan_screw_len", "fan_guard_gap", "fan_inlet", "fan_outlet", "fan_offset", "fan_clearance", "control_gap", "width", "tilt",
        ];
        let list = |typed: &str| variable_completion(&format!("#{typed}"), typed.len() + 1, LIVE).map(|c| c.matches).unwrap_or_default();
        assert_eq!(list("fancontrol"), ["fan_control_board_depth", "fan_control_board_wall_length"]);
        assert_eq!(list("fcb"), ["fan_control_board_depth", "fan_control_board_wall_length"]);
        assert_eq!(list("fc"), ["fan_blade_count", "fan_clearance", "fan_control_board_depth", "fan_control_board_wall_length"]);
        assert_eq!(list("board"), ["board_thickness", "fan_control_board_depth", "fan_control_board_wall_length"]);
        assert_eq!(list("b"), ["board_thickness", "fan_blade_count", "fan_control_board_depth", "fan_control_board_wall_length"]);
        assert_eq!(list("fanco"), ["fan_blade_count", "fan_control_board_depth", "fan_control_board_wall_length"]);
        assert_eq!(list("fancon"), ["fan_control_board_depth", "fan_control_board_wall_length"]);
        assert_eq!(list("board_wall"), ["fan_control_board_wall_length"]);
        let fan = list("fan");
        assert_eq!(fan.len(), 13);
        assert_eq!(&fan[..4], ["fan_blade_count", "fan_clearance", "fan_control_board_depth", "fan_control_board_wall_length"]);
        assert_eq!(fan[4], "Fan_Diameter", "case-insensitive order: Fan_Diameter among the fan_ ones");
        assert_eq!(list("FAN"), fan);
        assert_eq!(list("").len(), 17, "`#` alone: every variable");
        assert!(list("xyz").is_empty());
        // camelCase words too (live batch 2: #fanControlBoardHeight for "fcbh" and "control"), sorted upper-cased.
        assert!(words_match("boardWallLength", "bwl") && !words_match("boardwalllength", "bwl"));
        let names = ["fan_control_board_depth", "fanControlBoardHeight"];
        let c = variable_completion("#control", 8, names).unwrap();
        assert_eq!(c.matches, ["fanControlBoardHeight", "fan_control_board_depth"]);
        assert_eq!(variable_completion("#fcbh", 5, names).unwrap().matches, ["fanControlBoardHeight"]);
        assert_eq!(variable_completion("#boardwall", 10, LIVE).unwrap().matches, variable_completion("#board_wall", 11, LIVE).unwrap().matches);
    }
}
