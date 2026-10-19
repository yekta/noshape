//! Sketch text: glyph outlines of the bundled Liberation fonts (SIL Open
//! Font License 1.1, see `fonts/LICENSE-Liberation-OFL.txt`) laid out in a
//! text box and converted to sketch curves.
//!
//! A text box (Onshape `skText`) is a construction rectangle: its lower-left
//! corner is the start of the first baseline and its height is the distance
//! from the baseline to the ascender of the first line. The box keeps the
//! aspect ratio of the laid-out text through a [`ConstraintKind::LengthRatio`]
//! constraint, so dragging or dimensioning a corner scales the text. The
//! glyph curves themselves are derived geometry: they are regenerated from
//! the solved box whenever the sketch is evaluated (see
//! [`Sketch::add_text_outlines`]).

use crate::constraints::ConstraintKind;
use crate::entities::Geometry;
use crate::ids::EntityId;
use crate::nurbs::clamped_uniform_knots;
use crate::shapes::ShapeIds;
use crate::sketch::{Result, Sketch, SketchError};
use glam::DVec2;

/// Font families available for sketch text.
pub const FONTS: [&str; 3] = ["Liberation Sans", "Liberation Serif", "Liberation Mono"];

/// Default font family.
pub const DEFAULT_FONT: &str = "Liberation Sans";

/// Maximum number of characters in a text box (Onshape limit).
pub const MAX_CHARS: usize = 250;

/// Ids of glyph outline entities derived from text box `n` start at
/// `GLYPH_ID_BASE + n * GLYPH_ID_STRIDE`.
pub const GLYPH_ID_BASE: u32 = 1 << 30;
pub const GLYPH_ID_STRIDE: u32 = 1 << 16;

/// First glyph entity id of a text box.
pub fn glyph_id_base(text_id: u32) -> u32 {
    GLYPH_ID_BASE + (text_id % (1 << 13)) * GLYPH_ID_STRIDE
}

/// Text box id a derived glyph entity belongs to.
pub fn glyph_text_id(entity: EntityId) -> Option<u32> {
    (entity.0 >= GLYPH_ID_BASE).then(|| (entity.0 - GLYPH_ID_BASE) / GLYPH_ID_STRIDE)
}

static SANS: [&[u8]; 4] = [
    include_bytes!("../fonts/LiberationSans-Regular.ttf"),
    include_bytes!("../fonts/LiberationSans-Bold.ttf"),
    include_bytes!("../fonts/LiberationSans-Italic.ttf"),
    include_bytes!("../fonts/LiberationSans-BoldItalic.ttf"),
];
static SERIF: [&[u8]; 4] = [
    include_bytes!("../fonts/LiberationSerif-Regular.ttf"),
    include_bytes!("../fonts/LiberationSerif-Bold.ttf"),
    include_bytes!("../fonts/LiberationSerif-Italic.ttf"),
    include_bytes!("../fonts/LiberationSerif-BoldItalic.ttf"),
];
/// Noto Sans (SIL OFL 1.1): Onshape's default drawing font.
static NOTO_SANS: [&[u8]; 4] = [
    include_bytes!("../fonts/NotoSans-Regular.ttf"),
    include_bytes!("../fonts/NotoSans-Bold.ttf"),
    include_bytes!("../fonts/NotoSans-Italic.ttf"),
    include_bytes!("../fonts/NotoSans-BoldItalic.ttf"),
];
static MONO: [&[u8]; 4] = [
    include_bytes!("../fonts/LiberationMono-Regular.ttf"),
    include_bytes!("../fonts/LiberationMono-Bold.ttf"),
    include_bytes!("../fonts/LiberationMono-Italic.ttf"),
    include_bytes!("../fonts/LiberationMono-BoldItalic.ttf"),
];

fn font_data(font: &str, bold: bool, italic: bool) -> &'static [u8] {
    let f = font.to_ascii_lowercase();
    let family = if f.contains("noto") {
        &NOTO_SANS
    } else if f.contains("serif") && !f.contains("sans") {
        &SERIF
    } else if f.contains("mono") {
        &MONO
    } else {
        &SANS
    };
    family[usize::from(bold) + 2 * usize::from(italic)]
}

/// One piece of a glyph contour, in box units.
#[derive(Clone, Debug, PartialEq)]
pub enum TextSegment {
    Line(DVec2, DVec2),
    /// Quadratic or cubic Bézier control polygon.
    Bezier(Vec<DVec2>),
}

impl TextSegment {
    fn start(&self) -> DVec2 {
        match self {
            TextSegment::Line(a, _) => *a,
            TextSegment::Bezier(c) => c[0],
        }
    }
    fn end(&self) -> DVec2 {
        match self {
            TextSegment::Line(_, b) => *b,
            TextSegment::Bezier(c) => *c.last().unwrap_or(&DVec2::ZERO),
        }
    }
}

/// Laid-out text in box units: the first baseline starts at the origin and
/// the first line's ascender is at `y = 1`. Later lines go below.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TextLayout {
    /// Closed glyph contours.
    pub contours: Vec<Vec<TextSegment>>,
    /// Advance width of the longest line.
    pub width: f64,
    pub lines: usize,
}

impl TextLayout {
    /// Polylines approximating the contours (previews).
    pub fn polylines(&self, steps: usize) -> Vec<Vec<DVec2>> {
        let steps = steps.max(1);
        self.contours
            .iter()
            .map(|c| {
                let mut pts = Vec::new();
                for s in c {
                    match s {
                        TextSegment::Line(a, b) => {
                            if pts.is_empty() {
                                pts.push(*a);
                            }
                            pts.push(*b);
                        }
                        TextSegment::Bezier(ctrl) => {
                            if pts.is_empty() {
                                pts.push(ctrl[0]);
                            }
                            pts.extend((1..=steps).map(|i| de_casteljau(ctrl, i as f64 / steps as f64)));
                        }
                    }
                }
                pts
            })
            .collect()
    }
}

fn de_casteljau(pts: &[DVec2], t: f64) -> DVec2 {
    let mut v = pts.to_vec();
    while v.len() > 1 {
        for i in 0..v.len() - 1 {
            v[i] = v[i].lerp(v[i + 1], t);
        }
        v.pop();
    }
    v[0]
}

struct Outline {
    scale: f64,
    origin: DVec2,
    contours: Vec<Vec<TextSegment>>,
    cur: Vec<TextSegment>,
    last: DVec2,
    start: DVec2,
}

impl Outline {
    fn p(&self, x: f32, y: f32) -> DVec2 {
        self.origin + DVec2::new(x as f64, y as f64) * self.scale
    }
    fn flush(&mut self) {
        if (self.last - self.start).length() > 1e-9 && !self.cur.is_empty() {
            self.cur.push(TextSegment::Line(self.last, self.start));
        }
        if !self.cur.is_empty() {
            self.contours.push(std::mem::take(&mut self.cur));
        }
    }
}

impl ttf_parser::OutlineBuilder for Outline {
    fn move_to(&mut self, x: f32, y: f32) {
        self.flush();
        self.start = self.p(x, y);
        self.last = self.start;
    }
    fn line_to(&mut self, x: f32, y: f32) {
        let q = self.p(x, y);
        if (q - self.last).length() > 1e-9 {
            self.cur.push(TextSegment::Line(self.last, q));
        }
        self.last = q;
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let (c, q) = (self.p(x1, y1), self.p(x, y));
        if (q - self.last).length() > 1e-9 {
            self.cur.push(TextSegment::Bezier(vec![self.last, c, q]));
        }
        self.last = q;
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let (c1, c2, q) = (self.p(x1, y1), self.p(x2, y2), self.p(x, y));
        if (q - self.last).length() > 1e-9 {
            self.cur.push(TextSegment::Bezier(vec![self.last, c1, c2, q]));
        }
        self.last = q;
    }
    fn close(&mut self) {
        self.flush();
    }
}

/// Lays out `text` (newlines start new lines) in box units.
pub fn layout_text(text: &str, font: &str, bold: bool, italic: bool) -> TextLayout {
    let Ok(face) = ttf_parser::Face::parse(font_data(font, bold, italic), 0) else { return TextLayout::default() };
    let asc = (face.ascender() as f64).max(1.0);
    let scale = 1.0 / asc;
    let line_h = (face.ascender() as f64 - face.descender() as f64 + face.line_gap() as f64) * scale;
    let mut out = TextLayout::default();
    for (li, line) in text.chars().take(MAX_CHARS).collect::<String>().lines().enumerate() {
        let mut pen = DVec2::new(0.0, -line_h * li as f64);
        for ch in line.chars() {
            let Some(gid) = face.glyph_index(ch).or_else(|| face.glyph_index('?')) else { continue };
            let mut ol = Outline { scale, origin: pen, contours: Vec::new(), cur: Vec::new(), last: pen, start: pen };
            face.outline_glyph(gid, &mut ol);
            ol.flush();
            out.contours.extend(ol.contours.into_iter().filter(|c| c.len() >= 2 || matches!(c.first(), Some(TextSegment::Bezier(_)))));
            pen.x += face.glyph_hor_advance(gid).unwrap_or(0) as f64 * scale;
        }
        out.width = out.width.max(pen.x);
        out.lines = li + 1;
    }
    out
}

/// Placement of a text layout in the sketch: box corner, axis directions and
/// height (from the solved text box), plus Onshape's flip options.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextPlacement {
    /// Lower-left corner of the box (start of the first baseline).
    pub origin: DVec2,
    /// Unit vector along the baseline.
    pub x: DVec2,
    /// Unit vector from the baseline toward the ascender.
    pub y: DVec2,
    pub height: f64,
    /// Flip about the box's horizontal center line.
    pub mirror_horizontal: bool,
    /// Flip about the box's vertical center line.
    pub mirror_vertical: bool,
}

impl TextPlacement {
    /// Maps a layout point (box units) into sketch coordinates.
    pub fn map(&self, layout_width: f64, p: DVec2) -> DVec2 {
        let u = if self.mirror_vertical { layout_width - p.x } else { p.x };
        let v = if self.mirror_horizontal { 1.0 - p.y } else { p.y };
        self.origin + (self.x * u + self.y * v) * self.height
    }
}

/// Points and constraints of a text box created by [`Sketch::add_text_box`].
#[derive(Clone, Debug, PartialEq)]
pub struct TextBoxIds {
    /// Corners: lower-left, lower-right, upper-right, upper-left.
    pub points: [EntityId; 4],
    /// Construction lines: bottom, right, top, left.
    pub lines: [EntityId; 4],
    /// The aspect ratio constraint (bottom length / left length).
    pub ratio: Option<crate::ids::ConstraintId>,
    pub shape: ShapeIds,
}

impl Sketch {
    /// Creates a text box: a construction rectangle with its lower-left corner
    /// at `origin`, the given `height` and `aspect` (width / height), a
    /// horizontal bottom edge and an aspect-ratio constraint.
    pub fn add_text_box(&mut self, origin: DVec2, height: f64, aspect: f64) -> Result<TextBoxIds> {
        if !(height > 0.0 && height.is_finite()) {
            return Err(SketchError::InvalidGeometry("text height must be positive".into()));
        }
        let aspect = if aspect.is_finite() && aspect > 1e-6 { aspect } else { 0.5 };
        let width = height * aspect;
        let shape = self.three_point_rectangle(origin, origin + DVec2::X * width, origin + DVec2::Y * height)?;
        for l in &shape.curves {
            self.set_construction(*l, true)?;
        }
        let pts = [shape.points[0], shape.points[1], shape.points[2], shape.points[3]];
        let lines = [shape.curves[0], shape.curves[1], shape.curves[2], shape.curves[3]];
        let mut out = TextBoxIds { points: pts, lines, ratio: None, shape };
        if let Ok(c) = self.add_constraint(ConstraintKind::Horizontal { a: lines[0], b: None }) {
            out.shape.constraints.push(c);
        }
        // Left edge runs upper-left -> lower-left; its length is the height.
        let ratio = self.add_constraint(ConstraintKind::LengthRatio { a: lines[0], b: lines[3], ratio: aspect })?;
        out.shape.constraints.push(ratio);
        out.ratio = Some(ratio);
        Ok(out)
    }

    /// Placement of a text from its box corners (lower-left, lower-right,
    /// upper-right, upper-left).
    pub fn text_placement(&self, corners: &[EntityId], mirror_horizontal: bool, mirror_vertical: bool) -> Option<TextPlacement> {
        let p0 = self.point(*corners.first()?)?;
        let p1 = self.point(*corners.get(1)?)?;
        let p3 = self.point(*corners.get(3)?)?;
        let x = (p1 - p0).try_normalize()?;
        let up = p3 - p0;
        let height = up.length();
        // Keep a right-handed frame even if the box was flipped by dragging.
        let y = up.try_normalize().unwrap_or(x.perp());
        (height > 1e-12).then_some(TextPlacement { origin: p0, x, y, height, mirror_horizontal, mirror_vertical })
    }

    /// Adds the outlines of a laid-out text as fixed (external) curves.
    /// With `id_base`, entities get consecutive ids starting there (stable
    /// derived geometry, see [`Sketch::add_entity_with_id`]); otherwise ids
    /// are allocated normally. Returns the created curves.
    pub fn add_text_outlines(&mut self, layout: &TextLayout, place: &TextPlacement, id_base: Option<u32>) -> Vec<EntityId> {
        let mut next = id_base;
        let limit = id_base.map(|b| b.saturating_add(GLYPH_ID_STRIDE - 1));
        let mut add = |sk: &mut Sketch, g: Geometry| -> Option<EntityId> {
            match next.as_mut() {
                Some(n) => {
                    if limit.is_some_and(|l| *n > l) {
                        return None;
                    }
                    let id = EntityId(*n);
                    *n += 1;
                    sk.add_entity_with_id(id, g, false, true).ok()
                }
                None => sk.add_entity(g, false, true).ok(),
            }
        };
        let map = |p: DVec2| place.map(layout.width, p);
        let mut curves = Vec::new();
        for contour in &layout.contours {
            let Some(first) = contour.first() else { continue };
            let Some(start) = add(self, Geometry::Point { pos: map(first.start()) }) else { break };
            let mut prev = start;
            for (i, seg) in contour.iter().enumerate() {
                let last = i + 1 == contour.len();
                let end = if last && (seg.end() - first.start()).length() < 1e-9 {
                    start
                } else {
                    match add(self, Geometry::Point { pos: map(seg.end()) }) {
                        Some(p) => p,
                        None => break,
                    }
                };
                let g = match seg {
                    TextSegment::Line(..) => {
                        if end == prev {
                            continue;
                        }
                        Geometry::Line { start: prev, end }
                    }
                    TextSegment::Bezier(ctrl) => {
                        let mut control = vec![prev];
                        for q in &ctrl[1..ctrl.len() - 1] {
                            match add(self, Geometry::Point { pos: map(*q) }) {
                                Some(p) => control.push(p),
                                None => break,
                            }
                        }
                        control.push(end);
                        let degree = ctrl.len() - 1;
                        if control.len() != ctrl.len() {
                            break;
                        }
                        Geometry::BSpline { control, degree, knots: clamped_uniform_knots(ctrl.len(), degree), weights: None }
                    }
                };
                if let Some(c) = add(self, g) {
                    curves.push(c);
                }
                prev = end;
            }
        }
        curves
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_metrics() {
        let l = layout_text("Hi", DEFAULT_FONT, false, false);
        assert_eq!(l.lines, 1);
        assert!(l.width > 0.5 && l.width < 2.0, "width {}", l.width);
        // H: one contour, i: two contours.
        assert_eq!(l.contours.len(), 3);
        let bold = layout_text("Hi", DEFAULT_FONT, true, false);
        assert!(bold.width > l.width);
        let two = layout_text("A\nB", "Liberation Mono", false, true);
        assert_eq!(two.lines, 2);
        let ys: Vec<f64> = two.polylines(4).iter().flatten().map(|p| p.y).collect();
        assert!(ys.iter().cloned().fold(f64::INFINITY, f64::min) < -0.5, "second line below the first");
    }
}
