//! Feature parameter schema and values.
//!
//! Mirrors how Onshape's FeatureScript features are defined: every feature
//! declares an ordered list of typed parameters ([`ParamSpec`]) with defaults,
//! bounds, live visibility predicates and UI hints. Feature instances store a
//! generic [`ParamValues`] map, so one dialog renderer, one serializer and one
//! configuration system serve every feature.

use crate::reference::Reference;
use indexmap::IndexMap;
use noshape_units::{Dim, ExprValue, Quantity, UnitPrefs};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// A stored parameter value.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "t", content = "v")]
pub enum ParamValue {
    Bool(bool),
    /// Enum option id (e.g. "BLIND").
    Enum(String),
    /// Length / angle / real / integer value as an expression.
    Expr(ExprValue),
    Text(String),
    /// Ordered entity selection.
    Query(Vec<Reference>),
    /// Array parameter: one value map per item.
    Array(Vec<ParamValues>),
    /// Sketch payload (only used by the Sketch feature).
    Sketch(Box<crate::sketch_feature::SketchData>),
    /// Lookup table path (e.g. hole standard cascade), one entry per level.
    Lookup(Vec<String>),
}

/// Parameter values of a feature instance, keyed by parameter id.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ParamValues(pub IndexMap<String, ParamValue>);

impl ParamValues {
    pub fn get(&self, id: &str) -> Option<&ParamValue> {
        self.0.get(id)
    }
    pub fn set(&mut self, id: &str, v: ParamValue) {
        self.0.insert(id.to_string(), v);
    }
    pub fn bool(&self, id: &str) -> bool {
        matches!(self.0.get(id), Some(ParamValue::Bool(true)))
    }
    pub fn enum_(&self, id: &str) -> &str {
        match self.0.get(id) {
            Some(ParamValue::Enum(s)) => s,
            _ => "",
        }
    }
    pub fn is(&self, id: &str, option: &str) -> bool {
        self.enum_(id) == option
    }
    pub fn expr(&self, id: &str) -> Option<&ExprValue> {
        match self.0.get(id) {
            Some(ParamValue::Expr(e)) => Some(e),
            _ => None,
        }
    }
    pub fn text(&self, id: &str) -> &str {
        match self.0.get(id) {
            Some(ParamValue::Text(s)) => s,
            _ => "",
        }
    }
    pub fn query(&self, id: &str) -> &[Reference] {
        match self.0.get(id) {
            Some(ParamValue::Query(q)) => q,
            _ => &[],
        }
    }
    pub fn query_mut(&mut self, id: &str) -> &mut Vec<Reference> {
        let e = self.0.entry(id.to_string()).or_insert_with(|| ParamValue::Query(Vec::new()));
        if !matches!(e, ParamValue::Query(_)) {
            *e = ParamValue::Query(Vec::new());
        }
        match e {
            ParamValue::Query(q) => q,
            _ => unreachable!(),
        }
    }
    pub fn array(&self, id: &str) -> &[ParamValues] {
        match self.0.get(id) {
            Some(ParamValue::Array(a)) => a,
            _ => &[],
        }
    }
    pub fn lookup(&self, id: &str) -> &[String] {
        match self.0.get(id) {
            Some(ParamValue::Lookup(a)) => a,
            _ => &[],
        }
    }
}

/// What kind of quantity a numeric parameter holds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NumKind {
    Length,
    Angle,
    Real,
    Integer,
}

impl NumKind {
    pub fn dim(self) -> Dim {
        match self {
            NumKind::Length => Dim::LENGTH,
            NumKind::Angle => Dim::ANGLE,
            NumKind::Real | NumKind::Integer => Dim::UNITLESS,
        }
    }
}

/// Default and bounds for a numeric parameter. Lengths in meters, angles in
/// radians. Onshape picks per-unit-system defaults (25 mm vs 1 in) rather than
/// converting, so both are recorded.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NumSpec {
    pub kind: NumKind,
    pub default_metric: f64,
    pub default_imperial: f64,
    pub min: f64,
    pub max: f64,
}

impl NumSpec {
    pub const fn length(default_mm: f64, default_in: f64) -> Self {
        Self { kind: NumKind::Length, default_metric: default_mm * 1e-3, default_imperial: default_in * 0.0254, min: -500.0, max: 500.0 }
    }
    pub const fn positive_length(default_mm: f64, default_in: f64) -> Self {
        Self { kind: NumKind::Length, default_metric: default_mm * 1e-3, default_imperial: default_in * 0.0254, min: 0.0, max: 500.0 }
    }
    pub const fn angle(default_deg: f64, min_deg: f64, max_deg: f64) -> Self {
        let r = std::f64::consts::PI / 180.0;
        Self { kind: NumKind::Angle, default_metric: default_deg * r, default_imperial: default_deg * r, min: min_deg * r, max: max_deg * r }
    }
    pub const fn integer(default: i64, min: i64, max: i64) -> Self {
        Self { kind: NumKind::Integer, default_metric: default as f64, default_imperial: default as f64, min: min as f64, max: max as f64 }
    }
    pub const fn real(default: f64, min: f64, max: f64) -> Self {
        Self { kind: NumKind::Real, default_metric: default, default_imperial: default, min, max }
    }
    pub const fn bounded(mut self, min: f64, max: f64) -> Self {
        self.min = min;
        self.max = max;
        self
    }
}

/// Which entities a query parameter accepts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct QueryFilter(pub u32);

impl QueryFilter {
    pub const FACE: Self = Self(1);
    pub const PLANAR_FACE: Self = Self(1 << 1);
    pub const EDGE: Self = Self(1 << 2);
    pub const LINEAR_EDGE: Self = Self(1 << 3);
    pub const CIRCULAR_EDGE: Self = Self(1 << 4);
    pub const VERTEX: Self = Self(1 << 5);
    pub const SOLID_BODY: Self = Self(1 << 6);
    pub const SHEET_BODY: Self = Self(1 << 7);
    pub const WIRE_BODY: Self = Self(1 << 8);
    pub const SKETCH_REGION: Self = Self(1 << 9);
    pub const SKETCH_EDGE: Self = Self(1 << 10);
    pub const SKETCH_POINT: Self = Self(1 << 11);
    pub const PLANE: Self = Self(1 << 12);
    pub const MATE_CONNECTOR: Self = Self(1 << 13);
    pub const FEATURE: Self = Self(1 << 14);
    pub const CYLINDRICAL_FACE: Self = Self(1 << 15);
    pub const SKETCH: Self = Self(1 << 16);
    pub const POINT_BODY: Self = Self(1 << 17);
    /// Any axis-like entity: linear edges, sketch lines, cylinder/cone axes, mate connector Z.
    pub const AXIS: Self = Self(Self::LINEAR_EDGE.0 | Self::CYLINDRICAL_FACE.0 | Self::MATE_CONNECTOR.0 | Self::SKETCH_EDGE.0);
    /// Any direction-defining entity: axes plus planar faces and planes (normal).
    pub const DIRECTION: Self = Self(Self::AXIS.0 | Self::PLANAR_FACE.0 | Self::PLANE.0);
    pub const PLANE_LIKE: Self = Self(Self::PLANAR_FACE.0 | Self::PLANE.0 | Self::MATE_CONNECTOR.0);
    pub const POINT_LIKE: Self = Self(Self::VERTEX.0 | Self::SKETCH_POINT.0 | Self::MATE_CONNECTOR.0 | Self::POINT_BODY.0);
    pub const BODY: Self = Self(Self::SOLID_BODY.0 | Self::SHEET_BODY.0);
    pub const ANY_EDGE: Self = Self(Self::EDGE.0 | Self::SKETCH_EDGE.0);
    pub const ANY_FACE: Self = Self(Self::FACE.0 | Self::PLANAR_FACE.0 | Self::CYLINDRICAL_FACE.0);

    pub const fn or(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
    pub fn intersects(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }
}

impl std::ops::BitOr for QueryFilter {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

/// UI hints (subset of FeatureScript's UIHint).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Hints(pub u32);

impl Hints {
    pub const NONE: Self = Self(0);
    pub const HORIZONTAL_ENUM: Self = Self(1);
    pub const SHOW_LABEL: Self = Self(1 << 1);
    pub const REMEMBER_PREVIOUS_VALUE: Self = Self(1 << 2);
    /// Boolean shown as a flip-arrow at the end of the previous row.
    pub const OPPOSITE_DIRECTION: Self = Self(1 << 3);
    pub const OPPOSITE_DIRECTION_CIRCULAR: Self = Self(1 << 4);
    /// Checkbox and the following value field share one row.
    pub const DISPLAY_SHORT: Self = Self(1 << 5);
    pub const FIRST_IN_ROW: Self = Self(1 << 6);
    pub const READ_ONLY: Self = Self(1 << 7);
    pub const UNCONFIGURABLE: Self = Self(1 << 8);
    pub const ALLOW_QUERY_ORDER: Self = Self(1 << 9);
    pub const INITIAL_FOCUS: Self = Self(1 << 10);
    /// The selection field that takes the focus when an existing feature is
    /// opened for editing (FeatureScript `INITIAL_FOCUS_ON_EDIT`).
    pub const INITIAL_FOCUS_ON_EDIT: Self = Self(1 << 23);
    pub const COLLAPSE_ARRAY_ITEMS: Self = Self(1 << 11);
    pub const MATCH_LAST_ARRAY_ITEM: Self = Self(1 << 12);
    /// Boolean acts as a collapsible group header for the following params.
    pub const GROUP_HEADER: Self = Self(1 << 13);
    pub const CAN_BE_TOLERANT: Self = Self(1 << 14);
    pub const ALWAYS_HIDDEN: Self = Self(1 << 15);
    /// Array shown as one selection field ("Vertices"): picking adds an item
    /// for the entity (its first query param), removing a pick removes the
    /// item; the items' other params follow (Onshape's "Driven query").
    pub const DRIVEN_QUERY: Self = Self(1 << 16);
    /// With GROUP_HEADER on a boolean: a plain collapsible group title
    /// ("› Axis measurements"), no checkbox; the value is "unfolded".
    pub const GROUP_LABEL: Self = Self(1 << 17);
    /// A selection field that accepts mate connectors without offering the
    /// "Select mate connector" side button (Linear pattern's Direction).
    pub const NO_MATE_CONNECTOR_BUTTON: Self = Self(1 << 18);
    /// A tolerant diameter (FeatureScript `CAN_BE_TOLERANT_DIAMETER`): its
    /// tolerance types add Fit, Fit with tolerance and Fit (tolerance only).
    /// Implies CAN_BE_TOLERANT.
    pub const CAN_BE_TOLERANT_DIAMETER: Self = Self(1 << 19 | 1 << 14);
    /// A query field with the "Create selection" side button (FeatureScript
    /// `SHOW_CREATE_SELECTION`).
    pub const SHOW_CREATE_SELECTION: Self = Self(1 << 20);
    /// FeatureScript `UIHint.PRIMARY_AXIS`: a boolean shown as a 26 px flip
    /// button at the row's start (Mate connector's Flip primary axis).
    pub const PRIMARY_AXIS: Self = Self(1 << 21);
    /// FeatureScript `UIHint.MATE_CONNECTOR_AXIS_TYPE`: an enum shown as one
    /// icon button that steps through the options ("Reorient secondary axis").
    pub const MATE_CONNECTOR_AXIS_TYPE: Self = Self(1 << 22);

    pub const fn or(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
    pub fn has(self, h: Self) -> bool {
        self.0 & h.0 == h.0 && h.0 != 0
    }
}

impl std::ops::BitOr for Hints {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

pub type VisibleFn = Arc<dyn Fn(&ParamValues) -> bool + Send + Sync>;

#[derive(Clone)]
pub enum ParamKind {
    Bool { default: bool },
    Enum { options: &'static [(&'static str, &'static str)], default: &'static str },
    Num(NumSpec),
    Text { default: &'static str },
    Query { filter: QueryFilter, max_picks: Option<usize> },
    Array { item: Arc<Vec<ParamSpec>>, item_label: &'static str },
    /// A lookup-table cascade (hole standards etc.).
    Lookup { table: &'static str },
    /// Sketch payload; only for the Sketch feature.
    Sketch,
    /// A reference to another tab (FeatureScript `TableData`, `ImageData`,
    /// `PartStudioData`, ...): the tab id as text, empty when unset.
    Reference { kind: ReferenceKind },
}

/// What a reference parameter selects.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReferenceKind {
    /// A CSV file tab (`TableData`).
    Csv,
    /// An image file tab (`ImageData`).
    Image,
    /// A JSON file tab (`JSONData`).
    Json,
    /// A text file tab (`TextData`).
    Text,
    /// A Part Studio (`PartStudioData`).
    PartStudio,
    /// A Variable Studio (`VariableStudioData`).
    VariableStudio,
    /// An imported CAD file tab (`CADImportData`).
    CadImport,
}

#[derive(Clone)]
pub struct ParamSpec {
    pub id: &'static str,
    pub label: &'static str,
    pub kind: ParamKind,
    pub hints: Hints,
    pub visible: Option<VisibleFn>,
    pub description: &'static str,
    /// An array's item label (FeatureScript "Item label template"):
    /// `#param` stands for an item parameter's value, `#index` for its
    /// 1-based position ("[#continuity] #entities").
    pub item_template: Option<&'static str>,
}

impl ParamSpec {
    fn new(id: &'static str, label: &'static str, kind: ParamKind) -> Self {
        Self { id, label, kind, hints: Hints::NONE, visible: None, description: "", item_template: None }
    }
    pub fn item_template(mut self, t: &'static str) -> Self {
        self.item_template = Some(t);
        self
    }
    pub fn bool(id: &'static str, label: &'static str, default: bool) -> Self {
        Self::new(id, label, ParamKind::Bool { default })
    }
    pub fn flip(id: &'static str, label: &'static str) -> Self {
        Self::new(id, label, ParamKind::Bool { default: false }).hints(Hints::OPPOSITE_DIRECTION)
    }
    pub fn enum_(id: &'static str, label: &'static str, options: &'static [(&'static str, &'static str)]) -> Self {
        Self::new(id, label, ParamKind::Enum { options, default: options[0].0 })
    }
    pub fn num(id: &'static str, label: &'static str, spec: NumSpec) -> Self {
        Self::new(id, label, ParamKind::Num(spec))
    }
    pub fn length(id: &'static str, label: &'static str, default_mm: f64, default_in: f64) -> Self {
        Self::num(id, label, NumSpec::length(default_mm, default_in))
    }
    pub fn angle(id: &'static str, label: &'static str, default_deg: f64) -> Self {
        Self::num(id, label, NumSpec::angle(default_deg, -360.0, 360.0))
    }
    pub fn text(id: &'static str, label: &'static str, default: &'static str) -> Self {
        Self::new(id, label, ParamKind::Text { default })
    }
    pub fn query(id: &'static str, label: &'static str, filter: QueryFilter) -> Self {
        Self::new(id, label, ParamKind::Query { filter, max_picks: None })
    }
    pub fn query1(id: &'static str, label: &'static str, filter: QueryFilter) -> Self {
        Self::new(id, label, ParamKind::Query { filter, max_picks: Some(1) })
    }
    pub fn array(id: &'static str, label: &'static str, item_label: &'static str, item: Vec<ParamSpec>) -> Self {
        Self::new(id, label, ParamKind::Array { item: Arc::new(item), item_label })
    }
    pub fn lookup(id: &'static str, label: &'static str, table: &'static str) -> Self {
        Self::new(id, label, ParamKind::Lookup { table })
    }
    pub fn reference(id: &'static str, label: &'static str, kind: ReferenceKind) -> Self {
        Self::new(id, label, ParamKind::Reference { kind })
    }
    pub fn sketch(id: &'static str) -> Self {
        Self::new(id, "Sketch", ParamKind::Sketch).hints(Hints::ALWAYS_HIDDEN)
    }

    pub fn default_enum(mut self, option: &'static str) -> Self {
        if let ParamKind::Enum { default, .. } = &mut self.kind {
            *default = option;
        }
        self
    }
    pub fn hints(mut self, h: Hints) -> Self {
        self.hints = self.hints | h;
        self
    }
    pub fn visible(mut self, f: impl Fn(&ParamValues) -> bool + Send + Sync + 'static) -> Self {
        self.visible = Some(Arc::new(f));
        self
    }
    pub fn describe(mut self, d: &'static str) -> Self {
        self.description = d;
        self
    }

    pub fn is_visible(&self, values: &ParamValues) -> bool {
        !self.hints.has(Hints::ALWAYS_HIDDEN) && self.visible.as_ref().is_none_or(|f| f(values))
    }

    /// Default value for this parameter given the document's unit preferences.
    pub fn default_value(&self, units: &UnitPrefs) -> ParamValue {
        match &self.kind {
            ParamKind::Bool { default } => ParamValue::Bool(*default),
            ParamKind::Enum { default, .. } => ParamValue::Enum((*default).to_string()),
            ParamKind::Num(n) => ParamValue::Expr(default_expr(n, units)),
            ParamKind::Text { default } => ParamValue::Text((*default).to_string()),
            ParamKind::Query { .. } => ParamValue::Query(Vec::new()),
            ParamKind::Array { .. } => ParamValue::Array(Vec::new()),
            ParamKind::Lookup { .. } => ParamValue::Lookup(Vec::new()),
            ParamKind::Sketch => ParamValue::Sketch(Box::default()),
            ParamKind::Reference { .. } => ParamValue::Text(String::new()),
        }
    }
}

/// Default expression text for a numeric spec ("25 mm", "1 in", "3 deg").
pub fn default_expr(n: &NumSpec, units: &UnitPrefs) -> ExprValue {
    use noshape_units::LengthUnit;
    let imperial = matches!(units.length, LengthUnit::Inch | LengthUnit::Foot | LengthUnit::Yard);
    let si = if imperial { n.default_imperial } else { n.default_metric };
    let q = match n.kind {
        NumKind::Length => Quantity::new(si, Dim::LENGTH),
        NumKind::Angle => Quantity::new(si, Dim::ANGLE),
        NumKind::Real | NumKind::Integer => Quantity::new(si, Dim::UNITLESS),
    };
    ExprValue::from_quantity(q, units)
}

/// Fill in any missing parameter values with defaults (recursively for arrays).
pub fn fill_defaults(specs: &[ParamSpec], values: &mut ParamValues, units: &UnitPrefs) {
    for s in specs {
        if !values.0.contains_key(s.id) {
            values.0.insert(s.id.to_string(), s.default_value(units));
        }
        if let (ParamKind::Array { item, .. }, Some(ParamValue::Array(items))) = (&s.kind, values.0.get_mut(s.id)) {
            for it in items.iter_mut() {
                fill_defaults(item, it, units);
            }
        }
    }
}
