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
    /// ISO / JIS A4 portrait (210 x 297 mm; Onshape's ISO_A4_Portrait template).
    A4Portrait,
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
            SheetSize::A4Portrait => (210.0, 297.0),
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
    pub const STANDARD: [SheetSize; 12] = [SheetSize::A4, SheetSize::A4Portrait, SheetSize::A3, SheetSize::A2, SheetSize::A1, SheetSize::A0, SheetSize::AnsiA, SheetSize::AnsiAPortrait, SheetSize::AnsiB, SheetSize::AnsiC, SheetSize::AnsiD, SheetSize::AnsiE];

    /// The name in Onshape's size lists ("A Portrait", "A4 Portrait", else
    /// the designation).
    pub fn list_name(self) -> &'static str {
        match self {
            SheetSize::AnsiAPortrait => "A Portrait",
            SheetSize::A4Portrait => "A4 Portrait",
            s => s.label(),
        }
    }

    /// Onshape's size list of a standard (live drawing/800–825: ANSI A, A
    /// Portrait, B, C, D; ISO and JIS A4, A4 Portrait, A3, A2, A1, A0).
    pub fn standard_list(ansi: bool) -> &'static [SheetSize] {
        if ansi {
            &[SheetSize::AnsiA, SheetSize::AnsiAPortrait, SheetSize::AnsiB, SheetSize::AnsiC, SheetSize::AnsiD]
        } else {
            &[SheetSize::A4, SheetSize::A4Portrait, SheetSize::A3, SheetSize::A2, SheetSize::A1, SheetSize::A0]
        }
    }

    /// Size designation for title blocks ("A3", "B", "Custom").
    pub fn label(self) -> &'static str {
        match self {
            SheetSize::A4 | SheetSize::A4Portrait => "A4",
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

    /// Horizontal x vertical zones of Onshape's template for this size
    /// (live asb 10-24 "template frames", samples/tpl-*.dxf: ANSI A 2 x 2,
    /// B and C 4 x 4, D 8 x 8; ISO / JIS A3 6 x 4, A2 8 x 6, A1 12 x 8, A0
    /// 16 x 12, A4 none); custom sizes get zones about 66 mm wide.
    pub fn standard_zones(self) -> [u32; 2] {
        match self {
            SheetSize::AnsiA | SheetSize::AnsiAPortrait => [2, 2],
            SheetSize::AnsiB | SheetSize::AnsiC => [4, 4],
            SheetSize::AnsiD | SheetSize::AnsiE => [8, 8],
            SheetSize::A4 | SheetSize::A4Portrait => [0, 0],
            SheetSize::A3 => [6, 4],
            SheetSize::A2 => [8, 6],
            SheetSize::A1 => [12, 8],
            SheetSize::A0 => [16, 12],
            SheetSize::Custom { .. } => {
                let (w, h) = self.size_mm();
                [((w - 20.0) / 66.0).round().max(2.0) as u32, ((h - 20.0) / 66.0).round().max(2.0) as u32]
            }
        }
    }
}

impl SheetSize {
    /// The frame's inset at the bottom and top (mm) in Onshape's templates
    /// (live asb 10-24 "template frames"): ANSI A 0.38 in, B 0.5 in, C
    /// 0.62 in, D 0.75 in; ISO / JIS A4..A2 10 mm, A1 and A0 20 mm.
    pub fn frame_margin(self) -> f64 {
        match self {
            SheetSize::AnsiA | SheetSize::AnsiAPortrait => 0.38 * 25.4,
            SheetSize::AnsiB => 0.5 * 25.4,
            SheetSize::AnsiC => 0.62 * 25.4,
            SheetSize::AnsiD | SheetSize::AnsiE => 0.75 * 25.4,
            SheetSize::A1 | SheetSize::A0 => 20.0,
            SheetSize::A4 | SheetSize::A4Portrait | SheetSize::A3 | SheetSize::A2 | SheetSize::Custom { .. } => 10.0,
        }
    }

    /// The border frame [x0, y0, x1, y1] (mm): `frame_margin` m at the
    /// bottom and top, 1.05 m at the left and 0.95 m at the right (live's
    /// templates sit 0.05 m right of centre); A4 runs from 20 mm to the
    /// right edge (live ISO_A4 / JIS_A4, portrait too).
    pub fn frame_rect(self) -> [f64; 4] {
        let (w, h) = self.size_mm();
        let m = self.frame_margin();
        match self {
            SheetSize::A4 | SheetSize::A4Portrait => [20.0, m, w, h - m],
            SheetSize::Custom { .. } => [m, m, w - m, h - m],
            _ => [1.05 * m, m, w - 0.95 * m, h - m],
        }
    }

    /// The Onshape title block's height (mm; live: 1.5 in on ANSI A / ISO
    /// A4, 2 in on B, C, A3, A2, 2.5 in on D, A1, A0; everything in it
    /// scales with it).
    pub fn title_block_height(self) -> f64 {
        match self {
            SheetSize::AnsiB | SheetSize::AnsiC | SheetSize::A3 | SheetSize::A2 => 50.8,
            SheetSize::AnsiD | SheetSize::AnsiE | SheetSize::A1 | SheetSize::A0 => 63.5,
            _ => 38.1,
        }
    }

    /// Zone label height (mm; live: ANSI A 0.18 in, B..D 0.24 in, ISO / JIS
    /// 6 mm).
    pub fn zone_label_height(self) -> f64 {
        match self {
            SheetSize::AnsiA | SheetSize::AnsiAPortrait => 0.18 * 25.4,
            s if s.is_ansi() => 0.24 * 25.4,
            _ => 6.0,
        }
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
    /// `cuts`: the points the cutting line passes through (Edit section's
    /// orange dots); with several the line steps between them.
    Section {
        parent: ViewId,
        line: Vec<[f64; 2]>,
        label: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        cuts: Vec<[f64; 2]>,
    },
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

/// Edit hatch section... (live asb 10-23 views (4)): one hatch region of a
/// view's cut restyled. `at` (view coordinates) lies inside the region, so
/// the style stays with the region when the view regenerates.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SectionHatch {
    pub at: [f64; 2],
    /// Pattern group and index ([`hatch_pat::pattern`]); `pattern` None:
    /// General › None (the region unhatched).
    pub standard: usize,
    pub pattern: Option<usize>,
    /// Scale (1: the pattern's own size); angle in degrees added to the
    /// pattern's own.
    pub scale: f64,
    pub angle: f64,
    /// None: the drawing's hatch color.
    pub color: Option<[u8; 3]>,
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
    SmallZigZag,
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
    /// Where two straight edges meet in the view (a virtual sharp).
    Intersection { body: BodyId, name: TopoName, body2: BodyId, name2: TopoName },
}

impl PointRef {
    pub fn body(&self) -> &BodyId {
        match self {
            PointRef::Vertex { body, .. } | PointRef::Center { body, .. } | PointRef::Midpoint { body, .. } | PointRef::Quadrant { body, .. } | PointRef::Intersection { body, .. } => body,
        }
    }
}

/// Binding of one annotation input to model geometry. `slot` meaning per
/// annotation kind: linear dimension 0 = `a`, 1 = `b` (10 / 11: `a` / `b`
/// on a circle, from its center ref: a wall thickness); radial dimension and
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
    /// Auto centermarks on face-on circles (and, unless
    /// `auto_centerlines` says otherwise, centerlines on cylinder axes).
    #[serde(default)]
    pub show_centerlines: bool,
    /// Auto centerlines on cylinders seen from the side (None: as
    /// `show_centerlines`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_centerlines: Option<bool>,
    #[serde(default)]
    pub label: Option<String>,
    /// Section / detail views: View properties › Prefix and Suffix of the
    /// label (live asbv-97; None: Drawing properties' prefix, no suffix;
    /// lines split at "\n").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label_prefix: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label_suffix: Option<String>,
    /// View properties › ☐ View label on a view labelled by its kind
    /// (section, detail, auxiliary): the label isn't drawn.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub hide_label: bool,
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
    /// Section views: "Show cut geometry only" (just the cut faces).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub cut_only: bool,
    /// Section views: "Section depth" Blind (meters behind the cutting
    /// plane; beyond it the model is left out).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub section_depth: Option<f64>,
    /// Section depth "Up to entity": the depth reaches this point (kept
    /// associative; overrides `section_depth`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub section_depth_ref: Option<PointRef>,
    /// Section views of assemblies: "Exclude from cut" (these parts stay whole).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exclude_from_cut: Vec<crate::BodyId>,
    /// A section of a section view: "Cut from section" (cut the parent's
    /// sectioned geometry, else the base geometry).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub cut_from_section: bool,
    /// A sketch view (Onshape: insert a sketch; Part Studio "Create Drawing
    /// of Sketch N…"): base views of this sketch feature of the source show
    /// its curves; views derived from it too.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sketch: Option<crate::FeatureId>,
    /// Part views: source sketches shown in the view (Show/hide sketches),
    /// and whether their points are hidden (Show/Hide sketch points).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub shown_sketches: Vec<crate::FeatureId>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub hide_sketch_points: bool,
    /// Pin view reference: base views shown from a version of the source
    /// (Onshape shows a pin and the version name in the Sheets panel).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pinned: Option<PinnedVersion>,
    /// Show/Hide center of mass.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub show_com: bool,
    /// Flat pattern view of a sheet metal part (a root base view with a
    /// part; inserted from the Insert view chooser's flat pattern filter),
    /// with Show/Hide bend lines and bend notes (both shown by default).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub flat: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub hide_bend_lines: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub hide_bend_notes: bool,
    /// Show/Hide threads (cosmetic threads of tapped holes; shown by default).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub hide_threads: bool,
    /// Show/hide › Show faulty parts (parts whose features failed).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub show_faulty: bool,
    /// Section views: Show/hide › Show offset cut lines (a thin line across
    /// the hatch where the cutting line jogs; live asb 10-23 views (6)).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub show_offset_cut_lines: bool,
    /// Adjust linestyle: edges drawn in their own style.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub edge_styles: Vec<EdgeStyle>,
    /// Automatic centermarks deleted one by one (`build::centermark_key`
    /// of their centers).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub deleted_centermarks: Vec<[i64; 2]>,
    /// Show/hide edges: model edges left out of the view.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hidden_edges: Vec<(BodyId, TopoName)>,
    /// View properties › Show sketch appearances and Show annotations from
    /// model (Insert view sets them; V49, V5).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub sketch_appearances: bool,
    #[serde(default = "yes", skip_serializing_if = "Clone::clone")]
    pub model_annotations: bool,
    /// While the Show/hide edges dialog is open (not saved): hidden edges
    /// draw thick and grey and occluded edges grey, so both can be picked.
    #[serde(skip)]
    pub editing_edges: bool,
    /// Parts left out of this view (Show/hide parts › Hidden parts).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hidden_parts: Vec<BodyId>,
    /// Detail views: the profile as a polygon in the parent view (rectangle,
    /// polygon or sampled spline; None: the circle of the view kind).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail_profile: Option<Vec<[f64; 2]>>,
    /// Detail views: Profile style 0 In line, 1 Connected, 2 Leader.
    #[serde(default, skip_serializing_if = "is_zero_u8")]
    pub detail_style: u8,
    /// Detail views: "Show detail view profile" off.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub detail_hide_profile: bool,
    /// Detail views: "Clip border to geometry".
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub detail_clip_border: bool,
    /// Detail views: Add leader — where the label sits (sheet mm offset from
    /// the profile), with a leader to it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail_label_at: Option<[f64; 2]>,
    /// The view label's move from its place under the view (sheet mm;
    /// dragged on its own, live behaviours "view labels").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label_at: Option<[f64; 2]>,
    /// Auxiliary views: the view reference indicator (letter, where it sits
    /// on the picked edge in the parent, shown or not).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aux_ref: Option<AuxRef>,
    /// Section and auxiliary views: "Flip direction" (look the other way
    /// across the cutting line / reference edge).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub flip: bool,
    /// Broken-out sections (base, projected and auxiliary views).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub broken_out: Vec<BrokenOut>,
    /// Hatch regions of the cut restyled with Edit hatch section... (the
    /// rest keep the drawing's section hatch).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub section_hatches: Vec<SectionHatch>,
    /// Base views: explicit view frame (viewing direction, up) replacing the
    /// standard orientation, e.g. for a projected view moved to another sheet
    /// without its parent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_frame: Option<[[f64; 3]; 2]>,
    /// Base views: the source's named view shown (View orientation › a
    /// named view; `custom_frame` holds its camera). Listed grayed once the
    /// named view is deleted from the source.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub named_view: Option<String>,
    /// Assembly views › Explode/Position › a named position (index into the
    /// assembly's named positions; exclusive with `explode`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub named_position: Option<usize>,
    /// Assembly views › Display state (by name; None: Show all).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_state: Option<String>,
    /// Child views: their own Explode/Position (None: Follow parent).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub own_position: Option<AsmPosition>,
    /// Child views: their own Display state (None: Follow parent).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub own_display: Option<DisplayChoice>,
    /// View properties › View render mode › Best quality (exact hidden-line
    /// removal on the sheet too; default Best performance).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub best_quality: bool,
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
    /// Scale › Sheet scale (child views, help H:views): with no scale of its
    /// own the view follows the sheet's scale, not its parent's.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub sheet_scale: bool,
    /// View simplification (None: the parent's, Auto for base views).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub simplification: Option<Simplification>,
}

/// Where a datum's triangle sits on another annotation: segment `seg` of
/// the `line`-th polyline the annotation draws, at `t` along it (an
/// extension line can be passed: t > 1).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct DatumOn {
    pub ann: u64,
    pub line: u32,
    pub seg: u32,
    pub t: f64,
}

/// An assembly view's Explode/Position choice.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AsmPosition {
    /// The assembly as solved ("Default").
    Default,
    /// An exploded view (index into the assembly's exploded views).
    Explode(usize),
    /// A named position (index into the assembly's named positions).
    Position(usize),
}

/// An assembly view's Display state choice.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DisplayChoice {
    ShowAll,
    State(String),
}

impl DrawingView {
    /// Labelled by its kind (section, aligned section, detail; auxiliary
    /// with a reference letter) rather than by `label`.
    pub fn auto_label(&self) -> bool {
        matches!(self.kind, ViewKind::Section { .. } | ViewKind::AlignedSection { .. } | ViewKind::Detail { .. })
            || (matches!(self.kind, ViewKind::Auxiliary { .. }) && self.aux_ref.as_ref().is_some_and(|a| !a.letter.is_empty()))
    }

    /// View properties › View label: the label is drawn.
    pub fn view_label_shown(&self) -> bool {
        if self.auto_label() { !self.hide_label } else { self.label.as_ref().is_some_and(|l| !l.is_empty()) }
    }

    /// View properties › Scale label: the scale is drawn under the label
    /// (detail views by default; others with their own label and scale).
    pub fn scale_label_shown(&self) -> bool {
        self.scale_label.unwrap_or(if matches!(self.kind, ViewKind::Detail { .. }) { true } else { self.label.is_some() && self.scale.is_some() })
    }

    /// The base view's Explode/Position.
    pub fn asm_position(&self) -> AsmPosition {
        match (self.explode, self.named_position) {
            (Some(e), _) => AsmPosition::Explode(e),
            (None, Some(p)) => AsmPosition::Position(p),
            _ => AsmPosition::Default,
        }
    }

    /// Set the base view's Explode/Position.
    pub fn set_asm_position(&mut self, p: AsmPosition) {
        (self.explode, self.named_position) = match p {
            AsmPosition::Default => (None, None),
            AsmPosition::Explode(e) => (Some(e), None),
            AsmPosition::Position(n) => (None, Some(n)),
        };
    }
}

/// An auxiliary view's reference indicator: the arrow at `anchor` (parent
/// view coordinates, on the picked edge) labelled `letter`; the view is
/// labelled "Auxiliary <letter> - <letter>".
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AuxRef {
    pub anchor: [f64; 2],
    pub letter: String,
    #[serde(default = "yes")]
    pub show: bool,
    /// Distance (sheet mm) of the viewing-plane line from the edge toward
    /// the view; dragged with the indicator's middle snap point.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<f64>,
    /// How far (sheet mm) the line runs to either side of its middle
    /// (dragged with the end snap points, symmetrically; None: the edge's
    /// extent plus 0.105 in each side).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ext: Option<[f64; 2]>,
    /// Each letter moved by its snap point (sheet mm from its place).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub letter_at: Option<[[f64; 2]; 2]>,
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
            sheet_scale: false,
            style: RenderStyle::HiddenLinesRemoved,
            show_tangent_edges: true,
            show_centerlines: false,
            auto_centerlines: None,
            label: None,
            label_prefix: None,
            label_suffix: None,
            hide_label: false,
            rotation: 0.0,
            crop: None,
            breaks: Vec::new(),
            explode: None,
            partial: false,
            cut_only: false,
            section_depth: None,
            flip: false,
            aux_ref: None,
            sketch: None,
            shown_sketches: Vec::new(),
            hide_sketch_points: false,
            hidden_parts: Vec::new(),
            show_com: false,
            flat: false,
            hide_bend_lines: false,
            hide_bend_notes: false,
            hide_threads: false,
            show_faulty: false,
            show_offset_cut_lines: false,
            edge_styles: Vec::new(),
            hidden_edges: Vec::new(),
            sketch_appearances: false,
            model_annotations: true,
            deleted_centermarks: Vec::new(),
            editing_edges: false,
            pinned: None,
            detail_profile: None,
            detail_style: 0,
            detail_hide_profile: false,
            detail_clip_border: false,
            detail_label_at: None,
            label_at: None,
            section_depth_ref: None,
            exclude_from_cut: Vec::new(),
            cut_from_section: false,
            broken_out: Vec::new(),
            section_hatches: Vec::new(),
            custom_frame: None,
            named_view: None,
            named_position: None,
            display_state: None,
            own_position: None,
            own_display: None,
            best_quality: false,
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
    Note {
        text: String,
        height_mm: f64,
        /// Note editor formatting and leaders.
        #[serde(default, skip_serializing_if = "note::NoteStyle::is_default")]
        style: note::NoteStyle,
    },
    LinearDimension { view: ViewId, a: [f64; 2], b: [f64; 2], offset: f64, orientation: DimOrientation, override_text: Option<String> },
    /// `dim_line`: a diameter's line runs on through the center to the far
    /// side (Drawing properties › Show dimension lines by default when it
    /// was made); else a leader to the arrow on the near side.
    RadialDimension {
        view: ViewId,
        center: [f64; 2],
        radius: f64,
        diameter: bool,
        angle: f64,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        dim_line: bool,
        /// The leader's knee outside the circle (sheet mm), where it was
        /// dropped; None: 12.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        knee: Option<f64>,
    },
    /// Angle from leg `a` counterclockwise to leg `b`. `three_point`: made
    /// with the 3 point angular tool (dragging the arc across a leg swaps the
    /// legs: 360° minus the angle).
    AngularDimension {
        view: ViewId,
        vertex: [f64; 2],
        a: [f64; 2],
        b: [f64; 2],
        radius: f64,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        three_point: bool,
        /// Line to line angular: leg `a` / `b` runs along its edge's
        /// extension through the vertex (the supplementary or vertical
        /// angle, picked by where the dimension is dragged; help
        /// drawing_-_line_to_line_angular_dimension). The references stay
        /// on the edge.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        flip_a: bool,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        flip_b: bool,
    },
    /// `angle`: the mark's rotation in the view (radians; Align centermark,
    /// pattern centermarks).
    Centermark {
        view: ViewId,
        center: [f64; 2],
        size: f64,
        #[serde(default, skip_serializing_if = "is_zero")]
        angle: f64,
        /// Extra length of each arm (+x, +y, −x, −y of the mark; view
        /// units): dragging an end grip lengthens all four, Shift+drag
        /// only that arm (live drawing/483–493).
        #[serde(default, skip_serializing_if = "no_arms")]
        arms: [f64; 4],
    },
    Centerline { view: ViewId, a: [f64; 2], b: [f64; 2] },
    /// Circular centerline (bolt circle): through three points, or (two
    /// points) center then a point on the circle. View coordinates.
    CircleCenterline { view: ViewId, points: Vec<[f64; 2]> },
    /// Hole / thread callout on a hole circle (`center`, `radius` in view
    /// coordinates; slot 0 = the circle's center reference). The text comes
    /// from the Hole feature; the leader runs from the circle to `position`.
    HoleCallout { view: ViewId, center: [f64; 2], radius: f64 },
    /// Virtual sharp: where two straight edges (`a1`-`b1`, `a2`-`b2`) meet.
    VirtualSharp { view: ViewId, a1: [f64; 2], b1: [f64; 2], a2: [f64; 2], b2: [f64; 2] },
    /// Callout: `text` empty = the BOM / cut list item of the part at the
    /// anchor (refs slot 0); `callout` holds the dialog's other settings and
    /// the stacked callouts (refs slots 1..).
    Balloon {
        view: ViewId,
        anchor: [f64; 2],
        text: String,
        #[serde(default, skip_serializing_if = "CalloutStyle::is_default")]
        callout: CalloutStyle,
    },
    SurfaceFinish {
        anchor: [f64; 2],
        value: String,
        /// Leaders added with Add leader.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        leaders: Vec<Leader>,
    },
    /// Weld symbol: the arrow points at `target` (view coordinates in
    /// `view`, else sheet), the reference line starts at the annotation's
    /// position.
    Weld {
        #[serde(default)]
        view: Option<ViewId>,
        target: [f64; 2],
        spec: Box<weld::WeldSpec>,
    },
    /// Revision callout: a triangle with the revision label (Add revision
    /// callout; shown while the drawing has a revision table). Grouped with
    /// a dimension (`group`, dropped onto it), `anchor` is the offset from
    /// the upper right of that dimension's text and the callout follows it.
    RevisionCallout {
        anchor: [f64; 2],
        revision: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        group: Option<u64>,
    },
    /// Feature control frame with its lower-left corner at `anchor` (sheet
    /// mm) and optional leaders to the toleranced geometry.
    Gdt {
        anchor: [f64; 2],
        /// The first frame row: symbol, tolerance ("Ø0.1Ⓜ"), datums.
        frame: Vec<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        leaders: Vec<Leader>,
        /// Upper / lower text, prefixes and suffixes, more frame rows.
        #[serde(default, skip_serializing_if = "GdtExtra::is_default")]
        extra: Box<GdtExtra>,
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
    /// leader ending in a filled triangle at `anchor` (sheet mm). `on`: the
    /// triangle sits on another annotation (a GD&T frame edge, a dimension
    /// line, leader or extension line) and follows it; `anchor` is then
    /// where it was when last edited and the box moves with the difference.
    Datum {
        anchor: [f64; 2],
        label: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        on: Option<DatumOn>,
        /// On a view's geometry: that point (view coordinates). The
        /// triangle follows the view when it moves or rescales and the box
        /// moves with it (qa DATUM-SCALE-DETACHED); `anchor` is where it was
        /// when placed.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        at: Option<(ViewId, [f64; 2])>,
    },
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
    /// Slots: 0 = `a`, 1 = `b`; to a straight edge (live: any non-curve
    /// geometry), 2 and 3 = its ends and `b` is the perpendicular's foot.
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
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Leader {
    #[serde(default)]
    pub view: Option<ViewId>,
    pub point: [f64; 2],
    /// Bend nodes between the shoulder and the point (sheet mm).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub nodes: Vec<[f64; 2]>,
    /// Notes: the horizontal landing from the note's side to the bend (mm;
    /// None: 4). Live drw2 10-21 g: dragging the bend changes it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub landing: Option<f64>,
    /// Notes: the side the leader leaves (None: the nearer one).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub right: Option<bool>,
    /// Notes: where the leader starts (live: Add leader branches from the
    /// existing landing's bend, or from a node of a leader).
    #[serde(default, skip_serializing_if = "LeaderStart::is_side")]
    pub start: LeaderStart,
}

/// Where a note leader starts: the note's side, or a branch off the bend or
/// a node of an earlier leader of the same note (its index).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LeaderStart {
    #[default]
    Side,
    Bend(usize),
    Node(usize, usize),
}

impl LeaderStart {
    pub fn is_side(&self) -> bool {
        *self == LeaderStart::Side
    }
}

/// Remove node `node` of leader `leader`; branches from it start at the
/// leader's bend instead, later nodes' branches move down one.
pub fn remove_leader_node(leaders: &mut [Leader], leader: usize, node: usize) {
    let Some(l) = leaders.get_mut(leader) else { return };
    if node >= l.nodes.len() {
        return;
    }
    l.nodes.remove(node);
    for b in leaders.iter_mut() {
        b.start = match b.start {
            LeaderStart::Node(j, n) if j == leader && n == node => LeaderStart::Bend(j),
            LeaderStart::Node(j, n) if j == leader && n > node => LeaderStart::Node(j, n - 1),
            s => s,
        };
    }
}

/// Remove leader `leader`; branches off it start at the note's side, later
/// indices move down one.
pub fn remove_leader(leaders: &mut Vec<Leader>, leader: usize) {
    if leader >= leaders.len() {
        return;
    }
    leaders.remove(leader);
    for b in leaders.iter_mut() {
        let shift = |j: usize| if j > leader { j - 1 } else { j };
        b.start = match b.start {
            LeaderStart::Bend(j) | LeaderStart::Node(j, _) if j == leader => LeaderStart::Side,
            LeaderStart::Bend(j) => LeaderStart::Bend(shift(j)),
            LeaderStart::Node(j, n) => LeaderStart::Node(shift(j), n),
            s => s,
        };
    }
}

impl CalloutStyle {
    /// The callout's leaders as one list: 0 = the leader to `anchor` in
    /// `view` (with [`CalloutStyle::nodes`]), then the added ones.
    pub fn all_leaders(&self, view: ViewId, anchor: [f64; 2]) -> Vec<Leader> {
        let main = Leader { view: Some(view), point: anchor, nodes: self.nodes.clone(), ..Default::default() };
        std::iter::once(main).chain(self.leaders.iter().cloned()).collect()
    }

    /// Store `all` (as [`CalloutStyle::all_leaders`]) back: entry 0's nodes,
    /// the rest as the added leaders.
    pub fn set_all_leaders(&mut self, mut all: Vec<Leader>) {
        if all.is_empty() {
            return;
        }
        self.nodes = std::mem::take(&mut all[0].nodes);
        self.leaders = all.split_off(1);
    }

    /// Delete leader `leader` of [`CalloutStyle::all_leaders`]: 0 leaves the
    /// callout in space (its branches start at the border).
    pub fn remove_leader(&mut self, leader: usize, view: ViewId, anchor: [f64; 2]) {
        let mut all = self.all_leaders(view, anchor);
        if leader == 0 {
            self.free = true;
            all[0].nodes.clear();
            for b in all.iter_mut() {
                if matches!(b.start, LeaderStart::Bend(0) | LeaderStart::Node(0, _)) {
                    b.start = LeaderStart::Side;
                }
            }
        } else {
            remove_leader(&mut all, leader);
            // Indices past the removed one moved down; 0 stays the main one.
        }
        self.set_all_leaders(all);
    }
}

/// Target area of a datum target (sizes in model meters).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum DatumTargetArea {
    /// Point target drawn as an X.
    #[default]
    Point,
    /// "Point (dot)": a filled dot.
    Dot,
    Circle { diameter: f64 },
    Rectangle { width: f64, height: f64 },
    /// A custom shape (no origin; live drawing/509).
    Custom,
}

/// Units of a dimension value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DimUnits {
    Millimeter,
    Centimeter,
    Meter,
    Inch,
    Foot,
    Yard,
    /// Inches as fractions (to 1/64).
    InchFraction,
    /// Feet and inches: 1' 2.5".
    FeetInches,
}

impl DimUnits {
    /// Drawing properties › Units.
    pub const ALL: [DimUnits; 5] = [DimUnits::Millimeter, DimUnits::Centimeter, DimUnits::Meter, DimUnits::Inch, DimUnits::Foot];
    /// A hole callout's units list (live drawing/461, "Inches (Drawing)" is
    /// the drawing's own and listed apart).
    pub const CALLOUT: [DimUnits; 8] = [DimUnits::Millimeter, DimUnits::Centimeter, DimUnits::Meter, DimUnits::Inch, DimUnits::InchFraction, DimUnits::FeetInches, DimUnits::Foot, DimUnits::Yard];
    pub fn imperial(self) -> bool {
        matches!(self, DimUnits::Inch | DimUnits::Foot | DimUnits::Yard | DimUnits::InchFraction | DimUnits::FeetInches)
    }
    /// Meters per unit.
    pub fn meters(self) -> f64 {
        match self {
            DimUnits::Millimeter => 0.001,
            DimUnits::Centimeter => 0.01,
            DimUnits::Meter => 1.0,
            DimUnits::Inch | DimUnits::InchFraction => 0.0254,
            DimUnits::Foot | DimUnits::FeetInches => 0.3048,
            DimUnits::Yard => 0.9144,
        }
    }
    pub fn symbol(self) -> &'static str {
        match self {
            DimUnits::Millimeter => "mm",
            DimUnits::Centimeter => "cm",
            DimUnits::Meter => "m",
            DimUnits::Inch | DimUnits::InchFraction => "in",
            DimUnits::Foot => "ft",
            DimUnits::Yard => "yd",
            DimUnits::FeetInches => "",
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            DimUnits::Millimeter => "Millimeters",
            DimUnits::Centimeter => "Centimeters",
            DimUnits::Meter => "Meters",
            DimUnits::Inch => "Inches",
            DimUnits::Foot => "Feet",
            DimUnits::Yard => "Yards",
            DimUnits::InchFraction => "Inches fractional",
            DimUnits::FeetInches => "Feet and inches",
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
    /// Palette "Above text" / "Below text": lines over and under the value.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub above: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub below: String,
    /// Palette "Parentheses" (a reference value).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub reference: bool,
    /// Palette "Inspection dimension": an oval frame around the value.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub inspection_frame: bool,
    /// Palette "Dual dimension": Some(true) dual, Some(false) single;
    /// None the drawing's (Units and precision › Dual).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dual: Option<bool>,
    /// Palette "Count type" (None: no count).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub count: Option<HoleCount>,
    /// The palette's dual row (live asbdd-9: "[ 0.39 ▾ (in) ▾ 👁 ]").
    #[serde(default, skip_serializing_if = "DualFormat::is_default")]
    pub dual_fmt: DualFormat,
    /// A typed value overriding the measured one, for dimensions other than
    /// linear (those keep `LinearDimension::override_text`; D39).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    /// The Fit tolerance types' options.
    #[serde(default, skip_serializing_if = "FitOpts::is_default")]
    pub fit: FitOpts,
}

impl DimFormat {
    pub fn is_default(&self) -> bool {
        *self == DimFormat::default()
    }
}

/// One dimension's dual precision, units and Show/hide units (the
/// palette's dual row); None each: Drawing properties › Dual.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DualFormat {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub precision: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub units: Option<DimUnits>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub show_unit: Option<bool>,
}

impl DualFormat {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// The dimension palette's Fit tolerance options (live drawing/466–467):
/// which Fit entry (Fit, Fit with tolerance, Fit (Tolerance only)), the fit
/// type, standard, how the classes stack and the fit tolerance precision.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FitOpts {
    pub mode: FitMode,
    pub fit_type: FitType,
    /// Some(true) ISO, Some(false) ANSI; None the drawing's Fit standard.
    pub iso: Option<bool>,
    pub stacking: FitStacking,
    /// None: Units and precision › Fit class tolerance precision.
    pub precision: Option<u8>,
    /// "Toggle hole class visibility" / shaft class visibility.
    pub hide_hole: bool,
    pub hide_shaft: bool,
}

impl FitOpts {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum FitMode {
    #[default]
    Fit,
    WithTolerance,
    ToleranceOnly,
}

impl FitMode {
    pub fn name(self) -> &'static str {
        match self {
            FitMode::Fit => "Fit",
            FitMode::WithTolerance => "Fit with tolerance",
            FitMode::ToleranceOnly => "Fit (Tolerance only)",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum FitType {
    #[default]
    UserDefined,
    Clearance,
    Transition,
    Interference,
}

impl FitType {
    pub const ALL: [FitType; 4] = [FitType::UserDefined, FitType::Clearance, FitType::Transition, FitType::Interference];
    pub fn name(self) -> &'static str {
        match self {
            FitType::UserDefined => "User defined",
            FitType::Clearance => "Clearance",
            FitType::Transition => "Transition",
            FitType::Interference => "Interference",
        }
    }
}

/// How hole and shaft classes stack (D36); inline by default.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum FitStacking {
    WithoutLine,
    /// Inline "H7/g6" (the default).
    #[default]
    Diagonal,
    WithLine,
}

impl FitStacking {
    pub const ALL: [FitStacking; 3] = [FitStacking::WithoutLine, FitStacking::Diagonal, FitStacking::WithLine];
    pub fn name(self) -> &'static str {
        match self {
            FitStacking::WithoutLine => "Stacked without line",
            FitStacking::Diagonal => "Stacked diagonally",
            FitStacking::WithLine => "Stacked with line",
        }
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
    /// Bill of materials of the assembly shown by `view`: Flattened or
    /// Structured; Order "Bottom to top" puts the header at the bottom and
    /// reverses the rows.
    Bom {
        view: ViewId,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        structured: bool,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        bottom_to_top: bool,
        /// Overridden cells: (item number, column, value).
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        overrides: Vec<(String, usize, String)>,
    },
    /// Holes (face-on circular bores) of `view`, located from `origin`
    /// (view coordinates, associative through `origin_ref`).
    Hole {
        view: ViewId,
        origin: [f64; 2],
        #[serde(default)]
        origin_ref: Option<PointRef>,
        #[serde(default)]
        precision: Option<u8>,
        /// Tag axis › Flip X direction / Flip Y direction.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        flip_x: bool,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        flip_y: bool,
        /// Hole table › ☑ Show location data (the X LOC / Y LOC columns).
        #[serde(default = "yes_hole", skip_serializing_if = "is_true")]
        show_location: bool,
        /// Order: Bottom to top.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        bottom_to_top: bool,
    },
    /// Cut list of the frame members of the Part Studio shown by `view`
    /// (the Cut list feature's rows); Order "Bottom to top" reverses it.
    CutList {
        view: ViewId,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        bottom_to_top: bool,
    },
    /// The drawing's revisions from the release log (rows resolved when the
    /// sheet is built, from [`build::SheetInfo::revisions`]).
    Revision {
        /// Multi-sheet "See sheet 1": a small pointer table on the other
        /// sheets; else "Linked", the full table on every sheet.
        #[serde(default)]
        see_sheet1: bool,
        #[serde(default = "default_visible_rows")]
        visible_rows: u32,
        /// Order "Bottom to top" (newest at the top); else "Top to bottom".
        #[serde(default)]
        newest_top: bool,
        /// Hide revision callouts no longer shown in the table.
        #[serde(default)]
        hide_callouts: bool,
        /// Edited title and column headers (None = the default).
        #[serde(default)]
        title: Option<String>,
        #[serde(default)]
        headers: Vec<Option<String>>,
        /// Edited descriptions (revision, text).
        #[serde(default)]
        descriptions: Vec<(String, String)>,
        /// Revisions whose callouts are hidden (row › Hide revision callouts).
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        hidden: Vec<String>,
    },
    /// A FeatureScript custom table (`defineTable`) run on a whole Part
    /// Studio, which need not be shown in the drawing (Onshape's Custom
    /// table). Rows resolved when the sheet is built, from
    /// [`build::SheetInfo::custom_tables`].
    Custom {
        /// The Part Studio tabulated, and its pinned version (None: the workspace).
        studio: TabId,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        version: Option<String>,
        /// The Feature Studio defining the table, the table's exported
        /// constant, and the version it was added from (None: the
        /// workspace's own table).
        fs: TabId,
        name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fs_version: Option<String>,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        bottom_to_top: bool,
    },
}

fn default_visible_rows() -> u32 {
    5
}

impl TableKind {
    pub fn revision() -> Self {
        TableKind::Revision { see_sheet1: false, visible_rows: default_visible_rows(), newest_top: false, hide_callouts: false, title: None, headers: Vec::new(), descriptions: Vec::new(), hidden: Vec::new() }
    }
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
    /// `text_height` was set on the table (Text height ▸); else its rows
    /// follow Drawing properties › Tables.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub own_text_height: bool,
    /// Per-cell formatting (cells without an entry use the defaults).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cell_formats: Vec<CellStyle>,
    /// Merged cell ranges.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub merges: Vec<CellMerge>,
    /// Split BOM / cut list tables: the data rows this part shows
    /// [start, end) (None: all).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rows: Option<[usize; 2]>,
    /// Row heights set by dragging dividers (displayed rows below the
    /// title; empty or wrong length = automatic).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub row_heights: Vec<f64>,
    /// Move to: a format layer (its color, locked with the format).
    #[serde(default, skip_serializing_if = "SheetLayer::is_drawing")]
    pub layer: SheetLayer,
}

/// Horizontal alignment of a table cell's text.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CellAlign {
    Left,
    #[default]
    Center,
    Right,
}

/// Vertical alignment of a table cell's text.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CellVAlign {
    Top,
    #[default]
    Middle,
    Bottom,
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
    #[serde(default)]
    pub italic: bool,
    #[serde(default)]
    pub underline: bool,
    #[serde(default)]
    pub strikethrough: bool,
    #[serde(default)]
    pub valign: CellVAlign,
    /// Font family; None = the drawing's (Noto Sans).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font: Option<String>,
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
        Self { id, kind, position, corner: TableCorner::TopRight, col_widths: Vec::new(), row_height: default_row_height(), text_height: default_table_text(), own_text_height: false, cell_formats: Vec::new(), merges: Vec::new(), rows: None, row_heights: Vec::new(), layer: SheetLayer::Drawing }
    }

    /// Formatting of a cell (defaults when none is set).
    pub fn cell_format(&self, row: usize, col: usize) -> CellFormat {
        self.cell_formats.iter().find(|s| s.row == row && s.col == col).map(|s| s.format.clone()).unwrap_or_default()
    }

    /// The format a general table cell is drawn with: its own, or the
    /// default (live: header cells bold and centred, body cells left).
    pub fn shown_cell_format(&self, row: usize, col: usize) -> CellFormat {
        match self.cell_formats.iter().find(|s| s.row == row && s.col == col) {
            Some(s) => s.format.clone(),
            None => self.default_cell_format(row),
        }
    }

    /// A cell's format when it has none of its own.
    fn default_cell_format(&self, row: usize) -> CellFormat {
        match &self.kind {
            TableKind::General { header: true, .. } if row == 0 => CellFormat { bold: true, ..Default::default() },
            TableKind::General { .. } => CellFormat { align: CellAlign::Left, ..Default::default() },
            _ => CellFormat::default(),
        }
    }

    pub fn set_cell_format(&mut self, row: usize, col: usize, format: CellFormat) {
        self.cell_formats.retain(|s| !(s.row == row && s.col == col));
        if format != self.default_cell_format(row) {
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

    /// Duplicate rows `r0..=r1` below them (general tables: texts and
    /// formats).
    pub fn duplicate_rows(&mut self, r0: usize, r1: usize) {
        for k in 0..=(r1 - r0) {
            let (src, dst) = (r0 + k, r1 + 1 + k);
            self.insert_row(dst);
            if let Some(cells) = self.general_cells()
                && let Some(row) = cells.get(src).cloned()
                && let Some(d) = cells.get_mut(dst)
            {
                *d = row;
            }
            let copies: Vec<CellStyle> = self.cell_formats.iter().filter(|s| s.row == src).map(|s| CellStyle { row: dst, ..s.clone() }).collect();
            self.cell_formats.extend(copies);
        }
        self.cell_formats.sort_by_key(|s| (s.row, s.col));
    }

    /// Duplicate columns `c0..=c1` right of them.
    pub fn duplicate_cols(&mut self, c0: usize, c1: usize) {
        for k in 0..=(c1 - c0) {
            let (src, dst) = (c0 + k, c1 + 1 + k);
            self.insert_col(dst);
            if let Some(cells) = self.general_cells() {
                for row in cells.iter_mut() {
                    if let Some(v) = row.get(src).cloned()
                        && let Some(d) = row.get_mut(dst)
                    {
                        *d = v;
                    }
                }
            }
            let copies: Vec<CellStyle> = self.cell_formats.iter().filter(|s| s.col == src).map(|s| CellStyle { col: dst, ..s.clone() }).collect();
            self.cell_formats.extend(copies);
        }
        self.cell_formats.sort_by_key(|s| (s.row, s.col));
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
            TableKind::Revision { .. } => "Revision table",
            TableKind::CutList { .. } => "Cut list table",
            TableKind::Custom { .. } => "Custom table",
        }
    }
    pub fn view(&self) -> Option<ViewId> {
        match self.kind {
            TableKind::Bom { view, .. } | TableKind::Hole { view, .. } | TableKind::CutList { view, .. } => Some(view),
            TableKind::General { .. } | TableKind::Revision { .. } | TableKind::Custom { .. } => None,
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
    /// Inspection item number (Onshape's numbered bubble on the annotation).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inspection: Option<u32>,
    /// Arrows flipped to outside the extension lines (the selected
    /// dimension's flip node).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub flip_arrows: bool,
    /// Linear dimensions: jogged extension lines (right-click an extension
    /// line › Add jog), one per extension line.
    #[serde(default, skip_serializing_if = "no_jogs")]
    pub jogs: [Option<ExtJog>; 2],
    /// Unbreak dimension: crossing dimension lines don't cut this one's
    /// lines (new dimensions follow Drawing properties › Break dimension
    /// by default).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub unbroken: bool,
    /// Grouped with another annotation (a callout dropped onto a
    /// dimension): `position` is then the offset from the upper right of
    /// that annotation's text, so it moves with it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<u64>,
    /// Move to: the format layer it is on (its color applies, it locks with
    /// the format and exports on that DXF / DWG layer).
    #[serde(default, skip_serializing_if = "SheetLayer::is_drawing")]
    pub layer: SheetLayer,
    /// Group (a note or callout with a view): it moves with the view.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view_group: Option<ViewId>,
    /// Group annotation (notes and callouts): members move together.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ann_group: Option<u64>,
    /// Styles panel: this annotation's own style (None fields: the drawing
    /// properties' value, shown "(Drawing)").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<Box<AnnStyle>>,
    /// Hole callouts: the panel's options (None: the defaults).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hole: Option<Box<HoleCalloutOpts>>,
    /// Datum targets: style, origin and its dimensions (None: the defaults).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<Box<DatumTargetOpts>>,
    /// A dimension on drawing sketch geometry: the entity and the sketch
    /// constraint holding its value ([`sketch_dims`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sketch_dim: Option<SketchDim>,
    /// An MBD annotation of a Part Studio shown in a view ([`mbd`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<mbd::ModelRef>,
}

/// See [`Annotation::sketch_dim`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SketchDim {
    pub entity: u32,
    pub constraint: u32,
}

/// The sheet itself as a view: dimensions on drawing sketch geometry use
/// it (view metres = sheet millimetres / 1000, no rotation).
pub const SHEET_VIEW: ViewId = ViewId(u64::MAX);

/// A datum target symbol's style (live drawing/510).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TargetStyle {
    #[default]
    Inside,
    MoveableHorizontal,
    MoveableRotational,
}

impl TargetStyle {
    pub const ALL: [TargetStyle; 3] = [TargetStyle::Inside, TargetStyle::MoveableHorizontal, TargetStyle::MoveableRotational];
    pub fn name(self) -> &'static str {
        match self {
            TargetStyle::Inside => "Inside",
            TargetStyle::MoveableHorizontal => "Moveable horizontal inside",
            TargetStyle::MoveableRotational => "Moveable rotational inside",
        }
    }
}

/// Datum target options (live drawing/507–518, help datum): the symbol
/// style, the origin (view coordinates; the target's X / Y position
/// dimensions measure from it), Length or Angle positioning, the flipped
/// directions and "Hide dimension".
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DatumTargetOpts {
    pub style: TargetStyle,
    pub origin: Option<[f64; 2]>,
    pub origin_ref: Option<PointRef>,
    /// Angle positioning (origin at an arc center) instead of Length.
    pub angular: bool,
    pub flip_x: bool,
    pub flip_y: bool,
    /// Live (behaviours "A35 datum target"): a new target draws no
    /// dimensions until Show dimensions.
    pub hide_dims: bool,
}

impl Default for DatumTargetOpts {
    fn default() -> Self {
        Self { style: TargetStyle::default(), origin: None, origin_ref: None, angular: false, flip_x: false, flip_y: false, hide_dims: true }
    }
}

/// Where dual values sit relative to the primary ones.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum DualLocation {
    #[default]
    Top,
    Bottom,
    Left,
    Right,
}

impl DualLocation {
    pub const ALL: [DualLocation; 4] = [DualLocation::Top, DualLocation::Bottom, DualLocation::Left, DualLocation::Right];
    pub fn name(self) -> &'static str {
        match self {
            DualLocation::Top => "Top",
            DualLocation::Bottom => "Bottom",
            DualLocation::Left => "Left",
            DualLocation::Right => "Right",
        }
    }
}

/// Drawing properties › Units and precision › Dual (live drawing/18):
/// show dual dimensions / the dual unit, where the dual values go on
/// dimensions and hole callouts, their units and precisions.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DualDims {
    pub show: bool,
    pub show_unit: bool,
    pub dim_location: DualLocation,
    pub hole_location: DualLocation,
    /// None: millimeters on inch drawings, inches on metric ones.
    pub units: Option<DimUnits>,
    pub precision: Option<u8>,
    pub tolerance_precision: Option<u8>,
}

impl DualDims {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
    /// The dual units of a drawing whose primary units are metric or not.
    pub fn units_for(&self, metric: bool) -> DimUnits {
        self.units.unwrap_or(if metric { DimUnits::Inch } else { DimUnits::Millimeter })
    }
}

/// How a hole callout counts its holes (live drawing/461).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum HoleCount {
    None,
    /// Same-size holes in the view.
    View,
    /// Same-size holes made by the same hole feature on one face.
    Face,
    /// The hole's feature or pattern instances.
    #[default]
    Pattern,
    /// Same-size holes in the part.
    Part,
    /// One face, pattern instances not counted.
    FaceExcludingPatterns,
}

impl HoleCount {
    pub const ALL: [HoleCount; 6] = [HoleCount::None, HoleCount::View, HoleCount::Face, HoleCount::Pattern, HoleCount::Part, HoleCount::FaceExcludingPatterns];
    pub fn name(self) -> &'static str {
        match self {
            HoleCount::None => "None",
            HoleCount::View => "Count by view",
            HoleCount::Face => "Count by face",
            HoleCount::Pattern => "Count by pattern",
            HoleCount::Part => "Count by part",
            HoleCount::FaceExcludingPatterns => "Count by face (Excluding patterns)",
        }
    }
}

/// Hole callout panel options (live drawing/460–467) beyond the shared
/// palette fields of `Annotation::format` (above / below text, prefix,
/// suffix, units, parentheses, dual): the count, and whether the callout's
/// own precision / tolerance override the hole feature's.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct HoleCalloutOpts {
    pub count: HoleCount,
    pub override_tol: bool,
}

/// An annotation's own style from the Styles panel (live drawing/474–477):
/// font, text height, color, arrowhead size and type, and for dimensions
/// text justification, text alignment and text gap.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AnnStyle {
    pub font: Option<sheet::TextFont>,
    pub height: Option<f64>,
    pub color: Option<[u8; 3]>,
    pub arrow: Option<f64>,
    pub arrow_type: Option<sheet::ArrowType>,
    pub justify: Option<TextJustify>,
    /// Text alignment: Some(true) horizontal, Some(false) with the line.
    pub horizontal: Option<bool>,
    pub text_gap: Option<f64>,
}

impl AnnStyle {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    /// Apply to the sheet style for one annotation.
    pub fn apply(&self, s: &mut sheet::SheetStyle, dimension: bool) {
        if dimension {
            if let Some(h) = self.height {
                s.dim_text = h;
            }
            if let Some(a) = self.arrow {
                s.dim_arrow = a;
            }
            if let Some(t) = self.arrow_type {
                s.dim_arrow_type = t;
            }
            if let Some(h) = self.horizontal {
                s.text_horizontal = h;
            }
            if let Some(g) = self.text_gap {
                s.text_gap = g;
            }
        } else {
            if let Some(h) = self.height {
                s.ann_text = h;
            }
            if let Some(a) = self.arrow {
                s.ann_arrow = a;
            }
            if let Some(t) = self.arrow_type {
                s.ann_arrow_type = t;
            }
            if let Some(j) = self.justify {
                s.callout_justify = j;
            }
        }
    }
}

impl Default for Annotation {
    fn default() -> Self {
        Self { id: 0, kind: AnnotationKind::note(String::new(), 3.5), position: [0.0, 0.0], refs: Vec::new(), format: DimFormat::default(), inspection: None, flip_arrows: false, jogs: [None; 2], unbroken: false, group: None, layer: SheetLayer::Drawing, view_group: None, ann_group: None, style: None, hole: None, target: None, sketch_dim: None, model: None }
    }
}

/// Sheet layers (Move to): the drawing, or the format's border frame,
/// border zones and title block.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SheetLayer {
    #[default]
    Drawing,
    BorderFrame,
    BorderZones,
    TitleBlock,
}

impl SheetLayer {
    /// The format layers, in the Move to menu's order.
    pub const FORMAT: [SheetLayer; 3] = [SheetLayer::BorderFrame, SheetLayer::BorderZones, SheetLayer::TitleBlock];

    pub fn is_drawing(&self) -> bool {
        *self == SheetLayer::Drawing
    }

    pub fn name(self) -> &'static str {
        match self {
            SheetLayer::Drawing => "Drawing",
            SheetLayer::BorderFrame => "Border frame",
            SheetLayer::BorderZones => "Border zones",
            SheetLayer::TitleBlock => "Title block",
        }
    }

    /// DXF / DWG layer name (None: by line style).
    pub fn cad_layer(self) -> Option<&'static str> {
        match self {
            SheetLayer::Drawing => None,
            SheetLayer::BorderFrame => Some("BORDER_FRAME"),
            SheetLayer::BorderZones => Some("BORDER_ZONES"),
            SheetLayer::TitleBlock => Some("TITLE_BLOCK"),
        }
    }

    /// Its Formats pen (mm; Drawing properties › Formats › Thickness).
    pub fn thickness(self, p: &DrawingProps) -> Option<f64> {
        match self {
            SheetLayer::Drawing => None,
            SheetLayer::BorderFrame => Some(p.border_thickness),
            SheetLayer::BorderZones => Some(p.zone_thickness),
            SheetLayer::TitleBlock => Some(p.title_block_thickness),
        }
    }

    /// Its Formats color (Drawing properties › Formats).
    pub fn color(self, p: &DrawingProps) -> Option<[u8; 3]> {
        match self {
            SheetLayer::Drawing => None,
            SheetLayer::BorderFrame => p.border_color,
            SheetLayer::BorderZones => p.zone_color,
            SheetLayer::TitleBlock => p.title_block_color,
        }
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
    /// A callout with the default settings.
    pub fn balloon(view: ViewId, anchor: [f64; 2], text: impl Into<String>) -> Self {
        AnnotationKind::Balloon { view, anchor, text: text.into(), callout: CalloutStyle::default() }
    }

    /// A plain note.
    pub fn note(text: impl Into<String>, height_mm: f64) -> Self {
        AnnotationKind::Note { text: text.into(), height_mm, style: note::NoteStyle::default() }
    }

    /// Center and radius of a circle centerline (view coordinates).
    pub fn circle_centerline(points: &[[f64; 2]]) -> Option<([f64; 2], f64)> {
        match points {
            [c, p] => Some((*c, ((p[0] - c[0]).powi(2) + (p[1] - c[1]).powi(2)).sqrt())),
            [a, b, c] => {
                let d = 2.0 * (a[0] * (b[1] - c[1]) + b[0] * (c[1] - a[1]) + c[0] * (a[1] - b[1]));
                if d.abs() < 1e-18 {
                    return None;
                }
                let sq = |p: &[f64; 2]| p[0] * p[0] + p[1] * p[1];
                let ux = (sq(a) * (b[1] - c[1]) + sq(b) * (c[1] - a[1]) + sq(c) * (a[1] - b[1])) / d;
                let uy = (sq(a) * (c[0] - b[0]) + sq(b) * (a[0] - c[0]) + sq(c) * (b[0] - a[0])) / d;
                Some(([ux, uy], ((a[0] - ux).powi(2) + (a[1] - uy).powi(2)).sqrt()))
            }
            _ => None,
        }
    }

    /// Where the lines `a1`-`b1` and `a2`-`b2` meet (None when parallel).
    pub fn line_intersection(a1: [f64; 2], b1: [f64; 2], a2: [f64; 2], b2: [f64; 2]) -> Option<[f64; 2]> {
        let (r, s) = ([b1[0] - a1[0], b1[1] - a1[1]], [b2[0] - a2[0], b2[1] - a2[1]]);
        let den = r[0] * s[1] - r[1] * s[0];
        if den.abs() < 1e-18 {
            return None;
        }
        let t = ((a2[0] - a1[0]) * s[1] - (a2[1] - a1[1]) * s[0]) / den;
        Some([a1[0] + r[0] * t, a1[1] + r[1] * t])
    }

    /// Mutable access to the view an annotation is attached to.
    pub fn view_mut(&mut self) -> Option<&mut ViewId> {
        match self {
            AnnotationKind::LinearDimension { view, .. }
            | AnnotationKind::RadialDimension { view, .. }
            | AnnotationKind::AngularDimension { view, .. }
            | AnnotationKind::Centermark { view, .. }
            | AnnotationKind::Centerline { view, .. }
            | AnnotationKind::CircleCenterline { view, .. }
            | AnnotationKind::VirtualSharp { view, .. }
            | AnnotationKind::HoleCallout { view, .. }
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

    /// An angular dimension's vertex and legs as drawn (view coordinates):
    /// a flipped leg is its point mirrored through the vertex.
    pub fn angular_legs(&self) -> Option<([f64; 2], [f64; 2], [f64; 2])> {
        let AnnotationKind::AngularDimension { vertex: v, a, b, flip_a, flip_b, .. } = self else { return None };
        let leg = |p: [f64; 2], flip: bool| if flip { [2.0 * v[0] - p[0], 2.0 * v[1] - p[1]] } else { p };
        Some((*v, leg(*a, *flip_a), leg(*b, *flip_b)))
    }

    /// The view an annotation is attached to, if any.
    pub fn view(&self) -> Option<ViewId> {
        match self {
            AnnotationKind::LinearDimension { view, .. }
            | AnnotationKind::RadialDimension { view, .. }
            | AnnotationKind::AngularDimension { view, .. }
            | AnnotationKind::Centermark { view, .. }
            | AnnotationKind::Centerline { view, .. }
            | AnnotationKind::CircleCenterline { view, .. }
            | AnnotationKind::VirtualSharp { view, .. }
            | AnnotationKind::HoleCallout { view, .. }
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

    /// The value a linear or radial dimension measures (meters; a view
    /// rotated by `rotation` radians on the sheet measures along the sheet
    /// axes), for the palette's value field.
    pub fn measured(&self, rotation: f64) -> Option<f64> {
        match self {
            AnnotationKind::LinearDimension { a, b, orientation, .. } => {
                let d = [b[0] - a[0], b[1] - a[1]];
                Some(match orientation {
                    DimOrientation::Aligned => (d[0] * d[0] + d[1] * d[1]).sqrt(),
                    o => {
                        let axis = if *o == DimOrientation::Horizontal { [1.0, 0.0] } else { [0.0, 1.0] };
                        let va = rotate2(axis, -rotation);
                        (d[0] * va[0] + d[1] * va[1]).abs()
                    }
                })
            }
            AnnotationKind::RadialDimension { radius, diameter, .. } => Some(if *diameter { radius * 2.0 } else { *radius }),
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
            AnnotationKind::CircleCenterline { .. } => "Circle centerline",
            AnnotationKind::VirtualSharp { .. } => "Virtual sharp",
            AnnotationKind::HoleCallout { .. } => "Hole callout",
            AnnotationKind::Balloon { .. } => "Callout",
            AnnotationKind::SurfaceFinish { .. } => "Surface finish",
            AnnotationKind::Weld { .. } => "Weld symbol",
            AnnotationKind::RevisionCallout { .. } => "Revision callout",
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
    /// Drawing sketch geometry and hatch regions.
    #[serde(default, skip_serializing_if = "sheet_sketch::SheetSketch::is_empty")]
    pub sketch: sheet_sketch::SheetSketch,
    /// Inserted images (logos, QR and bar codes).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub images: Vec<SheetImage>,
    /// Border and zones (Custom template / Sheet properties).
    #[serde(default, skip_serializing_if = "SheetFormat::is_default")]
    pub format: SheetFormat,
    /// Custom template › Language of the title block's labels.
    #[serde(default, skip_serializing_if = "tb_lang::TitleLanguage::is_english_ref")]
    pub title_language: tb_lang::TitleLanguage,
    /// A sheet of an imported DWG / DXF (its model space or a layout).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub imported: Option<imported::ImportedSpace>,
    /// Sketch constraints made by sketch dimensions (removed with them).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sketch_dim_constraints: Vec<u32>,
    /// Stable id of the sheet (views linked across sheets, [`links`]).
    #[serde(default)]
    pub uid: u32,
    /// Sheet properties › Format: the DWT the sheet's format came from
    /// (None: Custom / a built-in template).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dwt: Option<String>,
    /// The sheet has had a view: its scale shows in the title block from then
    /// on, even when the views are undone or deleted (qa5 D14; "----" only
    /// before the first view, live 820).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub scale_shown: bool,
}

/// Where zone labelling starts (Custom template › Start zones): numbers
/// count from that corner's side and letters from its edge.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ZoneStart {
    #[default]
    BottomRight,
    TopLeft,
}

/// A sheet's border frame and zones (help H:custom_drawing_templates,
/// H:sheets): include the border or not, the number of horizontal and
/// vertical zones (`None` = the size's standard count) and the start corner.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SheetFormat {
    pub border: bool,
    pub zones: Option<[u32; 2]>,
    pub start: ZoneStart,
}

impl Default for SheetFormat {
    fn default() -> Self {
        Self { border: true, zones: None, start: ZoneStart::BottomRight }
    }
}

impl SheetFormat {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    /// Horizontal and vertical zone counts of a sheet of `size`.
    pub fn zone_counts(&self, size: SheetSize) -> [u32; 2] {
        self.zones.unwrap_or_else(|| size.standard_zones())
    }
}

/// An image placed on a sheet (Insert image): the encoded file (PNG, JPEG
/// or GIF, base64), its lower-left corner and size (mm).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SheetImage {
    pub id: u64,
    pub name: String,
    pub data_base64: std::sync::Arc<str>,
    pub pos: [f64; 2],
    pub size: [f64; 2],
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

    /// A view that turns on its own (Rotate view ▸, the API's rotations): no
    /// children, and no parent it is aligned with (none, one on another
    /// sheet, a detail view's, or the alignment suppressed). Live: Rotate
    /// view on views not aligned to a parent (pasted copies, the Iso view),
    /// not on the Top / Front / Right views.
    pub fn turns_freely(&self, id: ViewId) -> bool {
        let Some(v) = self.view(id) else { return false };
        let aligned = self.parent_of(id).is_some_and(|p| !p.is_ghost()) && !v.unaligned && !matches!(v.kind, ViewKind::Detail { .. });
        !aligned && !self.views.iter().any(|x| self.parent_of(x.id) == Some(id))
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

    /// The points a section's cutting line passes through (Edit section):
    /// the stored ones, else the middle of the line's first segment.
    /// Does `id` show an angular cutting line (an aligned section of it, or
    /// a section whose line has a leg off the view's axes)? Such views take
    /// no breaks (help drawing_section_view).
    pub fn has_angular_cut(&self, id: ViewId) -> bool {
        self.views.iter().any(|v| match &v.kind {
            ViewKind::AlignedSection { parent, .. } => *parent == id,
            ViewKind::Section { parent, line, .. } if *parent == id => line.windows(2).any(|w| {
                let d = [w[1][0] - w[0][0], w[1][1] - w[0][1]];
                let l = (d[0] * d[0] + d[1] * d[1]).sqrt().max(1e-12);
                (d[0] / l).abs() > 1e-6 && (d[1] / l).abs() > 1e-6
            }),
            _ => false,
        })
    }

    pub fn section_cuts(&self, id: ViewId) -> Option<Vec<[f64; 2]>> {
        match &self.view(id)?.kind {
            ViewKind::Section { cuts, .. } if !cuts.is_empty() => Some(cuts.clone()),
            ViewKind::Section { line, .. } if line.len() >= 2 => Some(vec![[(line[0][0] + line[1][0]) / 2.0, (line[0][1] + line[1][1]) / 2.0]]),
            _ => None,
        }
    }

    /// Edit section (live drawing/499–504): set the points the cutting line
    /// passes through; the line runs along its first segment's direction,
    /// through the points in order, stepping across halfway between
    /// neighbours, its ends where they were (extended to cover the points).
    pub fn set_section_cuts(&mut self, id: ViewId, mut cuts: Vec<[f64; 2]>) -> bool {
        let Some(v) = self.view_mut(id) else { return false };
        let ViewKind::Section { line, cuts: stored, .. } = &mut v.kind else { return false };
        if line.len() < 2 || cuts.is_empty() {
            return false;
        }
        let d = [line[1][0] - line[0][0], line[1][1] - line[0][1]];
        let l = (d[0] * d[0] + d[1] * d[1]).sqrt();
        if l < 1e-12 {
            return false;
        }
        let u = [d[0] / l, d[1] / l];
        let n = [-u[1], u[0]];
        let along = |p: [f64; 2]| p[0] * u[0] + p[1] * u[1];
        let across = |p: [f64; 2]| p[0] * n[0] + p[1] * n[1];
        let pt = |a: f64, c: f64| [u[0] * a + n[0] * c, u[1] * a + n[1] * c];
        cuts.sort_by(|a, b| along(*a).total_cmp(&along(*b)));
        let (s0, s1) = (along(line[0]), along(*line.last().unwrap_or(&line[0])));
        let margin = (s1 - s0).abs() * 0.02;
        let start = s0.min(along(cuts[0]) - margin);
        let end = s1.max(along(*cuts.last().unwrap_or(&cuts[0])) + margin);
        let mut out = vec![pt(start, across(cuts[0]))];
        for w in cuts.windows(2) {
            let m = (along(w[0]) + along(w[1])) / 2.0;
            if (across(w[0]) - across(w[1])).abs() > 1e-12 {
                out.push(pt(m, across(w[0])));
                out.push(pt(m, across(w[1])));
            }
        }
        out.push(pt(end, across(*cuts.last().unwrap_or(&cuts[0]))));
        *line = out;
        *stored = cuts;
        true
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
        ViewId(self.real_views().map(|v| v.id.0).max().unwrap_or(0) + 1)
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
                _ => v.aux_ref.as_ref().map(|a| a.letter.clone()),
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
        Self { name: "Sheet1".into(), size: SheetSize::A3, scale: 1.0, views: vec![], annotations: vec![], title_block: TitleBlock::default(), tables: vec![], title_style: TitleBlockStyle::Simple, sketch: Default::default(), images: vec![], format: SheetFormat::default(), title_language: Default::default(), imported: None, sketch_dim_constraints: vec![], uid: 0, dwt: None, scale_shown: false }
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
    /// The sheet shown (switching sheets is an undoable change in Onshape:
    /// live behaviours "Sheet properties › Select a DWT" (5)).
    #[serde(default, skip_serializing_if = "is_zero_usize")]
    pub active_sheet: usize,
    /// A picked DWT's continuation format, for new sheets ([`dwt`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub continuation: Option<Box<dwt::SheetTemplate>>,
    /// Who drew it (title block DRAWN, `{drawing.drawn_by}`): the user
    /// that created it, kept with the drawing (qa5 D13).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drawn_by: Option<String>,
    /// The document state (microversion id) its views were last made
    /// from: they keep that geometry until Update, also after the document
    /// is reopened (live "qa5 pass 7" (6); qa5 D17). None: the current one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sources_at: Option<String>,
    /// The drawing element's own properties (Tab › Properties).
    #[serde(default, skip_serializing_if = "meta_is_empty")]
    pub meta: Box<DrawingMeta>,
}

/// A drawing element's own properties (Tab › Properties). Live ("dx title
/// block mappings"): the title block's DWG NO. is its Part number and REV
/// its Revision (never the part's); DRAWN's date is when it was created.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DrawingMeta {
    pub number: String,
    pub revision: String,
    pub description: String,
    pub title1: String,
    pub title2: String,
    pub title3: String,
    /// Date drawn (YYYY-MM-DD, UTC), set when the drawing is made.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created: Option<String>,
}

fn meta_is_empty(m: &DrawingMeta) -> bool {
    *m == DrawingMeta::default()
}

fn is_zero_usize(n: &usize) -> bool {
    *n == 0
}

impl Default for Drawing {
    fn default() -> Self {
        Self { sheets: vec![Sheet::default()], projection: Projection::ThirdAngle, units_mm: true, props: DrawingProps::default(), active_sheet: 0, continuation: None, drawn_by: None, sources_at: None, meta: Box::default() }
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
    /// Units and precision › Fit class tolerance precision (live default
    /// 0.1234; None: 4 decimals).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fit_tolerance_precision: Option<u8>,
    /// Units and precision › Dual.
    #[serde(default, skip_serializing_if = "DualDims::is_default")]
    pub dual: DualDims,
    /// Footer: the template the properties were last updated from
    /// ("Update properties from a template…") and ☐ Lock drawing properties.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub locked: bool,
    /// Inspection: bubble shape, hide all (toolbar toggle), leave callouts
    /// out of automatic numbering.
    #[serde(default)]
    pub inspection_shape: InspectionShape,
    #[serde(default)]
    pub hide_inspection: bool,
    #[serde(default)]
    pub inspection_exclude_callouts: bool,
    /// Inspection › Item parsing: Row (one item per callout) instead of
    /// Parameter (hole callouts and chamfers split into parameters).
    #[serde(default)]
    pub inspection_row_parsing: bool,
    /// Inspection › Text height and Border size (mm).
    #[serde(default = "inspection_text")]
    pub inspection_text_height: f64,
    #[serde(default = "inspection_border")]
    pub inspection_border_size: f64,
    /// Inspection › Border size in characters (live select "2 Characters";
    /// 0: Tight Fit): the bubble is at least that wide.
    #[serde(default = "inspection_chars")]
    pub inspection_border_chars: u8,
    /// Inspection › Border color (None = black) and Solid fill (live
    /// default ☑, dom/drawing-wt-widgets).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inspection_color: Option<[u8; 3]>,
    #[serde(default)]
    pub inspection_solid_fill: bool,
    /// Inspection › Text colour (None = black) and Solid fill's colour
    /// (None = live's orange, rgb 255 99 51; drawing/1710).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inspection_text_color: Option<[u8; 3]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inspection_fill_color: Option<[u8; 3]>,
    /// Views › Insert view defaults (new base views; derived views copy
    /// their parent).
    #[serde(default)]
    pub view_defaults: ViewDefaults,
    /// Dimensions › All dimensions and Annotations › All annotations:
    /// text height and arrowhead (mm), arrowhead type, color.
    #[serde(default = "text_35")]
    pub dim_text_height: f64,
    #[serde(default = "arrow_3")]
    pub dim_arrow: f64,
    #[serde(default)]
    pub dim_arrow_type: sheet::ArrowType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dim_color: Option<[u8; 3]>,
    #[serde(default = "text_35")]
    pub ann_text_height: f64,
    #[serde(default = "arrow_3")]
    pub ann_arrow: f64,
    #[serde(default)]
    pub ann_arrow_type: sheet::ArrowType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ann_color: Option<[u8; 3]>,
    /// Image context menu › Show / Hide image borders (all images).
    #[serde(default)]
    pub image_borders: bool,
    /// Font of dimensions, annotations and inspection items (None: the
    /// default sans).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dim_font: Option<sheet::TextFont>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ann_font: Option<sheet::TextFont>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inspection_font: Option<sheet::TextFont>,
    /// Dimensions › Gaps and extensions (mm): text gap (text to the
    /// dimension line), geometry gap (geometry to the extension line) and
    /// extension past the dimension line; text aligned horizontally
    /// instead of with the dimension line.
    #[serde(default = "gap_text")]
    pub dim_text_gap: f64,
    #[serde(default = "gap_geometry")]
    pub dim_geometry_gap: f64,
    #[serde(default = "gap_extension")]
    pub dim_extension: f64,
    #[serde(default)]
    pub dim_text_horizontal: bool,
    /// Dimensions › All dimensions: Text justification, Underline overridden
    /// dimensions (on), Break dimension by default (on), Fit standard (None:
    /// the drawing's standard).
    #[serde(default)]
    pub dim_justify: TextJustify,
    #[serde(default = "yes")]
    pub underline_overridden: bool,
    #[serde(default = "yes")]
    pub break_dims: bool,
    #[serde(default)]
    pub dim_fit_ansi: Option<bool>,
    /// Gaps and extensions › Lock the angle of all jogged dimensions to 45
    /// degrees (off).
    #[serde(default)]
    pub lock_jog_45: bool,
    /// Ordinate dimensions › Dimension style (None: the drawing's standard).
    #[serde(default)]
    pub ordinate_ansi: Option<bool>,
    /// Diameter and radial dimensions › Show dimension lines by default
    /// (None: on for ISO, off for ANSI).
    #[serde(default)]
    pub radial_dim_lines: Option<bool>,
    /// Tables › All tables: horizontal and vertical cell padding (mm, each
    /// side; Onshape's default is 0.06 in).
    #[serde(default = "table_padding")]
    pub table_h_padding: f64,
    #[serde(default = "table_padding")]
    pub table_v_padding: f64,
    /// Formats: Locked (the border and title block can't be selected), and
    /// the colors of the border frame, border zones and title block.
    #[serde(default)]
    pub formats_locked: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub border_color: Option<[u8; 3]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zone_color: Option<[u8; 3]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title_block_color: Option<[u8; 3]>,
    /// Formats › Border frame / Border zone / Title block › Thickness (mm;
    /// live 0.15 each, dom/drawing-wt-widgets).
    #[serde(default = "format_pen")]
    pub border_thickness: f64,
    #[serde(default = "format_pen")]
    pub zone_thickness: f64,
    #[serde(default = "format_pen")]
    pub title_block_thickness: f64,
    /// Annotations › per type (Notes, Callouts, Datums, ...): font, text
    /// height, color overrides; Datums › Datum size (mm); Notes › Hide
    /// property placeholder (empty fields show nothing instead of "----").
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ann_types: Vec<(sheet::AnnType, sheet::AnnTypeStyle)>,
    #[serde(default = "datum_size")]
    pub datum_size: f64,
    #[serde(default)]
    pub hide_placeholder: bool,
    /// Views tab per view type and the Construction geometry tab.
    #[serde(default)]
    pub view_tab: Box<ViewTabProps>,
    /// Tables tab: each kind's line pen, font and row texts.
    #[serde(default)]
    pub table_styles: Box<TableStyles>,
    /// Annotations › Center of mass › indicator diameter (mm).
    #[serde(default = "com_diameter")]
    pub com_diameter: f64,
    /// Annotations › Weld symbols › standard: Some(true) ANSI, Some(false)
    /// ISO; None: the drawing's standard (its sheet size).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weld_ansi: Option<bool>,
    /// Dimensions › Chamfer: style (true: Dimension, false: Note) and
    /// content of 45° chamfers and of others, length and angle precision.
    #[serde(default)]
    pub chamfer_45_dimension: bool,
    #[serde(default)]
    pub chamfer_45_content: ChamferContent,
    #[serde(default = "yes")]
    pub chamfer_dimension: bool,
    #[serde(default = "length_x_length")]
    pub chamfer_content: ChamferContent,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chamfer_length_precision: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chamfer_angle_precision: Option<u8>,
    /// Annotations › Notes › Hide property background (the grey shown
    /// behind property fields on screen).
    #[serde(default)]
    pub hide_property_background: bool,
    /// Annotations › Flag notes › Border shape of new flag notes.
    #[serde(default = "flag_shape")]
    pub flag_shape: note::FlagShape,
    /// Annotations › Callouts › Text justification, Stack direction and
    /// Stack gap size (mm).
    #[serde(default)]
    pub callout_justify: TextJustify,
    #[serde(default)]
    pub callout_stack: StackDirection,
    #[serde(default = "stack_gap")]
    pub callout_stack_gap: f64,
    /// Annotations › Datums › Target hatch pattern: group and pattern
    /// (`hatch_pat::pattern`).
    #[serde(default)]
    pub target_hatch: (usize, usize),
    /// Annotations › Hole/thread callouts › Indicator (the count's "X")
    /// and Fit standard (ISO, else ANSI).
    #[serde(default = "hole_indicator")]
    pub hole_indicator: String,
    #[serde(default)]
    pub fit_iso: bool,
    /// The color dialog's Custom colors.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub custom_colors: Vec<[u8; 3]>,
}

/// A version a view reference is pinned to.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PinnedVersion {
    /// The store's version id (as text).
    pub id: String,
    /// Its name ("V1").
    pub name: String,
}

/// Drawing properties › Views (per view type) and › Construction geometry.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ViewTabProps {
    /// Hidden and tangent edge colors (None: ink).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hidden_color: Option<[u8; 3]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tangent_color: Option<[u8; 3]>,
    /// Section views: hatch line spacing (mm) and label prefix.
    pub hatch_spacing: f64,
    pub section_prefix: String,
    /// Detail views: profile lines (0 Phantom, 1 Dashed, 2 Solid), label
    /// prefix, and the defaults of new detail views (style 0 In line,
    /// 1 Connected, 2 Leader; show profile; clip border).
    pub detail_line: u8,
    pub detail_prefix: String,
    pub detail_style: u8,
    pub detail_show_profile: bool,
    pub detail_clip_border: bool,
    /// Auxiliary views: show the view reference indicator, label prefix,
    /// label style ("A" or "A-A") and suffix.
    pub aux_indicator: bool,
    pub aux_prefix: String,
    pub aux_double_label: bool,
    pub aux_suffix: String,
    /// View reference format: Arrow style (one arrow and letter at the
    /// middle, no viewing line) instead of Line style.
    #[serde(default)]
    pub aux_arrow_format: bool,
    /// Construction geometry: centermark and centerline colors (None: ink);
    /// `centermark_extension` is kept for old files (unused: live's rule is
    /// Mark size / Gap size below).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub centermark_color: Option<[u8; 3]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub centerline_color: Option<[u8; 3]>,
    pub centermark_extension: f64,
    /// Construction geometry › Centermarks › Mark size and Gap size (mm,
    /// live default 0; Onshape adds 0.045 in to both) and Extended lines
    /// (live ☑): the auto centermark's cross and extended lines (build.rs).
    pub centermark_mark_size: f64,
    pub centermark_gap_size: f64,
    pub centermark_extended: bool,
    /// Flat pattern views: up / down bend line styles (0 Solid, 1 Dashed,
    /// 2 Center) and colors, and the bend tangent edges' color.
    pub bend_up_line: u8,
    pub bend_down_line: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bend_up_color: Option<[u8; 3]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bend_down_color: Option<[u8; 3]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bend_tangent_color: Option<[u8; 3]>,
    /// Views › All views › Visible edges: pen (mm) and colour (None: ink).
    pub visible_thickness: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visible_color: Option<[u8; 3]>,
    /// Views › Hidden edges / Tangent edges / Exploded lines: pen (mm) and
    /// dash scale (1 = the line type's own dashes); the colours above.
    pub hidden_thickness: f64,
    pub hidden_dashes: f64,
    pub tangent_thickness: f64,
    pub tangent_dashes: f64,
    pub exploded_thickness: f64,
    pub exploded_dashes: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exploded_color: Option<[u8; 3]>,
    /// Views › All views › View labels: font (None: the drawing sans),
    /// height (mm; None: the annotations' text height, as Onshape's
    /// templates: 0.12 in / 3.5 mm) and colour.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub view_label_font: Option<sheet::TextFont>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub view_label_height: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub view_label_color: Option<[u8; 3]>,
    /// Section views › Cutting lines, Arrowhead, Cutting line label; View
    /// and region hatches' pen.
    pub section_line: PenProps,
    pub section_arrow: ArrowProps,
    pub section_label: LabelProps,
    pub hatch_line: PenProps,
    /// Detail views › Detail profile lines (pen; style `detail_line`),
    /// Arrowhead, Detail profile label.
    pub detail_pen: PenProps,
    pub detail_arrow: ArrowProps,
    pub detail_label: LabelProps,
    /// Auxiliary views › Viewing lines, Arrowhead, Viewing line label.
    pub aux_line: PenProps,
    pub aux_arrow: ArrowProps,
    pub aux_label: LabelProps,
    /// Break views › Break lines.
    pub break_line: PenProps,
    /// Cosmetic threads › Thread style (None: the sheet's, ANSI or ISO),
    /// pen and colour, dash scale.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thread_style: Option<ThreadStyle>,
    pub thread_line: PenProps,
    pub thread_dashes: f64,
    /// Flat pattern views › All bend lines' and Bend tangent edges' pens
    /// (colours above), Form centermarks' line style (0 Solid, 1 Dashed,
    /// 2 Center) and colour; Counterbore / Countersink up / down, Form
    /// tear / up / down styles and colours (Form up / down drawn; the
    /// counterbore / countersink rows wait for hole heads in the flat).
    pub bend_lines_thickness: f64,
    pub bend_tangent_thickness: f64,
    pub form_centermark_line: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub form_centermark_color: Option<[u8; 3]>,
    pub flat_lines: Vec<(u8, Option<[u8; 3]>)>,
    /// Construction geometry › Centermarks / Centerlines › Thickness (live
    /// 0.13 mm) and Centerlines › Dashes (the automatic ones in views).
    pub centermark_thickness: f64,
    pub centerline_thickness: f64,
    pub centerline_dashes: f64,
    /// Construction geometry › Created in drawing › Virtual sharps: Edge
    /// extension (else Centermark) and pen (live 0.40 mm); Sketch geometry's
    /// pen (live 0.15 mm) and colour.
    pub virtual_sharp_extension: bool,
    pub virtual_sharp_line: PenProps,
    pub sketch_line: PenProps,
    /// Created in drawing › Sketch driven dimensions color (live #666666).
    pub sketch_driven_dim_color: [u8; 3],
    /// Part Studio sketches shown in views (live 10-26 c): Solid lines (0.40
    /// mm) and Construction lines (0.13 mm, long-short dashes) pens; Sketch
    /// points' shape, size (sheet mm; live Circle, 1 mm) and colour.
    pub sketch_solid: PenProps,
    pub sketch_construction: PenProps,
    pub sketch_point_shape: SketchPointShape,
    pub sketch_point_size: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sketch_point_color: Option<[u8; 3]>,
    /// Views › View and region hatches › Hatch pattern: (standard, pattern)
    /// in `hatch_pat::GROUPS` (live default ANSI, "ANSI31 general").
    pub hatch_pattern: (usize, usize),
}

/// Construction geometry › Part Studio sketches › Sketch points shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SketchPointShape {
    None,
    Centermark,
    Cross,
    Circle,
    CentermarkCircle,
    CrossCircle,
}

impl SketchPointShape {
    pub const ALL: [SketchPointShape; 6] = [Self::None, Self::Centermark, Self::Cross, Self::Circle, Self::CentermarkCircle, Self::CrossCircle];
    pub fn name(self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Centermark => "Centermark",
            Self::Cross => "Cross",
            Self::Circle => "Circle",
            Self::CentermarkCircle => "Centermark circle",
            Self::CrossCircle => "Cross circle",
        }
    }
}

/// Sketch points sizes (mm), listed ".25 mm" … "5 mm".
pub const SKETCH_POINT_SIZES: [f64; 7] = [0.25, 0.5, 1.0, 2.0, 3.0, 4.0, 5.0];

/// Construction lines of shown sketches: long dash, gap, short dash, gap
/// (paper mm, live 10-26 c).
pub const SKETCH_CONSTRUCTION_DASHES: [f64; 4] = [8.5, 1.4, 2.0, 1.4];

/// Cosmetic thread drawing standard (Drawing properties › Views).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThreadStyle {
    Ansi,
    Iso,
    Jis,
}

impl ThreadStyle {
    pub const ALL: [ThreadStyle; 3] = [ThreadStyle::Ansi, ThreadStyle::Iso, ThreadStyle::Jis];
    pub fn name(self) -> &'static str {
        match self {
            ThreadStyle::Ansi => "ANSI",
            ThreadStyle::Iso => "ISO",
            ThreadStyle::Jis => "JIS",
        }
    }
}

/// Flat pattern views' stored line rows: Counterbore/Countersink up and
/// down, Form tear, up, down (live defaults Solid, Solid, Solid, Center,
/// Center).
pub const FLAT_LINE_ROWS: [(&str, u8); 5] = [("Counterbore/Countersink up", 0), ("Counterbore/Countersink down", 0), ("Form tear", 0), ("Form up", 2), ("Form down", 2)];

/// Drawing properties › Tables, per kind (live dom/drawing-wt-widgets:
/// BOM, Custom, Cut list, Hole, Revision tables and Tables).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TableStyles {
    pub general: TableStyle,
    pub bom: TableStyle,
    pub custom: TableStyle,
    pub cut_list: TableStyle,
    pub hole: TableStyle,
    pub revision: TableStyle,
    /// Hole tables › Indicator style (ISO, else ANSI; stored), Indicator
    /// arrowhead (mm; None: the drawing's default), Indicator lines (mm;
    /// live 0.15), Tags' font.
    pub hole_indicator_iso: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hole_indicator_arrow: Option<f64>,
    pub hole_indicator_line: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hole_tags_font: Option<sheet::TextFont>,
    /// Hole tables › Indicator lines' colour and the Tags' text (live 1710:
    /// the annotations' height).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hole_indicator_color: Option<[u8; 3]>,
    pub hole_tags: RowText,
    /// Revision tables › Revision callouts: border (live Triangle), fit in
    /// characters (0: Tight Fit; live 1), font and text.
    pub revision_callout_shape: CalloutBorder,
    pub revision_callout_chars: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revision_callout_font: Option<sheet::TextFont>,
    pub revision_callout_text: RowText,
}

impl Default for TableStyles {
    fn default() -> Self {
        let t = TableStyle::default();
        Self { general: t.clone(), bom: t.clone(), custom: t.clone(), cut_list: t.clone(), hole: t.clone(), revision: t, hole_indicator_iso: false,
            hole_indicator_arrow: None,
            hole_indicator_line: 0.15,
            hole_tags_font: None,
            hole_indicator_color: None,
            hole_tags: RowText::default(),
            revision_callout_shape: CalloutBorder::Triangle,
            revision_callout_chars: 1,
            revision_callout_font: None,
            revision_callout_text: RowText::default(),
        }
    }
}

impl TableStyles {
    pub fn of(&self, kind: &TableKind) -> &TableStyle {
        match kind {
            TableKind::General { .. } => &self.general,
            TableKind::Bom { .. } => &self.bom,
            TableKind::Custom { .. } => &self.custom,
            TableKind::CutList { .. } => &self.cut_list,
            TableKind::Hole { .. } => &self.hole,
            TableKind::Revision { .. } => &self.revision,
        }
    }
}

fn yes_hole() -> bool {
    true
}

fn is_true(b: &bool) -> bool {
    *b
}

/// One kind's table look: line pen (mm), font, title row height (None:
/// 1.3 × the content's, live 0.1560 in) and its Bold (live pressed) /
/// Italic / colour, header and content rows.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TableStyle {
    pub line: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font: Option<sheet::TextFont>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<f64>,
    pub title_text: RowText,
    pub header: RowText,
    pub content: RowText,
}

impl Default for TableStyle {
    fn default() -> Self {
        // Live: 0.13 mm, title and header rows Bold.
        Self { line: 0.13, font: None, title: None, title_text: RowText { bold: true, ..Default::default() }, header: RowText { bold: true, ..Default::default() }, content: RowText::default() }
    }
}

/// A table row's text: height (mm; None: the annotations' text height,
/// live 0.1200 in / 3.5 mm), Bold, Italic, colour.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RowText {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<f64>,
    pub bold: bool,
    pub italic: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<[u8; 3]>,
}

/// A line's pen (mm) and colour (None: ink).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PenProps {
    pub thickness: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<[u8; 3]>,
}

impl PenProps {
    pub fn new(thickness: f64) -> Self {
        Self { thickness, color: None }
    }
}

/// An arrowhead: size (mm; None: the drawing's default for it) and type.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ArrowProps {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<f64>,
    #[serde(default)]
    pub kind: sheet::ArrowType,
}

/// A label's font (None: the drawing sans), height (mm; None: its default)
/// and colour.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct LabelProps {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font: Option<sheet::TextFont>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<[u8; 3]>,
}

impl Default for ViewTabProps {
    fn default() -> Self {
        Self {
            hidden_color: None,
            tangent_color: None,
            hatch_spacing: 2.5,
            section_prefix: "SECTION".into(),
            // Live dom/drawing-properties-views.txt: Phantom.
            detail_line: 0,
            detail_prefix: "DETAIL".into(),
            detail_style: 0,
            detail_show_profile: true,
            detail_clip_border: false,
            aux_indicator: true,
            // Live dom/drawing-properties-views.txt: "Auxiliary", A-A.
            aux_prefix: "Auxiliary".into(),
            aux_double_label: true,
            aux_arrow_format: false,
            aux_suffix: String::new(),
            centermark_color: None,
            centerline_color: None,
            centermark_extension: 1.0,
            centermark_mark_size: 0.0,
            centermark_gap_size: 0.0,
            centermark_extended: true,
            bend_up_line: 2,
            bend_down_line: 2,
            bend_up_color: None,
            bend_down_color: None,
            bend_tangent_color: None,
            // Live dom/drawing-wt-widgets (ANSI inch and ISO mm alike).
            visible_thickness: sheet::LineStyle::Visible.width_mm(),
            visible_color: None,
            hidden_thickness: sheet::LineStyle::Hidden.width_mm(),
            hidden_dashes: 1.0,
            tangent_thickness: sheet::LineStyle::Tangent.width_mm(),
            tangent_dashes: 1.0,
            exploded_thickness: 0.13,
            exploded_dashes: 1.0,
            exploded_color: None,
            view_label_font: None,
            view_label_height: None,
            view_label_color: None,
            // Live dom/drawing-properties-views: 0.13 / 0.15 / 0.13 / 0.13 /
            // 0.25 mm.
            section_line: PenProps::new(0.13),
            section_arrow: ArrowProps::default(),
            section_label: LabelProps::default(),
            hatch_line: PenProps::new(0.15),
            detail_pen: PenProps::new(0.13),
            detail_arrow: ArrowProps::default(),
            detail_label: LabelProps::default(),
            aux_line: PenProps::new(0.13),
            aux_arrow: ArrowProps::default(),
            aux_label: LabelProps::default(),
            break_line: PenProps::new(0.25),
            thread_style: None,
            thread_line: PenProps::new(0.15),
            thread_dashes: 1.0,
            bend_lines_thickness: 0.13,
            bend_tangent_thickness: 0.15,
            form_centermark_line: 2,
            form_centermark_color: None,
            flat_lines: FLAT_LINE_ROWS.iter().map(|r| (r.1, None)).collect(),
            centermark_thickness: 0.13,
            centerline_thickness: 0.13,
            centerline_dashes: 1.0,
            virtual_sharp_extension: false,
            virtual_sharp_line: PenProps::new(0.40),
            sketch_line: PenProps::new(0.15),
            sketch_driven_dim_color: [102, 102, 102],
            sketch_solid: PenProps::new(0.40),
            sketch_construction: PenProps::new(0.13),
            sketch_point_shape: SketchPointShape::Circle,
            sketch_point_size: 1.0,
            sketch_point_color: None,
            hatch_pattern: (0, 0),
        }
    }
}

fn inspection_chars() -> u8 {
    2
}

fn datum_size() -> f64 {
    3.0
}

fn length_x_length() -> ChamferContent {
    ChamferContent::LengthXLength
}

/// What a chamfer dimension reads (Drawing properties › Dimensions ›
/// Chamfer content).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChamferContent {
    AngleXLength,
    #[default]
    LengthXAngle,
    LengthXLength,
    /// "C2" (45° chamfers only).
    CLength,
}

impl ChamferContent {
    pub const ALL: [ChamferContent; 4] = [ChamferContent::AngleXLength, ChamferContent::LengthXAngle, ChamferContent::LengthXLength, ChamferContent::CLength];

    pub fn name(self) -> &'static str {
        match self {
            ChamferContent::AngleXLength => "Angle x Length",
            ChamferContent::LengthXAngle => "Length x Angle",
            ChamferContent::LengthXLength => "Length x Length",
            ChamferContent::CLength => "C Length",
        }
    }
}

fn flag_shape() -> note::FlagShape {
    note::FlagShape::Flag
}

fn stack_gap() -> f64 {
    1.27
}

fn hole_indicator() -> String {
    "X".into()
}

/// A geometric tolerance's texts and extra frame rows (live dialog
/// drawing/200–215): upper text above the frame, lower text below, each
/// row's prefix (left of the frame) and suffix (right of its row), rows
/// after the first (their symbol "Composite" shares the first row's symbol
/// cell), and the leader end (Leader / All around / All over).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GdtExtra {
    pub upper: String,
    pub lower: String,
    /// Per row (the first row's at 0).
    pub prefix: Vec<String>,
    pub suffix: Vec<String>,
    /// Rows after the first: symbol (or "Composite"), tolerance, datums.
    pub rows: Vec<Vec<String>>,
    pub leader: GdtLeader,
}

impl GdtExtra {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// How a geometric tolerance's leader ends (the dialog's Leader menu).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum GdtLeader {
    #[default]
    Leader,
    AllAround,
    AllOver,
}

/// Callout border shapes (the Callout dialog's Border shape).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CalloutBorder {
    None,
    #[default]
    Circle,
    Diamond,
    DoubleCircle,
    Flag,
    Hexagon,
    Octagon,
    #[serde(alias = "Box")]
    Rectangle,
    SplitCircle,
    SquareCircle,
    Triangle,
    Underline,
}

impl CalloutBorder {
    /// Onshape's list order.
    pub const ALL: [CalloutBorder; 12] = [
        CalloutBorder::None,
        CalloutBorder::Circle,
        CalloutBorder::Diamond,
        CalloutBorder::DoubleCircle,
        CalloutBorder::Flag,
        CalloutBorder::Hexagon,
        CalloutBorder::Octagon,
        CalloutBorder::Rectangle,
        CalloutBorder::SplitCircle,
        CalloutBorder::SquareCircle,
        CalloutBorder::Triangle,
        CalloutBorder::Underline,
    ];

    pub fn name(self) -> &'static str {
        match self {
            CalloutBorder::None => "None",
            CalloutBorder::Circle => "Circle",
            CalloutBorder::Diamond => "Diamond",
            CalloutBorder::DoubleCircle => "Double Circle",
            CalloutBorder::Flag => "Flag",
            CalloutBorder::Hexagon => "Hexagon",
            CalloutBorder::Octagon => "Octagon",
            CalloutBorder::Rectangle => "Rectangle",
            CalloutBorder::SplitCircle => "Split Circle",
            CalloutBorder::SquareCircle => "Square Circle",
            CalloutBorder::Triangle => "Triangle",
            CalloutBorder::Underline => "Underline",
        }
    }
}

/// A callout's settings beyond its main text (Callout dialog): texts
/// above / below (outside the border) and left / right (beside the main
/// text), the border shape and size (characters), text height, and the
/// stacked callouts.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CalloutStyle {
    pub above: String,
    pub below: String,
    pub left: String,
    pub right: String,
    pub border: CalloutBorder,
    /// Border size in characters (0: Tight Fit).
    pub chars: u8,
    /// Attached to a face (an assembly part): the leader ends in a dot,
    /// else (an edge or point) in an arrow.
    pub on_face: bool,
    /// Placed in empty space: no leader.
    pub free: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<f64>,
    /// Stacked callouts (Add stacked callouts), in order.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub stacked: Vec<StackedCallout>,
    /// Bend nodes of the leader to the anchor (sheet mm; Add node, live
    /// drw2 10-21 g (6)).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub nodes: Vec<[f64; 2]>,
    /// Leaders added with Add leader (arrows). Their `start` indexes
    /// [`CalloutStyle::all_leaders`] (0 = the leader to the anchor).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub leaders: Vec<Leader>,
    /// The part clicked when the anchor has no point reference (an edge
    /// away from its snap points): its BOM item and properties (qa5 D8).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub part: Option<BodyId>,
    /// On a dimension (the Callout tool clicked on its lines or text; help
    /// callout_balloon): the leader ends there and follows it, and deleting
    /// the dimension deletes the callout.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on: Option<DatumOn>,
}

/// One stacked callout: its instance (part) and text (empty = that part's
/// BOM item number).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct StackedCallout {
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<BodyId>,
}

impl Default for CalloutStyle {
    fn default() -> Self {
        Self { above: String::new(), below: String::new(), left: String::new(), right: String::new(), border: CalloutBorder::Circle, chars: 2, on_face: false, free: false, height: None, stacked: Vec::new(), nodes: Vec::new(), leaders: Vec::new(), part: None, on: None }
    }
}

impl CalloutStyle {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// Adjust linestyle (live drawing/443–446): an edge's linestyle, dash
/// scale, thickness (mm) and color.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EdgeStyle {
    pub body: BodyId,
    pub name: TopoName,
    pub pattern: LinePattern,
    /// Dashes: the dash pattern's scale (1.00).
    pub dashes: f64,
    pub thickness: f64,
    #[serde(default)]
    pub color: Option<[u8; 3]>,
}

/// Adjust linestyle › Linestyle (live order).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LinePattern {
    #[default]
    Solid,
    Phantom,
    Center,
    Dashed,
    Dotted,
}

impl LinePattern {
    pub const ALL: [LinePattern; 5] = [LinePattern::Solid, LinePattern::Phantom, LinePattern::Center, LinePattern::Dashed, LinePattern::Dotted];

    pub fn name(self) -> &'static str {
        match self {
            LinePattern::Solid => "Solid",
            LinePattern::Phantom => "Phantom",
            LinePattern::Center => "Center",
            LinePattern::Dashed => "Dashed",
            LinePattern::Dotted => "Dotted",
        }
    }

    /// The dash pattern (sheet mm) at scale `k` (Dashes). Live (behaviours
    /// "dx linestyle dash lengths"; its DXF LTYPEs × $LTSCALE 0.5, in
    /// inches): Dashed 0.25 / 0.125 in, Phantom 0.125 + two 0.025 dashes,
    /// Center 0.225 + 0.045, Dotted a dot every 0.125 in (drawn as a dash
    /// as long as a 0.35 mm line is wide).
    pub fn dashes(self, k: f64) -> Vec<f64> {
        let base: &[f64] = match self {
            LinePattern::Solid => &[],
            LinePattern::Phantom => &[3.175, 0.635, 0.635, 0.635, 0.635, 0.635],
            LinePattern::Center => &[5.715, 1.143, 1.143, 1.143],
            LinePattern::Dashed => &[6.35, 3.175],
            LinePattern::Dotted => &[0.35, 2.825],
        };
        base.iter().map(|d| d * k.max(0.01)).collect()
    }
}

/// Adjust linestyle › Thickness choices (mm, live order).
pub const LINE_THICKNESSES: [f64; 19] = [0.13, 0.15, 0.18, 0.20, 0.25, 0.30, 0.35, 0.40, 0.50, 0.53, 0.60, 0.70, 0.80, 0.90, 1.00, 1.20, 1.40, 1.60, 2.00];

fn no_arms(a: &[f64; 4]) -> bool {
    a.iter().all(|x| *x == 0.0)
}

fn no_jogs(j: &[Option<ExtJog>; 2]) -> bool {
    j.iter().all(Option::is_none)
}

/// A jog in a dimension's extension line (help: jogged extension lines):
/// from `at` (fraction of the extension line from the model point) a
/// diagonal goes `drop` mm further and `shift` mm sideways (along the
/// dimension line); the rest of the line and the dimension line's end move
/// by `shift`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ExtJog {
    pub at: f64,
    pub shift: f64,
    pub drop: f64,
}

impl ExtJog {
    /// The jog Add jog makes: at the middle of the extension line, 45°.
    pub fn new(shift: f64) -> Self {
        Self { at: 0.5, shift, drop: shift.abs() }
    }
}

/// Text justification (Callouts).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextJustify {
    Left,
    #[default]
    Center,
    Right,
    /// Toward the leader (centered inside a border).
    Auto,
}

impl TextJustify {
    pub const ALL: [TextJustify; 4] = [TextJustify::Left, TextJustify::Center, TextJustify::Right, TextJustify::Auto];

    pub fn name(self) -> &'static str {
        match self {
            TextJustify::Left => "Left",
            TextJustify::Center => "Center",
            TextJustify::Right => "Right",
            TextJustify::Auto => "Auto",
        }
    }

    /// Horizontal anchor fraction (0 left, 0.5 center, 1 right); Auto
    /// follows the leader: `leader_left` when it leaves to the left, or
    /// centers inside a border (`bordered`).
    pub fn frac(self, bordered: bool, leader_left: bool) -> f64 {
        match self {
            TextJustify::Left => 0.0,
            TextJustify::Center => 0.5,
            TextJustify::Right => 1.0,
            TextJustify::Auto if bordered => 0.5,
            TextJustify::Auto => if leader_left { 0.0 } else { 1.0 },
        }
    }
}

/// Where stacked callouts go from the first one.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum StackDirection {
    #[default]
    Auto,
    Right,
    Left,
    Up,
    Down,
}

impl StackDirection {
    pub const ALL: [StackDirection; 5] = [StackDirection::Auto, StackDirection::Right, StackDirection::Left, StackDirection::Up, StackDirection::Down];

    pub fn name(self) -> &'static str {
        match self {
            StackDirection::Auto => "Auto",
            StackDirection::Right => "Right",
            StackDirection::Left => "Left",
            StackDirection::Up => "Up",
            StackDirection::Down => "Down",
        }
    }

    /// The unit step on the sheet; Auto stacks away from the leader
    /// (`from_anchor`: callout center minus anchor), horizontally unless
    /// the leader is mostly vertical.
    pub fn step(self, from_anchor: [f64; 2]) -> [f64; 2] {
        match self {
            StackDirection::Right => [1.0, 0.0],
            StackDirection::Left => [-1.0, 0.0],
            StackDirection::Up => [0.0, 1.0],
            StackDirection::Down => [0.0, -1.0],
            StackDirection::Auto if from_anchor[1].abs() > from_anchor[0].abs() * 2.0 => [0.0, from_anchor[1].signum()],
            StackDirection::Auto => [if from_anchor[0] < 0.0 { -1.0 } else { 1.0 }, 0.0],
        }
    }
}

impl DrawingProps {
    /// An annotation type's style (mutable; added when missing).
    pub fn ann_type_mut(&mut self, t: sheet::AnnType) -> &mut sheet::AnnTypeStyle {
        if let Some(i) = self.ann_types.iter().position(|(k, _)| *k == t) {
            return &mut self.ann_types[i].1;
        }
        self.ann_types.push((t, sheet::AnnTypeStyle::default()));
        &mut self.ann_types.last_mut().expect("just pushed").1
    }

    pub fn ann_type(&self, t: sheet::AnnType) -> sheet::AnnTypeStyle {
        self.ann_types.iter().find(|(k, _)| *k == t).map(|(_, s)| s.clone()).unwrap_or_default()
    }
}

fn com_diameter() -> f64 {
    6.35
}

fn table_padding() -> f64 {
    1.524
}

fn text_35() -> f64 {
    3.5
}

fn gap_text() -> f64 {
    1.2
}

fn gap_geometry() -> f64 {
    1.0
}

fn gap_extension() -> f64 {
    2.0
}

fn arrow_3() -> f64 {
    3.0
}

/// Drawing properties › Views › Insert view defaults.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ViewDefaults {
    pub simplification: Simplification,
    pub auto_centermarks: bool,
    pub auto_centerlines: bool,
    /// Hidden lines Shown.
    pub hidden_lines: bool,
    /// Tangent edges: 0 Hidden, 1 Solid, 2 Phantom.
    pub tangent: u8,
    pub scale_label: bool,
    /// Threads (Parts) Shown, Bend lines (Parts) On, Bend notes (Parts) On
    /// (live's defaults).
    #[serde(default = "yes")]
    pub threads: bool,
    #[serde(default = "yes")]
    pub bend_lines: bool,
    #[serde(default = "yes")]
    pub bend_notes: bool,
}

impl Default for ViewDefaults {
    fn default() -> Self {
        Self { simplification: Simplification::Auto, auto_centermarks: true, auto_centerlines: false, hidden_lines: false, tangent: 1, scale_label: true, threads: true, bend_lines: true, bend_notes: true }
    }
}

impl ViewDefaults {
    /// Apply to a new base view.
    pub fn apply(&self, v: &mut DrawingView) {
        v.style = if self.hidden_lines { RenderStyle::HiddenLinesVisible } else { RenderStyle::HiddenLinesRemoved };
        v.show_tangent_edges = self.tangent != 0;
        v.tangent_phantom = self.tangent == 2;
        v.show_centerlines = self.auto_centermarks;
        v.auto_centerlines = Some(self.auto_centerlines);
        v.simplification = Some(self.simplification);
        v.scale_label = (!self.scale_label).then_some(false);
        v.hide_threads = !self.threads;
        v.hide_bend_lines = !self.bend_lines;
        v.hide_bend_notes = !self.bend_notes;
    }
}

fn inspection_text() -> f64 {
    2.5
}

fn inspection_border() -> f64 {
    1.2
}

/// Border shape of inspection items (Drawing properties › Inspection).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum InspectionShape {
    #[default]
    Circle,
    Oval,
    Rectangle,
    Triangle,
    Flag,
}

impl InspectionShape {
    pub const ALL: [InspectionShape; 5] = [InspectionShape::Circle, InspectionShape::Oval, InspectionShape::Rectangle, InspectionShape::Triangle, InspectionShape::Flag];
    pub fn name(self) -> &'static str {
        match self {
            InspectionShape::Circle => "Circle",
            InspectionShape::Oval => "Oval",
            InspectionShape::Rectangle => "Rectangle",
            InspectionShape::Triangle => "Triangle",
            InspectionShape::Flag => "Flag",
        }
    }
}

fn yes() -> bool {
    true
}

fn format_pen() -> f64 {
    0.15
}

fn is_zero_u8(x: &u8) -> bool {
    *x == 0
}

fn is_zero(x: &f64) -> bool {
    *x == 0.0
}

impl Default for DrawingProps {
    fn default() -> Self {
        Self { units: None, decimal_comma: false, precision: None, tolerance_precision: None, angular_precision: None, angular_tolerance_precision: None, leading_zeros: true, trailing_zeros: true, tolerance_leading_zeros: true, tolerance_trailing_zeros: true, fit_tolerance_precision: None, dual: DualDims::default(), template: None, locked: false, inspection_shape: InspectionShape::Circle, hide_inspection: false, inspection_exclude_callouts: false, inspection_row_parsing: false, inspection_text_height: 2.5, inspection_border_size: 1.2, inspection_border_chars: 2, inspection_color: None, inspection_solid_fill: true, inspection_text_color: None, inspection_fill_color: None,
            view_defaults: ViewDefaults::default(),
            dim_text_height: 3.5,
            dim_arrow: 3.0,
            dim_arrow_type: sheet::ArrowType::Filled,
            dim_color: None,
            ann_text_height: 3.5,
            ann_arrow: 3.0,
            ann_arrow_type: sheet::ArrowType::Filled,
            ann_color: None,
            image_borders: false,
            dim_text_gap: gap_text(),
            dim_geometry_gap: gap_geometry(),
            dim_extension: gap_extension(),
            dim_text_horizontal: false,
            table_h_padding: table_padding(),
            table_v_padding: table_padding(),
            formats_locked: false,
            border_color: None,
            zone_color: None,
            title_block_color: None,
            border_thickness: format_pen(),
            zone_thickness: format_pen(),
            title_block_thickness: format_pen(),
            ann_types: Vec::new(),
            datum_size: datum_size(),
            hide_placeholder: false,
            view_tab: Box::default(),
            table_styles: Box::default(),
            custom_colors: Vec::new(),
            weld_ansi: None,
            hide_property_background: false,
            chamfer_45_dimension: false,
            chamfer_45_content: ChamferContent::default(),
            chamfer_dimension: true,
            chamfer_content: ChamferContent::LengthXLength,
            chamfer_length_precision: None,
            chamfer_angle_precision: None,
            flag_shape: flag_shape(),
            callout_justify: TextJustify::Center,
            callout_stack: StackDirection::Auto,
            callout_stack_gap: stack_gap(),
            target_hatch: (0, 0),
            hole_indicator: hole_indicator(),
            fit_iso: false,
            dim_justify: TextJustify::Center,
            underline_overridden: true,
            break_dims: true,
            dim_fit_ansi: None,
            lock_jog_45: false,
            ordinate_ansi: None,
            radial_dim_lines: None,
            com_diameter: com_diameter(),
            dim_font: None,
            ann_font: None,
            // Live 1710: Inspection › Text with Bold pressed.
            inspection_font: Some(sheet::TextFont { family: sheet::DRAWING_DEFAULT_FONT.into(), bold: true, italic: false }),
        }
    }
}

impl Drawing {
    /// Weld symbols follow ANSI (else ISO): Drawing properties › Weld
    /// symbols › Standard, else the drawing's standard (its sheet size).
    pub fn weld_ansi(&self) -> bool {
        self.props.weld_ansi.unwrap_or_else(|| self.sheets.first().is_none_or(|s| s.size.is_ansi()))
    }

    /// The drawing's standard is ANSI (its first sheet's size), else ISO.
    /// A table's content row text height (mm): its own (Text height ▸) or
    /// Drawing properties › Tables' for its kind.
    pub fn table_text_height(&self, t: &Table) -> f64 {
        if t.own_text_height {
            return t.text_height;
        }
        self.props.table_styles.of(&t.kind).content.height.unwrap_or(self.props.ann_text_height)
    }

    pub fn is_ansi(&self) -> bool {
        self.sheets.first().is_none_or(|s| s.size.is_ansi())
    }

    /// Dimensions › Fit standard ANSI (else ISO).
    pub fn dim_fit_ansi(&self) -> bool {
        self.props.dim_fit_ansi.unwrap_or_else(|| self.is_ansi())
    }

    /// Ordinate dimensions › Dimension style ANSI (else ISO).
    pub fn ordinate_ansi(&self) -> bool {
        self.props.ordinate_ansi.unwrap_or_else(|| self.is_ansi())
    }

    /// Diameter and radial dimensions › Show dimension lines by default.
    pub fn radial_dim_lines(&self) -> bool {
        self.props.radial_dim_lines.unwrap_or_else(|| !self.is_ansi())
    }

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
        // After a DWT was picked: its continuation format (live z2 (4)).
        if let Some(t) = self.new_sheet_template().cloned() {
            sheet.apply_template_format(&t);
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

    /// Sheets panel › Paste of a copied sheet: appended at the end, named
    /// "<name> (1)", "(2)" … (live "Sheets panel with several sheets").
    /// Returns its index.
    pub fn paste_sheet(&mut self, mut sheet: Sheet) -> usize {
        // Live: "<name> (1)", then "(2)" for a second copy.
        let base = sheet.name.clone();
        sheet.name = (1..).map(|n| format!("{base} ({n})")).find(|n| !self.sheets.iter().any(|s| &s.name == n)).expect("unbounded");
        self.sheets.push(sheet);
        self.sheets.len() - 1
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

    /// Move sheets `rows` (kept in their order) to stand before sheet `to`
    /// (`len`: at the end), as a drag in the Sheets panel does. Returns the
    /// new order as old indices, or None when nothing moves.
    pub fn move_sheets(&mut self, rows: &[usize], to: usize) -> Option<Vec<usize>> {
        let n = self.sheets.len();
        let mut moved: Vec<usize> = rows.iter().copied().filter(|r| *r < n).collect();
        moved.sort_unstable();
        moved.dedup();
        if moved.is_empty() {
            return None;
        }
        let rest: Vec<usize> = (0..n).filter(|i| !moved.contains(i)).collect();
        let at = rest.iter().take_while(|i| **i < to.min(n)).count();
        let mut order = rest[..at].to_vec();
        order.extend(&moved);
        order.extend(&rest[at..]);
        if order.iter().copied().eq(0..n) {
            return None;
        }
        let mut old: Vec<Option<Sheet>> = std::mem::take(&mut self.sheets).into_iter().map(Some).collect();
        self.sheets = order.iter().map(|i| old[*i].take().expect("each sheet once")).collect();
        Some(order)
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

pub mod sheet;
pub mod note;
pub mod weld;
pub mod tb_lang;
pub mod sheet_sketch;
pub mod imported;
pub mod templates;
pub mod font;
pub mod shx;
pub mod hatch_pat;
pub mod format;
pub mod cad_objects;
pub mod cad_layers;
pub mod links;
pub mod mbd;
pub mod dwt;

/// Rotate a 2D vector counter-clockwise by `a` radians.
pub fn rotate2(v: [f64; 2], a: f64) -> [f64; 2] {
    if a == 0.0 {
        return v;
    }
    let (s, c) = a.sin_cos();
    [v[0] * c - v[1] * s, v[0] * s + v[1] * c]
}
pub mod sketch_dims;
pub mod inspection;
