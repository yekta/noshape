//! Onshape's DXF / DWG layers (live exports samples/drw2-asbu-aux.dxf,
//! asbv-layouts.dxf: AC1032; qa5-sm-flat.dxf lists the full set): what each
//! exported element is (view edges, hatches, centermarks, annotations,
//! tables, the format), its layer name, line type and lineweight.

use super::SheetLayer;
use super::sheet::{LineStyle, SheetGeometry};

/// An Onshape export layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CadLayer {
    Visible,
    Hidden,
    TangentLines,
    TangentHidden,
    ViewHatches,
    SectionJogLines,
    Centermarks,
    Centerlines,
    SectionCuttingLine,
    DetailViewBorder,
    DetailViewParentBorder,
    BreakLines,
    VirtualSharps,
    AnnotationText,
    AnnotationLines,
    Tables,
    OriginIndicators,
    SketchedGeometry,
    Images,
    BendLinesUp,
    BendLinesDown,
    BendTangentLines,
    BorderFrame,
    BorderZones,
    TitleBlock,
}

impl CadLayer {
    /// The layer name. 2000+ files write "Visible" and "Hidden" in mixed
    /// case (live drw2-k-dxf2018); R12 / R14 write every name in capitals
    /// (drw2-k-dxfR14, qa5-sm-flat), truncated to 26 characters.
    pub fn name(self, upper: bool) -> &'static str {
        match self {
            CadLayer::Visible if upper => "VISIBLE",
            CadLayer::Visible => "Visible",
            CadLayer::Hidden if upper => "HIDDEN",
            CadLayer::Hidden => "Hidden",
            CadLayer::TangentLines => "TANGENT_LINES",
            CadLayer::TangentHidden => "TANGENT_HIDDEN",
            CadLayer::ViewHatches => "VIEW_HATCHES",
            CadLayer::SectionJogLines => "SECTION_JOG_LINES",
            CadLayer::Centermarks => "CENTERMARKS",
            CadLayer::Centerlines => "CENTERLINES",
            CadLayer::SectionCuttingLine => "SECTION_CUTTING_LINE",
            CadLayer::DetailViewBorder => "DETAIL_VIEW_BORDER",
            CadLayer::DetailViewParentBorder => "DETAIL_VIEW_PARENT_BORDER",
            CadLayer::BreakLines => "BREAK_LINES",
            CadLayer::VirtualSharps => "VIRTUAL_SHARPS",
            CadLayer::AnnotationText => "ANNOTATION_TEXT",
            CadLayer::AnnotationLines => "ANNOTATION_LINES",
            CadLayer::Tables => "TABLES",
            CadLayer::OriginIndicators => "ORIGIN_INDICATORS",
            CadLayer::SketchedGeometry => "SKETCHED_GEOMETRY",
            CadLayer::Images => "IMAGES",
            CadLayer::BendLinesUp => "SHEETMETAL_BEND_LINES_UP",
            CadLayer::BendLinesDown => "SHEETMETAL_BEND_LINES_DOWN",
            CadLayer::BendTangentLines if upper => "SHEETMETAL_BEND_TANGENT_LI",
            CadLayer::BendTangentLines => "SHEETMETAL_BEND_TANGENT_LINES",
            CadLayer::BorderFrame => "BORDER_FRAME",
            CadLayer::BorderZones => "BORDER_ZONES",
            CadLayer::TitleBlock => "TITLE_BLOCK",
        }
    }

    /// The layer's line type (live LAYER table).
    pub fn line_type(self) -> &'static str {
        match self {
            CadLayer::Hidden | CadLayer::TangentHidden => "HIDDEN",
            CadLayer::Centerlines | CadLayer::BendLinesUp | CadLayer::BendLinesDown => "CENTER",
            CadLayer::SectionCuttingLine | CadLayer::DetailViewParentBorder => "ONS_PHANTOM",
            _ => "Continuous",
        }
    }

    /// The layer's lineweight in 1/100 mm (live AC1032 group 370; -3 =
    /// default).
    pub fn line_weight(self) -> i16 {
        match self {
            CadLayer::Visible => 40,
            CadLayer::AnnotationText => 25,
            CadLayer::AnnotationLines | CadLayer::Centermarks | CadLayer::Centerlines => 13,
            CadLayer::SectionCuttingLine => 70,
            CadLayer::Tables => -3,
            _ => 15,
        }
    }

    /// The layer of an element drawn in `style` with no explicit tag.
    pub fn of_style(style: LineStyle) -> CadLayer {
        match style {
            LineStyle::Visible => CadLayer::Visible,
            LineStyle::Hidden => CadLayer::Hidden,
            LineStyle::Tangent | LineStyle::Phantom => CadLayer::TangentLines,
            LineStyle::Centerline => CadLayer::Centerlines,
            LineStyle::Thin => CadLayer::AnnotationLines,
            LineStyle::Border => CadLayer::BorderFrame,
            LineStyle::Section => CadLayer::SectionCuttingLine,
        }
    }

    fn of_sheet_layer(l: SheetLayer) -> Option<CadLayer> {
        match l {
            SheetLayer::Drawing => None,
            SheetLayer::BorderFrame => Some(CadLayer::BorderFrame),
            SheetLayer::BorderZones => Some(CadLayer::BorderZones),
            SheetLayer::TitleBlock => Some(CadLayer::TitleBlock),
        }
    }
}

/// The live text style of a kind of text (drw2-k-dxf2018; untagged texts
/// are Onshape-Standard, or Onshape-Standard-Bold when bold).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CadTextStyle {
    /// Title block property values.
    Fields,
    ViewLabel,
    Note,
    /// Hole table tags in the view.
    HoleTag,
}

impl CadTextStyle {
    pub fn name(self) -> &'static str {
        match self {
            CadTextStyle::Fields => "Onshape-Standard-Fields",
            CadTextStyle::ViewLabel => "Onshape-Standard-View-Label",
            CadTextStyle::Note => "Onshape-Standard-NoteStyle",
            CadTextStyle::HoleTag => "Onshape-Standard-Hole-Tag",
        }
    }
}

/// Texts of one text style (last span wins).
#[derive(Clone, Debug, PartialEq)]
pub struct CadTextStyleSpan {
    pub texts: std::ops::Range<usize>,
    pub style: CadTextStyle,
}

/// Lines, texts and fills of one export layer (last span wins).
#[derive(Clone, Debug, PartialEq)]
pub struct CadSpan {
    pub lines: std::ops::Range<usize>,
    pub texts: std::ops::Range<usize>,
    pub fills: std::ops::Range<usize>,
    pub layer: CadLayer,
}

impl SheetGeometry {
    /// Put everything added since `m` on `layer`.
    pub fn cad_since(&mut self, m: super::sheet::GeomMark, layer: CadLayer) {
        let span = CadSpan { lines: m.0..self.lines.len(), texts: m.1..self.texts.len(), fills: m.2..self.fills.len(), layer };
        if !(span.lines.is_empty() && span.texts.is_empty() && span.fills.is_empty()) {
            self.cad.push(span);
        }
    }

    /// Tag lines from `l0` and texts from `t0` one by one (None: untagged),
    /// as runs of equal layers.
    pub fn cad_runs(&mut self, l0: usize, lines: &[Option<CadLayer>], t0: usize, texts: &[Option<CadLayer>]) {
        for (k, list, base) in [(0u8, lines, l0), (1u8, texts, t0)] {
            let mut i = 0;
            while i < list.len() {
                let Some(layer) = list[i] else {
                    i += 1;
                    continue;
                };
                let mut j = i + 1;
                while j < list.len() && list[j] == Some(layer) {
                    j += 1;
                }
                let r = base + i..base + j;
                let (lines, texts) = if k == 0 { (r, 0..0) } else { (0..0, r) };
                self.cad.push(CadSpan { lines, texts, fills: 0..0, layer });
                i = j;
            }
        }
    }

    /// Put the texts from `t0` in text style `style`.
    pub fn cad_text_style_since(&mut self, t0: usize, style: CadTextStyle) {
        if t0 < self.texts.len() {
            self.cad_text_styles.push(CadTextStyleSpan { texts: t0..self.texts.len(), style });
        }
    }

    /// Text style of text `i` (None: Onshape-Standard / -Bold).
    pub fn cad_text_style(&self, i: usize) -> Option<CadTextStyle> {
        self.cad_text_styles.iter().rev().find(|s| s.texts.contains(&i)).map(|s| s.style)
    }

    /// Export layer of line `i`: its format layer, its tag, else its style's.
    pub fn cad_line_layer(&self, i: usize) -> CadLayer {
        self.line_sheet_layer(i)
            .and_then(CadLayer::of_sheet_layer)
            .or_else(|| self.cad.iter().rev().find(|s| s.lines.contains(&i)).map(|s| s.layer))
            .unwrap_or_else(|| CadLayer::of_style(self.lines[i].style))
    }

    /// Export layer of text `i` (untagged texts are annotation text).
    pub fn cad_text_layer(&self, i: usize) -> CadLayer {
        self.text_sheet_layer(i)
            .and_then(CadLayer::of_sheet_layer)
            .or_else(|| self.cad.iter().rev().find(|s| s.texts.contains(&i)).map(|s| s.layer))
            .unwrap_or(CadLayer::AnnotationText)
    }

    /// Export layer of fill `i`: its tag, else the layer of the line it
    /// touches (arrowheads go with their dimension, leader or cutting line).
    pub fn cad_fill_layer(&self, i: usize) -> CadLayer {
        if let Some(l) = self.fill_sheet_layer(i).and_then(CadLayer::of_sheet_layer).or_else(|| self.cad.iter().rev().find(|s| s.fills.contains(&i)).map(|s| s.layer)) {
            return l;
        }
        let pts = &self.fills[i].points;
        if pts.is_empty() {
            return CadLayer::AnnotationText;
        }
        let c = pts.iter().fold([0.0, 0.0], |a, p| [a[0] + p[0], a[1] + p[1]]);
        let c = [c[0] / pts.len() as f64, c[1] / pts.len() as f64];
        let d = |p: &[f64; 2]| (p[0] - c[0]).hypot(p[1] - c[1]);
        self.lines
            .iter()
            .enumerate()
            .filter_map(|(li, l)| l.points.iter().map(d).min_by(f64::total_cmp).map(|dist| (li, dist)))
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(li, _)| self.cad_line_layer(li))
            .unwrap_or(CadLayer::AnnotationText)
    }
}

/// A hatched section for DXF / DWG export (live: one HATCH entity, pattern
/// ANSI31, on VIEW_HATCHES): its boundary loops (sheet mm), the hatch
/// line spacing (mm) and the hatch lines drawn for it (which the HATCH
/// replaces).
#[derive(Clone, Debug, PartialEq)]
pub struct HatchRegion {
    pub loops: Vec<Vec<[f64; 2]>>,
    pub spacing: f64,
    pub lines: std::ops::Range<usize>,
}

/// Closed loops from a soup of boundary segments (ends within `tol`); None
/// when a chain doesn't close.
pub fn chain_loops(segs: &[[[f64; 2]; 2]], tol: f64) -> Option<Vec<Vec<[f64; 2]>>> {
    let near = |a: [f64; 2], b: [f64; 2]| (a[0] - b[0]).hypot(a[1] - b[1]) < tol;
    let mut left: Vec<[[f64; 2]; 2]> = segs.iter().copied().filter(|s| !near(s[0], s[1])).collect();
    let mut loops = Vec::new();
    while let Some(first) = left.pop() {
        let mut lp = vec![first[0], first[1]];
        loop {
            let end = *lp.last()?;
            if near(end, lp[0]) && lp.len() > 3 {
                lp.pop();
                break;
            }
            let i = left.iter().position(|s| near(s[0], end) || near(s[1], end))?;
            let s = left.swap_remove(i);
            lp.push(if near(s[0], end) { s[1] } else { s[0] });
        }
        loops.push(lp);
    }
    (!loops.is_empty()).then_some(loops)
}
