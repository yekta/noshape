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
    /// Tangent edges as phantom lines (long dash, two short dashes).
    Phantom,
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
            LineStyle::Hidden | LineStyle::Tangent | LineStyle::Phantom => 0.35,
            LineStyle::Centerline | LineStyle::Thin => 0.25,
        }
    }
    /// Dash pattern in millimeters (empty = solid).
    pub fn dashes(self) -> &'static [f64] {
        match self {
            LineStyle::Hidden => &[3.0, 1.5],
            LineStyle::Centerline => &[8.0, 1.5, 1.5, 1.5],
            LineStyle::Phantom => &[8.0, 1.5, 1.5, 1.5, 1.5, 1.5],
            LineStyle::Section => &[10.0, 2.0, 2.0, 2.0],
            _ => &[],
        }
    }
    pub fn layer(self) -> &'static str {
        match self {
            LineStyle::Visible => "VISIBLE",
            LineStyle::Hidden => "HIDDEN",
            LineStyle::Tangent => "TANGENT",
            LineStyle::Phantom => "PHANTOM",
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
    /// Editing grips of annotations (Onshape shows them on a hovered or
    /// selected dimension).
    pub grips: Vec<Grip>,
    /// Indices into `texts` of overridden dimension values (exports can
    /// underline them).
    pub overridden: Vec<usize>,
    /// Colored ranges (everything else is black ink); the last span that
    /// covers an element wins.
    pub colors: Vec<ColorSpan>,
    /// Dimension / annotation style (Drawing properties › Dimensions and
    /// Annotations), set by [`SheetGeometry::add_annotations`].
    pub style: SheetStyle,
    /// The annotation being drawn is a dimension (its arrowheads use the
    /// dimension style).
    in_dim: bool,
    /// The annotation being drawn has its arrows flipped (outside).
    flip_arrows: bool,
    /// The linear dimension being drawn: its extension line jogs, and where
    /// other dimensions' lines cross its lines (Break dimension).
    jogs: [Option<super::ExtJog>; 2],
    cuts: Vec<[f64; 2]>,
    /// Inserted images (drawn under the geometry; exports embed them).
    pub images: Vec<GeomImage>,
    /// Texts in a font other than the default (last span wins).
    pub fonts: Vec<FontSpan>,
    /// Colored areas drawn under all the ink (pending revision rows).
    pub backdrops: Vec<(Vec<[f64; 2]>, [u8; 3])>,
    /// Translucent areas drawn over all the ink (a release watermark):
    /// outline, colour, opacity. The colour mode leaves them as they are.
    pub overlays: Vec<(Vec<[f64; 2]>, [u8; 3], f32)>,
    /// Grey areas behind property fields, on screen only (Drawing
    /// properties › Notes › Hide property background turns them off).
    pub field_backdrops: Vec<Vec<[f64; 2]>>,
    /// Elements on a format layer (border frame, zones, title block; last
    /// span wins).
    pub layers: Vec<LayerSpan>,
    /// Lines drawn with their own pen (Adjust linestyle).
    pub pens: Vec<PenSpan>,
    /// What the elements are, for the DXF / DWG layers (`cad_layers`; last
    /// span wins, untagged ones by style).
    pub cad: Vec<super::cad_layers::CadSpan>,
    /// Text styles of the DXF / DWG export by kind of text (`cad_layers`).
    pub cad_text_styles: Vec<super::cad_layers::CadTextStyleSpan>,
    /// Hatched sections, written as HATCH entities by the DXF / DWG export.
    pub hatches: Vec<super::cad_layers::HatchRegion>,
    /// Element groups the DXF / DWG export writes as one object
    /// (dimensions), and their elements (`cad_objects`).
    pub cad_objects: Vec<super::cad_objects::CadObject>,
    pub cad_members: Vec<super::cad_objects::CadMember>,
    /// The projected edges of each view (its range of `lines`): a shaded
    /// view shows its picture instead, except while hovered or selected.
    pub view_edges: Vec<(super::ViewId, std::ops::Range<usize>)>,
    /// Callout leaders as drawn (annotation id, index in
    /// `CalloutStyle::all_leaders`, path), for picking their lines and nodes.
    pub leader_paths: Vec<(u64, usize, LeaderPath)>,
}

/// Chamfer dimension settings (Drawing properties › Dimensions › Chamfer).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChamferStyle {
    pub dim45: bool,
    pub content45: super::ChamferContent,
    pub dim: bool,
    pub content: super::ChamferContent,
    pub length_precision: Option<u8>,
    pub angle_precision: Option<u8>,
}

/// Lines, texts and fills on a sheet layer.
#[derive(Clone, Debug, PartialEq)]
pub struct LayerSpan {
    pub lines: std::ops::Range<usize>,
    pub texts: std::ops::Range<usize>,
    pub fills: std::ops::Range<usize>,
    pub layer: super::SheetLayer,
}

/// Annotation types with their own style (Drawing properties ›
/// Annotations › Notes, Callouts, Datums, ...).
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum AnnType {
    Notes,
    Callouts,
    Datums,
    GeometricTolerances,
    SurfaceFinish,
    HoleCallouts,
    FlagNotes,
    WeldSymbols,
    BendNotes,
}

impl AnnType {
    /// In the Annotations tab's order.
    pub const ALL: [AnnType; 9] = [
        AnnType::Notes,
        AnnType::FlagNotes,
        AnnType::Callouts,
        AnnType::Datums,
        AnnType::GeometricTolerances,
        AnnType::SurfaceFinish,
        AnnType::WeldSymbols,
        AnnType::BendNotes,
        AnnType::HoleCallouts,
    ];

    pub fn name(self) -> &'static str {
        match self {
            AnnType::Notes => "Notes",
            AnnType::Callouts => "Callouts",
            AnnType::Datums => "Datums",
            AnnType::GeometricTolerances => "Geometric tolerances",
            AnnType::SurfaceFinish => "Surface finish symbols",
            AnnType::HoleCallouts => "Hole/thread callouts",
            AnnType::FlagNotes => "Flag notes",
            AnnType::WeldSymbols => "Weld symbols",
            AnnType::BendNotes => "Bend notes",
        }
    }

    pub fn of(kind: &super::AnnotationKind) -> Option<Self> {
        use super::AnnotationKind as K;
        Some(match kind {
            K::Note { .. } => AnnType::Notes,
            K::Balloon { .. } => AnnType::Callouts,
            K::Datum { .. } | K::DatumTarget { .. } => AnnType::Datums,
            K::Gdt { .. } => AnnType::GeometricTolerances,
            K::SurfaceFinish { .. } => AnnType::SurfaceFinish,
            K::HoleCallout { .. } => AnnType::HoleCallouts,
            K::Weld { .. } => AnnType::WeldSymbols,
            _ => return None,
        })
    }
}

/// One annotation type's font, text height and color (None: the "All
/// annotations" value).
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AnnTypeStyle {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font: Option<TextFont>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<[u8; 3]>,
}

/// A text font (Drawing properties › Font): family, bold, italic.
#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct TextFont {
    #[serde(deserialize_with = "migrated_family")]
    pub family: String,
    #[serde(default)]
    pub bold: bool,
    #[serde(default)]
    pub italic: bool,
}

/// The drawing Font lists' families (live drawing/161, dom/drawing-
/// properties-annotations: these 22 in this order).
pub const FONT_FAMILIES: [&str; 22] = [
    "APMONO",
    "ARComp",
    "ARIsoP1",
    "ARItal",
    "ARMono",
    "ARSimp",
    "ARTxt",
    "Arimo",
    "DejaVu Sans",
    "DejaVu Sans Mono",
    "DejaVu Serif",
    "Noto Sans",
    "Noto Sans Chinese Simplified",
    "Noto Sans Chinese Simplified Light",
    "Noto Sans Chinese Traditional",
    "Noto Sans Chinese Traditional Light",
    "Noto Sans Japanese",
    "Noto Sans Korean",
    "Noto Sans UI",
    "Noto Serif",
    "Open Sans",
    "Tinos",
];

/// Onshape's default drawing font.
pub const DRAWING_DEFAULT_FONT: &str = "Noto Sans";

/// Families of earlier documents not on live's list, as their nearest one
/// on it (lead decision S28): Liberation Sans → Arimo, Liberation Serif →
/// Tinos (the same metrics), Liberation Mono → DejaVu Sans Mono; any other
/// unknown name is ARSimp, as live (asb 10-24: unknown fonts render in
/// ARSimp.shx and re-open as "ARSimp").
pub fn migrate_font_family(family: &str) -> &str {
    match family {
        "Liberation Sans" => "Arimo",
        "Liberation Serif" => "Tinos",
        "Liberation Mono" => "DejaVu Sans Mono",
        f if FONT_FAMILIES.contains(&f) => f,
        _ => "ARSimp",
    }
}

/// The SHX shape fonts of the list (live: no Bold / Italic for them).
pub fn is_shx(family: &str) -> bool {
    matches!(family, "APMONO" | "ARComp" | "ARIsoP1" | "ARItal" | "ARMono" | "ARSimp" | "ARTxt")
}

fn migrated_family<'de, D: serde::Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    let f = <String as serde::Deserialize>::deserialize(d)?;
    Ok(migrate_font_family(&f).to_string())
}

/// Texts drawn in a font.
#[derive(Clone, Debug, PartialEq)]
pub struct FontSpan {
    pub texts: std::ops::Range<usize>,
    pub font: TextFont,
}

/// An image on the sheet: lower-left corner and size (mm), the encoded
/// file, and (filled in by the app for PDF export) its decoded RGB pixels.
#[derive(Clone, Debug, PartialEq)]
pub struct GeomImage {
    pub pos: [f64; 2],
    pub size: [f64; 2],
    pub data_base64: std::sync::Arc<str>,
    pub rgb: Option<(u32, u32, std::sync::Arc<Vec<u8>>)>,
}

impl GeomImage {
    /// MIME type from the file's magic bytes.
    pub fn mime(&self) -> &'static str {
        let b = crate::sketch_feature::base64_decode(&self.data_base64.chars().take(16).collect::<String>()).unwrap_or_default();
        if b.starts_with(&[0xFF, 0xD8]) {
            "image/jpeg"
        } else if b.starts_with(b"GIF") {
            "image/gif"
        } else {
            "image/png"
        }
    }
}

/// Arrowhead types (Drawing properties › Dimensions / Annotations).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ArrowType {
    None,
    #[default]
    Filled,
    Unfilled,
    Open,
    Oblique,
    FilledDot,
}

impl ArrowType {
    pub const ALL: [ArrowType; 6] = [ArrowType::None, ArrowType::Filled, ArrowType::Unfilled, ArrowType::Open, ArrowType::Oblique, ArrowType::FilledDot];
    pub fn name(self) -> &'static str {
        match self {
            ArrowType::None => "None",
            ArrowType::Filled => "Filled",
            ArrowType::Unfilled => "Unfilled",
            ArrowType::Open => "Open",
            ArrowType::Oblique => "Oblique",
            ArrowType::FilledDot => "Filled dot",
        }
    }
}

/// Text height (mm), arrowhead length (mm) and type, and color of
/// dimensions and of other annotations.
#[derive(Clone, Debug, PartialEq)]
pub struct SheetStyle {
    pub dim_text: f64,
    pub dim_arrow: f64,
    pub dim_arrow_type: ArrowType,
    pub dim_color: Option<[u8; 3]>,
    pub ann_text: f64,
    pub ann_arrow: f64,
    pub ann_arrow_type: ArrowType,
    pub ann_color: Option<[u8; 3]>,
    /// Fonts (None: the default sans).
    pub dim_font: Option<TextFont>,
    pub ann_font: Option<TextFont>,
    /// Gaps and extensions (mm): text to dimension line, geometry to
    /// extension line, extension past the dimension line; horizontal text.
    pub text_gap: f64,
    pub geometry_gap: f64,
    pub extension: f64,
    pub text_horizontal: bool,
    /// Per annotation type overrides, and the datum triangle size (mm).
    pub ann_types: Vec<(AnnType, AnnTypeStyle)>,
    pub datum_size: f64,
    /// Callouts: text justification, stack direction and gap (mm).
    pub callout_justify: super::TextJustify,
    pub callout_stack: super::StackDirection,
    pub callout_stack_gap: f64,
    /// Datum target areas' hatch (standard, pattern).
    pub target_hatch: (usize, usize),
    /// Dimensions › Underline overridden dimensions (drawn on screen).
    pub underline_overridden: bool,
    /// Gaps and extensions › Lock the angle of all jogged dimensions to 45°.
    pub lock_jog_45: bool,
    /// Ordinate dimensions › Dimension style ISO: a small open circle at the
    /// end of each ordinate leader (live drawing/382).
    pub ordinate_iso: bool,
    /// An ISO (not ANSI) drawing: the DXF / DWG dimension styles are live's
    /// Onshape-ISO-25 family.
    pub iso: bool,
}

impl SheetStyle {
    /// An annotation type's own style (defaults when not set).
    pub fn type_style(&self, kind: &super::AnnotationKind) -> AnnTypeStyle {
        AnnType::of(kind).and_then(|t| self.ann_types.iter().find(|(k, _)| *k == t)).map(|(_, s)| s.clone()).unwrap_or_default()
    }
}

impl Default for SheetStyle {
    fn default() -> Self {
        Self { dim_text: 3.5, dim_arrow: 3.0, dim_arrow_type: ArrowType::Filled, dim_color: None, ann_text: 3.5, ann_arrow: 3.0, ann_arrow_type: ArrowType::Filled, ann_color: None, dim_font: None, ann_font: None, text_gap: 1.2, geometry_gap: 1.0, extension: 2.0, text_horizontal: false, ann_types: Vec::new(), datum_size: 3.0, callout_justify: super::TextJustify::Center, callout_stack: super::StackDirection::Auto, callout_stack_gap: 1.27, target_hatch: (0, 0), underline_overridden: true, lock_jog_45: false, ordinate_iso: false, iso: false }
    }
}

/// Lines drawn with their own width (mm) and dash pattern (last span wins).
#[derive(Clone, Debug, PartialEq)]
pub struct PenSpan {
    pub lines: std::ops::Range<usize>,
    pub width_mm: f64,
    pub dashes: Vec<f64>,
}

/// Lines, texts and fills drawn in a color (index ranges into the
/// geometry's vectors).
#[derive(Clone, Debug, PartialEq)]
pub struct ColorSpan {
    pub lines: std::ops::Range<usize>,
    pub texts: std::ops::Range<usize>,
    pub fills: std::ops::Range<usize>,
    pub rgb: [u8; 3],
}

/// Where the geometry ends (lines, texts, fills), for [`SheetGeometry::color_since`].
pub type GeomMark = (usize, usize, usize);

/// A grip of an annotation (sheet mm).
#[derive(Clone, Debug, PartialEq)]
pub struct Grip {
    pub pick: u32,
    pub pos: [f64; 2],
    pub kind: GripKind,
}

/// Where each annotation's geometry starts: (annotation, texts, lines, fills).
type Starts<'a> = [(&'a Annotation, usize, usize, usize)];
/// A point on a line with the segment ends and whether the line is closed.
type OnLine = ([f64; 2], [f64; 2], [f64; 2], bool);

impl SheetGeometry {
    /// The point a datum attached by `o` sits on, with the segment it lies
    /// on and whether that polyline is closed (a frame): from the geometry
    /// of the annotation laid out so far (`starts`, see `annotations`).
    fn attached_point(&self, starts: &Starts, o: &super::DatumOn) -> Option<OnLine> {
        let k = starts.iter().position(|s| s.0.id == o.ann)?;
        let l1 = starts.get(k + 1).map(|s| s.2).unwrap_or(self.lines.len());
        let pick = (o.ann & 0xFFFF_FFFF) as u32;
        let line = self.lines[starts[k].2..l1].iter().filter(|l| l.pick == pick).nth(o.line as usize)?;
        let n = line.points.len();
        let i = o.seg as usize;
        let (w0, w1) = (*line.points.get(i)?, if i + 1 < n { line.points[i + 1] } else if line.closed && n > 2 { line.points[0] } else { return None });
        Some(([w0[0] + (w1[0] - w0[0]) * o.t, w0[1] + (w1[1] - w0[1]) * o.t], w0, w1, line.closed))
    }

    /// A datum on a dimension hides the arrowheads it covers (help H:datum):
    /// the dimension's filled triangles with a corner within `r` of `at` are
    /// emptied (not removed, so later indices stay valid).
    fn hide_covered_arrows(&mut self, starts: &Starts, ann: u64, at: [f64; 2], r: f64) {
        let Some(k) = starts.iter().position(|s| s.0.id == ann) else { return };
        if !starts[k].0.kind.is_dimension() {
            return;
        }
        let f1 = starts.get(k + 1).map(|s| s.3).unwrap_or(self.fills.len());
        for f in &mut self.fills[starts[k].3..f1] {
            // An arrowhead (a filled triangle) with a corner under the datum's triangle.
            if f.points.len() == 3 && f.points.iter().any(|p| ((p[0] - at[0]).powi(2) + (p[1] - at[1]).powi(2)).sqrt() < r) {
                f.points.clear();
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GripKind {
    /// Where a dimension attaches to the geometry (Onshape: light-blue dot).
    Origin,
    /// End of a dimension line (orange dot).
    End,
    /// Arrow / flip handle on the dimension line (orange diamond).
    Handle,
    /// The two snap points of a jogged extension line (orange dots).
    Jog,
}

impl SheetGeometry {
    pub fn new(width: f64, height: f64) -> Self {
        Self { width, height, ..Default::default() }
    }

    /// The sheet layer of line / text / fill `i` (None: the drawing).
    pub fn line_sheet_layer(&self, i: usize) -> Option<super::SheetLayer> {
        self.layers.iter().rev().find(|s| s.lines.contains(&i)).map(|s| s.layer)
    }
    pub fn text_sheet_layer(&self, i: usize) -> Option<super::SheetLayer> {
        self.layers.iter().rev().find(|s| s.texts.contains(&i)).map(|s| s.layer)
    }
    pub fn fill_sheet_layer(&self, i: usize) -> Option<super::SheetLayer> {
        self.layers.iter().rev().find(|s| s.fills.contains(&i)).map(|s| s.layer)
    }

    /// DXF / DWG layer of line `i`: its sheet layer's, else by style.
    pub fn line_layer(&self, i: usize) -> &'static str {
        self.layers.iter().rev().find(|s| s.lines.contains(&i)).and_then(|s| s.layer.cad_layer()).unwrap_or_else(|| self.lines[i].style.layer())
    }

    /// DXF / DWG layer of text `i` (None: the annotation layer).
    pub fn text_layer(&self, i: usize) -> Option<&'static str> {
        self.layers.iter().rev().find(|s| s.texts.contains(&i)).and_then(|s| s.layer.cad_layer())
    }

    /// DXF / DWG layer of fill `i` (None: the annotation layer).
    pub fn fill_layer(&self, i: usize) -> Option<&'static str> {
        self.layers.iter().rev().find(|s| s.fills.contains(&i)).and_then(|s| s.layer.cad_layer())
    }

    /// Rotate everything added since `mark` by `angle` (radians) about `c`.
    pub fn rotate_since(&mut self, mark: GeomMark, c: [f64; 2], angle: f64) {
        let (s, co) = angle.sin_cos();
        let rot = |p: [f64; 2]| {
            let d = [p[0] - c[0], p[1] - c[1]];
            [c[0] + d[0] * co - d[1] * s, c[1] + d[0] * s + d[1] * co]
        };
        for l in &mut self.lines[mark.0..] {
            l.points.iter_mut().for_each(|p| *p = rot(*p));
        }
        for t in &mut self.texts[mark.1..] {
            t.pos = rot(t.pos);
            t.angle += angle;
        }
        for f in &mut self.fills[mark.2..] {
            f.points.iter_mut().for_each(|p| *p = rot(*p));
        }
    }

    pub fn mark(&self) -> GeomMark {
        (self.lines.len(), self.texts.len(), self.fills.len())
    }

    /// Color everything added since `m`.
    pub fn color_since(&mut self, m: GeomMark, rgb: [u8; 3]) {
        let span = ColorSpan { lines: m.0..self.lines.len(), texts: m.1..self.texts.len(), fills: m.2..self.fills.len(), rgb };
        if !(span.lines.is_empty() && span.texts.is_empty() && span.fills.is_empty()) {
            self.colors.push(span);
        }
    }

    pub fn text_font(&self, i: usize) -> Option<&TextFont> {
        self.fonts.iter().rev().find(|s| s.texts.contains(&i)).map(|s| &s.font)
    }

    /// Line `i`'s pen: width (mm) and dash pattern.
    pub fn line_pen(&self, i: usize) -> (f64, Vec<f64>) {
        match self.pens.iter().rev().find(|s| s.lines.contains(&i)) {
            Some(p) => (p.width_mm, p.dashes.clone()),
            None => (self.lines[i].style.width_mm(), self.lines[i].style.dashes().to_vec()),
        }
    }

    pub fn line_color(&self, i: usize) -> Option<[u8; 3]> {
        self.colors.iter().rev().find(|s| s.lines.contains(&i)).map(|s| s.rgb)
    }
    pub fn text_color(&self, i: usize) -> Option<[u8; 3]> {
        self.colors.iter().rev().find(|s| s.texts.contains(&i)).map(|s| s.rgb)
    }
    pub fn fill_color(&self, i: usize) -> Option<[u8; 3]> {
        self.colors.iter().rev().find(|s| s.fills.contains(&i)).map(|s| s.rgb)
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

    /// An arrowhead at `tip` pointing along `dir`; `len` is relative to
    /// the default 3 mm (scaled by the dimension / annotation style).
    pub fn arrow(&mut self, tip: [f64; 2], dir: [f64; 2], len: f64) {
        let (size, kind) = if self.in_dim { (self.style.dim_arrow, self.style.dim_arrow_type) } else { (self.style.ann_arrow, self.style.ann_arrow_type) };
        let len = len * size / 3.0;
        let l = (dir[0] * dir[0] + dir[1] * dir[1]).sqrt().max(1e-12);
        let (dx, dy) = (dir[0] / l, dir[1] / l);
        let w = len * 0.3;
        let base = [tip[0] - dx * len, tip[1] - dy * len];
        let (left, right) = ([base[0] - dy * w, base[1] + dx * w], [base[0] + dy * w, base[1] - dx * w]);
        match kind {
            ArrowType::None => {}
            ArrowType::Filled => self.fills.push(Fill2 { points: vec![tip, left, right] }),
            ArrowType::Unfilled => self.lines.push(Polyline2 { points: vec![tip, left, right], style: LineStyle::Thin, closed: true, pick: 0 }),
            ArrowType::Open => self.lines.push(Polyline2 { points: vec![left, tip, right], style: LineStyle::Thin, closed: false, pick: 0 }),
            ArrowType::Oblique => {
                // A 45° tick through the tip.
                let h = len * 0.5;
                let (ox, oy) = ((dx - dy) * std::f64::consts::FRAC_1_SQRT_2 * h, (dy + dx) * std::f64::consts::FRAC_1_SQRT_2 * h);
                self.lines.push(Polyline2 { points: vec![[tip[0] - ox, tip[1] - oy], [tip[0] + ox, tip[1] + oy]], style: LineStyle::Thin, closed: false, pick: 0 });
            }
            ArrowType::FilledDot => {
                let r = len * 0.25;
                let pts: Vec<[f64; 2]> = (0..12).map(|i| {
                    let t = i as f64 / 12.0 * std::f64::consts::TAU;
                    [tip[0] + r * t.cos(), tip[1] + r * t.sin()]
                }).collect();
                for i in 1..pts.len() - 1 {
                    self.fills.push(Fill2 { points: vec![pts[0], pts[i], pts[i + 1]] });
                }
            }
        }
    }

    /// Border frame with zone markers and an ISO-style title block.
    /// An imported DWG / DXF space: its geometry in its own colours (black,
    /// the foreground, stays the default ink) and texts; no border or title
    /// block.
    pub fn add_imported(&mut self, s: &super::imported::ImportedSpace) {
        for (pts, c) in &s.lines {
            let i = self.lines.len();
            self.lines.push(Polyline2 { points: pts.clone(), style: LineStyle::Thin, closed: false, pick: 0 });
            if *c != [0, 0, 0] {
                self.colors.push(ColorSpan { lines: i..i + 1, texts: 0..0, fills: 0..0, rgb: *c });
            }
        }
        for t in &s.texts {
            let i = self.texts.len();
            self.texts.push(Text2 { pos: t.pos, height: t.height, text: t.text.clone(), angle: t.angle, anchor: Anchor::BottomLeft, pick: 0 });
            if t.color != [0, 0, 0] {
                self.colors.push(ColorSpan { lines: 0..0, texts: i..i + 1, fills: 0..0, rgb: t.color });
            }
        }
    }

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
        // Onshape's templates (live asb 10-24 "template frames"): the frame
        // (`SheetSize::frame_rect`) and zone dividers across the band outside
        // it with the zone labels in it; nothing at the sheet edge (the
        // editor draws the paper's outline). Both ANSI and ISO templates
        // number zones from the right and letter them from the bottom.
        // Border "Do not include" (live drawing/820-a): only the frame stays;
        // no zone strip.
        let format = sheet.format;
        let [fx0, fy0, fx1, fy1] = sheet.size.frame_rect();
        self.rect(fx0, fy0, fx1, fy1, LineStyle::Border);
        if format.border {
            self.add_zones(sheet);
        }
        let value_of = |k: &str, v: &str| -> String {
            match k {
                // The sheet scale property ("----" until the first view, live 820).
                "Scale" if v.is_empty() => expand("{sheet.scale}"),
                "Size" if v.is_empty() => sheet.size.label().to_string(),
                _ => expand(v),
            }
        };
        match sheet.title_style {
            TitleBlockStyle::None => {}
            TitleBlockStyle::Simple => {
                // Two columns of fields, 180 mm wide, in the lower-right corner.
                let (tw, th) = (180.0, 8.0 * ((sheet.title_block.fields.len() as f64) / 2.0).ceil());
                let (x0, y0) = (fx1 - tw, fy0);
                self.rect(x0, y0, fx1, y0 + th, LineStyle::Border);
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
                        self.line([x0, cy + th / rows as f64], [fx1, cy + th / rows as f64], LineStyle::Thin);
                    }
                    self.text([cx + 1.5, cy + th / rows as f64 - 3.2], 1.8, k.to_uppercase(), Anchor::BottomLeft);
                    self.text([cx + 1.5, cy + 1.2], 3.0, value_of(k, v), Anchor::BottomLeft);
                }
            }
            style => self.onshape_title_block(sheet, style == TitleBlockStyle::Iso, projection, &value_of),
        }
    }

    /// Zone dividers and labels in the band between the sheet edge and the
    /// frame. Onshape's templates number zones from the right and letter them
    /// from the bottom (Start zones "Bottom right"); "Top left" numbers from
    /// the left and letters from the top. Letters skip O (live drawing/560:
    /// A0 runs A..L with I). Geometry as live's templates (asb 10-24
    /// "template frames", m = `frame_margin`): ANSI zones divide the sheet
    /// exactly, ISO / JIS ones are whole millimetres centred on it; the
    /// columns sit 0.05 m right like the frame. Dividers run from the frame
    /// to 0.05 m from the bottom / top edge, to 0.1 m from the left edge and
    /// to the right edge; labels at m / 2 from the bottom / top, 0.55 m from
    /// the left and 0.45 m from the right.
    fn add_zones(&mut self, sheet: &Sheet) {
        let (w, h) = (self.width, self.height);
        let [nx, ny] = sheet.format.zone_counts(sheet.size).map(|n| n as usize);
        let top_left = sheet.format.start == super::ZoneStart::TopLeft;
        let th = sheet.size.zone_label_height();
        let m = sheet.size.frame_margin();
        let [fx0, fy0, fx1, fy1] = sheet.size.frame_rect();
        // Zone pitch and the first divider's offset along a side of length `len`.
        let ansi = sheet.size.is_ansi() || matches!(sheet.size, super::SheetSize::Custom { .. });
        let grid = |len: f64, n: usize, shift: f64| {
            let step = if ansi { len / n as f64 } else { (len / n as f64).floor() };
            (step, (len - n as f64 * step) / 2.0 + shift)
        };
        if nx > 0 {
            let (step, x00) = grid(w, nx, 0.05 * m);
            for i in 1..nx {
                let x = x00 + step * i as f64;
                self.line([x, fy0], [x, 0.05 * m], LineStyle::Thin);
                self.line([x, fy1], [x, h - 0.05 * m], LineStyle::Thin);
            }
            for i in 0..nx {
                let x = x00 + step * (i as f64 + 0.5);
                let n = if top_left { i + 1 } else { nx - i };
                self.text([x, h - m / 2.0], th, n.to_string(), Anchor::Center);
                self.text([x, m / 2.0], th, n.to_string(), Anchor::Center);
            }
        }
        if ny > 0 {
            let (step, y00) = grid(h, ny, 0.0);
            for j in 1..ny {
                let y = y00 + step * j as f64;
                self.line([fx0, y], [0.1 * m, y], LineStyle::Thin);
                self.line([fx1, y], [w, y], LineStyle::Thin);
            }
            for j in 0..ny {
                let y = y00 + step * (j as f64 + 0.5);
                let letter = zone_letter(if top_left { ny - 1 - j } else { j });
                self.text([0.55 * m, y], th, letter.clone(), Anchor::Center);
                self.text([w - 0.45 * m, y], th, letter, Anchor::Center);
            }
        }
    }

    /// Onshape's title block (the ANSI and ISO templates share the layout,
    /// live drawing/156 and 189): the tolerance notes, DO NOT SCALE DRAWING,
    /// BREAK ALL SHARP EDGES AND REMOVE BURRS and the projection angle at the
    /// left; NAME (SIGNATURE on ISO) / DATE rows DRAWN, CHECKED, APPROVED, then
    /// MATERIAL / FINISH; the logo row, TITLE, SIZE / DWG NO. / REV. and
    /// SCALE / WEIGHT / SHEET at the right. Cell edges are fractions of the
    /// block measured on the live templates.
    fn onshape_title_block(&mut self, sheet: &Sheet, iso: bool, projection: Projection, value_of: &dyn Fn(&str, &str) -> String) {
        let Some([x0, y0, x1, y1]) = title_block_rect(sheet) else { return };
        let (bw, bh) = (x1 - x0, y1 - y0);
        let px = |u: f64| x0 + u * bw;
        let py = |v: f64| y0 + v * bh;
        // Column / row fractions (ANSI, ISO).
        let pick = |a: f64, i: f64| if iso { i } else { a };
        let left = pick(0.1976, 0.1885);
        let mid = pick(0.4940, 0.5179);
        let cols: Vec<f64> = if iso { vec![0.1885, 0.2705, 0.3644, 0.4590, 0.5179] } else { vec![0.1976, 0.2971, 0.4201, 0.4940] };
        let rows: [f64; 7] = if iso { [1.0, 0.882, 0.761, 0.640, 0.519, 0.398, 0.277] } else { [1.0, 0.878, 0.759, 0.640, 0.518, 0.399, 0.2805] };
        let left_rows = if iso { [1.0, 0.5, 0.379, 0.245, 0.166, 0.0] } else { [1.0, 0.5305, 0.4146, 0.2835, 0.2012, 0.0] };
        let split = pick(0.3462, 0.3525);
        let right_rows = if iso { [1.0, 0.748, 0.325, 0.127, 0.0] } else { [1.0, 0.747, 0.326, 0.134, 0.0] };
        let size_cols = if iso { [0.576, 0.9516] } else { [0.5543, 0.9706] };
        let scale_cols = if iso { [0.620, 0.8346] } else { [0.6033, 0.8258] };
        let k = bh / 38.1;
        // Text heights (live DXF, drw2 10-20 e): every one a multiple of the
        // label height L, 0.032 of the block's height (0.048 in on ANSI A,
        // 0.064 in on ISO A3; live asb 10-24 "template frames").
        let lab = 0.032 * bh;
        // Frame and grid (live 164: thin lines throughout).
        self.rect(x0, y0, x1, y1, LineStyle::Thin);
        self.line([px(left), y0], [px(left), y1], LineStyle::Thin);
        self.line([px(mid), y0], [px(mid), y1], LineStyle::Thin);
        for v in &left_rows[1..left_rows.len() - 1] {
            self.line([x0, py(*v)], [px(left), py(*v)], LineStyle::Thin);
        }
        for v in &rows[1..] {
            self.line([px(left), py(*v)], [px(mid), py(*v)], LineStyle::Thin);
        }
        for c in &cols[1..cols.len() - 1] {
            self.line([px(*c), py(rows[6])], [px(*c), y1], LineStyle::Thin);
        }
        self.line([px(split), y0], [px(split), py(rows[6])], LineStyle::Thin);
        for v in &right_rows[1..right_rows.len() - 1] {
            self.line([px(mid), py(*v)], [x1, py(*v)], LineStyle::Thin);
        }
        for c in size_cols {
            self.line([px(c), py(right_rows[3])], [px(c), py(right_rows[2])], LineStyle::Thin);
        }
        for c in scale_cols {
            self.line([px(c), y0], [px(c), py(right_rows[3])], LineStyle::Thin);
        }
        // Custom template › Language (live: only the labels are translated;
        // long translations wrap in their cell and some shrink).
        let lang = sheet.title_language;
        let tr = lang.strings();
        let en = lang.is_english();
        let label = |g: &mut Self, u: f64, v: f64, t: &str| g.text([px(u) + 0.8 * k, py(v) - 0.6 * k - lab], lab, t.to_string(), Anchor::BottomLeft);
        let label_h = |g: &mut Self, u: f64, v: f64, t: &str, h: f64| g.text([px(u) + 0.8 * k, py(v) - 0.6 * k - h], h, t.to_string(), Anchor::BottomLeft);
        // Left: tolerance notes, standard notes, projection.
        let top = left_rows[0];
        let units = sheet.title_field("Units").filter(|u| !u.is_empty()).unwrap_or(if iso { "MILLIMETERS" } else { "INCHES" });
        if en {
            label(self, 0.0, top, "UNLESS OTHERWISE SPECIFIED,");
            label(self, 0.0, top - 0.055, &format!("DIMENSIONS ARE IN {units}"));
        } else {
            let word = match units {
                "MILLIMETERS" => tr.millimeters,
                "INCHES" => tr.inches.unwrap_or("INCHES"),
                u => u,
            };
            let h = lab * tr.unless_h;
            // Live wraps at the full cell width (the text may touch the border).
            for (i, line) in wrap_text(&tr.unless.replace("{u}", word), h, left * bw).iter().enumerate() {
                label_h(self, 0.0, top - i as f64 * 0.055, line, h);
            }
        }
        let angular = tr.angular.to_string();
        if iso {
            self.text([px(0.11), py(0.80)], 0.9 * lab, angular, Anchor::BottomLeft);
        } else {
            for (i, t) in [".XX = ±.0-", ".XXX = ±.00-", ".XXXX = ±.000-"].iter().enumerate() {
                self.text([x0 + 1.0 * k, py(0.81 - i as f64 * 0.05)], lab, t.to_string(), Anchor::BottomLeft);
            }
            self.text([px(0.1), py(0.79)], 0.9 * lab, angular, Anchor::BottomLeft);
            self.text([px(0.1), py(0.74)], 0.9 * lab, tr.fractional.unwrap_or("FRACTIONAL = ±").to_string(), Anchor::BottomLeft);
        }
        self.text([x0 + 0.8 * k, py(left_rows[1] + 0.09)], lab * tr.surface_h, tr.surface_finish.to_string(), Anchor::BottomLeft);
        // The surface finish mark after the label (long translations push it right).
        let sf_x = px(0.098).max(x0 + 1.4 * k + font::text_width(tr.surface_finish, lab * tr.surface_h));
        let sf = [sf_x, py(left_rows[1] + 0.12)];
        self.lines.push(Polyline2 { points: vec![[sf[0], sf[1]], [sf[0] + 0.8 * k, sf[1] - 1.6 * k], [sf[0] + 2.4 * k, sf[1] + 1.6 * k], [sf[0] + 5.0 * k, sf[1] + 1.6 * k]], style: LineStyle::Thin, closed: false, pick: 0 });
        let centred = |g: &mut Self, v0: f64, v1: f64, h: f64, t: &str| g.text([px(left / 2.0), (py(v0) + py(v1)) / 2.0], h, t.to_string(), Anchor::Center);
        // 1.3 L, smaller in some languages (live).
        let dns = tr.do_not_scale;
        centred(self, left_rows[1], left_rows[2], 1.3 * lab * tr.dns_h, dns);
        if en {
            self.text([px(left / 2.0), py(left_rows[2]) - 1.4 * k], lab, "BREAK ALL SHARP EDGES AND".to_string(), Anchor::Center);
            self.text([px(left / 2.0), py(left_rows[3]) + 1.4 * k], lab, "REMOVE BURRS".to_string(), Anchor::Center);
        } else {
            let lines = wrap_text(tr.break_edges, lab, left * bw);
            let n = lines.len() as f64;
            let cy = (py(left_rows[2]) + py(left_rows[3])) / 2.0;
            for (i, l) in lines.iter().enumerate() {
                self.text([px(left / 2.0), cy + ((n - 1.0) / 2.0 - i as f64) * 2.8 * k], lab, l.clone(), Anchor::Center);
            }
        }
        let proj = if projection == Projection::FirstAngle { tr.first_angle } else { tr.third_angle.unwrap_or("THIRD ANGLE PROJECTION") };
        centred(self, left_rows[3], left_rows[4], lab, proj);
        self.projection_symbol([px(left / 2.0), py(left_rows[4] / 2.0)], projection, left_rows[4] * bh);
        // Middle: NAME (SIGNATURE) / DATE, DRAWN / CHECKED / APPROVED.
        let heads: Vec<&str> = if iso { vec![tr.name, tr.signature, tr.date] } else { vec![tr.name, tr.date] };
        for (i, h) in heads.iter().enumerate() {
            label(self, cols[i + 1], rows[0], h);
        }
        let fields: [(&str, &str, &str); 3] = [(tr.drawn, "Drawn by", "Drawn date"), (tr.checked, "Checked by", "Checked date"), (tr.approved, "Approved by", "Approved date")];
        // A field's value centred in its cell; several lines stack.
        let value_in = |g: &mut Self, u0: f64, u1: f64, v0: f64, v1: f64, key: &str, h: f64| {
            let v = value_of(key, sheet.title_field(key).unwrap_or(""));
            let lines: Vec<&str> = v.split('\n').filter(|l| !l.is_empty()).collect();
            let n = lines.len() as f64;
            let (cx, cy) = ((px(u0) + px(u1)) / 2.0, (py(v0) + py(v1)) / 2.0);
            for (i, l) in lines.iter().enumerate() {
                let y = cy + ((n - 1.0) / 2.0 - i as f64) * h * 1.6;
                g.text([cx, y], h, l.to_string(), Anchor::Center);
            }
        };
        for (r, (l, name, date)) in fields.iter().enumerate() {
            label_h(self, cols[0], rows[r + 1], l, lab * tr.signoff_h);
            value_in(self, cols[1], cols[2], rows[r + 2], rows[r + 1], name, 0.84 * lab);
            let dc = cols.len() - 2;
            value_in(self, cols[dc], cols[dc + 1], rows[r + 2], rows[r + 1], date, 0.84 * lab);
        }
        label(self, cols[0], rows[6], tr.material);
        label(self, split, rows[6], tr.finish);
        value_in(self, cols[0], split, 0.0, rows[6] - 0.08, "Material", 1.5 * lab);
        value_in(self, split, mid, 0.0, rows[6] - 0.08, "Finish", 1.5 * lab);
        // Right: the (empty) top row, TITLE, SIZE / DWG NO. / REV., SCALE /
        // WEIGHT / SHEET (live: no text in the top row).
        label(self, mid, right_rows[1], tr.title);
        value_in(self, mid, 1.0, right_rows[2], right_rows[1], "Title", 2.4 * lab);
        let small = 0.8 * lab;
        label_h(self, mid, right_rows[2], tr.size, small);
        label_h(self, size_cols[0], right_rows[2], tr.dwg_no, small);
        label_h(self, size_cols[1], right_rows[2], tr.rev, small);
        let size = value_of("Size", sheet.title_field("Size").unwrap_or(""));
        self.text([(px(mid) + px(size_cols[0])) / 2.0, (py(right_rows[3]) + py(right_rows[2])) / 2.0 - 0.6 * k], 3.0 * lab, size, Anchor::Center);
        value_in(self, size_cols[0], size_cols[1], right_rows[3], right_rows[2] - 0.05, "Part number", 3.0 * lab);
        value_in(self, size_cols[1], 1.0, right_rows[3], right_rows[2] - 0.05, "Revision", 3.0 * lab);
        label_h(self, mid, right_rows[3], tr.scale, small);
        label_h(self, scale_cols[0], right_rows[3], tr.weight, small);
        label_h(self, scale_cols[1], right_rows[3], tr.sheet, small);
        value_in(self, mid + 0.03, scale_cols[0], 0.0, right_rows[3] - 0.03, "Scale", 1.8 * lab);
        value_in(self, scale_cols[0], scale_cols[1], 0.0, right_rows[3] - 0.03, "Weight", 1.8 * lab);
        value_in(self, scale_cols[1] + 0.03, 1.0, 0.0, right_rows[3] - 0.03, "Sheet", 1.8 * lab);
    }

    /// First / third angle projection symbol (a truncated cone) centered at `c`.
    pub fn projection_symbol(&mut self, c: [f64; 2], projection: Projection, cell_h: f64) {
        // Live 820 (ISO A3, an 8.4 mm cell): circles r 3.7 / 1.85 mm, the cone
        // 7.7 mm long from 1.95 to 3.7 mm half height, 2.3 mm apart, centred;
        // the same proportions in other cells (live 831-en, ANSI A).
        let f = cell_h / 8.4;
        let (big, small, len, gap) = (3.7 * f, 1.85 * f, 7.7 * f, 2.3 * f);
        let half = (len + gap + 2.0 * big) / 2.0;
        // Third angle: end view left of the side view; first angle: right.
        let (circ_x, trap_x) = match projection {
            Projection::ThirdAngle => (c[0] - half + big, c[0] - half + 2.0 * big + gap),
            Projection::FirstAngle => (c[0] + half - big, c[0] - half),
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
        // Centre lines: the axis from the cone's far end to 0.8 mm past the
        // circle, and a vertical one through the circles.
        let over = 0.8 * f;
        let x0 = trap_x.min(circ_x - big - over);
        let x1 = (trap_x + len).max(circ_x + big + over);
        self.line([x0, y], [x1, y], LineStyle::Centerline);
        self.line([circ_x, y - big - over], [circ_x, y + big + over], LineStyle::Centerline);
    }

    /// An inspection item: the number in its border shape, left of `at`
    /// (the upper left corner of the annotation's text).
    fn inspection_bubble(&mut self, at: [f64; 2], n: u32, style: &InspectionStyle, pick: u32) {
        use super::InspectionShape as S;
        let (shape, h, border) = (style.shape, style.text_height, style.border);
        let m = self.mark();
        let text = n.to_string();
        let w = (text.chars().count() as f64 * h * 0.7).max(h);
        // Border size: at least that many characters wide (as callouts).
        let w = if style.chars == 0 { w } else { w.max(style.chars as f64 * h * 0.6) };
        let (rx, ry) = (w * 0.5 + border, h * 0.5 + border);
        let c = [at[0] - rx - 0.8, at[1]];
        let ring = |rx: f64, ry: f64| -> Vec<[f64; 2]> { (0..32).map(|i| {
            let t = i as f64 / 32.0 * std::f64::consts::TAU;
            [c[0] + rx * t.cos(), c[1] + ry * t.sin()]
        }).collect() };
        let pts = match shape {
            S::Circle => ring(rx.max(ry), rx.max(ry)),
            S::Oval => ring(rx + 1.0, ry),
            S::Rectangle => vec![[c[0] - rx, c[1] - ry], [c[0] + rx, c[1] - ry], [c[0] + rx, c[1] + ry], [c[0] - rx, c[1] + ry]],
            S::Triangle => vec![[c[0] - rx - 1.5, c[1] - ry], [c[0] + rx + 1.5, c[1] - ry], [c[0], c[1] + ry + 2.0]],
            S::Flag => vec![[c[0] - rx, c[1] - ry], [c[0] + rx, c[1] - ry], [c[0] + rx + 1.5, c[1]], [c[0] + rx, c[1] + ry], [c[0] - rx, c[1] + ry]],
        };
        let ink = style.color.unwrap_or([0, 0, 0]);
        if style.solid {
            // Solid fill: the shape filled in the border color, the number white.
            for i in 1..pts.len().saturating_sub(1) {
                self.fills.push(Fill2 { points: vec![pts[0], pts[i], pts[i + 1]] });
            }
        }
        self.lines.push(Polyline2 { points: pts, style: LineStyle::Thin, closed: true, pick });
        if style.color.is_some() || style.solid {
            self.color_since(m, ink);
        }
        let mt = self.mark();
        self.texts.push(Text2 { pos: c, height: h, text, angle: 0.0, anchor: Anchor::Center, pick });
        if let Some(f) = &style.font {
            self.fonts.push(FontSpan { texts: mt.1..self.texts.len(), font: f.clone() });
        }
        if style.solid {
            self.color_since(mt, [255, 255, 255]);
        } else if style.color.is_some() {
            self.color_since(mt, ink);
        }
    }

    /// Render the sheet's annotations (dimensions, notes, centermarks...).
    /// View-relative coordinates are mapped through `env`.
    /// ISO ordinate leader end: a small open circle (≈1.2 mm).
    fn ordinate_circle(&mut self, c: [f64; 2], pick: u32) {
        let points = (0..16).map(|i| {
            let t = i as f64 / 16.0 * std::f64::consts::TAU;
            [c[0] + 0.6 * t.cos(), c[1] + 0.6 * t.sin()]
        }).collect();
        self.lines.push(Polyline2 { points, style: LineStyle::Thin, closed: true, pick });
    }

    /// Break dimension: where linear dimensions' lines (extension lines and
    /// dimension lines) cross, the first of the two (in sheet order) that is
    /// broken gets a gap; only one of two crossing lines is broken.
    fn dimension_crossings(&self, sheet: &Sheet, env: &dyn AnnotationEnv) -> std::collections::HashMap<u64, Vec<[f64; 2]>> {
        let mut segs: Vec<DimSegments> = Vec::new();
        for a in &sheet.annotations {
            let AnnotationKind::LinearDimension { view, a: pa, b: pb, offset, orientation, .. } = &a.kind else { continue };
            let f = linear_frame(env.to_sheet(*view, *pa), env.to_sheet(*view, *pb), *offset, *orientation, &a.jogs, self.style.lock_jog_45, self.style.geometry_gap, self.style.extension);
            let mut s: Vec<[[f64; 2]; 2]> = f.ext.iter().flat_map(|e| e.windows(2).map(|w| [w[0], w[1]]).collect::<Vec<_>>()).collect();
            s.push([f.dp, f.dq]);
            segs.push((a.id, !a.unbroken, s));
        }
        let mut cuts: std::collections::HashMap<u64, Vec<[f64; 2]>> = std::collections::HashMap::new();
        for i in 0..segs.len() {
            for j in i + 1..segs.len() {
                let cut = if segs[i].1 { Some(segs[i].0) } else if segs[j].1 { Some(segs[j].0) } else { None };
                let Some(cut) = cut else { continue };
                for a in &segs[i].2 {
                    for b in &segs[j].2 {
                        if let Some(x) = crossing(*a, *b) {
                            cuts.entry(cut).or_default().push(x);
                        }
                    }
                }
            }
        }
        cuts
    }

    pub fn add_annotations(&mut self, sheet: &Sheet, env: &dyn AnnotationEnv) {
        // Where each annotation's geometry starts (inspection bubbles go
        // at the upper left of its text, after the loop).
        let mut starts: Vec<(&Annotation, usize, usize, usize)> = Vec::new();
        self.style = env.sheet_style();
        let mut cuts = self.dimension_crossings(sheet, env);
        let base = self.style.clone();
        for a in &sheet.annotations {
            // The annotation's own style (Styles panel) over the drawing's.
            self.style = base.clone();
            if let Some(st) = &a.style {
                st.apply(&mut self.style, a.kind.is_dimension());
            }
            let (dh, ah0) = (self.style.dim_text, self.style.ann_text);
            let pick = (a.id & 0xFFFF_FFFF) as u32;
            let ah = match a.style.as_ref().and_then(|s| s.height) {
                Some(h) => h,
                None => self.style.type_style(&a.kind).height.unwrap_or(ah0),
            };
            starts.push((a, self.texts.len(), self.lines.len(), self.fills.len()));
            self.in_dim = a.kind.is_dimension();
            self.flip_arrows = a.flip_arrows;
            (self.jogs, self.cuts) = match a.kind {
                AnnotationKind::LinearDimension { .. } => (a.jogs, cuts.remove(&a.id).unwrap_or_default()),
                _ => ([None; 2], Vec::new()),
            };
            match &a.kind {
                AnnotationKind::Note { text, height_mm, style } => {
                    let h = *height_mm;
                    let text = super::note::fractions(&env.expand(text));
                    let l = super::note::layout(&text, h, style, a.position);
                    let mark = self.mark();
                    for p in &l.pieces {
                        self.note_text(p.pos, h, &p.text, pick);
                        if let Some(i) = p.item.filter(|i| p.designator && style.flags.contains(i)) {
                            self.flag(p.pos, h, &p.text, style, i, pick);
                        }
                    }
                    for (at, w) in &l.runs {
                        if style.underline {
                            self.lines.push(Polyline2 { points: vec![[at[0], at[1] - h * 0.2], [at[0] + w, at[1] - h * 0.2]], style: LineStyle::Thin, closed: false, pick });
                        }
                        if style.strikethrough {
                            self.lines.push(Polyline2 { points: vec![[at[0], at[1] + h * 0.5], [at[0] + w, at[1] + h * 0.5]], style: LineStyle::Thin, closed: false, pick });
                        }
                    }
                    // Leaders leave a side of the box through a landing, or
                    // branch off another leader (`note_leader_paths`).
                    let targets: Vec<[f64; 2]> = style.leaders.iter().map(|ld| match ld.view {
                        Some(v) => env.to_sheet(v, ld.point),
                        None => ld.point,
                    }).collect();
                    for path in note_leader_paths(l.bbox, &style.leaders, &targets) {
                        let (t, last) = (path.points[path.points.len() - 1], path.points[path.points.len() - 2]);
                        self.lines.push(Polyline2 { points: path.points, style: LineStyle::Thin, closed: false, pick });
                        self.arrow(t, [t[0] - last[0], t[1] - last[1]], 3.0);
                    }
                    // A rotated note (no leaders) turns about its middle.
                    if style.angle != 0.0 && style.leaders.is_empty() {
                        self.rotate_since(mark, super::note::center(&l.bbox), style.angle);
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
                    let first = self.texts.len();
                    self.linear_dimension_text(p, q, *offset, *orientation, &t, pick);
                    if override_text.is_some() && self.texts.len() > first {
                        self.overridden.push(first);
                    }
                }
                AnnotationKind::RadialDimension { view, center, radius, diameter, angle, dim_line } => {
                    let c = env.to_sheet(*view, *center);
                    let r = radius * env.scale(*view);
                    let dir = [angle.cos(), angle.sin()];
                    let tip = [c[0] + dir[0] * r, c[1] + dir[1] * r];
                    let out = [c[0] + dir[0] * (r + 12.0), c[1] + dir[1] * (r + 12.0)];
                    let far = [c[0] - dir[0] * r, c[1] - dir[1] * r];
                    if self.flip_arrows {
                        // Flipped: arrows outside pointing in, no line inside.
                        self.lines.push(Polyline2 { points: vec![tip, out], style: LineStyle::Thin, closed: false, pick });
                        self.arrow(tip, [-dir[0], -dir[1]], 3.0);
                        if *diameter {
                            let ext = self.style.dim_arrow + 3.0;
                            self.lines.push(Polyline2 { points: vec![far, [far[0] - dir[0] * ext, far[1] - dir[1] * ext]], style: LineStyle::Thin, closed: false, pick });
                            self.arrow(far, dir, 3.0);
                        }
                    } else {
                        // Live drawing/363–368: a diameter has one arrow on the
                        // near side; with a dimension line the line runs on
                        // across the circle to the far edge.
                        let start = match (*diameter, *dim_line) {
                            (true, true) => far,
                            (true, false) => tip,
                            (false, _) => c,
                        };
                        self.lines.push(Polyline2 { points: vec![start, out], style: LineStyle::Thin, closed: false, pick });
                        self.arrow(tip, dir, 3.0);
                    }
                    // The flip node near the arrow.
                    self.grips.push(Grip { pick, pos: [tip[0] - dir[0] * 4.0, tip[1] - dir[1] * 4.0], kind: GripKind::Handle });
                    let t = if *diameter { env.text(a, radius * 2.0, false, "Ø") } else { env.text(a, *radius, false, "R") };
                    // Text on a short horizontal shoulder.
                    let left = dir[0] < 0.0;
                    let w = self.dim_label_width(&t, dh);
                    let sh = [out[0] + if left { -w - 2.0 } else { w + 2.0 }, out[1]];
                    self.lines.push(Polyline2 { points: vec![out, sh], style: LineStyle::Thin, closed: false, pick });
                    let at = [out[0] + if left { -1.0 } else { 1.0 }, out[1] + 0.8];
                    self.dim_label(at, 0.0, if left { 1.0 } else { 0.0 }, false, &t, dh, pick);
                }
                AnnotationKind::AngularDimension { view, vertex, a: pa, b: pb, radius, .. } => {
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
                    let e0 = [v[0] + radius * a0.cos(), v[1] + radius * a0.sin()];
                    let e1 = [v[0] + radius * a1.cos(), v[1] + radius * a1.sin()];
                    if self.flip_arrows {
                        // Flipped: arrows outside the arc ends pointing in, the arc runs on.
                        let ext = (self.style.dim_arrow + 3.0) / radius.max(1e-9);
                        for (from, to) in [(a0 - ext, a0), (a1, a1 + ext)] {
                            let seg: Vec<[f64; 2]> = (0..=6).map(|i| {
                                let t = from + (to - from) * i as f64 / 6.0;
                                [v[0] + radius * t.cos(), v[1] + radius * t.sin()]
                            }).collect();
                            self.lines.push(Polyline2 { points: seg, style: LineStyle::Thin, closed: false, pick });
                        }
                        self.arrow(e0, [-a0.sin(), a0.cos()], 3.0);
                        self.arrow(e1, [a1.sin(), -a1.cos()], 3.0);
                    } else if (a1 - a0) * radius > 7.0 {
                        self.arrow(e0, [a0.sin(), -a0.cos()], 3.0);
                        self.arrow(e1, [-a1.sin(), a1.cos()], 3.0);
                    }
                    // Flip nodes just inside the arc ends.
                    let k = 4.0 / radius.max(1e-9);
                    for t in [a0 + k, a1 - k] {
                        self.grips.push(Grip { pick, pos: [v[0] + radius * t.cos(), v[1] + radius * t.sin()], kind: GripKind::Handle });
                    }
                    let mid = (a0 + a1) / 2.0;
                    let t = env.text(a, a1 - a0, true, "");
                    self.dim_label([v[0] + (radius + 3.0) * mid.cos(), v[1] + (radius + 3.0) * mid.sin()], 0.0, 0.5, true, &t, dh, pick);
                }
                AnnotationKind::Centermark { view, center, size, angle, arms } => {
                    let c = env.to_sheet(*view, *center);
                    let k = env.scale(*view);
                    let rot = env.rotation(*view) + angle;
                    let (u, w) = (rotate2([1.0, 0.0], rot), rotate2([0.0, 1.0], rot));
                    let end = |d: [f64; 2], i: usize| {
                        let l = (size + arms[i]) * k;
                        [c[0] + d[0] * l, c[1] + d[1] * l]
                    };
                    let ends = [end(u, 0), end(w, 1), end([-u[0], -u[1]], 2), end([-w[0], -w[1]], 3)];
                    self.lines.push(Polyline2 { points: vec![ends[2], ends[0]], style: LineStyle::Centerline, closed: false, pick });
                    self.lines.push(Polyline2 { points: vec![ends[3], ends[1]], style: LineStyle::Centerline, closed: false, pick });
                    // Selected: orange grips at the arm ends, a blue one at the center.
                    for e in ends {
                        self.grips.push(Grip { pick, pos: e, kind: GripKind::Jog });
                    }
                    self.grips.push(Grip { pick, pos: c, kind: GripKind::Origin });
                }
                AnnotationKind::Centerline { view, a: pa, b: pb } => {
                    let p = env.to_sheet(*view, *pa);
                    let q = env.to_sheet(*view, *pb);
                    self.lines.push(Polyline2 { points: vec![p, q], style: LineStyle::Centerline, closed: false, pick });
                }
                AnnotationKind::CircleCenterline { view, points } => {
                    if let Some((c, r)) = AnnotationKind::circle_centerline(points) {
                        let c = env.to_sheet(*view, c);
                        let r = r * env.scale(*view);
                        let n = 96;
                        let pts: Vec<[f64; 2]> = (0..n).map(|i| {
                            let t = i as f64 / n as f64 * std::f64::consts::TAU;
                            [c[0] + r * t.cos(), c[1] + r * t.sin()]
                        }).collect();
                        self.lines.push(Polyline2 { points: pts, style: LineStyle::Centerline, closed: true, pick });
                    }
                }
                AnnotationKind::HoleCallout { view, center, radius } => {
                    let c = env.to_sheet(*view, *center);
                    let r = radius * env.scale(*view);
                    let lines = env.hole_callout(a);
                    let h = ah;
                    let t = a.position;
                    let d = [t[0] - c[0], t[1] - c[1]];
                    let l = (d[0] * d[0] + d[1] * d[1]).sqrt().max(1e-9);
                    let u = [d[0] / l, d[1] / l];
                    // Leader from the circle to a short shoulder at the text.
                    let on = [c[0] + u[0] * r, c[1] + u[1] * r];
                    let right = t[0] >= c[0];
                    let shoulder = [t[0] + if right { -4.0 } else { 4.0 }, t[1]];
                    self.lines.push(Polyline2 { points: vec![on, shoulder, t], style: LineStyle::Thin, closed: false, pick });
                    self.arrow(on, [-u[0], -u[1]], 3.0);
                    let w = lines.iter().map(|s| super::format::flatten_stacked(s).chars().count()).max().unwrap_or(0) as f64 * h * 0.62;
                    let x0 = if right { t[0] + 1.0 } else { t[0] - 1.0 - w };
                    let n = lines.len() as f64;
                    for (i, line) in lines.iter().enumerate() {
                        // Lines stacked around the shoulder (bottom-left anchored).
                        let y = t[1] + (n - 1.0) * h * 0.75 - i as f64 * h * 1.5 - h * 0.5;
                        match super::format::split_stacked(line) {
                            // Fit deviations stacked after the size, like a
                            // dimension's deviation tolerance.
                            (before, Some((up, lo)), after) => {
                                let hs = h * 0.65;
                                let xs = x0 + font::text_width(before, h);
                                let xa = xs + font::text_width(up, hs).max(font::text_width(lo, hs));
                                for (x, yy, hh, s) in [(x0, y, h, before), (xs, y + h * 0.5, hs, up), (xs, y - h * 0.25, hs, lo), (xa, y, h, after)] {
                                    if !s.is_empty() {
                                        self.texts.push(Text2 { pos: [x, yy], height: hh, text: s.to_string(), angle: 0.0, anchor: Anchor::BottomLeft, pick });
                                    }
                                }
                            }
                            _ => self.texts.push(Text2 { pos: [x0, y], height: h, text: line.clone(), angle: 0.0, anchor: Anchor::BottomLeft, pick }),
                        }
                    }
                }
                AnnotationKind::VirtualSharp { view, a1, b1, a2, b2 } => {
                    // Onshape's default style: a small cross at the intersection.
                    if let Some(x) = AnnotationKind::line_intersection(*a1, *b1, *a2, *b2) {
                        let c = env.to_sheet(*view, x);
                        let s = 1.5;
                        self.lines.push(Polyline2 { points: vec![[c[0] - s, c[1] - s], [c[0] + s, c[1] + s]], style: LineStyle::Thin, closed: false, pick });
                        self.lines.push(Polyline2 { points: vec![[c[0] - s, c[1] + s], [c[0] + s, c[1] - s]], style: LineStyle::Thin, closed: false, pick });
                    }
                }
                // Grouped callouts are drawn after the loop, at their dimension.
                AnnotationKind::Balloon { .. } if a.group.is_some() => {}
                AnnotationKind::Balloon { .. } => self.balloon(a, a.position, env, ah, pick),
                AnnotationKind::RevisionCallout { anchor, revision, group: None } => {
                    if let Some(label) = env.revision_label(revision) {
                        self.revision_callout(*anchor, label, ah, pick);
                    }
                }
                AnnotationKind::RevisionCallout { .. } => {}
                AnnotationKind::Weld { view, target, spec } => {
                    let map = |l: &super::Leader| match l.view {
                        Some(v) => env.to_sheet(v, l.point),
                        None => l.point,
                    };
                    let tip = match view {
                        Some(v) => env.to_sheet(*v, *target),
                        None => *target,
                    };
                    let tips: Vec<[f64; 2]> = std::iter::once(tip).chain(spec.leaders.iter().map(map)).collect();
                    super::weld::render(self, spec, &tips, a.position, ah * 0.75, !env.weld_ansi(), pick);
                }
                AnnotationKind::SurfaceFinish { anchor, value, leaders } => {
                    let p = *anchor;
                    // Leaders (Add leader) run from the symbol's point.
                    for l in leaders {
                        let t = l.view.map(|v| env.to_sheet(v, l.point)).unwrap_or(l.point);
                        self.lines.push(Polyline2 { points: vec![p, t], style: LineStyle::Thin, closed: false, pick });
                        self.arrow(t, [t[0] - p[0], t[1] - p[1]], self.style.ann_arrow);
                    }
                    self.lines.push(Polyline2 { points: vec![[p[0] - 3.0, p[1] + 5.0], p, [p[0] + 6.0, p[1] + 10.0], [p[0] + 14.0, p[1] + 10.0]], style: LineStyle::Thin, closed: false, pick });
                    self.lines.push(Polyline2 { points: vec![[p[0] - 3.0, p[1] + 5.0], [p[0] + 3.0, p[1] + 5.0]], style: LineStyle::Thin, closed: false, pick });
                    self.texts.push(Text2 { pos: [p[0] + 6.5, p[1] + 10.8], height: 2.5, text: env.expand(value), angle: 0.0, anchor: Anchor::BottomLeft, pick });
                }
                AnnotationKind::Gdt { anchor, frame, leaders, extra } => {
                    // The frame scales with the annotation text height; rows
                    // stack downward from the anchor row (live drawing/212–215).
                    let k = ah / 3.5;
                    let (h, rh) = (3.0 * k, 7.0 * k);
                    let (x0, y) = (anchor[0], anchor[1]);
                    let cell_w = |c: &str| (font::text_width(c, h) + 3.0 * k).max(7.0 * k);
                    let rows: Vec<&Vec<String>> = std::iter::once(frame).chain(extra.rows.iter()).collect();
                    let sym_w = rows.iter().filter_map(|r| r.first()).filter(|c| c.as_str() != "Composite").map(|c| cell_w(c)).fold(7.0 * k, f64::max);
                    let mut right = x0;
                    let mut sym_top = 0usize;
                    for (ri, row) in rows.iter().enumerate() {
                        let yb = y - ri as f64 * rh;
                        // A Composite row shares the symbol cell of the row(s) above.
                        let composite = ri > 0 && row.first().is_some_and(|c| c == "Composite" || c.is_empty());
                        if !composite {
                            sym_top = ri;
                        }
                        let span = rows[sym_top..].iter().skip(1).take_while(|r| r.first().is_some_and(|c| c == "Composite" || c.is_empty())).count();
                        if !composite {
                            let yl = yb - span as f64 * rh;
                            self.lines.push(Polyline2 { points: vec![[x0, yl], [x0 + sym_w, yl], [x0 + sym_w, yb + rh], [x0, yb + rh]], style: LineStyle::Thin, closed: true, pick });
                            if let Some(sym) = row.first() {
                                self.texts.push(Text2 { pos: [x0 + sym_w / 2.0, (yl + yb + rh) / 2.0], height: h, text: sym.clone(), angle: 0.0, anchor: Anchor::Center, pick });
                            }
                        }
                        let mut x = x0 + sym_w;
                        for cell in row.iter().skip(1).filter(|c| !c.is_empty()) {
                            let w = cell_w(cell);
                            self.lines.push(Polyline2 { points: vec![[x, yb], [x + w, yb], [x + w, yb + rh], [x, yb + rh]], style: LineStyle::Thin, closed: true, pick });
                            self.texts.push(Text2 { pos: [x + w / 2.0, yb + rh / 2.0], height: h, text: cell.clone(), angle: 0.0, anchor: Anchor::Center, pick });
                            x += w;
                        }
                        right = right.max(x);
                        // Prefix left of the frame, suffix right of its row.
                        if let Some(p) = extra.prefix.get(ri).filter(|p| !p.is_empty()) {
                            self.texts.push(Text2 { pos: [x0 - 1.0 * k - font::text_width(p, h), yb + rh / 2.0 - h / 2.0], height: h, text: p.clone(), angle: 0.0, anchor: Anchor::BottomLeft, pick });
                        }
                        if let Some(sfx) = extra.suffix.get(ri).filter(|p| !p.is_empty()) {
                            self.texts.push(Text2 { pos: [x + 1.0 * k, yb + rh / 2.0 - h / 2.0], height: h, text: sfx.clone(), angle: 0.0, anchor: Anchor::BottomLeft, pick });
                        }
                    }
                    let bottom = y - (rows.len() as f64 - 1.0) * rh;
                    // Upper text above, lower text below, left-aligned.
                    if !extra.upper.is_empty() {
                        self.texts.push(Text2 { pos: [x0, y + rh + 1.0 * k], height: h, text: extra.upper.clone(), angle: 0.0, anchor: Anchor::BottomLeft, pick });
                    }
                    if !extra.lower.is_empty() {
                        self.texts.push(Text2 { pos: [x0, bottom - 1.0 * k - h], height: h, text: extra.lower.clone(), angle: 0.0, anchor: Anchor::BottomLeft, pick });
                    }
                    // Leaders leave the nearer short side of the first row through
                    // a shoulder; All around / All over add a circle (two) there.
                    let (x1, ym) = (right, y + rh / 2.0);
                    for l in leaders {
                        let t = match l.view {
                            Some(v) => env.to_sheet(v, l.point),
                            None => l.point,
                        };
                        let to_right = t[0] > (x0 + x1) / 2.0;
                        let start = if to_right { [x1, ym] } else { [x0, ym] };
                        let sh = [start[0] + if to_right { 4.0 } else { -4.0 }, ym];
                        self.lines.push(Polyline2 { points: vec![start, sh, t], style: LineStyle::Thin, closed: false, pick });
                        self.arrow(t, [t[0] - sh[0], t[1] - sh[1]], 3.0);
                        let circle = |r: f64| (0..=24).map(|i| i as f64 / 24.0 * std::f64::consts::TAU).map(|a| [sh[0] + r * a.cos(), sh[1] + r * a.sin()]).collect::<Vec<_>>();
                        match extra.leader {
                            super::GdtLeader::Leader => {}
                            super::GdtLeader::AllAround => self.lines.push(Polyline2 { points: circle(1.2), style: LineStyle::Thin, closed: true, pick }),
                            super::GdtLeader::AllOver => {
                                self.lines.push(Polyline2 { points: circle(1.2), style: LineStyle::Thin, closed: true, pick });
                                self.lines.push(Polyline2 { points: circle(0.8), style: LineStyle::Thin, closed: true, pick });
                            }
                        }
                    }
                }
                AnnotationKind::Datum { anchor, label, on } => {
                    let (bw, bh) = ((font::text_width(label, ah) + 3.0).max(7.0), 7.0);
                    // On another annotation: the box follows it and the
                    // triangle sits on the line square below the box; past
                    // the end of an open line (an extension line) the line
                    // is carried on to it.
                    let (anchor, c, square) = match on.as_ref().and_then(|o| self.attached_point(&starts, o)) {
                        Some((p, w0, w1, closed)) => {
                            let c = [a.position[0] + p[0] - anchor[0], a.position[1] + p[1] - anchor[1]];
                            let d = [w1[0] - w0[0], w1[1] - w0[1]];
                            let dd = (d[0] * d[0] + d[1] * d[1]).max(1e-12);
                            let t = ((c[0] - w0[0]) * d[0] + (c[1] - w0[1]) * d[1]) / dd;
                            let t = if closed { t.clamp(0.0, 1.0) } else { t };
                            let q = [w0[0] + d[0] * t, w0[1] + d[1] * t];
                            if !(0.0..=1.0).contains(&t) {
                                let end = if t > 1.0 { w1 } else { w0 };
                                self.lines.push(Polyline2 { points: vec![end, q], style: LineStyle::Thin, closed: false, pick });
                            }
                            let n = [-d[1] / dd.sqrt(), d[0] / dd.sqrt()];
                            let side = if (c[0] - q[0]) * n[0] + (c[1] - q[1]) * n[1] < 0.0 { -1.0 } else { 1.0 };
                            if let Some(o) = on {
                                self.hide_covered_arrows(&starts, o.ann, q, self.style.datum_size * 0.6);
                            }
                            (q, c, Some([n[0] * side, n[1] * side]))
                        }
                        None => (*anchor, a.position, None),
                    };
                    self.lines.push(Polyline2 { points: vec![[c[0] - bw / 2.0, c[1] - bh / 2.0], [c[0] + bw / 2.0, c[1] - bh / 2.0], [c[0] + bw / 2.0, c[1] + bh / 2.0], [c[0] - bw / 2.0, c[1] + bh / 2.0]], style: LineStyle::Thin, closed: true, pick });
                    self.texts.push(Text2 { pos: c, height: ah, text: label.clone(), angle: 0.0, anchor: Anchor::Center, pick });
                    let d = [c[0] - anchor[0], c[1] - anchor[1]];
                    let l = (d[0] * d[0] + d[1] * d[1]).sqrt();
                    if l > 1e-6 {
                        let u = square.unwrap_or([d[0] / l, d[1] / l]);
                        let w = [-u[1], u[0]];
                        // Datum size: the triangle's height (base 1.2 x).
                        let ds = self.style.datum_size;
                        let apex = [anchor[0] + u[0] * ds, anchor[1] + u[1] * ds];
                        self.fills.push(Fill2 { points: vec![[anchor[0] + w[0] * ds * 0.6, anchor[1] + w[1] * ds * 0.6], apex, [anchor[0] - w[0] * ds * 0.6, anchor[1] - w[1] * ds * 0.6]] });
                        // Leader from the triangle to the box's near edge
                        // (`box_exit` walks from the centre against `d`).
                        let hit = box_exit(c, bw / 2.0, bh / 2.0, u);
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
                    if self.style.ordinate_iso {
                        self.ordinate_circle(end, pick);
                    }
                    let mut ang = n[1].atan2(n[0]);
                    let mut halign = if sg > 0.0 { 0.0 } else { 1.0 };
                    if ang > std::f64::consts::FRAC_PI_2 + 1e-9 || ang < -std::f64::consts::FRAC_PI_2 - 1e-9 {
                        ang += std::f64::consts::PI;
                        halign = 1.0 - halign;
                    }
                    let at = [end[0] + n[0] * sg * 1.0, end[1] + n[1] * sg * 1.0];
                    self.dim_label(at, ang, halign, true, &t, dh, pick);
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
                    let other = lu * (1.0 - cosang * cosang).max(0.0).sqrt();
                    let ang = cosang.acos();
                    // Drawing properties › Dimensions › Chamfer content and
                    // precisions (the style is chosen when placing: Dimensions
                    // style places two leg dimensions instead).
                    let c = env.chamfer_style();
                    let is45 = (ang.to_degrees() - 45.0).abs() < 0.05;
                    let content = if is45 { c.content45 } else { c.content };
                    let mut la = a.clone();
                    if c.length_precision.is_some() {
                        la.format.precision = c.length_precision;
                    }
                    let len = |m: f64| env.text(&la, m, false, "").main;
                    let deg = format!("{}°", super::format::format_number(ang.to_degrees(), c.angle_precision.or(Some(1)), 1));
                    let mut t = env.text(a, dist, false, "");
                    t.main = match content {
                        super::ChamferContent::AngleXLength => format!("{deg} X {}", len(dist)),
                        super::ChamferContent::LengthXAngle => format!("{} X {deg}", len(dist)),
                        super::ChamferContent::LengthXLength => format!("{} X {}", len(dist), len(other)),
                        super::ChamferContent::CLength if is45 => format!("C{}", len(dist)),
                        super::ChamferContent::CLength => format!("{} X {deg}", len(dist)),
                    };
                    let e = a.position;
                    let left = e[0] < m[0];
                    let wtxt = self.dim_label_width(&t, dh);
                    let sh = [e[0] + if left { -wtxt - 2.0 } else { wtxt + 2.0 }, e[1]];
                    self.lines.push(Polyline2 { points: vec![m, e, sh], style: LineStyle::Thin, closed: false, pick });
                    let d = [m[0] - e[0], m[1] - e[1]];
                    self.arrow(m, d, 3.0);
                    self.dim_label([e[0] + if left { -1.0 } else { 1.0 }, e[1] + 0.8], 0.0, if left { 1.0 } else { 0.0 }, false, &t, dh, pick);
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
                        let (gap, ext) = (self.style.geometry_gap, self.style.extension);
                        self.lines.push(Polyline2 { points: vec![[c[0] + (r0 + gap) * ct, c[1] + (r0 + gap) * st], [c[0] + (rr + ext) * ct, c[1] + (rr + ext) * st]], style: LineStyle::Thin, closed: false, pick });
                        let tip = [c[0] + rr * ct, c[1] + rr * st];
                        let s = if (k == 0) == (sweep > 0.0) { -1.0 } else { 1.0 };
                        self.arrow(tip, [-st * s, ct * s], 3.0);
                    }
                    let tm = a0 + sweep / 2.0;
                    let t = env.text(a, radius * sweep.abs(), false, "⌒");
                    self.dim_label([c[0] + (rr + 4.0) * tm.cos(), c[1] + (rr + 4.0) * tm.sin()], 0.0, 0.5, true, &t, dh, pick);
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
                    let text_pos = ordinate_text_positions(&along, dh + 1.5);
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
                        if self.style.ordinate_iso {
                            self.ordinate_circle(at(s1, level), pick);
                        }
                        self.dim_label(at(s1, level + sg * 1.0), ang, halign, true, &t, dh, pick);
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
                    self.dim_label([mid[0] + up[0] * 1.2, mid[1] + up[1] * 1.2], ang, 0.5, false, &t, dh, pick);
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
                    self.dim_label(mid, ang, 0.5, false, &txt, dh, pick);
                }
                AnnotationKind::DatumTarget { view, anchor, label, area } => {
                    let p = env.to_sheet(*view, *anchor);
                    let k = env.scale(*view);
                    let rot = env.rotation(*view);
                    let size = match area {
                        DatumTargetArea::Point | DatumTargetArea::Custom => {
                            for d in [[1.0, 1.0], [1.0, -1.0]] {
                                self.lines.push(Polyline2 { points: vec![[p[0] - d[0] * 1.6, p[1] - d[1] * 1.6], [p[0] + d[0] * 1.6, p[1] + d[1] * 1.6]], style: LineStyle::Visible, closed: false, pick });
                            }
                            String::new()
                        }
                        DatumTargetArea::Dot => {
                            let pts: Vec<[f64; 2]> = (0..16).map(|i| {
                                let t = i as f64 / 16.0 * std::f64::consts::TAU;
                                [p[0] + 0.7 * t.cos(), p[1] + 0.7 * t.sin()]
                            }).collect();
                            for i in 1..pts.len() - 1 {
                                self.fills.push(Fill2 { points: vec![pts[0], pts[i], pts[i + 1]] });
                            }
                            String::new()
                        }
                        DatumTargetArea::Circle { diameter } => {
                            let r = diameter / 2.0 * k;
                            let pts = (0..48).map(|i| {
                                let t = i as f64 / 48.0 * std::f64::consts::TAU;
                                [p[0] + r * t.cos(), p[1] + r * t.sin()]
                            });
                            let pts: Vec<[f64; 2]> = pts.collect();
                            self.target_hatch(&pts, pick);
                            self.lines.push(Polyline2 { points: pts, style: LineStyle::Centerline, closed: true, pick });
                            env.text(a, *diameter, false, "Ø").flat()
                        }
                        DatumTargetArea::Rectangle { width, height } => {
                            let (hx, hy) = (rotate2([width / 2.0 * k, 0.0], rot), rotate2([0.0, height / 2.0 * k], rot));
                            let c = |sx: f64, sy: f64| [p[0] + hx[0] * sx + hy[0] * sy, p[1] + hx[1] * sx + hy[1] * sy];
                            let pts = vec![c(-1.0, -1.0), c(1.0, -1.0), c(1.0, 1.0), c(-1.0, 1.0)];
                            self.target_hatch(&pts, pick);
                            self.lines.push(Polyline2 { points: pts, style: LineStyle::Centerline, closed: true, pick });
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
                    let opts = a.target.as_deref().cloned().unwrap_or_default();
                    // Moveable styles: a pointer on the bubble (horizontal, or
                    // turned toward the target).
                    if opts.style != super::TargetStyle::Inside && l > 1e-9 {
                        let u = if opts.style == super::TargetStyle::MoveableHorizontal { [if d[0] < 0.0 { -1.0 } else { 1.0 }, 0.0] } else { [d[0] / l, d[1] / l] };
                        let n = [-u[1], u[0]];
                        let tip = [c[0] + u[0] * (r + 2.5), c[1] + u[1] * (r + 2.5)];
                        let s1 = [c[0] + u[0] * r * 0.6 + n[0] * r * 0.8, c[1] + u[1] * r * 0.6 + n[1] * r * 0.8];
                        let s2 = [c[0] + u[0] * r * 0.6 - n[0] * r * 0.8, c[1] + u[1] * r * 0.6 - n[1] * r * 0.8];
                        self.lines.push(Polyline2 { points: vec![s1, tip, s2], style: LineStyle::Thin, closed: false, pick });
                    }
                    // The target's position dimensions from its origin (X and
                    // Y; hidden with "Hide dimension").
                    if let (Some(o), false) = (opts.origin, opts.hide_dims) {
                        let os = env.to_sheet(*view, o);
                        let (dx, dy) = ((anchor[0] - o[0]).abs(), (anchor[1] - o[1]).abs());
                        if dx > 1e-9 {
                            let t = env.text(a, dx, false, "");
                            self.linear_dimension_text(os, [p[0], os[1]], -8.0, DimOrientation::Horizontal, &t, pick);
                        }
                        if dy > 1e-9 {
                            let t = env.text(a, dy, false, "");
                            self.linear_dimension_text([p[0], os[1]], p, 8.0, DimOrientation::Vertical, &t, pick);
                        }
                    }
                }
            }
        }
        self.in_dim = false;
        self.flip_arrows = false;
        (self.jogs, self.cuts) = ([None; 2], Vec::new());
        self.style = base;
        // Dimension and annotation colors (construction geometry keeps ink).
        let end = self.mark();
        for (k, &(a, t0, l0, f0)) in starts.iter().enumerate() {
            let construction = matches!(a.kind, AnnotationKind::Centermark { .. } | AnnotationKind::Centerline { .. } | AnnotationKind::CircleCenterline { .. } | AnnotationKind::VirtualSharp { .. });
            let mut ts = self.style.type_style(&a.kind);
            if let AnnotationKind::Note { style, .. } = &a.kind {
                // The note's own color and font come first.
                ts.color = style.color.or(ts.color);
                ts.font = style.font.clone().or(ts.font);
            }
            let own = a.style.as_deref();
            let c = own.and_then(|s| s.color).or(if a.kind.is_dimension() { self.style.dim_color } else if construction { None } else { ts.color.or(self.style.ann_color) });
            let (l1, t1, f1) = starts.get(k + 1).map(|s| (s.2, s.1, s.3)).unwrap_or(end);
            if !a.layer.is_drawing() {
                self.layers.push(LayerSpan { lines: l0..l1, texts: t0..t1, fills: f0..f1, layer: a.layer });
            }
            let font = own.and_then(|s| s.font.clone()).or(if a.kind.is_dimension() { self.style.dim_font.clone() } else if construction { None } else { ts.font.or_else(|| self.style.ann_font.clone()) });
            if let Some(font) = font {
                self.fonts.push(FontSpan { texts: t0..t1, font });
            }
            let Some(c) = c else { continue };
            self.colors.push(ColorSpan { lines: l0..l1, texts: t0..t1, fills: f0..f1, rgb: c });
        }
        self.add_cad_objects(&starts, end, env);
        // Callouts grouped with a dimension: at the upper right of its text.
        let (tn, ln) = (self.texts.len(), self.lines.len());
        let corner = |g: &Self, id: u64| {
            let k = starts.iter().position(|s| s.0.id == id)?;
            let (t0, t1) = (starts[k].1, starts.get(k + 1).map(|s| s.1).unwrap_or(tn));
            let pts: Vec<[f64; 2]> = g.texts[t0..t1].iter().flat_map(super::font::text_box).collect();
            (!pts.is_empty()).then(|| [pts.iter().map(|p| p[0]).fold(f64::MIN, f64::max), pts.iter().map(|p| p[1]).fold(f64::MIN, f64::max)])
        };
        let grouped: Vec<([f64; 2], String, u32)> = sheet
            .annotations
            .iter()
            .filter_map(|a| match &a.kind {
                AnnotationKind::RevisionCallout { anchor, revision, group: Some(g) } => {
                    let c = corner(self, *g).unwrap_or([0.0, 0.0]);
                    Some(([c[0] + anchor[0], c[1] + anchor[1]], env.revision_label(revision)?, (a.id & 0xFFFF_FFFF) as u32))
                }
                _ => None,
            })
            .collect();
        let grouped_balloons: Vec<(&Annotation, [f64; 2])> = sheet
            .annotations
            .iter()
            .filter(|a| matches!(a.kind, AnnotationKind::Balloon { .. }))
            .filter_map(|a| {
                let c = corner(self, a.group?).unwrap_or([0.0, 0.0]);
                Some((a, [c[0] + a.position[0], c[1] + a.position[1]]))
            })
            .collect();
        let m = self.mark();
        let ah0 = self.style.ann_text;
        for (p, label, pick) in grouped {
            self.revision_callout(p, label, ah0, pick);
        }
        for (a, c) in grouped_balloons {
            let ah = self.style.type_style(&a.kind).height.unwrap_or(ah0);
            self.balloon(a, c, env, ah, (a.id & 0xFFFF_FFFF) as u32);
        }
        if let Some(c) = self.style.ann_color {
            self.color_since(m, c);
        }
        // (Inspection bubbles measure the annotations' own geometry only.)
        if let Some(style) = env.inspection_style() {
            for (k, &(a, t0, l0, _)) in starts.iter().enumerate() {
                let Some(n) = a.inspection else { continue };
                let (t1, l1) = starts.get(k + 1).map(|s| (s.1, s.2)).unwrap_or((tn, ln));
                let mut pts: Vec<[f64; 2]> = self.texts[t0..t1].iter().flat_map(super::font::text_box).collect();
                if pts.is_empty() {
                    pts = self.lines[l0..l1].iter().flat_map(|l| l.points.iter().copied()).collect();
                }
                let at = if pts.is_empty() { a.position } else { [pts.iter().map(|p| p[0]).fold(f64::MAX, f64::min), pts.iter().map(|p| p[1]).fold(f64::MIN, f64::max)] };
                self.inspection_bubble(at, n, &style, inspection_pick(a.id));
            }
        }
    }

    /// Note text at its top-left corner: plain runs and stacked fractions
    /// (numerator over denominator, with or without a bar).
    fn note_text(&mut self, pos: [f64; 2], h: f64, text: &str, pick: u32) {
        use super::note::{FRACTION_SCALE, Seg};
        let mut x = pos[0];
        for seg in super::note::segments(text) {
            match seg {
                Seg::Text(t) => {
                    let w = font::text_width(&t, h);
                    self.texts.push(Text2 { pos: [x, pos[1]], height: h, text: t, angle: 0.0, anchor: Anchor::TopLeft, pick });
                    x += w;
                }
                Seg::Fraction { num, den, bar } => {
                    let fh = h * FRACTION_SCALE;
                    let w = font::text_width(&num, fh).max(font::text_width(&den, fh));
                    let cx = x + h * 0.1 + w / 2.0;
                    self.texts.push(Text2 { pos: [cx, pos[1] + h * 0.15 - fh], height: fh, text: num, angle: 0.0, anchor: Anchor::BottomCenter, pick });
                    self.texts.push(Text2 { pos: [cx, pos[1] - h * 0.55 - fh], height: fh, text: den, angle: 0.0, anchor: Anchor::BottomCenter, pick });
                    if bar {
                        self.lines.push(Polyline2 { points: vec![[x + h * 0.05, pos[1] - h * 0.5], [x + h * 0.15 + w, pos[1] - h * 0.5]], style: LineStyle::Thin, closed: false, pick });
                    }
                    x += w + h * 0.2;
                }
            }
        }
    }

    /// A flag-note flag around a list designator (text top-left at `pos`):
    /// its size fits `flag_chars` characters.
    fn flag(&mut self, pos: [f64; 2], h: f64, designator: &str, style: &super::note::NoteStyle, _item: usize, pick: u32) {
        use super::note::FlagShape;
        let w = font::text_width(designator, h).max(font::text_width(&"0".repeat(style.flag_chars.max(1) as usize), h));
        let c = [pos[0] + font::text_width(designator, h) / 2.0, pos[1] - h / 2.0];
        let r = w / 2.0 + h * 0.5;
        let ring = |r: f64, n: usize, a0: f64| -> Vec<[f64; 2]> { (0..n).map(|i| a0 + i as f64 / n as f64 * std::f64::consts::TAU).map(|t| [c[0] + r * t.cos(), c[1] + r * t.sin()]).collect() };
        let pts: Vec<[f64; 2]> = match style.flag_shape {
            FlagShape::None => return,
            FlagShape::Underline => {
                self.lines.push(Polyline2 { points: vec![[c[0] - r, c[1] - r * 0.8], [c[0] + r, c[1] - r * 0.8]], style: LineStyle::Thin, closed: false, pick });
                return;
            }
            FlagShape::Triangle => vec![[c[0] - r * 1.2, c[1] - r * 0.7], [c[0] + r * 1.2, c[1] - r * 0.7], [c[0], c[1] + r * 1.3]],
            FlagShape::Rectangle => vec![[c[0] - r, c[1] - r * 0.8], [c[0] + r, c[1] - r * 0.8], [c[0] + r, c[1] + r * 0.8], [c[0] - r, c[1] + r * 0.8]],
            FlagShape::Flag => vec![[c[0] - r, c[1] - r * 0.8], [c[0] + r, c[1] - r * 0.8], [c[0] + r + r * 0.6, c[1]], [c[0] + r, c[1] + r * 0.8], [c[0] - r, c[1] + r * 0.8]],
            FlagShape::Diamond => vec![[c[0] - r * 1.3, c[1]], [c[0], c[1] - r * 1.3], [c[0] + r * 1.3, c[1]], [c[0], c[1] + r * 1.3]],
            FlagShape::Hexagon => ring(r * 1.15, 6, 0.0),
            FlagShape::Octagon => ring(r * 1.1, 8, std::f64::consts::PI / 8.0),
            FlagShape::Circle => ring(r, 32, 0.0),
            FlagShape::DoubleCircle => {
                self.lines.push(Polyline2 { points: ring(r * 1.2, 32, 0.0), style: LineStyle::Thin, closed: true, pick });
                ring(r, 32, 0.0)
            }
            FlagShape::SquareCircle => {
                let q = r * 1.15;
                self.lines.push(Polyline2 { points: vec![[c[0] - q, c[1] - q], [c[0] + q, c[1] - q], [c[0] + q, c[1] + q], [c[0] - q, c[1] + q]], style: LineStyle::Thin, closed: true, pick });
                ring(r, 32, 0.0)
            }
        };
        self.lines.push(Polyline2 { points: pts, style: LineStyle::Thin, closed: true, pick });
    }

    /// A callout (balloon) centered at `c`: the circle, its leader to the
    /// anchor with a dot on the part, and its text.
    fn balloon(&mut self, a: &Annotation, c: [f64; 2], env: &dyn AnnotationEnv, ah: f64, pick: u32) {
        use super::CalloutBorder as B;
        let AnnotationKind::Balloon { view, anchor, text, callout: cs } = &a.kind else { return };
        let h = cs.height.unwrap_or(ah);
        let p = env.to_sheet(*view, *anchor);
        // The callout and its stacked callouts: the same size, one gap apart.
        let mut items = vec![env.balloon_text(a, text)];
        items.extend(cs.stacked.iter().map(|st| env.stacked_text(a, st)));
        let line = |main: &str| format!("{}{main}{}", cs.left, cs.right);
        let widest_main = items.iter().map(|m| font::text_width(&line(m), h)).fold(0.0f64, f64::max);
        // Border size: characters (Tight Fit = the text).
        let w = if cs.chars == 0 { widest_main } else { widest_main.max(cs.chars as f64 * h * 0.6) };
        let split = cs.border == B::SplitCircle;
        let (rx, ry) = match cs.border {
            B::Circle | B::DoubleCircle | B::SquareCircle | B::Octagon => {
                let r = (w / 2.0 + h * 0.8).max(h * 1.2);
                (r, r)
            }
            B::SplitCircle => {
                let r = (w / 2.0 + h * 0.9).max(h * 1.6);
                (r, r)
            }
            B::Rectangle | B::Underline | B::None | B::Flag => (w / 2.0 + h * 0.5, h * 0.8),
            B::Hexagon => (w / 2.0 + h * 0.9, h * 0.95),
            B::Diamond => (w / 2.0 + h * 1.2, h * 1.3),
            B::Triangle => {
                let r = (w / 2.0 + h * 0.6) * 1.6;
                (r, r)
            }
        };
        let step = self.style.callout_stack.step([c[0] - p[0], c[1] - p[1]]);
        let pitch = if step[0] != 0.0 { 2.0 * rx } else { 2.0 * ry } + self.style.callout_stack_gap;
        let just = self.style.callout_justify;
        let leader_left = p[0] < c[0];
        let ring = |ck: [f64; 2], r: f64, n: usize, a0: f64| -> Vec<[f64; 2]> { (0..n).map(|i| a0 + i as f64 / n as f64 * std::f64::consts::TAU).map(|t| [ck[0] + r * t.cos(), ck[1] + r * t.sin()]).collect() };
        for (k, item) in items.iter().enumerate() {
            let ck = [c[0] + step[0] * pitch * k as f64, c[1] + step[1] * pitch * k as f64];
            let mut outlines: Vec<(Vec<[f64; 2]>, bool)> = match cs.border {
                B::None => vec![],
                B::Circle | B::SplitCircle => vec![(ring(ck, rx, 40, 0.0), true)],
                B::DoubleCircle => vec![(ring(ck, rx, 40, 0.0), true), (ring(ck, rx * 0.82, 40, 0.0), true)],
                B::SquareCircle => {
                    let q = rx * 1.08;
                    vec![(ring(ck, rx, 40, 0.0), true), (vec![[ck[0] - q, ck[1] - q], [ck[0] + q, ck[1] - q], [ck[0] + q, ck[1] + q], [ck[0] - q, ck[1] + q]], true)]
                }
                B::Octagon => vec![(ring(ck, rx / (std::f64::consts::PI / 8.0).cos(), 8, std::f64::consts::PI / 8.0), true)],
                B::Rectangle => vec![(vec![[ck[0] - rx, ck[1] - ry], [ck[0] + rx, ck[1] - ry], [ck[0] + rx, ck[1] + ry], [ck[0] - rx, ck[1] + ry]], true)],
                B::Underline => vec![(vec![[ck[0] - rx, ck[1] - ry], [ck[0] + rx, ck[1] - ry]], false)],
                B::Flag => vec![(vec![[ck[0] - rx, ck[1] - ry], [ck[0] + rx, ck[1] - ry], [ck[0] + rx + ry, ck[1]], [ck[0] + rx, ck[1] + ry], [ck[0] - rx, ck[1] + ry]], true)],
                B::Hexagon => {
                    let e = ry * 0.6;
                    vec![(vec![[ck[0] - rx, ck[1]], [ck[0] - rx + e, ck[1] - ry], [ck[0] + rx - e, ck[1] - ry], [ck[0] + rx, ck[1]], [ck[0] + rx - e, ck[1] + ry], [ck[0] - rx + e, ck[1] + ry]], true)]
                }
                B::Diamond => vec![(vec![[ck[0] - rx, ck[1]], [ck[0], ck[1] - ry], [ck[0] + rx, ck[1]], [ck[0], ck[1] + ry]], true)],
                B::Triangle => vec![(ring(ck, rx, 3, std::f64::consts::FRAC_PI_2).into_iter().map(|q| [q[0], q[1] - ry * 0.25]).collect(), true)],
            };
            if split {
                outlines.push((vec![[ck[0] - rx, ck[1]], [ck[0] + rx, ck[1]]], false));
            }
            for (pts, closed) in outlines {
                self.lines.push(Polyline2 { points: pts, style: LineStyle::Thin, closed, pick });
            }
            // The callout data (left + main + right) in the middle, justified
            // in the widest row; a split circle has it in the top half and the
            // text below in the bottom half.
            let main = line(item);
            let x_of = |t: &str| {
                let tw = font::text_width(t, h);
                ck[0] - widest_main / 2.0 + (widest_main - tw) * just.frac(cs.border != B::None, leader_left)
            };
            let mid_y = if split { ck[1] + ry * 0.45 } else { ck[1] };
            self.texts.push(Text2 { pos: [x_of(&main), mid_y - h / 2.0], height: h, text: main.clone(), angle: 0.0, anchor: Anchor::BottomLeft, pick });
            // Texts above / below: outside the border, centred (a split
            // circle keeps the text below inside its bottom half).
            let gap = h * 0.4;
            let centred = |t: &str| ck[0] - font::text_width(t, h) / 2.0;
            if !cs.above.is_empty() {
                let y = if cs.border == B::None { ck[1] + h * 0.9 } else { ck[1] + ry + gap };
                self.texts.push(Text2 { pos: [centred(&cs.above), y], height: h, text: cs.above.clone(), angle: 0.0, anchor: Anchor::BottomLeft, pick });
            }
            if !cs.below.is_empty() {
                let y = if split { ck[1] - ry * 0.45 - h / 2.0 } else if cs.border == B::None { ck[1] - h * 1.9 } else { ck[1] - ry - gap - h };
                self.texts.push(Text2 { pos: [centred(&cs.below), y], height: h, text: cs.below.clone(), angle: 0.0, anchor: Anchor::BottomLeft, pick });
            }
        }
        // The leader from the first callout to the part: an arrow on an edge,
        // a dot on a face. Border None: a short shoulder at the middle row.
        // Leaders added with Add leader end in an arrow; each runs through
        // its nodes, from the border or from a node of an earlier leader.
        let all = cs.all_leaders(*view, *anchor);
        for (i, ld) in all.iter().enumerate() {
            if i == 0 && cs.free {
                continue;
            }
            let p = if i == 0 { p } else { ld.view.map(|v| env.to_sheet(v, ld.point)).unwrap_or(ld.point) };
            let next = ld.nodes.first().copied().unwrap_or(p);
            let start = match ld.start {
                super::LeaderStart::Node(j, n) if j < i && (j > 0 || !cs.free) && n < all[j].nodes.len() => all[j].nodes[n],
                _ if cs.border == B::None => {
                    let left = next[0] < c[0];
                    let sx = if left { c[0] - w / 2.0 - h * 0.3 } else { c[0] + w / 2.0 + h * 0.3 };
                    let shoulder = [sx + if left { -h * 1.2 } else { h * 1.2 }, c[1]];
                    self.lines.push(Polyline2 { points: vec![[sx, c[1]], shoulder], style: LineStyle::Thin, closed: false, pick });
                    shoulder
                }
                _ => {
                    let d = [next[0] - c[0], next[1] - c[1]];
                    let l = (d[0] * d[0] + d[1] * d[1]).sqrt().max(1e-9);
                    let u = [d[0] / l, d[1] / l];
                    let r = match cs.border {
                        B::Circle | B::DoubleCircle | B::SplitCircle | B::Octagon | B::Triangle => rx,
                        B::SquareCircle => rx * 1.08 / u[0].abs().max(u[1].abs()).max(1e-9),
                        B::Rectangle | B::Flag | B::Underline => (rx / u[0].abs().max(1e-9)).min(ry / u[1].abs().max(1e-9)),
                        // The ellipse through the shape's extremes.
                        _ => rx * ry / (rx * u[1]).hypot(ry * u[0]).max(1e-9),
                    };
                    [c[0] + u[0] * r, c[1] + u[1] * r]
                }
            };
            let mut points = vec![start];
            points.extend(ld.nodes.iter().copied());
            points.push(p);
            let last = points[points.len() - 2];
            self.lines.push(Polyline2 { points: points.clone(), style: LineStyle::Thin, closed: false, pick });
            self.leader_paths.push((a.id, i, LeaderPath { points, head: 1 }));
            if i == 0 && cs.on_face {
                self.fills.push(Fill2 { points: (0..8).map(|i| {
                    let t = i as f64 / 8.0 * std::f64::consts::TAU;
                    [p[0] + 0.6 * t.cos(), p[1] + 0.6 * t.sin()]
                }).collect() });
            } else {
                self.arrow(p, [p[0] - last[0], p[1] - last[1]], self.style.ann_arrow);
            }
        }
    }

    /// A datum target area's hatch (Drawing properties › Datums › Target
    /// hatch pattern).
    fn target_hatch(&mut self, area: &[[f64; 2]], pick: u32) {
        let (std, pat) = self.style.target_hatch;
        for l in super::sheet_sketch::hatch_pattern(area, std, pat, 0.6, 0.0) {
            self.lines.push(Polyline2 { points: l.to_vec(), style: LineStyle::Thin, closed: false, pick });
        }
    }

    /// A revision callout: an equilateral triangle around the label, base
    /// below it, centered on `p`.
    fn revision_callout(&mut self, p: [f64; 2], label: String, ah: f64, pick: u32) {
        let s = (font::text_width(&label, ah) + ah * 2.4).max(ah * 3.2);
        let h = s * 0.866;
        let y0 = p[1] - h / 3.0;
        self.lines.push(Polyline2 { points: vec![[p[0] - s / 2.0, y0], [p[0] + s / 2.0, y0], [p[0], y0 + h]], style: LineStyle::Thin, closed: true, pick });
        self.texts.push(Text2 { pos: p, height: ah, text: label, angle: 0.0, anchor: Anchor::Center, pick });
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
            // Stacked with line: the rule between them, as wide as the wider.
            if t.rule {
                let w = font::text_width(up, hs).max(font::text_width(lo, hs));
                let y = (yu + yl + hs) / 2.0;
                self.lines.push(Polyline2 { points: vec![loc(xs, y), loc(xs + w, y)], style: LineStyle::Thin, closed: false, pick });
            }
        }
        if t.boxed {
            let (xa, xb, ya, yb) = (x0 - 0.8, x0 + total + 0.8, base - 0.8, base + h + 0.8);
            self.lines.push(Polyline2 { points: vec![loc(xa, ya), loc(xb, ya), loc(xb, yb), loc(xa, yb)], style: LineStyle::Thin, closed: true, pick });
        }
        // The dual value: a second line, the same height, centred on the
        // value, above (Top) or below (Bottom) it (live asb 10-24).
        if let Some(d) = &t.dual {
            let w = font::text_width(d, h);
            let y = if t.dual_below { base - h * 1.6 } else { base + h * 1.6 };
            self.texts.push(Text2 { pos: loc(x0 + (total - w) / 2.0, y), height: h, text: d.clone(), angle, anchor: Anchor::BottomLeft, pick });
        }
        // Above / below text, centred on the value.
        for (txt, y) in [(&t.above, base + h * 1.5), (&t.below, base - h * 1.5)] {
            if let Some(txt) = txt {
                let w = font::text_width(txt, h);
                self.texts.push(Text2 { pos: loc(x0 + (total - w) / 2.0, y), height: h, text: txt.clone(), angle, anchor: Anchor::BottomLeft, pick });
            }
        }
        // Inspection dimension: an oval (half circles at the ends).
        if t.oval {
            let (ya, yb) = (base - 0.8, base + h + 0.8);
            let r = (yb - ya) / 2.0;
            let (xa, xb) = (x0 - 0.6, x0 + total + 0.6);
            let yc = (ya + yb) / 2.0;
            let mut pts = Vec::new();
            for i in 0..=12 {
                let a = -std::f64::consts::FRAC_PI_2 + std::f64::consts::PI * i as f64 / 12.0;
                pts.push(loc(xb + r * a.cos(), yc + r * a.sin()));
            }
            for i in 0..=12 {
                let a = std::f64::consts::FRAC_PI_2 + std::f64::consts::PI * i as f64 / 12.0;
                pts.push(loc(xa + r * a.cos(), yc + r * a.sin()));
            }
            self.lines.push(Polyline2 { points: pts, style: LineStyle::Thin, closed: true, pick });
        }
    }

    /// Linear dimension between sheet points p and q.
    pub fn linear_dimension(&mut self, p: [f64; 2], q: [f64; 2], offset: f64, orientation: DimOrientation, text: &str, pick: u32) {
        self.linear_dimension_text(p, q, offset, orientation, &DimText::plain(text), pick);
    }

    /// Linear dimension with a formatted label.
    pub fn linear_dimension_text(&mut self, p: [f64; 2], q: [f64; 2], offset: f64, orientation: DimOrientation, text: &DimText, pick: u32) {
        let f = linear_frame(p, q, offset, orientation, &self.jogs, self.style.lock_jog_45, self.style.geometry_gap, self.style.extension);
        let (dir, dp, dq) = (f.dir, f.dp, f.dq);
        let half = self.style.dim_text * 0.4;
        let cuts = std::mem::take(&mut self.cuts);
        for e in &f.ext {
            for piece in cut_polyline(e, &cuts, half) {
                self.lines.push(Polyline2 { points: piece, style: LineStyle::Thin, closed: false, pick });
            }
        }
        for j in &f.jog_points {
            self.grips.push(Grip { pick, pos: *j, kind: GripKind::Jog });
        }
        let d = [dq[0] - dp[0], dq[1] - dp[1]];
        let dl = (d[0] * d[0] + d[1] * d[1]).sqrt().max(1e-12);
        let du = [d[0] / dl, d[1] / dl];
        // A dual value straddles the line (below): the line breaks around
        // the text block (live asbdd-20: DIMGAP, 0.75 of the text height from
        // the ink, = 0.65 from our text box), here the half gap along it.
        let along = !(self.style.text_horizontal && dir[1].abs() > 1e-9);
        let gap = dual_straddles(text).filter(|_| along).map(|dual| {
            let h = self.style.dim_text;
            font::text_width(&dual, h).max(font::text_width(&text.main, h)) / 2.0 + 0.65 * h
        });
        let c = [(dp[0] + dq[0]) / 2.0, (dp[1] + dq[1]) / 2.0];
        let broken = |line: [[f64; 2]; 2]| -> Vec<Vec<[f64; 2]>> {
            let Some(g) = gap else { return vec![line.to_vec()] };
            let t = |p: [f64; 2]| (p[0] - c[0]) * du[0] + (p[1] - c[1]) * du[1];
            let at = |t: f64| [c[0] + du[0] * t, c[1] + du[1] * t];
            let (t0, t1) = (t(line[0]), t(line[1]));
            let mut out = Vec::new();
            if t0 < -g {
                out.push(vec![line[0], at(t1.min(-g))]);
            }
            if t1 > g {
                out.push(vec![at(t0.max(g)), line[1]]);
            }
            out
        };
        if self.flip_arrows {
            // Flipped: arrows outside pointing in, the line runs past them.
            let ext = self.style.dim_arrow + 3.0;
            for seg in broken([[dp[0] - du[0] * ext, dp[1] - du[1] * ext], [dq[0] + du[0] * ext, dq[1] + du[1] * ext]]) {
                for piece in cut_polyline(&seg, &cuts, half) {
                    self.lines.push(Polyline2 { points: piece, style: LineStyle::Thin, closed: false, pick });
                }
            }
            self.arrow(dp, d, 3.0);
            self.arrow(dq, [-d[0], -d[1]], 3.0);
        } else {
            for seg in broken([dp, dq]) {
                for piece in cut_polyline(&seg, &cuts, half) {
                    self.lines.push(Polyline2 { points: piece, style: LineStyle::Thin, closed: false, pick });
                }
            }
            self.arrow(dp, [-d[0], -d[1]], 3.0);
            self.arrow(dq, d, 3.0);
        }
        for (pos, kind) in [
            (p, GripKind::Origin),
            (q, GripKind::Origin),
            (dp, GripKind::End),
            (dq, GripKind::End),
            ([dp[0] + du[0] * 4.0, dp[1] + du[1] * 4.0], GripKind::Handle),
            ([dq[0] - du[0] * 4.0, dq[1] - du[1] * 4.0], GripKind::Handle),
        ] {
            self.grips.push(Grip { pick, pos, kind });
        }
        let mut angle = dir[1].atan2(dir[0]);
        if angle > std::f64::consts::FRAC_PI_2 + 1e-9 || angle < -std::f64::consts::FRAC_PI_2 - 1e-9 {
            angle += std::f64::consts::PI;
        }
        // Place the text on its own "up" side of the dimension line.
        let up = [-angle.sin(), angle.cos()];
        let tg = self.style.text_gap;
        let mut mid = [(dp[0] + dq[0]) / 2.0 + up[0] * tg, (dp[1] + dq[1]) / 2.0 + up[1] * tg];
        if self.style.text_horizontal && angle.abs() > 1e-9 {
            // Horizontal text beside a non-horizontal dimension line: clear
            // the line by the text's half width along `up`.
            let w = self.dim_label_width(text, self.style.dim_text);
            let k = (w / 2.0 * angle.sin().abs() + self.style.dim_text / 2.0 * (1.0 - angle.cos().abs())).max(0.0);
            mid = [mid[0] + up[0] * k, mid[1] + up[1] * k];
            angle = 0.0;
        }
        if let Some(dual) = dual_straddles(text) {
            // Dual (live asb 10-24): the dimension line runs between the two
            // lines, broken around them; the dual above it for Top, below it
            // for Bottom.
            let h = self.style.dim_text;
            let below = [mid[0] - up[0] * (2.0 * tg + h), mid[1] - up[1] * (2.0 * tg + h)];
            let (top_text, bottom_text) = if text.dual_below { (text.main.clone(), dual) } else { (dual, text.main.clone()) };
            self.texts.push(Text2 { pos: mid, height: h, text: top_text, angle, anchor: Anchor::BottomCenter, pick });
            self.texts.push(Text2 { pos: below, height: h, text: bottom_text, angle, anchor: Anchor::BottomCenter, pick });
        } else if text.upper.is_none() && !text.boxed && text.above.is_none() && text.below.is_none() && !text.oval {
            self.texts.push(Text2 { pos: mid, height: self.style.dim_text, text: text.main.clone(), angle, anchor: Anchor::BottomCenter, pick });
        } else {
            let mid = if text.boxed { [mid[0] + up[0] * 0.8, mid[1] + up[1] * 0.8] } else { mid };
            self.dim_label(mid, angle, 0.5, false, text, self.style.dim_text, pick);
        }
    }
}

/// The dual value of a dimension label drawn as two lines across the
/// dimension line (plain values only).
fn dual_straddles(text: &DimText) -> Option<String> {
    text.dual.clone().filter(|_| text.upper.is_none() && !text.boxed && text.above.is_none() && text.below.is_none() && !text.oval)
}

/// Where a linear dimension's lines go (sheet mm): its direction, the
/// dimension line's ends (moved sideways by jogs), the extension lines as
/// polylines (with their jogs) and the jogs' snap points.
pub struct LinearFrame {
    pub dir: [f64; 2],
    pub dp: [f64; 2],
    pub dq: [f64; 2],
    pub ext: Vec<Vec<[f64; 2]>>,
    pub jog_points: Vec<[f64; 2]>,
    /// Per extension line: (model point, unit direction, length to the
    /// dimension line) — for jog editing.
    pub legs: [([f64; 2], [f64; 2], f64); 2],
}

/// The geometry of a linear dimension between sheet points `p` and `q`.
#[allow(clippy::too_many_arguments)]
pub fn linear_frame(p: [f64; 2], q: [f64; 2], offset: f64, orientation: DimOrientation, jogs: &[Option<super::ExtJog>; 2], lock45: bool, gap: f64, ext: f64) -> LinearFrame {
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
    let mut ends = [at(p), at(q)];
    let mut out = Vec::new();
    let mut jog_points = Vec::new();
    let mut legs = [([0.0; 2], [0.0, 1.0], 0.0); 2];
    for (k, src) in [p, q].into_iter().enumerate() {
        let dst = ends[k];
        let v = [dst[0] - src[0], dst[1] - src[1]];
        let l = (v[0] * v[0] + v[1] * v[1]).sqrt();
        let u = if l > 1e-9 { [v[0] / l, v[1] / l] } else { nrm };
        legs[k] = (src, u, l);
        let start = [src[0] + u[0] * gap, src[1] + u[1] * gap];
        match jogs[k] {
            Some(j) => {
                let j0 = [src[0] + u[0] * l * j.at.clamp(0.0, 1.0), src[1] + u[1] * l * j.at.clamp(0.0, 1.0)];
                let drop = if lock45 { j.shift.abs() } else { j.drop }.clamp(0.0, l * (1.0 - j.at.clamp(0.0, 1.0)));
                let j1 = [j0[0] + u[0] * drop + dir[0] * j.shift, j0[1] + u[1] * drop + dir[1] * j.shift];
                let end = [dst[0] + dir[0] * j.shift, dst[1] + dir[1] * j.shift];
                out.push(vec![start, j0, j1, [end[0] + u[0] * ext, end[1] + u[1] * ext]]);
                jog_points.extend([j0, j1]);
                ends[k] = end;
            }
            None if l > gap => out.push(vec![start, [dst[0] + u[0] * ext, dst[1] + u[1] * ext]]),
            None => {}
        }
    }
    LinearFrame { dir, dp: ends[0], dq: ends[1], ext: out, jog_points, legs }
}

/// A polyline with gaps of `half` mm on each side of the `cuts` points
/// that lie on it (Break dimension).
pub fn cut_polyline(points: &[[f64; 2]], cuts: &[[f64; 2]], half: f64) -> Vec<Vec<[f64; 2]>> {
    let mut out = vec![vec![points[0]]];
    for w in points.windows(2) {
        let (a, b) = (w[0], w[1]);
        let d = [b[0] - a[0], b[1] - a[1]];
        let len = (d[0] * d[0] + d[1] * d[1]).sqrt();
        if len < 1e-9 {
            continue;
        }
        let u = [d[0] / len, d[1] / len];
        // Cut points on this segment, by distance from `a`.
        let mut ts: Vec<f64> = cuts
            .iter()
            .filter_map(|c| {
                let t = (c[0] - a[0]) * u[0] + (c[1] - a[1]) * u[1];
                let off = ((c[0] - a[0]) * u[1] - (c[1] - a[1]) * u[0]).abs();
                (off < 1e-6 && t > -1e-9 && t < len + 1e-9).then_some(t)
            })
            .collect();
        ts.sort_by(f64::total_cmp);
        let pt = |t: f64| [a[0] + u[0] * t, a[1] + u[1] * t];
        for t in ts {
            let last = out.last_mut().expect("a piece");
            last.push(pt((t - half).max(0.0)));
            out.push(vec![pt((t + half).min(len))]);
        }
        out.last_mut().expect("a piece").push(b);
    }
    out.retain(|piece| piece.windows(2).map(|w| ((w[1][0] - w[0][0]).powi(2) + (w[1][1] - w[0][1]).powi(2)).sqrt()).sum::<f64>() > 1e-6);
    out
}

/// A linear dimension's line segments for Break dimension: (annotation id,
/// broken, segments).
type DimSegments = (u64, bool, Vec<[[f64; 2]; 2]>);

/// Where two segments cross (not at their ends).
fn crossing(a: [[f64; 2]; 2], b: [[f64; 2]; 2]) -> Option<[f64; 2]> {
    let r = [a[1][0] - a[0][0], a[1][1] - a[0][1]];
    let s = [b[1][0] - b[0][0], b[1][1] - b[0][1]];
    let den = r[0] * s[1] - r[1] * s[0];
    if den.abs() < 1e-12 {
        return None;
    }
    let w = [b[0][0] - a[0][0], b[0][1] - a[0][1]];
    let t = (w[0] * s[1] - w[1] * s[0]) / den;
    let u = (w[0] * r[1] - w[1] * r[0]) / den;
    let eps = 1e-6;
    (t > eps && t < 1.0 - eps && u > eps && u < 1.0 - eps).then(|| [a[0][0] + r[0] * t, a[0][1] + r[1] * t])
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

/// Sheet rectangle [x0, y0, x1, y1] (mm) of a sheet's title block.
pub fn title_block_rect(sheet: &Sheet) -> Option<[f64; 4]> {
    // Imported DWG / DXF sheets have no title block of ours.
    if sheet.imported.is_some() {
        return None;
    }
    let [_, y0, x1, _] = sheet.size.frame_rect();
    let tb = sheet.size.title_block_height();
    let (bw, bh) = match sheet.title_style {
        TitleBlockStyle::None => return None,
        TitleBlockStyle::Simple => (180.0, 8.0 * ((sheet.title_block.fields.len() as f64) / 2.0).ceil()),
        // Onshape's template block in the frame's lower right corner (live
        // asb 10-24 "template frames": 4.25 x its height on ISO / JIS, 4.05 x
        // on ANSI).
        TitleBlockStyle::Iso => (4.25 * tb, tb),
        TitleBlockStyle::Ansi => (4.05 * tb, tb),
    };
    Some([x1 - bw, y0, x1, y0 + bh])
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
    /// Text of a stacked callout (empty: its part's BOM item).
    fn stacked_text(&self, _a: &Annotation, st: &super::StackedCallout) -> String {
        if st.text.trim().is_empty() { "----".into() } else { st.text.clone() }
    }
    /// Drawing-wide dimension / annotation style.
    fn sheet_style(&self) -> SheetStyle {
        SheetStyle::default()
    }
    /// Inspection item bubbles' look, None while hidden.
    fn inspection_style(&self) -> Option<InspectionStyle> {
        Some(InspectionStyle { shape: super::InspectionShape::Circle, text_height: 2.5, border: 1.2, chars: 2, color: None, solid: false, font: None })
    }
    /// Weld symbols follow ANSI (else ISO).
    fn weld_ansi(&self) -> bool {
        true
    }
    /// Dimensions › Chamfer settings.
    fn chamfer_style(&self) -> ChamferStyle {
        ChamferStyle { dim45: false, content45: super::ChamferContent::LengthXAngle, dim: true, content: super::ChamferContent::LengthXAngle, length_precision: None, angle_precision: None }
    }
    /// Label of a revision callout ("B*" while pending), None when hidden.
    fn revision_label(&self, revision: &str) -> Option<String> {
        Some(revision.to_string())
    }
    /// Lines of a hole / thread callout (from the Hole feature).
    /// A dimension's dual units for the DXF / DWG export (None: single).
    fn dual_cad(&self, _a: &Annotation, _angular: bool) -> Option<super::cad_objects::CadDual> {
        None
    }
    fn hole_callout(&self, _a: &Annotation) -> Vec<String> {
        vec!["⌀?".into()]
    }
}

/// A simple environment for tests and free sheets: identity mapping per
/// view (`scale` mm per m) and default formatting.
pub struct PlainEnv<'a> {
    pub drawing: &'a Drawing,
    pub scale: f64,
}

impl AnnotationEnv for PlainEnv<'_> {
    fn chamfer_style(&self) -> ChamferStyle {
        let p = &self.drawing.props;
        ChamferStyle { dim45: p.chamfer_45_dimension, content45: p.chamfer_45_content, dim: p.chamfer_dimension, content: p.chamfer_content, length_precision: p.chamfer_length_precision, angle_precision: p.chamfer_angle_precision }
    }
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
        super::format::text_codes(s)
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

/// Pick id of an annotation's inspection item bubble (before the editor's
/// pick table remaps it): the annotation's id with the top bit set.
pub fn inspection_pick(id: u64) -> u32 {
    ((id & 0x7FFF_FFFF) as u32) | 0x8000_0000
}

/// Drawing properties › Inspection: the bubbles' look (text height and
/// border size in mm; color None = black; solid = filled, white number).
#[derive(Clone, Debug, PartialEq)]
pub struct InspectionStyle {
    pub shape: super::InspectionShape,
    pub text_height: f64,
    pub border: f64,
    /// Border size in characters (0: Tight Fit).
    pub chars: u8,
    pub color: Option<[u8; 3]>,
    pub solid: bool,
    pub font: Option<TextFont>,
}

/// The `i`th zone letter (0 = A), skipping O; past Z the letters double
/// (AA, BB, ...).
pub fn zone_letter(i: usize) -> String {
    const LETTERS: &[u8] = b"ABCDEFGHIJKLMNPQRSTUVWXYZ";
    let c = LETTERS[i % LETTERS.len()] as char;
    c.to_string().repeat(i / LETTERS.len() + 1)
}

/// Greedy line breaking of `text` at height `h` into lines at most `w` mm
/// wide (words; a word wider than a line, e.g. CJK text without spaces,
/// breaks between characters).
pub fn wrap_text(text: &str, h: f64, w: f64) -> Vec<String> {
    let fits = |s: &str| font::text_width(s, h) <= w;
    let mut lines: Vec<String> = Vec::new();
    let mut cur = String::new();
    for word in text.split(' ').filter(|x| !x.is_empty()) {
        let joined = if cur.is_empty() { word.to_string() } else { format!("{cur} {word}") };
        if fits(&joined) {
            cur = joined;
            continue;
        }
        if !cur.is_empty() {
            lines.push(std::mem::take(&mut cur));
        }
        if fits(word) {
            cur = word.to_string();
        } else {
            for c in word.chars() {
                let mut t = cur.clone();
                t.push(c);
                if !cur.is_empty() && !fits(&t) {
                    lines.push(std::mem::take(&mut cur));
                }
                cur.push(c);
            }
        }
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    lines
}

/// A note leader's polyline (sheet mm): `head` points before its nodes (the
/// side point and the bend, or the one branch point), then the nodes, then
/// the target.
#[derive(Clone, Debug, PartialEq)]
pub struct LeaderPath {
    pub points: Vec<[f64; 2]>,
    pub head: usize,
}

impl LeaderPath {
    /// The bend: where the landing ends (the branch point of a branch).
    pub fn bend(&self) -> [f64; 2] {
        self.points[self.head - 1]
    }
}

/// The paths of a note's leaders (live drw2 10-21 g): from the box `bbox`'s
/// side (`Leader.right`, else the one nearer the target) through a
/// horizontal landing (`Leader.landing`, 4 mm) to the bend, or from the
/// bend or a node of an earlier leader; then the nodes, then `targets[i]`.
pub fn note_leader_paths(bbox: [f64; 4], leaders: &[super::Leader], targets: &[[f64; 2]]) -> Vec<LeaderPath> {
    use super::LeaderStart;
    let [x0, y0, x1, y1] = bbox;
    let ym = (y0 + y1) / 2.0;
    let mut out: Vec<LeaderPath> = Vec::new();
    for (i, ld) in leaders.iter().enumerate() {
        let t = targets.get(i).copied().unwrap_or(ld.point);
        let head = match ld.start {
            LeaderStart::Bend(j) if j < i => vec![out[j].bend()],
            LeaderStart::Node(j, n) if j < i && n < leaders[j].nodes.len() => vec![leaders[j].nodes[n]],
            _ => {
                let right = ld.right.unwrap_or(t[0] > (x0 + x1) / 2.0);
                let start = if right { [x1 + 1.0, ym] } else { [x0 - 1.0, ym] };
                let l = ld.landing.unwrap_or(4.0);
                vec![start, [start[0] + if right { l } else { -l }, ym]]
            }
        };
        let n = head.len();
        let mut points = head;
        points.extend(ld.nodes.iter().copied());
        points.push(t);
        out.push(LeaderPath { points, head: n });
    }
    out
}
