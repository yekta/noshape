//! Weld symbols (Onshape's Weld symbol tool, ANSI / AWS A2.4 and ISO 2553):
//! a leader with an arrow to the joint, a reference line, the joint symbol
//! on the arrow side (below the line) and / or the other side (above),
//! size and length texts, a joint spacer, stagger, symmetric, second fillet,
//! "All around", a field-weld flag and a tail with reference text. ISO adds
//! the dashed identification line.

use super::Leader;
use super::sheet::{Anchor, Fill2, LineStyle, Polyline2, SheetGeometry, Text2};
use serde::{Deserialize, Serialize};

/// Joint types.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum WeldJoint {
    #[default]
    None,
    SquareGroove,
    VGroove,
    UGroove,
    JGroove,
    BevelGroove,
    BevelButtBroadRoot,
    VFlareGroove,
    BevelFlareGroove,
    Bead,
    Fillet,
    PlugSlot,
    Seam,
    Spot,
}

impl WeldJoint {
    /// The joint dropdown's order.
    pub const ALL: [WeldJoint; 14] = [
        WeldJoint::None,
        WeldJoint::SquareGroove,
        WeldJoint::VGroove,
        WeldJoint::UGroove,
        WeldJoint::JGroove,
        WeldJoint::BevelGroove,
        WeldJoint::BevelButtBroadRoot,
        WeldJoint::VFlareGroove,
        WeldJoint::BevelFlareGroove,
        WeldJoint::Bead,
        WeldJoint::Fillet,
        WeldJoint::PlugSlot,
        WeldJoint::Seam,
        WeldJoint::Spot,
    ];

    pub fn name(self) -> &'static str {
        match self {
            WeldJoint::None => "None",
            WeldJoint::SquareGroove => "Square groove",
            WeldJoint::VGroove => "V groove",
            WeldJoint::UGroove => "U groove",
            WeldJoint::JGroove => "J groove",
            WeldJoint::BevelGroove => "Bevel groove",
            WeldJoint::BevelButtBroadRoot => "Single bevel butt broad root",
            WeldJoint::VFlareGroove => "V flare groove",
            WeldJoint::BevelFlareGroove => "Bevel flare groove",
            WeldJoint::Bead => "Bead joint",
            WeldJoint::Fillet => "Fillet joint",
            WeldJoint::PlugSlot => "Plug or Slot joint",
            WeldJoint::Seam => "Seam joint",
            WeldJoint::Spot => "Spot joint",
        }
    }
}

/// Joint spacers (both sides at once).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum JointSpacer {
    #[default]
    None,
    DoubleV,
    DoubleBevel,
    DoubleU,
    DoubleJ,
    DoubleFlareV,
    DoubleFlareBevel,
}

impl JointSpacer {
    pub const ALL: [JointSpacer; 7] = [JointSpacer::None, JointSpacer::DoubleV, JointSpacer::DoubleBevel, JointSpacer::DoubleU, JointSpacer::DoubleJ, JointSpacer::DoubleFlareV, JointSpacer::DoubleFlareBevel];

    pub fn name(self) -> &'static str {
        match self {
            JointSpacer::None => "None",
            JointSpacer::DoubleV => "Double V groove",
            JointSpacer::DoubleBevel => "Double bevel groove",
            JointSpacer::DoubleU => "Double U groove",
            JointSpacer::DoubleJ => "Double J groove",
            JointSpacer::DoubleFlareV => "Double flare V",
            JointSpacer::DoubleFlareBevel => "Double flare bevel",
        }
    }

    /// The joint drawn on each side.
    pub fn joint(self) -> WeldJoint {
        match self {
            JointSpacer::None => WeldJoint::None,
            JointSpacer::DoubleV => WeldJoint::VGroove,
            JointSpacer::DoubleBevel => WeldJoint::BevelGroove,
            JointSpacer::DoubleU => WeldJoint::UGroove,
            JointSpacer::DoubleJ => WeldJoint::JGroove,
            JointSpacer::DoubleFlareV => WeldJoint::VFlareGroove,
            JointSpacer::DoubleFlareBevel => WeldJoint::BevelFlareGroove,
        }
    }
}

/// Weld contour (Contour dropdown).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum WeldContour {
    #[default]
    None,
    Flat,
    Convex,
    Concave,
}

impl WeldContour {
    pub const ALL: [WeldContour; 4] = [WeldContour::None, WeldContour::Flat, WeldContour::Convex, WeldContour::Concave];

    pub fn name(self) -> &'static str {
        match self {
            WeldContour::None => "None",
            WeldContour::Flat => "Flat",
            WeldContour::Convex => "Convex",
            WeldContour::Concave => "Concave",
        }
    }
}

/// Finishing methods (Finishing dropdown): the letter drawn and the name.
pub const FINISHES: [(&str, &str); 7] = [("", "None"), ("C", "Chipping"), ("G", "Grinding"), ("H", "Hammering"), ("M", "Machining"), ("R", "Rolling"), ("U", "Unspecified")];

/// One side of the reference line: the joint row (size, second size,
/// joint, length, pitch), Contour, Finishing, Root opening and Groove.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WeldSide {
    pub joint: WeldJoint,
    /// Size (left of the symbol), the second size (in parentheses after
    /// it), length and pitch (right: "length-pitch").
    pub size: String,
    pub depth: String,
    pub length: String,
    pub pitch: String,
    pub contour: WeldContour,
    /// A letter of [`FINISHES`] ("" = none).
    pub finish: String,
    /// Root opening (inside the symbol) and groove angle (outside it).
    pub root: String,
    pub groove: String,
}

/// Field-weld flag direction.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum WeldFlag {
    #[default]
    None,
    Up,
    Down,
}

/// A weld symbol's content.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WeldSpec {
    /// Arrow side (below the line in ANSI) and other side (above).
    pub arrow: WeldSide,
    pub other: WeldSide,
    pub spacer: JointSpacer,
    pub stagger: bool,
    /// Stagger › Right (else Left): the other side's symbol moves that way.
    pub stagger_right: bool,
    pub symmetric: bool,
    /// Flip symbol: swaps the arrow and other sides.
    pub flip: bool,
    /// ISO: Flip identification line (the dashed line on the other side).
    pub flip_id_line: bool,
    pub all_around: bool,
    pub flag: WeldFlag,
    /// Tail reference text (shown when set).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
    /// More leaders (Add leader).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub leaders: Vec<Leader>,
}

/// Size of a joint glyph (mm, the symbol height).
const S: f64 = 3.0;

/// Draw a joint glyph at `x` (its left edge) on the line `y`, on the side
/// `dir` (+1 above, -1 below).
fn glyph(g: &mut SheetGeometry, j: WeldJoint, x: f64, y: f64, dir: f64, pick: u32) {
    let line = |g: &mut SheetGeometry, pts: Vec<[f64; 2]>, closed: bool| g.lines.push(Polyline2 { points: pts, style: LineStyle::Thin, closed, pick });
    let arc = |c: [f64; 2], r: f64, a0: f64, a1: f64| -> Vec<[f64; 2]> { (0..=12).map(|i| a0 + (a1 - a0) * i as f64 / 12.0).map(|t| [c[0] + r * t.cos(), c[1] + r * t.sin()]).collect() };
    let h = S * dir;
    match j {
        WeldJoint::None => {}
        WeldJoint::SquareGroove => {
            line(g, vec![[x + 0.8, y], [x + 0.8, y + h]], false);
            line(g, vec![[x + 2.2, y], [x + 2.2, y + h]], false);
        }
        WeldJoint::VGroove => line(g, vec![[x, y + h], [x + 1.5, y], [x + 3.0, y + h]], false),
        WeldJoint::BevelGroove => line(g, vec![[x, y + h], [x, y], [x + 2.0, y + h]], false),
        WeldJoint::BevelButtBroadRoot => {
            line(g, vec![[x, y + h], [x, y], [x + 0.6, y], [x + 0.6, y + h * 0.4], [x + 2.2, y + h]], false);
        }
        WeldJoint::UGroove => {
            line(g, vec![[x + 1.5, y], [x + 1.5, y + h * 0.4]], false);
            let c = [x + 1.5, y + h * 0.75];
            let pts = if dir > 0.0 { arc(c, 1.2, std::f64::consts::PI, std::f64::consts::TAU) } else { arc(c, 1.2, 0.0, std::f64::consts::PI) };
            line(g, pts, false);
        }
        WeldJoint::JGroove => {
            line(g, vec![[x, y], [x, y + h]], false);
            let c = [x, y + h * 0.75];
            let pts = if dir > 0.0 { arc(c, 1.4, -std::f64::consts::FRAC_PI_2, 0.0) } else { arc(c, 1.4, 0.0, std::f64::consts::FRAC_PI_2) };
            line(g, pts, false);
            line(g, vec![[x, y + h * 0.75 - 1.4 * dir], [x, y + h * 0.4]], false);
        }
        WeldJoint::VFlareGroove => {
            let r = 1.5;
            line(g, arc([x - 0.2, y + h], r, if dir > 0.0 { -std::f64::consts::FRAC_PI_2 } else { 0.0 }, if dir > 0.0 { 0.0 } else { std::f64::consts::FRAC_PI_2 }), false);
            line(g, arc([x + 3.2, y + h], r, if dir > 0.0 { std::f64::consts::PI } else { std::f64::consts::FRAC_PI_2 }, if dir > 0.0 { 1.5 * std::f64::consts::PI } else { std::f64::consts::PI }), false);
        }
        WeldJoint::BevelFlareGroove => {
            line(g, vec![[x, y], [x, y + h]], false);
            let r = 1.6;
            line(g, arc([x + 2.6, y + h], r, if dir > 0.0 { std::f64::consts::PI } else { std::f64::consts::FRAC_PI_2 }, if dir > 0.0 { 1.5 * std::f64::consts::PI } else { std::f64::consts::PI }), false);
        }
        WeldJoint::Bead => {
            let pts = if dir > 0.0 { arc([x + 1.5, y], 1.5, 0.0, std::f64::consts::PI) } else { arc([x + 1.5, y], 1.5, std::f64::consts::PI, std::f64::consts::TAU) };
            line(g, pts, false);
        }
        WeldJoint::Fillet => line(g, vec![[x, y], [x, y + h], [x + S, y]], false),
        WeldJoint::PlugSlot => line(g, vec![[x, y], [x, y + h * 0.8], [x + 3.0, y + h * 0.8], [x + 3.0, y]], false),
        WeldJoint::Seam | WeldJoint::Spot => {
            let cy = y + h * 0.5;
            line(g, arc([x + 1.5, cy], 1.4, 0.0, std::f64::consts::TAU), true);
            if j == WeldJoint::Seam {
                line(g, vec![[x - 0.4, cy + 0.5], [x + 3.4, cy + 0.5]], false);
                line(g, vec![[x - 0.4, cy - 0.5], [x + 3.4, cy - 0.5]], false);
            }
        }
    }
}

/// Draw a weld symbol: the arrow tips `tips` (sheet; the first is the
/// main leader's), the reference line starting at `at` (the leaders' end),
/// text height `th`, ISO or ANSI.
pub fn render(g: &mut SheetGeometry, spec: &WeldSpec, tips: &[[f64; 2]], at: [f64; 2], th: f64, iso: bool, pick: u32) {
    let tip = tips.first().copied().unwrap_or(at);
    let push = |g: &mut SheetGeometry, pts: Vec<[f64; 2]>, style: LineStyle| g.lines.push(Polyline2 { points: pts, style, closed: false, pick });
    // The reference line runs away from the arrow.
    let right = at[0] >= tip[0];
    let k = if right { 1.0 } else { -1.0 };
    let len = 26.0;
    let end = [at[0] + k * len, at[1]];
    for &t in tips {
        push(g, vec![t, at], LineStyle::Thin);
        g.arrow(t, [t[0] - at[0], t[1] - at[1]], 3.0);
    }
    push(g, vec![at, end], LineStyle::Thin);
    // Sides: the arrow side below, the other side above; Flip swaps them
    // (not while symmetric).
    let flip = spec.flip && !spec.symmetric;
    let (arrow_dir, other_dir) = if flip { (1.0, -1.0) } else { (-1.0, 1.0) };
    // Symmetric: the other side's joint mirrored across the line.
    let (a_side, o_side) = if spec.symmetric { (&spec.other, &spec.other) } else { (&spec.arrow, &spec.other) };
    let (a_joint, o_joint) = match spec.spacer {
        JointSpacer::None => (a_side.joint, o_side.joint),
        s => (s.joint(), s.joint()),
    };
    // ISO: the dashed identification line just under the reference line
    // (above it with Flip identification line).
    if iso && !spec.symmetric {
        let dy = if spec.flip_id_line { 0.7 } else { -0.7 };
        push(g, vec![[at[0], at[1] + dy], [end[0], end[1] + dy]], LineStyle::Hidden);
    }
    let gs = th * 1.5 / S;
    let sym_x = at[0] + k * len * 0.45 - 1.5 * gs;
    // Stagger: the two glyphs offset, the other side toward Left / Right.
    let stagger = if spec.stagger { 1.5 * if spec.stagger_right { 1.0 } else { -1.0 } } else { 0.0 };
    for (joint, side, dir, dx) in [(a_joint, a_side, arrow_dir, -stagger), (o_joint, o_side, other_dir, stagger)] {
        if joint == WeldJoint::None {
            continue;
        }
        let x = sym_x + dx;
        // The glyph is drawn at 1.5 × the text height (live: ≈ 20 px glyph
        // for 13 px text).
        let mark = g.lines.len();
        glyph(g, joint, x, at[1], dir, pick);
        for l in &mut g.lines[mark..] {
            l.points.iter_mut().for_each(|p| *p = [x + (p[0] - x) * gs, at[1] + (p[1] - at[1]) * gs]);
        }
        let s_w = S * gs;
        // Text rows: next to the line, then outward past the symbol.
        let row = |k: f64| {
            // Row 0 beside the symbol, the others past it.
            let d = if k == 0.0 { 0.6 } else { 0.9 + s_w + (k - 1.0) * (th + 0.8) };
            if dir > 0.0 { at[1] + d } else { at[1] - d - th }
        };
        let text = |g: &mut SheetGeometry, x: f64, y: f64, t: &str| g.texts.push(Text2 { pos: [x, y], height: th, text: t.to_string(), angle: 0.0, anchor: Anchor::BottomLeft, pick });
        let size = match (side.size.is_empty(), side.depth.is_empty()) {
            (_, true) => side.size.clone(),
            (true, false) => format!("({})", side.depth),
            (false, false) => format!("{}({})", side.size, side.depth),
        };
        if !size.is_empty() {
            text(g, x - 0.8 - super::font::text_width(&size, th), row(0.0), &size);
        }
        // Length and pitch: "50-100" (ANSI), "50(100)" (ISO).
        let len = match (side.length.is_empty(), side.pitch.is_empty()) {
            (_, true) => side.length.clone(),
            (_, false) if iso => format!("{}({})", side.length, side.pitch),
            (true, false) => format!("-{}", side.pitch),
            (false, false) => format!("{}-{}", side.length, side.pitch),
        };
        if !len.is_empty() {
            text(g, x + s_w + 1.0, row(0.0), &len);
        }
        // Root opening inside the symbol, the groove angle just outside it.
        let center = |t: &str| x + s_w / 2.0 - super::font::text_width(t, th) / 2.0;
        if !side.root.is_empty() {
            text(g, center(&side.root), row(0.0), &side.root);
        }
        let mut k = 1.0;
        if !side.groove.is_empty() {
            text(g, center(&side.groove), row(k), &side.groove);
            k += 1.0;
        }
        // Contour over the symbol, the finishing letter beyond it.
        let cy = if dir > 0.0 { row(k) + th * 0.3 } else { row(k) + th * 0.7 };
        match side.contour {
            WeldContour::None => {}
            WeldContour::Flat => g.lines.push(Polyline2 { points: vec![[x - 0.3, cy], [x + s_w + 0.3, cy]], style: LineStyle::Thin, closed: false, pick }),
            WeldContour::Convex | WeldContour::Concave => {
                let bulge = if side.contour == WeldContour::Convex { dir } else { -dir } * 0.8;
                let pts = (0..=12).map(|i| i as f64 / 12.0).map(|t| [x - 0.3 + (s_w + 0.6) * t, cy + bulge * (1.0 - (2.0 * t - 1.0).powi(2))]).collect();
                g.lines.push(Polyline2 { points: pts, style: LineStyle::Thin, closed: false, pick });
            }
        }
        if side.contour != WeldContour::None {
            k += 1.0;
        }
        if !side.finish.is_empty() && !iso {
            text(g, center(&side.finish), row(k), &side.finish);
        }
    }
    // All around: a circle at the junction; field weld: a filled pennant
    // beside it, under (or over) the line.
    if spec.all_around {
        let pts: Vec<[f64; 2]> = (0..=24).map(|i| i as f64 / 24.0 * std::f64::consts::TAU).map(|t| [at[0] + 1.0 * t.cos(), at[1] + 1.0 * t.sin()]).collect();
        g.lines.push(Polyline2 { points: pts, style: LineStyle::Thin, closed: true, pick });
    }
    if spec.flag != WeldFlag::None {
        let d = if spec.flag == WeldFlag::Up { 1.0 } else { -1.0 };
        let (x0, x1) = (at[0] + 0.3 * k, at[0] + 3.5 * k);
        g.fills.push(Fill2 { points: vec![[x0, at[1] + 2.0 * d], [x1, at[1] + 1.0 * d], [x1, at[1] + 3.0 * d]] });
    }
    // The tail with the reference text.
    if let Some(r) = spec.reference.as_ref().filter(|r| !r.is_empty()) {
        push(g, vec![[end[0] + 3.0 * k, end[1] + 3.0], end, [end[0] + 3.0 * k, end[1] - 3.0]], LineStyle::Thin);
        let w = super::font::text_width(r, th);
        let x = if right { end[0] + 4.0 } else { end[0] - 4.0 - w };
        g.texts.push(Text2 { pos: [x, end[1] - th / 2.0], height: th, text: r.clone(), angle: 0.0, anchor: Anchor::BottomLeft, pick });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weld_symbol_parts() {
        let mut g = SheetGeometry::new(420.0, 297.0);
        let spec = WeldSpec {
            arrow: WeldSide { joint: WeldJoint::Fillet, size: "6".into(), length: "50".into(), ..Default::default() },
            all_around: true,
            flag: WeldFlag::Up,
            reference: Some("GMAW".into()),
            ..Default::default()
        };
        render(&mut g, &spec, &[[100.0, 100.0]], [110.0, 110.0], 2.5, false, 1);
        let texts: Vec<&str> = g.texts.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(texts, ["6", "50", "GMAW"]);
        // Pitch, second size, groove angle, finishing.
        let mut g3 = SheetGeometry::new(420.0, 297.0);
        let side = WeldSide { joint: WeldJoint::VGroove, size: "1/4".into(), depth: "5/16".into(), length: "1".into(), pitch: "3".into(), groove: "60°".into(), contour: WeldContour::Flat, finish: "G".into(), ..Default::default() };
        render(&mut g3, &WeldSpec { arrow: side, ..Default::default() }, &[[100.0, 100.0]], [110.0, 110.0], 2.5, false, 1);
        let texts: Vec<&str> = g3.texts.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(texts, ["1/4(5/16)", "1-3", "60°", "G"]);
        // Arrow side: rows go down, away from the line.
        assert!(g3.texts[3].pos[1] < g3.texts[2].pos[1] && g3.texts[2].pos[1] < g3.texts[0].pos[1]);
        // ISO: "length(pitch)" and no finishing letter.
        let mut g4 = SheetGeometry::new(420.0, 297.0);
        let side = WeldSide { joint: WeldJoint::Fillet, size: "6".into(), length: "50".into(), pitch: "100".into(), finish: "G".into(), ..Default::default() };
        render(&mut g4, &WeldSpec { arrow: side, ..Default::default() }, &[[100.0, 100.0]], [110.0, 110.0], 2.5, true, 1);
        let texts: Vec<&str> = g4.texts.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(texts, ["6", "50(100)"]);
        // ANSI: arrow-side text below the line; no dashed line.
        assert!(g.texts[0].pos[1] < 110.0);
        assert!(!g.lines.iter().any(|l| l.style == LineStyle::Hidden));
        // ISO adds the dashed identification line; Flip puts the symbol above.
        let mut g2 = SheetGeometry::new(420.0, 297.0);
        render(&mut g2, &WeldSpec { flip: true, ..spec.clone() }, &[[100.0, 100.0]], [110.0, 110.0], 2.5, true, 1);
        assert!(g2.lines.iter().any(|l| l.style == LineStyle::Hidden));
        assert!(g2.texts[0].pos[1] > 110.0);
    }
}
