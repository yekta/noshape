//! Flattened 2D geometry of a drawing sheet (in millimeters, origin at the
//! sheet's lower-left corner, Y up). Produced from views (via the kernel's
//! hidden-line projection), annotations, border and title block; consumed by
//! the drawing editor renderer and the SVG/DXF/PDF exporters.

use super::{AnnotationKind, DimOrientation, Sheet};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LineStyle {
    Visible,
    Hidden,
    Tangent,
    Centerline,
    Thin,
    Border,
    Section,
}

impl LineStyle {
    /// Pen width in millimeters (ISO 128 line weights).
    pub fn width_mm(self) -> f64 {
        match self {
            LineStyle::Visible => 0.5,
            LineStyle::Border => 0.7,
            LineStyle::Section => 0.7,
            LineStyle::Hidden | LineStyle::Tangent => 0.35,
            LineStyle::Centerline | LineStyle::Thin => 0.25,
        }
    }
    /// Dash pattern in millimeters (empty = solid).
    pub fn dashes(self) -> &'static [f64] {
        match self {
            LineStyle::Hidden => &[3.0, 1.5],
            LineStyle::Centerline => &[8.0, 1.5, 1.5, 1.5],
            LineStyle::Section => &[10.0, 2.0, 2.0, 2.0],
            _ => &[],
        }
    }
    pub fn layer(self) -> &'static str {
        match self {
            LineStyle::Visible => "VISIBLE",
            LineStyle::Hidden => "HIDDEN",
            LineStyle::Tangent => "TANGENT",
            LineStyle::Centerline => "CENTER",
            LineStyle::Thin => "ANNOTATION",
            LineStyle::Border => "BORDER_FRAME",
            LineStyle::Section => "SECTION",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Polyline2 {
    pub points: Vec<[f64; 2]>,
    pub style: LineStyle,
    pub closed: bool,
    /// Selection id in the drawing editor (0 = none).
    pub pick: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Anchor {
    BottomLeft,
    BottomCenter,
    Center,
    TopLeft,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Text2 {
    pub pos: [f64; 2],
    pub height: f64,
    pub text: String,
    /// Radians, counter-clockwise.
    pub angle: f64,
    pub anchor: Anchor,
    pub pick: u32,
}

/// Filled triangle (arrowheads).
#[derive(Clone, Debug, PartialEq)]
pub struct Fill2 {
    pub points: Vec<[f64; 2]>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SheetGeometry {
    pub width: f64,
    pub height: f64,
    pub lines: Vec<Polyline2>,
    pub texts: Vec<Text2>,
    pub fills: Vec<Fill2>,
}

impl SheetGeometry {
    pub fn new(width: f64, height: f64) -> Self {
        Self { width, height, ..Default::default() }
    }

    pub fn line(&mut self, a: [f64; 2], b: [f64; 2], style: LineStyle) {
        self.lines.push(Polyline2 { points: vec![a, b], style, closed: false, pick: 0 });
    }

    pub fn rect(&mut self, x0: f64, y0: f64, x1: f64, y1: f64, style: LineStyle) {
        self.lines.push(Polyline2 { points: vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1]], style, closed: true, pick: 0 });
    }

    pub fn text(&mut self, pos: [f64; 2], height: f64, text: impl Into<String>, anchor: Anchor) {
        self.texts.push(Text2 { pos, height, text: text.into(), angle: 0.0, anchor, pick: 0 });
    }

    pub fn arrow(&mut self, tip: [f64; 2], dir: [f64; 2], len: f64) {
        let l = (dir[0] * dir[0] + dir[1] * dir[1]).sqrt().max(1e-12);
        let (dx, dy) = (dir[0] / l, dir[1] / l);
        let w = len * 0.3;
        let base = [tip[0] - dx * len, tip[1] - dy * len];
        self.fills.push(Fill2 { points: vec![tip, [base[0] - dy * w, base[1] + dx * w], [base[0] + dy * w, base[1] - dx * w]] });
    }

    /// Border frame with zone markers and an ISO-style title block.
    pub fn add_border_and_title(&mut self, sheet: &Sheet) {
        let (w, h) = (self.width, self.height);
        let m = 10.0;
        self.rect(m, m, w - m, h - m, LineStyle::Border);
        // Zone ticks.
        let nx = ((w - 2.0 * m) / 50.0).round().max(2.0) as usize;
        let ny = ((h - 2.0 * m) / 50.0).round().max(2.0) as usize;
        for i in 1..nx {
            let x = m + (w - 2.0 * m) * i as f64 / nx as f64;
            self.line([x, m], [x, m - 4.0], LineStyle::Thin);
            self.line([x, h - m], [x, h - m + 4.0], LineStyle::Thin);
        }
        for i in 0..nx {
            let x = m + (w - 2.0 * m) * (i as f64 + 0.5) / nx as f64;
            self.text([x, h - m + 2.0], 2.5, format!("{}", i + 1), Anchor::BottomCenter);
        }
        for j in 1..ny {
            let y = m + (h - 2.0 * m) * j as f64 / ny as f64;
            self.line([m, y], [m - 4.0, y], LineStyle::Thin);
            self.line([w - m, y], [w - m + 4.0, y], LineStyle::Thin);
        }
        for j in 0..ny {
            let y = m + (h - 2.0 * m) * (j as f64 + 0.5) / ny as f64;
            let letter = (b'A' + (ny - 1 - j) as u8) as char;
            self.text([m - 2.0, y], 2.5, letter.to_string(), Anchor::Center);
        }
        // Title block (180 x 40 mm) in the lower-right corner.
        let (tw, th) = (180.0, 8.0 * ((sheet.title_block.fields.len() as f64) / 2.0).ceil());
        let (x0, y0) = (w - m - tw, m);
        self.rect(x0, y0, w - m, y0 + th, LineStyle::Border);
        let rows = ((sheet.title_block.fields.len() + 1) / 2).max(1);
        for (i, (k, v)) in sheet.title_block.fields.iter().enumerate() {
            let col = i % 2;
            let row = i / 2;
            let cx = x0 + col as f64 * tw / 2.0;
            let cy = y0 + th - (row as f64 + 1.0) * th / rows as f64;
            if col == 1 {
                self.line([cx, cy], [cx, cy + th / rows as f64], LineStyle::Thin);
            }
            if row > 0 && col == 0 {
                self.line([x0, cy + th / rows as f64], [w - m, cy + th / rows as f64], LineStyle::Thin);
            }
            self.text([cx + 1.5, cy + th / rows as f64 - 3.2], 1.8, k.to_uppercase(), Anchor::BottomLeft);
            let value = if k == "Scale" && v.is_empty() { format_scale(sheet.scale) } else { v.clone() };
            self.text([cx + 1.5, cy + 1.2], 3.0, value, Anchor::BottomLeft);
        }
    }

    /// Render the sheet's annotations (dimensions, notes, centermarks...).
    /// View-relative coordinates are mapped through `to_sheet(view, p)`.
    pub fn add_annotations(&mut self, sheet: &Sheet, to_sheet: &dyn Fn(super::ViewId, [f64; 2]) -> [f64; 2], fmt_len: &dyn Fn(f64) -> String) {
        for a in &sheet.annotations {
            let pick = (a.id & 0xFFFF_FFFF) as u32;
            match &a.kind {
                AnnotationKind::Note { text, height_mm } => {
                    for (i, line) in text.lines().enumerate() {
                        self.texts.push(Text2 { pos: [a.position[0], a.position[1] - i as f64 * height_mm * 1.5], height: *height_mm, text: line.to_string(), angle: 0.0, anchor: Anchor::TopLeft, pick });
                    }
                }
                AnnotationKind::LinearDimension { view, a: pa, b: pb, offset, orientation, override_text } => {
                    let p = to_sheet(*view, *pa);
                    let q = to_sheet(*view, *pb);
                    let model_len = match orientation {
                        DimOrientation::Horizontal => (pb[0] - pa[0]).abs(),
                        DimOrientation::Vertical => (pb[1] - pa[1]).abs(),
                        DimOrientation::Aligned => ((pb[0] - pa[0]).powi(2) + (pb[1] - pa[1]).powi(2)).sqrt(),
                    };
                    let text = override_text.clone().unwrap_or_else(|| fmt_len(model_len));
                    self.linear_dimension(p, q, *offset, *orientation, &text, pick);
                }
                AnnotationKind::RadialDimension { view, center, radius, diameter, angle } => {
                    let c = to_sheet(*view, *center);
                    let s = self.scale_of(*view, to_sheet);
                    let r = radius * s;
                    let dir = [angle.cos(), angle.sin()];
                    let tip = [c[0] + dir[0] * r, c[1] + dir[1] * r];
                    let out = [c[0] + dir[0] * (r + 12.0), c[1] + dir[1] * (r + 12.0)];
                    let start = if *diameter { [c[0] - dir[0] * r, c[1] - dir[1] * r] } else { c };
                    self.lines.push(Polyline2 { points: vec![start, out], style: LineStyle::Thin, closed: false, pick });
                    self.arrow(tip, dir, 3.0);
                    if *diameter {
                        self.arrow([c[0] - dir[0] * r, c[1] - dir[1] * r], [-dir[0], -dir[1]], 3.0);
                    }
                    let label = if *diameter { format!("Ø{}", fmt_len(radius * 2.0)) } else { format!("R{}", fmt_len(*radius)) };
                    self.texts.push(Text2 { pos: [out[0] + 1.0, out[1] + 0.8], height: 3.5, text: label, angle: 0.0, anchor: Anchor::BottomLeft, pick });
                }
                AnnotationKind::AngularDimension { view, vertex, a: pa, b: pb, radius } => {
                    let v = to_sheet(*view, *vertex);
                    let pa = to_sheet(*view, *pa);
                    let pb = to_sheet(*view, *pb);
                    let a0 = (pa[1] - v[1]).atan2(pa[0] - v[0]);
                    let mut a1 = (pb[1] - v[1]).atan2(pb[0] - v[0]);
                    if a1 < a0 {
                        a1 += std::f64::consts::TAU;
                    }
                    let n = 24;
                    let pts: Vec<[f64; 2]> = (0..=n).map(|i| {
                        let t = a0 + (a1 - a0) * i as f64 / n as f64;
                        [v[0] + radius * t.cos(), v[1] + radius * t.sin()]
                    }).collect();
                    self.lines.push(Polyline2 { points: pts, style: LineStyle::Thin, closed: false, pick });
                    if (a1 - a0) * radius > 7.0 {
                        let e0 = [v[0] + radius * a0.cos(), v[1] + radius * a0.sin()];
                        let e1 = [v[0] + radius * a1.cos(), v[1] + radius * a1.sin()];
                        self.arrow(e0, [a0.sin(), -a0.cos()], 3.0);
                        self.arrow(e1, [-a1.sin(), a1.cos()], 3.0);
                    }
                    let mid = (a0 + a1) / 2.0;
                    let deg = (a1 - a0).to_degrees();
                    self.texts.push(Text2 { pos: [v[0] + (radius + 3.0) * mid.cos(), v[1] + (radius + 3.0) * mid.sin()], height: 3.5, text: format!("{deg:.1}°"), angle: 0.0, anchor: Anchor::Center, pick });
                }
                AnnotationKind::Centermark { view, center, size } => {
                    let c = to_sheet(*view, *center);
                    let s = size * self.scale_of(*view, to_sheet);
                    self.lines.push(Polyline2 { points: vec![[c[0] - s, c[1]], [c[0] + s, c[1]]], style: LineStyle::Centerline, closed: false, pick });
                    self.lines.push(Polyline2 { points: vec![[c[0], c[1] - s], [c[0], c[1] + s]], style: LineStyle::Centerline, closed: false, pick });
                }
                AnnotationKind::Centerline { view, a: pa, b: pb } => {
                    let p = to_sheet(*view, *pa);
                    let q = to_sheet(*view, *pb);
                    self.lines.push(Polyline2 { points: vec![p, q], style: LineStyle::Centerline, closed: false, pick });
                }
                AnnotationKind::Balloon { view, anchor, text } => {
                    let p = to_sheet(*view, *anchor);
                    let c = a.position;
                    let r = 5.0;
                    let n = 32;
                    let circle: Vec<[f64; 2]> = (0..n).map(|i| {
                        let t = i as f64 / n as f64 * std::f64::consts::TAU;
                        [c[0] + r * t.cos(), c[1] + r * t.sin()]
                    }).collect();
                    self.lines.push(Polyline2 { points: circle, style: LineStyle::Thin, closed: true, pick });
                    let d = [p[0] - c[0], p[1] - c[1]];
                    let l = (d[0] * d[0] + d[1] * d[1]).sqrt().max(1e-9);
                    self.lines.push(Polyline2 { points: vec![[c[0] + d[0] / l * r, c[1] + d[1] / l * r], p], style: LineStyle::Thin, closed: false, pick });
                    self.texts.push(Text2 { pos: c, height: 3.5, text: text.clone(), angle: 0.0, anchor: Anchor::Center, pick });
                }
                AnnotationKind::SurfaceFinish { anchor, value } => {
                    let p = *anchor;
                    self.lines.push(Polyline2 { points: vec![[p[0] - 3.0, p[1] + 5.0], p, [p[0] + 6.0, p[1] + 10.0], [p[0] + 14.0, p[1] + 10.0]], style: LineStyle::Thin, closed: false, pick });
                    self.texts.push(Text2 { pos: [p[0] + 6.5, p[1] + 10.8], height: 2.5, text: value.clone(), angle: 0.0, anchor: Anchor::BottomLeft, pick });
                }
                AnnotationKind::Gdt { anchor, frame } => {
                    let mut x = anchor[0];
                    let y = anchor[1];
                    for cell in frame {
                        let w = 4.0 + cell.chars().count() as f64 * 2.2;
                        self.lines.push(Polyline2 { points: vec![[x, y], [x + w, y], [x + w, y + 6.0], [x, y + 6.0]], style: LineStyle::Thin, closed: true, pick });
                        self.texts.push(Text2 { pos: [x + w / 2.0, y + 3.0], height: 3.0, text: cell.clone(), angle: 0.0, anchor: Anchor::Center, pick });
                        x += w;
                    }
                }
            }
        }
    }

    fn scale_of(&self, view: super::ViewId, to_sheet: &dyn Fn(super::ViewId, [f64; 2]) -> [f64; 2]) -> f64 {
        let a = to_sheet(view, [0.0, 0.0]);
        let b = to_sheet(view, [1.0, 0.0]);
        ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt()
    }

    /// Linear dimension between sheet points p and q.
    pub fn linear_dimension(&mut self, p: [f64; 2], q: [f64; 2], offset: f64, orientation: DimOrientation, text: &str, pick: u32) {
        // Dimension line direction and normal.
        let (dir, nrm) = match orientation {
            DimOrientation::Horizontal => ([1.0, 0.0], [0.0, 1.0]),
            DimOrientation::Vertical => ([0.0, 1.0], [1.0, 0.0]),
            DimOrientation::Aligned => {
                let d = [q[0] - p[0], q[1] - p[1]];
                let l = (d[0] * d[0] + d[1] * d[1]).sqrt().max(1e-12);
                ([d[0] / l, d[1] / l], [-d[1] / l, d[0] / l])
            }
        };
        let proj = |x: [f64; 2]| x[0] * dir[0] + x[1] * dir[1];
        let base = |x: [f64; 2]| x[0] * nrm[0] + x[1] * nrm[1];
        let level = base(p).max(base(q)) + offset;
        let at = |x: [f64; 2]| {
            let s = proj(x);
            [dir[0] * s + nrm[0] * level, dir[1] * s + nrm[1] * level]
        };
        let (dp, dq) = (at(p), at(q));
        let gap = 1.0;
        let ext = 2.0;
        for (src, dst) in [(p, dp), (q, dq)] {
            let v = [dst[0] - src[0], dst[1] - src[1]];
            let l = (v[0] * v[0] + v[1] * v[1]).sqrt();
            if l > gap {
                let u = [v[0] / l, v[1] / l];
                self.lines.push(Polyline2 {
                    points: vec![[src[0] + u[0] * gap, src[1] + u[1] * gap], [dst[0] + u[0] * ext, dst[1] + u[1] * ext]],
                    style: LineStyle::Thin,
                    closed: false,
                    pick,
                });
            }
        }
        self.lines.push(Polyline2 { points: vec![dp, dq], style: LineStyle::Thin, closed: false, pick });
        let d = [dq[0] - dp[0], dq[1] - dp[1]];
        self.arrow(dp, [-d[0], -d[1]], 3.0);
        self.arrow(dq, d, 3.0);
        let mut angle = dir[1].atan2(dir[0]);
        if angle > std::f64::consts::FRAC_PI_2 + 1e-9 || angle < -std::f64::consts::FRAC_PI_2 - 1e-9 {
            angle += std::f64::consts::PI;
        }
        // Place the text on its own "up" side of the dimension line.
        let up = [-angle.sin(), angle.cos()];
        let mid = [(dp[0] + dq[0]) / 2.0 + up[0] * 1.2, (dp[1] + dq[1]) / 2.0 + up[1] * 1.2];
        self.texts.push(Text2 { pos: mid, height: 3.5, text: text.to_string(), angle, anchor: Anchor::BottomCenter, pick });
    }
}

pub fn format_scale(s: f64) -> String {
    if s >= 1.0 {
        format!("{}:1", trim(s))
    } else {
        format!("1:{}", trim(1.0 / s))
    }
}

fn trim(x: f64) -> String {
    let s = format!("{x:.3}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}
