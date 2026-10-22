//! Drawing views and annotations beyond Insert / Projected view and the
//! dimensions: Section, Detail, Auxiliary, Break and Crop views,
//! centerlines, virtual sharps and view tables. Each runs the drawing
//! editor's own tool code (`noshape_scene::drawing_tools`) on the clicks
//! given (sheet mm) and commits with the editor's edit text.

use super::doc_field;
use super::drawing::{commit, drawing_field, drawing_tab, point, point_value, reply, sheet_field, sheet_index};
use crate::drawing::{self as dr, SheetModel};
use crate::error::OpError;
use crate::registry::{Args, Op, OpOutput, opt, req};
use crate::{Ctx, OpenDoc, refs};
use noshape_model::drawing::{AnnotationKind, BreakStyle, CalloutBorder, CalloutStyle, Drawing, GdtLeader, ViewId, ViewKind};
use noshape_scene::drawing_tools::{self as dt, AuxOpts, DetailOpts, Pick, PickedPoint, SectionOpts, TableTool, Tool, ToolValues, ViewOptions};
use serde_json::{Value, json};

/// The Geometric tolerance dialog's symbols (index into `GDT_TYPES`).
const GDT_SYMBOLS: [(&str, usize); 14] = [
    ("position", 0),
    ("concentricity", 1),
    ("symmetry", 2),
    ("parallelism", 3),
    ("perpendicularity", 4),
    ("angularity", 5),
    ("cylindricity", 6),
    ("flatness", 7),
    ("roundness", 8),
    ("straightness", 9),
    ("profile_of_a_surface", 10),
    ("profile_of_a_line", 11),
    ("circular_runout", 12),
    ("total_runout", 13),
];

/// The Callout dialog's border shapes.
const BORDERS: [(&str, CalloutBorder); 12] = [
    ("none", CalloutBorder::None),
    ("circle", CalloutBorder::Circle),
    ("diamond", CalloutBorder::Diamond),
    ("double_circle", CalloutBorder::DoubleCircle),
    ("flag", CalloutBorder::Flag),
    ("hexagon", CalloutBorder::Hexagon),
    ("octagon", CalloutBorder::Octagon),
    ("rectangle", CalloutBorder::Rectangle),
    ("split_circle", CalloutBorder::SplitCircle),
    ("square_circle", CalloutBorder::SquareCircle),
    ("triangle", CalloutBorder::Triangle),
    ("underline", CalloutBorder::Underline),
];

fn view_id(name: &'static str, what: &'static str) -> crate::registry::Field {
    req(name, json!({ "type": "integer" }), what)
}

fn points(min: u64, max: u64) -> Value {
    json!({ "type": "array", "items": point(), "minItems": min, "maxItems": max })
}

pub fn ops() -> Vec<Op> {
    let common = || vec![doc_field(), drawing_field(), sheet_field()];
    let with = |mut v: Vec<crate::registry::Field>, more: Vec<crate::registry::Field>| {
        v.extend(more);
        v
    };
    vec![
        Op {
            group: "drawing",
            verb: "section_view",
            summary: "Section view of a view (full, partial, jogged by snap points)",
            description: "The Section view tool: the cutting line goes through `point` (sheet mm; a vertex or circle center there is snapped) across the whole parent view, `direction` vertical (the dialog's default), horizontal or angular (`angle`, default 45 deg). With `partial` the line runs from `point` to `end` (snapped to horizontal / vertical within 8 deg, as the second click) and only cuts along its length. `jogs`: snap points of the view the line jogs through. The view goes to `position`, projected perpendicular to the line. One edit (\"Section view\").",
            fields: with(
                common(),
                vec![
                    view_id("parent", "View to section (drawing_get)."),
                    req("point", point(), "A point of the cutting line (sheet mm)."),
                    opt("direction", json!({ "type": "string", "enum": ["vertical", "horizontal", "angular"], "default": "vertical" }), "Cutting line direction (full sections)."),
                    opt("angle", json!({ "type": "number", "default": 45 }), "Angular: the line's angle in degrees."),
                    opt("opposite", json!({ "type": "boolean", "default": false }), "Angular: Opposite direction."),
                    opt("partial", json!({ "type": "boolean", "default": false }), "Partial section: the line from `point` to `end` only."),
                    opt("end", point(), "Partial: the line's other end (sheet mm)."),
                    opt("jogs", points(0, 16), "Full sections: snap points the line jogs through, in order."),
                    req("position", point(), "Where the section view goes (sheet mm)."),
                    opt("label", json!({ "type": "string" }), "Section id (default the sheet's next letter)."),
                    opt("cut_only", json!({ "type": "boolean", "default": false }), "Show cut geometry only."),
                    opt("flip", json!({ "type": "boolean", "default": false }), "Flip the view direction."),
                    opt("depth", json!({ "type": "number", "exclusiveMinimum": 0 }), "Section depth (model mm)."),
                    opt("depth_to", point(), "Section depth up to this vertex / circle center (sheet mm)."),
                    opt("exclude", json!({ "type": "array", "items": { "type": "string" } }), "Parts excluded from the cut (`body:…`)."),
                    opt("cut_from_section", json!({ "type": "boolean", "default": true }), "Sectioning a section view: cut from the section."),
                ],
            ),
            mutates: true,
            destructive: false,
            run: section_view,
        },
        Op {
            group: "drawing",
            verb: "detail_view",
            summary: "Detail view of a circular region of a view",
            description: "The Detail view tool: the circle centered at `center` (sheet mm; a vertex or circle center there is snapped) with `radius` (sheet mm) on the parent view, shown at `scale` (default twice the parent's) at `position`. One edit (\"Detail view\").",
            fields: with(
                common(),
                vec![
                    view_id("parent", "View to detail."),
                    req("center", point(), "Detail circle center (sheet mm)."),
                    req("radius", json!({ "type": "number", "exclusiveMinimum": 0 }), "Detail circle radius (sheet mm)."),
                    req("position", point(), "Where the detail view goes (sheet mm)."),
                    opt("scale", json!({ "type": "number", "exclusiveMinimum": 0 }), "View scale (default 2 x the parent's)."),
                    opt("label", json!({ "type": "string" }), "Detail id (default the sheet's next letter)."),
                    opt("style", json!({ "type": "string", "enum": ["in_line", "connected", "leader"] }), "Label style (default Drawing properties')."),
                    opt("show_profile", json!({ "type": "boolean" }), "Show the detail profile (default Drawing properties')."),
                    opt("clip_border", json!({ "type": "boolean" }), "Clip the border (default Drawing properties')."),
                ],
            ),
            mutates: true,
            destructive: false,
            run: detail_view,
        },
        Op {
            group: "drawing",
            verb: "auxiliary_view",
            summary: "Auxiliary view looking normal to a straight edge",
            description: "The Auxiliary view tool (Normal): `edge` is a click on a straight model edge of the parent view (sheet mm); the view looks along it and goes to `position` (at least 15 mm from the parent, perpendicular to the edge). One edit (\"Auxiliary view\").",
            fields: with(
                common(),
                vec![
                    req("edge", point(), "A point on a straight edge of a view (sheet mm)."),
                    req("position", point(), "Where the view goes (sheet mm)."),
                    opt("label", json!({ "type": "string" }), "Auxiliary id (default the sheet's next letter)."),
                    opt("reverse", json!({ "type": "boolean", "default": false }), "Reverse the direction."),
                    opt("show_reference", json!({ "type": "boolean", "default": true }), "Show the view reference indicator."),
                ],
            ),
            mutates: true,
            destructive: false,
            run: auxiliary_view,
        },
        Op {
            group: "drawing",
            verb: "break_view",
            summary: "Break a view (remove a band between two points)",
            description: "The Break view tool: removes the band between `from` and `to` (sheet mm) across the view: `vertical` break lines cut along x, horizontal ones along y. One edit (\"Break view\").",
            fields: with(
                common(),
                vec![
                    view_id("view", "View to break."),
                    req("from", point(), "First break line (sheet mm)."),
                    req("to", point(), "Second break line (sheet mm)."),
                    opt("orientation", json!({ "type": "string", "enum": ["vertical", "horizontal"], "default": "vertical" }), "Break line orientation."),
                    opt("style", json!({ "type": "string", "enum": ["zigzag", "straight", "curve", "small_zigzag"], "default": "zigzag" }), "Break line style."),
                    opt("gap", json!({ "type": "number", "exclusiveMinimum": 0 }), "Gap (sheet mm; default 6 mm, 0.25 in in inch drawings)."),
                ],
            ),
            mutates: true,
            destructive: false,
            run: break_view,
        },
        Op {
            group: "drawing",
            verb: "crop_view",
            summary: "Crop a view to a rectangle or circle",
            description: "The Crop view tool: a rectangle with corners `from` and `to`, or a circle centered at `from` through `to` (sheet mm; `from` snaps to a vertex there). One edit (\"Crop view\").",
            fields: with(
                common(),
                vec![
                    view_id("view", "View to crop."),
                    opt("shape", json!({ "type": "string", "enum": ["rectangle", "circle"], "default": "rectangle" }), "Crop shape."),
                    req("from", point(), "Rectangle corner / circle center (sheet mm)."),
                    req("to", point(), "Opposite corner / a point on the circle (sheet mm)."),
                ],
            ),
            mutates: true,
            destructive: false,
            run: crop_view,
        },
        Op {
            group: "drawing",
            verb: "centerline",
            summary: "Centerlines: 2 point, edge to edge, 3 point / 2 point circle",
            description: "The centerline tools, on clicks `picks` (sheet mm): two_point (two snap points of one view), edge_to_edge (one cylinder silhouette edge gives its axis; or two edges give their middle line), circle_3_point (three snap points) and circle_2_point (center, then a point on the circle). Associative to the model points snapped. One edit with the tool's name.",
            fields: with(
                common(),
                vec![
                    req("kind", json!({ "type": "string", "enum": ["two_point", "edge_to_edge", "circle_3_point", "circle_2_point"] }), "Centerline tool."),
                    req("picks", points(1, 3), "The clicks (sheet mm)."),
                ],
            ),
            mutates: true,
            destructive: false,
            run: centerline,
        },
        Op {
            group: "drawing",
            verb: "virtual_sharp",
            summary: "Virtual sharp where two straight edges would meet",
            description: "The Virtual sharp tool: clicks on two straight edges of one view (sheet mm). Associative: dimensions can attach to it. One edit (\"Virtual sharp\").",
            fields: with(common(), vec![req("picks", points(2, 2), "A point on each edge (sheet mm).")]),
            mutates: true,
            destructive: false,
            run: virtual_sharp,
        },
        Op {
            group: "drawing",
            verb: "hole_callout",
            summary: "Hole / thread callout on a hole seen as a circle",
            description: "The Hole/thread callout tool: `hole` is a click on the hole's circle (sheet mm); the callout text (diameter, depth, thread from the Hole feature) goes to `position`. One edit (\"Hole callout\").",
            fields: with(common(), vec![req("hole", point(), "A point on the hole's circle (sheet mm)."), req("position", point(), "Where the callout text goes (sheet mm).")]),
            mutates: true,
            destructive: false,
            run: hole_callout,
        },
        Op {
            group: "drawing",
            verb: "callout",
            summary: "Callout (balloon) with or without a leader",
            description: "The Callout tool and its dialog: with `anchor` (a click on an edge or point, sheet mm) the leader starts there (in an assembly view an empty `text` shows the part's BOM item number); without it the callout sits alone at `position`. One edit (\"Callout\").",
            fields: with(
                common(),
                vec![
                    opt("anchor", point(), "Where the leader starts (sheet mm). Omit for a callout without a leader."),
                    req("position", point(), "Where the callout goes (sheet mm)."),
                    opt("text", json!({ "type": "string" }), "Centre text (empty: the BOM item number in assembly views)."),
                    opt("above", json!({ "type": "string" }), "Text above."),
                    opt("below", json!({ "type": "string" }), "Text below."),
                    opt("left", json!({ "type": "string" }), "Text left."),
                    opt("right", json!({ "type": "string" }), "Text right."),
                    opt("border", json!({ "type": "string", "enum": BORDERS.iter().map(|b| b.0).collect::<Vec<_>>(), "default": "circle" }), "Border shape."),
                    opt("chars", json!({ "type": "integer", "minimum": 0, "maximum": 20 }), "Border size in characters (0: tight fit)."),
                    opt("height", json!({ "type": "number", "exclusiveMinimum": 0 }), "Text height (mm)."),
                ],
            ),
            mutates: true,
            destructive: false,
            run: callout,
        },
        Op {
            group: "drawing",
            verb: "ordinate",
            summary: "Ordinate dimensions from a datum point",
            description: "The Ordinate dimension tool: `origin` is the datum (a vertex or circle center, sheet mm), `side` a click that sets the measured direction (beside the origin: horizontal values; above or below: vertical), then each of `points` adds an ordinate to the group. One edit per click, as with the mouse (\"Ordinate dimension\", then \"Add to ordinate group\").",
            fields: with(
                common(),
                vec![req("origin", point(), "The datum point (sheet mm)."), req("side", point(), "Where the values line up (sheet mm)."), opt("points", points(0, 64), "Points to dimension (vertices, circle centers; sheet mm).")],
            ),
            mutates: true,
            destructive: false,
            run: ordinate,
        },
        Op {
            group: "drawing",
            verb: "datum",
            summary: "Datum feature symbol",
            description: "The Datum tool: the triangle on the geometry at `anchor` (a vertex or edge point, sheet mm), the label box at `position` (at least 3 mm away). The label is the next free letter (A, B, …; I, O, Q, S, X, Z skipped) unless given. One edit (\"Datum\").",
            fields: with(common(), vec![req("anchor", point(), "Where the triangle attaches (sheet mm)."), req("position", point(), "Where the label box goes (sheet mm)."), opt("label", json!({ "type": "string" }), "Datum letter.")]),
            mutates: true,
            destructive: false,
            run: datum,
        },
        Op {
            group: "drawing",
            verb: "centermark",
            summary: "Centermarks: single, circular (bolt circle) or linear",
            description: "The Centermark tool and dialog: `holes` are clicks on hole circles of one view (sheet mm). single: a mark on each, one edit each (\"Centermark\"). circular: marks aligned to the pattern, with the bolt circle and reference center (and radial lines) as the dialog's options; linear: marks with connecting lines; one edit (\"Circular centermark\" / \"Linear centermark\").",
            fields: with(
                common(),
                vec![
                    opt("kind", json!({ "type": "string", "enum": ["single", "circular", "linear"], "default": "single" }), "Centermark type."),
                    req("holes", points(1, 64), "A point on each hole's circle (sheet mm)."),
                    opt("reference_center", json!({ "type": "boolean", "default": true }), "Circular: mark the pattern's center."),
                    opt("bolt_circle", json!({ "type": "boolean", "default": true }), "Circular: the bolt circle through the holes."),
                    opt("radial_lines", json!({ "type": "boolean", "default": false }), "Circular: lines from the center to each hole."),
                    opt("align_to_pattern", json!({ "type": "boolean", "default": true }), "Circular: marks rotated to the pattern."),
                    opt("connecting_lines", json!({ "type": "boolean", "default": true }), "Linear: lines between the marks."),
                ],
            ),
            mutates: true,
            destructive: false,
            run: centermark,
        },
        Op {
            group: "drawing",
            verb: "note",
            summary: "Note (text) with an optional leader",
            description: "The Note tool and panel: `text` (several lines with \\n) placed at `position` (sheet mm); with `leader` (a vertex, circle center or edge point, sheet mm) a leader points there. `height` sets the text height (sheet mm; it becomes the drawing's note height default, as typing it in the panel does). One edit (\"Note\").",
            fields: with(
                common(),
                vec![
                    req("text", json!({ "type": "string", "minLength": 1 }), "The note's text."),
                    req("position", point(), "Where the text goes (sheet mm)."),
                    opt("leader", point(), "Where the leader points (a snap point, sheet mm)."),
                    opt("height", json!({ "type": "number", "exclusiveMinimum": 0 }), "Text height (mm)."),
                ],
            ),
            mutates: true,
            destructive: false,
            run: note,
        },
        Op {
            group: "drawing",
            verb: "gdt",
            summary: "Geometric tolerance (feature control frame)",
            description: Box::leak(
                format!(
                    "The Geometric tolerance tool and dialog: the frame at `position` (sheet mm). Each row: `symbol` ({}; later rows default to composite), `tolerance` with optional `before` / `after` (e.g. \"Ø\", \"Ⓜ\"), up to three `datums` (\"A\", \"B Ⓜ\"), and `prefix` / `suffix` texts; `upper` / `lower` texts above and below. One edit (\"Geometric tolerance\").",
                    GDT_SYMBOLS.iter().map(|s| s.0).collect::<Vec<_>>().join(", ")
                )
                .into_boxed_str(),
            ),
            fields: with(
                common(),
                vec![
                    req("position", point(), "Where the frame goes (sheet mm)."),
                    req(
                        "rows",
                        json!({ "type": "array", "minItems": 1, "maxItems": 4, "items": { "type": "object", "properties": {
                            "symbol": { "type": "string", "enum": GDT_SYMBOLS.iter().map(|s| s.0).chain(["composite"]).collect::<Vec<_>>() },
                            "tolerance": { "type": "string" }, "before": { "type": "string" }, "after": { "type": "string" },
                            "datums": { "type": "array", "items": { "type": "string" }, "maxItems": 3 },
                            "prefix": { "type": "string" }, "suffix": { "type": "string" } } } }),
                        "Frame rows, top to bottom.",
                    ),
                    opt("upper", json!({ "type": "string" }), "Text above the frame."),
                    opt("lower", json!({ "type": "string" }), "Text below the frame."),
                    opt("leader", json!({ "type": "string", "enum": ["leader", "all_around", "all_over"], "default": "leader" }), "Leader type."),
                ],
            ),
            mutates: true,
            destructive: false,
            run: gdt,
        },
        Op {
            group: "drawing",
            verb: "view_table",
            summary: "Place a BOM, cut list or hole table of a view",
            description: "The BOM table, Cut list table and Hole table tools: the table of `view` with its top-left corner at `position` (sheet mm). A hole table measures from `origin` (a vertex or circle center of the view, sheet mm). One edit with the tool's name.",
            fields: with(
                common(),
                vec![
                    req("kind", json!({ "type": "string", "enum": ["bom", "cut_list", "hole"] }), "Table tool."),
                    view_id("view", "The view the table lists."),
                    opt("origin", point(), "Hole table: the origin vertex (sheet mm)."),
                    req("position", point(), "Where the table goes (sheet mm)."),
                ],
            ),
            mutates: true,
            destructive: false,
            run: view_table,
        },
    ]
}

// ------------------------------------------------------------ helpers

struct Target {
    tab: noshape_model::document::TabId,
    sheet: usize,
    d: Drawing,
    m: SheetModel,
}

fn target(od: &mut OpenDoc, a: &Args) -> Result<Target, OpError> {
    let tab = drawing_tab(od, a)?;
    let d = dr::drawing_of(od.doc(), tab).cloned().expect("drawing");
    let sheet = sheet_index(&d, a)?;
    let m = SheetModel::build(&od.session, tab, d.clone(), sheet, &[]);
    Ok(Target { tab, sheet, d, m })
}

fn pt_arg(a: &Args, name: &str) -> Result<[f64; 2], OpError> {
    point_value(a.get(name).ok_or_else(|| OpError::param(name, "required"))?, name)
}

fn view_arg(t: &Target, a: &Args, name: &str) -> Result<ViewId, OpError> {
    let id = ViewId(a.opt_u64(name)?.ok_or_else(|| OpError::param(name, "a view id (drawing_get)"))?);
    t.d.sheets[t.sheet].view(id).map(|_| id).ok_or_else(|| OpError::param(name, "no such view on the sheet"))
}

fn refused(t: &Target, tool: &str, view: ViewId, param: &str) -> Result<(), OpError> {
    match t.m.pick(&Tool::Select, &[], [0.0; 2]).view_refusal(tool, view) {
        Some(why) => Err(OpError::param(param, why)),
        None => Ok(()),
    }
}

fn enum_arg<T: Copy>(a: &Args, name: &str, options: &[(&str, T)], default: T) -> Result<T, OpError> {
    match a.opt_str(name)? {
        None => Ok(default),
        Some(s) => options.iter().find(|o| o.0 == s).map(|o| o.1).ok_or_else(|| OpError::param(name, &format!("one of {}", options.iter().map(|o| o.0).collect::<Vec<_>>().join(", ")))),
    }
}

/// Commit, then reply with the sheet and the new item's id under `key`.
fn done(od: &mut OpenDoc, t: Target, description: &str, key: &str, id: u64) -> Result<OpOutput, OpError> {
    commit(od, t.tab, t.sheet, t.d, description)?;
    let mut out = reply(od, t.tab, t.sheet, false)?;
    out[key] = json!(id);
    Ok(out.into())
}

/// The snap point (vertex, circle center...) of view `view` at `p`.
fn snap_in(t: &Target, tool: &Tool, view: ViewId, p: [f64; 2]) -> Option<PickedPoint> {
    t.m.pick(tool, &[], p).snap_at(p).filter(|s| s.view == view)
}

// ------------------------------------------------------------ views

fn section_view(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let t = target(od, a)?;
    let parent = view_arg(&t, a, "parent")?;
    let partial = a.bool_or("partial", false)?;
    let dir = enum_arg(a, "direction", &[("vertical", 0u8), ("horizontal", 1), ("angular", 2)], 0)?;
    if dir == 2 && !partial {
        refused(&t, "angular_section", parent, "parent")?;
    }
    let p = pt_arg(a, "point")?;
    let tool0 = Tool::Section { parent: None, p0: None, line: None, partial, jogs: vec![] };
    let pk = t.m.pick(&tool0, &[], p);
    let pl = pk.placement(parent).ok_or_else(|| OpError::param("parent", "the view isn't drawn"))?;
    let p0 = snap_in(&t, &tool0, parent, p).map(|s| s.p).unwrap_or(pl.to_view(p));
    let line = if partial {
        // The second click: snapped to horizontal / vertical, then to a snap point.
        let end = pt_arg(a, "end").map_err(|_| OpError::param("end", "a partial section needs the line's other end"))?;
        let s0 = pl.to_sheet(p0);
        let b = dt::ortho_point(s0, end);
        if dt::geom::len(dt::geom::sub(b, s0)) < 2.0 {
            return Err(OpError::param("end", "the line must be at least 2 mm long"));
        }
        (p0, snap_in(&t, &tool0, parent, end).map(|s| s.p).unwrap_or(pl.to_view(b)))
    } else {
        // Section view dialog › Vertical / Horizontal / Angular: one click places the whole line.
        let axis = match dir {
            0 => [0.0, 1.0],
            1 => [1.0, 0.0],
            _ => {
                let ang = a.opt_f64("angle")?.unwrap_or(45.0).to_radians() * if a.bool_or("opposite", false)? { -1.0 } else { 1.0 };
                [ang.cos(), ang.sin()]
            }
        };
        pk.extend_cut(parent, pl.to_sheet(p0), axis).ok_or_else(|| OpError::param("parent", "the view isn't drawn"))?
    };
    let mut jogs: Vec<[f64; 2]> = Vec::new();
    if let Some(list) = a.opt_array("jogs")? {
        for (i, j) in list.iter().enumerate() {
            let param = format!("jogs[{i}]");
            let c = point_value(j, &param)?;
            let tool = Tool::Section { parent: Some(parent), p0: Some(p0), line: Some(line), partial, jogs: jogs.clone() };
            let q = t.m.pick(&tool, &[], c).section_jog_at(parent, c).filter(|_| !partial).ok_or_else(|| OpError::param(&param, "not a snap point of the view off the cutting line"))?;
            jogs.push(q);
        }
    }
    let mut section = SectionOpts { cut_only: a.bool_or("cut_only", false)?, flip: a.bool_or("flip", false)?, cut_from: a.bool_or("cut_from_section", true)?, ..Default::default() };
    if let Some(depth) = a.opt_f64("depth")? {
        section.depth_on = true;
        section.depth_value = Some(depth / 1000.0);
    }
    if let Some(to) = a.get("depth_to") {
        let c = point_value(to, "depth_to")?;
        let r = snap_in(&t, &tool0, parent, c).and_then(|s| s.r).ok_or_else(|| OpError::param("depth_to", "Select a vertex or circle center for the section depth"))?;
        section.depth_on = true;
        section.up_to = true;
        section.depth_ref = Some(r);
    }
    if let Some(list) = a.opt_array("exclude")? {
        for (i, b) in list.iter().enumerate() {
            match b.as_str().and_then(refs::parse_str) {
                Some(noshape_model::reference::Reference::Body(b)) => section.exclude.push(b),
                _ => return Err(OpError::param(&format!("exclude[{i}]"), "a part id (`body:…`)")),
            }
        }
    }
    // A partial section has no dialog.
    let opts = ViewOptions { section: (!partial).then_some(section), ..Default::default() };
    let tool = Tool::Section { parent: Some(parent), p0: Some(p0), line: Some(line), partial, jogs };
    let position = pt_arg(a, "position")?;
    let mut v = t.m.pick(&tool, &[], position).pending_view(&opts).ok_or_else(|| OpError::param("position", "the section view can't be placed there"))?;
    if let (ViewKind::Section { label, .. }, Some(l)) = (&mut v.kind, a.opt_str("label")?.map(str::trim).filter(|l| !l.is_empty())) {
        *label = l.to_string();
    }
    let id = v.id;
    let mut t = t;
    t.d.sheets[t.sheet].views.push(v);
    done(od, t, "Section view", "view", id.0)
}

fn detail_view(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let t = target(od, a)?;
    let parent = view_arg(&t, a, "parent")?;
    let c = pt_arg(a, "center")?;
    let tool0 = Tool::Detail { parent: None, center: None, radius: None };
    let pl = t.m.pick(&tool0, &[], c).placement(parent).ok_or_else(|| OpError::param("parent", "the view isn't drawn"))?;
    let center = snap_in(&t, &tool0, parent, c).map(|s| s.p).unwrap_or(pl.to_view(c));
    let r = a.opt_f64("radius")?.unwrap_or(0.0) / (pl.scale * 1000.0);
    if r <= 1e-6 {
        return Err(OpError::param("radius", "must be greater than zero"));
    }
    let props = &t.d.props.view_tab;
    let style = enum_arg(a, "style", &[("in_line", 0u8), ("connected", 1), ("leader", 2)], props.detail_style)?;
    let detail = DetailOpts {
        scale: a.opt_f64("scale")?,
        profile: None,
        style,
        show_profile: a.bool_or("show_profile", props.detail_show_profile)?,
        clip_border: a.bool_or("clip_border", props.detail_clip_border)?,
        label: a.opt_str("label")?.unwrap_or("").trim().to_string(),
    };
    let opts = ViewOptions { detail: Some(detail), ..Default::default() };
    let tool = Tool::Detail { parent: Some(parent), center: Some(center), radius: Some(r) };
    let position = pt_arg(a, "position")?;
    let v = t.m.pick(&tool, &[], position).pending_view(&opts).ok_or_else(|| OpError::param("position", "the detail view can't be placed there"))?;
    let id = v.id;
    let mut t = t;
    t.d.sheets[t.sheet].views.push(v);
    done(od, t, "Detail view", "view", id.0)
}

fn auxiliary_view(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let t = target(od, a)?;
    let e = pt_arg(a, "edge")?;
    let tool0 = Tool::Auxiliary { parent: None };
    let (view, dir, anchor) = t.m.pick(&tool0, &[], e).aux_edge().ok_or_else(|| OpError::param("edge", "Select a straight edge of a view"))?;
    refused(&t, "auxiliary", view, "edge")?;
    let letter = match a.opt_str("label")?.map(str::trim).filter(|l| !l.is_empty()) {
        Some(l) => l.to_string(),
        None => t.d.sheets[t.sheet].next_label(),
    };
    let aux = AuxOpts { reverse: a.bool_or("reverse", false)?, show_ref: a.bool_or("show_reference", true)?, anchor: Some(anchor), letter };
    let opts = ViewOptions { aux: Some(aux), ..Default::default() };
    let tool = Tool::Auxiliary { parent: Some((view, dir)) };
    let position = pt_arg(a, "position")?;
    let v = t.m.pick(&tool, &[], position).pending_view(&opts).ok_or_else(|| OpError::param("position", "an auxiliary view goes at least 15 mm from its parent, perpendicular to the edge"))?;
    let id = v.id;
    let mut t = t;
    t.d.sheets[t.sheet].views.push(v);
    done(od, t, "Auxiliary view", "view", id.0)
}

fn break_view(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let mut t = target(od, a)?;
    let view = view_arg(&t, a, "view")?;
    refused(&t, "break", view, "view")?;
    let vertical = enum_arg(a, "orientation", &[("vertical", true), ("horizontal", false)], true)?;
    let style = enum_arg(a, "style", &[("zigzag", BreakStyle::ZigZag), ("straight", BreakStyle::Straight), ("curve", BreakStyle::Curve), ("small_zigzag", BreakStyle::SmallZigZag)], BreakStyle::ZigZag)?;
    let gap = a.opt_f64("gap")?.unwrap_or(dt::default_break_gap_mm(t.d.units_mm));
    let pl = t.m.pick(&Tool::Select, &[], [0.0; 2]).placement(view).ok_or_else(|| OpError::param("view", "the view isn't drawn"))?;
    let b = dt::view_break(&pl, vertical, pt_arg(a, "from")?, pt_arg(a, "to")?, style, gap).ok_or_else(|| OpError::param("to", "the break band is empty"))?;
    let sheet = t.sheet;
    if let Some(v) = t.d.sheets[sheet].views.iter_mut().find(|v| v.id == view) {
        v.breaks.push(b);
    }
    done(od, t, "Break view", "view", view.0)
}

fn crop_view(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let mut t = target(od, a)?;
    let view = view_arg(&t, a, "view")?;
    refused(&t, "crop", view, "view")?;
    let circle = enum_arg(a, "shape", &[("rectangle", false), ("circle", true)], false)?;
    let f = pt_arg(a, "from")?;
    let tool0 = Tool::Crop { parent: Some(view), first: None, circle };
    let pk = t.m.pick(&tool0, &[], f);
    let f = pk.snap_at(f).and_then(|s| pk.pick_sheet(&s)).unwrap_or(f);
    let pl = pk.placement(view).ok_or_else(|| OpError::param("view", "the view isn't drawn"))?;
    let crop = dt::view_crop(&pl, circle, f, pt_arg(a, "to")?).ok_or_else(|| OpError::param("to", "the crop shape is empty"))?;
    let sheet = t.sheet;
    if let Some(v) = t.d.sheets[sheet].views.iter_mut().find(|v| v.id == view) {
        v.crop = Some(crop);
    }
    done(od, t, "Crop view", "view", view.0)
}

// ------------------------------------------------------------ annotations

fn clicks(a: &Args) -> Result<Vec<[f64; 2]>, OpError> {
    a.array("picks")?.iter().enumerate().map(|(i, p)| point_value(p, &format!("picks[{i}]"))).collect()
}

/// A straight edge under click `i`: its ends and (for virtual sharps) its
/// model edge.
fn edge_at(t: &Target, tool: &Tool, c: [f64; 2], i: usize) -> Result<(PickedPoint, PickedPoint, Option<dt::EdgeRef>), OpError> {
    let pk = t.m.pick(tool, &[], c);
    match (pk.line_pick(), pk.hovered_edge()) {
        (Some(Pick::Line { a, b }), Some((_, me))) => Ok((a, b, Some((me.body.clone(), (*me.name).clone())))),
        _ => Err(OpError::param(&format!("picks[{i}]"), "Select a straight edge")),
    }
}

fn centerline(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let mut t = target(od, a)?;
    let kind = a.str("kind")?.to_string();
    let cs = clicks(a)?;
    let c = *cs.last().expect("minItems 1");
    let id = t.d.sheets[t.sheet].next_annotation_id();
    let snap = |t: &Target, tool: &Tool, i: usize, first: Option<&PickedPoint>| -> Result<PickedPoint, OpError> {
        t.m.pick(tool, &[], cs[i]).snap_at(cs[i]).filter(|p| first.is_none_or(|f| f.view == p.view)).ok_or_else(|| OpError::param(&format!("picks[{i}]"), "not a vertex, circle center or midpoint (of the first pick's view)"))
    };
    let (ann, name) = match kind.as_str() {
        "two_point" => {
            if cs.len() != 2 {
                return Err(OpError::param("picks", "two points"));
            }
            let p = snap(&t, &Tool::Centerline { first: None }, 0, None)?;
            let q = snap(&t, &Tool::Centerline { first: Some(p.clone()) }, 1, Some(&p))?;
            (dt::centerline(id, &p, &q, c), "Centerline")
        }
        "edge_to_edge" => {
            let tool = Tool::EdgeCenterline { first: None };
            let (a1, b1, _) = edge_at(&t, &tool, cs[0], 0)?;
            let (p, q) = match cs.get(1) {
                None => t.m.pick(&tool, &[], cs[0]).silhouette_centerline(&a1, &b1).ok_or_else(|| OpError::param("picks", "one edge must be a cylinder or cone silhouette; else pick two edges"))?,
                Some(c2) => {
                    let tool = Tool::EdgeCenterline { first: Some((a1.clone(), b1.clone())) };
                    let (a2, b2, _) = edge_at(&t, &tool, *c2, 1)?;
                    if a2.view != a1.view {
                        return Err(OpError::param("picks[1]", "Select an edge in the same view"));
                    }
                    dt::edge_pair_centerline(a1.p, b1.p, a2.p, b2.p)
                }
            };
            (dt::edge_centerline(id, a1.view, p, q, c), "Edge to edge centerline")
        }
        "circle_3_point" | "circle_2_point" => {
            let three = kind == "circle_3_point";
            let need = if three { 3 } else { 2 };
            if cs.len() != need {
                return Err(OpError::param("picks", &format!("{need} points")));
            }
            let mut pts: Vec<PickedPoint> = Vec::new();
            for i in 0..need {
                let tool = Tool::CircleCenterline { three, points: pts.clone() };
                let p = snap(&t, &tool, i, pts.first())?;
                pts.push(p);
            }
            let ann = dt::circle_centerline(id, &pts, c).ok_or_else(|| OpError::param("picks", "the points make no circle"))?;
            (ann, if three { "3 point circle centerline" } else { "2 point circle centerline" })
        }
        _ => return Err(OpError::param("kind", "two_point, edge_to_edge, circle_3_point or circle_2_point")),
    };
    let sheet = t.sheet;
    t.d.sheets[sheet].annotations.push(ann);
    done(od, t, name, "annotation", id)
}

fn virtual_sharp(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let mut t = target(od, a)?;
    let cs = clicks(a)?;
    if cs.len() != 2 {
        return Err(OpError::param("picks", "a point on each of two edges"));
    }
    let first = edge_at(&t, &Tool::VirtualSharp { first: None }, cs[0], 0)?;
    let second = edge_at(&t, &Tool::VirtualSharp { first: Some(first.clone()) }, cs[1], 1)?;
    if first.0.view != second.0.view {
        return Err(OpError::param("picks[1]", "Select an edge in the same view"));
    }
    let id = t.d.sheets[t.sheet].next_annotation_id();
    let ann = dt::virtual_sharp(id, first.0.view, (&first.0, &first.1, &first.2), (&second.0, &second.1, &second.2), cs[1]).map_err(|why| OpError::param("picks", why))?;
    let sheet = t.sheet;
    t.d.sheets[sheet].annotations.push(ann);
    done(od, t, "Virtual sharp", "annotation", id)
}

fn view_table(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let mut t = target(od, a)?;
    let kind = enum_arg(a, "kind", &[("bom", TableTool::Bom), ("cut_list", TableTool::CutList), ("hole", TableTool::Hole)], TableTool::Bom)?;
    let view = view_arg(&t, a, "view")?;
    let origin = match (kind, a.get("origin")) {
        (TableTool::Hole, Some(o)) => {
            let c = point_value(o, "origin")?;
            Some(snap_in(&t, &Tool::Table { kind, view: Some(view), origin: None }, view, c).ok_or_else(|| OpError::param("origin", "not a vertex or circle center of the view"))?)
        }
        (TableTool::Hole, None) => return Err(OpError::param("origin", "a hole table needs its origin (a vertex of the view)")),
        _ => None,
    };
    let id = t.d.sheets[t.sheet].next_table_id();
    let (table, name) = dt::view_table(id, kind, view, origin, pt_arg(a, "position")?);
    let sheet = t.sheet;
    t.d.sheets[sheet].tables.push(table);
    done(od, t, name, "table", id)
}

// ------------------------------------------------------------ callouts, ordinates, datums

fn hole_callout(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let mut t = target(od, a)?;
    let h = pt_arg(a, "hole")?;
    let Some(Pick::Circle { view, center, radius, r }) = t.m.pick(&Tool::HoleCallout { circle: None }, &[], h).circle_pick(h) else {
        return Err(OpError::param("hole", "not on a hole's circle"));
    };
    let tool = Tool::HoleCallout { circle: Some((view, center, radius, r)) };
    let ann = t.m.pick(&tool, &[], pt_arg(a, "position")?).pending_annotation(&ToolValues::default()).ok_or_else(|| OpError::param("position", "the callout can't be placed there"))?;
    let (id, sheet) = (ann.id, t.sheet);
    t.d.sheets[sheet].annotations.push(ann);
    done(od, t, "Hole callout", "annotation", id)
}

fn callout(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let mut t = target(od, a)?;
    let text = |k: &str| -> Result<String, OpError> { Ok(a.opt_str(k)?.unwrap_or("").to_string()) };
    let mut style = CalloutStyle { above: text("above")?, below: text("below")?, left: text("left")?, right: text("right")?, border: enum_arg(a, "border", &BORDERS, CalloutBorder::Circle)?, ..Default::default() };
    if let Some(n) = a.opt_u64("chars")? {
        style.chars = n.min(20) as u8;
    }
    style.height = a.opt_f64("height")?;
    let position = pt_arg(a, "position")?;
    let centre = text("text")?;
    let ann = match a.get("anchor") {
        // Empty space: a callout without a leader, right there.
        None => dt::free_callout(t.d.sheets[t.sheet].next_annotation_id(), position, centre, style),
        Some(p) => {
            let c = point_value(p, "anchor")?;
            let anchor = t.m.pick(&Tool::Balloon { anchor: None }, &[], c).any_point(c).ok_or_else(|| OpError::param("anchor", "not on a view's geometry"))?;
            let tool = Tool::Balloon { anchor: Some(anchor.clone()) };
            let pk = t.m.pick(&tool, &[], position);
            let mut ann = pk.pending_annotation(&ToolValues { callout: Some((centre, style)), ..Default::default() }).ok_or_else(|| OpError::param("position", "the callout can't be placed there"))?;
            pk.finish_callout(&mut ann, &anchor);
            ann
        }
    };
    let (id, sheet) = (ann.id, t.sheet);
    t.d.sheets[sheet].annotations.push(ann);
    done(od, t, "Callout", "annotation", id)
}

fn ordinate(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let t = target(od, a)?;
    let o = pt_arg(a, "origin")?;
    let origin = t.m.pick(&Tool::Ordinate { origin: None, horizontal: None, group: None }, &[], o).any_point(o).ok_or_else(|| OpError::param("origin", "not a point of a view"))?;
    let tool = Tool::Ordinate { origin: Some(origin.clone()), horizontal: None, group: None };
    let group = t.m.pick(&tool, &[], pt_arg(a, "side")?).pending_annotation(&ToolValues::default()).ok_or_else(|| OpError::param("side", "at least 3 mm from the origin"))?;
    let AnnotationKind::OrdinateGroup { horizontal, .. } = group.kind else { return Err(OpError::new("internal", "not an ordinate group")) };
    let (gid, mut sheet, tab) = (group.id, t.sheet, t.tab);
    let mut t = t;
    t.d.sheets[sheet].annotations.push(group);
    commit(od, tab, sheet, t.d, "Ordinate dimension")?;
    // Each further click adds a point to the group (one edit each).
    for (i, p) in a.opt_array("points")?.cloned().unwrap_or_default().iter().enumerate() {
        let param = format!("points[{i}]");
        let c = point_value(p, &param)?;
        let t2 = target(od, a)?;
        sheet = t2.sheet;
        let tool = Tool::Ordinate { origin: Some(origin.clone()), horizontal: Some(horizontal), group: Some(gid) };
        let g = t2.m.pick(&tool, &[], c).pending_annotation(&ToolValues::default()).ok_or_else(|| OpError::param(&param, "not a new point of the origin's view (a vertex or circle center)"))?;
        let mut d = t2.d;
        if let Some(x) = d.sheets[sheet].annotations.iter_mut().find(|x| x.id == gid) {
            *x = g;
        }
        commit(od, tab, sheet, d, "Add to ordinate group")?;
    }
    let mut out = reply(od, tab, sheet, false)?;
    out["annotation"] = json!(gid);
    Ok(out.into())
}

fn datum(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let mut t = target(od, a)?;
    let c = pt_arg(a, "anchor")?;
    let pk = t.m.pick(&Tool::Datum { anchor: None }, &[], c);
    let anchor = pk.snap_at(c).and_then(|s| pk.pick_sheet(&s)).unwrap_or(c);
    let label = match a.opt_str("label")?.map(str::trim).filter(|l| !l.is_empty()) {
        Some(l) => l.to_string(),
        None => pk.next_datum_label(),
    };
    let tool = Tool::Datum { anchor: Some(anchor) };
    let ann = t.m.pick(&tool, &[], pt_arg(a, "position")?).pending_annotation(&ToolValues { datum_label: Some(label), ..Default::default() }).ok_or_else(|| OpError::param("position", "at least 3 mm from the anchor"))?;
    let (id, sheet) = (ann.id, t.sheet);
    t.d.sheets[sheet].annotations.push(ann);
    done(od, t, "Datum", "annotation", id)
}

fn centermark(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let mode = enum_arg(a, "kind", &[("single", 0u8), ("circular", 1), ("linear", 2)], 0)?;
    let d = dt::CentermarkOpts::default();
    let o = dt::CentermarkOpts {
        mode,
        reference_center: a.bool_or("reference_center", d.reference_center)?,
        bolt_circle: a.bool_or("bolt_circle", d.bolt_circle)?,
        radial_lines: a.bool_or("radial_lines", d.radial_lines)?,
        pattern: a.bool_or("align_to_pattern", d.pattern)?,
        connecting: a.bool_or("connecting_lines", d.connecting)?,
    };
    let holes: Vec<[f64; 2]> = a.array("holes")?.iter().enumerate().map(|(i, p)| point_value(p, &format!("holes[{i}]"))).collect::<Result<_, _>>()?;
    let t = target(od, a)?;
    let (tab, sheet) = (t.tab, t.sheet);
    let mut picks: Vec<dt::CmCircle> = Vec::new();
    for (i, h) in holes.iter().enumerate() {
        let Some(Pick::Circle { view, center, radius, r }) = t.m.pick(&Tool::Centermark, &[], *h).circle_pick(*h) else {
            return Err(OpError::param(&format!("holes[{i}]"), "not on a hole's circle"));
        };
        if picks.first().is_some_and(|p| p.view != view) {
            return Err(OpError::param(&format!("holes[{i}]"), "all holes must be in the same view"));
        }
        picks.push(dt::CmCircle { view, center, radius, r });
    }
    let mut d = t.d;
    let mut ids: Vec<u64> = Vec::new();
    if mode == 0 {
        // Single: each click places a mark (one edit each).
        for (c, h) in picks.iter().zip(&holes) {
            let id = d.sheets[sheet].next_annotation_id();
            d.sheets[sheet].annotations.push(dt::centermark(id, c, *h));
            ids.push(id);
            commit(od, tab, sheet, d.clone(), "Centermark")?;
        }
    } else {
        let (new, name) = dt::centermark_set(&picks, &o);
        let pos = *holes.last().expect("minItems 1");
        for (kind, refs) in new {
            let id = d.sheets[sheet].next_annotation_id();
            d.sheets[sheet].annotations.push(noshape_model::drawing::Annotation { id, kind, position: pos, refs, ..Default::default() });
            ids.push(id);
        }
        commit(od, tab, sheet, d, name)?;
    }
    let mut out = reply(od, tab, sheet, false)?;
    out["annotations"] = json!(ids);
    Ok(out.into())
}

fn note(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let mut t = target(od, a)?;
    let mut style = noshape_model::drawing::note::NoteStyle::default();
    // A first click on a snap point anchors a leader.
    if let Some(l) = a.get("leader") {
        let c = point_value(l, "leader")?;
        let p = t.m.pick(&Tool::Note, &[], c).snap_at(c).ok_or_else(|| OpError::param("leader", "not a vertex, circle center or edge point of a view"))?;
        style.leaders.push(noshape_model::drawing::Leader { view: Some(p.view), point: p.p, nodes: Vec::new() });
    }
    let sheet = t.sheet;
    let ann = dt::new_note(&mut t.d, sheet, a.str("text")?.to_string(), a.opt_f64("height")?, style, pt_arg(a, "position")?).ok_or_else(|| OpError::param("text", "a note needs text"))?;
    let id = ann.id;
    t.d.sheets[sheet].annotations.push(ann);
    done(od, t, "Note", "annotation", id)
}

fn gdt(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let mut t = target(od, a)?;
    let mut rows: Vec<dt::GdtRowValues> = Vec::new();
    for (i, r) in a.array("rows")?.iter().enumerate() {
        let p = format!("rows[{i}].");
        let text = |k: &str| -> Result<String, OpError> {
            match r.get(k) {
                None | Some(Value::Null) => Ok(String::new()),
                Some(Value::String(s)) => Ok(s.clone()),
                Some(_) => Err(OpError::param(&format!("{p}{k}"), "a string")),
            }
        };
        let symbol = match r.get("symbol").and_then(Value::as_str) {
            None | Some("composite") => None,
            Some(s) => Some(GDT_SYMBOLS.iter().find(|x| x.0 == s).map(|x| x.1).ok_or_else(|| OpError::param(&format!("{p}symbol"), "unknown symbol"))?),
        };
        let mut datums: [String; 3] = Default::default();
        if let Some(list) = r.get("datums").and_then(Value::as_array) {
            for (k, d) in list.iter().take(3).enumerate() {
                datums[k] = d.as_str().ok_or_else(|| OpError::param(&format!("{p}datums[{k}]"), "a string"))?.to_string();
            }
        }
        rows.push(dt::GdtRowValues { symbol, prefix: text("prefix")?, before: text("before")?, value: text("tolerance")?, after: text("after")?, datums, suffix: text("suffix")? });
    }
    let leader = enum_arg(a, "leader", &[("leader", GdtLeader::Leader), ("all_around", GdtLeader::AllAround), ("all_over", GdtLeader::AllOver)], GdtLeader::Leader)?;
    let (frame, extra) = dt::gdt_values(&rows, a.opt_str("upper")?.unwrap_or(""), a.opt_str("lower")?.unwrap_or(""), leader).ok_or_else(|| OpError::param("rows", "nothing entered"))?;
    let sheet = t.sheet;
    let id = t.d.sheets[sheet].next_annotation_id();
    t.d.sheets[sheet].annotations.push(dt::gdt(id, pt_arg(a, "position")?, frame, extra));
    done(od, t, "Geometric tolerance", "annotation", id)
}
