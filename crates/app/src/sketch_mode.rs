//! Sketch editing: drawing tools with inference snapping and automatic
//! constraints, live drag solving, smart dimensions and constraint tools.
//!
//! The editor operates on the [`SketchData`] of the sketch feature being
//! edited (owned by the feature dialog's draft). Every edit re-solves the
//! sketch immediately; the Part Studio view then regenerates the display.

use glam::{DVec2, DVec3};
use noshape_model::display::PlaneFrame;
use noshape_model::reference::FeatureId;
use noshape_model::sketch_feature::SketchData;
use noshape_render::{LineData, LineSegment, PointData, PointSprite};
use noshape_sketch::{AutoConstraint, ConstraintId, ConstraintKind, Curve2, DistanceMode, DragHandle, EntityId, EntityKind, Inference, InferenceContext, InferenceKind, Sketch, SketchTool};
use noshape_units::{Dim, ExprValue, Quantity, UnitPrefs};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConstraintTool {
    Coincident,
    Concentric,
    Parallel,
    Tangent,
    Horizontal,
    Vertical,
    Midpoint,
    Perpendicular,
    Equal,
    Symmetric,
    Fix,
    Normal,
    Pierce,
    Curvature,
}

impl ConstraintTool {
    pub const ALL: [ConstraintTool; 14] = [
        ConstraintTool::Coincident,
        ConstraintTool::Concentric,
        ConstraintTool::Parallel,
        ConstraintTool::Tangent,
        ConstraintTool::Horizontal,
        ConstraintTool::Vertical,
        ConstraintTool::Midpoint,
        ConstraintTool::Normal,
        ConstraintTool::Pierce,
        ConstraintTool::Perpendicular,
        ConstraintTool::Equal,
        ConstraintTool::Symmetric,
        ConstraintTool::Fix,
        ConstraintTool::Curvature,
    ];
    pub fn label(self) -> &'static str {
        match self {
            ConstraintTool::Coincident => "Coincident (I)",
            ConstraintTool::Concentric => "Concentric (Shift+O)",
            ConstraintTool::Parallel => "Parallel (B)",
            ConstraintTool::Tangent => "Tangent (T)",
            ConstraintTool::Horizontal => "Horizontal (H)",
            ConstraintTool::Vertical => "Vertical (V)",
            ConstraintTool::Midpoint => "Midpoint (Shift+M)",
            ConstraintTool::Perpendicular => "Perpendicular (Shift+L)",
            ConstraintTool::Equal => "Equal (E)",
            ConstraintTool::Symmetric => "Symmetric (Shift+Q)",
            ConstraintTool::Fix => "Fix (Shift+J)",
            ConstraintTool::Normal => "Normal (Shift+K)",
            ConstraintTool::Pierce => "Pierce (Shift+G)",
            ConstraintTool::Curvature => "Curvature (Shift+U)",
        }
    }
    pub fn icon(self) -> &'static str {
        match self {
            ConstraintTool::Coincident => "coincident",
            ConstraintTool::Concentric => "concentric",
            ConstraintTool::Parallel => "parallel",
            ConstraintTool::Tangent => "tangent",
            ConstraintTool::Horizontal => "horizontal",
            ConstraintTool::Vertical => "vertical",
            ConstraintTool::Midpoint => "midpoint",
            ConstraintTool::Perpendicular => "perpendicular",
            ConstraintTool::Equal => "equal",
            ConstraintTool::Symmetric => "symmetric",
            ConstraintTool::Fix => "fix",
            ConstraintTool::Normal => "normal",
            ConstraintTool::Pierce => "pierce",
            ConstraintTool::Curvature => "curvature",
        }
    }
    /// Entities needed before the constraint can be created.
    fn arity(self) -> usize {
        match self {
            ConstraintTool::Horizontal | ConstraintTool::Vertical | ConstraintTool::Fix => 1,
            ConstraintTool::Symmetric => 3,
            _ => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    Select,
    Line,
    CornerRect,
    CenterRect,
    Circle,
    ThreePointCircle,
    Arc3,
    TangentArc,
    CenterArc,
    Point,
    Spline,
    Slot,
    Polygon,
    Ellipse,
    Fillet,
    Trim,
    Extend,
    Split,
    Offset,
    Mirror,
    Use,
    Dimension,
    Constraint(ConstraintTool),
}

impl Tool {
    pub fn label(self) -> &'static str {
        match self {
            Tool::Select => "Select",
            Tool::Line => "Line (L)",
            Tool::CornerRect => "Corner rectangle (G)",
            Tool::CenterRect => "Center rectangle (R)",
            Tool::Circle => "Center point circle (C)",
            Tool::ThreePointCircle => "3 point circle",
            Tool::Arc3 => "3 point arc (A)",
            Tool::TangentArc => "Tangent arc",
            Tool::CenterArc => "Center point arc",
            Tool::Point => "Point (Shift+S)",
            Tool::Spline => "Spline",
            Tool::Slot => "Slot",
            Tool::Polygon => "Polygon",
            Tool::Ellipse => "Ellipse",
            Tool::Fillet => "Sketch fillet (Shift+F)",
            Tool::Trim => "Trim (M)",
            Tool::Extend => "Extend (X)",
            Tool::Split => "Split",
            Tool::Offset => "Offset (O)",
            Tool::Mirror => "Mirror",
            Tool::Use => "Use / project edges (U)",
            Tool::Dimension => "Dimension (D)",
            Tool::Constraint(c) => c.label(),
        }
    }
    pub fn icon(self) -> &'static str {
        match self {
            Tool::Select => "select-box",
            Tool::Line => "line",
            Tool::CornerRect => "corner-rectangle",
            Tool::CenterRect => "center-rectangle",
            Tool::Circle => "circle",
            Tool::ThreePointCircle => "three-point-circle",
            Tool::Arc3 => "three-point-arc",
            Tool::TangentArc => "tangent-arc",
            Tool::CenterArc => "center-point-arc",
            Tool::Point => "point-sketch",
            Tool::Spline => "spline",
            Tool::Slot => "slot",
            Tool::Polygon => "polygon-inscribed",
            Tool::Ellipse => "ellipse",
            Tool::Fillet => "sketch-fillet",
            Tool::Trim => "trim",
            Tool::Extend => "extend",
            Tool::Split => "split-sketch",
            Tool::Offset => "offset",
            Tool::Mirror => "sketch-mirror",
            Tool::Use => "use",
            Tool::Dimension => "dimension",
            Tool::Constraint(c) => c.icon(),
        }
    }
    fn inference_tool(self) -> SketchTool {
        match self {
            Tool::Select => SketchTool::Select,
            Tool::Point => SketchTool::Point,
            Tool::Line => SketchTool::Line,
            Tool::CornerRect | Tool::CenterRect => SketchTool::Rectangle,
            Tool::Circle | Tool::ThreePointCircle => SketchTool::Circle,
            Tool::Arc3 | Tool::CenterArc => SketchTool::Arc,
            Tool::TangentArc => SketchTool::TangentArc,
            Tool::Spline => SketchTool::Spline,
            _ => SketchTool::Other,
        }
    }
    /// Tools that place points by clicking in empty space.
    pub fn draws(self) -> bool {
        matches!(
            self,
            Tool::Line
                | Tool::CornerRect
                | Tool::CenterRect
                | Tool::Circle
                | Tool::ThreePointCircle
                | Tool::Arc3
                | Tool::TangentArc
                | Tool::CenterArc
                | Tool::Point
                | Tool::Spline
                | Tool::Slot
                | Tool::Polygon
                | Tool::Ellipse
        )
    }
}

#[derive(Clone, Debug)]
struct Click {
    pos: DVec2,
    inference: Option<Inference>,
}

#[derive(Clone, Debug)]
enum DragState {
    /// Pressed on an entity; becomes a geometry drag when moved.
    Pending { entity: EntityId, start: DVec2, additive: bool },
    Geometry { handle: DragHandle },
    /// Moving a dimension label.
    Label { constraint: ConstraintId },
    Box { start: DVec2 },
}

/// Pending inline dimension edit.
#[derive(Clone, Debug)]
pub struct DimEdit {
    pub constraint: ConstraintId,
    pub text: String,
}

pub struct SketchEditor {
    pub feature: FeatureId,
    pub frame: PlaneFrame,
    pub tool: Tool,
    clicks: Vec<Click>,
    /// End point of the previous line (chaining).
    chain_end: Option<EntityId>,
    pub cursor: Option<DVec2>,
    pub inference: Option<Inference>,
    pub selection: Vec<EntityId>,
    pub selected_constraints: Vec<ConstraintId>,
    pub hover: Option<EntityId>,
    drag: Option<DragState>,
    box_end: Option<DVec2>,
    /// Entities picked so far for a constraint/dimension tool.
    picks: Vec<(EntityId, DVec2)>,
    pub dim_edit: Option<DimEdit>,
    pub construction: bool,
    pub polygon_sides: usize,
    pub fillet_radius: f64,
    pub offset_distance: f64,
    /// World units per pixel at the sketch (for snap radii).
    pub world_per_px: f64,
    pub message: Option<String>,
}

pub struct EditOutcome {
    /// Sketch data changed (commit draft + regenerate).
    pub changed: bool,
    /// Only the overlay changed (redraw).
    pub redraw: bool,
}

impl EditOutcome {
    fn none() -> Self {
        Self { changed: false, redraw: false }
    }
    fn redraw() -> Self {
        Self { changed: false, redraw: true }
    }
    fn changed() -> Self {
        Self { changed: true, redraw: true }
    }
}

fn is_point(sk: &Sketch, id: EntityId) -> bool {
    sk.kind(id) == Some(EntityKind::Point)
}

impl SketchEditor {
    pub fn new(feature: FeatureId, frame: PlaneFrame) -> Self {
        Self {
            feature,
            frame,
            tool: Tool::Select,
            clicks: Vec::new(),
            chain_end: None,
            cursor: None,
            inference: None,
            selection: Vec::new(),
            selected_constraints: Vec::new(),
            hover: None,
            drag: None,
            box_end: None,
            picks: Vec::new(),
            dim_edit: None,
            construction: false,
            polygon_sides: 6,
            fillet_radius: 0.005,
            offset_distance: 0.005,
            world_per_px: 1e-4,
            message: None,
        }
    }

    pub fn set_tool(&mut self, tool: Tool) {
        // Pressing the active tool's key again toggles back to select.
        self.tool = if self.tool == tool && tool != Tool::Select { Tool::Select } else { tool };
        self.clicks.clear();
        self.chain_end = None;
        self.picks.clear();
        self.inference = None;
        self.message = match self.tool {
            Tool::Fillet => Some("Click a corner where two curves meet".into()),
            Tool::Trim => Some("Click the part of a curve to remove".into()),
            Tool::Extend => Some("Click near the end of a curve to extend it".into()),
            Tool::Dimension => Some("Select one or two entities, then click to place the dimension".into()),
            Tool::Use => Some("Click model edges or faces to project them into the sketch".into()),
            Tool::Constraint(c) => Some(format!("Select {} entit{} for {}", c.arity(), if c.arity() == 1 { "y" } else { "ies" }, c.label())),
            _ => None,
        };
    }

    /// Cancel the in-progress operation (Esc). Returns true if something was cancelled.
    pub fn cancel(&mut self) -> bool {
        if self.dim_edit.take().is_some() {
            return true;
        }
        if !self.clicks.is_empty() || !self.picks.is_empty() || self.chain_end.is_some() {
            self.clicks.clear();
            self.picks.clear();
            self.chain_end = None;
            return true;
        }
        if self.tool != Tool::Select {
            self.set_tool(Tool::Select);
            return true;
        }
        if !self.selection.is_empty() || !self.selected_constraints.is_empty() {
            self.selection.clear();
            self.selected_constraints.clear();
            return true;
        }
        false
    }

    /// Cursor position on the sketch plane from a world ray.
    pub fn plane_point(&self, origin: DVec3, dir: DVec3) -> Option<DVec2> {
        self.frame.intersect_ray(origin, dir).map(|p| self.frame.to_local(p))
    }

    fn infer(&self, sk: &Sketch, p: DVec2, suppress: bool) -> Option<Inference> {
        if suppress {
            return None;
        }
        let ctx = InferenceContext {
            tool: self.tool.inference_tool(),
            anchor: self.clicks.last().map(|c| c.pos),
            anchor_point: self.chain_end,
            snap_radius: self.world_per_px * 9.0,
            angle_tolerance: 3f64.to_radians(),
            exclude: Vec::new(),
        };
        noshape_sketch::infer(sk, p, &ctx).into_iter().next()
    }

    pub fn pointer_move(&mut self, data: &mut SketchData, p: DVec2, shift: bool) -> EditOutcome {
        self.cursor = Some(p);
        if self.tool.draws() {
            self.inference = self.infer(&data.sketch, p, shift);
            return EditOutcome::redraw();
        }
        self.inference = None;
        EditOutcome::redraw()
    }

    fn snapped(&self) -> Option<(DVec2, Option<Inference>)> {
        let c = self.cursor?;
        Some(match &self.inference {
            Some(i) => (i.position, Some(i.clone())),
            None => (c, None),
        })
    }

    /// Create (or reuse) a point entity for a placed click and return it plus
    /// constraints to apply once the new curve exists.
    fn point_for(sk: &mut Sketch, click: &Click) -> (EntityId, Vec<AutoConstraint>) {
        if let Some(inf) = &click.inference
            && inf.kind == InferenceKind::Endpoint
            && let Some(AutoConstraint::CoincidentPoint(p)) = inf.constraints.first()
        {
            let rest = inf.constraints.iter().skip(1).copied().collect();
            return (*p, rest);
        }
        let id = sk.add_point(click.pos);
        (id, click.inference.as_ref().map(|i| i.constraints.clone()).unwrap_or_default())
    }

    fn apply_auto(sk: &mut Sketch, point: EntityId, curve: Option<EntityId>, autos: &[AutoConstraint]) {
        for a in autos {
            if let Some(kind) = a.to_constraint(point, curve) {
                let _ = sk.add_constraint(kind);
            }
        }
    }

    fn finish_create(&mut self, data: &mut SketchData, created: &[EntityId]) {
        if self.construction {
            for id in created {
                let _ = data.sketch.set_construction(*id, true);
            }
        }
        data.sketch.solve();
    }

    pub fn tool_down(&mut self, data: &mut SketchData, p: DVec2, hovered: Option<EntityId>, shift: bool, double_click: bool) -> EditOutcome {
        self.cursor = Some(p);
        match self.tool {
            Tool::Select => {
                if let Some(lbl) = self.label_at(data, p) {
                    self.drag = Some(DragState::Label { constraint: lbl });
                    return EditOutcome::none();
                }
                match hovered {
                    Some(e) => {
                        self.drag = Some(DragState::Pending { entity: e, start: p, additive: shift });
                    }
                    None => {
                        self.drag = Some(DragState::Box { start: p });
                        self.box_end = Some(p);
                    }
                }
                EditOutcome::none()
            }
            t if t.draws() => {
                if self.inference.is_none() {
                    self.inference = self.infer(&data.sketch, p, shift);
                }
                let (pos, inference) = self.snapped().unwrap_or((p, None));
                let click = Click { pos, inference };
                self.place(data, click, double_click)
            }
            Tool::Fillet => {
                let Some(pid) = hovered.filter(|e| is_point(&data.sketch, *e)).or_else(|| nearest_point(&data.sketch, p, self.world_per_px * 10.0)) else {
                    self.message = Some("Click a corner point".into());
                    return EditOutcome::none();
                };
                match data.sketch.fillet_corner(pid, self.fillet_radius) {
                    Ok(r) => {
                        self.finish_create(data, &r.created);
                        EditOutcome::changed()
                    }
                    Err(e) => {
                        self.message = Some(format!("Fillet failed: {e}"));
                        EditOutcome::redraw()
                    }
                }
            }
            Tool::Trim | Tool::Extend | Tool::Split => {
                let Some(id) = hovered.filter(|e| !is_point(&data.sketch, *e)).or_else(|| data.sketch.pick(p, self.world_per_px * 8.0)) else {
                    return EditOutcome::none();
                };
                let r = match self.tool {
                    Tool::Trim => data.sketch.trim(id, p),
                    Tool::Extend => data.sketch.extend(id, p),
                    _ => data.sketch.split(id, p),
                };
                match r {
                    Ok(_) => {
                        data.sketch.solve();
                        prune_exprs(data);
                        EditOutcome::changed()
                    }
                    Err(e) => {
                        self.message = Some(format!("{e}"));
                        EditOutcome::redraw()
                    }
                }
            }
            Tool::Offset => {
                let ids: Vec<EntityId> = if self.selection.is_empty() { hovered.into_iter().collect() } else { self.selection.clone() };
                if ids.is_empty() {
                    return EditOutcome::none();
                }
                match data.sketch.offset(&ids, self.offset_distance) {
                    Ok(r) => {
                        self.finish_create(data, &r.created);
                        EditOutcome::changed()
                    }
                    Err(e) => {
                        self.message = Some(format!("Offset failed: {e}"));
                        EditOutcome::redraw()
                    }
                }
            }
            Tool::Mirror => {
                // First select entities, then click the mirror line.
                let Some(axis) = hovered.filter(|e| data.sketch.kind(*e) == Some(EntityKind::Line)) else {
                    self.message = Some("Select entities, then click a line to mirror about".into());
                    return EditOutcome::none();
                };
                let ids: Vec<EntityId> = self.selection.iter().copied().filter(|e| *e != axis).collect();
                if ids.is_empty() {
                    return EditOutcome::none();
                }
                match data.sketch.mirror(&ids, axis) {
                    Ok(r) => {
                        self.finish_create(data, &r.created);
                        EditOutcome::changed()
                    }
                    Err(e) => {
                        self.message = Some(format!("Mirror failed: {e}"));
                        EditOutcome::redraw()
                    }
                }
            }
            Tool::Dimension => self.dimension_click(data, p, hovered),
            Tool::Constraint(c) => {
                let Some(e) = hovered else { return EditOutcome::none() };
                if !self.picks.iter().any(|(x, _)| *x == e) {
                    self.picks.push((e, p));
                }
                if self.picks.len() >= c.arity() {
                    let ids: Vec<EntityId> = self.picks.iter().map(|x| x.0).collect();
                    self.picks.clear();
                    match make_constraint(&data.sketch, c, &ids) {
                        Some(kind) => match data.sketch.add_constraint(kind) {
                            Ok(_) => {
                                data.sketch.solve();
                                return EditOutcome::changed();
                            }
                            Err(err) => self.message = Some(format!("Can't add constraint: {err}")),
                        },
                        None => self.message = Some("Those entities can't take this constraint".into()),
                    }
                }
                EditOutcome::redraw()
            }
            _ => EditOutcome::none(),
        }
    }

    /// Place a click for the active drawing tool.
    fn place(&mut self, data: &mut SketchData, click: Click, double_click: bool) -> EditOutcome {
        let sk = &mut data.sketch;
        match self.tool {
            Tool::Point => {
                let (pid, autos) = Self::point_for(sk, &click);
                Self::apply_auto(sk, pid, None, &autos);
                self.finish_create(data, &[pid]);
                EditOutcome::changed()
            }
            Tool::Line => {
                if double_click {
                    self.clicks.clear();
                    self.chain_end = None;
                    return EditOutcome::redraw();
                }
                let Some(prev) = self.clicks.last().cloned() else {
                    self.clicks.push(click);
                    return EditOutcome::redraw();
                };
                if (prev.pos - click.pos).length() < 1e-9 {
                    return EditOutcome::none();
                }
                let (a, autos_a) = match self.chain_end {
                    Some(e) => (e, Vec::new()),
                    None => Self::point_for(sk, &prev),
                };
                let (b, autos_b) = Self::point_for(sk, &click);
                let Ok(line) = sk.add_line_between(a, b) else { return EditOutcome::none() };
                Self::apply_auto(sk, a, Some(line), &autos_a);
                Self::apply_auto(sk, b, Some(line), &autos_b);
                // Closing onto an existing point ends the chain (Onshape).
                let closed = click.inference.as_ref().is_some_and(|i| i.kind == InferenceKind::Endpoint);
                self.finish_create(data, &[line]);
                if closed {
                    self.clicks.clear();
                    self.chain_end = None;
                } else {
                    self.clicks = vec![click];
                    self.chain_end = Some(b);
                }
                EditOutcome::changed()
            }
            Tool::CornerRect | Tool::CenterRect => {
                if self.clicks.is_empty() {
                    self.clicks.push(click);
                    return EditOutcome::redraw();
                }
                let first = self.clicks.remove(0);
                let ids = if self.tool == Tool::CornerRect { sk.corner_rectangle(first.pos, click.pos) } else { sk.center_rectangle(first.pos, click.pos) };
                if self.tool == Tool::CornerRect {
                    if let (Some(&p0), Some(inf)) = (ids.points.first(), &first.inference) {
                        Self::apply_auto(sk, p0, None, &inf.constraints);
                    }
                    if let (Some(&p2), Some(inf)) = (ids.points.get(2), &click.inference) {
                        Self::apply_auto(sk, p2, None, &inf.constraints);
                    }
                } else if let (Some(inf), Some(center)) = (&first.inference, ids.points.last().copied().or(ids.construction.first().copied())) {
                    let _ = (inf, center);
                }
                let mut created = ids.curves.clone();
                created.extend(ids.construction.iter().copied());
                self.finish_create(data, &ids.curves);
                EditOutcome::changed()
            }
            Tool::Circle => {
                if self.clicks.is_empty() {
                    self.clicks.push(click);
                    return EditOutcome::redraw();
                }
                let c = self.clicks.remove(0);
                let r = (click.pos - c.pos).length();
                if r < 1e-9 {
                    return EditOutcome::none();
                }
                let (center, autos) = Self::point_for(sk, &c);
                let Ok(circle) = sk.add_circle_at(center, r) else { return EditOutcome::none() };
                Self::apply_auto(sk, center, None, &autos);
                self.finish_create(data, &[circle]);
                EditOutcome::changed()
            }
            Tool::ThreePointCircle => {
                self.clicks.push(click);
                if self.clicks.len() < 3 {
                    return EditOutcome::redraw();
                }
                let pts: Vec<DVec2> = self.clicks.drain(..).map(|c| c.pos).collect();
                match sk.three_point_circle(pts[0], pts[1], pts[2]) {
                    Ok(id) => {
                        self.finish_create(data, &[id]);
                        EditOutcome::changed()
                    }
                    Err(_) => EditOutcome::redraw(),
                }
            }
            Tool::Arc3 => {
                // Click start, end, then a point on the arc.
                self.clicks.push(click);
                if self.clicks.len() < 3 {
                    return EditOutcome::redraw();
                }
                let c: Vec<Click> = self.clicks.drain(..).collect();
                match sk.three_point_arc(c[0].pos, c[2].pos, c[1].pos) {
                    Ok(arc) => {
                        if let Some((s, e)) = arc_endpoints(sk, arc) {
                            for (pt, cl) in [(s, &c[0]), (e, &c[1])] {
                                let p = sk.point(pt).unwrap_or_default();
                                let near = |q: DVec2| (q - p).length();
                                let cl2 = if near(c[0].pos) < near(c[1].pos) { &c[0] } else { &c[1] };
                                let _ = cl;
                                if let Some(inf) = &cl2.inference {
                                    Self::apply_auto(sk, pt, Some(arc), &inf.constraints);
                                }
                            }
                        }
                        self.finish_create(data, &[arc]);
                        EditOutcome::changed()
                    }
                    Err(_) => EditOutcome::redraw(),
                }
            }
            Tool::CenterArc => {
                self.clicks.push(click);
                if self.clicks.len() < 3 {
                    return EditOutcome::redraw();
                }
                let c: Vec<Click> = self.clicks.drain(..).collect();
                let id = sk.center_point_arc(c[0].pos, c[1].pos, c[2].pos, true);
                self.finish_create(data, &[id]);
                EditOutcome::changed()
            }
            Tool::TangentArc => {
                let from = self.chain_end.or_else(|| click.inference.as_ref().and_then(|i| match i.constraints.first() {
                    Some(AutoConstraint::CoincidentPoint(p)) => Some(*p),
                    _ => None,
                }));
                let Some(from) = from else {
                    if let Some(p) = nearest_point(sk, click.pos, self.world_per_px * 10.0) {
                        self.chain_end = Some(p);
                    } else {
                        self.message = Some("Start a tangent arc at the end of a curve".into());
                    }
                    return EditOutcome::redraw();
                };
                if self.chain_end.is_none() {
                    self.chain_end = Some(from);
                    return EditOutcome::redraw();
                }
                match sk.tangent_arc(from, click.pos) {
                    Ok(ids) => {
                        self.chain_end = ids.points.last().copied();
                        self.finish_create(data, &ids.curves);
                        EditOutcome::changed()
                    }
                    Err(e) => {
                        self.message = Some(format!("{e}"));
                        EditOutcome::redraw()
                    }
                }
            }
            Tool::Spline => {
                if double_click && self.clicks.len() >= 2 {
                    let pts: Vec<DVec2> = self.clicks.drain(..).map(|c| c.pos).collect();
                    if let Ok(id) = sk.add_fit_spline(&pts) {
                        self.finish_create(data, &[id]);
                        return EditOutcome::changed();
                    }
                    return EditOutcome::redraw();
                }
                self.clicks.push(click);
                EditOutcome::redraw()
            }
            Tool::Slot => {
                self.clicks.push(click);
                if self.clicks.len() < 3 {
                    return EditOutcome::redraw();
                }
                let c: Vec<Click> = self.clicks.drain(..).collect();
                let width = 2.0 * dist_point_line(c[2].pos, c[0].pos, c[1].pos);
                match sk.straight_slot(c[0].pos, c[1].pos, width.max(1e-6)) {
                    Ok(ids) => {
                        self.finish_create(data, &ids.curves);
                        EditOutcome::changed()
                    }
                    Err(_) => EditOutcome::redraw(),
                }
            }
            Tool::Polygon => {
                if self.clicks.is_empty() {
                    self.clicks.push(click);
                    return EditOutcome::redraw();
                }
                let c = self.clicks.remove(0);
                match sk.polygon(c.pos, click.pos, self.polygon_sides, true) {
                    Ok(ids) => {
                        self.finish_create(data, &ids.curves);
                        EditOutcome::changed()
                    }
                    Err(_) => EditOutcome::redraw(),
                }
            }
            Tool::Ellipse => {
                self.clicks.push(click);
                if self.clicks.len() < 3 {
                    return EditOutcome::redraw();
                }
                let c: Vec<Click> = self.clicks.drain(..).collect();
                match sk.ellipse_3pt(c[0].pos, c[1].pos, c[2].pos) {
                    Ok(id) => {
                        self.finish_create(data, &[id]);
                        EditOutcome::changed()
                    }
                    Err(_) => EditOutcome::redraw(),
                }
            }
            _ => EditOutcome::none(),
        }
    }

    pub fn tool_drag(&mut self, data: &mut SketchData, p: DVec2) -> EditOutcome {
        self.cursor = Some(p);
        match self.drag.clone() {
            Some(DragState::Pending { entity, start, .. }) => {
                if (p - start).length() < self.world_per_px * 3.0 {
                    return EditOutcome::none();
                }
                let handle = if is_point(&data.sketch, entity) { DragHandle::Point(entity) } else { DragHandle::Entity { id: entity, grab: start } };
                self.drag = Some(DragState::Geometry { handle });
                self.tool_drag(data, p)
            }
            Some(DragState::Geometry { handle }) => {
                let target = p;
                let report = data.sketch.drag(&[(handle, target)]);
                if let DragHandle::Entity { id, .. } = handle {
                    self.drag = Some(DragState::Geometry { handle: DragHandle::Entity { id, grab: p } });
                }
                let _ = report;
                EditOutcome::changed()
            }
            Some(DragState::Label { constraint }) => {
                data.dimension_labels.insert(constraint.0, [p.x, p.y]);
                EditOutcome::changed()
            }
            Some(DragState::Box { .. }) => {
                self.box_end = Some(p);
                EditOutcome::redraw()
            }
            None => EditOutcome::none(),
        }
    }

    pub fn tool_up(&mut self, data: &mut SketchData, p: DVec2) -> EditOutcome {
        let out = match self.drag.take() {
            Some(DragState::Pending { entity, additive, .. }) => {
                if let Some(pos) = self.selection.iter().position(|e| *e == entity) {
                    self.selection.remove(pos);
                } else {
                    if !additive && false {
                        self.selection.clear();
                    }
                    self.selection.push(entity);
                }
                EditOutcome::redraw()
            }
            Some(DragState::Geometry { .. }) => {
                data.sketch.solve();
                EditOutcome::changed()
            }
            Some(DragState::Box { start }) => {
                let (lo, hi) = (start.min(p), start.max(p));
                if (hi - lo).length() > self.world_per_px * 3.0 {
                    let crossing = p.x < start.x;
                    for e in data.sketch.entities() {
                        let Some(bb) = data.sketch.entity_bbox(e.id) else { continue };
                        let inside = bb.min.x >= lo.x && bb.min.y >= lo.y && bb.max.x <= hi.x && bb.max.y <= hi.y;
                        let touches = bb.max.x >= lo.x && bb.min.x <= hi.x && bb.max.y >= lo.y && bb.min.y <= hi.y;
                        if (inside || (crossing && touches)) && !self.selection.contains(&e.id) {
                            self.selection.push(e.id);
                        }
                    }
                } else {
                    self.selection.clear();
                    self.selected_constraints.clear();
                }
                self.box_end = None;
                EditOutcome::redraw()
            }
            Some(DragState::Label { .. }) => EditOutcome::changed(),
            None => EditOutcome::none(),
        };
        self.box_end = None;
        out
    }

    // ------------------------------------------------------------ dimensions

    fn dimension_click(&mut self, data: &mut SketchData, p: DVec2, hovered: Option<EntityId>) -> EditOutcome {
        if let Some(e) = hovered {
            if !self.picks.iter().any(|(x, _)| *x == e) && self.picks.len() < 2 {
                self.picks.push((e, p));
                return EditOutcome::redraw();
            }
        }
        if self.picks.is_empty() {
            return EditOutcome::none();
        }
        // Click in empty space places the dimension.
        let ids: Vec<EntityId> = self.picks.iter().map(|x| x.0).collect();
        self.picks.clear();
        let Some(kind) = smart_dimension(&data.sketch, &ids, p) else {
            self.message = Some("Can't dimension that selection".into());
            return EditOutcome::redraw();
        };
        // Add as a driving dimension at its current measured value; if that
        // over-constrains, add it as driven (Onshape behavior).
        let cid = match data.sketch.add_constraint(kind.clone()) {
            Ok(c) => c,
            Err(_) => match data.sketch.add_reference(kind) {
                Ok(c) => c,
                Err(e) => {
                    self.message = Some(format!("{e}"));
                    return EditOutcome::redraw();
                }
            },
        };
        let report = data.sketch.solve();
        if let noshape_sketch::SolveStatus::Overconstrained { conflicting } = &report.status
            && conflicting.contains(&cid)
        {
            let _ = data.sketch.set_driving(cid, false);
            data.sketch.solve();
            self.message = Some("Dimension would over-constrain the sketch; added as driven".into());
        }
        data.dimension_labels.insert(cid.0, [p.x, p.y]);
        let value = data.sketch.constraint(cid).and_then(|c| c.kind.value()).unwrap_or(0.0);
        let is_angle = data.sketch.constraint(cid).is_some_and(|c| matches!(c.kind, ConstraintKind::Angle { .. } | ConstraintKind::ArcAngle { .. }));
        let driving = data.sketch.constraint(cid).is_some_and(|c| c.driving);
        if driving {
            let q = Quantity::new(value, if is_angle { Dim::ANGLE } else { Dim::LENGTH });
            self.dim_edit = Some(DimEdit { constraint: cid, text: noshape_units::format_quantity(q, &UnitPrefs::default()) });
        }
        EditOutcome::changed()
    }

    /// Apply a typed dimension value (expression). Returns an error message.
    pub fn apply_dimension(&mut self, data: &mut SketchData, cid: ConstraintId, text: &str, units: &UnitPrefs, vars: &dyn noshape_units::VariableScope) -> Result<(), String> {
        let Some(c) = data.sketch.constraint(cid) else { return Err("Dimension no longer exists".into()) };
        let dim = if matches!(c.kind, ConstraintKind::Angle { .. } | ConstraintKind::ArcAngle { .. }) { Dim::ANGLE } else { Dim::LENGTH };
        let q = noshape_units::evaluate_as(text, dim, units, vars).map_err(|e| e.to_string())?;
        if dim == Dim::LENGTH && q.value <= 0.0 && matches!(c.kind, ConstraintKind::Radius { .. } | ConstraintKind::Diameter { .. } | ConstraintKind::Length { .. }) {
            return Err("Value must be positive".into());
        }
        data.sketch.set_dimension_value(cid, q.value).map_err(|e| e.to_string())?;
        // Plain numbers are stored as literals; expressions keep their text.
        data.dimension_exprs.insert(cid.0, ExprValue::new(text.trim()));
        let report = data.sketch.solve();
        if !report.is_solved() {
            return Err("The sketch could not be solved with that value".into());
        }
        Ok(())
    }

    fn label_at(&self, data: &SketchData, p: DVec2) -> Option<ConstraintId> {
        let r = self.world_per_px * 14.0;
        data.dimension_labels.iter().find(|(_, l)| (DVec2::new(l[0], l[1]) - p).length() < r).map(|(c, _)| ConstraintId(*c))
    }

    /// Dimension label positions and texts for display (local coords).
    pub fn dimension_labels(&self, data: &SketchData, units: &UnitPrefs) -> Vec<(ConstraintId, DVec2, String, bool)> {
        let mut out = Vec::new();
        for c in data.sketch.constraints() {
            let Some(v) = c.kind.value() else { continue };
            let pos = data.dimension_labels.get(&c.id.0).map(|l| DVec2::new(l[0], l[1])).or_else(|| default_label_pos(&data.sketch, &c.kind));
            let Some(pos) = pos else { continue };
            let is_angle = matches!(c.kind, ConstraintKind::Angle { .. } | ConstraintKind::ArcAngle { .. });
            let q = Quantity::new(v, if is_angle { Dim::ANGLE } else { Dim::LENGTH });
            let mut text = match data.dimension_exprs.get(&c.id.0) {
                Some(e) if e.text.contains('#') => format!("{} = {}", e.text, noshape_units::format_quantity(q, units)),
                _ => noshape_units::format_quantity(q, units),
            };
            match c.kind {
                ConstraintKind::Diameter { .. } => text = format!("Ø{text}"),
                ConstraintKind::Radius { .. } => text = format!("R{text}"),
                _ => {}
            }
            if !c.driving {
                text = format!("({text})");
            }
            out.push((c.id, pos, text, c.driving));
        }
        out
    }

    // ------------------------------------------------------------ editing commands

    pub fn delete_selection(&mut self, data: &mut SketchData) -> bool {
        let mut changed = false;
        for c in self.selected_constraints.drain(..) {
            if data.sketch.remove_constraint(c).is_ok() {
                data.dimension_exprs.remove(&c.0);
                data.dimension_labels.remove(&c.0);
                changed = true;
            }
        }
        if !self.selection.is_empty() {
            let ids = std::mem::take(&mut self.selection);
            data.sketch.remove_entities(&ids);
            changed = true;
        }
        if changed {
            prune_exprs(data);
            data.sketch.solve();
        }
        changed
    }

    pub fn toggle_construction(&mut self, data: &mut SketchData) -> bool {
        if self.selection.is_empty() {
            self.construction = !self.construction;
            return false;
        }
        for id in &self.selection {
            let cur = data.sketch.entity(*id).is_some_and(|e| e.construction);
            let _ = data.sketch.set_construction(*id, !cur);
        }
        true
    }

    /// Apply a constraint tool to the current selection if it fits; else
    /// switch to the constraint tool.
    pub fn constraint_command(&mut self, data: &mut SketchData, c: ConstraintTool) -> bool {
        if self.selection.len() >= c.arity()
            && let Some(kind) = make_constraint(&data.sketch, c, &self.selection)
            && data.sketch.add_constraint(kind).is_ok()
        {
            data.sketch.solve();
            self.selection.clear();
            return true;
        }
        self.set_tool(Tool::Constraint(c));
        false
    }

    // ------------------------------------------------------------ overlay

    /// Preview geometry, inference guides, picks and box selection (world space).
    pub fn overlay(&self, data: &SketchData) -> (LineData, PointData) {
        let mut lines = LineData::default();
        let mut points = PointData::default();
        let w = |p: DVec2| -> [f32; 3] {
            let v = self.frame.to_world(p) + self.frame.normal * 1e-6;
            [v.x as f32, v.y as f32, v.z as f32]
        };
        let preview_color = [0x8b, 0x7c, 0xff, 255];
        let guide_color = [0xff, 0xc2, 0x0e, 255];
        let on_top = LineSegment::ON_TOP;
        let mut poly = |pts: &[DVec2], color: [u8; 4], dashed: bool| {
            let pts3: Vec<[f32; 3]> = pts.iter().map(|p| w(*p)).collect();
            lines.push_polyline(&pts3, color, 0, 1.5, on_top | if dashed { LineSegment::DASHED } else { 0 });
        };
        let cursor = self.snapped().map(|s| s.0);
        if let Some(c) = cursor {
            let anchors: Vec<DVec2> = self.clicks.iter().map(|k| k.pos).collect();
            match self.tool {
                Tool::Line => {
                    if let Some(a) = anchors.last() {
                        poly(&[*a, c], preview_color, false);
                    }
                }
                Tool::CornerRect => {
                    if let Some(a) = anchors.first() {
                        poly(&[*a, DVec2::new(c.x, a.y), c, DVec2::new(a.x, c.y), *a], preview_color, false);
                    }
                }
                Tool::CenterRect => {
                    if let Some(a) = anchors.first() {
                        let d = c - *a;
                        let q = [*a + DVec2::new(-d.x, -d.y), *a + DVec2::new(d.x, -d.y), *a + d, *a + DVec2::new(-d.x, d.y)];
                        poly(&[q[0], q[1], q[2], q[3], q[0]], preview_color, false);
                    }
                }
                Tool::Circle | Tool::Polygon => {
                    if let Some(a) = anchors.first() {
                        let r = (c - *a).length();
                        let n = if self.tool == Tool::Polygon { self.polygon_sides } else { 72 };
                        let start = (c - *a).y.atan2((c - *a).x);
                        let pts: Vec<DVec2> = (0..=n).map(|i| *a + DVec2::from_angle(start + i as f64 / n as f64 * std::f64::consts::TAU) * r).collect();
                        poly(&pts, preview_color, false);
                        poly(&[*a, c], preview_color, true);
                    }
                }
                Tool::Arc3 => match anchors.len() {
                    1 => poly(&[anchors[0], c], preview_color, true),
                    2 => {
                        if let Some((ctr, r)) = noshape_sketch::shapes::circle_through(anchors[0], c, anchors[1]) {
                            let a0 = (anchors[0] - ctr).y.atan2((anchors[0] - ctr).x);
                            let a1 = (anchors[1] - ctr).y.atan2((anchors[1] - ctr).x);
                            let am = (c - ctr).y.atan2((c - ctr).x);
                            let (s, e) = arc_span(a0, a1, am);
                            let pts: Vec<DVec2> = (0..=48).map(|i| ctr + DVec2::from_angle(s + (e - s) * i as f64 / 48.0) * r).collect();
                            poly(&pts, preview_color, false);
                        }
                    }
                    _ => {}
                },
                Tool::Spline | Tool::ThreePointCircle | Tool::Slot | Tool::Ellipse | Tool::CenterArc => {
                    let mut pts = anchors.clone();
                    pts.push(c);
                    if pts.len() >= 2 {
                        poly(&pts, preview_color, true);
                    }
                }
                _ => {}
            }
            if self.tool.draws() {
                points.points.push(PointSprite { position: w(c), color: preview_color, pick_id: 0, size: 6.0, style: on_top, _pad: 0 });
            }
        }
        if let Some(inf) = &self.inference
            && let Some(r) = inf.reference
        {
            let mut lines2 = LineData::default();
            let pts3 = [w(r), w(inf.position)];
            lines2.push_polyline(&pts3, guide_color, 0, 1.0, on_top | LineSegment::DASHED);
            lines.segments.extend(lines2.segments);
        }
        if let Some(inf) = &self.inference {
            points.points.push(PointSprite { position: w(inf.position), color: guide_color, pick_id: 0, size: 8.0, style: on_top, _pad: 0 });
        }
        // Box selection rectangle.
        if let (Some(DragState::Box { start }), Some(end)) = (&self.drag, self.box_end) {
            let color = if end.x < start.x { [0x4a, 0xde, 0x80, 255] } else { [0x4c, 0xc9, 0xf0, 255] };
            let a = *start;
            let b = end;
            let pts3: Vec<[f32; 3]> = [a, DVec2::new(b.x, a.y), b, DVec2::new(a.x, b.y), a].iter().map(|p| w(*p)).collect();
            lines.push_polyline(&pts3, color, 0, 1.0, on_top | if end.x < start.x { LineSegment::DASHED } else { 0 });
        }
        // Dimension leaders from label to measured geometry.
        for (cid, pos, _, driving) in self.dimension_labels(data, &UnitPrefs::default()) {
            if let Some(c) = data.sketch.constraint(cid)
                && let Some(anchor) = dimension_anchor(&data.sketch, &c.kind)
            {
                let color = if driving { [0xa0, 0xa8, 0xb6, 255] } else { [0x6b, 0x73, 0x82, 255] };
                let pts3 = [w(pos), w(anchor)];
                lines.push_polyline(&pts3, color, 0, 1.0, on_top);
            }
        }
        (lines, points)
    }

    /// References of picked entities for constraint/dimension tools (accent highlight).
    pub fn picked(&self) -> Vec<EntityId> {
        self.picks.iter().map(|p| p.0).collect()
    }
}

fn arc_span(a0: f64, a1: f64, am: f64) -> (f64, f64) {
    use std::f64::consts::TAU;
    let norm = |x: f64| x.rem_euclid(TAU);
    let s = a0;
    let mut e = a1;
    // Choose the direction that passes through am.
    let ccw_sweep = norm(e - s);
    let mid_off = norm(am - s);
    if mid_off <= ccw_sweep {
        e = s + ccw_sweep;
    } else {
        e = s - (TAU - ccw_sweep);
    }
    (s, e)
}

fn arc_endpoints(sk: &Sketch, arc: EntityId) -> Option<(EntityId, EntityId)> {
    sk.endpoints(arc)
}

fn nearest_point(sk: &Sketch, p: DVec2, radius: f64) -> Option<EntityId> {
    sk.entities()
        .filter(|e| e.kind() == EntityKind::Point)
        .filter_map(|e| sk.point(e.id).map(|q| (e.id, (q - p).length())))
        .filter(|(_, d)| *d <= radius)
        .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
        .map(|x| x.0)
}

fn dist_point_line(p: DVec2, a: DVec2, b: DVec2) -> f64 {
    let d = b - a;
    let l = d.length();
    if l < 1e-15 {
        return (p - a).length();
    }
    (d.perp_dot(p - a) / l).abs()
}

/// Remove dimension expressions/labels whose constraints are gone.
fn prune_exprs(data: &mut SketchData) {
    let live: std::collections::HashSet<u32> = data.sketch.constraints().map(|c| c.id.0).collect();
    data.dimension_exprs.retain(|k, _| live.contains(k));
    data.dimension_labels.retain(|k, _| live.contains(k));
}

fn curve_mid(sk: &Sketch, id: EntityId) -> Option<DVec2> {
    match sk.curve(id)? {
        Curve2::Line { a, b } => Some((a + b) * 0.5),
        Curve2::Circle { center, radius } => Some(center + DVec2::new(radius, 0.0)),
        Curve2::Arc { center, radius, start_angle, end_angle } => Some(center + DVec2::from_angle((start_angle + end_angle) * 0.5) * radius),
        _ => sk.entity_bbox(id).map(|b| (b.min + b.max) * 0.5),
    }
}

fn entity_anchor(sk: &Sketch, id: EntityId) -> Option<DVec2> {
    sk.point(id).or_else(|| curve_mid(sk, id))
}

fn dimension_anchor(sk: &Sketch, k: &ConstraintKind) -> Option<DVec2> {
    match k {
        ConstraintKind::Distance { a, b, .. } | ConstraintKind::HorizontalDistance { a, b, .. } | ConstraintKind::VerticalDistance { a, b, .. } | ConstraintKind::Angle { a, b, .. } => {
            Some((entity_anchor(sk, *a)? + entity_anchor(sk, *b)?) * 0.5)
        }
        ConstraintKind::Length { curve, .. } | ConstraintKind::Radius { curve, .. } | ConstraintKind::Diameter { curve, .. } => curve_mid(sk, *curve),
        ConstraintKind::ArcAngle { arc, .. } => curve_mid(sk, *arc),
        ConstraintKind::EllipseRadius { ellipse, .. } => curve_mid(sk, *ellipse),
        _ => None,
    }
}

fn default_label_pos(sk: &Sketch, k: &ConstraintKind) -> Option<DVec2> {
    let a = dimension_anchor(sk, k)?;
    let size = (sk.bbox().max - sk.bbox().min).length().max(0.01);
    Some(a + DVec2::new(0.0, size * 0.08))
}

/// Onshape-style smart dimension from picked entities and cursor position.
fn smart_dimension(sk: &Sketch, ids: &[EntityId], cursor: DVec2) -> Option<ConstraintKind> {
    let kind = |id: EntityId| sk.kind(id);
    match ids {
        [a] => match kind(*a)? {
            EntityKind::Line => {
                let (pa, pb) = sk.endpoints(*a)?;
                let (qa, qb) = (sk.point(pa)?, sk.point(pb)?);
                Some(directed(pa, pb, qa, qb, cursor).unwrap_or(ConstraintKind::Length { curve: *a, value: (qb - qa).length() }))
            }
            EntityKind::Circle => Some(ConstraintKind::Diameter { curve: *a, value: 0.0 }),
            EntityKind::Arc => Some(ConstraintKind::Radius { curve: *a, value: 0.0 }),
            EntityKind::Ellipse | EntityKind::EllipticalArc => Some(ConstraintKind::EllipseRadius { ellipse: *a, major: true, value: 0.0 }),
            _ => None,
        },
        [a, b] => {
            let (ka, kb) = (kind(*a)?, kind(*b)?);
            match (ka, kb) {
                (EntityKind::Point, EntityKind::Point) => {
                    let (qa, qb) = (sk.point(*a)?, sk.point(*b)?);
                    Some(directed(*a, *b, qa, qb, cursor).unwrap_or(ConstraintKind::Distance { a: *a, b: *b, value: 0.0, mode: DistanceMode::Center }))
                }
                (EntityKind::Line, EntityKind::Line) => {
                    let dir = |l: EntityId| -> Option<DVec2> {
                        let (pa, pb) = sk.endpoints(l)?;
                        Some((sk.point(pb)? - sk.point(pa)?).normalize())
                    };
                    let (da, db) = (dir(*a)?, dir(*b)?);
                    if da.perp_dot(db).abs() < 1e-6 {
                        Some(ConstraintKind::Distance { a: *a, b: *b, value: 0.0, mode: DistanceMode::Center })
                    } else {
                        Some(ConstraintKind::Angle { a: *a, b: *b, value: 0.0 })
                    }
                }
                _ => Some(ConstraintKind::Distance { a: *a, b: *b, value: 0.0, mode: DistanceMode::Center }),
            }
        }
        _ => None,
    }
}

/// Horizontal/vertical distance when the cursor is outside the band between two points.
fn directed(a: EntityId, b: EntityId, qa: DVec2, qb: DVec2, cursor: DVec2) -> Option<ConstraintKind> {
    let (lo, hi) = (qa.min(qb), qa.max(qb));
    let dx = (qb.x - qa.x).abs();
    let dy = (qb.y - qa.y).abs();
    if (cursor.y > hi.y || cursor.y < lo.y) && cursor.x >= lo.x && cursor.x <= hi.x && dx > 1e-9 {
        return Some(ConstraintKind::HorizontalDistance { a, b, value: dx });
    }
    if (cursor.x > hi.x || cursor.x < lo.x) && cursor.y >= lo.y && cursor.y <= hi.y && dy > 1e-9 {
        return Some(ConstraintKind::VerticalDistance { a, b, value: dy });
    }
    None
}

fn make_constraint(sk: &Sketch, c: ConstraintTool, ids: &[EntityId]) -> Option<ConstraintKind> {
    let pt = |i: usize| ids.get(i).copied();
    let a = pt(0)?;
    let b = pt(1);
    Some(match c {
        ConstraintTool::Horizontal => ConstraintKind::Horizontal { a, b },
        ConstraintTool::Vertical => ConstraintKind::Vertical { a, b },
        ConstraintTool::Fix => ConstraintKind::Fix { entity: a },
        ConstraintTool::Coincident => ConstraintKind::Coincident { a, b: b? },
        ConstraintTool::Concentric => ConstraintKind::Concentric { a, b: b? },
        ConstraintTool::Parallel => ConstraintKind::Parallel { a, b: b? },
        ConstraintTool::Perpendicular => ConstraintKind::Perpendicular { a, b: b? },
        ConstraintTool::Tangent => ConstraintKind::Tangent { a, b: b? },
        ConstraintTool::Equal => ConstraintKind::Equal { a, b: b? },
        ConstraintTool::Curvature => ConstraintKind::Curvature { a, b: b? },
        ConstraintTool::Midpoint => {
            let b = b?;
            if is_point(sk, a) { ConstraintKind::Midpoint { point: a, curve: b } } else { ConstraintKind::Midpoint { point: b, curve: a } }
        }
        ConstraintTool::Normal => ConstraintKind::Normal { line: a, curve: b? },
        ConstraintTool::Pierce => ConstraintKind::Pierce { point: a, target: b? },
        ConstraintTool::Symmetric => ConstraintKind::Symmetric { a, b: b?, axis: pt(2)? },
    })
}

impl SketchEditor {
    /// Add a projected (used) curve linked to a 3D reference.
    pub fn add_used(&mut self, data: &mut SketchData, curve: &Curve2, reference: noshape_model::reference::Reference) -> bool {
        match data.sketch.use_external(curve) {
            Ok(r) => {
                if let Some(&id) = r.created.last() {
                    data.external_refs.insert(id.0, reference);
                }
                data.sketch.solve();
                true
            }
            Err(e) => {
                self.message = Some(format!("{e}"));
                false
            }
        }
    }
}
