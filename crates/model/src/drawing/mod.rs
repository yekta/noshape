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
            SheetSize::AnsiB => (431.8, 279.4),
            SheetSize::AnsiC => (558.8, 431.8),
            SheetSize::AnsiD => (863.6, 558.8),
            SheetSize::AnsiE => (1117.6, 863.6),
            SheetSize::Custom { width_mm, height_mm } => (width_mm, height_mm),
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
    Gdt { anchor: [f64; 2], frame: Vec<String> },
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
}

fn default_row_height() -> f64 {
    7.0
}
fn default_table_text() -> f64 {
    3.0
}

impl Table {
    pub fn new(id: u64, kind: TableKind, position: [f64; 2]) -> Self {
        Self { id, kind, position, corner: TableCorner::TopRight, col_widths: Vec::new(), row_height: default_row_height(), text_height: default_table_text() }
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

impl AnnotationKind {
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
            | AnnotationKind::ArcLengthDimension { view, .. } => Some(*view),
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
        Self { name: "Sheet1".into(), size: SheetSize::A3, scale: 1.0, views: vec![], annotations: vec![], title_block: TitleBlock::default(), tables: vec![] }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Drawing {
    pub sheets: Vec<Sheet>,
    pub projection: Projection,
    pub units_mm: bool,
}

impl Default for Drawing {
    fn default() -> Self {
        Self { sheets: vec![Sheet::default()], projection: Projection::ThirdAngle, units_mm: true }
    }
}

impl Drawing {
    /// "SheetN" with the smallest N not used by another sheet.
    pub fn unique_sheet_name(&self) -> String {
        (1..).map(|n| format!("Sheet{n}")).find(|n| !self.sheets.iter().any(|s| &s.name == n)).expect("unbounded")
    }

    /// Insert an empty sheet after `after` (same size, scale and title block
    /// labels as that sheet); returns its index.
    pub fn add_sheet(&mut self, after: usize) -> usize {
        let name = self.unique_sheet_name();
        let mut sheet = Sheet { name, ..Sheet::default() };
        if let Some(src) = self.sheets.get(after) {
            sheet.size = src.size;
            sheet.scale = src.scale;
            sheet.title_block = src.title_block.clone();
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
pub mod export;
pub mod font;
pub mod format;
pub mod sheet;
pub mod showcase;
pub mod tables;
pub mod views;

#[cfg(test)]
mod tests;
