//! The drawing view menu's and View properties dialog's edits of a view,
//! shared by the drawing editor and the agent API.

use noshape_model::drawing::views;
use noshape_model::drawing::*;

/// Tangent edges display (Onshape: Hidden / Solid / Phantom).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TangentMode {
    Hidden,
    Solid,
    Phantom,
}

impl TangentMode {
    pub fn of(v: &DrawingView) -> Self {
        match (v.show_tangent_edges, v.tangent_phantom) {
            (false, _) => TangentMode::Hidden,
            (true, false) => TangentMode::Solid,
            (true, true) => TangentMode::Phantom,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            TangentMode::Hidden => "Hidden",
            TangentMode::Solid => "Solid",
            TangentMode::Phantom => "Phantom",
        }
    }
    pub fn apply(self, v: &mut DrawingView) {
        v.show_tangent_edges = self != TangentMode::Hidden;
        v.tangent_phantom = self == TangentMode::Phantom;
    }
}

pub const ORIENTATIONS: [(ViewOrientation, &str); 7] = [
    (ViewOrientation::Top, "Top"),
    (ViewOrientation::Left, "Left"),
    (ViewOrientation::Right, "Right"),
    (ViewOrientation::Front, "Front"),
    (ViewOrientation::Back, "Back"),
    (ViewOrientation::Bottom, "Bottom"),
    (ViewOrientation::Isometric, "Isometric"),
];

pub fn orientation_name(o: ViewOrientation) -> &'static str {
    ORIENTATIONS.iter().find(|(x, _)| *x == o).map(|(_, n)| *n).unwrap_or("Front")
}

/// A base view back on a standard orientation (no named view; a sketch
/// view keeps facing its sketch).
pub fn set_standard_orientation(v: &mut DrawingView, o: ViewOrientation) {
    if let ViewKind::Base { orientation, .. } = &mut v.kind {
        *orientation = o;
    }
    if v.named_view.take().is_some() || v.sketch.is_none() {
        v.custom_frame = None;
    }
}

/// Default view name: the orientation of a base view or the standard
/// orientation a projected view shows ("Right"), else the view type.
pub fn default_view_name(drawing: &Drawing, sheet: &Sheet, v: &DrawingView) -> String {
    match &v.kind {
        ViewKind::Base { orientation, .. } => v.named_view.clone().unwrap_or_else(|| orientation_name(*orientation).to_string()),
        ViewKind::Projected { .. } => views::standard_orientation(drawing, sheet, v.id).map(|o| orientation_name(o).to_string()).unwrap_or_else(|| "Projected view".into()),
        ViewKind::Auxiliary { .. } => "Auxiliary view".into(),
        ViewKind::Section { label, .. } | ViewKind::AlignedSection { label, .. } => format!("Section {label}-{label}"),
        ViewKind::Detail { label, .. } => format!("Detail {label}"),
    }
}

/// Order ▸ Bring to front / Send to back (the view list is the layer stack).
pub fn order_view(s: &mut Sheet, id: ViewId, front: bool) {
    if let Some(i) = s.views.iter().position(|v| v.id == id) {
        let v = s.views.remove(i);
        if front {
            s.views.push(v);
        } else {
            s.views.insert(0, v);
        }
    }
}

/// Align views ▸ Vertically / Horizontally: the view's center lines up with
/// the clicked view's center `at` (vertically: the same x).
pub fn align_view(v: &mut DrawingView, at: [f64; 2], vertical: bool) {
    if vertical {
        v.position[0] = at[0];
    } else {
        v.position[1] = at[1];
    }
}

/// Rotate view ▸ Vertical to… / Horizontal to…: turn the view by the
/// smallest angle so the straight edge from `pa` to `pb` (sheet) is vertical
/// / horizontal (lines have no direction: mod 180°).
pub fn rotate_to_line(v: &mut DrawingView, pa: [f64; 2], pb: [f64; 2], vertical: bool) {
    let angle = (pb[1] - pa[1]).atan2(pb[0] - pa[0]);
    let target = if vertical { std::f64::consts::FRAC_PI_2 } else { 0.0 };
    let pi = std::f64::consts::PI;
    let mut delta = (target - angle).rem_euclid(pi);
    if delta > pi / 2.0 {
        delta -= pi;
    }
    v.rotation += delta;
}
