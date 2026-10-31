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
    v.named_section.clear();
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

/// Rotate view ▸ Vertical to… / Horizontal to…: the rotation (from
/// `rotation`) that turns the view by the smallest angle so the straight
/// edge from `pa` to `pb` (sheet) is vertical / horizontal (lines have no
/// direction: mod 180°). Apply it with [`set_view_rotation`].
pub fn rotation_to_line(rotation: f64, pa: [f64; 2], pb: [f64; 2], vertical: bool) -> f64 {
    let angle = (pb[1] - pa[1]).atan2(pb[0] - pa[0]);
    let target = if vertical { std::f64::consts::FRAC_PI_2 } else { 0.0 };
    let pi = std::f64::consts::PI;
    let mut delta = (target - angle).rem_euclid(pi);
    if delta > pi / 2.0 {
        delta -= pi;
    }
    rotation + delta
}

/// Set view `id`'s rotation (radians). Its edge dimensions turn with it,
/// as live (behaviours "V44: Rotate view › Vertical to… with dimensions",
/// qa DWG-ROTATE-VIEW-DIMS-ZERO): a horizontal / vertical linear dimension
/// whose points lie along its measured axis keeps its value and side,
/// swapping orientation on a quarter turn (aligned for other angles);
/// others keep their sheet orientation and re-measure.
pub fn set_view_rotation(s: &mut Sheet, id: ViewId, rotation: f64) {
    let Some(v) = s.view_mut(id) else { return };
    let old = v.rotation;
    v.rotation = rotation;
    let delta = rotation - old;
    if delta.abs() < 1e-12 {
        return;
    }
    let half_pi = std::f64::consts::FRAC_PI_2;
    let quarters = (delta / half_pi).round();
    let square = (delta - quarters * half_pi).abs() < 1e-6;
    let turn = |x: [f64; 2], a: f64| [x[0] * a.cos() - x[1] * a.sin(), x[0] * a.sin() + x[1] * a.cos()];
    for a in &mut s.annotations {
        let AnnotationKind::LinearDimension { view, a: pa, b: pb, offset, orientation, .. } = &mut a.kind else { continue };
        if *view != id || *orientation == DimOrientation::Aligned {
            continue;
        }
        let (dir, nrm) = if *orientation == DimOrientation::Horizontal { ([1.0, 0.0], [0.0, 1.0]) } else { ([0.0, 1.0], [1.0, 0.0]) };
        // The measured segment on the sheet before the turn (view → sheet
        // turns by the old rotation; the scale doesn't matter here).
        let d = turn([pb[0] - pa[0], pb[1] - pa[1]], old);
        let l = (d[0] * d[0] + d[1] * d[1]).sqrt();
        if l < 1e-12 || (d[0] * dir[1] - d[1] * dir[0]).abs() > l * 1e-6 {
            continue;
        }
        let new_d = turn(d, delta);
        let side = turn(nrm, delta);
        let (o, n) = if !square {
            let u = [new_d[0] / l, new_d[1] / l];
            (DimOrientation::Aligned, [-u[1], u[0]])
        } else if quarters.rem_euclid(2.0) == 1.0 {
            if *orientation == DimOrientation::Horizontal { (DimOrientation::Vertical, [1.0, 0.0]) } else { (DimOrientation::Horizontal, [0.0, 1.0]) }
        } else {
            (*orientation, nrm)
        };
        *orientation = o;
        // The same side of the edge as before.
        if side[0] * n[0] + side[1] * n[1] < 0.0 {
            *offset = -*offset;
        }
    }
}

/// Adjust linestyle's Linestyle, Dashes, Thickness and color.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EdgeLook {
    pub pattern: LinePattern,
    pub dashes: f64,
    pub thickness: f64,
    pub color: Option<[u8; 3]>,
}

/// Adjust linestyle ✓: the picked model edges of view `v` get `look`
/// (None: Revert to drawing properties).
pub fn set_edge_styles(v: &mut DrawingView, edges: &[(noshape_model::BodyId, noshape_model::reference::TopoName)], look: Option<EdgeLook>) {
    for (body, name) in edges {
        v.edge_styles.retain(|s| !(s.body == *body && s.name == *name));
        if let Some(l) = look {
            v.edge_styles.push(EdgeStyle { body: body.clone(), name: name.clone(), pattern: l.pattern, dashes: l.dashes, thickness: l.thickness, color: l.color });
        }
    }
}

/// Reference › Replace… (sheets panel, view row): on every sheet, the base
/// views of `old` showing `old_part` show `new` / `new_part` instead (one
/// drawing edit "Replace"; help H:sheets).
pub fn replace_reference(d: &mut Drawing, old: noshape_model::document::TabId, old_part: &Option<noshape_model::BodyId>, new: noshape_model::document::TabId, new_part: Option<noshape_model::BodyId>) {
    for v in d.sheets.iter_mut().flat_map(|s| s.views.iter_mut()) {
        if let ViewKind::Base { source: src, part: p, .. } = &mut v.kind
            && *src == old
            && p == old_part
        {
            *src = new;
            *p = new_part.clone();
        }
    }
}

/// Reference › Change configuration… (sheets panel): on every sheet, the
/// base views of `tab` showing `part` show configuration `config`, with
/// the views derived from them (one drawing edit "Change configuration";
/// live behaviours S6: every view of the reference changes, from either
/// tab of the panel). Returns how many base views changed.
pub fn change_configuration(d: &mut Drawing, tab: noshape_model::document::TabId, part: &Option<noshape_model::BodyId>, config: &noshape_model::configuration::ConfigValues) -> usize {
    let mut n = 0;
    for v in d.sheets.iter_mut().flat_map(|s| s.views.iter_mut()) {
        if let ViewKind::Base { source, part: p, config: c, .. } = &mut v.kind
            && *source == tab
            && p == part
        {
            *c = config.clone();
            n += 1;
        }
    }
    n
}

/// BOM table › Replace: every base view of `old` on the sheet shows `new`
/// (one sheet edit "Replace").
pub fn replace_sheet_source(s: &mut Sheet, old: noshape_model::document::TabId, new: noshape_model::document::TabId) {
    for v in s.views.iter_mut() {
        if let ViewKind::Base { source: src, .. } = &mut v.kind
            && *src == old
        {
            *src = new;
        }
    }
}
