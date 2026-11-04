//! Edge condition symbols (Onshape's Edge condition symbol tool, ISO 13715): an L-shaped glyph on a line, the
//! edge's allowed burr (+), undercut (−) or either (±) beside it (Asymmetric: two values stacked), a main value,
//! All around (a circle at the leader's corner) and ( ). The leader runs from the line to the edge, with an arrow
//! in ANSI and none in ISO (live behaviours "dx (2026-11-04 00:19, P2) drawing Edge condition symbol").

use super::Leader;
use super::sheet::{Anchor, LineStyle, Polyline2, SheetGeometry, Text2};
use serde::{Deserialize, Serialize};

/// The panel's Direction.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum EdgeDirection {
    #[default]
    Undefined,
    Vertical,
    Horizontal,
    Asymmetric,
}

impl EdgeDirection {
    pub const ALL: [EdgeDirection; 4] = [EdgeDirection::Undefined, EdgeDirection::Vertical, EdgeDirection::Horizontal, EdgeDirection::Asymmetric];

    pub fn label(self) -> &'static str {
        match self {
            EdgeDirection::Undefined => "Undefined",
            EdgeDirection::Vertical => "Vertical",
            EdgeDirection::Horizontal => "Horizontal",
            EdgeDirection::Asymmetric => "Asymmetric",
        }
    }
}

/// A row's sign: None leaves the row out (its value field disabled).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum EdgeSign {
    #[default]
    None,
    Plus,
    Minus,
    PlusMinus,
}

impl EdgeSign {
    pub const ALL: [EdgeSign; 4] = [EdgeSign::None, EdgeSign::Plus, EdgeSign::Minus, EdgeSign::PlusMinus];

    /// The dropdown's entry.
    pub fn label(self) -> &'static str {
        match self {
            EdgeSign::None => "None",
            EdgeSign::Plus => "+",
            EdgeSign::Minus => "−",
            EdgeSign::PlusMinus => "±",
        }
    }

    /// As drawn before the value (live draws the minus as a hyphen).
    pub fn prefix(self) -> &'static str {
        match self {
            EdgeSign::None => "",
            EdgeSign::Plus => "+",
            EdgeSign::Minus => "-",
            EdgeSign::PlusMinus => "±",
        }
    }
}

/// One of the panel's two rows: a sign and a length (meters).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EdgeRow {
    pub sign: EdgeSign,
    pub value: f64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EdgeConditionSpec {
    pub direction: EdgeDirection,
    /// The main value, drawn as typed (left of the glyph).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub main: String,
    pub rows: [EdgeRow; 2],
    pub all_around: bool,
    /// ( ): the symbol in parentheses.
    pub parens: bool,
    /// More leaders (Add leader).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub leaders: Vec<Leader>,
}

impl EdgeConditionSpec {
    /// The rows drawn: the first with a sign; both with Asymmetric.
    pub fn shown_rows(&self) -> Vec<EdgeRow> {
        let n = if self.direction == EdgeDirection::Asymmetric { 2 } else { 1 };
        self.rows.iter().take(n).copied().filter(|r| r.sign != EdgeSign::None).collect()
    }
}

/// Draws the symbol with its glyph corner at `at`, the leader to `tip` (`(tip, at)`); `values` are the shown rows' texts (sign
/// and value in the drawing's format), `h` the text height (mm), `arrow` an arrowhead at the edge (ANSI).
/// Proportions from live's ANSI DXF (h = 0.12 in): the glyph's legs 1.42 h, the line 0.25 h below the glyph's
/// corner, a value's top 1.387 h above it and 0.36 h right of the upright, an Asymmetric pair 1.71 h left of it with
/// tops 4.27 h and 2.99 h up, the main value ending 0.39 h left of it, All around a 0.25 h circle at the leader's
/// corner, the parentheses arcs of 2.82 h over ±22.6°.
pub fn render(g: &mut SheetGeometry, spec: &EdgeConditionSpec, values: &[String], (tip, at): ([f64; 2], [f64; 2]), h: f64, arrow: bool, pick: u32) {
    let [cx, cy] = at;
    let width = |s: &str| super::font::text_width(s, h);
    let text = |g: &mut SheetGeometry, pos: [f64; 2], s: &str| g.texts.push(Text2 { pos, height: h, text: s.to_string(), angle: 0.0, anchor: Anchor::TopLeft, pick });
    let line = |g: &mut SheetGeometry, points: Vec<[f64; 2]>| g.lines.push(Polyline2 { points, style: LineStyle::Thin, closed: false, pick });
    // The glyph.
    line(g, vec![[cx, cy + 1.42 * h], [cx, cy], [cx + 1.42 * h, cy]]);
    let asym = spec.direction == EdgeDirection::Asymmetric && values.len() == 2;
    let mut left = cx - 0.25 * h;
    let mut right = cx + 1.67 * h;
    if asym {
        for (s, top) in values.iter().zip([4.27, 2.99]) {
            text(g, [cx - 1.71 * h, cy + top * h], s);
        }
    } else if let Some(s) = values.first() {
        let x = cx + 0.355 * h;
        text(g, [x, cy + 1.387 * h], s);
        right = right.max(x + width(s) + 0.11 * h);
    }
    if !spec.main.is_empty() {
        let x = cx - 0.39 * h - width(&spec.main);
        text(g, [x, cy + 1.26 * h], &spec.main);
        left = left.min(x - 0.25 * h);
    }
    // The line under them; the leader from its right end (from its left with Asymmetric).
    let y = cy - 0.25 * h;
    let corner = if asym { [left, y] } else { [right, y] };
    line(g, vec![[left, y], [right, y]]);
    line(g, vec![corner, tip]);
    if arrow {
        g.arrow(tip, [tip[0] - corner[0], tip[1] - corner[1]], 3.0);
    }
    if spec.all_around {
        let mut ring = arc(corner, 0.25 * h, 0.0, std::f64::consts::TAU, 24);
        ring.pop();
        g.lines.push(Polyline2 { points: ring, style: LineStyle::Thin, closed: true, pick });
    }
    if spec.parens {
        let (r, half) = (2.82 * h, 22.62f64.to_radians());
        let mid = cy + 0.58 * h;
        let (l, rr) = (left - 0.47 * h, right + 0.47 * h);
        line(g, arc([l + r, mid], r, std::f64::consts::PI - half, std::f64::consts::PI + half, 12));
        line(g, arc([rr - r, mid], r, -half, half, 12));
    }
}

/// `n` points on a circle from angle `a0` to `a1`, both ends included.
fn arc(c: [f64; 2], r: f64, a0: f64, a1: f64, n: usize) -> Vec<[f64; 2]> {
    (0..=n).map(|i| a0 + (a1 - a0) * i as f64 / n as f64).map(|t| [c[0] + r * t.cos(), c[1] + r * t.sin()]).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const IN: f64 = 25.4;

    fn near(a: [f64; 2], b: [f64; 2], tol: f64) -> bool {
        (a[0] - b[0]).abs() < tol && (a[1] - b[1]).abs() < tol
    }

    /// Live's ANSI DXF (samples/dx-edge-condition, inches, h 0.12): "+.300" with its glyph corner at (2.7361,
    /// 7.4794), its top-left at (2.7787, 7.6458), the line at y 7.4494 from x 2.7061, the leader from its right end
    /// (3.2199) down to the edge at (3.209, 6.9214), an arrowhead there.
    #[test]
    fn a_value_beside_its_glyph_as_lives() {
        let mut g = SheetGeometry::new(279.4, 215.9);
        let spec = EdgeConditionSpec { rows: [EdgeRow { sign: EdgeSign::Plus, value: 0.3 * IN / 1000.0 }, EdgeRow::default()], ..Default::default() };
        let mm = |x: f64, y: f64| [x * IN, y * IN];
        render(&mut g, &spec, &["+.300".to_string()], (mm(3.209, 6.9214), mm(2.7361, 7.4794)), 0.12 * IN, true, 1);
        assert_eq!(g.lines[0].points.len(), 3);
        for (p, q) in g.lines[0].points.iter().zip([mm(2.7361, 7.6498), mm(2.7361, 7.4794), mm(2.9065, 7.4794)]) {
            assert!(near(*p, q, 0.01), "glyph {:?}", g.lines[0].points);
        }
        let t = &g.texts[0];
        assert!(t.text == "+.300" && near(t.pos, mm(2.7787, 7.6458), 0.01) && t.anchor == Anchor::TopLeft, "{t:?}");
        let shoulder = &g.lines[1].points;
        assert!(near(shoulder[0], mm(2.7061, 7.4494), 0.01) && (shoulder[1][1] - 7.4494 * IN).abs() < 0.01, "{shoulder:?}");
        // Its right end: after the text (our font's width; live's 3.2199 in).
        assert!((shoulder[1][0] - 3.2199 * IN).abs() < 0.2 * 0.12 * IN, "{shoulder:?}");
        assert_eq!(g.lines[2].points, vec![shoulder[1], mm(3.209, 6.9214)], "the leader");
        assert!(!g.fills.is_empty(), "an arrowhead");
    }

    /// Asymmetric (live: "+.300" over "-.100" left of the glyph at (5.0005, 6.624), the leader from the line's left
    /// end); a main value with All around and ( ) (live: "0.5" ending left of the glyph at (2.7361, 3.957), a
    /// 0.03 in circle at the leader's corner (2.9365, 3.927), two arcs of r 0.3385 in).
    #[test]
    fn asymmetric_main_value_all_around_and_parentheses_as_lives() {
        let h = 0.12 * IN;
        let mm = |x: f64, y: f64| [x * IN, y * IN];
        let mut g = SheetGeometry::new(279.4, 215.9);
        let spec = EdgeConditionSpec { direction: EdgeDirection::Asymmetric, rows: [EdgeRow { sign: EdgeSign::Plus, value: 0.0 }, EdgeRow { sign: EdgeSign::Minus, value: 0.0 }], ..Default::default() };
        render(&mut g, &spec, &["+.300".into(), "-.100".into()], (mm(4.3901, 6.3308), mm(5.0005, 6.624)), h, false, 1);
        assert!(near(g.texts[0].pos, mm(4.7957, 7.1362), 0.02) && near(g.texts[1].pos, mm(4.7941, 6.9824), 0.06), "{:?}", g.texts);
        assert!(near(g.lines[1].points[0], mm(4.9705, 6.594), 0.01) && near(g.lines[1].points[1], mm(5.2009, 6.594), 0.01), "{:?}", g.lines[1]);
        assert_eq!(g.lines[2].points[0], g.lines[1].points[0], "the leader from the line's left end");
        assert!(g.fills.is_empty(), "no arrowhead (ISO)");
        let mut g = SheetGeometry::new(279.4, 215.9);
        let spec = EdgeConditionSpec { main: "0.5".into(), all_around: true, parens: true, ..Default::default() };
        render(&mut g, &spec, &[], (mm(3.209, 3.4845), mm(2.7361, 3.957)), h, true, 1);
        let ring = g.lines.iter().find(|l| l.closed).expect("All around");
        let c = ring.points.iter().fold([0.0, 0.0], |s, p| [s[0] + p[0] / ring.points.len() as f64, s[1] + p[1] / ring.points.len() as f64]);
        assert!(near(c, mm(2.9365, 3.927), 0.01) && ((ring.points[0][0] - c[0]).hypot(ring.points[0][1] - c[1]) - 0.03 * IN).abs() < 0.01, "{c:?}");
        let arcs: Vec<_> = g.lines.iter().filter(|l| !l.closed && l.points.len() == 13).collect();
        assert_eq!(arcs.len(), 2, "two parentheses");
        assert!(g.texts[0].text == "0.5" && (g.texts[0].pos[1] - 4.1084 * IN).abs() < 0.01, "{:?}", g.texts);
    }
}
