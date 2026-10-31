//! The drawing view context menu: Show/hide, View orientation, Tangent
//! edges, Rotate, Order, Align views, Move to sheet, the crop / break /
//! section removals, View properties and the Show/hide parts, sketches and
//! edges dialogs. Each makes the drawing editor's edit (shared code in
//! `noshape_scene::drawing_tools::view_edit` and `crate::drawing`) with its
//! edit text.

use noshape_model::prelude::*;
use super::drawing::{ORIENTATIONS, commit, drawing_field, point, reply, sheet_field};
use super::drawing_tools::{Target, edge_at, edit_view, pt_arg, target, view_arg};
use super::{doc_field, s};
use crate::drawing::{self as dr, SheetModel};
use crate::error::OpError;
use crate::registry::{Args, Op, OpOutput, opt, req};
use crate::{Ctx, OpenDoc};
use noshape_model::drawing::views;
use noshape_model::drawing::{DisplayChoice, DrawingView, RenderStyle, Simplification, ViewId, ViewKind};
use noshape_scene::drawing_tools::Tool;
use noshape_scene::drawing_tools::view_edit::{self as ve, TangentMode};
use serde_json::{Value, json};

const TANGENTS: [(&str, TangentMode); 3] = [("hidden", TangentMode::Hidden), ("solid", TangentMode::Solid), ("phantom", TangentMode::Phantom)];

/// Show/hide ▸ toggles: (field, menu text, edit text).
const SHOW_HIDE: [(&str, &str, &str); 11] = [
    ("hidden_lines", "Show/Hide hidden lines", "Hidden lines"),
    ("shaded", "Show/Hide shaded view", "Shaded view"),
    ("threads", "Show/Hide threads", "Threads"),
    ("faulty_parts", "Show/Hide faulty parts", "Faulty parts"),
    ("offset_cut_lines", "Show/Hide offset cut lines (section views: a line across the hatch where the cutting line steps)", "Offset cut lines"),
    ("sketch_points", "Show/Hide sketch points (views showing sketches)", "Sketch points"),
    ("auto_centermarks", "Show/Hide auto centermarks (showing them again brings back deleted ones)", "Centermarks"),
    ("centerlines", "Show/Hide centerlines (views with cylinder axes seen from the side)", "Centerlines"),
    ("center_of_mass", "Show/Hide center of mass (part views, not flat patterns)", "Center of mass"),
    ("bend_lines", "Show/Hide bend lines (flat pattern views)", "Bend lines"),
    ("bend_notes", "Show/Hide bend notes (flat pattern views)", "Bend notes"),
];

const ACTIONS: [&str; 13] = ["rotate", "rotate_to_edge", "order", "align", "suppress_alignment", "flip_direction", "cutting_line_angle", "remove_crop", "remove_breaks", "remove_broken_out", "move_to_sheet", "unpin_reference", "copy"];

pub fn ops() -> Vec<Op> {
    let common = || vec![doc_field(), drawing_field(), sheet_field(), req("view", json!({ "type": "integer" }), "View id (drawing_get).")];
    let with = |mut v: Vec<crate::registry::Field>, more: Vec<crate::registry::Field>| {
        v.extend(more);
        v
    };
    let mut display = vec![
        opt("tangent_edges", json!({ "type": "string", "enum": TANGENTS.iter().map(|t| t.0).collect::<Vec<_>>() }), "Tangent edges ▸ Hidden / Solid / Phantom."),
        opt("orientation", s(), "View orientation ▸ (base views): front, back, top, bottom, left, right, isometric, or a named view of the source."),
    ];
    display.extend(SHOW_HIDE.iter().map(|(f, what, _)| opt(f, json!({ "type": "boolean" }), what)));
    vec![
        Op {
            group: "drawing",
            verb: "view_display",
            summary: "Show/hide, View orientation and Tangent edges of a view",
            description: "The view menu's Show/hide ▸ toggles (true shows, false hides; the menu only offers what applies to the view), View orientation ▸ and Tangent edges ▸. Each field given is one edit with the menu's edit text (\"Hidden lines\", \"View orientation\", \"Tangent edges\"...), in the order listed.",
            fields: with(common(), display),
            mutates: true,
            destructive: false,
            run: view_display,
        },
        Op {
            group: "drawing",
            verb: "view_edit",
            summary: "Rotate, order, align, move to sheet or copy a view; remove its crop, breaks or broken-out sections",
            description: "The view menu's actions, one edit each:\n- `rotate`: `by` degrees counter-clockwise (Rotate ▸ 90° counter-clockwise = 90, clockwise = -90, 180°) or `to` an absolute angle (Custom angle…; 0 = Reset rotation). \"Rotate view\". Views without children and not aligned with a parent (none, a detail view's, or its alignment suppressed), as the rotation angle.\n- `rotate_to_edge`: Rotate view ▸ Vertical to… / Horizontal to… (`direction`): turn by the smallest angle so the straight edge of the view at `edge` (sheet mm) is vertical / horizontal; the same views.\n- `order`: Order ▸ Bring to front / Send to back (`to`: front | back).\n- `align`: Align views ▸ Vertically / Horizontally (`direction`): the view's center lines up with view `with`'s.\n- `suppress_alignment`: (Un)suppress alignment with parent (`on`, default true); projected, auxiliary and section views; a rotated view can't be re-aligned.\n- `flip_direction`: section views: Flip direction (the view looks the other way across the cutting line).\n- `cutting_line_angle`: section views with a two-point cutting line: Cutting line angle… `to` degrees (the line turned about its middle, its length kept). \"Cutting line angle\".\n- `remove_crop`, `remove_breaks` (Remove all breaks), `remove_broken_out` (Remove section, broken-out section `index`); a removed crop or broken-out section leaves its boundary on the sheet as a sketch circle, polygon or spline (live).\n- `move_to_sheet`: Move to sheet… `to_sheet` (number or name; omitted: a new sheet); the view moves alone, linked to its parent.\n- `unpin_reference`: Unpin view reference (every view of the same source and part shows the workspace again).\n- `copy`: Copy then Paste: the view with its derived views, annotations and tables, pasted centered `at` (sheet mm) on sheet `to_sheet` (default this one); \"Paste views\"; the reply's `pasted` lists the new view ids.",
            fields: with(
                common(),
                vec![
                    req("action", json!({ "type": "string", "enum": ACTIONS }), "The menu action."),
                    opt("by", json!({ "type": "number" }), "rotate: degrees counter-clockwise to add."),
                    opt("to", json!({ "type": ["number", "string"] }), "rotate: the absolute angle (degrees); order: \"front\" or \"back\"."),
                    opt("edge", point(), "rotate_to_edge: a point on a straight edge of the view (sheet mm)."),
                    opt("direction", json!({ "type": "string", "enum": ["vertical", "horizontal"] }), "rotate_to_edge, align."),
                    opt("with", json!({ "type": "integer" }), "align: the view to line up with."),
                    opt("on", json!({ "type": "boolean", "default": true }), "suppress_alignment: suppress (true) or unsuppress."),
                    opt("to_sheet", json!({ "type": ["integer", "string"] }), "move_to_sheet / copy: the target sheet, number (1 = first) or name."),
                    opt("index", json!({ "type": "integer", "minimum": 0 }), "remove_broken_out: which broken-out section (0-based, drawing_get; default 0)."),
                    opt("at", point(), "copy: where the copy's center goes (sheet mm; default 20 mm right of and below the view)."),
                ],
            ),
            mutates: true,
            destructive: false,
            run: view_edit,
        },
        Op {
            group: "drawing",
            verb: "view_properties",
            summary: "Read or change a view's View properties",
            description: "The View properties dialog of one view (live drawing/177). Without `set`, replies with its fields as the dialog opens (`properties`). `set` changes the fields it names (same names), then ✓: scale (\"1:2\", or null to inherit), sheet_scale (inherit from the sheet rather than the parent), rotation (degrees; views without a parent or children), tangent_edges, best_quality, orientation / named_view (base views), explode_position and display_state (assemblies; child views: null = Follow parent), suppress_alignment, sync_breaks (projected views: Synchronize breaks from parent), cut_from_section, sketch_appearances, model_annotations, simplification + simplification_value, name, view_label, label (section / detail views: the letter), prefix / suffix (section / detail views; \"\\n\" starts a line), scale_label, sheet (moves the view, linked to its parent). One edit (\"View properties\").",
            fields: with(common(), vec![opt("set", json!({ "type": "object" }), "Fields to change (see the reply without `set`).")]),
            mutates: true,
            destructive: false,
            run: view_properties,
        },
        Op {
            group: "drawing",
            verb: "view_show_hide",
            summary: "Show/hide parts, sketches or edges of a view",
            description: "The Show/hide parts…, Show/hide sketches… and Show/hide edges… dialogs, ✓ each (one edit each: \"Show/hide parts\", \"Show/hide sketches\", \"Show/hide edges\"). `hidden_parts`: the parts left out of the view (names; views of a whole Part Studio or assembly). `shown_sketches`: the source's sketches shown (names; part views). `toggle_edges`: points on model edges of the view (sheet mm), each hidden, or shown again when hidden; `show_all_edges` first shows every hidden edge.",
            fields: with(
                common(),
                vec![
                    opt("hidden_parts", json!({ "type": "array", "items": { "type": "string" } }), "Part names to hide (replaces the list; [] shows all)."),
                    opt("shown_sketches", json!({ "type": "array", "items": { "type": "string" } }), "Sketch names to show (replaces the list)."),
                    opt("toggle_edges", json!({ "type": "array", "items": point() }), "Edge points (sheet mm) to hide / show."),
                    opt("show_all_edges", json!({ "type": "boolean", "default": false }), "Show every hidden edge first."),
                ],
            ),
            mutates: true,
            destructive: false,
            run: view_show_hide,
        },
        Op {
            group: "drawing",
            verb: "view_linestyle",
            summary: "Adjust linestyle of model edges in a view",
            description: "View menu › Adjust linestyle… (live drawing/443–446): the model edges of `view` at `edges` (points on them, sheet mm) get Linestyle `linestyle` (solid, phantom, center, dashed, dotted; default solid), Dashes `dashes` (scale, default 1; not for solid), Thickness `thickness` (mm, one of 0.13–2.00; default 0.35) and `color` ([r, g, b] 0–255; default the drawing's); `revert` instead puts them back to the drawing properties. One edit (\"Adjust linestyle\").",
            fields: with(
                common(),
                vec![
                    req("edges", json!({ "type": "array", "items": point(), "minItems": 1 }), "Points on the model edges (sheet mm)."),
                    opt("linestyle", json!({ "type": "string", "enum": ["solid", "phantom", "center", "dashed", "dotted"], "default": "solid" }), "Linestyle."),
                    opt("dashes", json!({ "type": "number", "default": 1.0 }), "Dash scale (not for solid)."),
                    opt("thickness", json!({ "type": "number", "default": 0.35 }), "Line thickness, mm."),
                    opt("color", json!({ "type": ["array", "null"], "items": { "type": "integer", "minimum": 0, "maximum": 255 }, "minItems": 3, "maxItems": 3 }), "Line color."),
                    opt("revert", json!({ "type": "boolean", "default": false }), "Revert to drawing properties."),
                ],
            ),
            mutates: true,
            destructive: false,
            run: view_linestyle,
        },
        Op {
            group: "drawing",
            verb: "sheet",
            summary: "Add, rename, duplicate, copy, move or delete a sheet, or change its Sheet properties",
            description: "The Sheets panel's sheet menu and the Sheet properties dialog, on sheet `sheet` (default: the one shown):\n- `add`: Insert sheet after it (same size, scale and title block; \"Add sheet\").\n- `rename`: Rename… to `name` (\"Rename sheet\").\n- `duplicate`: Duplicate, right after it, \"<name> copy\" (\"Duplicate sheet\").\n- `copy`: Copy then Paste: appended at the end, \"<name> (1)\" (\"Paste sheet\").\n- `move`: Move up / Move down to position `to` (1 = first; \"Move sheet\").\n- `delete`: Delete (never the last sheet; \"Delete sheet\").\n- `properties`: without `set`, replies with the dialog's fields (`properties`); `set` changes scale (\"1:2\"), size (\"A3\", \"A Portrait\"…, \"Custom\"), width / height (mm; Custom), zones ([horizontal, vertical], 0–20 even; Custom), zone_start (bottom_right | top_left), border (Custom), dwt (a DWT file tab: Select a DWT) or dwt: null (Format › Custom); one edit each (\"Select a DWT\", \"Format\", \"Sheet scale\", \"Sheet size\", \"Sheet zones\", \"Sheet border\").\nReplies with the sheet acted on (`sheet`, 1 = first) and the sheet list.",
            fields: vec![
                doc_field(),
                drawing_field(),
                sheet_field(),
                req("action", json!({ "type": "string", "enum": ["add", "rename", "duplicate", "copy", "move", "delete", "properties"] }), "The sheet menu action."),
                opt("name", s(), "rename: the new name."),
                opt("to", json!({ "type": "integer", "minimum": 1 }), "move: the new position (1 = first)."),
                opt("set", json!({ "type": "object" }), "properties: fields to change (see the reply without `set`)."),
            ],
            mutates: true,
            destructive: false,
            run: sheet,
        },
    ]
}

fn od<'a>(ctx: &'a mut Ctx, a: &Args) -> Result<&'a mut OpenDoc, OpError> {
    ctx.doc(a.opt_str("doc")?)
}

/// A view without a parent (on this sheet) or children: it can turn.
fn free(t: &Target, id: ViewId) -> bool {
    t.d.sheets[t.sheet].turns_freely(id)
}

/// A sheet given as in `sheet`: a number (1 = first) or a name.
fn to_sheet(t: &Target, v: Option<&Value>, param: &str) -> Result<Option<usize>, OpError> {
    let names = || t.d.sheets.iter().map(|x| x.name.clone()).collect::<Vec<_>>().join(", ");
    match v {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(n)) => n.as_u64().filter(|n| (1..=t.d.sheets.len() as u64).contains(n)).map(|n| Some(n as usize - 1)).ok_or_else(|| OpError::param(param, &format!("sheet numbers are 1 to {}", t.d.sheets.len()))),
        Some(Value::String(s)) => t.d.sheets.iter().position(|x| x.name == *s).map(Some).ok_or_else(|| OpError::param(param, &format!("no sheet `{s}`; sheets: {}", names()))),
        Some(_) => Err(OpError::param(param, "a sheet number (1 = first) or name")),
    }
}

fn view_of(t: &Target, id: ViewId) -> DrawingView {
    t.d.sheets[t.sheet].view(id).cloned().expect("view checked")
}

fn view_display(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    type Edit = Box<dyn Fn(&mut DrawingView)>;
    let od = od(ctx, a)?;
    let mut t = target(od, a)?;
    let id = view_arg(&t, a, "view")?;
    let v = view_of(&t, id);
    let axes = t.m.geoms.get(&id).is_some_and(|g| !g.axes.is_empty());
    // Every choice checked first, then one edit each, as in the editor.
    let mut edits: Vec<(&'static str, Edit)> = Vec::new();
    if let Some(name) = a.opt_str("orientation")? {
        let ViewKind::Base { source, .. } = v.kind else { return Err(OpError::param("orientation", "base views only (View orientation ▸)")) };
        let doc = od.doc();
        match ORIENTATIONS.iter().find(|o| o.0 == name) {
            Some(&(_, o)) => edits.push(("View orientation", Box::new(move |v| ve::set_standard_orientation(v, o)))),
            None => {
                let frame = dr::named_view_frame(doc, source, name).ok_or_else(|| {
                    let mut names: Vec<String> = ORIENTATIONS.iter().map(|o| o.0.to_string()).collect();
                    names.extend(dr::named_views(doc, source).iter().map(|v| v.name.clone()));
                    OpError::param("orientation", &format!("one of {}", names.join(", ")))
                })?;
                let n = name.to_string();
                let section = dr::named_view_section(doc, source, name);
                edits.push((
                    "View orientation",
                    Box::new(move |v| {
                        v.custom_frame = Some(frame);
                        v.named_view = Some(n.clone());
                        v.named_section.clone_from(&section);
                    }),
                ));
            }
        }
    }
    if let Some(m) = a.opt_str("tangent_edges")? {
        let m = TANGENTS.iter().find(|x| x.0 == m).map(|x| x.1).ok_or_else(|| OpError::param("tangent_edges", "hidden, solid or phantom"))?;
        edits.push(("Tangent edges", Box::new(move |v| m.apply(v))));
    }
    for (field, _, desc) in SHOW_HIDE {
        let Some(on) = a.get(field).map(|x| x.as_bool().ok_or_else(|| OpError::param(field, "true (show) or false (hide)"))).transpose()? else { continue };
        let refuse = |why: &str| Err(OpError::param(field, why));
        let edit: Edit = match field {
            "hidden_lines" => Box::new(move |v| v.style = if on { RenderStyle::HiddenLinesVisible } else { RenderStyle::HiddenLinesRemoved }),
            "shaded" => Box::new(move |v| v.style = if on { RenderStyle::Shaded } else { RenderStyle::HiddenLinesRemoved }),
            "threads" => Box::new(move |v| v.hide_threads = !on),
            "faulty_parts" => Box::new(move |v| v.show_faulty = on),
            "sketch_points" if v.shown_sketches.is_empty() => return refuse("the view shows no sketches (drawing_view_show_hide shown_sketches)"),
            "sketch_points" => Box::new(move |v| v.hide_sketch_points = !on),
            "auto_centermarks" => Box::new(move |v| {
                v.show_centerlines = on;
                v.deleted_centermarks.clear();
            }),
            "centerlines" if !axes => return refuse("the view has no cylinder axes seen from the side"),
            "centerlines" => Box::new(move |v| v.auto_centerlines = Some(on)),
            "offset_cut_lines" if !matches!(v.kind, ViewKind::Section { .. }) => return refuse("section views only"),
            "offset_cut_lines" => Box::new(move |v| v.show_offset_cut_lines = on),
            "center_of_mass" if v.sketch.is_some() || v.flat => return refuse("part views only, not flat patterns or sketch views"),
            "center_of_mass" => Box::new(move |v| v.show_com = on),
            "bend_lines" | "bend_notes" if !v.flat => return refuse("flat pattern views only"),
            "bend_lines" => Box::new(move |v| v.hide_bend_lines = !on),
            _ => Box::new(move |v| v.hide_bend_notes = !on),
        };
        edits.push((desc, edit));
    }
    if edits.is_empty() {
        return Err(OpError::invalid("nothing to change: give tangent_edges, orientation or a Show/hide field"));
    }
    for (desc, f) in edits {
        edit_view(&mut t, id, |v| f(v));
        commit(od, t.tab, t.sheet, t.d.clone(), desc)?;
    }
    Ok(reply(od, t.tab, t.sheet, false)?.into())
}

fn direction(a: &Args) -> Result<bool, OpError> {
    match a.opt_str("direction")? {
        Some("vertical") => Ok(true),
        Some("horizontal") => Ok(false),
        _ => Err(OpError::param("direction", "vertical or horizontal")),
    }
}

/// Turn view `id` (its edge dimensions turn too: `set_view_rotation`).
fn rotate_view(t: &mut Target, id: ViewId, to: impl FnOnce(f64) -> f64) {
    let (si, id) = t.d.real_view_of(t.sheet, id);
    if let Some(s) = t.d.sheets.get_mut(si)
        && let Some(r) = s.view(id).map(|v| v.rotation)
    {
        ve::set_view_rotation(s, id, to(r));
    }
}

fn view_edit(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let action = a.str("action")?;
    let od = od(ctx, a)?;
    let mut t = target(od, a)?;
    let id = view_arg(&t, a, "view")?;
    let v = view_of(&t, id);
    let desc: &str = match action {
        "rotate" => {
            if !free(&t, id) {
                return Err(OpError::param("view", "views aligned with a parent, or with children, can't turn"));
            }
            let deg = std::f64::consts::PI / 180.0;
            match (a.opt_f64("by")?, a.get("to")) {
                (Some(by), None) => rotate_view(&mut t, id, |r| r + by * deg),
                (None, Some(to)) => {
                    let to = to.as_f64().ok_or_else(|| OpError::param("to", "an angle in degrees"))?;
                    rotate_view(&mut t, id, |_| to * deg)
                }
                _ => return Err(OpError::param("by", "give `by` (degrees to add) or `to` (absolute degrees)")),
            }
            "Rotate view"
        }
        "rotate_to_edge" => {
            let vertical = direction(a)?;
            if !free(&t, id) {
                return Err(OpError::param("view", "views aligned with a parent, or with children, can't turn"));
            }
            let p = pt_arg(a, "edge")?;
            let (pa, pb, _) = edge_at(&t, &Tool::Select, p, 0).map_err(|_| OpError::param("edge", "Select a straight edge of the view"))?;
            if pa.view != id {
                return Err(OpError::param("edge", "Select a straight edge of the view"));
            }
            let pl = t.m.pick(&Tool::Select, &[], p).placement(id).ok_or_else(|| OpError::param("view", "the view has no geometry"))?;
            let (sa, sb) = (pl.to_sheet(pa.p), pl.to_sheet(pb.p));
            rotate_view(&mut t, id, |r| ve::rotation_to_line(r, sa, sb, vertical));
            if vertical { "Rotate view vertically" } else { "Rotate view horizontally" }
        }
        "order" => {
            let front = match a.get("to").and_then(Value::as_str) {
                Some("front") => true,
                Some("back") => false,
                _ => return Err(OpError::param("to", "\"front\" or \"back\"")),
            };
            ve::order_view(&mut t.d.sheets[t.sheet], id, front);
            if front { "Bring to front" } else { "Send to back" }
        }
        "align" => {
            let vertical = direction(a)?;
            let with = view_arg(&t, a, "with")?;
            if with == id {
                return Err(OpError::param("with", "another view"));
            }
            let at = t.d.sheets[t.sheet].view(with).map(|w| w.position).expect("view checked");
            edit_view(&mut t, id, |v| ve::align_view(v, at, vertical));
            "Align views"
        }
        "suppress_alignment" => {
            if !matches!(v.kind, ViewKind::Projected { .. } | ViewKind::Auxiliary { .. } | ViewKind::Section { .. } | ViewKind::AlignedSection { .. }) {
                return Err(OpError::param("view", "projected, auxiliary and section views only"));
            }
            let on = a.bool_or("on", true)?;
            if !on && v.unaligned && v.rotation.abs() > 1e-9 {
                return Err(OpError::param("on", "a rotated view can't be re-aligned"));
            }
            edit_view(&mut t, id, |v| v.unaligned = on);
            if on { "Suppress alignment with parent" } else { "Unsuppress alignment with parent" }
        }
        "cutting_line_angle" => {
            let deg = match a.get("to") {
                Some(Value::Number(n)) => n.as_f64().unwrap_or(0.0),
                Some(Value::String(s)) => s.trim().trim_end_matches('°').trim().parse::<f64>().map_err(|_| OpError::param("to", "the angle in degrees"))?,
                _ => return Err(OpError::param("to", "the angle in degrees")),
            };
            let sheet = t.sheet;
            if !noshape_scene::drawing_tools::annotation_edit::set_cutting_line_angle(&mut t.d.sheets[sheet], id, deg) {
                return Err(OpError::param("view", "a section view with a two-point cutting line"));
            }
            "Cutting line angle"
        }
        "flip_direction" => {
            if !matches!(v.kind, ViewKind::Section { .. } | ViewKind::AlignedSection { .. }) {
                return Err(OpError::param("view", "section views only"));
            }
            edit_view(&mut t, id, |v| v.flip = !v.flip);
            "Flip direction"
        }
        "remove_crop" => {
            if v.crop.is_none() {
                return Err(OpError::param("view", "the view is not cropped"));
            }
            // Live asb 10-23 views (3): the boundary stays as a sketch entity.
            let pl = t.m.built.as_ref().and_then(|b| b.placement(id).cloned());
            let si = t.sheet;
            noshape_scene::drawing_tools::remove_crop(&mut t.d.sheets[si], pl.as_ref(), id);
            "Remove crop"
        }
        "remove_breaks" => {
            if v.breaks.is_empty() {
                return Err(OpError::param("view", "the view has no breaks"));
            }
            edit_view(&mut t, id, |v| v.breaks.clear());
            "Remove breaks"
        }
        "remove_broken_out" => {
            if v.broken_out.is_empty() {
                return Err(OpError::param("view", "the view has no broken-out sections"));
            }
            // Live asb 10-23 views (3): Remove section on one broken-out
            // section (`index`, default 0); its profile stays as a sketch spline.
            let index = a.opt_u64("index")?.unwrap_or(0) as usize;
            if index >= v.broken_out.len() {
                return Err(OpError::param("index", &format!("the view has {} broken-out section(s)", v.broken_out.len())));
            }
            let pl = t.m.built.as_ref().and_then(|b| b.placement(id).cloned());
            let si = t.sheet;
            noshape_scene::drawing_tools::remove_broken_out(&mut t.d.sheets[si], pl.as_ref(), id, index);
            "Remove section"
        }
        "move_to_sheet" => {
            let to = match to_sheet(&t, a.get("to_sheet"), "to_sheet")? {
                Some(i) if i == t.sheet => return Err(OpError::param("to_sheet", "another sheet")),
                Some(i) => i,
                None => t.d.add_sheet(t.sheet),
            };
            let moved = t.d.move_views_to_sheet_linked(t.sheet, &[id], to).map_err(OpError::invalid)?;
            commit(od, t.tab, to, t.d, "Move to sheet")?;
            let mut out = reply(od, t.tab, to, false)?;
            out["view"] = json!(moved.first().map(|m| m.1.0));
            out["sheet"] = json!(to + 1);
            return Ok(out.into());
        }
        "unpin_reference" => {
            let sheet = &t.d.sheets[t.sheet];
            let root = views::root_view(sheet, id).and_then(|r| sheet.view(r)).cloned();
            let Some(ViewKind::Base { source, part, .. }) = root.as_ref().filter(|r| r.pinned.is_some()).map(|r| r.kind.clone()) else {
                return Err(OpError::param("view", "the view's reference is not pinned"));
            };
            for v in t.d.sheets.iter_mut().flat_map(|s| s.views.iter_mut()) {
                if matches!(&v.kind, ViewKind::Base { source: s, part: p, .. } if *s == source && *p == part) {
                    v.pinned = None;
                }
            }
            "Unpin view reference"
        }
        "copy" => {
            let to = to_sheet(&t, a.get("to_sheet"), "to_sheet")?.unwrap_or(t.sheet);
            let at = match a.get("at") {
                Some(_) => pt_arg(a, "at")?,
                None => [v.position[0] + 20.0, v.position[1] - 20.0],
            };
            let clip = t.d.copy_views(t.sheet, &[id], Some(&t.m.sources)).map_err(OpError::invalid)?;
            let pasted = t.d.sheets[to].paste_views(&clip, Some(at));
            commit(od, t.tab, to, t.d, "Paste views")?;
            let mut out = reply(od, t.tab, to, false)?;
            out["pasted"] = json!(pasted.iter().map(|v| v.0).collect::<Vec<_>>());
            return Ok(out.into());
        }
        _ => return Err(OpError::param("action", &format!("one of {}", ACTIONS.join(", ")))),
    };
    commit(od, t.tab, t.sheet, t.d, desc)?;
    Ok(reply(od, t.tab, t.sheet, false)?.into())
}

// ------------------------------------------------------------ View properties

fn simplification_name(s: Simplification) -> &'static str {
    match s {
        Simplification::None => "none",
        Simplification::Auto => "auto",
        Simplification::Absolute(_) => "absolute",
        Simplification::RatioToStudio(_) => "ratio_to_studio",
        Simplification::RatioToPart(_) => "ratio_to_part",
    }
}

/// The dialog's fields as JSON.
fn props_json(od: &OpenDoc, t: &Target, id: ViewId, p: &dr::ViewPropValues) -> Value {
    let sheet = &t.d.sheets[t.sheet];
    let asm = views::view_source(sheet, id).and_then(|(k, _)| match od.doc().tab(k.tab).map(|t| &t.content) {
        Some(noshape_model::document::TabContent::Assembly(a)) => Some(a.clone()),
        _ => None,
    });
    let mut j = json!({
        "scale": p.scale.map(|s| noshape_model::drawing::sheet::scale_text(s, p.scale_text.as_deref())),
        "scale_shown": views::view_scale_text(sheet, id),
        "sheet_scale": p.sheet_scale,
        "rotation": ((t.d.sheets[t.sheet].view(id).map_or(0.0, |v| v.rotation).to_degrees().rem_euclid(360.0)) * 1000.0).round() / 1000.0,
        "tangent_edges": TANGENTS.iter().find(|x| x.1 == p.tangent).map(|x| x.0),
        "best_quality": p.best_quality,
        "suppress_alignment": p.unaligned,
        "cut_from_section": p.cut_from_section,
        "sync_breaks": p.sync_breaks,
        "sketch_appearances": p.sketch_appearances,
        "model_annotations": p.model_annotations,
        "simplification": p.simplification.map(simplification_name),
        "name": p.name,
        "view_label": p.view_label,
        "label": p.label,
        "prefix": p.prefix,
        "suffix": p.suffix,
        "scale_label": p.scale_label,
        "sheet": t.sheet + 1,
    });
    if let Some(s) = p.simplification.filter(|s| matches!(s, Simplification::Absolute(_) | Simplification::RatioToStudio(_) | Simplification::RatioToPart(_))) {
        j["simplification_value"] = json!(dr::simplification_text(s, t.d.units_mm));
    }
    if let Some(o) = p.orientation {
        j["orientation"] = json!(ORIENTATIONS.iter().find(|x| x.1 == o).map(|x| x.0));
        j["named_view"] = json!(p.named);
    }
    if let Some(asm) = &asm {
        j["explode_position"] = json!(p.asm_position.map(|x| dr::asm_position_name(asm, x)));
        j["display_state"] = json!(match &p.display {
            Some(DisplayChoice::State(n)) => Some(n.clone()),
            Some(DisplayChoice::ShowAll) => Some("Show all".to_string()),
            None => None,
        });
    }
    j
}

fn view_properties(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = od(ctx, a)?;
    let mut t = target(od, a)?;
    let id = view_arg(&t, a, "view")?;
    let mut p = dr::ViewPropValues::of(&t.d, t.sheet, id).expect("view checked");
    let Some(set) = a.object("set")? else {
        let mut out = reply(od, t.tab, t.sheet, false)?;
        out["properties"] = props_json(od, &t, id, &p);
        return Ok(out.into());
    };
    let v = view_of(&t, id);
    let base = matches!(v.kind, ViewKind::Base { .. });
    let sheet = &t.d.sheets[t.sheet];
    let asm = views::view_source(sheet, id).and_then(|(k, _)| match od.doc().tab(k.tab).map(|t| &t.content) {
        Some(noshape_model::document::TabContent::Assembly(a)) => Some(a.clone()),
        _ => None,
    });
    let err = |k: &str, m: &str| OpError::param(&format!("set.{k}"), m);
    let boolean = |k: &str, x: &Value| x.as_bool().ok_or_else(|| err(k, "true or false"));
    let string = |k: &str, x: &Value| x.as_str().map(str::to_string).ok_or_else(|| err(k, "a string"));
    let mut dest = t.sheet;
    let mut simp_value: Option<String> = None;
    for (k, x) in set {
        let k = k.as_str();
        match k {
            "scale" => {
                // Typed text is kept as live does ("2:4", "0.5" → "0.5:1").
                let typed = x.as_str().and_then(noshape_model::drawing::sheet::typed_scale);
                p.scale_text = typed.as_ref().and_then(|t| t.1.clone());
                p.scale = match x {
                    Value::Null => None,
                    _ => Some(typed.map(|t| t.0).or(x.as_f64()).filter(|s| *s > 0.0).ok_or_else(|| err(k, "a scale (\"1:2\", \"2:1\") or null to inherit"))?),
                }
            }
            "sheet_scale" => p.sheet_scale = boolean(k, x)?,
            "rotation" => {
                let r = x.as_f64().filter(|r| (0.0..=360.0).contains(&r.abs())).ok_or_else(|| err(k, "degrees, -360 to 360"))?;
                if !free(&t, id) {
                    return Err(err(k, "views with a parent or children can't turn"));
                }
                p.rotation_deg = Some(r);
            }
            "tangent_edges" => p.tangent = x.as_str().and_then(|m| TANGENTS.iter().find(|t| t.0 == m)).map(|t| t.1).ok_or_else(|| err(k, "hidden, solid or phantom"))?,
            "best_quality" => p.best_quality = boolean(k, x)?,
            "suppress_alignment" => p.unaligned = boolean(k, x)?,
            "cut_from_section" => p.cut_from_section = boolean(k, x)?,
            "sync_breaks" => p.sync_breaks = boolean(k, x)?,
            "sketch_appearances" => p.sketch_appearances = boolean(k, x)?,
            "model_annotations" => p.model_annotations = boolean(k, x)?,
            "name" => p.name = string(k, x)?,
            "view_label" => p.view_label = boolean(k, x)?,
            "label" => p.label = string(k, x)?,
            "prefix" | "suffix" if dr::default_label_prefix(&t.d, &t.d.sheets[t.sheet].view(id).expect("view").kind).is_none() => return Err(err(k, "section and detail views only")),
            "prefix" => p.prefix = string(k, x)?,
            "suffix" => p.suffix = string(k, x)?,
            "scale_label" => p.scale_label = boolean(k, x)?,
            "orientation" | "named_view" if !base => return Err(err(k, "base views only")),
            "orientation" => {
                let name = string(k, x)?;
                p.orientation = Some(ORIENTATIONS.iter().find(|o| o.0 == name).map(|o| o.1).ok_or_else(|| err(k, "front, back, top, bottom, left, right or isometric"))?);
                if !set.contains_key("named_view") {
                    p.named = None;
                }
            }
            "named_view" => {
                p.named = match x {
                    Value::Null => None,
                    _ => {
                        let n = string(k, x)?;
                        let ViewKind::Base { source, .. } = v.kind else { unreachable!() };
                        if dr::named_view_frame(od.doc(), source, &n).is_none() {
                            let names: Vec<String> = dr::named_views(od.doc(), source).iter().map(|v| v.name.clone()).collect();
                            return Err(err(k, &format!("the source's named views: {}", names.join(", "))));
                        }
                        Some(n)
                    }
                }
            }
            "explode_position" | "display_state" if asm.is_none() => return Err(err(k, "assembly views only")),
            "explode_position" | "display_state" if matches!(v.kind, ViewKind::Section { .. } | ViewKind::AlignedSection { .. } | ViewKind::Detail { .. }) => return Err(err(k, "section and detail views follow their parent")),
            "explode_position" => {
                let asm = asm.as_ref().expect("checked");
                p.asm_position = match x {
                    Value::Null if !base => None,
                    _ => {
                        let n = string(k, x)?;
                        let choices = dr::asm_positions(asm);
                        Some(choices.iter().find(|c| c.0 == n).map(|c| c.1).ok_or_else(|| err(k, &format!("one of {}{}", choices.iter().map(|c| c.0.as_str()).collect::<Vec<_>>().join(", "), if base { "" } else { ", or null (Follow parent)" })))?)
                    }
                }
            }
            "display_state" => {
                let asm = asm.as_ref().expect("checked");
                p.display = match x {
                    Value::Null if !base => None,
                    _ => {
                        let n = string(k, x)?;
                        if n == "Show all" {
                            Some(DisplayChoice::ShowAll)
                        } else if asm.display_states.iter().any(|s| s.name == n) {
                            Some(DisplayChoice::State(n))
                        } else {
                            let mut names = vec!["Show all".to_string()];
                            names.extend(asm.display_states.iter().map(|s| s.name.clone()));
                            return Err(err(k, &format!("one of {}{}", names.join(", "), if base { "" } else { ", or null (Follow parent)" })));
                        }
                    }
                }
            }
            "simplification" => {
                p.simplification = match x {
                    Value::Null => None,
                    _ => Some(match x.as_str() {
                        Some("none") => Simplification::None,
                        Some("auto") => Simplification::Auto,
                        Some("absolute") => Simplification::Absolute(0.001),
                        Some("ratio_to_studio") => Simplification::RatioToStudio(0.01),
                        Some("ratio_to_part") => Simplification::RatioToPart(0.01),
                        _ => return Err(err(k, "none, auto, absolute, ratio_to_studio or ratio_to_part")),
                    }),
                }
            }
            "simplification_value" => simp_value = Some(string(k, x)?),
            "sheet" => dest = to_sheet(&t, Some(x), "set.sheet")?.unwrap_or(t.sheet),
            "scale_shown" => return Err(err(k, "read only; set `scale`")),
            _ => return Err(err(k, "not a View properties field (see the reply without `set`)")),
        }
    }
    if let Some(text) = simp_value {
        let kind = p.simplification.filter(|s| matches!(s, Simplification::Absolute(_) | Simplification::RatioToStudio(_) | Simplification::RatioToPart(_))).ok_or_else(|| err("simplification_value", "only for absolute and ratio simplifications"))?;
        p.simplification = Some(dr::parse_simplification(kind, &text, t.d.units_mm).ok_or_else(|| err("simplification_value", "a length (\"1 mm\") or a percentage (\"1 %\")"))?);
    }
    let doc = od.doc().clone();
    dr::apply_view_props(&doc, &mut t.d, t.sheet, id, &p);
    if dest != t.sheet {
        t.d.move_views_to_sheet_linked(t.sheet, &[id], dest).map_err(OpError::invalid)?;
    }
    commit(od, t.tab, dest, t.d, "View properties")?;
    Ok(reply(od, t.tab, dest, false)?.into())
}

// ------------------------------------------------------------ Show/hide dialogs

fn view_show_hide(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = od(ctx, a)?;
    let mut t = target(od, a)?;
    let id = view_arg(&t, a, "view")?;
    let v = view_of(&t, id);
    let sheet = &t.d.sheets[t.sheet];
    let source = views::view_source(sheet, id);
    let src = source.as_ref().and_then(|(k, _)| t.m.sources.get(k));
    let names = |k: &str| -> Result<Option<Vec<String>>, OpError> {
        a.opt_array(k)?.map(|l| l.iter().enumerate().map(|(i, x)| x.as_str().map(str::to_string).ok_or_else(|| OpError::param(&format!("{k}[{i}]"), "a name"))).collect()).transpose()
    };
    let mut steps: Vec<(Value, &'static str)> = Vec::new();
    let mut d = t.d.clone();
    if let Some(list) = names("hidden_parts")? {
        // A whole Part Studio or assembly (a one-part view has no Show/hide parts).
        let Some(src) = src.filter(|_| v.sketch.is_none() && source.as_ref().is_some_and(|s| s.1.is_none())) else {
            return Err(OpError::param("hidden_parts", "views of a whole Part Studio or assembly only"));
        };
        let mut parts = Vec::new();
        for (i, n) in list.iter().enumerate() {
            let b = src.context.bodies.iter().find(|b| &b.name == n).ok_or_else(|| OpError::param(&format!("hidden_parts[{i}]"), &format!("the view's parts: {}", src.context.bodies.iter().map(|b| b.name.as_str()).collect::<Vec<_>>().join(", "))))?;
            parts.push(b.id.clone());
        }
        let (si, rid) = d.real_view_of(t.sheet, id);
        if let Some(v) = d.sheets.get_mut(si).and_then(|s| s.view_mut(rid)) {
            v.hidden_parts = parts;
        }
        commit(od, t.tab, t.sheet, d.clone(), "Show/hide parts")?;
        steps.push((json!("hidden_parts"), "Show/hide parts"));
    }
    if let Some(list) = names("shown_sketches")? {
        let sketches: Vec<(noshape_model::FeatureId, String)> = match src {
            Some(s) if !s.assembly && v.sketch.is_none() => s.context.sketches.iter().map(|s| (s.feature, s.name.clone())).collect(),
            _ => Vec::new(),
        };
        if sketches.is_empty() {
            return Err(OpError::param("shown_sketches", "part views of a Part Studio with sketches only"));
        }
        let mut picked = Vec::new();
        for (i, n) in list.iter().enumerate() {
            let f = sketches.iter().find(|s| &s.1 == n).ok_or_else(|| OpError::param(&format!("shown_sketches[{i}]"), &format!("the source's sketches: {}", sketches.iter().map(|s| s.1.as_str()).collect::<Vec<_>>().join(", "))))?;
            picked.push(f.0);
        }
        let (si, rid) = d.real_view_of(t.sheet, id);
        if let Some(v) = d.sheets.get_mut(si).and_then(|s| s.view_mut(rid)) {
            v.shown_sketches = picked;
        }
        commit(od, t.tab, t.sheet, d.clone(), "Show/hide sketches")?;
        steps.push((json!("shown_sketches"), "Show/hide sketches"));
    }
    let toggles = a.opt_array("toggle_edges")?.cloned().unwrap_or_default();
    if !toggles.is_empty() || a.bool_or("show_all_edges", false)? {
        // As the dialog shows them: hidden edges drawn (grey) so they can be picked.
        let mut edges = if a.bool_or("show_all_edges", false)? { Vec::new() } else { v.hidden_edges.clone() };
        let mut marked = d.clone();
        if let Some(mv) = marked.sheets[t.sheet].view_mut(id) {
            mv.hidden_edges = edges.clone();
            mv.editing_edges = true;
        }
        let m = SheetModel::build(&od.session, t.tab, marked, t.sheet, &[]);
        for (i, p) in toggles.iter().enumerate() {
            let field = format!("toggle_edges[{i}]");
            let p = super::drawing::point_value(p, &field)?;
            let (view, me) = m.pick(&Tool::Select, &[], p).hovered_edge().ok_or_else(|| OpError::param(&field, "no model edge of the view there"))?;
            if view != id {
                return Err(OpError::param(&field, "an edge of another view"));
            }
            let key = (me.body.clone(), (*me.name).clone());
            match edges.iter().position(|e| *e == key) {
                Some(i) => {
                    edges.remove(i);
                }
                None => edges.push(key),
            }
        }
        if let Some(v) = d.sheets[t.sheet].view_mut(id) {
            v.hidden_edges = edges;
        }
        commit(od, t.tab, t.sheet, d.clone(), "Show/hide edges")?;
        steps.push((json!("toggle_edges"), "Show/hide edges"));
    }
    if steps.is_empty() {
        return Err(OpError::invalid("nothing to change: give hidden_parts, shown_sketches, toggle_edges or show_all_edges"));
    }
    t.d = d;
    let mut out = reply(od, t.tab, t.sheet, false)?;
    let v = t.d.sheets[t.sheet].view(id).expect("view");
    out["hidden_parts"] = json!(src.map(|s| v.hidden_parts.iter().filter_map(|b| s.context.bodies.iter().find(|x| &x.id == b).map(|x| x.name.clone())).collect::<Vec<_>>()));
    out["hidden_edges"] = json!(v.hidden_edges.len());
    Ok(out.into())
}

// ------------------------------------------------------------ sheets

fn size_by_name(name: &str) -> Option<noshape_model::drawing::SheetSize> {
    noshape_model::drawing::SheetSize::STANDARD.into_iter().find(|s| s.list_name() == name)
}

fn sheet_props_json(d: &noshape_model::drawing::Drawing, i: usize) -> Value {
    let s = &d.sheets[i];
    let (w, h) = s.size.size_mm();
    let mut j = json!({
        "name": s.name,
        "scale": noshape_model::drawing::sheet::scale_text(s.scale, s.scale_text.as_deref()),
        "size": s.size.list_name(),
        "width": (w * 1000.0).round() / 1000.0,
        "height": (h * 1000.0).round() / 1000.0,
        "dwt": s.dwt,
    });
    if matches!(s.size, noshape_model::drawing::SheetSize::Custom { .. }) && s.dwt.is_none() {
        j["zones"] = json!(s.format.zone_counts(s.size));
        j["zone_start"] = json!(if s.format.start == noshape_model::drawing::ZoneStart::TopLeft { "top_left" } else { "bottom_right" });
        j["border"] = json!(s.format.border);
    }
    j
}

type SheetStep = (&'static str, Box<dyn Fn(&mut noshape_model::drawing::Drawing)>);

/// Sheet properties `set`: checked first, then one edit per field in the
/// dialog's order.
fn sheet_prop_steps(od: &OpenDoc, d: &noshape_model::drawing::Drawing, i: usize, set: &serde_json::Map<String, Value>) -> Result<Vec<SheetStep>, OpError> {
    use noshape_model::drawing::{SheetSize, ZoneStart};
    let err = |k: &str, m: &str| OpError::param(&format!("set.{k}"), m);
    let order = ["dwt", "scale", "size", "width", "height", "zones", "zone_start", "border"];
    if set.contains_key("name") {
        return Err(err("name", "use action rename"));
    }
    if let Some(k) = set.keys().find(|k| !order.contains(&k.as_str())) {
        return Err(err(k, &format!("not a Sheet properties field ({})", order.join(", "))));
    }
    let mut steps: Vec<SheetStep> = Vec::new();
    let mut size = d.sheets[i].size;
    let mut dwt = d.sheets[i].dwt.clone();
    for k in order {
        let Some(x) = set.get(k) else { continue };
        match k {
            "dwt" if x.is_null() => {
                if dwt.take().is_some() {
                    steps.push(("Format", Box::new(move |d| d.detach_dwt(i))));
                }
            }
            "dwt" => {
                let name = x.as_str().ok_or_else(|| err(k, "a DWT file tab name"))?;
                let tab = crate::doc::find_tab(od.doc(), name).map_err(|e| err(k, &e.message))?;
                let Some(noshape_model::document::TabContent::File(f)) = od.doc().tab(tab).map(|t| &t.content) else { return Err(err(k, "not a file tab")) };
                if f.kind() != noshape_model::file_tab::FileKind::Template {
                    return Err(err(k, "not a DWT file"));
                }
                let templates = f.bytes().ok_or_else(|| err(k, "The file can't be read")).and_then(|b| noshape_model::drawing::dwt::read_dwt(&b).map_err(|e| err(k, &format!("Select a DWT failed: {e}"))))?;
                let file_name = f.file_name.clone();
                dwt = Some(file_name.clone());
                steps.push(("Select a DWT", Box::new(move |d| d.apply_dwt(i, &file_name, &templates))));
            }
            "scale" => {
                let typed = x.as_str().and_then(noshape_model::drawing::sheet::typed_scale);
                let text = typed.as_ref().and_then(|t| t.1.clone());
                let sc = typed.map(|t| t.0).or(x.as_f64().filter(|v| *v > 0.0)).ok_or_else(|| err(k, "a scale (\"1:2\")"))?;
                steps.push((
                    "Sheet scale",
                    Box::new(move |d| {
                        d.sheets[i].scale = sc;
                        d.sheets[i].scale_text.clone_from(&text);
                    }),
                ));
            }
            "size" if dwt.is_some() => return Err(err(k, "the sheet's format is a DWT's (set dwt: null first)")),
            "size" if x.as_str() == Some("Custom") => {
                // Custom keeps the current paper size and zones, with the border.
                steps.push((
                    "Sheet size",
                    Box::new(move |d| {
                        let sh = &mut d.sheets[i];
                        if !matches!(sh.size, SheetSize::Custom { .. }) {
                            let (w, h) = sh.size.size_mm();
                            sh.format.zones = Some(sh.format.zone_counts(sh.size));
                            sh.size = SheetSize::Custom { width_mm: w, height_mm: h };
                        }
                    }),
                ));
                let (w, h) = size.size_mm();
                size = SheetSize::Custom { width_mm: w, height_mm: h };
            }
            "size" => {
                let ansi = match size {
                    SheetSize::Custom { .. } => !d.units_mm,
                    s => s.is_ansi(),
                };
                let list: Vec<&str> = SheetSize::standard_list(ansi).iter().map(|s| s.list_name()).chain(["Custom"]).collect();
                let s = x.as_str().and_then(size_by_name).filter(|s| list.contains(&s.list_name()) || *s == size).ok_or_else(|| err(k, &format!("one of {}", list.join(", "))))?;
                steps.push(("Sheet size", Box::new(move |d| d.sheets[i].size = s)));
                size = s;
            }
            _ if !matches!(size, SheetSize::Custom { .. }) || dwt.is_some() => return Err(err(k, "Custom size only (set size: \"Custom\")")),
            "width" | "height" => {
                let v = x.as_f64().filter(|v| *v > 0.0).ok_or_else(|| err(k, "a length in mm"))?;
                let (w0, h0) = size.size_mm();
                let (w, h) = if k == "width" { (v, h0) } else { (w0, v) };
                size = SheetSize::Custom { width_mm: w, height_mm: h };
                steps.push(("Sheet size", Box::new(move |d| d.sheets[i].size = SheetSize::Custom { width_mm: w, height_mm: h })));
            }
            "zones" => {
                let z: Vec<u32> = x.as_array().map(|l| l.iter().filter_map(Value::as_u64).map(|v| v as u32).collect()).unwrap_or_default();
                if z.len() != 2 || !z.iter().all(|v| *v <= 20 && v % 2 == 0) {
                    return Err(err(k, "[horizontal, vertical], each 0 to 20 and even"));
                }
                let z = [z[0], z[1]];
                steps.push(("Sheet zones", Box::new(move |d| d.sheets[i].format.zones = Some(z))));
            }
            "zone_start" => {
                let z = match x.as_str() {
                    Some("bottom_right") => ZoneStart::BottomRight,
                    Some("top_left") => ZoneStart::TopLeft,
                    _ => return Err(err(k, "bottom_right or top_left")),
                };
                steps.push(("Sheet zones", Box::new(move |d| d.sheets[i].format.start = z)));
            }
            _ => {
                let on = x.as_bool().ok_or_else(|| err(k, "true or false"))?;
                steps.push(("Sheet border", Box::new(move |d| d.sheets[i].format.border = on)));
            }
        }
    }
    if steps.is_empty() {
        return Err(OpError::param("set", "nothing to change"));
    }
    Ok(steps)
}

fn sheet(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = od(ctx, a)?;
    let tab = super::drawing::drawing_tab(od, a)?;
    let mut d = dr::drawing_of(od.doc(), tab).cloned().expect("drawing");
    let i = super::drawing::sheet_index(&d, a)?;
    let n = d.sheets.len();
    let (desc, at): (&str, usize) = match a.str("action")? {
        "add" => ("Add sheet", d.add_sheet(i)),
        "rename" => {
            if !d.rename_sheet(i, a.str("name")?) {
                return Err(OpError::param("name", "a name"));
            }
            ("Rename sheet", i)
        }
        "duplicate" => ("Duplicate sheet", d.duplicate_sheet(i).expect("sheet checked")),
        "copy" => {
            let copy = d.sheets[i].clone();
            ("Paste sheet", d.paste_sheet(copy))
        }
        "move" => {
            let to = a.opt_u64("to")?.map(|x| x as usize).filter(|x| (1..=n).contains(x) && *x != i + 1).ok_or_else(|| OpError::param("to", &format!("another position, 1 to {n}")))?;
            d.move_sheet(i, to - 1);
            ("Move sheet", to - 1)
        }
        "delete" => {
            if !d.delete_sheet(i) {
                return Err(OpError::param("sheet", "the last sheet can't be deleted"));
            }
            ("Delete sheet", i.min(n - 2))
        }
        "properties" => {
            if let Some(set) = a.object("set")? {
                for (desc, f) in sheet_prop_steps(od, &d, i, set)? {
                    f(&mut d);
                    commit(od, tab, i, d.clone(), desc)?;
                }
            }
            let mut out = reply(od, tab, i, false)?;
            out["properties"] = sheet_props_json(&d, i);
            out["sheet"] = json!(i + 1);
            return Ok(out.into());
        }
        other => return Err(OpError::param("action", &format!("`{other}`: add, rename, duplicate, copy, move, delete or properties"))),
    };
    commit(od, tab, at, d, desc)?;
    let mut out = reply(od, tab, at, false)?;
    out["sheet"] = json!(at + 1);
    Ok(out.into())
}

// ------------------------------------------------------------ Adjust linestyle

fn view_linestyle(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    use noshape_model::drawing::{LINE_THICKNESSES, LinePattern};
    let od = od(ctx, a)?;
    let mut t = target(od, a)?;
    let id = view_arg(&t, a, "view")?;
    let mut edges = Vec::new();
    for (i, p) in a.array("edges")?.iter().enumerate() {
        let field = format!("edges[{i}]");
        let p = super::drawing::point_value(p, &field)?;
        let (view, me) = t.m.pick(&Tool::Select, &[], p).hovered_edge().ok_or_else(|| OpError::param(&field, "no model edge there"))?;
        if view != id {
            return Err(OpError::param(&field, "an edge of another view"));
        }
        let key = (me.body.clone(), (*me.name).clone());
        if !edges.contains(&key) {
            edges.push(key);
        }
    }
    let look = if a.bool_or("revert", false)? {
        None
    } else {
        let pattern = match a.opt_str("linestyle")?.unwrap_or("solid") {
            "solid" => LinePattern::Solid,
            "phantom" => LinePattern::Phantom,
            "center" => LinePattern::Center,
            "dashed" => LinePattern::Dashed,
            "dotted" => LinePattern::Dotted,
            _ => return Err(OpError::param("linestyle", "solid, phantom, center, dashed or dotted")),
        };
        let dashes = a.opt_f64("dashes")?.unwrap_or(1.0);
        if dashes <= 0.0 {
            return Err(OpError::param("dashes", "a positive scale"));
        }
        if pattern == LinePattern::Solid && a.get("dashes").is_some() {
            return Err(OpError::param("dashes", "not for a solid line"));
        }
        let thickness = a.opt_f64("thickness")?.unwrap_or(0.35);
        if !LINE_THICKNESSES.iter().any(|x| (x - thickness).abs() < 1e-9) {
            return Err(OpError::param("thickness", &format!("one of {}", LINE_THICKNESSES.iter().map(|x| format!("{x:.2}")).collect::<Vec<_>>().join(", "))));
        }
        let color = match a.get("color") {
            None => None,
            Some(c) => {
                let v: Vec<u8> = c.as_array().map(|l| l.iter().filter_map(|x| x.as_u64().filter(|x| *x <= 255).map(|x| x as u8)).collect()).unwrap_or_default();
                if v.len() != 3 {
                    return Err(OpError::param("color", "[r, g, b], each 0–255"));
                }
                Some([v[0], v[1], v[2]])
            }
        };
        Some(ve::EdgeLook { pattern, dashes, thickness, color })
    };
    edit_view(&mut t, id, |v| ve::set_edge_styles(v, &edges, look));
    commit(od, t.tab, t.sheet, t.d, "Adjust linestyle")?;
    Ok(reply(od, t.tab, t.sheet, false)?.into())
}
