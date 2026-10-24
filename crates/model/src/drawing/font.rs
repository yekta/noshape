//! Single-stroke vector font for drawing text (in the spirit of ISO 3098
//! type B lettering). Used by the drawing editor to render text as line
//! segments on the GPU (any rotation, any zoom, identical to the sheet
//! geometry) and to measure text extents for hit testing and layout.
//!
//! Glyphs are designed on a grid with cap height 10, x-height 7 and
//! descenders down to -3. A glyph is a list of strokes separated by `;`;
//! each stroke is a sequence of points `x,y` and elliptical arcs
//! `@cx,cy,rx,ry,a0,a1` (degrees, sampled from a0 to a1).

use super::sheet::{Anchor, Text2};
use std::collections::HashMap;
use std::sync::OnceLock;

/// Units of the glyph grid per cap height.
const CAP: f64 = 10.0;
/// Gap between glyphs (grid units).
const SPACING: f64 = 2.0;

struct Glyph {
    advance: f64,
    strokes: Vec<Vec<[f64; 2]>>,
}

const GLYPHS: &[(char, f64, &str)] = &[
    (' ', 3.0, ""),
    ('0', 6.0, "@3,5,3,5,0,360"),
    ('1', 6.0, "1,7.8 3.5,10 3.5,0"),
    ('2', 6.0, "@3,7.2,2.9,2.8,155,-35 0,0 6,0"),
    ('3', 6.0, "@3,7.6,2.7,2.4,150,-90 @3,2.6,3,2.6,90,-150"),
    ('4', 6.0, "4.5,0 4.5,10 0,3 6,3"),
    ('5', 6.0, "5.5,10 0.9,10 0.6,5.2 @3,3.2,3,3.2,140,-150"),
    ('6', 6.0, "@3,3.1,3,6.9,65,180 @3,3.1,3,3.1,180,540"),
    ('7', 6.0, "0,10 6,10 2,0"),
    ('8', 6.0, "@3,7.6,2.6,2.4,0,360;@3,2.6,3,2.6,0,360"),
    ('9', 6.0, "@3,6.9,3,3.1,0,360;@3,6.9,3,6.9,0,-115"),
    ('A', 6.0, "0,0 3,10 6,0;1,3.3 5,3.3"),
    ('B', 6.0, "0,0 0,10 3.6,10 @3.6,7.6,2.2,2.4,90,-90 0,5.2 4,5.2 @4,2.6,2,2.6,90,-90 0,0"),
    ('C', 6.2, "@3.3,5,3.3,5,50,310"),
    ('D', 6.0, "0,0 0,10 2.5,10 @2.5,5,3.5,5,90,-90 0,0"),
    ('E', 5.5, "5.5,10 0,10 0,0 5.5,0;0,5.2 4.5,5.2"),
    ('F', 5.5, "5.5,10 0,10 0,0;0,5.2 4.5,5.2"),
    ('G', 6.4, "@3.2,5,3.2,5,50,355 6.4,4.5 3.8,4.5"),
    ('H', 6.0, "0,0 0,10;6,0 6,10;0,5.2 6,5.2"),
    ('I', 0.0, "0,0 0,10"),
    ('J', 4.5, "4.5,10 4.5,3 @2.25,3,2.25,3,0,-165"),
    ('K', 6.0, "0,0 0,10;6,10 0,3.5;1.8,5.4 6,0"),
    ('L', 5.5, "0,10 0,0 5.5,0"),
    ('M', 7.0, "0,0 0,10 3.5,2 7,10 7,0"),
    ('N', 6.0, "0,0 0,10 6,0 6,10"),
    ('O', 6.6, "@3.3,5,3.3,5,0,360"),
    ('P', 6.0, "0,0 0,10 3.6,10 @3.6,7.4,2.4,2.6,90,-90 0,4.8"),
    ('Q', 6.6, "@3.3,5,3.3,5,0,360;3.8,2.5 6.6,-0.5"),
    ('R', 6.0, "0,0 0,10 3.6,10 @3.6,7.4,2.4,2.6,90,-90 0,4.8;3.2,4.8 6,0"),
    ('S', 6.0, "@3,7.5,2.8,2.5,30,270 @3,2.5,3,2.5,90,-150"),
    ('T', 6.0, "0,10 6,10;3,10 3,0"),
    ('U', 6.0, "0,10 0,3 @3,3,3,3,180,360 6,10"),
    ('V', 6.0, "0,10 3,0 6,10"),
    ('W', 8.0, "0,10 2,0 4,8 6,0 8,10"),
    ('X', 6.0, "0,0 6,10;0,10 6,0"),
    ('Y', 6.0, "0,10 3,5 6,10;3,5 3,0"),
    ('Z', 6.0, "0,10 6,10 0,0 6,0"),
    ('a', 5.0, "@2.5,3.5,2.5,3.5,0,360;5,7 5,0"),
    ('b', 5.0, "0,10 0,0;@2.5,3.5,2.5,3.5,0,360"),
    ('c', 4.8, "@2.6,3.5,2.6,3.5,45,315"),
    ('d', 5.0, "5,10 5,0;@2.5,3.5,2.5,3.5,0,360"),
    ('e', 5.0, "0,3.5 5,3.5 @2.5,3.5,2.5,3.5,0,315"),
    ('f', 3.6, "@2.8,8.2,1.2,1.8,45,180 1.6,0;0,7 3.6,7"),
    ('g', 5.0, "@2.5,3.5,2.5,3.5,0,360;5,7 5,-1 @2.5,-1,2.5,2,0,-160"),
    ('h', 5.0, "0,10 0,0;0,4.5 @2.5,4.5,2.5,2.5,180,0 5,0"),
    ('i', 0.0, "0,0 0,7;0,9 0,9.6"),
    ('j', 1.5, "1.5,7 1.5,-1.5 @0,-1.5,1.5,1.5,0,-150;1.5,9 1.5,9.6"),
    ('k', 4.8, "0,10 0,0;4.5,7 0,2.5;1.6,4 4.8,0"),
    ('l', 0.0, "0,10 0,0"),
    ('m', 8.0, "0,0 0,7;0,5 @2,5,2,2,180,0 4,0;4,5 @6,5,2,2,180,0 8,0"),
    ('n', 5.0, "0,0 0,7;0,4.5 @2.5,4.5,2.5,2.5,180,0 5,0"),
    ('o', 5.0, "@2.5,3.5,2.5,3.5,0,360"),
    ('p', 5.0, "0,7 0,-3;@2.5,3.5,2.5,3.5,0,360"),
    ('q', 5.0, "5,7 5,-3;@2.5,3.5,2.5,3.5,0,360"),
    ('r', 4.5, "0,0 0,7;0,4 @3,4.2,3,2.8,180,60"),
    ('s', 4.6, "@2.3,5.3,2.1,1.7,20,270 @2.3,1.8,2.3,1.8,90,-160"),
    ('t', 3.6, "1.6,9.5 1.6,1.2 @2.8,1.2,1.2,1.2,180,300;0,7 3.6,7"),
    ('u', 5.0, "0,7 0,2.5 @2.5,2.5,2.5,2.5,180,360;5,7 5,0"),
    ('v', 5.0, "0,7 2.5,0 5,7"),
    ('w', 7.0, "0,7 1.75,0 3.5,6 5.25,0 7,7"),
    ('x', 5.0, "0,0 5,7;0,7 5,0"),
    ('y', 5.0, "0,7 2.6,0.5;5,7 1.6,-3 0.4,-3"),
    ('z', 5.0, "0,7 5,7 0,0 5,0"),
    ('.', 0.0, "0,0 0,0.5"),
    (',', 0.4, "0.4,0.6 0,-1.5"),
    (':', 0.0, "0,0 0,0.5;0,6 0,6.5"),
    (';', 0.4, "0.4,6 0.4,6.5;0.4,0.5 0,-1.5"),
    ('-', 4.0, "0,4 4,4"),
    ('+', 6.0, "0,4 6,4;3,1 3,7"),
    ('=', 6.0, "0,3 6,3;0,5.4 6,5.4"),
    ('/', 5.0, "0,-0.5 5,10.5"),
    ('\\', 5.0, "0,10.5 5,-0.5"),
    ('(', 3.0, "@4,4.5,3.5,6.3,113,247"),
    (')', 3.0, "@-1,4.5,3.5,6.3,67,-67"),
    ('[', 3.0, "3,10.5 0,10.5 0,-1.5 3,-1.5"),
    (']', 3.0, "0,10.5 3,10.5 3,-1.5 0,-1.5"),
    ('{', 3.0, "3,10.5 2,10.5 1.5,10 1.5,5.4 0,4.5 1.5,3.6 1.5,-1 2,-1.5 3,-1.5"),
    ('}', 3.0, "0,10.5 1,10.5 1.5,10 1.5,5.4 3,4.5 1.5,3.6 1.5,-1 1,-1.5 0,-1.5"),
    ('<', 6.0, "6,8 0,4 6,0"),
    ('>', 6.0, "0,8 6,4 0,0"),
    ('%', 6.0, "0,0 6,10;@1.2,8.5,1.2,1.5,0,360;@4.8,1.5,1.2,1.5,0,360"),
    ('#', 6.0, "1.5,0 2.5,10;4,0 5,10;0,3.2 6,3.2;0.2,6.8 6.2,6.8"),
    ('*', 6.0, "3,10 3,4;0.4,8.5 5.6,5.5;0.4,5.5 5.6,8.5"),
    ('!', 0.0, "0,10 0,3;0,0 0,0.5"),
    ('?', 5.0, "@2.5,7.5,2.5,2.5,160,-90 2.5,3;2.5,0 2.5,0.5"),
    ('"', 2.0, "0,10 0,7.5;2,10 2,7.5"),
    ('\'', 0.0, "0,10 0,7.5"),
    ('`', 1.0, "0,10 1,8.5"),
    ('^', 5.0, "0,7 2.5,10 5,7"),
    ('_', 6.0, "0,-1.5 6,-1.5"),
    ('~', 6.0, "@1.5,4.5,1.5,1,180,0 @4.5,4.5,1.5,1,180,360"),
    ('|', 0.0, "0,-1.5 0,10.5"),
    ('&', 6.0, "6,0 1,6.5 1,8.5 2,10 3.5,10 4,8.5 3.5,7 0,3.5 0,1 1,0 3,0 6,4"),
    ('@', 7.0, "4.8,6.5 4.8,3 @5.9,3,1.1,1,180,360 7,5 @3.5,5,3.5,5,0,320;@3.4,4.9,1.4,1.7,0,360"),
    ('$', 6.0, "@3,7.5,2.8,2.5,30,270 @3,2.5,3,2.5,90,-150;3,11 3,-1"),
    // Dimension and GD&T symbols (ASME Y14.5 / ISO 1101).
    ('⌒', 7.0, "@3.5,3,3.5,4,180,0"),
    ('⏤', 7.0, "0,4.5 7,4.5"),
    ('▱', 7.5, "1.5,7 7.5,7 6,2 0,2 1.5,7"),
    ('○', 7.0, "@3.5,4.5,3.5,3.5,0,360"),
    ('⌭', 7.0, "@3.5,4.5,2.6,2.6,0,360;0.2,0 2.6,9;4.4,0 6.8,9"),
    ('⌓', 7.0, "@3.5,2,3.5,4,0,180 7,2"),
    ('∠', 7.0, "7,7 0,1 7,1"),
    ('⟂', 7.0, "3.5,8 3.5,1;0,1 7,1"),
    ('⊥', 7.0, "3.5,8 3.5,1;0,1 7,1"),
    ('∥', 6.0, "1.5,1 4,8;3.5,1 6,8"),
    ('⌖', 7.0, "@3.5,4.5,2.5,2.5,0,360;3.5,0.5 3.5,8.5;-0.5,4.5 7.5,4.5"),
    ('◎', 7.0, "@3.5,4.5,3.5,3.5,0,360;@3.5,4.5,1.8,1.8,0,360"),
    ('⌯', 7.0, "0,4.5 7,4.5;1.5,6.5 5.5,6.5;1.5,2.5 5.5,2.5"),
    ('↗', 6.0, "0,1 6,7;3,7 6,7 6,4"),
    ('⌰', 7.5, "0,1 5,6;2.5,6 5,6 5,3.5;2.5,1 7.5,6;5,6 7.5,6 7.5,3.5;0,1 2.5,1"),
    ('Ⓜ', 8.0, "@4,4.5,4,4,0,360;2.1,2 2.1,7 4,3.8 5.9,7 5.9,2"),
    ('Ⓛ', 8.0, "@4,4.5,4,4,0,360;2.6,7 2.6,2 5.6,2"),
    ('Ⓢ', 8.0, "@4,4.5,4,4,0,360;@4,5.7,1.4,1.2,30,270 @4,3.3,1.4,1.2,90,-150"),
    ('Ⓟ', 8.0, "@4,4.5,4,4,0,360;2.6,2 2.6,7 4.4,7 @4.4,5.9,1.1,1.1,90,-90 2.6,4.8"),
    ('Ⓕ', 8.0, "@4,4.5,4,4,0,360;5.6,7 2.6,7 2.6,2;2.6,4.6 5,4.6"),
    ('Ⓣ', 8.0, "@4,4.5,4,4,0,360;2.2,7 5.8,7;4,7 4,2"),
    ('⌴', 7.0, "0,8 0,2 7,2 7,8"),
    ('⌵', 7.0, "0,7 3.5,1 7,7"),
    ('↧', 7.0, "3.5,9 3.5,1;1.5,3.5 3.5,1 5.5,3.5;0.5,9 6.5,9"),
    ('□', 7.0, "0,1 7,1 7,8 0,8 0,1"),
    ('℄', 7.0, "@3.5,5,3.5,5,50,310;3,-0.5 3,10.5;3,0 7,0"),
    ('√', 7.0, "0,5 2,1 7,11"),
    ('°', 2.4, "@1.2,8.8,1.2,1.2,0,360"),
    ('Ø', 6.6, "@3.3,5,3.3,5,0,360;0,-0.5 6.6,10.5"),
    ('⌀', 6.6, "@3.3,5,3.3,5,0,360;0,-0.5 6.6,10.5"),
    ('±', 6.0, "0,5 6,5;3,2 3,8;0,0 6,0"),
    ('×', 5.0, "0,1 5,7;0,7 5,1"),
    ('µ', 5.0, "0,-3 0,7;0,2.5 @2.5,2.5,2.5,2.5,180,360;5,7 5,0"),
    ('²', 3.6, "@1.8,8.6,1.7,1.4,155,-35 0,5.5 3.6,5.5"),
    ('³', 3.6, "@1.8,9,1.6,1,150,-90 @1.8,6.8,1.8,1.2,90,-150"),
];

fn parse_stroke(s: &str) -> Vec<[f64; 2]> {
    let mut pts: Vec<[f64; 2]> = Vec::new();
    for tok in s.split_whitespace() {
        if let Some(arc) = tok.strip_prefix('@') {
            let v: Vec<f64> = arc.split(',').filter_map(|x| x.parse().ok()).collect();
            if v.len() != 6 {
                continue;
            }
            let (cx, cy, rx, ry, a0, a1) = (v[0], v[1], v[2], v[3], v[4].to_radians(), v[5].to_radians());
            let n = (((a1 - a0).abs() / 15f64.to_radians()).ceil() as usize).max(2);
            for i in 0..=n {
                let t = a0 + (a1 - a0) * i as f64 / n as f64;
                let p = [cx + rx * t.cos(), cy + ry * t.sin()];
                if pts.last().is_none_or(|q| (q[0] - p[0]).abs() + (q[1] - p[1]).abs() > 1e-9) {
                    pts.push(p);
                }
            }
        } else {
            let v: Vec<f64> = tok.split(',').filter_map(|x| x.parse().ok()).collect();
            if v.len() == 2 {
                pts.push([v[0], v[1]]);
            }
        }
    }
    pts
}

fn glyphs() -> &'static HashMap<char, Glyph> {
    static G: OnceLock<HashMap<char, Glyph>> = OnceLock::new();
    G.get_or_init(|| {
        GLYPHS
            .iter()
            .map(|(c, w, def)| {
                let strokes = def.split(';').map(parse_stroke).filter(|s| !s.is_empty()).collect();
                (*c, Glyph { advance: w + SPACING, strokes })
            })
            .collect()
    })
}

fn fallback() -> &'static Glyph {
    static G: OnceLock<Glyph> = OnceLock::new();
    G.get_or_init(|| Glyph { advance: 5.0 + SPACING, strokes: vec![vec![[0.0, 0.0], [5.0, 0.0], [5.0, 10.0], [0.0, 10.0], [0.0, 0.0]]] })
}

fn glyph(c: char) -> &'static Glyph {
    let g = glyphs();
    g.get(&c).or_else(|| c.to_uppercase().next().and_then(|u| g.get(&u))).unwrap_or_else(fallback)
}

/// The drawing's default font (Onshape: "Noto Sans ( Note )").
pub const DEFAULT_FAMILY: &str = "Noto Sans";

/// Advance of a character per cap height: Noto Sans' advance for the
/// characters it draws ([`fill_supported`]), the stroke glyph's otherwise.
fn char_width(c: char) -> f64 {
    use std::sync::Mutex;
    static CACHE: OnceLock<Mutex<HashMap<char, f64>>> = OnceLock::new();
    if let Some(v) = CACHE.get_or_init(Default::default).lock().ok().and_then(|m| m.get(&c).copied()) {
        return v;
    }
    let mut buf = [0u8; 4];
    let v = if fill_supported(c.encode_utf8(&mut buf)) {
        noshape_sketch::text::layout_drawing_text(c.encode_utf8(&mut buf), DEFAULT_FAMILY, false, false).width / fill_cap(DEFAULT_FAMILY, false, false)
    } else {
        glyph(c).advance / CAP
    };
    if let Ok(mut m) = CACHE.get_or_init(Default::default).lock() {
        m.insert(c, v);
    }
    v
}

/// Width of a single line of text of cap height `height` (Noto Sans
/// advances, the text as Onshape lays it out).
pub fn text_width(text: &str, height: f64) -> f64 {
    text.chars().map(char_width).sum::<f64>() * height
}

/// Offset of the text origin (left end of the baseline) from the anchor
/// point, in the text's local frame.
fn anchor_offset(anchor: Anchor, width: f64, height: f64) -> [f64; 2] {
    match anchor {
        Anchor::BottomLeft => [0.0, 0.0],
        Anchor::BottomCenter => [-width / 2.0, 0.0],
        Anchor::Center => [-width / 2.0, -height / 2.0],
        Anchor::TopLeft => [0.0, -height],
    }
}

/// Text outline as polylines in sheet coordinates.
pub fn text_strokes(t: &Text2) -> Vec<Vec<[f64; 2]>> {
    let s = t.height / CAP;
    let w = text_width(&t.text, t.height);
    let off = anchor_offset(t.anchor, w, t.height);
    let (c, sn) = (t.angle.cos(), t.angle.sin());
    let mut out = Vec::new();
    let mut x = 0.0;
    for ch in t.text.chars() {
        let g = glyph(ch);
        // Each stroke glyph sits centred in the character's slot of the layout.
        let slot = char_width(ch) * CAP;
        let gx = x + (slot - (g.advance - SPACING)) / 2.0;
        for stroke in &g.strokes {
            let pts: Vec<[f64; 2]> = stroke
                .iter()
                .map(|p| {
                    let lx = off[0] + (gx + p[0]) * s;
                    let ly = off[1] + p[1] * s;
                    [t.pos[0] + lx * c - ly * sn, t.pos[1] + lx * sn + ly * c]
                })
                .collect();
            if pts.len() == 1 {
                out.push(vec![pts[0], [pts[0][0] + 1e-3 * s, pts[0][1]]]);
            } else {
                out.push(pts);
            }
        }
        x += slot;
    }
    out
}

/// Liberation Sans cap height over its ascender (the unit of
/// `noshape_sketch::text::layout_text`).
const FILL_CAP: f64 = 1409.0 / 1854.0;

/// Whether [`text_fill`] can draw every character (Latin and Greek; GD&T
/// and other symbols only exist in the stroke font).
pub fn fill_supported(text: &str) -> bool {
    text.chars().all(|c| (c as u32) < 0x250 || ('\u{370}'..='\u{3ff}').contains(&c) || matches!(c, '\u{2212}' | '\u{2013}' | '\u{2014}' | '\u{2026}' | '\u{2018}'..='\u{201d}'))
}

/// The text as filled glyphs (Onshape draws sheet text in a filled sans
/// font), as triangles in sheet coordinates: same cap height, anchor and
/// angle as [`text_strokes`]. Glyphs are narrowed when they would be wider
/// than [`text_width`], so layouts computed from it (dimension gaps, table
/// cells, title block) still fit.
pub fn text_fill(t: &Text2) -> (Vec<[f64; 2]>, Vec<u32>) {
    text_fill_font(t, None)
}

/// The text height of a font in [`noshape_sketch::text::layout_drawing_text`]
/// units: the top of its capital "A" (live asb 10-24: a note's text height
/// is the "A"'s yMax for every TrueType / OpenType font), cached.
fn fill_cap(family: &str, bold: bool, italic: bool) -> f64 {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};
    /// (family, bold, italic) -> cap height.
    type CapCache = Mutex<HashMap<(String, bool, bool), f64>>;
    static CACHE: OnceLock<CapCache> = OnceLock::new();
    let key = (family.to_string(), bold, italic);
    if let Some(v) = CACHE.get_or_init(Default::default).lock().ok().and_then(|c| c.get(&key).copied()) {
        return v;
    }
    let l = noshape_sketch::text::layout_drawing_text("A", family, bold, italic);
    let top = l.polylines(2).iter().flatten().map(|p| p.y).fold(0.0f64, f64::max);
    let v = if top > 0.1 { top } else { FILL_CAP };
    if let Ok(mut c) = CACHE.get_or_init(Default::default).lock() {
        c.insert(key, v);
    }
    v
}

/// [`text_fill`] in a font (None: the drawing's default sans).
pub fn text_fill_font(t: &Text2, font: Option<&super::sheet::TextFont>) -> (Vec<[f64; 2]>, Vec<u32>) {
    let (family, bold, italic) = match font {
        Some(f) => (f.family.as_str(), f.bold, f.italic),
        None => (DEFAULT_FAMILY, false, false),
    };
    let layout = noshape_sketch::text::layout_drawing_text(&t.text, family, bold, italic);
    if layout.contours.is_empty() || layout.width <= 0.0 {
        return (Vec::new(), Vec::new());
    }
    let cap = fill_cap(family, bold, italic);
    let s = t.height / cap;
    let natural = layout.width * s;
    let target = text_width(&t.text, t.height);
    let sx = if natural > target && target > 0.0 { target / natural } else { 1.0 };
    let off = anchor_offset(t.anchor, natural * sx, t.height);
    let (c, sn) = (t.angle.cos(), t.angle.sin());
    let map = |x: f64, y: f64| {
        let (lx, ly) = (off[0] + x * s * sx, off[1] + y * s);
        [t.pos[0] + lx * c - ly * sn, t.pos[1] + lx * sn + ly * c]
    };
    let steps = if t.height * 4.0 > 6.0 { 4 } else { 2 };
    let mut rings: Vec<Vec<[f64; 2]>> = layout
        .polylines(steps)
        .into_iter()
        .map(|mut r| {
            if r.len() > 1 && (r[0] - r[r.len() - 1]).length() < 1e-9 {
                r.pop();
            }
            r.into_iter().map(|p| [p.x, p.y]).collect::<Vec<_>>()
        })
        .filter(|r: &Vec<[f64; 2]>| r.len() >= 3)
        .collect();
    let area = |r: &[[f64; 2]]| r.iter().zip(r.iter().cycle().skip(1)).map(|(a, b)| a[0] * b[1] - b[0] * a[1]).sum::<f64>() / 2.0;
    let inside = |p: [f64; 2], r: &[[f64; 2]]| {
        let mut odd = false;
        for (a, b) in r.iter().zip(r.iter().cycle().skip(1)) {
            if (a[1] > p[1]) != (b[1] > p[1]) && p[0] < a[0] + (p[1] - a[1]) / (b[1] - a[1]) * (b[0] - a[0]) {
                odd = !odd;
            }
        }
        odd
    };
    // Outer contours share the orientation of the largest one; each hole
    // goes to the smallest outer contour around it.
    let areas: Vec<f64> = rings.iter().map(|r| area(r)).collect();
    let outer_sign = areas.iter().copied().fold(0.0f64, |m, a| if a.abs() > m.abs() { a } else { m }).signum();
    let outers: Vec<usize> = (0..rings.len()).filter(|&i| areas[i].signum() == outer_sign).collect();
    let mut holes: Vec<Vec<usize>> = vec![Vec::new(); rings.len()];
    for i in (0..rings.len()).filter(|i| !outers.contains(i)) {
        if let Some(&o) = outers.iter().filter(|&&o| inside(rings[i][0], &rings[o])).min_by(|&&a, &&b| areas[a].abs().total_cmp(&areas[b].abs())) {
            holes[o].push(i);
        }
    }
    let (mut pts, mut idx) = (Vec::new(), Vec::new());
    for &o in &outers {
        let mut flat: Vec<f64> = Vec::new();
        let mut hole_starts = Vec::new();
        let base = pts.len() as u32;
        for (k, &r) in std::iter::once(&o).chain(holes[o].iter()).enumerate() {
            if k > 0 {
                hole_starts.push(flat.len() / 2);
            }
            for p in std::mem::take(&mut rings[r]) {
                flat.extend([p[0], p[1]]);
                pts.push(map(p[0], p[1]));
            }
        }
        if let Ok(tris) = earcutr::earcut(&flat, &hole_starts, 2) {
            idx.extend(tris.into_iter().map(|i| base + i as u32));
        }
    }
    (pts, idx)
}

/// The four corners of the text's bounding box (sheet coordinates,
/// counter-clockwise from the lower-left of the text frame), including a
/// small margin and room for descenders.
pub fn text_box(t: &Text2) -> [[f64; 2]; 4] {
    let w = text_width(&t.text, t.height);
    let off = anchor_offset(t.anchor, w, t.height);
    let m = t.height * 0.15;
    let (x0, y0, x1, y1) = (off[0] - m, off[1] - t.height * 0.3 - m, off[0] + w + m, off[1] + t.height + m);
    let (c, s) = (t.angle.cos(), t.angle.sin());
    let tr = |x: f64, y: f64| [t.pos[0] + x * c - y * s, t.pos[1] + x * s + y * c];
    [tr(x0, y0), tr(x1, y0), tr(x1, y1), tr(x0, y1)]
}

/// Whether a sheet point lies inside the text's bounding box.
pub fn text_hit(t: &Text2, p: [f64; 2]) -> bool {
    let w = text_width(&t.text, t.height);
    let off = anchor_offset(t.anchor, w, t.height);
    let m = t.height * 0.15;
    let (c, s) = (t.angle.cos(), t.angle.sin());
    let d = [p[0] - t.pos[0], p[1] - t.pos[1]];
    let lx = d[0] * c + d[1] * s;
    let ly = -d[0] * s + d[1] * c;
    lx >= off[0] - m && lx <= off[0] + w + m && ly >= off[1] - t.height * 0.3 - m && ly <= off[1] + t.height + m
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filled_text_fits_stroke_width() {
        for text in ["SECTION A-A", "100", "Ø24 H7", "B"] {
            let t = Text2 { pos: [10.0, 10.0], height: 3.5, text: text.into(), angle: 0.0, anchor: Anchor::BottomLeft, pick: 0 };
            let (pts, idx) = text_fill(&t);
            assert!(!idx.is_empty() && idx.len() % 3 == 0, "{text}");
            let max_x = pts.iter().map(|p| p[0]).fold(f64::MIN, f64::max);
            assert!(max_x <= 10.0 + text_width(text, 3.5) + 1e-6, "{text}");
            let max_y = pts.iter().map(|p| p[1]).fold(f64::MIN, f64::max);
            assert!(max_y < 10.0 + 3.5 * 1.25, "{text}");
        }
    }

    #[test]
    fn every_glyph_parses() {
        for (c, _, def) in GLYPHS {
            let g = glyph(*c);
            if !def.is_empty() {
                assert!(!g.strokes.is_empty(), "glyph {c:?} has no strokes");
                for s in &g.strokes {
                    assert!(!s.is_empty(), "glyph {c:?}");
                }
            }
        }
        assert!(text_width("100", 3.5) > 0.0);
    }

    #[test]
    fn anchors_and_rotation() {
        let t = Text2 { pos: [10.0, 10.0], height: 5.0, text: "AB".into(), angle: 0.0, anchor: Anchor::BottomCenter, pick: 0 };
        let strokes = text_strokes(&t);
        let xs: Vec<f64> = strokes.iter().flatten().map(|p| p[0]).collect();
        let (lo, hi) = (xs.iter().cloned().fold(f64::MAX, f64::min), xs.iter().cloned().fold(f64::MIN, f64::max));
        assert!(((lo + hi) / 2.0 - 10.0).abs() < 0.2);
        assert!(text_hit(&t, [10.0, 12.0]));
        assert!(!text_hit(&t, [10.0, 20.0]));
        let r = Text2 { angle: std::f64::consts::FRAC_PI_2, ..t };
        assert!(text_hit(&r, [8.0, 10.0]));
    }
}
