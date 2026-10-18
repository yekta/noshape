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
            | AnnotationKind::Balloon { view, .. } => Some(*view),
            _ => None,
        }
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
            ViewKind::Projected { parent, .. } | ViewKind::Auxiliary { parent, .. } | ViewKind::Section { parent, .. } | ViewKind::Detail { parent, .. } => Some(*parent),
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

    /// Remove a view, its descendants and every annotation attached to them.
    pub fn remove_view(&mut self, id: ViewId) {
        let gone = self.descendants(id);
        self.views.retain(|v| !gone.contains(&v.id));
        self.annotations.retain(|a| a.kind.view().is_none_or(|v| !gone.contains(&v)));
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
                ViewKind::Section { label, .. } | ViewKind::Detail { label, .. } => Some(label.clone()),
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
        Self { name: "Sheet1".into(), size: SheetSize::A3, scale: 1.0, views: vec![], annotations: vec![], title_block: TitleBlock::default() }
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

pub mod build;
pub mod export;
pub mod font;
pub mod sheet;
pub mod views;

#[cfg(test)]
mod tests;
