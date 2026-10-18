//! Drawings: sheets with views projected from Part Studios/Assemblies plus
//! dimensions, annotations and tables.

use crate::configuration::ConfigValues;
use crate::document::TabId;
use crate::reference::{BodyId, TopoName};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ViewId(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum SheetSize {
    A4,
    A3,
    A2,
    A1,
    A0,
    AnsiA,
    /// ANSI A portrait (8.5 x 11 in; Onshape's ANSI_A_Portrait templates).
    AnsiAPortrait,
    AnsiB,
    AnsiC,
    AnsiD,
    AnsiE,
    Custom { width_mm: f64, height_mm: f64 },
}

impl SheetSize {
    /// Landscape size in millimeters.
    pub fn size_mm(self) -> (f64, f64) {
        match self {
            SheetSize::A4 => (297.0, 210.0),
            SheetSize::A3 => (420.0, 297.0),
            SheetSize::A2 => (594.0, 420.0),
            SheetSize::A1 => (841.0, 594.0),
            SheetSize::A0 => (1189.0, 841.0),
            SheetSize::AnsiA => (279.4, 215.9),
            SheetSize::AnsiAPortrait => (215.9, 279.4),
            SheetSize::AnsiB => (431.8, 279.4),
            SheetSize::AnsiC => (558.8, 431.8),
            SheetSize::AnsiD => (863.6, 558.8),
            SheetSize::AnsiE => (1117.6, 863.6),
            SheetSize::Custom { width_mm, height_mm } => (width_mm, height_mm),
        }
    }

    /// Standard sheet sizes (ISO A4..A0, then ANSI A..E).
    pub const STANDARD: [SheetSize; 11] = [SheetSize::A4, SheetSize::A3, SheetSize::A2, SheetSize::A1, SheetSize::A0, SheetSize::AnsiA, SheetSize::AnsiAPortrait, SheetSize::AnsiB, SheetSize::AnsiC, SheetSize::AnsiD, SheetSize::AnsiE];

    /// Size designation for title blocks ("A3", "B", "Custom").
    pub fn label(self) -> &'static str {
        match self {
            SheetSize::A4 => "A4",
            SheetSize::A3 => "A3",
            SheetSize::A2 => "A2",
            SheetSize::A1 => "A1",
            SheetSize::A0 => "A0",
            SheetSize::AnsiA | SheetSize::AnsiAPortrait => "A",
            SheetSize::AnsiB => "B",
            SheetSize::AnsiC => "C",
            SheetSize::AnsiD => "D",
            SheetSize::AnsiE => "E",
            SheetSize::Custom { .. } => "Custom",
        }
    }

    /// ANSI (inch) sheet sizes.
    pub fn is_ansi(self) -> bool {
        matches!(self, SheetSize::AnsiA | SheetSize::AnsiAPortrait | SheetSize::AnsiB | SheetSize::AnsiC | SheetSize::AnsiD | SheetSize::AnsiE)
    }
}

/// Layout of a sheet's title block.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TitleBlockStyle {
    /// Two columns of labeled fields (any field list).
    #[default]
    Simple,
    /// ISO 7200 style block with a projection symbol.
    Iso,
    /// ASME Y14.1 (ANSI) block with name/date rows and a tolerance block.
    Ansi,
    /// No title block.
    None,
}

impl TitleBlockStyle {
    pub const ALL: [TitleBlockStyle; 4] = [TitleBlockStyle::Simple, TitleBlockStyle::Iso, TitleBlockStyle::Ansi, TitleBlockStyle::None];
    pub fn name(self) -> &'static str {
        match self {
            TitleBlockStyle::Simple => "Simple",
            TitleBlockStyle::Iso => "ISO 7200",
            TitleBlockStyle::Ansi => "ANSI (ASME Y14.1)",
            TitleBlockStyle::None => "None",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Projection {
    ThirdAngle,
    FirstAngle,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ViewOrientation {
    Front,
    Back,
    Top,
    Bottom,
    Left,
    Right,
    Isometric,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ViewKind {
    /// A view of a Part Studio (all solid parts, or only `part`).
    Base {
        source: TabId,
        config: ConfigValues,
        orientation: ViewOrientation,
        #[serde(default)]
        part: Option<BodyId>,
    },
    /// Orthographic (or, for diagonal directions, isometric) projection of
    /// `parent`. `direction` is the unit sheet direction from the parent to
    /// this view chosen at creation (inferred from positions when absent).
    Projected {
        parent: ViewId,
        #[serde(default)]
        direction: Option<[f64; 2]>,
    },
    Auxiliary { parent: ViewId, edge_dir: [f64; 2] },
    /// Full section: `line` (two points in the parent's view coordinates,
    /// meters) is the cutting line; the viewing direction points from the
    /// side of the line the section view is placed on toward the line.
    Section { parent: ViewId, line: Vec<[f64; 2]>, label: String },
    /// Enlarged circular crop of the parent (`center`/`radius` in the
    /// parent's view coordinates, meters).
    Detail { parent: ViewId, center: [f64; 2], radius: f64, label: String },
    /// Aligned section: the cutting line runs from `a` to `center` and on
    /// to `b` (parent view coordinates). The view looks perpendicular to the
    /// `center`-`a` leg; the `center`-`b` leg (with the material behind
    /// it) is revolved about the axis through `center` into that plane.
    AlignedSection { parent: ViewId, center: [f64; 2], a: [f64; 2], b: [f64; 2], label: String },
}

/// Crop boundary of a view (view coordinates, meters).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Crop {
    Circle { center: [f64; 2], radius: f64 },
    /// Closed polygon (e.g. a rectangle).
    Polygon(Vec<[f64; 2]>),
    /// Closed smooth curve through the points (Catmull-Rom spline).
    Spline(Vec<[f64; 2]>),
}

/// A broken-out section of a view: material inside the closed `profile`
/// (view coordinates, meters; a smooth closed curve through the points when
/// `spline`, else a polygon) is removed from the viewer down to `depth`
/// (meters, measured along the line of sight from the model's nearest
/// point), exposing the interior locally. With `depth_ref` the depth runs
/// to that model point instead (associative "up to entity").
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BrokenOut {
    pub profile: Vec<[f64; 2]>,
    #[serde(default)]
    pub spline: bool,
    pub depth: f64,
    #[serde(default)]
    pub depth_ref: Option<PointRef>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum BreakStyle {
    #[default]
    ZigZag,
    Straight,
    Curve,
}

/// A break (broken view): the band between `a` and `b` (view coordinates,
/// meters, `a < b`) is removed and the geometry beyond it moved up to a gap
/// of `gap_mm` on the sheet. `vertical`: the break lines are vertical (the
/// band is an x interval); otherwise horizontal (a y interval).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ViewBreak {
    pub vertical: bool,
    pub a: f64,
    pub b: f64,
    #[serde(default = "default_gap")]
    pub gap_mm: f64,
    #[serde(default)]
    pub style: BreakStyle,
}

fn default_gap() -> f64 {
    6.0
}

/// A point of model geometry referenced by an annotation. Resolved against
/// the regenerated Part Studio and projected into the annotation's view every
/// time the sheet is built, so dimensions follow model changes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum PointRef {
    /// A B-rep vertex.
    Vertex { body: BodyId, name: TopoName },
    /// The center of a circular edge (also supplies the radius).
    Center { body: BodyId, name: TopoName },
    /// The middle of an edge (Shift+Q snaps).
    Midpoint { body: BodyId, name: TopoName },
    /// A quadrant point of a circular edge in the view: 0 right, 1 top,
    /// 2 left, 3 bottom (Shift+Q snaps).
    Quadrant { body: BodyId, name: TopoName, k: u8 },
}

impl PointRef {
    pub fn body(&self) -> &BodyId {
        match self {
            PointRef::Vertex { body, .. } | PointRef::Center { body, .. } | PointRef::Midpoint { body, .. } | PointRef::Quadrant { body, .. } => body,
        }
    }
}

/// Binding of one annotation input to model geometry. `slot` meaning per
/// annotation kind: linear dimension 0 = `a`, 1 = `b`; radial dimension and
/// centermark 0 = center/radius; angular dimension 0 = vertex, 1 = `a`,
/// 2 = `b`; centerline 0 = `a`, 1 = `b`; balloon 0 = anchor.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AnnotationRef {
    pub slot: u8,
    pub point: PointRef,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RenderStyle {
    HiddenLinesRemoved,
    HiddenLinesVisible,
    Shaded,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DrawingView {
    pub id: ViewId,
    pub kind: ViewKind,
    /// Position of the view center on the sheet (mm).
    pub position: [f64; 2],
    /// Scale; None = inherit from parent / sheet.
    pub scale: Option<f64>,
    pub style: RenderStyle,
    #[serde(default)]
    pub show_tangent_edges: bool,
    #[serde(default)]
    pub show_centerlines: bool,
    #[serde(default)]
    pub label: Option<String>,
    /// Rotation of the view on the sheet (radians, counter-clockwise).
    #[serde(default)]
    pub rotation: f64,
    #[serde(default)]
    pub crop: Option<Crop>,
    #[serde(default)]
    pub breaks: Vec<ViewBreak>,
    /// Exploded view (index into the assembly's exploded views) shown by a
    /// base view of an Assembly.
    #[serde(default)]
    pub explode: Option<usize>,
    /// Section views: partial section, the material is removed only along
    /// the cutting line (its ends are not extended past the model).
    #[serde(default)]
    pub partial: bool,
    /// Broken-out sections (base, projected and auxiliary views).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub broken_out: Vec<BrokenOut>,
    /// Base views: explicit view frame (viewing direction, up) replacing the
    /// standard orientation, e.g. for a projected view moved to another sheet
    /// without its parent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_frame: Option<[[f64; 3]; 2]>,
    /// Tangent edges drawn as phantom lines (View properties › Tangent
    /// edges › Phantom); `show_tangent_edges` off = Hidden.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub tangent_phantom: bool,
    /// View properties › Name (None: the orientation, e.g. "Front").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// View properties › Scale label (None: shown with the view label when
    /// the view has its own scale).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale_label: Option<bool>,
    /// "Suppress alignment with parent": a derived view placed freely (it
    /// neither slides along its alignment axis nor follows its parent).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub unaligned: bool,
    /// View simplification (None: the parent's, Auto for base views).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub simplification: Option<Simplification>,
}

/// Onshape's View simplification: geometry below a threshold is left out.
/// Absolute and Ratio to studio also leave out whole parts smaller than it;
/// Ratio to part keeps every part and simplifies the detail inside it.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Simplification {
    None,
    Auto,
    /// Threshold in meters.
    Absolute(f64),
    /// Fraction of the Part Studio / assembly size.
    RatioToStudio(f64),
    /// Fraction of each part's size.
    RatioToPart(f64),
}

impl Simplification {
    pub fn name(self) -> &'static str {
        match self {
            Simplification::None => "None",
            Simplification::Auto => "Auto",
            Simplification::Absolute(_) => "Absolute",
            Simplification::RatioToStudio(_) => "Ratio to studio",
            Simplification::RatioToPart(_) => "Ratio to part",
        }
    }
}

impl Default for DrawingView {
    fn default() -> Self {
        Self {
            id: ViewId(0),
            kind: ViewKind::Projected { parent: ViewId(0), direction: None },
            position: [0.0, 0.0],
            scale: None,
            style: RenderStyle::HiddenLinesRemoved,
            show_tangent_edges: true,
            show_centerlines: false,
            label: None,
            rotation: 0.0,
            crop: None,
            breaks: Vec::new(),
            explode: None,
            partial: false,
            broken_out: Vec::new(),
            custom_frame: None,
            tangent_phantom: false,
            name: None,
            scale_label: None,
            unaligned: false,
            simplification: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum AnnotationKind {
    Note { text: String, height_mm: f64 },
    LinearDimension { view: ViewId, a: [f64; 2], b: [f64; 2], offset: f64, orientation: DimOrientation, override_text: Option<String> },
    RadialDimension { view: ViewId, center: [f64; 2], radius: f64, diameter: bool, angle: f64 },
    AngularDimension { view: ViewId, vertex: [f64; 2], a: [f64; 2], b: [f64; 2], radius: f64 },
    Centermark { view: ViewId, center: [f64; 2], size: f64 },
    Centerline { view: ViewId, a: [f64; 2], b: [f64; 2] },
    Balloon { view: ViewId, anchor: [f64; 2], text: String },
    SurfaceFinish { anchor: [f64; 2], value: String },
    /// Feature control frame with its lower-left corner at `anchor` (sheet
    /// mm) and optional leaders to the toleranced geometry.
    Gdt {
        anchor: [f64; 2],
        frame: Vec<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        leaders: Vec<Leader>,
    },
    /// One ordinate value: distance of `point` from the group `origin`
    /// along x (`horizontal`) or y; the text sits at the annotation position.
    OrdinateDimension { view: ViewId, origin: [f64; 2], point: [f64; 2], horizontal: bool },
    /// Chamfer `a`-`b` measured against the adjacent edge `c`-`d`
    /// ("distance x angle").
    ChamferDimension { view: ViewId, a: [f64; 2], b: [f64; 2], c: [f64; 2], d: [f64; 2] },
    /// Length of the arc from `a` to `b` about `center` (on the side of
    /// `mid`); `offset` mm outside the arc.
    ArcLengthDimension { view: ViewId, center: [f64; 2], radius: f64, a: [f64; 2], b: [f64; 2], mid: [f64; 2], offset: f64 },
    /// Datum feature symbol: boxed `label` at the annotation position with a
    /// leader ending in a filled triangle at `anchor` (sheet mm).
    Datum { anchor: [f64; 2], label: String },
    /// Ordinate dimension group: one datum (`origin`) and the values of
    /// every point measured from it along x (`horizontal`) or y. The texts
    /// share one line through the annotation position; leaders jog when
    /// neighboring values would overlap. Reference slots: 0 = origin,
    /// `i + 1` = `points[i]`.
    OrdinateGroup { view: ViewId, origin: [f64; 2], points: Vec<[f64; 2]>, horizontal: bool },
    /// Maximum / minimum (arc condition) dimension between `a` and `b`
    /// (view coordinates). An end with a radius (`ra` / `rb` > 0) is a
    /// circle center; its condition picks the center, the near side (Min)
    /// or the far side (Max) of the circle. Slots: 0 = `a`, 1 = `b` (circle
    /// center references also update the radius).
    MaxMinDimension { view: ViewId, a: [f64; 2], ra: f64, ca: ArcCondition, b: [f64; 2], rb: f64, cb: ArcCondition, offset: f64, orientation: DimOrientation },
    /// Foreshortened radius: like a radial dimension, but the leader starts
    /// at a false center `jog` (view coordinates) and reaches the arc through
    /// a zigzag, for arcs whose true center is far away. Slot 0 = center.
    ForeshortenedRadial { view: ViewId, center: [f64; 2], radius: f64, angle: f64, jog: [f64; 2] },
    /// Foreshortened (half) diameter of a turned part: measured from the
    /// `axis` (two points, view coordinates) to `edge` and shown as a
    /// diameter with one arrow, the dimension line crossing the axis a
    /// little. `direct` shows the full diameter with both arrows. Slots:
    /// 0, 1 = axis points, 2 = edge.
    ForeshortenedDiameter { view: ViewId, axis: [[f64; 2]; 2], edge: [f64; 2], direct: bool },
    /// Datum target: target symbol at `anchor` (view coordinates) with a
    /// callout bubble (label below, area size above) at the annotation
    /// position. Slot 0 = anchor.
    DatumTarget { view: ViewId, anchor: [f64; 2], label: String, area: DatumTargetArea },
}

/// End condition of a maximum / minimum dimension at a circle.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArcCondition {
    #[default]
    Center,
    Min,
    Max,
}

/// A leader from an annotation to a point: view coordinates (meters) of
/// `view` when set (the leader follows the view), else sheet millimeters.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Leader {
    #[serde(default)]
    pub view: Option<ViewId>,
    pub point: [f64; 2],
}

/// Target area of a datum target (sizes in model meters).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum DatumTargetArea {
    /// Point target drawn as an X.
    #[default]
    Point,
    Circle { diameter: f64 },
    Rectangle { width: f64, height: f64 },
}

/// Units of a dimension value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DimUnits {
    Millimeter,
    Centimeter,
    Meter,
    Inch,
    Foot,
}

impl DimUnits {
    pub const ALL: [DimUnits; 5] = [DimUnits::Millimeter, DimUnits::Centimeter, DimUnits::Meter, DimUnits::Inch, DimUnits::Foot];
    /// Meters per unit.
    pub fn meters(self) -> f64 {
        match self {
            DimUnits::Millimeter => 0.001,
            DimUnits::Centimeter => 0.01,
            DimUnits::Meter => 1.0,
            DimUnits::Inch => 0.0254,
            DimUnits::Foot => 0.3048,
        }
    }
    pub fn symbol(self) -> &'static str {
        match self {
            DimUnits::Millimeter => "mm",
            DimUnits::Centimeter => "cm",
            DimUnits::Meter => "m",
            DimUnits::Inch => "in",
            DimUnits::Foot => "ft",
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            DimUnits::Millimeter => "Millimeters",
            DimUnits::Centimeter => "Centimeters",
            DimUnits::Meter => "Meters",
            DimUnits::Inch => "Inches",
            DimUnits::Foot => "Feet",
        }
    }
}

/// Tolerance of a dimension. Values are SI (meters for lengths, radians
/// for angles), shown in the dimension's units.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum Tolerance {
    #[default]
    None,
    /// ±value
    Symmetric { value: f64 },
    /// Stacked +upper / -lower deviations (lower is usually negative).
    Deviation { upper: f64, lower: f64 },
    /// Stacked limit values: nominal + upper over nominal + lower.
    Limits { upper: f64, lower: f64 },
    /// Theoretically exact (boxed) dimension.
    Basic,
    Min,
    Max,
    /// ISO fit: hole class (e.g. "H7") and/or shaft class (e.g. "g6").
    Fit { hole: String, shaft: String },
}

impl Tolerance {
    pub fn name(&self) -> &'static str {
        match self {
            Tolerance::None => "None",
            Tolerance::Symmetric { .. } => "Symmetrical",
            Tolerance::Deviation { .. } => "Deviation",
            Tolerance::Limits { .. } => "Limits",
            Tolerance::Basic => "Basic",
            Tolerance::Min => "MIN",
            Tolerance::Max => "MAX",
            Tolerance::Fit { .. } => "Fit",
        }
    }
}

/// Per-dimension display settings (Onshape's dimension palette).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DimFormat {
    /// Decimal places; None = drawing default (trailing zeros trimmed).
    #[serde(default)]
    pub precision: Option<u8>,
    /// None = drawing units.
    #[serde(default)]
    pub units: Option<DimUnits>,
    #[serde(default)]
    pub show_units: bool,
    #[serde(default)]
    pub tolerance: Tolerance,
    #[serde(default)]
    pub prefix: String,
    #[serde(default)]
    pub suffix: String,
}

impl DimFormat {
    pub fn is_default(&self) -> bool {
        *self == DimFormat::default()
    }
}

/// Fixed corner of a table (the corner that stays put when it resizes).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TableCorner {
    TopLeft,
    #[default]
    TopRight,
    BottomLeft,
    BottomRight,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum TableKind {
    /// Free table; the first row is a header when `header`.
    General {
        cells: Vec<Vec<String>>,
        #[serde(default)]
        title: Option<String>,
        #[serde(default)]
        header: bool,
    },
    /// Bill of materials of the assembly shown by `view`.
    Bom { view: ViewId },
    /// Holes (face-on circular bores) of `view`, located from `origin`
    /// (view coordinates, associative through `origin_ref`).
    Hole {
        view: ViewId,
        origin: [f64; 2],
        #[serde(default)]
        origin_ref: Option<PointRef>,
        #[serde(default)]
        precision: Option<u8>,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Table {
    pub id: u64,
    pub kind: TableKind,
    /// Sheet position (mm) of the fixed corner.
    pub position: [f64; 2],
    #[serde(default)]
    pub corner: TableCorner,
    /// Column widths in mm (empty or wrong length = fit to content).
    #[serde(default)]
    pub col_widths: Vec<f64>,
    #[serde(default = "default_row_height")]
    pub row_height: f64,
    #[serde(default = "default_table_text")]
    pub text_height: f64,
    /// Per-cell formatting (cells without an entry use the defaults).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cell_formats: Vec<CellStyle>,
    /// Merged cell ranges.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub merges: Vec<CellMerge>,
}

/// Horizontal alignment of a table cell's text.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CellAlign {
    Left,
    #[default]
    Center,
    Right,
}

/// Formatting of one table cell.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CellFormat {
    #[serde(default)]
    pub align: CellAlign,
    /// Text height (mm); None = the table's text height.
    #[serde(default)]
    pub height: Option<f64>,
    #[serde(default)]
    pub bold: bool,
}

/// Format of the cell at (`row`, `col`). Rows count every displayed row
/// below the title (a header row is row 0), like [`tables::TableLayout::cell_at`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CellStyle {
    pub row: usize,
    pub col: usize,
    pub format: CellFormat,
}

/// `rows` x `cols` cells starting at (`row`, `col`) shown as one cell
/// (the text of the top-left cell).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CellMerge {
    pub row: usize,
    pub col: usize,
    pub rows: usize,
    pub cols: usize,
}

impl CellMerge {
    pub fn contains(&self, r: usize, c: usize) -> bool {
        r >= self.row && r < self.row + self.rows && c >= self.col && c < self.col + self.cols
    }
    fn overlaps(&self, o: &CellMerge) -> bool {
        self.row < o.row + o.rows && o.row < self.row + self.rows && self.col < o.col + o.cols && o.col < self.col + self.cols
    }
    fn inside(&self, o: &CellMerge) -> bool {
        self.row >= o.row && self.col >= o.col && self.row + self.rows <= o.row + o.rows && self.col + self.cols <= o.col + o.cols
    }
}

fn default_row_height() -> f64 {
    7.0
}
fn default_table_text() -> f64 {
    3.0
}

impl Table {
    pub fn new(id: u64, kind: TableKind, position: [f64; 2]) -> Self {
        Self { id, kind, position, corner: TableCorner::TopRight, col_widths: Vec::new(), row_height: default_row_height(), text_height: default_table_text(), cell_formats: Vec::new(), merges: Vec::new() }
    }

    /// Formatting of a cell (defaults when none is set).
    pub fn cell_format(&self, row: usize, col: usize) -> CellFormat {
        self.cell_formats.iter().find(|s| s.row == row && s.col == col).map(|s| s.format.clone()).unwrap_or_default()
    }

    pub fn set_cell_format(&mut self, row: usize, col: usize, format: CellFormat) {
        self.cell_formats.retain(|s| !(s.row == row && s.col == col));
        if format != CellFormat::default() {
            self.cell_formats.push(CellStyle { row, col, format });
            self.cell_formats.sort_by_key(|s| (s.row, s.col));
        }
    }

    /// The merged range containing a cell.
    pub fn merge_at(&self, row: usize, col: usize) -> Option<CellMerge> {
        self.merges.iter().find(|m| m.contains(row, col)).copied()
    }

    /// The cell holding the text shown at (`row`, `col`): the top-left cell
    /// of a merged range, else the cell itself.
    pub fn anchor_cell(&self, row: usize, col: usize) -> (usize, usize) {
        self.merge_at(row, col).map(|m| (m.row, m.col)).unwrap_or((row, col))
    }

    /// Merge the cells between two corners (inclusive). Merges inside the
    /// range are absorbed; partial overlaps with other merges are rejected.
    pub fn merge_cells(&mut self, a: (usize, usize), b: (usize, usize)) -> bool {
        let m = CellMerge { row: a.0.min(b.0), col: a.1.min(b.1), rows: a.0.abs_diff(b.0) + 1, cols: a.1.abs_diff(b.1) + 1 };
        if m.rows * m.cols < 2 || self.merges.iter().any(|o| o.overlaps(&m) && !o.inside(&m)) {
            return false;
        }
        self.merges.retain(|o| !o.inside(&m));
        self.merges.push(m);
        true
    }

    /// Split the merged range containing a cell.
    pub fn unmerge(&mut self, row: usize, col: usize) -> bool {
        let n = self.merges.len();
        self.merges.retain(|m| !m.contains(row, col));
        self.merges.len() != n
    }

    fn general_cells(&mut self) -> Option<&mut Vec<Vec<String>>> {
        match &mut self.kind {
            TableKind::General { cells, .. } => Some(cells),
            _ => None,
        }
    }

    /// Insert an empty row at `at` (general tables), shifting formats and
    /// merges; a merge spanning the new row grows.
    pub fn insert_row(&mut self, at: usize) {
        if let Some(cells) = self.general_cells() {
            let n = cells.iter().map(|r| r.len()).max().unwrap_or(1).max(1);
            let at = at.min(cells.len());
            cells.insert(at, vec![String::new(); n]);
        }
        for s in &mut self.cell_formats {
            if s.row >= at {
                s.row += 1;
            }
        }
        for m in &mut self.merges {
            if m.row >= at {
                m.row += 1;
            } else if m.row + m.rows > at {
                m.rows += 1;
            }
        }
    }

    /// Delete row `r` (general tables keep at least one row).
    pub fn delete_row(&mut self, r: usize) -> bool {
        if let Some(cells) = self.general_cells() {
            if cells.len() <= 1 || r >= cells.len() {
                return false;
            }
            cells.remove(r);
        }
        self.cell_formats.retain(|s| s.row != r);
        for s in &mut self.cell_formats {
            if s.row > r {
                s.row -= 1;
            }
        }
        for m in &mut self.merges {
            if m.row > r {
                m.row -= 1;
            } else if m.row + m.rows > r {
                m.rows -= 1;
            }
        }
        self.merges.retain(|m| m.rows * m.cols >= 2 && m.rows > 0);
        true
    }

    /// Insert an empty column at `at`.
    pub fn insert_col(&mut self, at: usize) {
        if let Some(cells) = self.general_cells() {
            for row in cells.iter_mut() {
                let i = at.min(row.len());
                row.insert(i, String::new());
            }
        }
        if !self.col_widths.is_empty() {
            let w = self.col_widths.get(at.min(self.col_widths.len() - 1)).copied().unwrap_or(20.0);
            self.col_widths.insert(at.min(self.col_widths.len()), w);
        }
        for s in &mut self.cell_formats {
            if s.col >= at {
                s.col += 1;
            }
        }
        for m in &mut self.merges {
            if m.col >= at {
                m.col += 1;
            } else if m.col + m.cols > at {
                m.cols += 1;
            }
        }
    }

    /// Delete column `c` (general tables keep at least one column).
    pub fn delete_col(&mut self, c: usize) -> bool {
        if let Some(cells) = self.general_cells() {
            if cells.first().is_none_or(|r| r.len() <= 1) {
                return false;
            }
            for row in cells.iter_mut() {
                if c < row.len() {
                    row.remove(c);
                }
            }
        }
        if c < self.col_widths.len() {
            self.col_widths.remove(c);
        }
        self.cell_formats.retain(|s| s.col != c);
        for s in &mut self.cell_formats {
            if s.col > c {
                s.col -= 1;
            }
        }
        for m in &mut self.merges {
            if m.col > c {
                m.col -= 1;
            } else if m.col + m.cols > c {
                m.cols -= 1;
            }
        }
        self.merges.retain(|m| m.rows * m.cols >= 2 && m.cols > 0);
        true
    }
    pub fn type_name(&self) -> &'static str {
        match self.kind {
            TableKind::General { .. } => "Table",
            TableKind::Bom { .. } => "BOM table",
            TableKind::Hole { .. } => "Hole table",
        }
    }
    pub fn view(&self) -> Option<ViewId> {
        match self.kind {
            TableKind::Bom { view } | TableKind::Hole { view, .. } => Some(view),
            TableKind::General { .. } => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DimOrientation {
    Aligned,
    Horizontal,
    Vertical,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Annotation {
    pub id: u64,
    pub kind: AnnotationKind,
    /// Position on the sheet (mm) where relevant.
    pub position: [f64; 2],
    /// Associative references to model geometry (empty = free annotation).
    #[serde(default)]
    pub refs: Vec<AnnotationRef>,
    /// Dimension display settings (precision, units, tolerance, text).
    #[serde(default, skip_serializing_if = "DimFormat::is_default")]
    pub format: DimFormat,
}

impl Default for Annotation {
    fn default() -> Self {
        Self { id: 0, kind: AnnotationKind::Note { text: String::new(), height_mm: 3.5 }, position: [0.0, 0.0], refs: Vec::new(), format: DimFormat::default() }
    }
}

impl ViewKind {
    /// The parent view, for derived views.
    pub fn parent_mut(&mut self) -> Option<&mut ViewId> {
        match self {
            ViewKind::Base { .. } => None,
            ViewKind::Projected { parent, .. } | ViewKind::Auxiliary { parent, .. } | ViewKind::Section { parent, .. } | ViewKind::Detail { parent, .. } | ViewKind::AlignedSection { parent, .. } => Some(parent),
        }
    }
}

impl AnnotationKind {
    /// Mutable access to the view an annotation is attached to.
    pub fn view_mut(&mut self) -> Option<&mut ViewId> {
        match self {
            AnnotationKind::LinearDimension { view, .. }
            | AnnotationKind::RadialDimension { view, .. }
            | AnnotationKind::AngularDimension { view, .. }
            | AnnotationKind::Centermark { view, .. }
            | AnnotationKind::Centerline { view, .. }
            | AnnotationKind::Balloon { view, .. }
            | AnnotationKind::OrdinateDimension { view, .. }
            | AnnotationKind::ChamferDimension { view, .. }
            | AnnotationKind::ArcLengthDimension { view, .. }
            | AnnotationKind::OrdinateGroup { view, .. }
            | AnnotationKind::MaxMinDimension { view, .. }
            | AnnotationKind::ForeshortenedRadial { view, .. }
            | AnnotationKind::ForeshortenedDiameter { view, .. }
            | AnnotationKind::DatumTarget { view, .. } => Some(view),
            _ => None,
        }
    }

    /// The view an annotation is attached to, if any.
    pub fn view(&self) -> Option<ViewId> {
        match self {
            AnnotationKind::LinearDimension { view, .. }
            | AnnotationKind::RadialDimension { view, .. }
            | AnnotationKind::AngularDimension { view, .. }
            | AnnotationKind::Centermark { view, .. }
            | AnnotationKind::Centerline { view, .. }
            | AnnotationKind::Balloon { view, .. }
            | AnnotationKind::OrdinateDimension { view, .. }
            | AnnotationKind::ChamferDimension { view, .. }
            | AnnotationKind::ArcLengthDimension { view, .. }
            | AnnotationKind::OrdinateGroup { view, .. }
            | AnnotationKind::MaxMinDimension { view, .. }
            | AnnotationKind::ForeshortenedRadial { view, .. }
            | AnnotationKind::ForeshortenedDiameter { view, .. }
            | AnnotationKind::DatumTarget { view, .. } => Some(*view),
            _ => None,
        }
    }

    /// Dimensions (that show a measured value and accept a [`DimFormat`]).
    pub fn is_dimension(&self) -> bool {
        matches!(
            self,
            AnnotationKind::LinearDimension { .. }
                | AnnotationKind::RadialDimension { .. }
                | AnnotationKind::AngularDimension { .. }
                | AnnotationKind::OrdinateDimension { .. }
                | AnnotationKind::ArcLengthDimension { .. }
                | AnnotationKind::ChamferDimension { .. }
                | AnnotationKind::OrdinateGroup { .. }
                | AnnotationKind::MaxMinDimension { .. }
                | AnnotationKind::ForeshortenedRadial { .. }
                | AnnotationKind::ForeshortenedDiameter { .. }
        )
    }

    pub fn type_name(&self) -> &'static str {
        match self {
            AnnotationKind::Note { .. } => "Note",
            AnnotationKind::LinearDimension { .. } => "Linear dimension",
            AnnotationKind::RadialDimension { diameter: true, .. } => "Diameter dimension",
            AnnotationKind::RadialDimension { .. } => "Radial dimension",
            AnnotationKind::AngularDimension { .. } => "Angular dimension",
            AnnotationKind::Centermark { .. } => "Centermark",
            AnnotationKind::Centerline { .. } => "Centerline",
            AnnotationKind::Balloon { .. } => "Callout",
            AnnotationKind::SurfaceFinish { .. } => "Surface finish",
            AnnotationKind::Gdt { .. } => "Geometric tolerance",
            AnnotationKind::OrdinateDimension { .. } => "Ordinate dimension",
            AnnotationKind::ChamferDimension { .. } => "Chamfer dimension",
            AnnotationKind::ArcLengthDimension { .. } => "Arc length dimension",
            AnnotationKind::Datum { .. } => "Datum",
            AnnotationKind::OrdinateGroup { .. } => "Ordinate dimension group",
            AnnotationKind::MaxMinDimension { .. } => "Maximum / minimum dimension",
            AnnotationKind::ForeshortenedRadial { .. } => "Foreshortened radial dimension",
            AnnotationKind::ForeshortenedDiameter { .. } => "Foreshortened diameter dimension",
            AnnotationKind::DatumTarget { .. } => "Datum target",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TitleBlock {
    pub fields: Vec<(String, String)>,
}

impl Default for TitleBlock {
    fn default() -> Self {
        Self {
            fields: ["Title", "Drawn by", "Date", "Part number", "Revision", "Material", "Scale", "Sheet"]
                .into_iter()
                .map(|k| (k.to_string(), String::new()))
                .collect(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Sheet {
    pub name: String,
    pub size: SheetSize,
    pub scale: f64,
    pub views: Vec<DrawingView>,
    pub annotations: Vec<Annotation>,
    pub title_block: TitleBlock,
    #[serde(default)]
    pub tables: Vec<Table>,
    #[serde(default)]
    pub title_style: TitleBlockStyle,
}

impl Sheet {
    pub fn view(&self, id: ViewId) -> Option<&DrawingView> {
        self.views.iter().find(|v| v.id == id)
    }

    pub fn view_mut(&mut self, id: ViewId) -> Option<&mut DrawingView> {
        self.views.iter_mut().find(|v| v.id == id)
    }

    pub fn parent_of(&self, id: ViewId) -> Option<ViewId> {
        match &self.view(id)?.kind {
            ViewKind::Base { .. } => None,
            ViewKind::Projected { parent, .. }
            | ViewKind::Auxiliary { parent, .. }
            | ViewKind::Section { parent, .. }
            | ViewKind::Detail { parent, .. }
            | ViewKind::AlignedSection { parent, .. } => Some(*parent),
        }
    }

    /// `id` and all views derived from it (children, grandchildren...).
    pub fn descendants(&self, id: ViewId) -> Vec<ViewId> {
        let mut out = vec![id];
        let mut i = 0;
        while i < out.len() {
            let cur = out[i];
            for v in &self.views {
                if self.parent_of(v.id) == Some(cur) && !out.contains(&v.id) {
                    out.push(v.id);
                }
            }
            i += 1;
        }
        out
    }

    /// Remove a view, its descendants and every annotation and table
    /// attached to them.
    pub fn remove_view(&mut self, id: ViewId) {
        let gone = self.descendants(id);
        self.views.retain(|v| !gone.contains(&v.id));
        self.annotations.retain(|a| a.kind.view().is_none_or(|v| !gone.contains(&v)));
        self.tables.retain(|t| t.view().is_none_or(|v| !gone.contains(&v)));
        for a in &mut self.annotations {
            if let AnnotationKind::Gdt { leaders, .. } = &mut a.kind {
                leaders.retain(|l| l.view.is_none_or(|v| !gone.contains(&v)));
            }
        }
    }

    /// Points of a section view's cutting line in the parent's view
    /// coordinates (`a`, center, `b` for an aligned section).
    pub fn section_points(&self, id: ViewId) -> Option<Vec<[f64; 2]>> {
        match &self.view(id)?.kind {
            ViewKind::Section { line, .. } => Some(line.clone()),
            ViewKind::AlignedSection { center, a, b, .. } => Some(vec![*a, *center, *b]),
            _ => None,
        }
    }

    /// Replace a section view's cutting line points (see [`Self::section_points`]).
    pub fn set_section_points(&mut self, id: ViewId, pts: Vec<[f64; 2]>) -> bool {
        let Some(v) = self.view_mut(id) else { return false };
        match &mut v.kind {
            ViewKind::Section { line, .. } if pts.len() >= 2 => *line = pts,
            ViewKind::AlignedSection { center, a, b, .. } if pts.len() == 3 => {
                *a = pts[0];
                *center = pts[1];
                *b = pts[2];
            }
            _ => return false,
        }
        true
    }

    pub fn next_table_id(&self) -> u64 {
        self.tables.iter().map(|t| t.id).max().unwrap_or(0) + 1
    }

    pub fn table(&self, id: u64) -> Option<&Table> {
        self.tables.iter().find(|t| t.id == id)
    }

    pub fn table_mut(&mut self, id: u64) -> Option<&mut Table> {
        self.tables.iter_mut().find(|t| t.id == id)
    }

    pub fn next_view_id(&self) -> ViewId {
        ViewId(self.views.iter().map(|v| v.id.0).max().unwrap_or(0) + 1)
    }

    /// Annotation ids are small sequential numbers (they double as pick ids).
    pub fn next_annotation_id(&self) -> u64 {
        self.annotations.iter().map(|a| a.id).max().unwrap_or(0) + 1
    }

    /// Next free section/detail label: A-Y skipping I, O, Q, S, X, Z.
    pub fn next_label(&self) -> String {
        let used: Vec<String> = self
            .views
            .iter()
            .filter_map(|v| match &v.kind {
                ViewKind::Section { label, .. } | ViewKind::Detail { label, .. } | ViewKind::AlignedSection { label, .. } => Some(label.clone()),
                _ => None,
            })
            .collect();
        for round in 0.. {
            for c in "ABCDEFGHJKLMNPRTUVWY".chars() {
                let l = if round == 0 { c.to_string() } else { format!("{c}{round}") };
                if !used.contains(&l) {
                    return l;
                }
            }
        }
        unreachable!()
    }

    /// Value of a title block field.
    pub fn title_field(&self, key: &str) -> Option<&str> {
        self.title_block.fields.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }
}

impl Default for Sheet {
    fn default() -> Self {
        Self { name: "Sheet1".into(), size: SheetSize::A3, scale: 1.0, views: vec![], annotations: vec![], title_block: TitleBlock::default(), tables: vec![], title_style: TitleBlockStyle::Simple }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Drawing {
    pub sheets: Vec<Sheet>,
    pub projection: Projection,
    pub units_mm: bool,
    /// Drawing properties › Units and precision.
    #[serde(default)]
    pub props: DrawingProps,
}

impl Default for Drawing {
    fn default() -> Self {
        Self { sheets: vec![Sheet::default()], projection: Projection::ThirdAngle, units_mm: true, props: DrawingProps::default() }
    }
}

/// Onshape's Drawing properties › Units and precision (Primary and Leading
/// and trailing zeros). `None` precisions keep noshape's default (per-unit
/// digits, trailing zeros trimmed).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DrawingProps {
    /// Length units (None: millimeters or inches from `units_mm`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub units: Option<DimUnits>,
    /// Decimal separator: comma instead of period.
    #[serde(default)]
    pub decimal_comma: bool,
    /// Decimal places of lengths, tolerances, angles, angular tolerances.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub precision: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tolerance_precision: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub angular_precision: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub angular_tolerance_precision: Option<u8>,
    /// Lengths below 1 show "0.5" (true) or ".5".
    #[serde(default = "yes")]
    pub leading_zeros: bool,
    /// With a fixed precision, keep trailing zeros ("2.50").
    #[serde(default = "yes")]
    pub trailing_zeros: bool,
    #[serde(default = "yes")]
    pub tolerance_leading_zeros: bool,
    #[serde(default = "yes")]
    pub tolerance_trailing_zeros: bool,
}

fn yes() -> bool {
    true
}

impl Default for DrawingProps {
    fn default() -> Self {
        Self { units: None, decimal_comma: false, precision: None, tolerance_precision: None, angular_precision: None, angular_tolerance_precision: None, leading_zeros: true, trailing_zeros: true, tolerance_leading_zeros: true, tolerance_trailing_zeros: true }
    }
}

impl Drawing {
    /// "SheetN" with the smallest N not used by another sheet.
    pub fn unique_sheet_name(&self) -> String {
        (1..).map(|n| format!("Sheet{n}")).find(|n| !self.sheets.iter().any(|s| &s.name == n)).expect("unbounded")
    }

    /// Insert an empty sheet after `after` (same size, scale and title block
    /// layout and labels as that sheet); returns its index.
    pub fn add_sheet(&mut self, after: usize) -> usize {
        let name = self.unique_sheet_name();
        let mut sheet = Sheet { name, ..Sheet::default() };
        if let Some(src) = self.sheets.get(after) {
            sheet.size = src.size;
            sheet.scale = src.scale;
            sheet.title_block = src.title_block.clone();
            sheet.title_style = src.title_style;
        }
        let at = (after + 1).min(self.sheets.len());
        self.sheets.insert(at, sheet);
        at
    }

    /// Copy sheet `i` (views, annotations, tables) right after it.
    pub fn duplicate_sheet(&mut self, i: usize) -> Option<usize> {
        let mut copy = self.sheets.get(i)?.clone();
        let base = copy.name.clone();
        copy.name = (1..).map(|n| if n == 1 { format!("{base} copy") } else { format!("{base} copy {n}") }).find(|n| !self.sheets.iter().any(|s| &s.name == n)).expect("unbounded");
        self.sheets.insert(i + 1, copy);
        Some(i + 1)
    }

    /// Delete sheet `i`; the last remaining sheet cannot be deleted.
    pub fn delete_sheet(&mut self, i: usize) -> bool {
        if self.sheets.len() <= 1 || i >= self.sheets.len() {
            return false;
        }
        self.sheets.remove(i);
        true
    }

    /// Move sheet `from` to index `to` (clamped).
    pub fn move_sheet(&mut self, from: usize, to: usize) -> bool {
        if from >= self.sheets.len() {
            return false;
        }
        let to = to.min(self.sheets.len() - 1);
        if from == to {
            return false;
        }
        let s = self.sheets.remove(from);
        self.sheets.insert(to, s);
        true
    }

    /// Move views of sheet `from` (each with every view derived from it,
    /// the annotations and tables attached to them) to sheet `to`. Views get
    /// new ids on the target sheet; the returned pairs map old to new ids.
    ///
    /// A moved view whose parent stays behind becomes an independent base
    /// view showing the same projection (explicit frame, effective scale and
    /// rotation; a detail view becomes a circle-cropped view). Section views
    /// need their parent's cutting line, so they only move with their parent.
    /// `sources` (when given) keeps converted detail views' content in place.
    pub fn move_views_to_sheet(&mut self, from: usize, ids: &[ViewId], to: usize, sources: Option<&views::Sources>) -> Result<Vec<(ViewId, ViewId)>, String> {
        if from == to || from >= self.sheets.len() || to >= self.sheets.len() {
            return Err("Choose another sheet".into());
        }
        let src = &self.sheets[from];
        let mut moved: Vec<ViewId> = Vec::new();
        for id in ids {
            if src.view(*id).is_none() {
                return Err("View not found".into());
            }
            for d in src.descendants(*id) {
                if !moved.contains(&d) {
                    moved.push(d);
                }
            }
        }
        // Views in sheet order (parents before children).
        let order: Vec<ViewId> = src.views.iter().map(|v| v.id).filter(|v| moved.contains(v)).collect();
        let mut out_views: Vec<DrawingView> = Vec::new();
        for id in &order {
            let mut v = src.view(*id).cloned().expect("listed above");
            let orphan = src.parent_of(*id).is_some_and(|p| !moved.contains(&p));
            if orphan {
                if matches!(v.kind, ViewKind::Section { .. } | ViewKind::AlignedSection { .. }) {
                    return Err("A section view can only move together with its parent view".into());
                }
                let frame = views::view_frame(self, src, *id).ok_or("The view cannot be computed")?;
                let (key, part) = views::view_source(src, *id).ok_or("The view has no source")?;
                let scale = views::view_scale(src, *id);
                let rotation = views::view_rotation(src, *id);
                let root_orientation = match views::root_view(src, *id).and_then(|r| src.view(r)).map(|r| &r.kind) {
                    Some(ViewKind::Base { orientation, .. }) => *orientation,
                    _ => ViewOrientation::Front,
                };
                if let ViewKind::Detail { center, radius, label, .. } = &v.kind {
                    v.crop = Some(Crop::Circle { center: *center, radius: *radius });
                    if v.label.is_none() {
                        v.label = Some(format!("DETAIL {label}"));
                    }
                    // Keep the detail content where it was on the sheet.
                    if let Some(o) = sources.and_then(|s| views::source_origin(s, &key, part.as_ref(), &frame)) {
                        let k = scale * 1000.0;
                        let d = views::rotate2([(o[0] - center[0]) * k, (o[1] - center[1]) * k], rotation);
                        v.position = [v.position[0] + d[0], v.position[1] + d[1]];
                    }
                }
                v.kind = ViewKind::Base { source: key.tab, config: key.config, orientation: root_orientation, part };
                v.explode = key.explode;
                v.custom_frame = Some([frame.dir.to_array(), frame.up.to_array()]);
                v.scale = Some(scale);
                v.rotation = rotation;
            }
            out_views.push(v);
        }
        // New ids on the target sheet.
        let dst = &self.sheets[to];
        let mut next = dst.next_view_id().0;
        let map: Vec<(ViewId, ViewId)> = order
            .iter()
            .map(|id| {
                let n = ViewId(next);
                next += 1;
                (*id, n)
            })
            .collect();
        let remap = |id: ViewId| map.iter().find(|(o, _)| *o == id).map(|(_, n)| *n).unwrap_or(id);
        for v in &mut out_views {
            v.id = remap(v.id);
            if let Some(p) = v.kind.parent_mut() {
                *p = remap(*p);
            }
        }
        let mut anns: Vec<Annotation> = Vec::new();
        for (id, a) in (dst.next_annotation_id()..).zip(src.annotations.iter().filter(|a| a.kind.view().is_some_and(|v| moved.contains(&v)))) {
            let mut a = a.clone();
            a.id = id;
            if let Some(v) = a.kind.view_mut() {
                *v = remap(*v);
            }
            anns.push(a);
        }
        let mut tbls: Vec<Table> = Vec::new();
        for (id, t) in (dst.next_table_id()..).zip(src.tables.iter().filter(|t| t.view().is_some_and(|v| moved.contains(&v)))) {
            let mut t = t.clone();
            t.id = id;
            match &mut t.kind {
                TableKind::Bom { view } | TableKind::Hole { view, .. } => *view = remap(*view),
                TableKind::General { .. } => {}
            }
            tbls.push(t);
        }
        let src = &mut self.sheets[from];
        src.views.retain(|v| !moved.contains(&v.id));
        src.annotations.retain(|a| a.kind.view().is_none_or(|v| !moved.contains(&v)));
        src.tables.retain(|t| t.view().is_none_or(|v| !moved.contains(&v)));
        for a in &mut src.annotations {
            if let AnnotationKind::Gdt { leaders, .. } = &mut a.kind {
                leaders.retain(|l| l.view.is_none_or(|v| !moved.contains(&v)));
            }
        }
        let dst = &mut self.sheets[to];
        dst.views.extend(out_views);
        dst.annotations.extend(anns);
        dst.tables.extend(tbls);
        Ok(map)
    }

    /// Rename sheet `i`; empty names are rejected.
    pub fn rename_sheet(&mut self, i: usize, name: &str) -> bool {
        let name = name.trim();
        match self.sheets.get_mut(i) {
            Some(s) if !name.is_empty() => {
                s.name = name.to_string();
                true
            }
            _ => false,
        }
    }
}

pub mod build;
pub mod clipboard;
pub mod create;
pub mod export;
pub mod font;
pub mod format;
pub mod sheet;
pub mod showcase;
pub mod tables;
pub mod templates;
pub mod views;

#[cfg(test)]
mod tests;
