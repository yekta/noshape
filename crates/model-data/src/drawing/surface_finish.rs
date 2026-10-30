//! Surface finish symbols (live request 1628, samples/dx1628-surface-finish.dxf):
//! the six types, the left / right cells, All around, parentheses, the
//! direction lay and extra right rows. Sizes scale with the annotation text
//! height `h` (live: h 3.5 mm).

use super::font;
use super::sheet::{Anchor, LineStyle, Polyline2, SheetGeometry, Text2};
use serde::{Deserialize, Serialize};

/// The Type list, in live's order.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SfKind {
    #[default]
    Basic,
    RemovalRequired,
    RemovalProhibited,
    IsoBasic,
    IsoRemovalRequired,
    IsoRemovalProhibited,
}

impl SfKind {
    pub const ALL: [SfKind; 6] = [SfKind::Basic, SfKind::RemovalRequired, SfKind::RemovalProhibited, SfKind::IsoBasic, SfKind::IsoRemovalRequired, SfKind::IsoRemovalProhibited];

    pub fn name(self) -> &'static str {
        match self {
            SfKind::Basic => "Basic",
            SfKind::RemovalRequired => "Material removal required",
            SfKind::RemovalProhibited => "Material removal prohibited",
            SfKind::IsoBasic => "ISO Basic",
            SfKind::IsoRemovalRequired => "ISO material removal required",
            SfKind::IsoRemovalProhibited => "ISO material removal prohibited",
        }
    }

    pub fn iso(self) -> bool {
        matches!(self, SfKind::IsoBasic | SfKind::IsoRemovalRequired | SfKind::IsoRemovalProhibited)
    }

    fn required(self) -> bool {
        matches!(self, SfKind::RemovalRequired | SfKind::IsoRemovalRequired)
    }

    fn prohibited(self) -> bool {
        matches!(self, SfKind::RemovalProhibited | SfKind::IsoRemovalProhibited)
    }
}

/// Direction lay ▾ (live: glyph, then name).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SfLay {
    Angular,
    Circular,
    MultiDirectional,
    Parallel,
    Perpendicular,
    Protuberant,
    Radial,
}

impl SfLay {
    pub const ALL: [SfLay; 7] = [SfLay::Angular, SfLay::Circular, SfLay::MultiDirectional, SfLay::Parallel, SfLay::Perpendicular, SfLay::Protuberant, SfLay::Radial];

    pub fn glyph(self) -> &'static str {
        match self {
            SfLay::Angular => "X",
            SfLay::Circular => "C",
            SfLay::MultiDirectional => "M",
            SfLay::Parallel => "=",
            SfLay::Perpendicular => "⊥",
            SfLay::Protuberant => "P",
            SfLay::Radial => "R",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            SfLay::Angular => "Angular",
            SfLay::Circular => "Circular",
            SfLay::MultiDirectional => "MultiDirectional",
            SfLay::Parallel => "Parallel",
            SfLay::Perpendicular => "Perpendicular",
            SfLay::Protuberant => "Protuberant",
            SfLay::Radial => "Radial",
        }
    }
}

/// The Surface finish dialog beyond the first right cell (the symbol's
/// `value`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SfSpec {
    pub kind: SfKind,
    /// ☐ All around (not offered for the ISO types).
    pub all_around: bool,
    /// ☐ ( ).
    pub parens: bool,
    /// The left cells, top to bottom.
    pub left: [String; 3],
    /// The right cells after the first, top to bottom: the dialog's two,
    /// then one per Add row.
    pub right: Vec<String>,
    pub lay: Option<SfLay>,
}

impl SfSpec {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    /// Right rows (live: 3, Add row adds one each).
    pub fn rows(&self) -> usize {
        (1 + self.right.len()).max(3)
    }
}

const TAN60: f64 = 1.732_050_807_568_877_2;

impl SheetGeometry {
    /// A surface finish symbol with its tip at `p` (upright), `value` in
    /// the first right cell; text height `h`. Live's DXF in units of h:
    /// arms at 60°, the short one 1.75 h high, the long one 3.25 h (+1.5 h
    /// per right row past three); right texts hang off a top bar from the
    /// long arm's top, the left ones stand beside the arms.
    pub fn surface_finish_symbol(&mut self, p: [f64; 2], value: &str, spec: &SfSpec, h: f64, pick: u32) {
        let line = |points: Vec<[f64; 2]>, closed: bool| Polyline2 { points, style: LineStyle::Thin, closed, pick };
        let circle = |c: [f64; 2], r: f64| (0..32).map(|i| i as f64 / 32.0 * std::f64::consts::TAU).map(|a| [c[0] + r * a.cos(), c[1] + r * a.sin()]).collect::<Vec<_>>();
        let iso = spec.kind.iso();
        let sa = [p[0] - 1.75 * h / TAN60, p[1] + 1.75 * h];
        let arm = 3.25 * h + 1.5 * h * (spec.rows() - 3) as f64;
        let la = [p[0] + arm / TAN60, p[1] + arm];
        // The right column: the first cell over the bar, the others under it.
        let mut right: Vec<&str> = vec![value];
        right.extend(spec.right.iter().map(String::as_str));
        while right.len() < spec.rows() {
            right.push("");
        }
        let col = la[0] + 0.68 * h;
        let lay = spec.lay.map(|l| l.glyph());
        let last = right.len() - 1;
        let shift = |i: usize| if lay.is_some() && i == last { 0.5 * h } else { 0.0 };
        let widths = right.iter().enumerate().map(|(i, t)| if t.is_empty() { 0.0 } else { shift(i) + font::text_width(t, h) }).fold(0.0, f64::max);
        let bar = right.iter().any(|t| !t.is_empty()) || lay.is_some();
        let mut arms = vec![sa, p, la];
        if bar {
            arms.push([col + widths.max(lay.map(|g| font::text_width(g, h)).unwrap_or(0.0)) + 0.4 * h, la[1]]);
        } else if iso {
            arms.push([la[0] + 0.3125 / 3.5 * h, la[1]]);
        }
        self.lines.push(line(arms, false));
        if spec.kind.required() {
            self.lines.push(line(vec![sa, [p[0] + 1.75 * h / TAN60, sa[1]]], false));
        }
        if spec.kind.prohibited() {
            self.lines.push(line(circle([p[0], p[1] + 1.3135 * h], 0.6567 * h), true));
        }
        if iso {
            let y = sa[1] + 0.6875 * h;
            self.lines.push(line(vec![[sa[0] + 0.3969 * h, y], [sa[0] + 1.7103 * h, y]], false));
        }
        if spec.all_around && !iso {
            self.lines.push(line(circle(la, 0.5 * h), true));
        }
        let text = |g: &mut SheetGeometry, x: f64, top: f64, t: &str, right_aligned: bool| {
            if t.is_empty() {
                return;
            }
            let x = if right_aligned { x - font::text_width(t, h) } else { x };
            g.texts.push(Text2 { pos: [x, top - h], height: h, text: t.to_string(), angle: 0.0, anchor: Anchor::BottomLeft, pick });
        };
        // Right cells: over the bar, then under it (live 1.25 h; −0.24 h,
        // −1.82 h, −3.32 h, then 1.5 h steps).
        for (i, t) in right.iter().enumerate() {
            let top = match i {
                0 => la[1] + 1.25 * h,
                1 => la[1] - 0.2356 * h,
                2 => la[1] - 1.8167 * h,
                k => la[1] - 3.3214 * h - 1.5 * h * (k - 3) as f64,
            };
            text(self, col + shift(i), top, t, false);
            if i == last
                && let Some(g) = lay
            {
                text(self, col - 0.69 * h, top, g, false);
            }
        }
        // Left cells: L1 / L2 beside the long arm (above the ISO bar for
        // the ISO types), L3 at the V.
        let up = if iso { 0.806 * h } else { 0.0 };
        text(self, p[0] + 1.313 * h, p[1] + 4.2457 * h + up, &spec.left[0], true);
        text(self, p[0] + 1.083 * h, p[1] + 3.01 * h + up, &spec.left[1], true);
        text(self, sa[0], p[1] + 1.514 * h, &spec.left[2], true);
    }

    /// ( ): two tall arcs around what was drawn since `mark` (live: r
    /// 7.2 h, ±18.18°, centred on the text block).
    pub fn surface_finish_parens(&mut self, mark: super::sheet::GeomMark, h: f64, pick: u32) {
        let mut lo = [f64::MAX; 2];
        let mut hi = [f64::MIN; 2];
        for q in self.lines[mark.0..].iter().flat_map(|l| l.points.iter().copied()).chain(self.texts[mark.1..].iter().flat_map(font::text_box)) {
            lo = [lo[0].min(q[0]), lo[1].min(q[1])];
            hi = [hi[0].max(q[0]), hi[1].max(q[1])];
        }
        if lo[0] > hi[0] {
            return;
        }
        let (r, half) = (7.2047 * h, 18.18f64.to_radians());
        let cy = (lo[1] + hi[1]) / 2.0;
        for (edge, dir) in [(lo[0] - 0.3 * h, 1.0), (hi[0] + 0.3 * h, -1.0)] {
            let cx = edge + dir * r;
            let points = (0..=12)
                .map(|i| std::f64::consts::PI - half + 2.0 * half * i as f64 / 12.0)
                .map(|a| [cx + dir * r * a.cos(), cy + r * a.sin()])
                .collect();
            self.lines.push(Polyline2 { points, style: LineStyle::Thin, closed: false, pick });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draw(value: &str, spec: &SfSpec) -> SheetGeometry {
        let mut g = SheetGeometry::new(420.0, 297.0);
        let mark = g.mark();
        g.surface_finish_symbol([0.0, 0.0], value, spec, 3.5, 1);
        if spec.parens {
            g.surface_finish_parens(mark, 3.5, 1);
        }
        g
    }

    fn near(a: [f64; 2], b: [f64; 2]) -> bool {
        (a[0] - b[0]).abs() < 2e-3 && (a[1] - b[1]).abs() < 2e-3
    }

    /// Live request 1628 (samples/dx1628-surface-finish.dxf), h 3.5.
    #[test]
    fn symbols_match_live() {
        // Basic: short arm (−3.5363, 6.125), long arm (6.5674, 11.375).
        let g = draw("", &SfSpec::default());
        assert_eq!(g.lines.len(), 1);
        let arms = &g.lines[0].points;
        assert!(near(arms[0], [-3.5363, 6.125]) && near(arms[1], [0.0, 0.0]) && near(arms[2], [6.5674, 11.375]), "{arms:?}");
        // Material removal required: the V closed at the short arm's top.
        let g = draw("", &SfSpec { kind: SfKind::RemovalRequired, ..Default::default() });
        let l = &g.lines[1].points;
        assert!(near(l[0], [-3.5363, 6.125]) && (l[1][0] - l[0][0] - 7.0726).abs() < 2e-3, "{l:?}");
        // Prohibited: a circle r 2.2986 centred 4.5972 above the tip.
        let g = draw("", &SfSpec { kind: SfKind::RemovalProhibited, ..Default::default() });
        let c = &g.lines[1].points;
        let (cx, cy) = (c.iter().map(|p| p[0]).sum::<f64>() / c.len() as f64, c.iter().map(|p| p[1]).sum::<f64>() / c.len() as f64);
        assert!(cx.abs() < 1e-6 && (cy - 4.5972).abs() < 2e-3 && (c[0][0] - 2.2986).abs() < 2e-3);
        // ISO: the bar 2.4063 above the short arm's top, 4.5972 long.
        let g = draw("", &SfSpec { kind: SfKind::IsoBasic, ..Default::default() });
        let bar = &g.lines[1].points;
        assert!(near(bar[0], [-3.5363 + 1.389, 6.125 + 2.4063]) && (bar[1][0] - bar[0][0] - 4.5972).abs() < 2e-3, "{bar:?}");
        // A right text: the top bar from the long arm's top; R1 over it,
        // R2 / R3 under it; L3 ends at the short arm's top.
        let spec = SfSpec { left: ["L1".into(), "L2".into(), "L3".into()], right: vec!["R2".into(), "R3".into()], ..Default::default() };
        let g = draw("R1", &spec);
        assert_eq!(g.lines[0].points.len(), 4);
        assert!((g.lines[0].points[3][1] - 11.375).abs() < 1e-9);
        let top = |g: &SheetGeometry, t: &str| g.texts.iter().find(|x| x.text == t).map(|x| [x.pos[0], x.pos[1] + x.height]).unwrap();
        assert!((top(&g, "R1")[1] - (11.375 + 4.375)).abs() < 1e-3 && (top(&g, "R1")[0] - (6.5674 + 2.38)).abs() < 1e-3);
        assert!(top(&g, "R2")[1] < 11.375 && top(&g, "R3")[1] < top(&g, "R2")[1]);
        let l3 = g.texts.iter().find(|x| x.text == "L3").unwrap();
        assert!((l3.pos[0] + font::text_width("L3", 3.5) - -3.5363).abs() < 1e-3);
        // Add row: the long arm grows by 1.5 h.
        let g = draw("R1", &SfSpec { right: vec!["".into(), "".into(), "R4".into()], ..Default::default() });
        assert!((g.lines[0].points[2][1] - 16.625).abs() < 1e-9);
        // A lay glyph before the last right cell, which moves 0.5 h right.
        let g = draw("R1", &SfSpec { right: vec!["".into(), "R3".into()], lay: Some(SfLay::Angular), ..Default::default() });
        let (x, r3) = (top(&g, "X"), top(&g, "R3"));
        assert!((x[1] - r3[1]).abs() < 1e-9 && x[0] < r3[0] && (r3[0] - (6.5674 + 2.38 + 1.75)).abs() < 1e-3);
        // All around: a circle r h/2 on the knee; ( ): two arcs.
        let g = draw("R1", &SfSpec { all_around: true, parens: true, ..Default::default() });
        assert_eq!(g.lines.len(), 4);
        assert!(g.lines[1].points.iter().all(|p| ((p[0] - 6.5674).hypot(p[1] - 11.375) - 1.75).abs() < 1e-3));
    }
}
