//! Note formatting (Onshape's note editor): font, color, underline and
//! strikethrough, line spacing and paragraph spacing, horizontal and
//! vertical alignment, lists (numbers, bullets, letters) with their order
//! and flag notes, a bounding box width (wrapping), fractions, and leaders.

use super::Leader;
use super::font;
use super::sheet::TextFont;
use serde::{Deserialize, Serialize};

/// List designators.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum NoteList {
    #[default]
    None,
    Numbers,
    Bullets,
    UpperLetters,
    LowerLetters,
}

impl NoteList {
    pub const ALL: [NoteList; 5] = [NoteList::None, NoteList::Numbers, NoteList::Bullets, NoteList::UpperLetters, NoteList::LowerLetters];

    pub fn name(self) -> &'static str {
        match self {
            NoteList::None => "None",
            NoteList::Numbers => "Numbers",
            NoteList::Bullets => "Bullets",
            NoteList::UpperLetters => "Uppercase letters",
            NoteList::LowerLetters => "Lowercase letters",
        }
    }

    /// The designator of item `n` (0-based).
    pub fn designator(self, n: usize) -> String {
        let letters = |upper: bool| {
            let mut s = String::new();
            let mut k = n;
            loop {
                let c = (b'a' + (k % 26) as u8) as char;
                s.insert(0, if upper { c.to_ascii_uppercase() } else { c });
                if k < 26 {
                    break;
                }
                k = k / 26 - 1;
            }
            format!("{s}.")
        };
        match self {
            NoteList::None => String::new(),
            NoteList::Numbers => format!("{}.", n + 1),
            NoteList::Bullets => "•".into(),
            NoteList::UpperLetters => letters(true),
            NoteList::LowerLetters => letters(false),
        }
    }
}

/// Horizontal alignment of a note's lines.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum NoteAlign {
    #[default]
    Left,
    Center,
    Right,
    Justified,
}

/// Where the note's position is: its top, middle or bottom.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum NoteVAlign {
    #[default]
    Top,
    Middle,
    Bottom,
}

/// Flag note border shapes (Drawing properties › Flag notes › Border
/// shape and the note editor's flag type).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum FlagShape {
    None,
    Circle,
    Diamond,
    DoubleCircle,
    Flag,
    Hexagon,
    Octagon,
    #[serde(alias = "Square")]
    Rectangle,
    SquareCircle,
    #[default]
    Triangle,
    Underline,
}

impl FlagShape {
    /// Onshape's list order.
    pub const ALL: [FlagShape; 11] = [
        FlagShape::None,
        FlagShape::Circle,
        FlagShape::Diamond,
        FlagShape::DoubleCircle,
        FlagShape::Flag,
        FlagShape::Hexagon,
        FlagShape::Octagon,
        FlagShape::Rectangle,
        FlagShape::SquareCircle,
        FlagShape::Triangle,
        FlagShape::Underline,
    ];

    pub fn name(self) -> &'static str {
        match self {
            FlagShape::None => "None",
            FlagShape::Circle => "Circle",
            FlagShape::Diamond => "Diamond",
            FlagShape::DoubleCircle => "Double Circle",
            FlagShape::Flag => "Flag",
            FlagShape::Hexagon => "Hexagon",
            FlagShape::Octagon => "Octagon",
            FlagShape::Rectangle => "Rectangle",
            FlagShape::SquareCircle => "Square Circle",
            FlagShape::Triangle => "Triangle",
            FlagShape::Underline => "Underline",
        }
    }
}

/// Tab stop alignment (the ruler's selector cycles through them).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TabKind {
    #[default]
    Left,
    Center,
    Right,
    Decimal,
}

impl TabKind {
    pub const ALL: [TabKind; 4] = [TabKind::Left, TabKind::Center, TabKind::Right, TabKind::Decimal];

    pub fn name(self) -> &'static str {
        match self {
            TabKind::Left => "Left",
            TabKind::Center => "Center",
            TabKind::Right => "Right",
            TabKind::Decimal => "Decimal",
        }
    }

    pub fn next(self) -> Self {
        Self::ALL[(Self::ALL.iter().position(|k| *k == self).unwrap_or(0) + 1) % Self::ALL.len()]
    }
}

/// A tab stop: position from the box's left (mm) and alignment.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct TabStop {
    pub pos: f64,
    pub kind: TabKind,
}

/// Spacing of the predefined (left) tab stops when none is set (mm).
pub const DEFAULT_TAB: f64 = 12.7;

/// A note's formatting (all defaults = a plain note).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct NoteStyle {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font: Option<TextFont>,
    pub underline: bool,
    pub strikethrough: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<[u8; 3]>,
    /// 1.0, 1.5, 2.0, 2.5 or 3.0.
    pub line_spacing: f64,
    pub space_before: bool,
    pub space_after: bool,
    pub align: NoteAlign,
    pub valign: NoteVAlign,
    pub list: NoteList,
    /// Note order "Bottom to top": list items count up from the bottom.
    pub bottom_to_top: bool,
    /// Bounding box width (mm): lines wrap at it. None: as wide as the text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<f64>,
    /// Leaders from the text box to the geometry.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub leaders: Vec<Leader>,
    /// Flag notes: flagged list items (0-based), the flag shape and size in
    /// characters.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub flags: Vec<usize>,
    pub flag_shape: FlagShape,
    pub flag_chars: u8,
    /// Explode: exported as polylines.
    pub explode: bool,
    /// Formatting ruler: tab stops, and the first-line / left / right
    /// indents (mm from the box's sides).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tabs: Vec<TabStop>,
    pub first_indent: f64,
    pub left_indent: f64,
    pub right_indent: f64,
    /// Rotation (radians, counter-clockwise) about the note's position;
    /// notes with leaders don't rotate.
    pub angle: f64,
}

impl Default for NoteStyle {
    fn default() -> Self {
        Self {
            font: None,
            underline: false,
            strikethrough: false,
            color: None,
            line_spacing: 1.0,
            space_before: false,
            space_after: false,
            align: NoteAlign::Left,
            valign: NoteVAlign::Top,
            list: NoteList::None,
            bottom_to_top: false,
            width: None,
            leaders: Vec::new(),
            flags: Vec::new(),
            flag_shape: FlagShape::Triangle,
            flag_chars: 2,
            explode: false,
            angle: 0.0,
            tabs: Vec::new(),
            first_indent: 0.0,
            left_indent: 0.0,
            right_indent: 0.0,
        }
    }
}

impl NoteStyle {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// One laid-out piece of a note: text with its top-left corner.
#[derive(Clone, Debug, PartialEq)]
pub struct NotePiece {
    pub text: String,
    pub pos: [f64; 2],
    /// A list designator (not underlined, struck or justified).
    pub designator: bool,
    /// The item index for flag notes (designators only).
    pub item: Option<usize>,
}

/// A laid-out note: its pieces and box [x0, y0, x1, y1] (y up).
#[derive(Clone, Debug, PartialEq)]
pub struct NoteLayout {
    pub pieces: Vec<NotePiece>,
    pub bbox: [f64; 4],
    /// Each line's (left x, baseline y, width) for underline / strikethrough.
    pub runs: Vec<([f64; 2], f64)>,
}

/// The middle of a note's box (it rotates about it).
pub fn center(bbox: &[f64; 4]) -> [f64; 2] {
    [(bbox[0] + bbox[2]) / 2.0, (bbox[1] + bbox[3]) / 2.0]
}

/// Note fractions: "3/4 " (horizontal bar), "3#4 " (diagonal) and "3^4 "
/// (stacked) become the fraction characters the renderer draws.
pub fn fractions(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        // digits, a fraction mark, digits, then a space
        let start = i;
        let mut j = i;
        while j < chars.len() && chars[j].is_ascii_digit() {
            j += 1;
        }
        if j > start && j < chars.len() && matches!(chars[j], '/' | '#' | '^') {
            let mark = chars[j];
            let mut k = j + 1;
            while k < chars.len() && chars[k].is_ascii_digit() {
                k += 1;
            }
            if k > j + 1 && k < chars.len() && chars[k] == ' ' && (start == 0 || !chars[start - 1].is_ascii_alphanumeric()) {
                let num: String = chars[start..j].iter().collect();
                let den: String = chars[j + 1..k].iter().collect();
                // Markers the renderer turns into small numerals: stacked
                // (with or without a bar), or diagonal around a slash line
                // (the drawing fonts have no ⁄; qa DWG-NOTE-DIAG-FRACTION).
                match mark {
                    '#' => out.push_str(&format!("\u{E004}{num}\u{E001}{den}\u{E002}")),
                    '/' => out.push_str(&format!("\u{E000}{num}\u{E001}{den}\u{E002}")),
                    _ => out.push_str(&format!("\u{E003}{num}\u{E001}{den}\u{E002}")),
                }
                // The space that triggered it stays.
                i = k;
                continue;
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

/// A run of note text: plain, a stacked fraction (with or without a bar), or
/// a diagonal one (help A13: `n#n`).
#[derive(Clone, Debug, PartialEq)]
pub enum Seg {
    Text(String),
    Fraction { num: String, den: String, bar: bool },
    Diagonal { num: String, den: String },
}

/// Split text into plain runs and stacked fractions (see [`fractions`]).
pub fn segments(s: &str) -> Vec<Seg> {
    let mut out = Vec::new();
    let mut rest = s;
    while let Some(i) = rest.find(['\u{E000}', '\u{E003}', '\u{E004}']) {
        if i > 0 {
            out.push(Seg::Text(rest[..i].to_string()));
        }
        let bar = rest[i..].starts_with('\u{E000}');
        let diagonal = rest[i..].starts_with('\u{E004}');
        let body = &rest[i + '\u{E000}'.len_utf8()..];
        let (Some(m), Some(e)) = (body.find('\u{E001}'), body.find('\u{E002}')) else {
            out.push(Seg::Text(body.to_string()));
            return out;
        };
        let (num, den) = (body[..m].to_string(), body[m + '\u{E001}'.len_utf8()..e].to_string());
        out.push(if diagonal { Seg::Diagonal { num, den } } else { Seg::Fraction { num, den, bar } });
        rest = &body[e + '\u{E002}'.len_utf8()..];
    }
    if !rest.is_empty() {
        out.push(Seg::Text(rest.to_string()));
    }
    out
}

/// Size of a stacked fraction's digits (of the text height).
pub const FRACTION_SCALE: f64 = 0.6;

/// Width of a diagonal fraction: the numerator, the slash, the denominator.
pub fn diagonal_width(num: &str, den: &str, h: f64) -> f64 {
    font::text_width(num, h * FRACTION_SCALE) + h * 0.3 + font::text_width(den, h * FRACTION_SCALE) + h * 0.1
}

/// Width of note text, stacked fractions included.
pub fn text_width(s: &str, h: f64) -> f64 {
    segments(s)
        .iter()
        .map(|g| match g {
            Seg::Text(t) => font::text_width(t, h),
            Seg::Fraction { num, den, .. } => font::text_width(num, h * FRACTION_SCALE).max(font::text_width(den, h * FRACTION_SCALE)) + h * 0.2,
            Seg::Diagonal { num, den } => diagonal_width(num, den, h),
        })
        .sum()
}

/// Wrap a line to `width` (mm) at spaces.
fn wrap(line: &str, h: f64, width: f64) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for word in line.split(' ') {
        let cand = if cur.is_empty() { word.to_string() } else { format!("{cur} {word}") };
        if !cur.is_empty() && text_width(&cand, h) > width {
            out.push(std::mem::take(&mut cur));
            cur = word.to_string();
        } else {
            cur = cand;
        }
    }
    out.push(cur);
    out
}

/// Lay out a note's (expanded) text at `pos` with text height `h`.
pub fn layout(text: &str, h: f64, style: &NoteStyle, pos: [f64; 2]) -> NoteLayout {
    let width_of = |s: &str| text_width(s, h);
    let paragraphs: Vec<&str> = text.split('\n').collect();
    // List items: non-empty paragraphs; an empty one restarts numbering.
    let mut items: Vec<Option<usize>> = Vec::new();
    let mut n = 0;
    for p in &paragraphs {
        if p.trim().is_empty() {
            items.push(None);
            n = 0;
        } else {
            items.push(Some(n));
            n += 1;
        }
    }
    if style.bottom_to_top {
        // Count each run of items from its bottom.
        let mut k = 0;
        while k < items.len() {
            let mut e = k;
            while e < items.len() && items[e].is_some() {
                e += 1;
            }
            let len = e - k;
            for (o, it) in items[k..e].iter_mut().enumerate() {
                *it = Some(len - 1 - o);
            }
            k = e.max(k + 1);
        }
    }
    let list = style.list != NoteList::None;
    let indent = if list { paragraphs.iter().zip(&items).filter_map(|(_, it)| it.map(|i| width_of(&style.list.designator(i)))).fold(0.0, f64::max) + h * 0.8 } else { 0.0 };
    let wrap_w = style.width.map(|w| (w - indent - style.first_indent.max(style.left_indent) - style.right_indent).max(h));
    // Lines: (text, designator item, last line of its paragraph, paragraph start).
    let mut lines: Vec<(String, Option<usize>, bool, bool)> = Vec::new();
    for (p, it) in paragraphs.iter().zip(&items) {
        let parts = match wrap_w {
            Some(w) => wrap(p, h, w),
            None => vec![p.to_string()],
        };
        let k = parts.len();
        for (i, l) in parts.into_iter().enumerate() {
            lines.push((l, if i == 0 { *it } else { None }, i + 1 == k, i == 0));
        }
    }
    let text_w = lines.iter().map(|l| width_of(&l.0)).fold(0.0, f64::max);
    let box_w = style.width.unwrap_or(indent + text_w);
    let pitch = h * 1.5 * style.line_spacing.max(0.5);
    let para_gap = h * 0.5;
    // Total height (first line: its cap height).
    let mut heights = Vec::new();
    let mut y = 0.0;
    for (i, l) in lines.iter().enumerate() {
        if l.3 && i > 0 {
            if style.space_before {
                y += para_gap;
            }
            if style.space_after {
                y += para_gap;
            }
        }
        heights.push(y);
        if i + 1 < lines.len() {
            y += pitch;
        }
    }
    let total = y + h;
    let top = match style.valign {
        NoteVAlign::Top => pos[1],
        NoteVAlign::Middle => pos[1] + total / 2.0,
        NoteVAlign::Bottom => pos[1] + total,
    };
    let mut pieces = Vec::new();
    let mut runs = Vec::new();
    for ((l, item, last, first), dy) in lines.iter().zip(heights) {
        let ly = top - dy;
        if let Some(i) = item.filter(|_| list) {
            pieces.push(NotePiece { text: style.list.designator(i), pos: [pos[0], ly], designator: true, item: Some(i) });
        }
        let lw = width_of(l);
        let ind = if *first { style.first_indent } else { style.left_indent };
        let room = box_w - indent - ind - style.right_indent;
        let x0 = pos[0] + indent + ind;
        // Tabs: each segment goes to the next stop past the text so far.
        if l.contains('\t') {
            let mut x = x0;
            for (k, seg) in l.split('\t').enumerate() {
                if k > 0 {
                    let rel = x - pos[0];
                    let stop = style.tabs.iter().filter(|t| t.pos > rel + 1e-9).min_by(|a, b| a.pos.total_cmp(&b.pos)).copied().unwrap_or(TabStop { pos: ((rel / DEFAULT_TAB).floor() + 1.0) * DEFAULT_TAB, kind: TabKind::Left });
                    let sw = width_of(seg);
                    let at = pos[0] + stop.pos;
                    x = match stop.kind {
                        TabKind::Left => at,
                        TabKind::Center => at - sw / 2.0,
                        TabKind::Right => at - sw,
                        TabKind::Decimal => at - seg.find(['.', ',', ' ']).map(|i| width_of(&seg[..i])).unwrap_or(sw),
                    }
                    .max(x);
                }
                if !seg.is_empty() {
                    pieces.push(NotePiece { text: seg.to_string(), pos: [x, ly], designator: false, item: None });
                }
                x += width_of(seg);
            }
            runs.push(([x0, ly - h], x - x0));
            continue;
        }
        match style.align {
            NoteAlign::Justified if !*last && l.contains(' ') && style.width.is_some() => {
                let words: Vec<&str> = l.split(' ').filter(|w| !w.is_empty()).collect();
                let ww: f64 = words.iter().map(|w| width_of(w)).sum();
                let gap = (room - ww) / (words.len() - 1).max(1) as f64;
                let mut x = x0;
                for w in words {
                    pieces.push(NotePiece { text: w.to_string(), pos: [x, ly], designator: false, item: None });
                    x += width_of(w) + gap;
                }
                runs.push(([x0, ly - h], room));
            }
            align => {
                let k = match align {
                    NoteAlign::Center => 0.5,
                    NoteAlign::Right => 1.0,
                    _ => 0.0,
                };
                let x = x0 + (room - lw).max(0.0) * k;
                pieces.push(NotePiece { text: l.clone(), pos: [x, ly], designator: false, item: None });
                runs.push(([x, ly - h], lw));
            }
        }
    }
    NoteLayout { pieces, bbox: [pos[0], top - total, pos[0] + box_w, top], runs }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_alignment_and_fractions() {
        let s = NoteStyle { list: NoteList::Numbers, ..Default::default() };
        let l = layout("FIRST\nSECOND\n\nTHIRD", 3.5, &s, [10.0, 100.0]);
        let des: Vec<&str> = l.pieces.iter().filter(|p| p.designator).map(|p| p.text.as_str()).collect();
        assert_eq!(des, ["1.", "2.", "1."], "a gap restarts numbering");
        let first = l.pieces.iter().find(|p| p.text == "FIRST").unwrap();
        assert!(first.pos[0] > 10.0, "text indented, designator not");
        let s = NoteStyle { list: NoteList::UpperLetters, bottom_to_top: true, ..Default::default() };
        let l = layout("A\nB\nC", 3.5, &s, [0.0, 0.0]);
        assert_eq!(l.pieces.iter().filter(|p| p.designator).map(|p| p.text.as_str()).collect::<Vec<_>>(), ["C.", "B.", "A."]);
        // Right alignment in a fixed-width box; Middle centers on the position.
        let s = NoteStyle { align: NoteAlign::Right, valign: NoteVAlign::Middle, width: Some(60.0), ..Default::default() };
        let l = layout("AB", 3.5, &s, [0.0, 50.0]);
        let p = &l.pieces[0];
        assert!((p.pos[0] + font::text_width("AB", 3.5) - 60.0).abs() < 1e-9);
        assert!(((l.bbox[1] + l.bbox[3]) / 2.0 - 50.0).abs() < 1e-9);
        // Wrapping and line spacing.
        let s = NoteStyle { width: Some(20.0), line_spacing: 2.0, ..Default::default() };
        let l = layout("WORD WORD WORD WORD", 3.5, &s, [0.0, 0.0]);
        assert!(l.pieces.len() >= 2 && (l.pieces[0].pos[1] - l.pieces[1].pos[1] - 3.5 * 3.0).abs() < 1e-9);
        assert_eq!(fractions("1/2 IN"), "\u{E000}1\u{E001}2\u{E002} IN");
        assert_eq!(fractions("3#4 X"), "\u{E004}3\u{E001}4\u{E002} X");
        assert_eq!(segments(&fractions("3#4 X")), [Seg::Diagonal { num: "3".into(), den: "4".into() }, Seg::Text(" X".into())]);
        assert_eq!(fractions("A1/2 "), "A1/2 ", "not after a letter");
    }

    #[test]
    fn tab_stops_and_indents() {
        let h = 3.5;
        let pos = [10.0, 100.0];
        // Default stops every DEFAULT_TAB from the box's left.
        let l = layout("A\tB", h, &NoteStyle::default(), pos);
        let b = l.pieces.iter().find(|p| p.text == "B").unwrap();
        assert!((b.pos[0] - (pos[0] + DEFAULT_TAB)).abs() < 1e-9);
        // A right stop puts the segment's end on it; a decimal stop its point.
        let style = NoteStyle { tabs: vec![TabStop { pos: 40.0, kind: TabKind::Right }, TabStop { pos: 60.0, kind: TabKind::Decimal }], ..Default::default() };
        let l = layout("A\tRR\t12.5", h, &style, pos);
        let rr = l.pieces.iter().find(|p| p.text == "RR").unwrap();
        assert!((rr.pos[0] + text_width("RR", h) - (pos[0] + 40.0)).abs() < 1e-9);
        let d = l.pieces.iter().find(|p| p.text == "12.5").unwrap();
        assert!((d.pos[0] + text_width("12", h) - (pos[0] + 60.0)).abs() < 1e-9);
        // First-line and left indents.
        let style = NoteStyle { first_indent: 5.0, left_indent: 2.0, width: Some(30.0), ..Default::default() };
        let l = layout("WORD WORD WORD WORD WORD WORD", h, &style, pos);
        let xs: Vec<f64> = l.pieces.iter().map(|p| p.pos[0]).collect();
        assert!((xs[0] - 15.0).abs() < 1e-9 && xs.len() > 1 && (xs[1] - 12.0).abs() < 1e-9, "{xs:?}");
    }
}
