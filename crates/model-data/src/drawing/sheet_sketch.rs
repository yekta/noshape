//! Drawing sketch geometry (Onshape's sheet sketch tools): lines, circles,
//! arcs, rectangles, splines and points drawn on the sheet (sheet mm), with
//! drawing constraints, on the same engine as Part Studio sketches; plus
//! hatch regions (a closed profile filled with a hatch pattern).

use super::sheet::{LineStyle, Polyline2, SheetGeometry};
use noshape_sketch::{EntityId, Geometry, Sketch};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Line styles of drawing sketch geometry (SK16).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SketchLine {
    #[default]
    Solid,
    Phantom,
    Center,
    Dashed,
    Dotted,
}

impl SketchLine {
    pub const ALL: [SketchLine; 5] = [SketchLine::Solid, SketchLine::Phantom, SketchLine::Center, SketchLine::Dashed, SketchLine::Dotted];
    pub fn name(self) -> &'static str {
        match self {
            SketchLine::Solid => "Solid",
            SketchLine::Phantom => "Phantom",
            SketchLine::Center => "Center",
            SketchLine::Dashed => "Dashed",
            SketchLine::Dotted => "Dotted",
        }
    }
    fn line_style(self) -> LineStyle {
        match self {
            SketchLine::Solid => LineStyle::Thin,
            SketchLine::Phantom => LineStyle::Phantom,
            SketchLine::Center => LineStyle::Centerline,
            SketchLine::Dashed | SketchLine::Dotted => LineStyle::Hidden,
        }
    }
}

/// Per-entity sketch style: line style, color (None = black), and the sheet
/// layer (Move to ▸: a format layer's lines take its Formats pen, S19).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SketchStyle {
    pub line: SketchLine,
    pub color: Option<[u8; 3]>,
    #[serde(default, skip_serializing_if = "super::SheetLayer::is_drawing")]
    pub layer: super::SheetLayer,
}

/// A hatch region: a closed boundary (sheet mm) filled with a pattern.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HatchRegion {
    pub id: u64,
    pub boundary: Vec<[f64; 2]>,
    /// Islands left unhatched.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub holes: Vec<Vec<[f64; 2]>>,
    /// A region of a view (boundary in that view's coordinates, so the
    /// hatch moves with the view); None: sheet coordinates.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view: Option<super::ViewId>,
    /// Pattern group and index ([`super::hatch_pat::pattern`]).
    pub standard: usize,
    pub pattern: usize,
    /// Scale (1: the pattern's own size); angle in degrees added to the
    /// pattern's own.
    pub scale: f64,
    pub angle: f64,
    /// None: the drawing's hatch color (black).
    pub color: Option<[u8; 3]>,
}

/// The sheet's sketch: entities, styles, hatch regions.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SheetSketch {
    pub sketch: Sketch,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub styles: BTreeMap<u32, SketchStyle>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hatches: Vec<HatchRegion>,
}

impl PartialEq for SheetSketch {
    fn eq(&self, other: &Self) -> bool {
        self.styles == other.styles && self.hatches == other.hatches && serde_json::to_value(&self.sketch).ok() == serde_json::to_value(&other.sketch).ok()
    }
}

impl SheetSketch {
    pub fn is_empty(&self) -> bool {
        self.sketch.entity_count() == 0 && self.hatches.is_empty()
    }

    pub fn style(&self, id: EntityId) -> SketchStyle {
        self.styles.get(&id.0).copied().unwrap_or_default()
    }

    pub fn next_hatch_id(&self) -> u64 {
        self.hatches.iter().map(|h| h.id).max().unwrap_or(0) + 1
    }

    /// Draw the sketch: curves as polylines (`pick(entity)` gives their pick
    /// ids), points as small crosses, hatches (`hatch_pick(id)`).
    /// `to_sheet` maps view coordinates of view hatches to the sheet.
    pub fn render(&self, g: &mut SheetGeometry, pick: &dyn Fn(EntityId) -> u32, hatch_pick: &dyn Fn(u64) -> u32, to_sheet: &dyn Fn(super::ViewId, [f64; 2]) -> Option<[f64; 2]>) {
        for h in &self.hatches {
            let m = g.mark();
            let pk = hatch_pick(h.id);
            let map = |pts: &[[f64; 2]]| -> Option<Vec<[f64; 2]>> {
                match h.view {
                    Some(v) => pts.iter().map(|p| to_sheet(v, *p)).collect(),
                    None => Some(pts.to_vec()),
                }
            };
            let (Some(boundary), Some(holes)) = (map(&h.boundary), h.holes.iter().map(|x| map(x)).collect::<Option<Vec<_>>>()) else { continue };
            let h = &HatchRegion { boundary, holes, view: None, ..h.clone() };
            for s in hatch_pattern_holes(&h.boundary, &h.holes, h.standard, h.pattern, h.scale, h.angle) {
                g.lines.push(Polyline2 { points: vec![s[0], s[1]], style: LineStyle::Thin, closed: false, pick: pk });
            }
            if super::hatch_pat::pattern(h.standard, h.pattern).is_some_and(|p| p.families.is_empty()) {
                for i in 1..h.boundary.len().saturating_sub(1) {
                    g.fills.push(super::sheet::Fill2 { points: vec![h.boundary[0], h.boundary[i], h.boundary[i + 1]] });
                }
            }
            if let Some(c) = h.color {
                g.color_since(m, c);
            }
        }
        for e in self.sketch.entities() {
            let st = self.style(e.id);
            let m = g.mark();
            let pk = pick(e.id);
            match &e.geometry {
                Geometry::Point { pos } => {
                    // Only free sketch points (not curve end points) are drawn.
                    if !self.sketch.users_of_point(e.id).is_empty() {
                        continue;
                    }
                    let r = 0.6;
                    g.lines.push(Polyline2 { points: (0..12).map(|i| {
                        let t = i as f64 / 12.0 * std::f64::consts::TAU;
                        [pos.x + r * t.cos(), pos.y + r * t.sin()]
                    }).collect(), style: LineStyle::Thin, closed: true, pick: pk });
                }
                _ => {
                    // Construction and external (view geometry used for
                    // constraints) entities are not drawn.
                    if e.construction || e.external {
                        continue;
                    }
                    let Some(c) = self.sketch.curve(e.id) else { continue };
                    // Fine enough to read as a true curve zoomed in (live draws
                    // vectors; qa DWG-CIRCLE-FACETS: 0.02 mm showed 24-gons).
                    let pts: Vec<[f64; 2]> = c.tessellate(0.002).into_iter().map(|p| [p.x, p.y]).collect();
                    if pts.len() >= 2 {
                        let closed = matches!(c, noshape_sketch::Curve2::Circle { .. } | noshape_sketch::Curve2::Ellipse { .. });
                        g.lines.push(Polyline2 { points: pts, style: st.line.line_style(), closed, pick: pk });
                    }
                }
            }
            if let Some(c) = st.color {
                g.color_since(m, c);
            }
        }
    }
}

/// Hatch lines inside `boundary` for a pattern (turned by `angle` degrees
/// from its own, sized by `scale`).
pub fn hatch_pattern(boundary: &[[f64; 2]], standard: usize, pattern: usize, scale: f64, angle: f64) -> Vec<[[f64; 2]; 2]> {
    hatch_pattern_holes(boundary, &[], standard, pattern, scale, angle)
}

/// [`hatch_pattern`] leaving `holes` out (even-odd rule).
pub fn hatch_pattern_holes(boundary: &[[f64; 2]], holes: &[Vec<[f64; 2]>], standard: usize, pattern: usize, scale: f64, angle: f64) -> Vec<[[f64; 2]; 2]> {
    let p = super::hatch_pat::pattern(standard, pattern).unwrap_or(&super::hatch_pat::ANSI[0]);
    super::hatch_pat::lines(p, boundary, holes, scale, angle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use noshape_sketch::DVec2;

    #[test]
    fn sketch_renders_and_hatches_fill_a_square() {
        let mut sk = SheetSketch::default();
        let l = sk.sketch.add_line(DVec2::new(10.0, 10.0), DVec2::new(50.0, 10.0));
        sk.sketch.add_circle(DVec2::new(30.0, 30.0), 5.0);
        sk.styles.insert(l.0, SketchStyle { line: SketchLine::Center, color: Some([255, 0, 0]), ..Default::default() });
        let sq = vec![[0.0, 0.0], [20.0, 0.0], [20.0, 20.0], [0.0, 20.0]];
        sk.hatches.push(HatchRegion { id: 1, boundary: sq.clone(), holes: vec![], view: None, standard: 0, pattern: 0, scale: 1.0, angle: 0.0, color: None });
        let mut g = SheetGeometry::new(297.0, 210.0);
        sk.render(&mut g, &|_| 7, &|_| 9, &|_, p| Some(p));
        let hatch: Vec<_> = g.lines.iter().filter(|l| l.pick == 9).collect();
        // 20 mm square, ANSI31 3.175 mm apart at 45°: 20 * √2 / 3.175 ≈ 9 lines.
        assert!(hatch.len() >= 8 && hatch.len() <= 10, "{}", hatch.len());
        let sketch_lines: Vec<_> = g.lines.iter().enumerate().filter(|(_, l)| l.pick == 7).collect();
        assert_eq!(sketch_lines.len(), 2);
        assert!(sketch_lines.iter().any(|(i, l)| l.style == LineStyle::Centerline && g.line_color(*i) == Some([255, 0, 0])));
        // A cross pattern (ANSI37) has two families; turning by 90° keeps the count.
        let cross = hatch_pattern(&sq, 0, 6, 1.0, 0.0);
        assert!(cross.len() > hatch.len());
        assert!(hatch_pattern(&sq, 0, 0, 1.0, 90.0).len().abs_diff(hatch.len()) <= 1);
        // Round trip.
        let json = serde_json::to_string(&sk).unwrap();
        let back: SheetSketch = serde_json::from_str(&json).unwrap();
        assert!(back == sk);
    }
}
