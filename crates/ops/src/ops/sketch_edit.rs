//! Sketch edit tools: trim, extend, split, spline control points, fillet,
//! chamfer, offset, mirror, linear / circular patterns, transform, Use
//! (project model geometry), Intersection, text, and the Insert DXF or DWG /
//! Insert image imports. Each drives the sketch editor's own tool with the
//! clicks and values a person gives it, and commits one "Edit : Sketch N".

use super::sketch::{SketchTarget, WORLD_PER_PX, click, commit, editor, entity_id, parse_entity, point_value, sketch_state, target, value_text, variables_at};
use super::{doc_field, s, studio_target, tab_field};
use crate::error::OpError;
use crate::registry::{Args, Op, OpOutput, opt, req};
use crate::{Ctx, OpenDoc, refs, sketch_use};
use glam::DVec2;
use noshape_scene::sketch_mode::{self as sm, PatternHandle, SketchEditor, Tool};
use noshape_sketch::{EntityId, EntityKind};
use noshape_units::{Dim, Quantity};
use serde_json::{Value, json};
use std::collections::BTreeMap;

fn point() -> Value {
    json!({ "type": "array", "items": { "type": ["number", "string"] }, "minItems": 2, "maxItems": 2 })
}

fn points() -> Value {
    json!({ "type": "array", "items": point(), "minItems": 1 })
}

fn ids() -> Value {
    json!({ "type": "array", "items": { "type": "string" }, "minItems": 1 })
}

fn value() -> Value {
    json!({ "type": ["string", "number"] })
}

fn sketch_field() -> crate::registry::Field {
    req("sketch", s(), "Sketch feature id or name.")
}

fn op(verb: &'static str, summary: &'static str, description: &'static str, fields: Vec<crate::registry::Field>, run: crate::registry::Handler) -> Op {
    let mut all = vec![doc_field(), tab_field(), sketch_field()];
    all.extend(fields);
    Op { group: "sketch", verb, summary, description, fields: all, mutates: true, destructive: matches!(verb, "trim" | "split"), run }
}

pub fn ops() -> Vec<Op> {
    vec![
        op(
            "trim",
            "Trim sketch curves (the Trim tool)",
            "Each click in `points` removes the piece of the curve under it, up to the nearest intersections (a curve crossing nothing is deleted; a lone point is deleted). With `path`, the pointer is dragged along the points instead and every curve the path crosses is trimmed. Give `entities` (same length as `points`) to say which curve each click is on; otherwise the curve within 6 µm of the click is used.",
            vec![opt("points", points(), "Clicks on the pieces to remove, in sketch coordinates."), opt("entities", json!({ "type": "array", "items": { "type": "string" } }), "The curve under each click (optional)."), opt("path", points(), "Drag path (2 or more points): trims every curve it crosses.")],
            trim,
        ),
        op(
            "extend",
            "Extend lines or arcs to the next curve (the Extend tool)",
            "Each click extends the line or arc under it, from its end nearest the click, to the next curve it meets. `entities` (optional, same length) names the curve under each click.",
            vec![req("points", points(), "Clicks near the end to extend."), opt("entities", json!({ "type": "array", "items": { "type": "string" } }), "The curve under each click (optional).")],
            extend,
        ),
        op(
            "split",
            "Split sketch curves at points (the Split tool)",
            "Each click splits the curve under it at the closest point on the curve. `entities` (optional, same length) names the curve under each click.",
            vec![req("points", points(), "Split positions."), opt("entities", json!({ "type": "array", "items": { "type": "string" } }), "The curve under each click (optional).")],
            split,
        ),
        op(
            "spline_point",
            "Add a control point to a spline or bezier",
            "The Spline control point tool: inserts a point into the spline / bezier at the click.",
            vec![req("entity", s(), "Spline or bezier entity id."), req("point", point(), "Where on the curve (or its control polygon).")],
            spline_point,
        ),
        op(
            "fillet",
            "Fillet sketch corners (the Sketch fillet tool)",
            "Each corner point in `entities` is filleted; two curves given one after the other are filleted where they meet (the tool's two-curve mode). All fillets of one call are one tool session: the first gets a radius dimension set to `radius` (a value or expression), later ones are constrained equal to it, as in Onshape.",
            vec![req("entities", ids(), "Corner points (entity ids), or pairs of curves."), opt("radius", value(), "Radius (default: the tool's, 20 mm / 0.5 in).")],
            fillet,
        ),
        op(
            "chamfer",
            "Chamfer sketch corners (the Sketch chamfer tool)",
            "Like sketch_fillet, with an equal-distance chamfer: each corner point (or pair of curves) gets a chamfer line, the first with distance dimensions set to `distance`.",
            vec![req("entities", ids(), "Corner points, or pairs of curves."), opt("distance", value(), "Chamfer distance (default: the tool's, 5 mm / 0.25 in).")],
            chamfer,
        ),
        op(
            "offset",
            "Offset sketch curves (the Offset tool)",
            "Offsets the curves (a connected chain offsets as one) by `distance`. The default side is the one the tool's arrow shows first; `toward` picks the side whose offset passes nearer that point, `flip` flips the side.",
            vec![req("entities", ids(), "Curves to offset."), opt("distance", value(), "Offset distance (default: the tool's, 5 mm / 0.25 in)."), opt("toward", point(), "A point on the side to offset to."), opt("flip", json!({ "type": "boolean" }), "Offset to the other side.")],
            offset,
        ),
        op(
            "mirror",
            "Mirror sketch entities about a line (the Mirror tool)",
            "Copies the entities mirrored about `axis` and adds symmetric constraints. The axis is a sketch line, or model geometry (an edge, a plane) whose projection is used.",
            vec![req("entities", ids(), "Entities to mirror."), req("axis", s(), "Mirror line: a sketch line id, or an edge / plane id.")],
            mirror,
        ),
        op(
            "linear_pattern",
            "Pattern sketch entities in a line or grid (the sketch Linear pattern tool)",
            "Copies the entities `count` times (the seed included) `distance` apart along `angle`, optionally in a second direction (`second_count`, `second_distance`, `second_angle`, default 90 deg). Counts are 1 to 100. Give `pattern` (an id from the reply or sketch_get) to edit an existing pattern instead.",
            vec![
                opt("entities", ids(), "Seed entities (not needed when editing)."),
                opt("count", json!({ "type": "integer", "minimum": 1, "maximum": 100 }), "Instances in the first direction (default 3)."),
                opt("distance", value(), "Spacing (default 25 mm / 1 in)."),
                opt("angle", value(), "First direction (default 0 deg: +x)."),
                opt("second_count", json!({ "type": "integer", "minimum": 1, "maximum": 100 }), "Instances in the second direction (default 1)."),
                opt("second_distance", value(), "Second spacing."),
                opt("second_angle", value(), "Second direction (default 90 deg)."),
                opt("pattern", json!({ "type": "integer" }), "Pattern id to edit."),
            ],
            linear_pattern,
        ),
        op(
            "circular_pattern",
            "Pattern sketch entities around a center (the sketch Circular pattern tool)",
            "Copies the entities around `center`: `count` instances (the seed included, 2 to 100) over `angle` (default 360 deg, spread evenly; a partial angle puts the last instance at the angle). Give `pattern` to edit an existing pattern.",
            vec![
                opt("entities", ids(), "Seed entities (not needed when editing)."),
                opt("center", json!({ "type": ["array", "string"] }), "Center: [x, y], a point entity id or `origin` (default: the origin)."),
                opt("count", json!({ "type": "integer", "minimum": 2, "maximum": 100 }), "Instances (default 3)."),
                opt("angle", value(), "Total angle (default 360 deg)."),
                opt("pattern", json!({ "type": "integer" }), "Pattern id to edit."),
            ],
            circular_pattern,
        ),
        op(
            "transform",
            "Move, rotate or scale sketch entities (the Transform tool)",
            "Rotates by `rotate` and scales by `scale` about `center` (default: the selection's center), then translates by `translate`, as the tool's triad does.",
            vec![req("entities", ids(), "Entities to transform."), opt("translate", point(), "Translation [dx, dy]."), opt("rotate", value(), "Rotation angle (counter-clockwise)."), opt("scale", json!({ "type": "number", "exclusiveMinimum": 0 }), "Scale factor."), opt("center", point(), "Pivot [x, y].")],
            transform,
        ),
        op(
            "use",
            "Use (project) model geometry into the sketch (the Use tool)",
            "Projects each reference onto the sketch plane as fixed geometry linked to the model (it follows model changes): edges and vertices (`edge:…`, `vertex:…` from query_entities), other sketches' entities, a planar face (all its edges), a curved face (its silhouettes) or a plane (its trace line). Returns the created entity ids.",
            vec![req("references", ids(), "Model reference ids.")],
            use_refs,
        ),
        op(
            "intersection",
            "Add a face's intersection with the sketch plane (the Intersection tool)",
            "Sections the face with the sketch plane; the curves stay linked to the face.",
            vec![req("face", s(), "Face id (`face:…`) crossing the sketch plane.")],
            intersection,
        ),
        op(
            "text",
            "Add or edit sketch text (the Text tool)",
            "Places a text box with its lower-left corner at `corner` and `height` (default 10 mm), with the Text dialog's options; or, with `edit` (an entity of an existing text), changes that text. Text is limited to 250 characters.",
            vec![
                opt("text", s(), "The text (default \"Default text\" for a new box)."),
                opt("corner", point(), "Lower-left corner (new text)."),
                opt("height", value(), "Height of the first line (new text)."),
                opt("font", s(), "Font family."),
                opt("bold", json!({ "type": "boolean" }), "Bold."),
                opt("italic", json!({ "type": "boolean" }), "Italic."),
                opt("flip_horizontal", json!({ "type": "boolean" }), "Mirror horizontally."),
                opt("flip_vertical", json!({ "type": "boolean" }), "Mirror vertically."),
                opt("expression", json!({ "type": "boolean" }), "Convert to expression (`#var` in the text shows its value)."),
                opt("edit", s(), "An entity id of the text to edit."),
            ],
            text,
        ),
        op(
            "insert_dxf",
            "Import a DXF or DWG drawing into the sketch (Insert DXF or DWG)",
            "Adds the drawing's geometry, in the file's units unless `units` is given, centered on the sketch origin unless `use_origin`.",
            vec![
                req("file", s(), "Path of the .dxf / .dwg file."),
                opt("units", json!({ "type": "string", "enum": sm::DXF_UNITS.iter().map(|u| u.0).collect::<Vec<_>>() }), "Drawing units (default \"From file\"; millimeters when the file has none)."),
                opt("use_origin", json!({ "type": "boolean" }), "Keep the file's origin at the sketch origin."),
            ],
            insert_dxf,
        ),
        op(
            "insert_image",
            "Import an image into the sketch (Insert image)",
            "Places a PNG, JPEG, GIF or BMP image in the rectangle from the first corner toward the second (the aspect ratio is kept, as when drawing the rectangle).",
            vec![req("file", s(), "Path of the image."), req("points", json!({ "type": "array", "items": point(), "minItems": 2, "maxItems": 2 }), "[corner, opposite corner].")],
            insert_image,
        ),
    ]
}

// ------------------------------------------------------------ helpers

/// A value with units (expression; variables visible to the sketch), SI out.
fn quantity(v: &Value, dim: Dim, t: &SketchTarget, vars: &BTreeMap<String, Quantity>, param: &str) -> Result<f64, OpError> {
    let text = value_text(v).map_err(|e| e.with_param(param))?;
    noshape_units::evaluate_as(&text, dim, &t.units, vars).map(|q| q.value).map_err(|e| OpError::param(param, &format!("`{text}`: {e}")))
}

fn entity_list(a: &Args, name: &str, t: &mut SketchTarget, external: bool) -> Result<Vec<EntityId>, OpError> {
    a.array(name)?.iter().enumerate().map(|(i, v)| parse_entity(v, t, external, false).map_err(|e| e.with_param(format!("{name}[{i}]")))).collect()
}

fn point_list(a: &Args, name: &str, t: &SketchTarget) -> Result<Vec<DVec2>, OpError> {
    a.array(name)?.iter().enumerate().map(|(i, p)| point_value(p, &t.units).map_err(|e| e.with_param(format!("{name}[{i}]")))).collect()
}

/// Commit and reply with the new entities, solve state and regions; fails
/// with the tool's message when the edit changed nothing.
fn finish(od: &mut OpenDoc, t: SketchTarget, ed: &SketchEditor, changed: bool, before: &[EntityId]) -> Result<OpOutput, OpError> {
    if !changed {
        return Err(OpError::invalid(ed.message.clone().unwrap_or_else(|| "the tool changed nothing".into())));
    }
    let (id, tab) = (t.id, t.tab);
    commit(od, t)?;
    let mut v = sketch_state(od, tab, id, Some(before))?;
    if let Some(m) = &ed.message {
        v["tool_message"] = json!(m);
    }
    Ok(v.into())
}

fn existing(t: &SketchTarget) -> Vec<EntityId> {
    t.data.sketch.entities().map(|e| e.id).collect()
}

/// Clicks of the trim / extend / split tools: the hovered curve is the
/// named entity, or what lies under the click.
fn curve_clicks(ctx: &mut Ctx, a: &Args, tool: Tool) -> Result<OpOutput, OpError> {
    let (od, tab) = studio_target(ctx, a)?;
    let mut t = target(od, tab, a)?;
    let before = existing(&t);
    let pts = if a.get("points").is_some() { point_list(a, "points", &t)? } else { Vec::new() };
    let hovered: Vec<Option<EntityId>> = match a.opt_array("entities")? {
        Some(list) => {
            if list.len() != pts.len() {
                return Err(OpError::param("entities", "one entity per click in `points`"));
            }
            list.iter().enumerate().map(|(i, v)| parse_entity(v, &mut t, false, false).map(Some).map_err(|e| e.with_param(format!("entities[{i}]")))).collect::<Result<_, _>>()?
        }
        None => vec![None; pts.len()],
    };
    let path = if a.get("path").is_some() { point_list(a, "path", &t)? } else { Vec::new() };
    if pts.is_empty() && path.len() < 2 {
        return Err(OpError::param("points", "at least one click (or a `path` of 2 or more points)"));
    }
    let mut ed = editor(&t);
    ed.set_tool(tool);
    let mut changed = false;
    for (p, h) in pts.iter().zip(hovered) {
        ed.pointer_move(&mut t.data, *p, false, false);
        let h = h.or_else(|| t.data.sketch.pick(*p, WORLD_PER_PX * 4.0));
        ed.hover = h;
        changed |= ed.tool_down(&mut t.data, *p, h, false, false).changed;
        changed |= ed.tool_up(&mut t.data, *p).changed;
    }
    if let [first, rest @ ..] = path.as_slice() {
        ed.pointer_move(&mut t.data, *first, false, false);
        let h = t.data.sketch.pick(*first, WORLD_PER_PX * 4.0);
        ed.hover = h;
        changed |= ed.tool_down(&mut t.data, *first, h, false, false).changed;
        for p in rest {
            changed |= ed.tool_drag(&mut t.data, *p).changed;
        }
        changed |= ed.tool_up(&mut t.data, *path.last().expect("path")).changed;
    }
    let mut out = finish(od, t, &ed, changed, &before)?;
    out.value.as_object_mut().expect("object").remove("tool_message");
    Ok(out)
}

// ------------------------------------------------------------ handlers

fn trim(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    curve_clicks(ctx, a, Tool::Trim)
}

fn extend(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    curve_clicks(ctx, a, Tool::Extend)
}

fn split(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    curve_clicks(ctx, a, Tool::Split)
}

fn spline_point(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = studio_target(ctx, a)?;
    let mut t = target(od, tab, a)?;
    let before = existing(&t);
    let e = parse_entity(a.get("entity").expect("required"), &mut t, false, false).map_err(|e| e.with_param("entity"))?;
    if !matches!(t.data.sketch.kind(e), Some(EntityKind::FitSpline | EntityKind::BSpline)) {
        return Err(OpError::param("entity", "not a spline or bezier"));
    }
    let p = point_value(a.get("point").expect("required"), &t.units).map_err(|e| e.with_param("point"))?;
    let mut ed = editor(&t);
    ed.set_tool(Tool::SplinePoint);
    let changed = ed.tool_down(&mut t.data, p, Some(e), false, false).changed;
    finish(od, t, &ed, changed, &before)
}

/// Fillet / chamfer: corner clicks (points) or curve pairs, each previewed
/// and confirmed, in one tool session; the first one's value box takes the
/// size (`dim_edit`), as typing it does.
fn corners(ctx: &mut Ctx, a: &Args, tool: Tool, size: &str) -> Result<OpOutput, OpError> {
    let (od, tab) = studio_target(ctx, a)?;
    let units = od.doc().settings.units;
    let vars = variables_at(od, tab)?;
    let mut t = target(od, tab, a)?;
    let before = existing(&t);
    let picks = entity_list(a, "entities", &mut t, false)?;
    let size_text = match a.get(size) {
        Some(v) => {
            let m = quantity(v, Dim::LENGTH, &t, &vars, size)?;
            if m <= 0.0 {
                return Err(OpError::param(size, "must be greater than zero"));
            }
            Some((value_text(v)?, m))
        }
        None => None,
    };
    let mut ed = editor(&t);
    ed.set_tool(tool);
    if let Some((_, m)) = &size_text {
        if tool == Tool::Fillet {
            ed.fillet_radius = *m;
        } else {
            ed.chamfer_distance = *m;
        }
    }
    let mut changed = false;
    let mut first_dim = None;
    let mut pending_curve = false;
    for (i, e) in picks.iter().enumerate() {
        let p = t.data.sketch.point(*e).or_else(|| t.data.sketch.curve(*e).map(|c| c.point({
            let (lo, hi) = c.domain();
            (lo + hi) * 0.5
        }))).unwrap_or_default();
        let is_point = t.data.sketch.kind(*e) == Some(EntityKind::Point);
        if !is_point {
            pending_curve = !pending_curve;
        } else if pending_curve {
            return Err(OpError::param(&format!("entities[{i}]"), "a curve was given without its second curve"));
        }
        ed.hover = Some(*e);
        ed.tool_down(&mut t.data, p, Some(*e), false, false);
        if ed.corner_pending() {
            let out = ed.commit_corner(&mut t.data);
            changed |= out.changed;
            if first_dim.is_none() {
                first_dim = ed.dim_edit.as_ref().map(|d| d.constraint);
            }
        } else if is_point || !pending_curve {
            let why = ed.message.clone().unwrap_or_else(|| "this corner can't be filleted".into());
            return Err(OpError::param(&format!("entities[{i}]"), &why));
        }
    }
    if pending_curve {
        return Err(OpError::param("entities", "curves are filleted in pairs; the last curve has no partner"));
    }
    // The value box of the first fillet / chamfer takes what was typed.
    if let (Some(cid), Some((text, _))) = (first_dim, &size_text) {
        ed.apply_dimension(&mut t.data, cid, text, &units, &vars).map_err(|m| OpError::param(size, &m))?;
    }
    ed.dim_edit = None;
    ed.message = None;
    finish(od, t, &ed, changed, &before)
}

fn fillet(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    corners(ctx, a, Tool::Fillet, "radius")
}

fn chamfer(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    corners(ctx, a, Tool::Chamfer, "distance")
}

fn offset(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = studio_target(ctx, a)?;
    let vars = variables_at(od, tab)?;
    let mut t = target(od, tab, a)?;
    let before = existing(&t);
    let curves = entity_list(a, "entities", &mut t, false)?;
    if let Some(i) = curves.iter().position(|e| t.data.sketch.kind(*e) == Some(EntityKind::Point)) {
        return Err(OpError::param(&format!("entities[{i}]"), "points can't be offset; give curves"));
    }
    let mut ed = editor(&t);
    ed.set_tool(Tool::Offset);
    if let Some(v) = a.get("distance") {
        let d = quantity(v, Dim::LENGTH, &t, &vars, "distance")?;
        if d <= 0.0 {
            return Err(OpError::param("distance", "must be greater than zero (use `toward` or `flip` for the side)"));
        }
        ed.offset_distance = d;
    }
    // Select the curves, then the tool (the preselection becomes the offset).
    ed.selection = curves;
    ed.apply_preselection(&mut t.data);
    if let Some(p) = a.get("toward") {
        let p = point_value(p, &t.units).map_err(|e| e.with_param("toward"))?;
        // The side whose offset curves pass nearer the point (clicking the
        // arrow flips it to the other side).
        let pending = ed.offset_pending.clone().unwrap_or_default();
        let gap = |flip: bool| -> f64 {
            let mut tmp = t.data.sketch.clone();
            let d = if flip { -ed.offset_distance } else { ed.offset_distance };
            match tmp.offset(&pending, d) {
                Ok(r) => r.created.iter().filter_map(|e| tmp.curve(*e)).map(|c| c.closest(p).2).fold(f64::INFINITY, f64::min),
                Err(_) => f64::INFINITY,
            }
        };
        if gap(!ed.offset_flip) < gap(ed.offset_flip) {
            ed.offset_flip = !ed.offset_flip;
        }
    }
    if a.bool_or("flip", false)? {
        ed.offset_flip = !ed.offset_flip;
    }
    let changed = ed.commit_offset(&mut t.data).changed;
    finish(od, t, &ed, changed, &before)
}

fn mirror(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = studio_target(ctx, a)?;
    let mut t = target(od, tab, a)?;
    let before = existing(&t);
    let list = entity_list(a, "entities", &mut t, false)?;
    let axis = parse_entity(a.get("axis").expect("required"), &mut t, true, false).map_err(|e| e.with_param("axis"))?;
    if t.data.sketch.kind(axis) != Some(EntityKind::Line) {
        return Err(OpError::param("axis", "the mirror line must be a line"));
    }
    let p = t.data.sketch.curve(axis).map(|c| c.point({
        let (lo, hi) = c.domain();
        (lo + hi) * 0.5
    })).unwrap_or_default();
    let mut ed = editor(&t);
    ed.set_tool(Tool::Mirror);
    ed.selection = list;
    ed.hover = Some(axis);
    let changed = ed.tool_down(&mut t.data, p, Some(axis), false, false).changed;
    ed.message = None;
    finish(od, t, &ed, changed, &before)
}

/// The pattern tools: select the seeds (or open the pattern to edit), set
/// the values the manipulators and their labels set, accept.
fn pattern(ctx: &mut Ctx, a: &Args, tool: Tool, set: impl FnOnce(&mut SketchEditor, &mut SketchTarget, &BTreeMap<String, Quantity>) -> Result<(), OpError>) -> Result<OpOutput, OpError> {
    let (od, tab) = studio_target(ctx, a)?;
    let vars = variables_at(od, tab)?;
    let mut t = target(od, tab, a)?;
    let before = existing(&t);
    let mut ed = editor(&t);
    ed.set_tool(tool);
    match a.opt_u64("pattern")? {
        Some(pid) => {
            let pid = pid as u32;
            if t.data.patterns.iter().find(|p| p.id == pid).is_none_or(|p| p.circular != (tool == Tool::CircularPattern)) {
                return Err(OpError::param("pattern", "no such pattern of this kind in the sketch"));
            }
            if !ed.edit_pattern(&t.data, pid) {
                return Err(OpError::param("pattern", "the pattern's seeds are gone"));
            }
            if a.get("entities").is_some() {
                ed.selection = entity_list(a, "entities", &mut t, false)?;
            }
        }
        None => {
            if a.get("entities").is_none() {
                return Err(OpError::param("entities", "seed entities to pattern"));
            }
            ed.selection = entity_list(a, "entities", &mut t, false)?;
        }
    }
    set(&mut ed, &mut t, &vars)?;
    let changed = ed.apply_pattern(&mut t.data).changed;
    let pid = t.data.patterns.iter().map(|p| p.id).max();
    let mut out = finish(od, t, &ed, changed, &before)?;
    out.value["pattern"] = json!(pid);
    Ok(out)
}

fn count_arg(a: &Args, name: &str) -> Result<Option<usize>, OpError> {
    Ok(a.opt_u64(name)?.map(|n| n as usize))
}

fn linear_pattern(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    pattern(ctx, a, Tool::LinearPattern, |ed, t, vars| {
        if let Some(n) = count_arg(a, "count")? {
            ed.set_pattern_count(PatternHandle::LinearCount, n);
        }
        if let Some(n) = count_arg(a, "second_count")? {
            ed.set_pattern_count(PatternHandle::LinearSecondCount, n);
        }
        for (name, h) in [("distance", PatternHandle::LinearStep), ("second_distance", PatternHandle::LinearSecondStep)] {
            if let Some(v) = a.get(name) {
                let d = quantity(v, Dim::LENGTH, t, vars, name)?;
                if d <= 0.0 {
                    return Err(OpError::param(name, "must be greater than zero (set the direction with the angle)"));
                }
                ed.set_pattern_value(h, d);
            }
        }
        if let Some(v) = a.get("angle") {
            ed.pattern_angle = quantity(v, Dim::ANGLE, t, vars, "angle")?;
        }
        if let Some(v) = a.get("second_angle") {
            ed.pattern_second_angle = quantity(v, Dim::ANGLE, t, vars, "second_angle")?;
        }
        Ok(())
    })
}

fn circular_pattern(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    pattern(ctx, a, Tool::CircularPattern, |ed, t, vars| {
        if let Some(n) = count_arg(a, "count")? {
            ed.set_pattern_count(PatternHandle::CircularCount, n);
        }
        if let Some(v) = a.get("angle") {
            let ang = quantity(v, Dim::ANGLE, t, vars, "angle")?;
            if ang.abs() < 1e-9 || ang.abs() > std::f64::consts::TAU + 1e-9 {
                return Err(OpError::param("angle", "between -360 and 360 deg, not zero"));
            }
            ed.set_pattern_value(PatternHandle::CircularAngle, ang);
        }
        match a.get("center") {
            Some(Value::Array(_)) => ed.pattern_center = point_value(a.get("center").expect("center"), &t.units).map_err(|e| e.with_param("center"))?,
            Some(v @ Value::String(_)) => {
                let e = parse_entity(v, t, true, false).map_err(|e| e.with_param("center"))?;
                ed.pattern_center = t.data.sketch.point(e).ok_or_else(|| OpError::param("center", "not a point"))?;
            }
            Some(_) => return Err(OpError::param("center", "[x, y] or a point id")),
            None if a.opt_u64("pattern")?.is_none() => ed.pattern_center = DVec2::ZERO,
            None => {}
        }
        Ok(())
    })
}

fn transform(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    pattern(ctx, a, Tool::Transform, |ed, t, vars| {
        let bb = t.data.sketch.entities_bbox(&ed.selection);
        ed.pattern_center = match a.get("center") {
            Some(c) => point_value(c, &t.units).map_err(|e| e.with_param("center"))?,
            None if !bb.is_empty() => bb.center(),
            None => DVec2::ZERO,
        };
        if let Some(v) = a.get("translate") {
            ed.xf_offset = point_value(v, &t.units).map_err(|e| e.with_param("translate"))?;
        }
        if let Some(v) = a.get("rotate") {
            ed.set_pattern_value(PatternHandle::Rotate, quantity(v, Dim::ANGLE, t, vars, "rotate")?);
        }
        if let Some(f) = a.opt_f64("scale")? {
            if f <= 0.0 {
                return Err(OpError::param("scale", "must be greater than zero"));
            }
            ed.set_pattern_value(PatternHandle::Scale, f);
        }
        if ed.xf_offset == DVec2::ZERO && ed.xf_angle == 0.0 && ed.xf_scale == 1.0 {
            return Err(OpError::invalid("nothing to do: give `translate`, `rotate` or `scale`"));
        }
        Ok(())
    })
}

fn use_refs(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = studio_target(ctx, a)?;
    let mut t = target(od, tab, a)?;
    let before = existing(&t);
    let bb = t.data.sketch.bbox();
    let cursor = if bb.is_empty() { DVec2::ZERO } else { bb.center() };
    let (regen, frame) = (t.regen.clone(), t.frame);
    let model = sketch_use::Model { context: &regen.context, display: &regen.display, frame: &frame };
    for (i, v) in a.array("references")?.iter().enumerate() {
        let r = refs::parse(v).map_err(|e| e.with_param(format!("references[{i}]")))?;
        if matches!(&r, noshape_model::reference::Reference::SketchEntity { sketch, .. } if *sketch == t.id) {
            return Err(OpError::param(&format!("references[{i}]"), "already in this sketch"));
        }
        if !sketch_use::use_reference(&mut t.data, model, WORLD_PER_PX, cursor, &r) {
            return Err(OpError::param(&format!("references[{i}]"), sketch_use::USE_FAILED));
        }
    }
    let ed = editor(&t);
    finish(od, t, &ed, true, &before)
}

fn intersection(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = studio_target(ctx, a)?;
    let mut t = target(od, tab, a)?;
    let before = existing(&t);
    let r = refs::parse(a.get("face").expect("required")).map_err(|e| e.with_param("face"))?;
    let (regen, frame) = (t.regen.clone(), t.frame);
    let model = sketch_use::Model { context: &regen.context, display: &regen.display, frame: &frame };
    let mut ed = editor(&t);
    ed.set_tool(Tool::Intersection);
    sketch_use::intersect_face(&mut ed, &mut t.data, model, &r).map_err(|m| OpError::param("face", &m))?;
    ed.message = None;
    finish(od, t, &ed, true, &before)
}

fn text(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = studio_target(ctx, a)?;
    let vars = variables_at(od, tab)?;
    let mut t = target(od, tab, a)?;
    let before = existing(&t);
    let mut ed = editor(&t);
    if let Some(v) = a.get("edit") {
        let e = parse_entity(v, &mut t, false, false).map_err(|e| e.with_param("edit"))?;
        let id = t.data.text_of_entity(e).ok_or_else(|| OpError::param("edit", "not part of a text"))?;
        ed.begin_text_edit(&t.data, id);
    } else {
        let corner = point_value(a.get("corner").ok_or_else(|| OpError::param("corner", "the text box's lower-left corner (or `edit`)"))?, &t.units).map_err(|e| e.with_param("corner"))?;
        let h = match a.get("height") {
            Some(v) => quantity(v, Dim::LENGTH, &t, &vars, "height")?,
            None => ed.text_height,
        };
        if h <= 0.0 {
            return Err(OpError::param("height", "must be greater than zero"));
        }
        // Drag the box: the corner, then the top of the first line.
        ed.set_tool(Tool::Text);
        click(&mut ed, &mut t.data, corner);
        click(&mut ed, &mut t.data, corner + DVec2::Y * h);
        if !ed.text_dialog_open() {
            return Err(OpError::param("corner", "the text box couldn't be placed"));
        }
    }
    if let Some(s) = a.opt_str("text")? {
        ed.text_string = s.to_string();
    }
    if let Some(f) = a.opt_str("font")? {
        ed.text_font = f.to_string();
    }
    for (name, field) in [("bold", &mut ed.text_bold), ("italic", &mut ed.text_italic), ("flip_horizontal", &mut ed.text_flip_horizontal), ("flip_vertical", &mut ed.text_flip_vertical), ("expression", &mut ed.text_expression)] {
        if a.get(name).is_some() {
            *field = a.bool_or(name, false)?;
        }
    }
    let changed = ed.accept_text(&mut t.data).changed;
    finish(od, t, &ed, changed, &before)
}

fn insert_dxf(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = studio_target(ctx, a)?;
    let mut t = target(od, tab, a)?;
    let before = existing(&t);
    let scale = match a.opt_str("units")? {
        Some(u) => sm::DXF_UNITS.iter().find(|x| x.0.eq_ignore_ascii_case(u)).ok_or_else(|| OpError::param("units", "one of the listed units"))?.1,
        None => None,
    };
    let path = std::path::PathBuf::from(a.str("file")?);
    let ids = sm::insert_dxf_file(&mut t.data, &path, scale, a.bool_or("use_origin", false)?).map_err(|m| OpError::param("file", &m))?;
    if ids.is_empty() {
        return Err(OpError::param("file", "the drawing has no geometry a sketch can take"));
    }
    t.data.sketch.solve();
    let ed = editor(&t);
    let mut out = finish(od, t, &ed, true, &before)?;
    out.value["message"] = json!(format!("Inserted {} entities from {}", ids.len(), path.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default()));
    Ok(out)
}

fn insert_image(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = studio_target(ctx, a)?;
    let mut t = target(od, tab, a)?;
    let before = existing(&t);
    let image = sm::load_image(std::path::Path::new(a.str("file")?)).map_err(|m| OpError::param("file", &m))?;
    let pts = point_list(a, "points", &t)?;
    let mut ed = editor(&t);
    ed.set_tool(Tool::InsertImage);
    ed.pending_image = Some(image);
    let mut changed = false;
    for p in &pts {
        ed.pointer_move(&mut t.data, *p, false, false);
        ed.hover = None;
        changed |= ed.tool_down(&mut t.data, *p, None, false, false).changed;
        changed |= ed.tool_up(&mut t.data, *p).changed;
    }
    finish(od, t, &ed, changed, &before)
}

/// Pattern summaries for sketch_get.
pub(super) fn patterns_json(data: &noshape_model::sketch_feature::SketchData, sketch: noshape_model::reference::FeatureId, k: f64) -> Value {
    json!(data
        .patterns
        .iter()
        .map(|p| json!({
            "id": p.id,
            "kind": if p.circular { "circular" } else { "linear" },
            "seeds": p.seeds.iter().map(|e| entity_id(sketch, EntityId(*e))).collect::<Vec<_>>(),
            "count": p.count,
            "distance": p.distance * k,
            "angle_deg": p.angle.to_degrees(),
            "center": [p.center[0] * k, p.center[1] * k],
            "second_count": p.second_count,
        }))
        .collect::<Vec<_>>())
}
