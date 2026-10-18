//! Flattened 2D geometry of a drawing sheet (in millimeters, origin at the
//! sheet's lower-left corner, Y up). Produced from views (via the kernel's
//! hidden-line projection), annotations, border and title block; consumed by
//! the drawing editor renderer and the SVG/DXF/PDF exporters.

use super::font;
use super::format::DimText;
use super::{Annotation, AnnotationKind, ArcCondition, DatumTargetArea, DimOrientation, Drawing, Projection, Sheet, TitleBlockStyle, ViewId};

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
        self.add_border_and_title_with(sheet, &|s| s.to_string());
    }

    /// Border and title block with property fields expanded by `expand`
    /// (third angle projection symbol).
    pub fn add_border_and_title_with(&mut self, sheet: &Sheet, expand: &dyn Fn(&str) -> String) {
        self.add_border_and_title_full(sheet, Projection::ThirdAngle, expand);
    }

    /// Border frame, zone markers and the sheet's title block layout.
    pub fn add_border_and_title_full(&mut self, sheet: &Sheet, projection: Projection, expand: &dyn Fn(&str) -> String) {
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
        let value_of = |k: &str, v: &str| -> String {
            match k {
                "Scale" if v.is_empty() => format_scale(sheet.scale),
                "Size" if v.is_empty() => sheet.size.label().to_string(),
                _ => expand(v),
            }
        };
        match sheet.title_style {
            TitleBlockStyle::None => {}
            TitleBlockStyle::Simple => {
                // Two columns of fields, 180 mm wide, in the lower-right corner.
                let (tw, th) = (180.0, 8.0 * ((sheet.title_block.fields.len() as f64) / 2.0).ceil());
                let (x0, y0) = (w - m - tw, m);
                self.rect(x0, y0, w - m, y0 + th, LineStyle::Border);
                let rows = sheet.title_block.fields.len().div_ceil(2).max(1);
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
                    self.text([cx + 1.5, cy + 1.2], 3.0, value_of(k, v), Anchor::BottomLeft);
                }
            }
            style => {
                let Some([x0, y0, x1, y1]) = title_block_rect(sheet) else { return };
                self.rect(x0, y0, x1, y1, LineStyle::Border);
                for &(key, cx, cy, cw, ch) in title_cells(style) {
                    let (x, y) = (x0 + cx, y0 + cy);
                    self.rect(x, y, x + cw, y + ch, LineStyle::Thin);
                    if key == PROJECTION_CELL {
                        self.projection_symbol([x + cw / 2.0, y + ch / 2.0], projection);
                        continue;
                    }
                    self.text([x + 1.2, y + ch - 2.6], 1.8, key.to_uppercase(), Anchor::BottomLeft);
                    let v = sheet.title_field(key).unwrap_or("");
                    let value = value_of(key, v);
                    let lines: Vec<&str> = value.lines().collect();
                    if lines.len() > 1 {
                        let th = 2.0;
                        for (i, l) in lines.iter().enumerate() {
                            self.text([x + 1.2, y + ch - 6.2 - i as f64 * th * 1.4], th, l.to_string(), Anchor::BottomLeft);
                        }
                        continue;
                    }
                    let mut th = ((ch - 3.6) * 0.8).clamp(2.0, 5.0);
                    let tw = font::text_width(&value, th);
                    if tw > cw - 2.4 && tw > 0.0 {
                        th = (th * (cw - 2.4) / tw).max(1.2);
                    }
                    self.text([x + 1.2, y + 1.0], th, value, Anchor::BottomLeft);
                }
            }
        }
    }

    /// First / third angle projection symbol (a truncated cone) centered at `c`.
    pub fn projection_symbol(&mut self, c: [f64; 2], projection: Projection) {
        let (big, small, len) = (3.0, 1.5, 8.0);
        // Third angle: end view left of the side view; first angle: right.
        let (circ_x, trap_x) = match projection {
            Projection::ThirdAngle => (c[0] - 6.5, c[0] - 1.0),
            Projection::FirstAngle => (c[0] + 6.5, c[0] - 9.5),
        };
        let y = c[1];
        let n = 32;
        for r in [big, small] {
            let pts = (0..n).map(|i| {
                let t = i as f64 / n as f64 * std::f64::consts::TAU;
                [circ_x + r * t.cos(), y + r * t.sin()]
            });
            self.lines.push(Polyline2 { points: pts.collect(), style: LineStyle::Thin, closed: true, pick: 0 });
        }
        // Narrow end on the left in both conventions.
        self.lines.push(Polyline2 { points: vec![[trap_x, y - small], [trap_x + len, y - big], [trap_x + len, y + big], [trap_x, y + small]], style: LineStyle::Thin, closed: true, pick: 0 });
        self.line([circ_x - big - 1.0, y], [trap_x + len + 1.0, y], LineStyle::Centerline);
    }

    /// Render the sheet's annotations (dimensions, notes, centermarks...).
    /// View-relative coordinates are mapped through `env`.
    pub fn add_annotations(&mut self, sheet: &Sheet, env: &dyn AnnotationEnv) {
        for a in &sheet.annotations {
            let pick = (a.id & 0xFFFF_FFFF) as u32;
            match &a.kind {
                AnnotationKind::Note { text, height_mm } => {
                    let text = env.expand(text);
                    for (i, line) in text.lines().enumerate() {
                        self.texts.push(Text2 { pos: [a.position[0], a.position[1] - i as f64 * height_mm * 1.5], height: *height_mm, text: line.to_string(), angle: 0.0, anchor: Anchor::TopLeft, pick });
                    }
                }
                AnnotationKind::LinearDimension { view, a: pa, b: pb, offset, orientation, override_text } => {
                    let p = env.to_sheet(*view, *pa);
                    let q = env.to_sheet(*view, *pb);
                    // Measured in view coordinates (true length), along the
                    // view's axes for horizontal/vertical dimensions.
                    let rot = env.rotation(*view);
                    let d = [pb[0] - pa[0], pb[1] - pa[1]];
                    let model_len = match orientation {
                        DimOrientation::Aligned => (d[0] * d[0] + d[1] * d[1]).sqrt(),
                        o => {
                            // Sheet axis -> view axis.
                            let axis = if *o == DimOrientation::Horizontal { [1.0, 0.0] } else { [0.0, 1.0] };
                            let va = rotate2(axis, -rot);
                            (d[0] * va[0] + d[1] * va[1]).abs()
                        }
                    };
                    let t = match override_text {
                        Some(o) => DimText::plain(o.clone()),
                        None => env.text(a, model_len, false, ""),
                    };
                    self.linear_dimension_text(p, q, *offset, *orientation, &t, pick);
                }
                AnnotationKind::RadialDimension { view, center, radius, diameter, angle } => {
                    let c = env.to_sheet(*view, *center);
                    let r = radius * env.scale(*view);
                    let dir = [angle.cos(), angle.sin()];
                    let tip = [c[0] + dir[0] * r, c[1] + dir[1] * r];
                    let out = [c[0] + dir[0] * (r + 12.0), c[1] + dir[1] * (r + 12.0)];
                    let start = if *diameter { [c[0] - dir[0] * r, c[1] - dir[1] * r] } else { c };
                    self.lines.push(Polyline2 { points: vec![start, out], style: LineStyle::Thin, closed: false, pick });
                    self.arrow(tip, dir, 3.0);
                    if *diameter {
                        self.arrow([c[0] - dir[0] * r, c[1] - dir[1] * r], [-dir[0], -dir[1]], 3.0);
                    }
                    let t = if *diameter { env.text(a, radius * 2.0, false, "Ø") } else { env.text(a, *radius, false, "R") };
                    // Text on a short horizontal shoulder.
                    let left = dir[0] < 0.0;
                    let w = self.dim_label_width(&t, 3.5);
                    let sh = [out[0] + if left { -w - 2.0 } else { w + 2.0 }, out[1]];
                    self.lines.push(Polyline2 { points: vec![out, sh], style: LineStyle::Thin, closed: false, pick });
                    let at = [out[0] + if left { -1.0 } else { 1.0 }, out[1] + 0.8];
                    self.dim_label(at, 0.0, if left { 1.0 } else { 0.0 }, false, &t, 3.5, pick);
                }
                AnnotationKind::AngularDimension { view, vertex, a: pa, b: pb, radius } => {
                    let v = env.to_sheet(*view, *vertex);
                    let pa = env.to_sheet(*view, *pa);
                    let pb = env.to_sheet(*view, *pb);
                    let a0 = (pa[1] - v[1]).atan2(pa[0] - v[0]);
                    let mut a1 = (pb[1] - v[1]).atan2(pb[0] - v[0]);
                    if a1 < a0 {
                        a1 += std::f64::consts::TAU;
                    }
                    let n = 24;
                    let pts: Vec<[f64; 2]> = (0..=n)
                        .map(|i| {
                            let t = a0 + (a1 - a0) * i as f64 / n as f64;
                            [v[0] + radius * t.cos(), v[1] + radius * t.sin()]
                        })
                        .collect();
                    self.lines.push(Polyline2 { points: pts, style: LineStyle::Thin, closed: false, pick });
                    if (a1 - a0) * radius > 7.0 {
                        let e0 = [v[0] + radius * a0.cos(), v[1] + radius * a0.sin()];
                        let e1 = [v[0] + radius * a1.cos(), v[1] + radius * a1.sin()];
                        self.arrow(e0, [a0.sin(), -a0.cos()], 3.0);
                        self.arrow(e1, [-a1.sin(), a1.cos()], 3.0);
                    }
                    let mid = (a0 + a1) / 2.0;
                    let t = env.text(a, a1 - a0, true, "");
                    self.dim_label([v[0] + (radius + 3.0) * mid.cos(), v[1] + (radius + 3.0) * mid.sin()], 0.0, 0.5, true, &t, 3.5, pick);
                }
                AnnotationKind::Centermark { view, center, size } => {
                    let c = env.to_sheet(*view, *center);
                    let s = size * env.scale(*view);
                    let (u, w) = (rotate2([1.0, 0.0], env.rotation(*view)), rotate2([0.0, 1.0], env.rotation(*view)));
                    self.lines.push(Polyline2 { points: vec![[c[0] - u[0] * s, c[1] - u[1] * s], [c[0] + u[0] * s, c[1] + u[1] * s]], style: LineStyle::Centerline, closed: false, pick });
                    self.lines.push(Polyline2 { points: vec![[c[0] - w[0] * s, c[1] - w[1] * s], [c[0] + w[0] * s, c[1] + w[1] * s]], style: LineStyle::Centerline, closed: false, pick });
                }
                AnnotationKind::Centerline { view, a: pa, b: pb } => {
                    let p = env.to_sheet(*view, *pa);
                    let q = env.to_sheet(*view, *pb);
                    self.lines.push(Polyline2 { points: vec![p, q], style: LineStyle::Centerline, closed: false, pick });
                }
                AnnotationKind::Balloon { view, anchor, text } => {
                    let p = env.to_sheet(*view, *anchor);
                    let c = a.position;
                    let r = 5.0;
                    let n = 32;
                    let circle: Vec<[f64; 2]> = (0..n)
                        .map(|i| {
                            let t = i as f64 / n as f64 * std::f64::consts::TAU;
                            [c[0] + r * t.cos(), c[1] + r * t.sin()]
                        })
                        .collect();
                    self.lines.push(Polyline2 { points: circle, style: LineStyle::Thin, closed: true, pick });
                    let d = [p[0] - c[0], p[1] - c[1]];
                    let l = (d[0] * d[0] + d[1] * d[1]).sqrt().max(1e-9);
                    self.lines.push(Polyline2 { points: vec![[c[0] + d[0] / l * r, c[1] + d[1] / l * r], p], style: LineStyle::Thin, closed: false, pick });
                    // Dot on the part.
                    self.fills.push(Fill2 { points: (0..8).map(|i| {
                        let t = i as f64 / 8.0 * std::f64::consts::TAU;
                        [p[0] + 0.6 * t.cos(), p[1] + 0.6 * t.sin()]
                    }).collect() });
                    self.texts.push(Text2 { pos: c, height: 3.5, text: env.balloon_text(a, text), angle: 0.0, anchor: Anchor::Center, pick });
                }
                AnnotationKind::SurfaceFinish { anchor, value } => {
                    let p = *anchor;
                    self.lines.push(Polyline2 { points: vec![[p[0] - 3.0, p[1] + 5.0], p, [p[0] + 6.0, p[1] + 10.0], [p[0] + 14.0, p[1] + 10.0]], style: LineStyle::Thin, closed: false, pick });
                    self.lines.push(Polyline2 { points: vec![[p[0] - 3.0, p[1] + 5.0], [p[0] + 3.0, p[1] + 5.0]], style: LineStyle::Thin, closed: false, pick });
                    self.texts.push(Text2 { pos: [p[0] + 6.5, p[1] + 10.8], height: 2.5, text: env.expand(value), angle: 0.0, anchor: Anchor::BottomLeft, pick });
                }
                AnnotationKind::Gdt { anchor, frame, leaders } => {
                    let mut x = anchor[0];
                    let y = anchor[1];
                    let h = 3.0;
                    for cell in frame {
                        let w = (font::text_width(cell, h) + 3.0).max(7.0);
                        self.lines.push(Polyline2 { points: vec![[x, y], [x + w, y], [x + w, y + 7.0], [x, y + 7.0]], style: LineStyle::Thin, closed: true, pick });
                        self.texts.push(Text2 { pos: [x + w / 2.0, y + 3.5], height: h, text: cell.clone(), angle: 0.0, anchor: Anchor::Center, pick });
                        x += w;
                    }
                    // Leaders leave the nearer short side through a shoulder.
                    let (x0, x1, ym) = (anchor[0], x, y + 3.5);
                    for l in leaders {
                        let t = match l.view {
                            Some(v) => env.to_sheet(v, l.point),
                            None => l.point,
                        };
                        let right = t[0] > (x0 + x1) / 2.0;
                        let start = if right { [x1, ym] } else { [x0, ym] };
                        let sh = [start[0] + if right { 4.0 } else { -4.0 }, ym];
                        self.lines.push(Polyline2 { points: vec![start, sh, t], style: LineStyle::Thin, closed: false, pick });
                        self.arrow(t, [t[0] - sh[0], t[1] - sh[1]], 3.0);
                    }
                }
                AnnotationKind::Datum { anchor, label } => {
                    let c = a.position;
                    let (bw, bh) = ((font::text_width(label, 3.5) + 3.0).max(7.0), 7.0);
                    self.lines.push(Polyline2 { points: vec![[c[0] - bw / 2.0, c[1] - bh / 2.0], [c[0] + bw / 2.0, c[1] - bh / 2.0], [c[0] + bw / 2.0, c[1] + bh / 2.0], [c[0] - bw / 2.0, c[1] + bh / 2.0]], style: LineStyle::Thin, closed: true, pick });
                    self.texts.push(Text2 { pos: c, height: 3.5, text: label.clone(), angle: 0.0, anchor: Anchor::Center, pick });
                    let d = [c[0] - anchor[0], c[1] - anchor[1]];
                    let l = (d[0] * d[0] + d[1] * d[1]).sqrt();
                    if l > 1e-6 {
                        let u = [d[0] / l, d[1] / l];
                        let w = [-u[1], u[0]];
                        let apex = [anchor[0] + u[0] * 3.0, anchor[1] + u[1] * 3.0];
                        self.fills.push(Fill2 { points: vec![[anchor[0] + w[0] * 1.8, anchor[1] + w[1] * 1.8], apex, [anchor[0] - w[0] * 1.8, anchor[1] - w[1] * 1.8]] });
                        // Leader from the triangle to the box edge.
                        let hit = box_exit(c, bw / 2.0, bh / 2.0, [-u[0], -u[1]]);
                        self.lines.push(Polyline2 { points: vec![apex, hit], style: LineStyle::Thin, closed: false, pick });
                    }
                }
                AnnotationKind::OrdinateDimension { view, origin, point, horizontal } => {
                    let p = env.to_sheet(*view, *point);
                    let rot = env.rotation(*view);
                    // Leader direction (perpendicular to the measured axis).
                    let n = rotate2(if *horizontal { [0.0, 1.0] } else { [1.0, 0.0] }, rot);
                    let value = if *horizontal { (point[0] - origin[0]).abs() } else { (point[1] - origin[1]).abs() };
                    let t = env.text(a, value, false, "");
                    let e = a.position;
                    let along = (e[0] - p[0]) * n[0] + (e[1] - p[1]) * n[1];
                    let sg = if along < 0.0 { -1.0 } else { 1.0 };
                    let start = [p[0] + n[0] * sg * 1.0, p[1] + n[1] * sg * 1.0];
                    let knee_t = (along - sg * 4.0).abs().min(along.abs()) * sg;
                    let knee = [p[0] + n[0] * knee_t, p[1] + n[1] * knee_t];
                    let end = e;
                    let pts = if ((end[0] - knee[0]) * n[1] - (end[1] - knee[1]) * n[0]).abs() < 0.05 { vec![start, end] } else { vec![start, knee, [end[0] - n[0] * sg * 2.0, end[1] - n[1] * sg * 2.0], end] };
                    self.lines.push(Polyline2 { points: pts, style: LineStyle::Thin, closed: false, pick });
                    let mut ang = n[1].atan2(n[0]);
                    let mut halign = if sg > 0.0 { 0.0 } else { 1.0 };
                    if ang > std::f64::consts::FRAC_PI_2 + 1e-9 || ang < -std::f64::consts::FRAC_PI_2 - 1e-9 {
                        ang += std::f64::consts::PI;
                        halign = 1.0 - halign;
                    }
                    let at = [end[0] + n[0] * sg * 1.0, end[1] + n[1] * sg * 1.0];
                    self.dim_label(at, ang, halign, true, &t, 3.5, pick);
                }
                AnnotationKind::ChamferDimension { view, a: ca, b: cb, c: cc, d: cd } => {
                    let pa = env.to_sheet(*view, *ca);
                    let pb = env.to_sheet(*view, *cb);
                    let m = [(pa[0] + pb[0]) / 2.0, (pa[1] + pb[1]) / 2.0];
                    let u = [cb[0] - ca[0], cb[1] - ca[1]];
                    let w = [cd[0] - cc[0], cd[1] - cc[1]];
                    let (lu, lw) = ((u[0] * u[0] + u[1] * u[1]).sqrt(), (w[0] * w[0] + w[1] * w[1]).sqrt());
                    let cosang = if lu * lw > 0.0 { ((u[0] * w[0] + u[1] * w[1]) / (lu * lw)).abs().min(1.0) } else { 1.0 };
                    let dist = lu * cosang;
                    let ang = cosang.acos();
                    let mut t = env.text(a, dist, false, "");
                    t.main = format!("{} × {}°", t.main, super::format::format_number(ang.to_degrees(), a.format.precision, 1));
                    let e = a.position;
                    let left = e[0] < m[0];
                    let wtxt = self.dim_label_width(&t, 3.5);
                    let sh = [e[0] + if left { -wtxt - 2.0 } else { wtxt + 2.0 }, e[1]];
                    self.lines.push(Polyline2 { points: vec![m, e, sh], style: LineStyle::Thin, closed: false, pick });
                    let d = [m[0] - e[0], m[1] - e[1]];
                    self.arrow(m, d, 3.0);
                    self.dim_label([e[0] + if left { -1.0 } else { 1.0 }, e[1] + 0.8], 0.0, if left { 1.0 } else { 0.0 }, false, &t, 3.5, pick);
                }
                AnnotationKind::ArcLengthDimension { view, center, radius, a: pa, b: pb, mid, offset } => {
                    let c = env.to_sheet(*view, *center);
                    let (sa, sb, sm) = (env.to_sheet(*view, *pa), env.to_sheet(*view, *pb), env.to_sheet(*view, *mid));
                    let ang = |p: [f64; 2]| (p[1] - c[1]).atan2(p[0] - c[0]);
                    let tau = std::f64::consts::TAU;
                    let a0 = ang(sa);
                    let mut sweep = (ang(sb) - a0).rem_euclid(tau);
                    if sweep < 1e-9 {
                        sweep = tau;
                    }
                    if (ang(sm) - a0).rem_euclid(tau) > sweep {
                        sweep -= tau;
                    }
                    let r0 = radius * env.scale(*view);
                    let rr = r0 + offset.max(2.0);
                    let n = 48;
                    let pts: Vec<[f64; 2]> = (0..=n).map(|i| {
                        let t = a0 + sweep * i as f64 / n as f64;
                        [c[0] + rr * t.cos(), c[1] + rr * t.sin()]
                    }).collect();
                    self.lines.push(Polyline2 { points: pts, style: LineStyle::Thin, closed: false, pick });
                    for (k, t) in [a0, a0 + sweep].into_iter().enumerate() {
                        let (ct, st) = (t.cos(), t.sin());
                        self.lines.push(Polyline2 { points: vec![[c[0] + (r0 + 1.0) * ct, c[1] + (r0 + 1.0) * st], [c[0] + (rr + 2.0) * ct, c[1] + (rr + 2.0) * st]], style: LineStyle::Thin, closed: false, pick });
                        let tip = [c[0] + rr * ct, c[1] + rr * st];
                        let s = if (k == 0) == (sweep > 0.0) { -1.0 } else { 1.0 };
                        self.arrow(tip, [-st * s, ct * s], 3.0);
                    }
                    let tm = a0 + sweep / 2.0;
                    let t = env.text(a, radius * sweep.abs(), false, "⌒");
                    self.dim_label([c[0] + (rr + 4.0) * tm.cos(), c[1] + (rr + 4.0) * tm.sin()], 0.0, 0.5, true, &t, 3.5, pick);
                }
                AnnotationKind::OrdinateGroup { view, origin, points, horizontal } => {
                    let rot = env.rotation(*view);
                    // u: measured axis on the sheet; n: leader direction.
                    let u = rotate2(if *horizontal { [1.0, 0.0] } else { [0.0, 1.0] }, rot);
                    let n = rotate2(if *horizontal { [0.0, 1.0] } else { [1.0, 0.0] }, rot);
                    let dot = |p: [f64; 2], d: [f64; 2]| p[0] * d[0] + p[1] * d[1];
                    let at = |s: f64, l: f64| [u[0] * s + n[0] * l, u[1] * s + n[1] * l];
                    let level = dot(a.position, n);
                    let sheet_pts: Vec<[f64; 2]> = points.iter().map(|p| env.to_sheet(*view, *p)).collect();
                    if sheet_pts.is_empty() {
                        continue;
                    }
                    let mean = sheet_pts.iter().map(|p| dot(*p, n)).sum::<f64>() / sheet_pts.len() as f64;
                    let sg = if level >= mean { 1.0 } else { -1.0 };
                    let along: Vec<f64> = sheet_pts.iter().map(|p| dot(*p, u)).collect();
                    let text_pos = ordinate_text_positions(&along, 3.5 + 1.5);
                    let mut ang = n[1].atan2(n[0]);
                    let mut halign = if sg > 0.0 { 0.0 } else { 1.0 };
                    if ang > std::f64::consts::FRAC_PI_2 + 1e-9 || ang < -std::f64::consts::FRAC_PI_2 - 1e-9 {
                        ang += std::f64::consts::PI;
                        halign = 1.0 - halign;
                    }
                    for (i, p) in points.iter().enumerate() {
                        let value = if *horizontal { (p[0] - origin[0]).abs() } else { (p[1] - origin[1]).abs() };
                        let t = env.text(a, value, false, "");
                        let (s0, s1) = (along[i], text_pos[i]);
                        let l0 = dot(sheet_pts[i], n) + sg * 1.0;
                        let pts = if (s1 - s0).abs() < 1e-6 {
                            vec![at(s0, l0), at(s0, level)]
                        } else {
                            // Jog: straight, slanted over to the text, straight again.
                            let k1 = level - sg * 6.0;
                            let k2 = level - sg * 3.0;
                            let k1 = if (k1 - l0) * sg < 0.0 { l0 } else { k1 };
                            vec![at(s0, l0), at(s0, k1), at(s1, k2), at(s1, level)]
                        };
                        self.lines.push(Polyline2 { points: pts, style: LineStyle::Thin, closed: false, pick });
                        self.dim_label(at(s1, level + sg * 1.0), ang, halign, true, &t, 3.5, pick);
                    }
                }
                AnnotationKind::MaxMinDimension { view, a: pa, ra, ca, b: pb, rb, cb, offset, orientation } => {
                    let (sa, sb) = (env.to_sheet(*view, *pa), env.to_sheet(*view, *pb));
                    let rot = env.rotation(*view);
                    let k = env.scale(*view);
                    let (dir, va) = match orientation {
                        DimOrientation::Aligned => {
                            let d = [sb[0] - sa[0], sb[1] - sa[1]];
                            let l = (d[0] * d[0] + d[1] * d[1]).sqrt().max(1e-12);
                            let m = [pb[0] - pa[0], pb[1] - pa[1]];
                            let lm = (m[0] * m[0] + m[1] * m[1]).sqrt().max(1e-300);
                            ([d[0] / l, d[1] / l], [m[0] / lm, m[1] / lm])
                        }
                        o => {
                            let axis = if *o == DimOrientation::Horizontal { [1.0, 0.0] } else { [0.0, 1.0] };
                            (axis, rotate2(axis, -rot))
                        }
                    };
                    let m = [pb[0] - pa[0], pb[1] - pa[1]];
                    let base = m[0] * va[0] + m[1] * va[1];
                    let s = if base < 0.0 { -1.0 } else { 1.0 };
                    // Signed shift of each end along the a -> b direction.
                    let shift = |r: f64, c: ArcCondition, toward: f64| match c {
                        _ if r <= 0.0 => 0.0,
                        ArcCondition::Center => 0.0,
                        ArcCondition::Min => r * toward,
                        ArcCondition::Max => -r * toward,
                    };
                    let (da, db) = (shift(*ra, *ca, 1.0), shift(*rb, *cb, -1.0));
                    let value = base.abs() - da + db;
                    let qa = [sa[0] + dir[0] * s * da * k, sa[1] + dir[1] * s * da * k];
                    let qb = [sb[0] + dir[0] * s * db * k, sb[1] + dir[1] * s * db * k];
                    let t = env.text(a, value.abs(), false, "");
                    self.linear_dimension_text(qa, qb, *offset, *orientation, &t, pick);
                }
                AnnotationKind::ForeshortenedRadial { view, center, radius, angle, jog } => {
                    let c = env.to_sheet(*view, *center);
                    let r = radius * env.scale(*view);
                    let dir = [angle.cos(), angle.sin()];
                    let tip = [c[0] + dir[0] * r, c[1] + dir[1] * r];
                    let f = env.to_sheet(*view, *jog);
                    let leg = (r * 0.4).clamp(3.0, 12.0);
                    let q = [tip[0] - dir[0] * leg, tip[1] - dir[1] * leg];
                    let t0 = ((q[0] - f[0]) * dir[0] + (q[1] - f[1]) * dir[1]) * 0.55;
                    let b = [f[0] + dir[0] * t0.max(0.0), f[1] + dir[1] * t0.max(0.0)];
                    self.lines.push(Polyline2 { points: vec![f, b, q, tip], style: LineStyle::Thin, closed: false, pick });
                    self.arrow(tip, dir, 3.0);
                    // A short cross marks the false center.
                    let w = [-dir[1], dir[0]];
                    self.line([f[0] - w[0] * 1.5, f[1] - w[1] * 1.5], [f[0] + w[0] * 1.5, f[1] + w[1] * 1.5], LineStyle::Thin);
                    if let Some(l) = self.lines.last_mut() {
                        l.pick = pick;
                    }
                    let t = env.text(a, *radius, false, "R");
                    let mut ang = dir[1].atan2(dir[0]);
                    if ang > std::f64::consts::FRAC_PI_2 + 1e-9 || ang < -std::f64::consts::FRAC_PI_2 - 1e-9 {
                        ang += std::f64::consts::PI;
                    }
                    let mid = [(f[0] + b[0]) / 2.0, (f[1] + b[1]) / 2.0];
                    let up = [-ang.sin(), ang.cos()];
                    self.dim_label([mid[0] + up[0] * 1.2, mid[1] + up[1] * 1.2], ang, 0.5, false, &t, 3.5, pick);
                }
                AnnotationKind::ForeshortenedDiameter { view, axis, edge, direct } => {
                    let (a0, a1) = (env.to_sheet(*view, axis[0]), env.to_sheet(*view, axis[1]));
                    let d = [a1[0] - a0[0], a1[1] - a0[1]];
                    let l = (d[0] * d[0] + d[1] * d[1]).sqrt();
                    if l < 1e-9 {
                        continue;
                    }
                    let u = [d[0] / l, d[1] / l];
                    let n = [-u[1], u[0]];
                    let dot = |p: [f64; 2], q: [f64; 2]| p[0] * q[0] + p[1] * q[1];
                    let e = env.to_sheet(*view, *edge);
                    let off = dot([e[0] - a0[0], e[1] - a0[1]], n);
                    let sg = if off < 0.0 { -1.0 } else { 1.0 };
                    let t = dot([a.position[0] - a0[0], a.position[1] - a0[1]], u);
                    let on_axis = [a0[0] + u[0] * t, a0[1] + u[1] * t];
                    let pe = [on_axis[0] + n[0] * off, on_axis[1] + n[1] * off];
                    // Extension line from the edge point to the dimension line.
                    let ev = [pe[0] - e[0], pe[1] - e[1]];
                    let el = (ev[0] * ev[0] + ev[1] * ev[1]).sqrt();
                    if el > 1.0 {
                        let w = [ev[0] / el, ev[1] / el];
                        self.lines.push(Polyline2 { points: vec![[e[0] + w[0], e[1] + w[1]], [pe[0] + w[0] * 2.0, pe[1] + w[1] * 2.0]], style: LineStyle::Thin, closed: false, pick });
                    }
                    let far = if *direct { [on_axis[0] - n[0] * off, on_axis[1] - n[1] * off] } else { [on_axis[0] - n[0] * sg * 6.0, on_axis[1] - n[1] * sg * 6.0] };
                    self.lines.push(Polyline2 { points: vec![pe, far], style: LineStyle::Thin, closed: false, pick });
                    self.arrow(pe, [n[0] * sg, n[1] * sg], 3.0);
                    if *direct {
                        self.arrow(far, [-n[0] * sg, -n[1] * sg], 3.0);
                    }
                    let model_off = {
                        let (m0, m1) = (axis[0], axis[1]);
                        let md = [m1[0] - m0[0], m1[1] - m0[1]];
                        let ml = (md[0] * md[0] + md[1] * md[1]).sqrt().max(1e-300);
                        ((edge[0] - m0[0]) * -md[1] / ml + (edge[1] - m0[1]) * md[0] / ml).abs()
                    };
                    let txt = env.text(a, model_off * 2.0, false, "Ø");
                    let mut ang = n[1].atan2(n[0]);
                    if ang > std::f64::consts::FRAC_PI_2 + 1e-9 || ang < -std::f64::consts::FRAC_PI_2 - 1e-9 {
                        ang += std::f64::consts::PI;
                    }
                    let up = [-ang.sin(), ang.cos()];
                    let mid = [(pe[0] + on_axis[0]) / 2.0 + up[0] * 1.2, (pe[1] + on_axis[1]) / 2.0 + up[1] * 1.2];
                    self.dim_label(mid, ang, 0.5, false, &txt, 3.5, pick);
                }
                AnnotationKind::DatumTarget { view, anchor, label, area } => {
                    let p = env.to_sheet(*view, *anchor);
                    let k = env.scale(*view);
                    let rot = env.rotation(*view);
                    let size = match area {
                        DatumTargetArea::Point => {
                            for d in [[1.0, 1.0], [1.0, -1.0]] {
                                self.lines.push(Polyline2 { points: vec![[p[0] - d[0] * 1.6, p[1] - d[1] * 1.6], [p[0] + d[0] * 1.6, p[1] + d[1] * 1.6]], style: LineStyle::Visible, closed: false, pick });
                            }
                            String::new()
                        }
                        DatumTargetArea::Circle { diameter } => {
                            let r = diameter / 2.0 * k;
                            let pts = (0..48).map(|i| {
                                let t = i as f64 / 48.0 * std::f64::consts::TAU;
                                [p[0] + r * t.cos(), p[1] + r * t.sin()]
                            });
                            self.lines.push(Polyline2 { points: pts.collect(), style: LineStyle::Centerline, closed: true, pick });
                            env.text(a, *diameter, false, "Ø").flat()
                        }
                        DatumTargetArea::Rectangle { width, height } => {
                            let (hx, hy) = (rotate2([width / 2.0 * k, 0.0], rot), rotate2([0.0, height / 2.0 * k], rot));
                            let c = |sx: f64, sy: f64| [p[0] + hx[0] * sx + hy[0] * sy, p[1] + hx[1] * sx + hy[1] * sy];
                            self.lines.push(Polyline2 { points: vec![c(-1.0, -1.0), c(1.0, -1.0), c(1.0, 1.0), c(-1.0, 1.0)], style: LineStyle::Centerline, closed: true, pick });
                            format!("{}×{}", env.text(a, *width, false, "").flat(), env.text(a, *height, false, "").flat())
                        }
                    };
                    // Callout bubble: size above, label below the divider.
                    let c = a.position;
                    let r = 5.5;
                    let pts = (0..40).map(|i| {
                        let t = i as f64 / 40.0 * std::f64::consts::TAU;
                        [c[0] + r * t.cos(), c[1] + r * t.sin()]
                    });
                    self.lines.push(Polyline2 { points: pts.collect(), style: LineStyle::Thin, closed: true, pick });
                    self.lines.push(Polyline2 { points: vec![[c[0] - r, c[1]], [c[0] + r, c[1]]], style: LineStyle::Thin, closed: false, pick });
                    self.texts.push(Text2 { pos: [c[0], c[1] - 2.6], height: 2.8, text: label.clone(), angle: 0.0, anchor: Anchor::Center, pick });
                    if !size.is_empty() {
                        let h = (2.4f64).min(8.5 / font::text_width(&size, 1.0).max(1e-9));
                        self.texts.push(Text2 { pos: [c[0], c[1] + 2.6], height: h, text: size, angle: 0.0, anchor: Anchor::Center, pick });
                    }
                    let d = [p[0] - c[0], p[1] - c[1]];
                    let l = (d[0] * d[0] + d[1] * d[1]).sqrt();
                    if l > r + 0.5 {
                        self.lines.push(Polyline2 { points: vec![[c[0] + d[0] / l * r, c[1] + d[1] / l * r], p], style: LineStyle::Thin, closed: false, pick });
                    }
                }
            }
        }
    }

    /// Width (mm) of a dimension label.
    pub fn dim_label_width(&self, t: &DimText, h: f64) -> f64 {
        let wm = font::text_width(&t.main, h);
        match (&t.upper, &t.lower) {
            (Some(u), Some(l)) => wm + if t.main.is_empty() { 0.0 } else { 1.0 } + font::text_width(u, h * 0.65).max(font::text_width(l, h * 0.65)),
            _ => wm,
        }
    }

    /// Place a dimension label at `at` (baseline start/center/end by
    /// `halign` 0/0.5/1 along `angle`; vertically centered when
    /// `vcenter`), with stacked tolerances and a basic-dimension frame.
    #[allow(clippy::too_many_arguments)]
    pub fn dim_label(&mut self, at: [f64; 2], angle: f64, halign: f64, vcenter: bool, t: &DimText, h: f64, pick: u32) {
        let (sn, cs) = angle.sin_cos();
        let u = [cs, sn];
        let v = [-sn, cs];
        let total = self.dim_label_width(t, h);
        let x0 = -halign * total;
        let stacked = t.upper.is_some() && t.lower.is_some();
        let base = if vcenter { if stacked && t.main.is_empty() { -h * 0.15 } else { -h / 2.0 } } else if stacked && t.main.is_empty() { h * 0.95 } else { 0.0 };
        let loc = |x: f64, y: f64| [at[0] + u[0] * x + v[0] * y, at[1] + u[1] * x + v[1] * y];
        let wm = font::text_width(&t.main, h);
        if !t.main.is_empty() {
            self.texts.push(Text2 { pos: loc(x0, base), height: h, text: t.main.clone(), angle, anchor: Anchor::BottomLeft, pick });
        }
        if let (Some(up), Some(lo)) = (&t.upper, &t.lower) {
            let xs = x0 + wm + if t.main.is_empty() { 0.0 } else { 1.0 };
            let hs = if t.main.is_empty() { h * 0.8 } else { h * 0.65 };
            let (yu, yl) = if t.main.is_empty() { (base + h * 0.05, base - h * 0.95) } else { (base + h * 0.5, base - h * 0.25) };
            self.texts.push(Text2 { pos: loc(xs, yu), height: hs, text: up.clone(), angle, anchor: Anchor::BottomLeft, pick });
            self.texts.push(Text2 { pos: loc(xs, yl), height: hs, text: lo.clone(), angle, anchor: Anchor::BottomLeft, pick });
        }
        if t.boxed {
            let (xa, xb, ya, yb) = (x0 - 0.8, x0 + total + 0.8, base - 0.8, base + h + 0.8);
            self.lines.push(Polyline2 { points: vec![loc(xa, ya), loc(xb, ya), loc(xb, yb), loc(xa, yb)], style: LineStyle::Thin, closed: true, pick });
        }
    }

    /// Linear dimension between sheet points p and q.
    pub fn linear_dimension(&mut self, p: [f64; 2], q: [f64; 2], offset: f64, orientation: DimOrientation, text: &str, pick: u32) {
        self.linear_dimension_text(p, q, offset, orientation, &DimText::plain(text), pick);
    }

    /// Linear dimension with a formatted label.
    pub fn linear_dimension_text(&mut self, p: [f64; 2], q: [f64; 2], offset: f64, orientation: DimOrientation, text: &DimText, pick: u32) {
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
        if text.upper.is_none() && !text.boxed {
            self.texts.push(Text2 { pos: mid, height: 3.5, text: text.main.clone(), angle, anchor: Anchor::BottomCenter, pick });
        } else {
            let mid = if text.boxed { [mid[0] + up[0] * 0.8, mid[1] + up[1] * 0.8] } else { mid };
            self.dim_label(mid, angle, 0.5, false, text, 3.5, pick);
        }
    }
}

/// Text positions (along the measured axis) of ordinate values at `along`,
/// at least `gap` apart: overlapping runs are spread around their mean.
pub fn ordinate_text_positions(along: &[f64], gap: f64) -> Vec<f64> {
    let mut order: Vec<usize> = (0..along.len()).collect();
    order.sort_by(|a, b| along[*a].total_cmp(&along[*b]));
    // Clusters of (indices in order, positions).
    let mut clusters: Vec<(Vec<usize>, f64)> = Vec::new();
    for &i in &order {
        clusters.push((vec![i], along[i]));
        // Merge while the last two clusters overlap.
        while clusters.len() >= 2 {
            let n = clusters.len();
            let (ref a, sa) = clusters[n - 2];
            let (ref b, sb) = clusters[n - 1];
            let end_a = sa + (a.len() as f64 - 1.0) * gap;
            if sb >= end_a + gap - 1e-9 {
                break;
            }
            let mut idx = a.clone();
            idx.extend(b.iter().copied());
            let mean = idx.iter().map(|k| along[*k]).sum::<f64>() / idx.len() as f64;
            let start = mean - (idx.len() as f64 - 1.0) * gap / 2.0;
            clusters.truncate(n - 2);
            clusters.push((idx, start));
        }
    }
    let mut out = vec![0.0; along.len()];
    for (idx, start) in clusters {
        for (j, i) in idx.into_iter().enumerate() {
            out[i] = start + j as f64 * gap;
        }
    }
    out
}

/// Marker key of the projection symbol cell in title block layouts.
pub const PROJECTION_CELL: &str = "<projection>";

/// Cells (field key, x, y, width, height in mm from the block's lower-left
/// corner) of the fixed title block layouts.
pub fn title_cells(style: TitleBlockStyle) -> &'static [(&'static str, f64, f64, f64, f64)] {
    match style {
        TitleBlockStyle::Iso => &[
            ("Legal owner", 0.0, 18.0, 60.0, 18.0),
            (PROJECTION_CELL, 0.0, 9.0, 25.0, 9.0),
            ("Scale", 25.0, 9.0, 35.0, 9.0),
            ("Material", 0.0, 0.0, 60.0, 9.0),
            ("Document type", 60.0, 27.0, 60.0, 9.0),
            ("Document status", 60.0, 18.0, 30.0, 9.0),
            ("Date", 90.0, 18.0, 30.0, 9.0),
            ("Drawn by", 60.0, 9.0, 60.0, 9.0),
            ("Approved by", 60.0, 0.0, 60.0, 9.0),
            ("Title", 120.0, 18.0, 60.0, 18.0),
            ("Supplementary title", 120.0, 9.0, 60.0, 9.0),
            ("Part number", 120.0, 0.0, 30.0, 9.0),
            ("Revision", 150.0, 0.0, 12.0, 9.0),
            ("Sheet", 162.0, 0.0, 18.0, 9.0),
        ],
        TitleBlockStyle::Ansi => &[
            ("Tolerances", 0.0, 14.0, 70.0, 26.0),
            ("Material", 0.0, 7.0, 70.0, 7.0),
            ("Finish", 0.0, 0.0, 70.0, 7.0),
            ("Drawn by", 70.0, 30.0, 40.0, 10.0),
            ("Drawn date", 110.0, 30.0, 30.0, 10.0),
            ("Checked by", 70.0, 20.0, 40.0, 10.0),
            ("Checked date", 110.0, 20.0, 30.0, 10.0),
            ("Eng approval", 70.0, 10.0, 40.0, 10.0),
            ("Eng date", 110.0, 10.0, 30.0, 10.0),
            ("Mfg approval", 70.0, 0.0, 40.0, 10.0),
            ("Mfg date", 110.0, 0.0, 30.0, 10.0),
            ("Company", 140.0, 28.0, 110.0, 12.0),
            ("Title", 140.0, 14.0, 110.0, 14.0),
            ("Size", 140.0, 7.0, 14.0, 7.0),
            ("CAGE code", 154.0, 7.0, 26.0, 7.0),
            ("Part number", 180.0, 7.0, 56.0, 7.0),
            ("Revision", 236.0, 7.0, 14.0, 7.0),
            ("Scale", 140.0, 0.0, 40.0, 7.0),
            ("Weight", 180.0, 0.0, 35.0, 7.0),
            ("Sheet", 215.0, 0.0, 35.0, 7.0),
        ],
        TitleBlockStyle::Simple | TitleBlockStyle::None => &[],
    }
}

/// Sheet rectangle [x0, y0, x1, y1] (mm) of a sheet's title block.
pub fn title_block_rect(sheet: &Sheet) -> Option<[f64; 4]> {
    let (w, _) = sheet.size.size_mm();
    let m = 10.0;
    let (bw, bh) = match sheet.title_style {
        TitleBlockStyle::None => return None,
        TitleBlockStyle::Simple => (180.0, 8.0 * ((sheet.title_block.fields.len() as f64) / 2.0).ceil()),
        TitleBlockStyle::Iso => (180.0, 36.0),
        TitleBlockStyle::Ansi => (250.0, 40.0),
    };
    Some([w - m - bw, m, w - m, m + bh])
}

/// Where a ray from the center of a `hw` x `hh` (half sizes) box in
/// direction `d` leaves the box.
fn box_exit(c: [f64; 2], hw: f64, hh: f64, d: [f64; 2]) -> [f64; 2] {
    let tx = if d[0].abs() > 1e-12 { hw / d[0].abs() } else { f64::INFINITY };
    let ty = if d[1].abs() > 1e-12 { hh / d[1].abs() } else { f64::INFINITY };
    let t = tx.min(ty);
    [c[0] - d[0] * t, c[1] - d[1] * t]
}

/// How annotations map to the sheet and how their texts are produced.
pub trait AnnotationEnv {
    /// View coordinates (meters) -> sheet (mm).
    fn to_sheet(&self, view: ViewId, p: [f64; 2]) -> [f64; 2];
    /// Sheet mm per model meter in a view.
    fn scale(&self, view: ViewId) -> f64;
    /// Rotation of a view on the sheet (radians).
    fn rotation(&self, view: ViewId) -> f64;
    /// Formatted value of a dimension.
    fn text(&self, a: &Annotation, value: f64, angular: bool, symbol: &str) -> DimText;
    /// Property fields expanded.
    fn expand(&self, s: &str) -> String;
    /// Displayed text of a callout (BOM item when linked).
    fn balloon_text(&self, a: &Annotation, text: &str) -> String;
}

/// A simple environment for tests and free sheets: identity mapping per
/// view (`scale` mm per m) and default formatting.
pub struct PlainEnv<'a> {
    pub drawing: &'a Drawing,
    pub scale: f64,
}

impl AnnotationEnv for PlainEnv<'_> {
    fn to_sheet(&self, _: ViewId, p: [f64; 2]) -> [f64; 2] {
        [p[0] * self.scale, p[1] * self.scale]
    }
    fn scale(&self, _: ViewId) -> f64 {
        self.scale
    }
    fn rotation(&self, _: ViewId) -> f64 {
        0.0
    }
    fn text(&self, a: &Annotation, value: f64, angular: bool, symbol: &str) -> DimText {
        super::format::dim_text(self.drawing, &a.format, value, angular, symbol)
    }
    fn expand(&self, s: &str) -> String {
        s.to_string()
    }
    fn balloon_text(&self, _: &Annotation, text: &str) -> String {
        text.to_string()
    }
}

/// Rotate a 2D vector counter-clockwise by `a` radians.
pub fn rotate2(v: [f64; 2], a: f64) -> [f64; 2] {
    super::views::rotate2(v, a)
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
