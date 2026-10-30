//! The drawing editor's tools, picks, selection and drags (from
//! `crates/app/src/drawing_view.rs`).

use noshape_model::document::TabId;
use noshape_model::drawing::*;

/// A mouse button (the drawing editor's drags; no GUI types here).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    Left,
    Right,
    Middle,
    /// Navigation (back / forward) buttons.
    Other,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DimTool {
    Linear,
    Radius,
    Diameter,
    Angle,
    /// 3 point angular: the vertex, then a point on each leg.
    Angle3,
    Chamfer,
    ArcLength,
    /// Maximum / minimum (arc condition) dimension.
    MaxMin,
    /// Foreshortened (half) diameter from an axis.
    Foreshorten,
    /// 2 point linear: two points only (help H:drawing_-_2_point_linear_dimension).
    Point2,
    /// Point to line: one point and one line.
    PointLine,
    /// Line to line: two parallel lines.
    LineLine,
}

impl DimTool {
    /// The linear dimension tools (smart Dimension and the typed ones).
    pub fn linear(self) -> bool {
        matches!(self, DimTool::Linear | DimTool::Point2 | DimTool::PointLine | DimTool::LineLine)
    }

    pub fn name(self) -> &'static str {
        match self {
            DimTool::Linear => "Linear dimension",
            DimTool::Point2 => "2 point linear dimension",
            DimTool::PointLine => "Point to line dimension",
            DimTool::LineLine => "Line to line dimension",
            DimTool::Radius => "Radial dimension",
            DimTool::Diameter => "Diameter dimension",
            DimTool::Angle => "Angular dimension",
            DimTool::Angle3 => "3 point angular dimension",
            DimTool::Chamfer => "Chamfer dimension",
            DimTool::ArcLength => "Arc length dimension",
            DimTool::MaxMin => "Maximum / minimum dimension",
            DimTool::Foreshorten => "Foreshortened diameter",
        }
    }
}

/// A point picked in a view (view coordinates) with its model reference.
#[derive(Clone, Debug)]
pub struct PickedPoint {
    pub view: ViewId,
    pub p: [f64; 2],
    pub r: Option<PointRef>,
}

#[derive(Clone, Debug)]
pub enum Pick {
    Point(PickedPoint),
    Line { a: PickedPoint, b: PickedPoint },
    Circle { view: ViewId, center: [f64; 2], radius: f64, r: Option<PointRef> },
    /// An arc edge: center, radius, end points and a point on the arc.
    Arc { view: ViewId, center: [f64; 2], radius: f64, a: PickedPoint, b: PickedPoint, mid: [f64; 2], r: Option<PointRef> },
    /// A circle clicked at `at` (view coordinates; max/min dimensions).
    CircleAt { view: ViewId, center: [f64; 2], radius: f64, r: Option<PointRef>, at: [f64; 2] },
}

/// Kinds of drawing tables the table tools create.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TableTool {
    General,
    Bom,
    Hole,
    Revision,
    CutList,
    /// FeatureScript custom table of a Part Studio.
    Custom,
}

/// A model edge (body and topological name), for associative references.
pub type EdgeRef = (noshape_model::BodyId, noshape_model::reference::TopoName);

#[derive(Clone, Debug)]
pub enum Tool {
    Select,
    InsertView { source: TabId, orientation: ViewOrientation, part: Option<noshape_model::BodyId> },
    Projected { parent: Option<ViewId> },
    Auxiliary { parent: Option<(ViewId, [f64; 2])> },
    /// Section view; `partial`: the cutting line is used as clicked (not
    /// extended across the view) and only cuts along its length.
    /// `jogs`: snap points clicked after the line, each jogging the cutting
    /// line through it (help drawing_section_view step 7).
    Section { parent: Option<ViewId>, p0: Option<[f64; 2]>, line: Option<([f64; 2], [f64; 2])>, partial: bool, jogs: Vec<[f64; 2]> },
    Detail { parent: Option<ViewId>, center: Option<[f64; 2]>, radius: Option<f64> },
    Dimension { kind: DimTool, picks: Vec<Pick> },
    Note,
    Centermark,
    Centerline { first: Option<PickedPoint> },
    /// Hole / thread callout: the picked hole circle, then the placement.
    HoleCallout { circle: Option<(ViewId, [f64; 2], f64, Option<PointRef>)> },
    /// Edge-to-edge centerline: one cylinder silhouette edge, or two edges.
    EdgeCenterline { first: Option<(PickedPoint, PickedPoint)> },
    /// Circle centerline through 3 points, or (`three` false) center + point.
    CircleCenterline { three: bool, points: Vec<PickedPoint> },
    /// Virtual sharp: two straight edges (with the first edge's reference).
    VirtualSharp { first: Option<(PickedPoint, PickedPoint, Option<EdgeRef>)> },
    Balloon { anchor: Option<PickedPoint> },
    /// Jogged section: orthogonal cutting line points (parent view coords);
    /// `done` once the line is finished (Enter / double-click).
    JoggedSection { parent: Option<ViewId>, points: Vec<[f64; 2]>, done: bool },
    AlignedSection { parent: Option<ViewId>, center: Option<[f64; 2]>, a: Option<[f64; 2]>, b: Option<[f64; 2]> },
    /// Crop a view with a rectangle (two corners) or a circle (center, radius).
    Crop { parent: Option<ViewId>, first: Option<[f64; 2]>, circle: bool },
    /// Break a view: two clicks give the removed band.
    Break { parent: Option<ViewId>, first: Option<[f64; 2]>, vertical: bool },
    /// Ordinate dimensions: datum, then the side/level of the values, then
    /// points, all in one ordinate group (`group` once created).
    Ordinate { origin: Option<PickedPoint>, horizontal: Option<bool>, group: Option<u64> },
    /// Place a table: BOM / hole tables need a view (and hole tables an origin).
    Table { kind: TableTool, view: Option<ViewId>, origin: Option<PickedPoint> },
    Gdt,
    Datum { anchor: Option<[f64; 2]> },
    SurfaceFinish,
    /// Weld symbol (drawing_weld.rs): click the joint, then the place.
    Weld,
    /// Place a revision callout of `revision` (drawing_revision.rs).
    RevisionCallout { revision: String },
    /// Broken-out section: profile points in the view (view coordinates),
    /// closed with Enter / double-click; then a depth (panel) or a depth point.
    BrokenOut { parent: Option<ViewId>, points: Vec<[f64; 2]>, done: bool },
    /// Polygon or spline crop: points in the view (view coordinates).
    CropPoly { parent: Option<ViewId>, points: Vec<[f64; 2]>, spline: bool },
    DatumTarget { anchor: Option<PickedPoint> },
    /// Add a leader to geometric tolerance `id`.
    GdtLeader { id: u64 },
    /// Drawing sketch tools (see `drawing_sketch`): the clicked points and
    /// the sketch point the next segment starts from.
    Sketch { tool: super::SkTool, pts: Vec<[f64; 2]>, from: Option<u32> },
    /// Drawing constraints: the picked sketch entities.
    Constrain { kind: super::SkCon, picks: Vec<u32> },
    /// Insert image: the file, its aspect ratio and the drag start.
    PlaceImage { name: String, data: std::sync::Arc<str>, aspect: f64, start: Option<[f64; 2]> },
}

impl Tool {
    pub fn name(&self) -> &'static str {
        match self {
            Tool::Select => "Select",
            Tool::InsertView { .. } => "Insert view",
            Tool::Projected { .. } => "Projected view",
            Tool::Auxiliary { .. } => "Auxiliary view",
            Tool::Section { .. } => "Section view",
            Tool::Detail { .. } => "Detail view",
            Tool::Dimension { kind, .. } => kind.name(),
            Tool::Note => "Note",
            Tool::Centermark => "Centermark",
            Tool::Centerline { .. } => "Centerline",
            Tool::EdgeCenterline { .. } => "Edge to edge centerline",
            Tool::HoleCallout { .. } => "Hole/thread callout",
            Tool::CircleCenterline { three: true, .. } => "3 point circle centerline",
            Tool::CircleCenterline { .. } => "2 point circle centerline",
            Tool::VirtualSharp { .. } => "Virtual sharp",
            Tool::Balloon { .. } => "Callout",
            Tool::JoggedSection { .. } => "Jogged section view",
            Tool::AlignedSection { .. } => "Aligned section view",
            Tool::Crop { .. } => "Crop view",
            Tool::Break { .. } => "Break view",
            Tool::Ordinate { .. } => "Ordinate dimension",
            Tool::Table { kind: TableTool::General, .. } => "Table",
            Tool::Table { kind: TableTool::Bom, .. } => "BOM table",
            Tool::Table { kind: TableTool::Hole, .. } => "Hole table",
            Tool::Table { kind: TableTool::Revision, .. } => "Revision table",
            Tool::Table { kind: TableTool::CutList, .. } => "Cut list table",
            Tool::Table { kind: TableTool::Custom, .. } => "Custom table",
            Tool::Gdt => "Geometric tolerance",
            Tool::Datum { .. } => "Datum",
            Tool::SurfaceFinish => "Surface finish",
            Tool::Weld => "Weld symbol",
            Tool::RevisionCallout { .. } => "Revision callout",
            Tool::BrokenOut { .. } => "Broken-out section",
            Tool::CropPoly { spline: false, .. } => "Crop view (polygon)",
            Tool::CropPoly { .. } => "Crop view (spline)",
            Tool::DatumTarget { .. } => "Datum target",
            Tool::GdtLeader { .. } => "Add leader",
            Tool::Sketch { tool, .. } => tool.name(),
            Tool::Constrain { kind, .. } => kind.name(),
            Tool::PlaceImage { .. } => "Insert image",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sel {
    View(ViewId),
    Annotation(u64),
    Table(u64),
    /// Break `1` of view `0`.
    Break(ViewId, usize),
    /// A drawing sketch entity / hatch region.
    Sketch(u32),
    Hatch(u64),
    /// An inserted image.
    Image(u64),
    /// An automatic centermark of a view (`build::centermark_key`).
    AutoCentermark(ViewId, [i64; 2]),
    /// The label under a view, selected on its own (live: orange fill, a
    /// grip at each end; dragged freely).
    ViewLabel(ViewId),
    /// An auxiliary view's reference indicator, selected on its own (live
    /// u-906: orange, its parent edge orange, the view framed light blue).
    AuxIndicator(ViewId),
    /// A hole table's tag axis (its own menu: Hole table properties, flips).
    HoleAxis(u64),
}

#[derive(Clone, Debug)]
pub enum Drag {
    None,
    Pending { button: Button, start: [f64; 2], start_px: [f64; 2], pick: Option<u32> },
    Pan { last_px: [f64; 2] },
    /// Drag zoom about `anchor` (mouse-control presets with a zoom drag).
    Zoom { anchor: [f64; 2], last_px: [f64; 2] },
    MoveViews { start: [f64; 2], orig: Vec<(ViewId, [f64; 2])>, constraint: Option<([f64; 2], [f64; 2])> },
    MoveAnnotation { start: [f64; 2], orig: Annotation },
    /// A jog snap point of a linear dimension (extension line `k`; `far`:
    /// the diagonal's end).
    Jog { orig: Annotation, k: usize, far: bool },
    /// An attachment point (end `k`) of a linear dimension dragged to
    /// another edge, point or center (re-attaches that end).
    Reattach { orig: Annotation, k: usize },
    /// An arm end of a centermark (`snap`: the red diamond on the edge it
    /// snapped to).
    CentermarkArm { orig: Annotation, k: usize, snap: Option<[f64; 2]> },
    /// An end grip of a centerline (along its line).
    CenterlineEnd { orig: Annotation, k: usize },
    MoveTable { id: u64, start: [f64; 2], orig: [f64; 2] },
    /// Drag-select table cells: a rubber band from `start` (help H:table:
    /// the cells it touches are selected on release).
    SelectCells { id: u64, start: [f64; 2] },
    /// Box select: a left-drag from empty sheet (live navigation/44: a blue
    /// window box; what it wholly encloses is selected on release).
    SelectBox { start: [f64; 2] },
    /// Resize a table from its free corner (scales column widths and rows).
    /// A resize grip: `axis` 0 the columns, 1 the rows (live 1607 (b)).
    ResizeTable { id: u64, start: [f64; 2], orig: Table, size: [f64; 2], widths: Vec<f64>, axis: usize },
    /// A table divider: column boundary `col` or row boundary `row` (index
    /// into the layout's xs / ys), with the widths and heights at the start.
    TableDivider { id: u64, col: Option<usize>, row: Option<usize>, start: [f64; 2], widths: Vec<f64>, heights: Vec<f64> },
    /// Move a section view's cutting line (parallel offset).
    MoveSectionLine { view: ViewId, start: [f64; 2], orig: ViewKind },
    /// A view break dragged by one of its lines along its axis (help V35).
    MoveBreak { view: ViewId, index: usize, start: [f64; 2], orig: noshape_model::drawing::ViewBreak },
    /// Move a detail view's circle in its parent.
    MoveDetailCircle { view: ViewId, start: [f64; 2], orig: [f64; 2] },
    /// Resize a detail view's circle by its grip (live asbv-9).
    ResizeDetailCircle { view: ViewId },
    /// Move a detail circle's label on its leader (`orig`: its
    /// `detail_label_at`).
    MoveDetailLabel { view: ViewId, start: [f64; 2], orig: [f64; 2] },
    /// Drag a snap point of an auxiliary view's reference indicator (0 / 1
    /// an end, 2 the middle).
    AuxIndicator { view: ViewId, grip: usize, orig: noshape_model::drawing::AuxRef },
    /// Move view `view`'s label on its own (`orig`: its `label_at`).
    MoveViewLabel { view: ViewId, start: [f64; 2], orig: Option<[f64; 2]> },
    /// Move one point of a section view's cutting line.
    MoveSectionPoint { view: ViewId, index: usize, orig: Vec<[f64; 2]> },
    /// Drag annotation `0`'s inspection item onto another annotation.
    MoveInspection(u64),
    /// Move an inserted image.
    MoveImage { id: u64, start: [f64; 2], orig: [f64; 2] },
    /// A selected note's grip: 0 / 1 the left / right side (bounding box
    /// width), 2 the middle (rotate; notes without leaders).
    NoteGrip { id: u64, grip: u8, start: [f64; 2], orig: Annotation },
    /// A note leader's bend: the landing's length (live "Move by grip").
    LeaderBend { id: u64, leader: usize, start: [f64; 2], orig: Annotation },
    /// A note's or callout's leader anchor grip: detach / re-attach.
    LeaderEnd { id: u64, leader: usize, orig: Annotation },
    /// Drag a drawing sketch entity (from `orig`).
    MoveSketch { id: u32, start: [f64; 2], orig: noshape_model::drawing::sheet_sketch::SheetSketch },
}

