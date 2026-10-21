//! Sketches: created on a plane or planar face, drawn with the sketch
//! editor's own tools (the clicks a person makes, with the same inference
//! snapping and automatic constraints), constrained with the constraint
//! tools and dimensioned with the dimension tool.

use super::{doc_field, edit_studio, feature_arg, regen_report, s, studio_target, tab_field};
use crate::error::OpError;
use crate::registry::{Args, Op, OpOutput, opt, req};
use crate::{Ctx, OpenDoc, doc, refs, studio};
use glam::DVec2;
use noshape_model::display::PlaneFrame;
use noshape_model::document::TabId;
use noshape_model::params::ParamValue;
use noshape_model::reference::{FeatureId, Reference};
use noshape_model::regen::RegenResult;
use noshape_model::sketch_feature::SketchData;
use noshape_scene::sketch_mode::{self as sm, ConstraintTool, SketchEditor, Tool};
use noshape_sketch::{ConstraintId, ConstraintKind, DistanceMode, EntityId, Geometry};
use noshape_units::{Dim, UnitPrefs};
use serde_json::{Value, json};

/// Sketch units per pixel of the headless editor: the 9 px snap radius is
/// 9 µm, so clicks snap to geometry only at (practically) the same place.
pub(super) const WORLD_PER_PX: f64 = 1e-6;

/// Drawing tools: (name, tool, clicks it takes, how).
const DRAW_TOOLS: [(&str, Tool, &str); 19] = [
    ("line", Tool::Line, "Clicks chain segments: [p0, p1, p2...] makes p0-p1, p1-p2...; repeat the first point to close the loop."),
    ("midpoint_line", Tool::MidpointLine, "[midpoint, end]."),
    ("corner_rectangle", Tool::CornerRect, "[corner, opposite corner]. Horizontal/vertical and coincident constraints, like the UI."),
    ("center_rectangle", Tool::CenterRect, "[center, corner]."),
    ("aligned_rectangle", Tool::AlignedRect, "[corner, second corner (edge direction), third point (width)]."),
    ("circle", Tool::Circle, "[center, point on the circle]."),
    ("three_point_circle", Tool::ThreePointCircle, "[p1, p2, p3] on the circle."),
    ("three_point_arc", Tool::Arc3, "[start, end, point on the arc]."),
    ("tangent_arc", Tool::TangentArc, "[start at the end of a curve, end]."),
    ("center_point_arc", Tool::CenterArc, "[center, start, end] (counter-clockwise)."),
    ("point", Tool::Point, "One click per point."),
    ("spline", Tool::Spline, "Fit spline through the clicked points (2 or more)."),
    ("bezier", Tool::Bezier, "Bezier through control points."),
    ("conic", Tool::Conic, "[start, end, control]."),
    ("slot", Tool::Slot, "Click on an existing line/arc (its centerline), then a point setting the width."),
    ("inscribed_polygon", Tool::Polygon, "[center, vertex]; `sides` sets the number of sides (default 6)."),
    ("circumscribed_polygon", Tool::CircumPolygon, "[center, edge midpoint]; `sides` as above."),
    ("ellipse", Tool::Ellipse, "[center, major axis end, point setting the minor radius]."),
    ("elliptical_arc", Tool::EllipticalArc, "[center, major axis end, start point (also sets the minor radius), end point] (counter-clockwise)."),
];

const CONSTRAINTS: [(&str, ConstraintTool, &str); 14] = [
    ("coincident", ConstraintTool::Coincident, "points/curves coincide (point on curve, collinear lines, same circle)"),
    ("concentric", ConstraintTool::Concentric, "centers of circles/arcs coincide"),
    ("parallel", ConstraintTool::Parallel, "lines parallel"),
    ("tangent", ConstraintTool::Tangent, "curves tangent"),
    ("horizontal", ConstraintTool::Horizontal, "lines horizontal, or two points level"),
    ("vertical", ConstraintTool::Vertical, "lines vertical, or two points aligned vertically"),
    ("midpoint", ConstraintTool::Midpoint, "a point at the midpoint of a line or arc"),
    ("perpendicular", ConstraintTool::Perpendicular, "lines perpendicular"),
    ("equal", ConstraintTool::Equal, "equal lengths / radii"),
    ("symmetric", ConstraintTool::Symmetric, "two entities symmetric about a line (the last entity)"),
    ("fix", ConstraintTool::Fix, "fix in place"),
    ("normal", ConstraintTool::Normal, "line normal to a curve"),
    ("pierce", ConstraintTool::Pierce, "point pierces external geometry"),
    ("curvature", ConstraintTool::Curvature, "curvature continuity at a joint"),
];

const DIMENSION_KINDS: &str = "auto (what the Dimension tool infers from the entities and `label` position), distance (point/line/circle to point/line/circle; `mode` center|minimum|maximum for circles), horizontal_distance, vertical_distance, length (line or arc), radius, diameter, angle (two lines), arc_angle, centerline (twice the distance to a line: revolve diameters).";

pub fn ops() -> Vec<Op> {
    let point = json!({ "type": "array", "items": { "type": ["number", "string"] }, "minItems": 2, "maxItems": 2 });
    vec![
        Op {
            group: "sketch",
            verb: "create",
            summary: "Create a sketch on a plane or planar face",
            description: "Inserts a Sketch feature (\"Insert feature : Sketch 1\") at the rollback bar. Returns its id and plane frame: sketch coordinates (x, y) map to world = origin + x*xdir + y*ydir. On `plane:top` x = world X, y = world Y; on `plane:front` x = X, y = Z; on `plane:right` x = Y, y = Z.",
            fields: vec![doc_field(), tab_field(), req("plane", s(), "Plane: `plane:top`, `plane:front`, `plane:right`, a Plane feature (`fplane:…`), a planar face id or a mate connector."), opt("name", s(), "Name (default \"Sketch N\").")],
            mutates: true,
            destructive: false,
            run: create,
        },
        Op {
            group: "sketch",
            verb: "draw",
            summary: "Draw sketch geometry with a sketch tool (line, rectangle, circle, arc, spline, slot, polygon, ellipse, point...)",
            description: Box::leak(
                format!(
                    "Runs the sketch editor's drawing tool on the clicks you give, exactly as clicking in the sketch: inference snaps to existing points/curves within 9 µm and adds the same automatic constraints (coincident, horizontal, vertical, tangent...). Coordinates are sketch-plane (x, y) in workspace units, or length expressions (\"10 mm\"). One call = one edit of the sketch (\"Edit : Sketch 1\"). Returns the created entity ids (`entity:…`), the sketch's solve state and its regions (`region:…` ids for Extrude etc.). Tools:\n{}",
                    DRAW_TOOLS.iter().map(|(n, _, h)| format!("- {n}: {h}")).collect::<Vec<_>>().join("\n")
                )
                .into_boxed_str(),
            ),
            fields: vec![
                doc_field(),
                tab_field(),
                req("sketch", s(), "Sketch feature id or name."),
                req("tool", json!({ "type": "string", "enum": DRAW_TOOLS.iter().map(|t| t.0).collect::<Vec<_>>() }), "Drawing tool."),
                req("points", json!({ "type": "array", "items": point }), "The clicks, in order: [[x, y], ...]."),
                opt("construction", json!({ "type": "boolean" }), "Make the new geometry construction geometry."),
                opt("sides", json!({ "type": "integer", "minimum": 3, "maximum": 1000 }), "Polygon sides."),
            ],
            mutates: true,
            destructive: false,
            run: draw,
        },
        Op {
            group: "sketch",
            verb: "constrain",
            summary: "Add a geometric constraint between sketch entities",
            description: Box::leak(
                format!(
                    "Selects the entities and applies the constraint tool, as in the UI (select-first). Entity ids come from `sketch_draw` / `sketch_get` (`entity:…`; curves, or their points). Constraints:\n{}",
                    CONSTRAINTS.iter().map(|(n, _, h)| format!("- {n}: {h}")).collect::<Vec<_>>().join("\n")
                )
                .into_boxed_str(),
            ),
            fields: vec![
                doc_field(),
                tab_field(),
                req("sketch", s(), "Sketch feature id or name."),
                req("constraint", json!({ "type": "string", "enum": CONSTRAINTS.iter().map(|c| c.0).collect::<Vec<_>>() }), "Constraint."),
                req("entities", json!({ "type": "array", "items": { "type": "string" }, "minItems": 1 }), "Sketch entity ids, or the origin (`origin`) to constrain to it."),
            ],
            mutates: true,
            destructive: false,
            run: constrain,
        },
        Op {
            group: "sketch",
            verb: "dimension",
            summary: "Add a driving dimension (value or expression) to sketch entities",
            description: Box::leak(format!("The Dimension tool: placed as a reference, made driving unless that over-constrains the sketch, then set to `value` (\"20 mm\", \"#width\", \"45 deg\"; a bare number uses workspace units). Kinds: {DIMENSION_KINDS}").into_boxed_str()),
            fields: vec![
                doc_field(),
                tab_field(),
                req("sketch", s(), "Sketch feature id or name."),
                req("entities", json!({ "type": "array", "items": { "type": "string" }, "minItems": 1, "maxItems": 2 }), "One or two sketch entity ids (`entity:…`) or `origin`."),
                req("value", json!({ "type": ["string", "number"] }), "Value or expression."),
                opt("kind", json!({ "type": "string", "enum": ["auto", "distance", "horizontal_distance", "vertical_distance", "length", "radius", "diameter", "angle", "arc_angle", "centerline"] }), "Dimension kind (default auto)."),
                opt("mode", json!({ "type": "string", "enum": ["center", "minimum", "maximum"] }), "Distance to a circle/arc measured to its center (default), nearest or farthest point."),
                opt("label", point.clone(), "Label position in sketch coordinates (also picks the auto kind: above/below two points gives a horizontal distance, beside them a vertical one)."),
            ],
            mutates: true,
            destructive: false,
            run: dimension,
        },
        Op {
            group: "sketch",
            verb: "set_dimension",
            summary: "Change a sketch dimension's value or expression",
            description: "Like double-clicking a dimension and typing a value. Negative distances and angles flip the geometry, as in Onshape.",
            fields: vec![doc_field(), tab_field(), req("sketch", s(), "Sketch feature id or name."), req("dimension", s(), "Dimension id (`dim:…` from sketch_get / sketch_dimension)."), req("value", json!({ "type": ["string", "number"] }), "Value or expression.")],
            mutates: true,
            destructive: false,
            run: set_dimension,
        },
        Op {
            group: "sketch",
            verb: "delete",
            summary: "Delete sketch entities, constraints or dimensions",
            description: "Deleting a curve deletes its constraints; its points stay when other curves use them.",
            fields: vec![
                doc_field(),
                tab_field(),
                req("sketch", s(), "Sketch feature id or name."),
                opt("entities", json!({ "type": "array", "items": { "type": "string" } }), "Entity ids."),
                opt("constraints", json!({ "type": "array", "items": { "type": "string" } }), "Constraint or dimension ids (`con:…`, `dim:…`)."),
            ],
            mutates: true,
            destructive: true,
            run: delete,
        },
        Op {
            group: "sketch",
            verb: "set_construction",
            summary: "Toggle construction geometry",
            description: "",
            fields: vec![doc_field(), tab_field(), req("sketch", s(), "Sketch feature id or name."), req("entities", json!({ "type": "array", "items": { "type": "string" } }), "Entity ids."), opt("construction", json!({ "type": "boolean", "default": true }), "true: construction.")],
            mutates: true,
            destructive: false,
            run: set_construction,
        },
        Op {
            group: "sketch",
            verb: "get",
            summary: "A sketch's entities, constraints, dimensions, solve state and regions",
            description: "Entities with ids and geometry (sketch coordinates, workspace units), constraints and dimensions (`dim:…`, value and expression), DOF and status (fully defined, under defined, over defined), regions (`region:…` ids, area) for Extrude / Revolve, and the plane frame.",
            fields: vec![doc_field(), tab_field(), req("sketch", s(), "Sketch feature id or name.")],
            mutates: false,
            destructive: false,
            run: get,
        },
    ]
}

// ------------------------------------------------------------ values

/// A length from JSON: a number in workspace units or an expression with units (meters out).
pub(crate) fn length_value(v: &Value, units: &UnitPrefs) -> Result<f64, OpError> {
    let text = match v {
        Value::Number(n) => n.to_string(),
        Value::String(s) => s.clone(),
        _ => return Err(OpError::invalid("a length: a number (workspace units) or an expression like \"10 mm\"")),
    };
    noshape_units::evaluate_as(&text, Dim::LENGTH, units, &noshape_units::EmptyScope).map(|q| q.value).map_err(|e| OpError::invalid(format!("`{text}`: {e}")))
}

pub(super) fn point_value(v: &Value, units: &UnitPrefs) -> Result<DVec2, OpError> {
    let a = v.as_array().filter(|a| a.len() == 2).ok_or_else(|| OpError::invalid("a point is [x, y]"))?;
    Ok(DVec2::new(length_value(&a[0], units)?, length_value(&a[1], units)?))
}

pub(super) fn entity_id(sketch: FeatureId, e: EntityId) -> String {
    refs::to_id(&Reference::SketchEntity { sketch, entity: e.0 })
}

pub(super) fn constraint_id(c: ConstraintId, dimension: bool) -> String {
    format!("{}:{}", if dimension { "dim" } else { "con" }, c.0)
}

fn parse_constraint_id(s: &str) -> Option<ConstraintId> {
    let n = s.strip_prefix("dim:").or_else(|| s.strip_prefix("con:")).unwrap_or(s);
    n.parse().ok().map(ConstraintId)
}

/// A sketch entity argument: `entity:F…:n` of this sketch, `origin` (the
/// sketch's origin point, created on demand like the UI's origin snaps), or
/// model geometry (`edge:…`, `vertex:…`, `face:…`, a plane, another
/// sketch's entity) when `external` allows it: as when the dimension and
/// constraint tools pick it, a linked construction entity stands in
/// (`pierce`: the point where an edge crosses the sketch plane).
pub(super) fn parse_entity(v: &Value, t: &mut SketchTarget, external: bool, pierce: bool) -> Result<EntityId, OpError> {
    let s = v.as_str().ok_or_else(|| OpError::invalid("entity id string"))?;
    if s == "origin" {
        return Ok(sm::origin_point(&mut t.data.sketch));
    }
    let r = refs::parse_str(s);
    match r {
        Some(Reference::SketchEntity { sketch: sk, entity }) if sk == t.id => {
            let id = EntityId(entity);
            t.data.sketch.entity(id).map(|_| id).ok_or_else(|| OpError::not_found(format!("`{s}` is not in the sketch (deleted?)")))
        }
        Some(r @ (Reference::SketchEntity { .. } | Reference::Edge(_) | Reference::Vertex(_) | Reference::Face(_) | Reference::Plane(_) | Reference::FeaturePlane(_))) if external => {
            let pierce = pierce && matches!(r, Reference::Edge(_));
            let bb = t.data.sketch.bbox();
            let cursor = if bb.is_empty() { DVec2::ZERO } else { bb.center() };
            let model = crate::sketch_use::Model { context: &t.regen.context, display: &t.regen.display, frame: &t.frame };
            crate::sketch_use::external_entity(&mut t.data, model, WORLD_PER_PX, cursor, &r, pierce, true)
                .ok_or_else(|| OpError::invalid(format!("`{s}` can't be referenced in this sketch (not found in the model up to the sketch, or parallel to the sketch plane)")))
        }
        Some(Reference::SketchEntity { .. }) => Err(OpError::invalid(format!("`{s}` belongs to another sketch"))),
        _ => Err(OpError::invalid(format!("`{s}` is not a sketch entity id (`entity:…`)"))),
    }
}

// ------------------------------------------------------------ helpers

pub(super) struct SketchTarget {
    pub tab: TabId,
    pub id: FeatureId,
    pub index: usize,
    pub name: String,
    pub data: SketchData,
    pub frame: PlaneFrame,
    /// The Part Studio regenerated up to the sketch, as while editing it.
    pub regen: RegenResult,
    pub units: UnitPrefs,
}


pub(super) fn target(od: &mut OpenDoc, tab: TabId, a: &Args) -> Result<SketchTarget, OpError> {
    let ps = doc::studio(od.doc(), tab)?;
    let id = feature_arg(ps, a, "sketch")?;
    let f = ps.feature(id).expect("found");
    if f.type_id != "sketch" {
        return Err(OpError::param("sketch", &format!("`{}` is a {}, not a sketch", f.name, f.type_id)));
    }
    let index = ps.index_of(id).expect("found");
    let data = match f.params.get("sketch") {
        Some(ParamValue::Sketch(d)) => (**d).clone(),
        _ => SketchData::default(),
    };
    let name = f.name.clone();
    // The sketch mode regenerates up to the sketch being edited.
    let regen = od.regenerate_with(tab, None, |o| {
        o.upto = Some(index + 1);
        o.editing_sketch = Some(id);
    })?;
    let frame = regen.context.sketch(id).map(|s| s.frame).unwrap_or(PlaneFrame::TOP);
    let units = od.doc().settings.units;
    Ok(SketchTarget { tab, id, index, name, data, frame, regen, units })
}

pub(super) fn editor(t: &SketchTarget) -> SketchEditor {
    let mut ed = SketchEditor::new(t.id, t.frame);
    ed.world_per_px = WORLD_PER_PX;
    // Tool sizes (fillet radius, offset, pattern spacing...) as the sketch
    // mode sets them for the workspace unit.
    ed.apply_unit_defaults(t.units.length);
    ed
}

/// Commit the edited sketch data ("Edit : Sketch 1").
pub(super) fn commit(od: &mut OpenDoc, t: SketchTarget) -> Result<(), OpError> {
    let ps = doc::studio(od.doc(), t.tab)?;
    let config = ps.configuration.defaults();
    let mut feature = ps.configuration.for_editing(&ps.features[t.index], &config);
    feature.params.set("sketch", ParamValue::Sketch(Box::new(t.data)));
    edit_studio(od, t.tab, &studio::edit_description(&t.name), |ps| {
        studio::edit_feature(ps, t.index, feature, &config);
        Ok(())
    })
}

fn geometry_json(data: &SketchData, sketch: FeatureId, e: &noshape_sketch::Entity, k: f64) -> Value {
    let p = |id: EntityId| data.sketch.point(id).map(|q| json!([q.x * k, q.y * k])).unwrap_or(Value::Null);
    let eid = |id: EntityId| json!(entity_id(sketch, id));
    let mut v = match &e.geometry {
        Geometry::Point { pos } => json!({ "type": "point", "at": [pos.x * k, pos.y * k] }),
        Geometry::Line { start, end } => json!({ "type": "line", "start": p(*start), "end": p(*end), "start_point": eid(*start), "end_point": eid(*end) }),
        Geometry::Circle { center, radius } => json!({ "type": "circle", "center": p(*center), "radius": radius * k, "center_point": eid(*center) }),
        Geometry::Arc { center, start, end } => json!({ "type": "arc", "center": p(*center), "start": p(*start), "end": p(*end), "center_point": eid(*center), "start_point": eid(*start), "end_point": eid(*end) }),
        Geometry::Ellipse { center, major, minor } => json!({ "type": "ellipse", "center": p(*center), "major": [major.x * k, major.y * k], "minor_radius": minor * k }),
        Geometry::EllipticalArc { center, major, minor, start, end } => json!({ "type": "elliptical_arc", "center": p(*center), "major": [major.x * k, major.y * k], "minor_radius": minor * k, "start_point": eid(*start), "end_point": eid(*end) }),
        Geometry::BSpline { control, degree, .. } => json!({ "type": "bspline", "degree": degree, "control_points": control.iter().map(|c| p(*c)).collect::<Vec<_>>() }),
        Geometry::FitSpline { points, .. } => json!({ "type": "spline", "points": points.iter().map(|c| p(*c)).collect::<Vec<_>>(), "point_ids": points.iter().map(|c| eid(*c)).collect::<Vec<_>>() }),
        Geometry::Conic { start, control, end, rho } => json!({ "type": "conic", "start": p(*start), "control": p(*control), "end": p(*end), "rho": rho }),
    };
    v["id"] = json!(entity_id(sketch, e.id));
    if e.construction {
        v["construction"] = json!(true);
    }
    if e.external {
        v["external"] = json!(true);
    }
    v
}

fn constraint_json(data: &SketchData, sketch: FeatureId, c: &noshape_sketch::Constraint, units: &UnitPrefs) -> Value {
    let dim = c.kind.is_dimension();
    let mut v = json!({
        "id": constraint_id(c.id, dim),
        "type": c.kind.name(),
        "entities": c.kind.entities().iter().map(|e| entity_id(sketch, *e)).collect::<Vec<_>>(),
    });
    if dim {
        if let Some(val) = c.kind.value() {
            let shown = val * sm::dim_display_scale(&c.kind);
            v["value"] = json!(sm::dim_value_text(&c.kind, shown, units));
        }
        v["driving"] = json!(c.driving);
        if let Some(e) = data.dimension_exprs.get(&c.id.0) {
            v["expression"] = json!(e.text);
        }
    }
    v
}

/// What the reply shows after a sketch edit: new entities, solve state, regions.
pub(super) fn sketch_state(od: &mut OpenDoc, tab: TabId, id: FeatureId, before: Option<&[EntityId]>) -> Result<Value, OpError> {
    let units = od.doc().settings.units;
    let k = crate::ops::query::len_scale(&units);
    let ps = doc::studio(od.doc(), tab)?;
    let data = match ps.feature(id).and_then(|f| f.params.get("sketch")) {
        Some(ParamValue::Sketch(d)) => (**d).clone(),
        _ => SketchData::default(),
    };
    let r = od.regenerate(tab, None)?;
    let mut v = json!({ "sketch": id.to_string(), "units": crate::ops::query::units_json(&units) });
    let entities: Vec<Value> = data.sketch.entities().filter(|e| before.is_none_or(|b| !b.contains(&e.id))).map(|e| geometry_json(&data, id, e, k)).collect();
    v[if before.is_some() { "created" } else { "entities" }] = json!(entities);
    if before.is_none() {
        v["constraints"] = json!(data.sketch.constraints().map(|c| constraint_json(&data, id, c, &units)).collect::<Vec<_>>());
        if !data.patterns.is_empty() {
            v["patterns"] = super::sketch_edit::patterns_json(&data, id, k);
        }
        if !data.texts.is_empty() {
            v["texts"] = json!(data.texts.iter().map(|t| json!({ "text": t.text, "font": t.font, "box_lines": t.box_lines.iter().map(|l| entity_id(id, EntityId(*l))).collect::<Vec<_>>() })).collect::<Vec<_>>());
        }
        if !data.images.is_empty() {
            v["images"] = json!(data.images.iter().map(|i| json!({ "name": i.name, "box_lines": i.box_lines.iter().map(|l| entity_id(id, EntityId(*l))).collect::<Vec<_>>() })).collect::<Vec<_>>());
        }
    }
    if let Some(sr) = r.context.sketch(id) {
        let status = match &sr.report.status {
            noshape_sketch::SolveStatus::Solved => {
                if sr.report.dof == Some(0) {
                    "fully defined".to_string()
                } else {
                    "under defined".to_string()
                }
            }
            noshape_sketch::SolveStatus::Overconstrained { .. } => "over defined (conflicting constraints)".to_string(),
            noshape_sketch::SolveStatus::Failed { reason } => format!("failed: {reason}"),
        };
        v["status"] = json!(status);
        v["dof"] = json!(sr.report.dof);
        let disp = r.display.sketches.iter().find(|d| d.feature == id);
        v["regions"] = json!(sr
            .regions
            .iter()
            .map(|g| {
                let key = noshape_model::regen::eval::encode_region_key(&g.id);
                let area = disp.and_then(|d| d.regions.iter().find(|x| x.key == key)).map(|x| x.area * k * k);
                let mut rv = json!({ "id": refs::to_id(&Reference::SketchRegion { sketch: id, key: key.clone() }) });
                if let Some(a) = area {
                    rv["area"] = json!((a * 1e6).round() / 1e6);
                }
                let b = g.outer.bbox();
                let c = (b.min + b.max) * 0.5;
                rv["center"] = json!([c.x * k, c.y * k]);
                rv["holes"] = json!(g.holes.len());
                rv
            })
            .collect::<Vec<_>>());
        let fr = sr.frame;
        v["frame"] = json!({ "origin": crate::ops::query::pt(fr.origin, k), "x": [fr.x.x, fr.x.y, fr.x.z], "y": [fr.y.x, fr.y.y, fr.y.z], "normal": [fr.normal.x, fr.normal.y, fr.normal.z] });
    }
    if let Some(st) = r.display.feature_state(id)
        && let Some(m) = &st.message
    {
        v["message"] = json!(m);
    }
    Ok(v)
}

// ------------------------------------------------------------ handlers

fn create(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = studio_target(ctx, a)?;
    let units = od.doc().settings.units;
    let plane = refs::parse(a.get("plane").expect("required")).map_err(|e| e.with_param("plane"))?;
    let ps = doc::studio(od.doc(), tab)?;
    let mut f = ps.new_feature("sketch", &units).expect("sketch type");
    f.params.set("sketchPlane", ParamValue::Query(vec![plane]));
    if let Some(n) = a.opt_str("name")? {
        f.name = n.trim().to_string();
        f.custom_name = true;
    }
    let (id, name) = (f.id, f.name.clone());
    let index = ps.rollback_index();
    edit_studio(od, tab, &studio::insert_description(&name), |ps| Ok(studio::insert_feature(ps, f, index)))?;
    let mut v = sketch_state(od, tab, id, None)?;
    v["name"] = json!(name);
    let report = regen_report(od, tab)?;
    if let Some(st) = report["features"].as_array().and_then(|fs| fs.iter().find(|x| x["id"] == id.to_string())) {
        v["feature_status"] = st.clone();
    }
    Ok(v.into())
}

/// Move to `p` and click (press and release in place), as the editor's
/// pointer events arrive from the viewport.
pub(super) fn click(ed: &mut SketchEditor, data: &mut SketchData, p: DVec2) {
    ed.pointer_move(data, p, false, false);
    let h = data.sketch.pick(p, WORLD_PER_PX * 4.0);
    ed.hover = h;
    ed.tool_down(data, p, h, false, false);
    ed.tool_up(data, p);
}

fn draw(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = studio_target(ctx, a)?;
    let units = od.doc().settings.units;
    let mut t = target(od, tab, a)?;
    let name = a.str("tool")?;
    let tool = DRAW_TOOLS.iter().find(|x| x.0 == name).map(|x| x.1).ok_or_else(|| OpError::param("tool", "unknown drawing tool"))?;
    let points = a.array("points")?.iter().enumerate().map(|(i, p)| point_value(p, &units).map_err(|e| e.with_param(format!("points[{i}]")))).collect::<Result<Vec<_>, _>>()?;
    if points.is_empty() {
        return Err(OpError::param("points", "at least one click"));
    }
    let before: Vec<EntityId> = t.data.sketch.entities().map(|e| e.id).collect();
    let mut ed = editor(&t);
    ed.set_tool(tool);
    if let Some(n) = a.opt_u64("sides")? {
        ed.polygon_sides = n.clamp(3, 1000) as usize;
    }
    for p in &points {
        click(&mut ed, &mut t.data, *p);
    }
    // Commit what the tool still holds (spline, slot, polygon), then leave it.
    let _ = ed.escape_commit(&mut t.data);
    if ed.polygon_sides_pending() {
        ed.finish_polygon(&mut t.data);
    }
    ed.cancel();
    let created: Vec<EntityId> = t.data.sketch.entities().map(|e| e.id).filter(|e| !before.contains(e)).collect();
    if created.is_empty() {
        let hint = ed.message.clone().unwrap_or_else(|| format!("the clicks didn't complete a {name} (see sketch_draw's tool list for the clicks it takes)"));
        return Err(OpError::invalid(format!("nothing was drawn: {hint}")).with_param("points"));
    }
    if a.bool_or("construction", false)? {
        for e in &created {
            let _ = t.data.sketch.set_construction(*e, true);
        }
        t.data.sketch.solve();
    }
    let (id, tab) = (t.id, t.tab);
    commit(od, t)?;
    Ok(sketch_state(od, tab, id, Some(&before))?.into())
}

fn constrain(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = studio_target(ctx, a)?;
    let mut t = target(od, tab, a)?;
    let name = a.str("constraint")?;
    let c = CONSTRAINTS.iter().find(|x| x.0 == name).map(|x| x.1).ok_or_else(|| OpError::param("constraint", "unknown constraint"))?;
    let pierce = c == ConstraintTool::Pierce;
    let ids = a.array("entities")?.iter().enumerate().map(|(i, v)| parse_entity(v, &mut t, true, pierce).map_err(|e| e.with_param(format!("entities[{i}]")))).collect::<Result<Vec<_>, _>>()?;
    let before: Vec<ConstraintId> = t.data.sketch.constraints().map(|c| c.id).collect();
    let mut ed = editor(&t);
    ed.selection = ids;
    if !ed.constraint_command(&mut t.data, c) {
        let why = ed.message.clone().unwrap_or_else(|| format!("{name} doesn't apply to these entities"));
        return Err(OpError::invalid(why).with_param("entities"));
    }
    let added: Vec<String> = t.data.sketch.constraints().filter(|c| !before.contains(&c.id)).map(|c| constraint_id(c.id, false)).collect();
    let (id, tab) = (t.id, t.tab);
    commit(od, t)?;
    let mut v = sketch_state(od, tab, id, Some(&[]))?;
    v.as_object_mut().expect("object").remove("created");
    v["added"] = json!(added);
    Ok(v.into())
}

fn dimension_kind(kind: &str, ids: &[EntityId], mode: DistanceMode, data: &SketchData, label: Option<DVec2>) -> Result<ConstraintKind, OpError> {
    let one = || ids.first().copied().ok_or_else(|| OpError::param("entities", "one entity"));
    let two = || match ids {
        [a, b] => Ok((*a, *b)),
        _ => Err(OpError::param("entities", "two entities")),
    };
    Ok(match kind {
        "auto" => {
            let cursor = label.or_else(|| {
                let pts: Vec<DVec2> = ids.iter().filter_map(|e| entity_anchor(&data.sketch, *e)).collect();
                (!pts.is_empty()).then(|| pts.iter().sum::<DVec2>() / pts.len() as f64 + DVec2::new(1e-3, 1e-3))
            });
            let c = cursor.ok_or_else(|| OpError::param("entities", "no geometry to dimension"))?;
            sm::smart_dimension(&data.sketch, ids, c).ok_or_else(|| OpError::param("entities", "the Dimension tool can't dimension these entities; give `kind`"))?
        }
        "distance" => {
            let (a, b) = two()?;
            ConstraintKind::Distance { a, b, value: 0.0, mode }
        }
        "centerline" => {
            let (a, b) = two()?;
            ConstraintKind::Distance { a, b, value: 0.0, mode: DistanceMode::Centerline }
        }
        "horizontal_distance" => {
            let (a, b) = two()?;
            ConstraintKind::HorizontalDistance { a, b, value: 0.0 }
        }
        "vertical_distance" => {
            let (a, b) = two()?;
            ConstraintKind::VerticalDistance { a, b, value: 0.0 }
        }
        "length" => ConstraintKind::Length { curve: one()?, value: 0.0 },
        "radius" => ConstraintKind::Radius { curve: one()?, value: 0.0 },
        "diameter" => ConstraintKind::Diameter { curve: one()?, value: 0.0 },
        "angle" => {
            let (a, b) = two()?;
            ConstraintKind::Angle { a, b, value: 0.0 }
        }
        "arc_angle" => ConstraintKind::ArcAngle { arc: one()?, value: 0.0 },
        _ => return Err(OpError::param("kind", "unknown dimension kind")),
    })
}

/// Where a dimension label goes by default: a point, or a curve's middle.
fn entity_anchor(sk: &noshape_sketch::Sketch, e: EntityId) -> Option<DVec2> {
    if let Some(p) = sk.point(e) {
        return Some(p);
    }
    let pts: Vec<DVec2> = sk.entity(e)?.point_refs().iter().filter_map(|p| sk.point(*p)).collect();
    (!pts.is_empty()).then(|| pts.iter().sum::<DVec2>() / pts.len() as f64)
}

pub(super) fn value_text(v: &Value) -> Result<String, OpError> {
    match v {
        Value::Number(n) => Ok(n.to_string()),
        Value::String(s) if !s.trim().is_empty() => Ok(s.trim().to_string()),
        _ => Err(OpError::param("value", "a value or expression")),
    }
}

/// Variables visible to a sketch's dimensions (Part Studio, Variable Studio
/// and configuration variables at the sketch).
pub(super) fn variables_at(od: &mut OpenDoc, tab: TabId) -> Result<std::collections::BTreeMap<String, noshape_units::Quantity>, OpError> {
    // As regeneration layers them: Variable Studio and configuration
    // variables, then the Part Studio's own.
    let config = od.current_config(tab)?;
    let units = od.doc().settings.units;
    let mut vars = od.regen_options(tab).document_variables;
    vars.extend(doc::studio(od.doc(), tab)?.configuration.variable_values(&config, &units));
    vars.extend(od.regenerate(tab, None)?.context.variables.clone());
    Ok(vars)
}

fn dimension(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = studio_target(ctx, a)?;
    let units = od.doc().settings.units;
    let vars = variables_at(od, tab)?;
    let mut t = target(od, tab, a)?;
    let ids = a.array("entities")?.iter().enumerate().map(|(i, v)| parse_entity(v, &mut t, true, false).map_err(|e| e.with_param(format!("entities[{i}]")))).collect::<Result<Vec<_>, _>>()?;
    let mode = match a.opt_str("mode")? {
        None | Some("center") => DistanceMode::Center,
        Some("minimum") => DistanceMode::Minimum,
        Some("maximum") => DistanceMode::Maximum,
        Some(_) => return Err(OpError::param("mode", "center, minimum or maximum")),
    };
    let label = match a.get("label") {
        Some(l) => Some(point_value(l, &units).map_err(|e| e.with_param("label"))?),
        None => None,
    };
    let kind = dimension_kind(a.opt_str("kind")?.unwrap_or("auto"), &ids, mode, &t.data, label)?;
    let text = value_text(a.get("value").expect("required"))?;
    let cid = sm::add_dimension(&mut t.data, kind, label, None).ok_or_else(|| OpError::invalid("the dimension doesn't apply to these entities").with_param("entities"))?;
    let driving = t.data.sketch.constraint(cid).is_some_and(|c| c.driving);
    if driving {
        let mut ed = editor(&t);
        ed.apply_dimension(&mut t.data, cid, &text, &units, &vars).map_err(|m| OpError::invalid(m).with_param("value"))?;
    }
    let (id, tab) = (t.id, t.tab);
    commit(od, t)?;
    let mut v = sketch_state(od, tab, id, Some(&[]))?;
    v.as_object_mut().expect("object").remove("created");
    v["dimension"] = json!(constraint_id(cid, true));
    if !driving {
        v["warning"] = json!("The dimension would over-constrain the sketch, so it was added as a driven (reference) dimension, as in Onshape.");
    }
    Ok(v.into())
}

fn set_dimension(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = studio_target(ctx, a)?;
    let units = od.doc().settings.units;
    let vars = variables_at(od, tab)?;
    let mut t = target(od, tab, a)?;
    let cid = parse_constraint_id(a.str("dimension")?).filter(|c| t.data.sketch.constraint(*c).is_some_and(|c| c.kind.is_dimension())).ok_or_else(|| OpError::param("dimension", "not a dimension of this sketch"))?;
    let text = value_text(a.get("value").expect("required"))?;
    let mut ed = editor(&t);
    ed.apply_dimension(&mut t.data, cid, &text, &units, &vars).map_err(|m| OpError::invalid(m).with_param("value"))?;
    let (id, tab) = (t.id, t.tab);
    commit(od, t)?;
    let mut v = sketch_state(od, tab, id, Some(&[]))?;
    v.as_object_mut().expect("object").remove("created");
    Ok(v.into())
}

fn delete(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = studio_target(ctx, a)?;
    let mut t = target(od, tab, a)?;
    let mut ed = editor(&t);
    if let Some(list) = a.opt_array("entities")? {
        ed.selection = list.iter().enumerate().map(|(i, v)| parse_entity(v, &mut t, false, false).map_err(|e| e.with_param(format!("entities[{i}]")))).collect::<Result<Vec<_>, _>>()?;
    }
    if let Some(list) = a.opt_array("constraints")? {
        ed.selected_constraints = list
            .iter()
            .enumerate()
            .map(|(i, v)| v.as_str().and_then(parse_constraint_id).filter(|c| t.data.sketch.constraint(*c).is_some()).ok_or_else(|| OpError::param(&format!("constraints[{i}]"), "not a constraint of this sketch")))
            .collect::<Result<Vec<_>, _>>()?;
    }
    if !ed.delete_selection(&mut t.data) {
        return Err(OpError::invalid(ed.message.clone().unwrap_or_else(|| "nothing to delete".into())));
    }
    let (id, tab) = (t.id, t.tab);
    commit(od, t)?;
    Ok(sketch_state(od, tab, id, None)?.into())
}

fn set_construction(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = studio_target(ctx, a)?;
    let mut t = target(od, tab, a)?;
    let on = a.bool_or("construction", true)?;
    let ids = a.array("entities")?.iter().enumerate().map(|(i, v)| parse_entity(v, &mut t, false, false).map_err(|e| e.with_param(format!("entities[{i}]")))).collect::<Result<Vec<_>, _>>()?;
    for e in &ids {
        t.data.sketch.set_construction(*e, on).map_err(|e| OpError::invalid(e.to_string()))?;
    }
    t.data.sketch.solve();
    let (id, tab) = (t.id, t.tab);
    commit(od, t)?;
    Ok(sketch_state(od, tab, id, None)?.into())
}

fn get(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = studio_target(ctx, a)?;
    let t = target(od, tab, a)?;
    let mut v = sketch_state(od, tab, t.id, None)?;
    v["name"] = json!(t.name);
    Ok(v.into())
}
