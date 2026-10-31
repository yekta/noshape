//! Drawing views and annotations beyond Insert / Projected view and the
//! dimensions: Section, Detail, Auxiliary, Break and Crop views,
//! centerlines, virtual sharps and view tables. Each runs the drawing
//! editor's own tool code (`noshape_scene::drawing_tools`) on the clicks
//! given (sheet mm) and commits with the editor's edit text.

use super::{doc_field, s};
use super::drawing::{commit, drawing_field, drawing_tab, point, point_value, reply, sheet_field, sheet_index};
use crate::drawing::{self as dr, SheetModel};
use crate::error::OpError;
use crate::registry::{Args, Op, OpOutput, opt, req};
use crate::{Ctx, OpenDoc, refs};
use noshape_model::drawing::{AnnotationKind, BreakStyle, CalloutBorder, CalloutStyle, Crop, Drawing, GdtLeader, TableCorner, ViewId, ViewKind};
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
            verb: "jogged_section_view",
            summary: "Jogged section view (an orthogonal cutting line through several points)",
            description: "The Jogged section view tool: clicks `points` (sheet mm, at least 2) on the parent view; each one continues the cutting line horizontally or vertically (in the view's frame) toward it, and finishing (Enter) extends both ends past the view. The section goes to `position`, projected from the first segment. One edit (\"Section view\").",
            fields: with(common(), vec![view_id("parent", "View to section."), req("points", points(2, 16), "The cutting line's clicks (sheet mm)."), req("position", point(), "Where the section view goes (sheet mm).")]),
            mutates: true,
            destructive: false,
            run: jogged_section_view,
        },
        Op {
            group: "drawing",
            verb: "aligned_section_view",
            summary: "Aligned section view (two cutting legs about a center, unfolded)",
            description: "The Aligned section view tool: `center` (sheet mm) on the parent view, then `a` and `b`, the directions of the two cutting legs from it (each leg is extended past the view). The view goes to `position`, projected from the first leg. One edit (\"Aligned section view\").",
            fields: with(common(), vec![view_id("parent", "View to section."), req("center", point(), "The legs' common point (sheet mm)."), req("a", point(), "A point along the first leg (sheet mm)."), req("b", point(), "A point along the second leg (sheet mm)."), req("position", point(), "Where the view goes (sheet mm).")]),
            mutates: true,
            destructive: false,
            run: aligned_section_view,
        },
        Op {
            group: "drawing",
            verb: "detail_view",
            summary: "Detail view of a region of a view (circle, rectangle, spline or polygon profile)",
            description: "The Detail view tool: the circle centered at `center` (sheet mm; a vertex or circle center there is snapped) with `radius` (sheet mm), or a rectangle / spline / polygon `profile` through `points`, on the parent view, shown at `scale` (default twice the parent's) at `position`. One edit (\"Detail view\").",
            fields: with(
                common(),
                vec![
                    view_id("parent", "View to detail."),
                    opt("profile", json!({ "type": "string", "enum": ["circle", "rectangle", "spline", "polygon"], "default": "circle" }), "The dialog's profile tool."),
                    opt("center", point(), "Circle: the center (sheet mm; a vertex or circle center there is snapped)."),
                    opt("radius", json!({ "type": "number", "exclusiveMinimum": 0 }), "Circle: the radius (sheet mm)."),
                    opt("points", json!({ "type": "array", "items": point() }), "Rectangle: two corners; spline / polygon: at least 3 points (sheet mm), closed."),
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
            verb: "edit_detail",
            summary: "Edit a detail view: move / resize its circle, its label and leader, Edit detail's options",
            description: "The detail circle's grips and menu and Edit detail... (live asb 10-23 views (1), (2)): `center` moves the circle (sheet mm, in its parent), `radius` resizes it (sheet mm; the detail view grows with it); `leader` true puts the label on a leader with an arrow to the circle (Add leader), false back beside it; `label_at` places the label on its leader (sheet mm); the Detail view panel's `scale`, `label`, `style`, `show_profile` and `clip_border`. One edit (\"Edit detail view\").",
            fields: with(
                common(),
                vec![
                    view_id("view", "The detail view (drawing_get)."),
                    opt("center", point(), "New circle centre (sheet mm)."),
                    opt("radius", json!({ "type": "number", "exclusiveMinimum": 0 }), "New circle radius (sheet mm)."),
                    opt("leader", json!({ "type": "boolean" }), "Label on a leader (Add leader) or beside the circle."),
                    opt("label_at", point(), "The label's position on its leader (sheet mm)."),
                    opt("scale", json!({ "type": "number", "exclusiveMinimum": 0 }), "View scale."),
                    opt("label", json!({ "type": "string", "minLength": 1 }), "View label (detail id)."),
                    opt("style", json!({ "type": "string", "enum": ["in_line", "connected", "leader"] }), "Profile style."),
                    opt("show_profile", json!({ "type": "boolean" }), "Show detail view profile."),
                    opt("clip_border", json!({ "type": "boolean" }), "Clip border to geometry."),
                ],
            ),
            mutates: true,
            destructive: false,
            run: edit_detail,
        },
        Op {
            group: "drawing",
            verb: "auxiliary_view",
            summary: "Auxiliary view: normal to an edge, in line with a sketch line / centerline, or down a hole's axis",
            description: "The Auxiliary view tool. `edge` is a click (sheet mm) on the fold line for `mode`: normal, a straight model edge of the parent view (the view looks along it); in_line, a sheet sketch line or a centerline annotation; axis, a hole's axis seen side-on (its centerline or silhouette; the view looks down it). The view goes to `position` (at least 15 mm from the parent, perpendicular to the fold line). One edit (\"Auxiliary view\").",
            fields: with(
                common(),
                vec![
                    opt("mode", json!({ "type": "string", "enum": ["normal", "in_line", "axis"], "default": "normal" }), "The dialog's tab."),
                    req("edge", point(), "A point on the fold line (sheet mm)."),
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
            description: "The Crop view tools: a rectangle with corners `from` and `to`, a circle centered at `from` through `to` (sheet mm; `from` snaps to a vertex there), or a closed polygon / spline through `points` (sheet mm, at least 3; vertices snap). One edit (\"Crop view\"); a view's new crop replaces its old one.",
            fields: with(
                common(),
                vec![
                    view_id("view", "View to crop."),
                    opt("shape", json!({ "type": "string", "enum": ["rectangle", "circle", "polygon", "spline"], "default": "rectangle" }), "Crop shape."),
                    opt("from", point(), "Rectangle corner / circle center (sheet mm)."),
                    opt("to", point(), "Opposite corner / a point on the circle (sheet mm)."),
                    opt("points", points(3, 1000), "Polygon / spline points (sheet mm)."),
                ],
            ),
            mutates: true,
            destructive: false,
            run: crop_view,
        },
        Op {
            group: "drawing",
            verb: "broken_out_section",
            summary: "Broken-out section: cut away a closed profile of a view",
            description: "The Broken-out section tool: the closed spline profile through `points` (sheet mm, at least 3; vertices snap) cut into the view to a blind `depth` from the model's nearest point (default 25 mm, 1 in in inch drawings, as the dialog), or up to the model point snapped at `depth_to` (plus `offset`). `index` edits the view's existing broken-out section instead. One edit (\"Broken-out section\"). Projected views of the view show the cut too.",
            fields: with(
                common(),
                vec![
                    opt("view", json!({ "type": "integer" }), "View to cut (default: the view under the first point)."),
                    req("points", points(3, 1000), "Profile points (sheet mm)."),
                    opt("depth", json!({ "type": ["string", "number"] }), "Blind depth: a length (\"5 mm\"; a bare number is in workspace units)."),
                    opt("depth_to", point(), "Up to entity: a model point (vertex, circle center...) of a view of the same model (sheet mm)."),
                    opt("offset", json!({ "type": ["string", "number"] }), "Up to entity: the Offset past the point (a length; negative stops short)."),
                    opt("exclude", json!({ "type": "array", "items": { "type": "string" } }), "Exclude from cut: parts left whole (`body:…`)."),
                    opt("index", json!({ "type": "integer", "minimum": 0 }), "Edit the view's broken-out section number `index` (0-based, drawing_get) instead of adding one."),
                ],
            ),
            mutates: true,
            destructive: false,
            run: broken_out_section,
        },
        Op {
            group: "drawing",
            verb: "edit_hatch",
            summary: "Edit hatch section...: restyle one hatch region of a view's cut",
            description: "The view menu's Edit hatch section... (live asb 10-23 views (4)): the hatch region of view `view`'s cut under `at` (sheet mm) takes `standard` (ansi, iso, general) and `pattern` (its name as listed, e.g. \"ANSI32 steel\", \"ISO02W100\", \"Solid\"; general \"None\" leaves the region unhatched), `scale` and `angle` (\"auto\" or numbers; angle in degrees, the pattern's own when auto) and `color` (\"#rrggbb\", default the drawing's); `reset` puts the region back to the drawing's hatch. Each region is separate. One edit (\"Edit hatch\").",
            fields: with(
                common(),
                vec![
                    view_id("view", "The view (drawing_get)."),
                    req("at", point(), "A point inside the hatch region (sheet mm)."),
                    opt("standard", json!({ "type": "string", "enum": ["ansi", "iso", "general"], "default": "ansi" }), "Pattern group (the dialog's tab)."),
                    opt("pattern", json!({ "type": "string" }), "Pattern name (default the group's first in the dialog's order)."),
                    opt("scale", json!({ "type": ["string", "number"], "default": "auto" }), "Scale (auto = 1)."),
                    opt("angle", json!({ "type": ["string", "number"], "default": "auto" }), "Angle in degrees (auto = the pattern's own)."),
                    opt("color", json!({ "type": "string" }), "Color \"#rrggbb\" (default the drawing's)."),
                    opt("reset", json!({ "type": "boolean", "default": false }), "Back to the drawing's section hatch."),
                ],
            ),
            mutates: true,
            destructive: false,
            run: edit_hatch,
        },
        Op {
            group: "drawing",
            verb: "element_properties",
            summary: "Read or set the drawing's own properties (Tab › Properties: part number, revision, description, titles)",
            description: "The drawing element's Properties (right-click its tab › Properties). Live (behaviours \"dx title block mappings\"): the title block's DWG NO. is the drawing's Part number and REV its Revision (\"-\" when unset), never the part's; TITLE shows the sheet reference part's Title 1–3; DRAWN's date is when the drawing was made (`created`). Without `set`, replies with them; `set` changes the ones it names (part_number, revision, description, title1, title2, title3; strings). One edit (\"Properties\").",
            fields: with(common(), vec![opt("set", json!({ "type": "object" }), "Properties to change: part_number, revision, description, title1, title2, title3.")]),
            mutates: true,
            destructive: false,
            run: element_properties,
        },
        Op {
            group: "drawing",
            verb: "title_field",
            summary: "Edit a title block field: its cell's note",
            description: "Double-click a title block cell (live qa4 round 5 s11 (4)): each field's cell is a note, editable while the formats are locked. `text` replaces field `field` (as drawing_get's `title_fields`, e.g. \"Title\", \"Drawn by\"; a key the layout lacks is added). Lines split on \\n; property codes like {part.name} expand. One edit (\"Edit title block\").",
            fields: with(
                common(),
                vec![
                    req("field", json!({ "type": "string" }), "The field (drawing_get `title_fields`)."),
                    req("text", json!({ "type": "string" }), "Its text."),
                ],
            ),
            mutates: true,
            destructive: false,
            run: title_field,
        },
        Op {
            group: "drawing",
            verb: "hatch_region",
            summary: "Hatch closed profiles of the sheet sketch or of views",
            description: "The Hatch region dialog: each of `points` (sheet mm) picks the smallest closed sheet-sketch profile around it, else the smallest closed region of the visible edges of the view there (not detail or break views); a region per profile. Pattern tab `standard` and `pattern` (its name as listed in the dialog, e.g. \"ANSI31 general\", or just its first word \"ANSI31\"; default the tab's default), `scale` and `angle` (\"Auto\" or a number; not on the General tab), `color` (default: the drawing's hatch color). `hatch` edits that hatch region instead (its profile replaced when `points` are given). One edit (\"Hatch region\" / \"Edit hatch\"); replies with the hatch ids.",
            fields: with(
                common(),
                vec![
                    opt("points", points(1, 100), "Points inside the profiles to hatch (sheet mm)."),
                    opt("standard", json!({ "type": "string", "enum": ["ansi", "iso", "general"], "default": "ansi" }), "Pattern tab."),
                    opt("pattern", json!({ "type": "string" }), "Pattern name."),
                    opt("scale", json!({ "type": ["string", "number"], "default": "Auto" }), "Pattern scale (\"Auto\" = 1)."),
                    opt("angle", json!({ "type": ["string", "number"], "default": "Auto" }), "Pattern angle in degrees (\"Auto\": the pattern's own)."),
                    opt("color", json!({ "type": "array", "items": { "type": "integer", "minimum": 0, "maximum": 255 }, "minItems": 3, "maxItems": 3 }), "Color [r, g, b] (default: the drawing's hatch color)."),
                    opt("hatch", json!({ "type": "integer" }), "Hatch region id to edit (drawing_get)."),
                ],
            ),
            mutates: true,
            destructive: false,
            run: hatch_region,
        },
        Op {
            group: "drawing",
            verb: "image",
            summary: "An image's context menu: Move to sheet, Show / Hide image borders",
            description: "Right-click an image: `move_to` (Move to sheet ›) moves image `image` to that sheet at the same place, numbered after its images (\"Move image to sheet\"; replies with that sheet and the new id); `borders` is Show image borders (true) / Hide image borders (false), for every image of the drawing (\"Image borders\"). Delete an image with drawing_delete.",
            fields: with(
                common(),
                vec![
                    req("image", json!({ "type": "integer" }), "Image id (drawing_get `images`)."),
                    opt("move_to", json!({ "type": ["integer", "string"] }), "Sheet to move it to: number (1 = first) or name."),
                    opt("borders", json!({ "type": "boolean" }), "Show (true) or hide (false) image borders."),
                ],
            ),
            mutates: true,
            destructive: false,
            run: image,
        },
        Op {
            group: "drawing",
            verb: "custom_table",
            summary: "Insert a custom table, or edit a custom table's properties",
            description: "The Custom table dialog: custom table `table` (its name or display name) defined by a Feature Studio of this document, computed on Part Studio `studio` (default: the Part Studio of `view`, else the first one shown in the drawing), placed with its `corner` at `position` (sheet mm). With `id`: Custom table properties of that table (Part Studio, table, order, corner; unset fields kept). One edit (\"Custom table\" / \"Custom table properties\"). Its rows are in drawing_get's tables.",
            fields: with(
                common(),
                vec![
                    opt("table", json!({ "type": "string" }), "Custom table name or display name."),
                    opt("feature_studio", json!({ "type": "string" }), "Feature Studio tab (name or id) defining it, when several define the name."),
                    opt("studio", json!({ "type": "string" }), "Part Studio tab (name or id)."),
                    opt("view", json!({ "type": "integer" }), "A view whose Part Studio to use."),
                    opt("position", point(), "Where the fixed corner goes (sheet mm; inserting)."),
                    opt("corner", json!({ "type": "string", "enum": ["top_left", "top_right", "bottom_left", "bottom_right"], "default": "top_right" }), "Fixed corner."),
                    opt("bottom_to_top", json!({ "type": "boolean", "default": false }), "Row order bottom to top."),
                    opt("id", json!({ "type": "integer" }), "Custom table to edit (drawing_get)."),
                ],
            ),
            mutates: true,
            destructive: false,
            run: custom_table,
        },
        Op {
            group: "drawing",
            verb: "insert_image",
            summary: "Insert an image on the sheet",
            description: "Toolbar › Insert image: image file tab `file` of the document (PNG, JPEG, GIF), or a local file `path` imported first as a file tab (its own edit \"Import <name>\", as the picker's Import). Placed as a drag from `position` to `to` (sheet mm; the image's aspect kept), or without `to` 30 mm high with its lower-left corner at `position`. One edit (\"Insert image\"); replies with the image id.",
            fields: with(
                common(),
                vec![
                    opt("file", json!({ "type": "string" }), "Image file tab (name or id)."),
                    opt("path", json!({ "type": "string" }), "Local image file to import and insert."),
                    req("position", point(), "Drag start / lower-left corner (sheet mm)."),
                    opt("to", point(), "Drag end (sheet mm)."),
                ],
            ),
            mutates: true,
            destructive: false,
            run: insert_image,
        },
        Op {
            group: "drawing",
            verb: "insert_cad",
            summary: "Insert a DXF or DWG as drawing sketch geometry",
            description: "Toolbar › Insert DXF or DWG: the model-space wireframe of DXF / DWG file tab `file` (or local file `path`, imported first as a file tab: its own edit \"Import <name>\") becomes sheet sketch geometry and its TEXT / MTEXT editable notes, at the file's own coordinates (its unit, else the drawing's; blocks and polylines exploded, colors dropped). One edit (\"Insert DXF or DWG\"); replies with the number of entities and of notes.",
            fields: with(
                common(),
                vec![opt("file", json!({ "type": "string" }), "DXF / DWG file tab (name or id)."), opt("path", json!({ "type": "string" }), "Local DXF / DWG file to import and insert.")],
            ),
            mutates: true,
            destructive: false,
            run: insert_cad,
        },
        Op {
            group: "drawing",
            verb: "sketch",
            summary: "Drawing sketch tools: line, circles, rectangles, tangent arc, spline, points, trim",
            description: "The drawing toolbar's sketch tools on the sheet sketch, on clicks `picks` (sheet mm) as in the editor: a click snaps to a sketch point (within 6 px), else to a view snap point (vertex, center...), and lines within 1.5° of horizontal / vertical get that constraint. line: a chain through the clicks; circle (center, then a point on it), corner_rectangle / center_rectangle (two corners / center and corner) and tangent_arc (a line or arc end point, then the arc's end): pairs of clicks; circle_3_point: triples; spline: one spline through all clicks; spline_point: each click on a spline adds a point there; point: a sketch point per click; trim: each click trims the curve under it (or deletes a point). One edit with the tool's name (\"Line\", \"Center point circle\", ...).",
            fields: with(
                common(),
                vec![
                    req("tool", json!({ "type": "string", "enum": ["line", "circle", "circle_3_point", "tangent_arc", "corner_rectangle", "center_rectangle", "spline", "spline_point", "point", "trim"] }), "Sketch tool."),
                    req("picks", points(1, 1000), "Clicks (sheet mm)."),
                ],
            ),
            mutates: true,
            destructive: false,
            run: drawing_sketch,
        },
        Op {
            group: "drawing",
            verb: "delete",
            summary: "Delete views, annotations, tables, images, hatches, sketch entities or view breaks",
            description: "Select and Delete on the sheet (ids from drawing_get). Deleting a view deletes its projected / section / detail children's link and its label; deleting an annotation deletes the callouts grouped with it; a datum shown from the model is hidden instead. One edit (\"Delete\").",
            fields: with(
                common(),
                vec![
                    opt("views", json!({ "type": "array", "items": { "type": "integer" } }), "View ids."),
                    opt("annotations", json!({ "type": "array", "items": { "type": "integer" } }), "Annotation ids (dimensions, notes, callouts...)."),
                    opt("tables", json!({ "type": "array", "items": { "type": "integer" } }), "Table ids."),
                    opt("images", json!({ "type": "array", "items": { "type": "integer" } }), "Image ids."),
                    opt("hatches", json!({ "type": "array", "items": { "type": "integer" } }), "Hatch region ids."),
                    opt("sketch_entities", json!({ "type": "array", "items": { "type": "integer" } }), "Drawing sketch entity ids."),
                    opt("breaks", json!({ "type": "array", "items": { "type": "object" } }), "View breaks: [{\"view\": id, \"index\": n}] (drawing_get lists a view's breaks)."),
                ],
            ),
            mutates: true,
            destructive: true,
            run: delete,
        },
        Op {
            group: "drawing",
            verb: "edit_dimension",
            summary: "Edit a dimension or hole callout: text, precision, units, tolerance, dual, parentheses",
            description: "The dimension palette (double-click a dimension or hole callout): text above / below, prefix, suffix, the value (typing it overrides a linear dimension's text; empty puts the measured value back), precision, units, tolerance and its values, Parentheses, Inspection dimension, Dual dimension, a radial dimension's radius / diameter; hole callouts: Override precision/tolerance. Tolerance values are in the dimension's units (or with units: \"0.05 mm\"). One edit (\"Edit dimension\" / \"Edit hole callout\").",
            fields: with(
                common(),
                vec![
                    req("annotation", json!({ "type": "integer" }), "Dimension or hole callout id (drawing_get)."),
                    opt("above", s(), "Text above."),
                    opt("below", s(), "Text below."),
                    opt("prefix", s(), "Prefix."),
                    opt("suffix", s(), "Suffix."),
                    opt("value", s(), "Dimensions: the value text (an override, drawn underlined; empty: the measured value)."),
                    opt("precision", json!({ "type": ["integer", "string"] }), "Decimals 0–8, or \"drawing\" (the drawing's)."),
                    opt("units", s(), "Millimeters, Centimeters, Meters, Inches, Inches fractional, Feet and inches, Feet, Yards, or \"drawing\"."),
                    opt("tolerance", s(), "None, Symmetrical, Deviation, Limits, Basic (not hole callouts), MIN, MAX, Fit, Fit with tolerance, Fit (Tolerance only)."),
                    opt("tolerance_value", json!({ "type": ["number", "string"] }), "Symmetrical: ± value."),
                    opt("upper", json!({ "type": ["number", "string"] }), "Deviation / Limits: upper value."),
                    opt("lower", json!({ "type": ["number", "string"] }), "Deviation / Limits: lower value."),
                    opt("hole_class", s(), "Fits: hole class (\"H7\")."),
                    opt("shaft_class", s(), "Fits: shaft class (\"g6\")."),
                    opt("parentheses", json!({ "type": "boolean" }), "Parentheses (a reference value)."),
                    opt("inspection", json!({ "type": "boolean" }), "Inspection dimension (oval frame)."),
                    opt("dual", json!({ "type": "string", "enum": ["single", "drawing", "dual"] }), "Single unit, Single unit (Drawing) or Dual unit."),
                    opt("dual_precision", json!({ "type": ["integer", "string"] }), "The dual row: decimals 0–8, or \"drawing\"."),
                    opt("dual_units", s(), "The dual row: units as `units`, or \"drawing\"."),
                    opt("dual_show_units", json!({ "type": ["boolean", "string"] }), "The dual row's Show/hide units: true / false, or \"drawing\"."),
                    opt("diameter", json!({ "type": "boolean" }), "Radial dimensions: diameter (true) or radius (false)."),
                    opt("override_tolerance", json!({ "type": "boolean" }), "Hole callouts: Override precision/tolerance."),
                ],
            ),
            mutates: true,
            destructive: false,
            run: edit_dimension,
        },
        Op {
            group: "drawing",
            verb: "format_painter",
            summary: "Format painter: copy a dimension's or annotation's format to others",
            description: "The Format painter dialog: from annotation `source` (a dimension, note, GD&T frame..., drawing_get ids) to each of `targets`, pasting its style (on by default), tolerance and precision, and / or content. One edit (\"Format painter\").",
            fields: with(
                common(),
                vec![
                    req("source", json!({ "type": "integer" }), "Annotation to copy from."),
                    req("targets", json!({ "type": "array", "items": { "type": "integer" }, "minItems": 1 }), "Annotations to paint."),
                    opt("style", json!({ "type": "boolean", "default": true }), "Paste style."),
                    opt("tolerance_precision", json!({ "type": "boolean", "default": false }), "Paste tolerance and precision."),
                    opt("content", json!({ "type": "boolean", "default": false }), "Paste content."),
                ],
            ),
            mutates: true,
            destructive: false,
            run: format_painter,
        },
        Op {
            group: "drawing",
            verb: "inspection",
            summary: "Inspection items: number annotations (manual, reorder, automatic), edit, move, remove, hide, border shape",
            description: "The Inspection item tool and its menus. add: number `annotations` in order, from `number` (Reorder inspection items) or the drawing's next number (\"Inspection item\"). automatic: number every dimension and annotation of `scope` (drawing, sheet, or the view `view`) (\"Inspection items\"). edit: annotation `id` gets `number` (\"Edit inspection item\"). move: `id`'s number moves to annotation `to` (\"Move inspection item\"). remove: `annotations` lose theirs (\"Remove inspection item\"); remove_all (\"Remove all inspection items\"). hide / show: the toolbar's Hide inspection items toggle (\"Hide inspection items\" / \"Show inspection items\"). shape: the bubbles' border `shape` (\"Inspection border shape\"). drawing_get lists each annotation's `inspection` number.",
            fields: with(
                common(),
                vec![
                    req("action", json!({ "type": "string", "enum": ["add", "automatic", "edit", "move", "remove", "remove_all", "hide", "show", "shape"] }), "What to do."),
                    opt("annotations", json!({ "type": "array", "items": { "type": "integer" } }), "Annotations (add, remove)."),
                    opt("number", json!({ "type": "integer", "minimum": 1 }), "First number (add: reorder) / the number (edit)."),
                    opt("id", json!({ "type": "integer" }), "Annotation (edit, move)."),
                    opt("to", json!({ "type": "integer" }), "Annotation the number moves to (move)."),
                    opt("scope", json!({ "type": "string", "enum": ["drawing", "sheet", "view"], "default": "drawing" }), "Automatic scope."),
                    opt("view", json!({ "type": "integer" }), "View (scope view)."),
                    opt("shape", json!({ "type": "string", "enum": ["circle", "oval", "rectangle", "triangle", "flag"] }), "Border shape (shape)."),
                ],
            ),
            mutates: true,
            destructive: false,
            run: inspection,
        },
        Op {
            group: "drawing",
            verb: "properties",
            summary: "Read or change the Drawing properties (units, precision, text, arrows, views, tables, inspection...)",
            description: "The Drawing properties panel: without `set`, replies with the properties (`properties`: field names and values, lengths in mm; unset optional fields and false flags are left out). `set` changes the fields it names (same names and value shapes; null clears an optional one), as the panel does: with `locked` on before and after, nothing else changes; choosing `units` sets the drawing's metric flag. One edit (\"Drawing properties\").",
            fields: with(common(), vec![opt("set", json!({ "type": "object" }), "Fields to change (see the reply without `set`).")]),
            mutates: true,
            destructive: false,
            run: properties,
        },
        Op {
            group: "drawing",
            verb: "centerline",
            summary: "Centerlines: 2 point, edge to edge, 3 point / 2 point circle",
            description: "The centerline tools, on clicks `picks` (sheet mm): two_point (two points of one view: snap points, else the clicks themselves; centerline ends snap too), edge_to_edge (one cylinder silhouette edge gives its axis; or two edges give their middle line), circle_3_point (three snap points) and circle_2_point (center, then a point on the circle). Associative to the model points snapped. One edit with the tool's name.",
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
            description: "The Callout tool and its dialog: with `anchor` (a click on an edge or point, or on a dimension's lines or text: the callout is attached to it, follows it and goes when it is deleted; sheet mm) the leader starts there (in an assembly view an empty `text` shows the part's BOM item number); without it the callout sits alone at `position`. One edit (\"Callout\").",
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
            description: "The Ordinate dimension tool: `origin` is the datum (a vertex or circle center, sheet mm), `side` a click that sets the measured direction (beside the origin: horizontal values; above or below: vertical), then each of `points` adds an ordinate to the group. One edit per click, as with the mouse (\"Ordinate dimension\", then \"Add to ordinate group\"). With `group` (an ordinate dimension's id: its menu's Add to ordinate group), `points` go into that group, measured from its origin.",
            fields: with(
                common(),
                vec![
                    opt("origin", point(), "The datum point (sheet mm); required without `group`."),
                    opt("side", point(), "Where the values line up (sheet mm); required without `group`."),
                    opt("points", points(0, 64), "Points to dimension (vertices, circle centers; sheet mm)."),
                    opt("group", json!({ "type": "integer" }), "An existing ordinate dimension to add `points` to."),
                ],
            ),
            mutates: true,
            destructive: false,
            run: ordinate,
        },
        Op {
            group: "drawing",
            verb: "datum",
            summary: "Datum feature symbol",
            description: "The Datum tool: the triangle at `anchor` (sheet mm): on a GD&T frame edge or a dimension's lines when there (it follows them), else on the geometry (a vertex or edge point), the label box at `position` (at least 3 mm away). The label is the next free letter (A, B, …; I, O, Q, S, X, Z skipped) unless given. One edit (\"Datum\").",
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
                    "The Geometric tolerance tool and dialog: the frame at `position` (sheet mm). Each row: `symbol` ({}; later rows default to composite), `tolerance` with optional `before` / `after` (e.g. \"Ø\", \"Ⓜ\"), up to three `datums` (\"A\", \"B Ⓜ\"), and `prefix` / `suffix` texts; `upper` / `lower` texts above and below; `anchor`: its leader's point on the geometry. One edit (\"Geometric tolerance\").",
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
                    opt("anchor", point(), "Where the leader points (sheet mm: a view's edge, vertex or circle point snaps to it), as clicking the geometry first does; else no leader (drawing_annotation_edit add_leader adds more)."),
                ],
            ),
            mutates: true,
            destructive: false,
            run: gdt,
        },
        Op {
            group: "drawing",
            verb: "surface_finish",
            summary: "Surface finish symbol",
            description: "The Surface finish symbol tool and panel: the symbol at `position` (sheet mm) with `value` in its first right cell (the panel starts empty) and the rest of the dialog in `spec`. On a view's edge or a dimension's extension line it stands on it, turned square to it (drawing_annotation_edit `flip`: the other side); elsewhere it is free. One edit (\"Surface finish\").",
            fields: with(
                common(),
                vec![
                    req("position", point(), "Where the symbol sits (sheet mm)."),
                    opt("value", json!({ "type": "string", "default": "" }), "The first right cell (the roughness, e.g. \"Ra 3.2\")."),
                    opt(
                        "spec",
                        json!({ "type": "object" }),
                        "The rest of the dialog: `kind` (Basic, RemovalRequired, RemovalProhibited, IsoBasic, IsoRemovalRequired, IsoRemovalProhibited), `all_around`, `parens`, `left` (3 cells, top to bottom), `right` (the right cells after the first; more than 2 adds rows), `lay` (Angular, Circular, MultiDirectional, Parallel, Perpendicular, Protuberant, Radial).",
                    ),
                ],
            ),
            mutates: true,
            destructive: false,
            run: surface_finish,
        },
        Op {
            group: "drawing",
            verb: "weld",
            summary: "Weld symbol",
            description: Box::leak(
                format!(
                    "The Weld symbol tool and dialog: the arrow points at `target` (a vertex, edge or point of a view, sheet mm), the reference line starts at `position`. `spec` is the dialog: `arrow` and `other` sides ({{joint, size, depth, length, pitch, contour, finish, root, groove}}; joints: {}; contours: None, Flat, Convex, Concave; finishes: \"\", C, G, H, M, R, U), `spacer` ({}), `stagger`, `stagger_right`, `symmetric`, `flip`, `flip_id_line`, `all_around`, `flag` (None, Up, Down), `reference` (tail text). One edit (\"Weld symbol\").",
                    noshape_model::drawing::weld::WeldJoint::ALL.iter().map(|j| format!("{j:?}")).collect::<Vec<_>>().join(", "),
                    noshape_model::drawing::weld::JointSpacer::ALL.iter().map(|j| format!("{j:?}")).collect::<Vec<_>>().join(", ")
                )
                .into_boxed_str(),
            ),
            fields: with(common(), vec![req("target", point(), "What the arrow points at (sheet mm)."), req("position", point(), "Where the reference line starts (sheet mm)."), opt("spec", json!({ "type": "object" }), "The Weld symbol dialog (see the description); empty: no joint symbol.")]),
            mutates: true,
            destructive: false,
            run: weld,
        },
        Op {
            group: "drawing",
            verb: "revision_table",
            summary: "Revision table (rows from the document's releases)",
            description: "The Revision table tool and dialog: the table with its fixed corner at `position` (sheet mm). Its rows are the drawing's revisions from releases. One edit (\"Revision table\").",
            fields: with(
                common(),
                vec![
                    req("position", point(), "Where the fixed corner goes (sheet mm)."),
                    opt("corner", json!({ "type": "string", "enum": ["top_left", "top_right", "bottom_left", "bottom_right"], "default": "top_right" }), "The table's fixed corner."),
                    opt("visible_rows", json!({ "type": "integer", "minimum": 1, "default": 5 }), "Rows shown."),
                    opt("newest_top", json!({ "type": "boolean", "default": false }), "Newest revision at the top."),
                    opt("see_sheet1", json!({ "type": "boolean", "default": false }), "Show \"See sheet 1\" on other sheets."),
                    opt("hide_callouts", json!({ "type": "boolean", "default": false }), "Hide revision callouts."),
                ],
            ),
            mutates: true,
            destructive: false,
            run: revision_table,
        },
        Op {
            group: "drawing",
            verb: "revision_callout",
            summary: "Revision callout (triangle with the revision)",
            description: "Add revision callout: the triangle at `position` (sheet mm) with `revision` (default: the latest revision, else \"A\"). One edit (\"Revision callout\").",
            fields: with(common(), vec![req("position", point(), "Where the callout goes (sheet mm)."), opt("revision", json!({ "type": "string" }), "Revision label.")]),
            mutates: true,
            destructive: false,
            run: revision_callout,
        },
        Op {
            group: "drawing",
            verb: "table",
            summary: "General table (rows x columns of text)",
            description: "The Table tool and dialog: an empty table of `rows` x `columns` (defaults 2 x 2; 1–60 x 1–20) with an optional title row and header row, its fixed corner at `position` (sheet mm). One edit (\"Table\"). Fill it with drawing_table_cell.",
            fields: with(
                common(),
                vec![
                    req("position", point(), "Where the fixed corner goes (sheet mm)."),
                    opt("rows", json!({ "type": "integer", "minimum": 1, "maximum": 60, "default": 2 }), "Rows."),
                    opt("columns", json!({ "type": "integer", "minimum": 1, "maximum": 20, "default": 2 }), "Columns."),
                    opt("title_row", json!({ "type": "boolean", "default": false }), "A title row on top."),
                    opt("header_row", json!({ "type": "boolean", "default": false }), "A header row."),
                    opt("corner", json!({ "type": "string", "enum": ["top_left", "top_right", "bottom_left", "bottom_right"], "default": "top_right" }), "The table's fixed corner."),
                ],
            ),
            mutates: true,
            destructive: false,
            run: general_table,
        },
        Op {
            group: "drawing",
            verb: "table_cell",
            summary: "Type text into a general table cell",
            description: "The cell editor of a general table: `text` into cell (`row`, `column`), counted from 0 (the header row, if any, is row 0). The table grows when the cell lies beyond it. One edit (\"Edit cell\").",
            fields: with(common(), vec![req("table", json!({ "type": "integer" }), "Table id (drawing_get)."), req("row", json!({ "type": "integer", "minimum": 0 }), "Row."), req("column", json!({ "type": "integer", "minimum": 0 }), "Column."), req("text", json!({ "type": "string" }), "Cell text.")]),
            mutates: true,
            destructive: false,
            run: table_cell,
        },
        Op {
            group: "drawing",
            verb: "view_table",
            summary: "Place a BOM, cut list or hole table of a view",
            description: "The BOM table, Cut list table and Hole table tools: the table of `view` with its anchor corner at `position` (sheet mm): the reply's `corner`, a BOM's top right as in Onshape (A3: [410, 287] is the frame's top right). A reply `warning` says when the table runs off the sheet. A hole table measures from `origin` (a vertex or circle center of the view, sheet mm). One edit with the tool's name.",
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

pub(super) struct Target {
    pub(super) tab: noshape_model::document::TabId,
    pub(super) sheet: usize,
    pub(super) d: Drawing,
    pub(super) m: SheetModel,
}

pub(super) fn target(od: &mut OpenDoc, a: &Args) -> Result<Target, OpError> {
    let tab = drawing_tab(od, a)?;
    let d = dr::drawing_of(od.doc(), tab).cloned().expect("drawing");
    let sheet = sheet_index(&d, a)?;
    let m = SheetModel::build(&od.session, tab, d.clone(), sheet, &[]);
    Ok(Target { tab, sheet, d, m })
}

pub(super) fn pt_arg(a: &Args, name: &str) -> Result<[f64; 2], OpError> {
    point_value(a.get(name).ok_or_else(|| OpError::param(name, "required"))?, name)
}

pub(super) fn view_arg(t: &Target, a: &Args, name: &str) -> Result<ViewId, OpError> {
    let id = ViewId(a.opt_u64(name)?.ok_or_else(|| OpError::param(name, "a view id (drawing_get)"))?);
    t.d.sheets[t.sheet].view(id).map(|_| id).ok_or_else(|| OpError::param(name, "no such view on the sheet"))
}

fn refused(t: &Target, tool: &str, view: ViewId, param: &str) -> Result<(), OpError> {
    match t.m.pick(&Tool::Select, &[], [0.0; 2]).view_refusal(tool, view) {
        Some(why) => Err(OpError::param(param, why)),
        None => Ok(()),
    }
}

pub(super) fn enum_arg<T: Copy>(a: &Args, name: &str, options: &[(&str, T)], default: T) -> Result<T, OpError> {
    match a.opt_str(name)? {
        None => Ok(default),
        Some(s) => options.iter().find(|o| o.0 == s).map(|o| o.1).ok_or_else(|| OpError::param(name, &format!("one of {}", options.iter().map(|o| o.0).collect::<Vec<_>>().join(", ")))),
    }
}

/// Commit, then reply with the sheet and the new item's id under `key`.
pub(super) fn done(od: &mut OpenDoc, t: Target, description: &str, key: &str, id: u64) -> Result<OpOutput, OpError> {
    commit(od, t.tab, t.sheet, t.d, description)?;
    let mut out = reply(od, t.tab, t.sheet, false)?;
    out[key] = json!(id);
    // A table placed (partly) off the sheet: say so, with its anchor corner.
    if key == "table"
        && let Some(tb) = out["tables"].as_array().and_then(|l| l.iter().find(|x| x["id"] == id)).cloned()
    {
        let (w, h) = (out["size_mm"][0].as_f64().unwrap_or(f64::MAX), out["size_mm"][1].as_f64().unwrap_or(f64::MAX));
        let b = |k: &str, i: usize| tb["bounds"][k][i].as_f64().unwrap_or(0.0);
        if b("min", 0) < 0.0 || b("min", 1) < 0.0 || b("max", 0) > w || b("max", 1) > h {
            out["warning"] = json!(format!("the table runs off the {w} x {h} mm sheet (from [{:.1}, {:.1}] to [{:.1}, {:.1}]): `position` is its {} corner", b("min", 0), b("min", 1), b("max", 0), b("max", 1), tb["corner"].as_str().unwrap_or("anchor")));
        }
    }
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
    let snap0 = snap_in(&t, &tool0, parent, p);
    let p0 = snap0.as_ref().map(|s| s.p).unwrap_or(pl.to_view(p));
    // On a model point the line follows it (live request 1625).
    let anchor = snap0.and_then(|s| s.r);
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
    let mut section = SectionOpts { cut_only: a.bool_or("cut_only", false)?, flip: a.bool_or("flip", false)?, cut_from: a.bool_or("cut_from_section", true)?, anchor, ..Default::default() };
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
    let shape = enum_arg(a, "profile", &[("circle", 0u8), ("rectangle", 1), ("spline", 2), ("polygon", 3)], 0)?;
    let tool0 = Tool::Detail { parent: None, center: None, radius: None };
    let (center, r, profile) = if shape == 0 {
        let c = pt_arg(a, "center")?;
        let pl = t.m.pick(&tool0, &[], c).placement(parent).ok_or_else(|| OpError::param("parent", "the view isn't drawn"))?;
        let center = snap_in(&t, &tool0, parent, c).map(|s| s.p).unwrap_or(pl.to_view(c));
        let r = a.opt_f64("radius")?.unwrap_or(0.0) / (pl.scale * 1000.0);
        if r <= 1e-6 {
            return Err(OpError::param("radius", "must be greater than zero"));
        }
        (center, r, None)
    } else {
        let pts = a.opt_array("points")?.ok_or_else(|| OpError::param("points", "the profile's points"))?.iter().enumerate().map(|(i, p)| point_value(p, &format!("points[{i}]"))).collect::<Result<Vec<_>, _>>()?;
        let pl = pts.first().and_then(|p| t.m.pick(&tool0, &[], *p).placement(parent)).ok_or_else(|| OpError::param("parent", "the view isn't drawn"))?;
        let local: Vec<[f64; 2]> = pts.iter().map(|p| pl.to_view(*p)).collect();
        let (poly, center, r) = dt::detail_profile(shape, &local).ok_or_else(|| OpError::param("points", if shape == 1 { "two corners" } else { "at least 3 points" }))?;
        (center, r, Some(poly))
    };
    let props = &t.d.props.view_tab;
    let style = enum_arg(a, "style", &[("in_line", 0u8), ("connected", 1), ("leader", 2)], props.detail_style)?;
    let detail = DetailOpts {
        scale: a.opt_f64("scale")?,
        profile,
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
    let mode = enum_arg(a, "mode", &[("normal", 0u8), ("in_line", 1), ("axis", 2)], 0)?;
    let (view, dir, anchor) = t.m.pick(&tool0, &[], e).aux_pick(mode).ok_or_else(|| {
        OpError::param(
            "edge",
            match mode {
                1 => "Select a sheet sketch line or a centerline",
                2 => "Select a hole's axis (its centerline or silhouette) in a view",
                _ => "Select a straight edge of a view",
            },
        )
    })?;
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
    edit_view(&mut t, view, |v| v.breaks.push(b));
    done(od, t, "Break view", "view", view.0)
}

fn crop_view(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let mut t = target(od, a)?;
    let view = view_arg(&t, a, "view")?;
    refused(&t, "crop", view, "view")?;
    let shape = enum_arg(a, "shape", &[("rectangle", 0), ("circle", 1), ("polygon", 2), ("spline", 3)], 0)?;
    let crop = if shape >= 2 {
        let (_, pts) = profile(&t, a, Some(view))?;
        if shape == 3 { Crop::Spline(pts) } else { Crop::Polygon(pts) }
    } else {
        let circle = shape == 1;
        let f = pt_arg(a, "from")?;
        let tool0 = Tool::Crop { parent: Some(view), first: None, circle };
        let pk = t.m.pick(&tool0, &[], f);
        let f = pk.snap_at(f).and_then(|s| pk.pick_sheet(&s)).unwrap_or(f);
        let pl = pk.placement(view).ok_or_else(|| OpError::param("view", "the view isn't drawn"))?;
        dt::view_crop(&pl, circle, f, pt_arg(a, "to")?).ok_or_else(|| OpError::param("to", "the crop shape is empty"))?
    };
    edit_view(&mut t, view, |v| v.crop = Some(crop));
    done(od, t, "Crop view", "view", view.0)
}

/// Profile clicks `points` (sheet mm) as the profile tools take them: the
/// first picks the view (unless given), each snaps to a point of it.
fn profile(t: &Target, a: &Args, view: Option<ViewId>) -> Result<(ViewId, Vec<[f64; 2]>), OpError> {
    let cs: Vec<[f64; 2]> = a.array("points")?.iter().enumerate().map(|(i, p)| point_value(p, &format!("points[{i}]"))).collect::<Result<_, _>>()?;
    if cs.len() < 3 {
        return Err(OpError::param("points", "at least 3 points"));
    }
    let mut view = view;
    let mut pts = Vec::new();
    for (i, c) in cs.iter().enumerate() {
        let p = t.m.pick(&Tool::Select, &[], *c).view_point(*c, view).ok_or_else(|| OpError::param(&format!("points[{i}]"), "not in a view"))?;
        view = Some(p.view);
        pts.push(p.p);
    }
    Ok((view.expect("3 points"), pts))
}

/// Edit view `id` of the sheet (a mirror of a view on another sheet: the
/// real view), as the drawing editor does.
pub(super) fn edit_view(t: &mut Target, id: ViewId, f: impl FnOnce(&mut noshape_model::drawing::DrawingView)) {
    let (si, id) = t.d.real_view_of(t.sheet, id);
    if let Some(v) = t.d.sheets.get_mut(si).and_then(|s| s.view_mut(id)) {
        f(v);
    }
}

fn broken_out_section(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let units = od.session.doc.settings.units;
    let mut t = target(od, a)?;
    let given = match a.opt_u64("view")? {
        Some(_) => Some(view_arg(&t, a, "view")?),
        None => None,
    };
    let (view, pts) = profile(&t, a, given)?;
    if let Some(why) = dt::broken_out_refused(&t.d.sheets[t.sheet], view) {
        return Err(OpError::param("view", why));
    }
    let index = a.opt_u64("index")?.map(|i| i as usize);
    if let Some(i) = index
        && t.d.sheets[t.sheet].view(view).is_none_or(|v| i >= v.broken_out.len())
    {
        return Err(OpError::param("index", "the view has no broken-out section with that index"));
    }
    let depth_ref = match a.get("depth_to") {
        Some(v) => {
            let c = point_value(v, "depth_to")?;
            Some(t.m.pick(&Tool::Select, &[], c).snap_at(c).and_then(|p| p.r).ok_or_else(|| OpError::param("depth_to", "not a model point (vertex, circle center...)"))?)
        }
        None => None,
    };
    let depth = match a.get("depth") {
        Some(v) => Some(super::sketch::length_value(v, &units).map_err(|e| OpError::param("depth", &e.message))?),
        None if depth_ref.is_none() => Some(dt::broken_out_blind_default(noshape_model::drawing::format::length_units(&t.d, &Default::default()))),
        None => None,
    };
    let offset = match a.get("offset") {
        Some(_) if depth_ref.is_none() => return Err(OpError::param("offset", "Offset needs depth_to (Up to entity)")),
        Some(v) => super::sketch::length_value(v, &units).map_err(|e| OpError::param("offset", &e.message))?,
        None => 0.0,
    };
    let spline = index.and_then(|i| t.d.sheets[t.sheet].view(view)?.broken_out.get(i).map(|b| b.spline)).unwrap_or(true);
    let mut b = dt::broken_out(pts, spline, depth, depth_ref, offset).ok_or_else(|| OpError::param("depth", "Invalid depth"))?;
    if let Some(list) = a.opt_array("exclude")? {
        for (i, x) in list.iter().enumerate() {
            match x.as_str().and_then(refs::parse_str) {
                Some(noshape_model::reference::Reference::Body(x)) => b.exclude.push(x),
                _ => return Err(OpError::param(&format!("exclude[{i}]"), "a part id (`body:…`)")),
            }
        }
    }
    edit_view(&mut t, view, |v| dt::set_broken_out(v, index, b));
    done(od, t, "Broken-out section", "view", view.0)
}

// ------------------------------------------------------------ annotations

fn clicks(a: &Args) -> Result<Vec<[f64; 2]>, OpError> {
    a.array("picks")?.iter().enumerate().map(|(i, p)| point_value(p, &format!("picks[{i}]"))).collect()
}

/// A straight edge under click `i`: its ends and (for virtual sharps) its
/// model edge.
pub(super) fn edge_at(t: &Target, tool: &Tool, c: [f64; 2], i: usize) -> Result<(PickedPoint, PickedPoint, Option<dt::EdgeRef>), OpError> {
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
            // Snaps optional (help A50): else the click itself in the view.
            let free = |i: usize, view: Option<noshape_model::drawing::ViewId>| t.m.pick(&Tool::Centerline { first: None }, &[], cs[i]).centerline_point(cs[i], view).ok_or_else(|| OpError::param(&format!("picks[{i}]"), "not in a view"));
            let p = free(0, None)?;
            let q = free(1, Some(p.view))?;
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
            // On a dimension's lines or text, else on a view's geometry.
            let first = t.m.pick(&Tool::Balloon { anchor: None }, &[], c);
            let on = first.dimension_anchor(c);
            let anchor = match &on {
                Some(o) => o.0.clone(),
                None => first.any_point(c).ok_or_else(|| OpError::param("anchor", "not on a view's geometry or a dimension"))?,
            };
            let tool = Tool::Balloon { anchor: Some(anchor.clone()) };
            let pk = t.m.pick(&tool, &[], position);
            let mut ann = pk.pending_annotation(&ToolValues { callout: Some((centre, style)), datum_on: on.map(|o| o.1), ..Default::default() }).ok_or_else(|| OpError::param("position", "the callout can't be placed there"))?;
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
    let (origin, horizontal, gid, mut sheet, tab) = match a.opt_u64("group")? {
        // The group's menu › Add to ordinate group: its origin and direction.
        Some(g) => {
            if a.get("origin").is_some() || a.get("side").is_some() {
                return Err(OpError::param("group", "give `group` or `origin` and `side`, not both"));
            }
            let x = t.d.sheets[t.sheet].annotations.iter().find(|x| x.id == g).ok_or_else(|| OpError::param("group", "no such annotation on the sheet"))?;
            let AnnotationKind::OrdinateGroup { view, origin, horizontal, .. } = x.kind else { return Err(OpError::param("group", "not an ordinate dimension")) };
            if a.opt_array("points")?.is_none_or(|p| p.is_empty()) {
                return Err(OpError::param("points", "the points to add to the group"));
            }
            let r = x.refs.iter().find(|x| x.slot == 0).map(|x| x.point.clone());
            (PickedPoint { view, p: origin, r }, horizontal, g, t.sheet, t.tab)
        }
        None => {
            let o = pt_arg(a, "origin")?;
            let origin = t.m.pick(&Tool::Ordinate { origin: None, horizontal: None, group: None }, &[], o).any_point(o).ok_or_else(|| OpError::param("origin", "not a point of a view"))?;
            let tool = Tool::Ordinate { origin: Some(origin.clone()), horizontal: None, group: None };
            let group = t.m.pick(&tool, &[], pt_arg(a, "side")?).pending_annotation(&ToolValues::default()).ok_or_else(|| OpError::param("side", "at least 3 mm from the origin"))?;
            let AnnotationKind::OrdinateGroup { horizontal, .. } = group.kind else { return Err(OpError::new("internal", "not an ordinate group")) };
            let (gid, sheet, tab) = (group.id, t.sheet, t.tab);
            let mut t = t;
            t.d.sheets[sheet].annotations.push(group);
            commit(od, tab, sheet, t.d, "Ordinate dimension")?;
            (origin, horizontal, gid, sheet, tab)
        }
    };
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
    // On a GD&T frame edge or a dimension's lines, else on the geometry.
    let on = pk.datum_attach_at(c);
    // On a view's geometry: kept as that view point (it follows the view).
    let pp = if on.is_none() { pk.any_point(c) } else { None };
    let anchor = on.map(|o| o.0).or_else(|| pp.as_ref().and_then(|s| pk.pick_sheet(s))).unwrap_or(c);
    let label = match a.opt_str("label")?.map(str::trim).filter(|l| !l.is_empty()) {
        Some(l) => l.to_string(),
        None => pk.next_datum_label(),
    };
    let tool = Tool::Datum { anchor: Some(anchor) };
    let ann = t.m.pick(&tool, &[], pt_arg(a, "position")?).pending_annotation(&ToolValues { datum_label: Some(label), datum_on: on.map(|o| o.1), datum_at: pp.map(|s| (s.view, s.p)), ..Default::default() }).ok_or_else(|| OpError::param("position", "at least 3 mm from the anchor"))?;
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
        style.leaders.push(noshape_model::drawing::Leader { view: Some(p.view), point: p.p, nodes: Vec::new(), ..Default::default() });
    }
    let sheet = t.sheet;
    let ann = dt::new_note(&mut t.d, sheet, a.str("text")?.to_string(), a.opt_f64("height")?, style, pt_arg(a, "position")?).ok_or_else(|| OpError::param("text", "a note needs text"))?;
    let id = ann.id;
    t.d.sheets[sheet].annotations.push(ann);
    done(od, t, "Note", "annotation", id)
}

fn edit_hatch(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    use noshape_model::drawing::{SectionHatch, hatch_pat};
    let od = ctx.doc(a.opt_str("doc")?)?;
    let mut t = target(od, a)?;
    let view = view_arg(&t, a, "view")?;
    let si = t.sheet;
    let pl = t.m.built.as_ref().and_then(|b| b.placement(view).cloned()).ok_or_else(|| OpError::param("view", "the view isn't drawn"))?;
    let geo = t.m.geoms.get(&view).cloned().ok_or_else(|| OpError::param("view", "the view isn't drawn"))?;
    let at = pl.to_view(pt_arg(a, "at")?);
    let style = if a.bool_or("reset", false)? {
        None
    } else {
        let standard = enum_arg(a, "standard", &[("ansi", 0usize), ("iso", 1), ("general", 2)], 0)?;
        let pattern = match a.opt_str("pattern")? {
            Some(n) if standard == 2 && n.trim().eq_ignore_ascii_case("none") => None,
            Some(n) => Some(hatch_pat::find(standard, n).ok_or_else(|| OpError::param("pattern", "no such pattern in that group"))?),
            None => Some(hatch_pat::default_pattern(standard)),
        };
        // "auto" or a number: scale 1, the pattern's own angle.
        let num = |name: &str| -> Result<Option<f64>, OpError> {
            match a.get(name) {
                None => Ok(None),
                Some(v) if v.as_str().is_some_and(|s| s.trim().eq_ignore_ascii_case("auto")) => Ok(None),
                Some(v) => v.as_f64().or_else(|| v.as_str().and_then(|s| s.trim().trim_end_matches('°').parse().ok())).map(Some).ok_or_else(|| OpError::param(name, "\"auto\" or a number")),
            }
        };
        let scale = num("scale")?.unwrap_or(1.0);
        if scale <= 0.0 {
            return Err(OpError::param("scale", "must be > 0"));
        }
        let base = pattern.map(|p| hatch_pat::base_angle(standard, p)).unwrap_or(0.0);
        let angle = num("angle")?.map(|deg| deg - base).unwrap_or(0.0);
        let color = match a.opt_str("color")? {
            Some(c) => {
                let h = c.trim().trim_start_matches('#');
                let v = (h.len() == 6).then(|| u32::from_str_radix(h, 16).ok()).flatten().ok_or_else(|| OpError::param("color", "\"#rrggbb\""))?;
                Some([(v >> 16) as u8, (v >> 8) as u8, v as u8])
            }
            None => None,
        };
        Some(SectionHatch { at, standard, pattern, scale, angle, color })
    };
    let v = t.d.sheets[si].view_mut(view).expect("view");
    if !dt::set_section_hatch(v, &geo, at, style) {
        return Err(OpError::param("at", "not inside a hatch region of the view's cut"));
    }
    done(od, t, "Edit hatch", "view", view.0)
}

fn element_properties(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let mut t = target(od, a)?;
    let json_of = |m: &noshape_model::drawing::DrawingMeta| json!({ "part_number": m.number, "revision": m.revision, "description": m.description, "title1": m.title1, "title2": m.title2, "title3": m.title3, "created": m.created });
    let Some(set) = a.get("set") else { return Ok(json!({ "properties": json_of(&t.d.meta) }).into()) };
    let set = set.as_object().ok_or_else(|| OpError::param("set", "an object"))?;
    for (k, v) in set {
        let text = v.as_str().ok_or_else(|| OpError::param(&format!("set.{k}"), "a string"))?.to_string();
        let m = &mut t.d.meta;
        match k.as_str() {
            "part_number" => m.number = text,
            "revision" => m.revision = text,
            "description" => m.description = text,
            "title1" => m.title1 = text,
            "title2" => m.title2 = text,
            "title3" => m.title3 = text,
            _ => return Err(OpError::param(&format!("set.{k}"), "part_number, revision, description, title1, title2 or title3")),
        }
    }
    let (si, meta) = (t.sheet, t.d.meta.clone());
    commit(od, t.tab, si, t.d, "Properties")?;
    let mut v = reply(od, t.tab, si, false)?;
    v["properties"] = json_of(&meta);
    Ok(v.into())
}

fn title_field(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let mut t = target(od, a)?;
    let (key, text) = (a.str("field")?.to_string(), a.str("text")?.to_string());
    if key.trim().is_empty() {
        return Err(OpError::param("field", "empty"));
    }
    let si = t.sheet;
    dt::set_title_field(&mut t.d.sheets[si], &key, &text);
    commit(od, t.tab, si, t.d, "Edit title block")?;
    Ok(reply(od, t.tab, si, false)?.into())
}

fn edit_detail(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let mut t = target(od, a)?;
    let id = view_arg(&t, a, "view")?;
    let si = t.sheet;
    let parent = match t.d.sheets[si].view(id).map(|v| &v.kind) {
        Some(ViewKind::Detail { parent, .. }) => *parent,
        _ => return Err(OpError::param("view", "not a detail view")),
    };
    let ppl = t.m.built.as_ref().and_then(|b| b.placement(parent).cloned()).ok_or_else(|| OpError::param("view", "its parent view isn't drawn"))?;
    let center = a.get("center").map(|v| point_value(v, "center")).transpose()?;
    let radius = a.opt_f64("radius")?;
    let leader = a.get("leader").and_then(|v| v.as_bool());
    let label_at = a.get("label_at").map(|v| point_value(v, "label_at")).transpose()?;
    let style = if a.get("style").is_some() { Some(enum_arg(a, "style", &[("in_line", 0u8), ("connected", 1), ("leader", 2)], 0)?) } else { None };
    let (scale, label) = (a.opt_f64("scale")?, a.opt_str("label")?.map(str::to_string));
    let (show, clip) = (a.get("show_profile").and_then(|v| v.as_bool()), a.get("clip_border").and_then(|v| v.as_bool()));
    if center.is_none() && radius.is_none() && leader.is_none() && label_at.is_none() && style.is_none() && scale.is_none() && label.is_none() && show.is_none() && clip.is_none() {
        return Err(OpError::param("center", "nothing to change"));
    }
    let v = t.d.sheets[si].view_mut(id).expect("view");
    if let Some(s) = scale {
        v.scale = Some(s);
    }
    if let Some(s) = style {
        v.detail_style = s;
    }
    if let Some(b) = show {
        v.detail_hide_profile = !b;
    }
    if let Some(b) = clip {
        v.detail_clip_border = b;
    }
    let ViewKind::Detail { center: c, radius: r, label: l, .. } = &mut v.kind else { unreachable!("a detail view") };
    if let Some(x) = label {
        *l = x;
    }
    if let Some(p) = center {
        *c = ppl.to_view(p);
    }
    if let Some(rs) = radius {
        *r = rs / ppl.len_to_sheet(1.0).max(1e-12);
    }
    // The label's offset from the circle centre (sheet mm).
    let (cs, rs) = (ppl.to_sheet(*c), ppl.len_to_sheet(*r));
    match (leader, label_at) {
        (Some(false), _) => v.detail_label_at = None,
        (_, Some(p)) => v.detail_label_at = Some([p[0] - cs[0], p[1] - cs[1]]),
        (Some(true), None) if v.detail_label_at.is_none() => {
            let d = rs * std::f64::consts::FRAC_1_SQRT_2;
            v.detail_label_at = Some([d + 12.0, d + 10.0]);
        }
        _ => {}
    }
    done(od, t, "Edit detail view", "view", id.0)
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
    let ann = dt::gdt(id, pt_arg(a, "position")?, frame, extra);
    let start = noshape_scene::drawing_tools::annotation_edit::note_leader_start(&ann);
    t.d.sheets[sheet].annotations.push(ann);
    // Clicking the geometry first: the frame's leader goes there.
    if let Some(v) = a.get("anchor") {
        use noshape_scene::drawing_tools::annotation_edit as ae;
        let p = point_value(v, "anchor")?;
        let tool = Tool::GdtLeader { id };
        let pk = t.m.pick(&tool, &[], p);
        let leader = ae::leader_to(&pk, p);
        // On a model point: associative (a lost point turns the leader red).
        let r = pk.snap_at(p).and_then(|s| s.r);
        ae::add_leader(&mut t.d.sheets[sheet], id, leader, start);
        if let (Some(r), Some(a)) = (r, t.d.sheets[sheet].annotations.iter_mut().find(|a| a.id == id)) {
            a.refs.push(noshape_model::drawing::AnnotationRef { slot: 100, point: r });
        }
    }
    done(od, t, "Geometric tolerance", "annotation", id)
}

fn surface_finish(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let mut t = target(od, a)?;
    let sheet = t.sheet;
    let id = t.d.sheets[sheet].next_annotation_id();
    let value = a.opt_str("value")?.unwrap_or(dt::SURFACE_FINISH_DEFAULT).to_string();
    let spec: noshape_model::drawing::SfSpec = match a.get("spec") {
        None | Some(Value::Null) => Default::default(),
        Some(v) => serde_json::from_value(v.clone()).map_err(|e| OpError::param("spec", &e.to_string()))?,
    };
    let p = pt_arg(a, "position")?;
    let pk = t.m.pick_snapping(&Tool::SurfaceFinish, &[], p, false);
    let on_dim = pk.surface_finish_on_dimension(p);
    let ext = on_dim.and_then(|_| pk.hovered_dim_on(p));
    let on = on_dim.or_else(|| pk.surface_finish_on(p));
    let mut sf = dt::surface_finish(id, p, value, spec);
    if let noshape_model::drawing::AnnotationKind::SurfaceFinish { on: o, ext: e, .. } = &mut sf.kind {
        *o = on;
        *e = ext;
    }
    t.d.sheets[sheet].annotations.push(sf);
    done(od, t, "Surface finish", "annotation", id)
}

fn weld(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let mut t = target(od, a)?;
    let spec: noshape_model::drawing::weld::WeldSpec = match a.get("spec") {
        None | Some(Value::Null) => Default::default(),
        Some(v) => serde_json::from_value(v.clone()).map_err(|e| OpError::param("spec", &e.to_string()))?,
    };
    let c = pt_arg(a, "target")?;
    let (view, target) = t.m.pick(&Tool::Weld, &[], c).weld_target(c);
    let sheet = t.sheet;
    let id = t.d.sheets[sheet].next_annotation_id();
    t.d.sheets[sheet].annotations.push(dt::weld(id, view, target, spec, pt_arg(a, "position")?));
    done(od, t, "Weld symbol", "annotation", id)
}

fn revision_table(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let mut t = target(od, a)?;
    let d = dt::RevisionTableOpts::default();
    let o = dt::RevisionTableOpts {
        see_sheet1: a.bool_or("see_sheet1", d.see_sheet1)?,
        visible_rows: a.opt_u64("visible_rows")?.map(|n| n.clamp(1, 1000) as u32).unwrap_or(d.visible_rows),
        newest_top: a.bool_or("newest_top", d.newest_top)?,
        hide_callouts: a.bool_or("hide_callouts", d.hide_callouts)?,
        corner: enum_arg(a, "corner", &CORNERS, d.corner)?,
    };
    let sheet = t.sheet;
    let id = t.d.sheets[sheet].next_table_id();
    t.d.sheets[sheet].tables.push(dt::revision_table(id, &o, pt_arg(a, "position")?));
    done(od, t, "Revision table", "table", id)
}

fn revision_callout(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let mut t = target(od, a)?;
    let revision = match a.opt_str("revision")?.map(str::trim).filter(|r| !r.is_empty()) {
        Some(r) => r.to_string(),
        None => dt::latest_revision(&t.m.info),
    };
    let sheet = t.sheet;
    let id = t.d.sheets[sheet].next_annotation_id();
    t.d.sheets[sheet].annotations.push(dt::revision_callout(id, pt_arg(a, "position")?, revision));
    done(od, t, "Revision callout", "annotation", id)
}

fn jogged_section_view(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let t = target(od, a)?;
    let parent = view_arg(&t, a, "parent")?;
    let mut pts: Vec<[f64; 2]> = Vec::new();
    for (i, p) in a.array("points")?.iter().enumerate() {
        let param = format!("points[{i}]");
        let c = point_value(p, &param)?;
        let tool = Tool::JoggedSection { parent: Some(parent), points: pts.clone(), done: false };
        let pp = t.m.pick(&tool, &[], c).view_point(c, Some(parent)).ok_or_else(|| OpError::param(&param, "not on the parent view"))?;
        match pts.last() {
            None => pts.push(pp.p),
            Some(last) => {
                let q = dt::ortho_view(*last, pp.p);
                if dt::geom::len(dt::geom::sub(q, *last)) > 1e-7 {
                    pts.push(q);
                }
            }
        }
    }
    if pts.len() < 2 {
        return Err(OpError::param("points", "the cutting line needs two different points"));
    }
    // Finishing the line (Enter) extends its ends past the view.
    let pts = t.m.pick(&Tool::Select, &[], [0.0; 2]).extend_line_ends(parent, &pts);
    let tool = Tool::JoggedSection { parent: Some(parent), points: pts, done: true };
    let v = t.m.pick(&tool, &[], pt_arg(a, "position")?).pending_view(&ViewOptions::default()).ok_or_else(|| OpError::param("position", "the section view can't be placed there"))?;
    let id = v.id;
    let mut t = t;
    t.d.sheets[t.sheet].views.push(v);
    done(od, t, "Section view", "view", id.0)
}

fn aligned_section_view(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let t = target(od, a)?;
    let parent = view_arg(&t, a, "parent")?;
    let click = |name: &str, tool: &Tool| -> Result<[f64; 2], OpError> {
        let c = pt_arg(a, name)?;
        t.m.pick(tool, &[], c).view_point(c, Some(parent)).map(|p| p.p).ok_or_else(|| OpError::param(name, "not on the parent view"))
    };
    let tool0 = Tool::AlignedSection { parent: Some(parent), center: None, a: None, b: None };
    let center = click("center", &tool0)?;
    let leg = |name: &str| -> Result<[f64; 2], OpError> {
        let p = click(name, &Tool::AlignedSection { parent: Some(parent), center: Some(center), a: None, b: None })?;
        if dt::geom::len(dt::geom::sub(p, center)) <= 1e-7 {
            return Err(OpError::param(name, "must differ from the center"));
        }
        Ok(t.m.pick(&Tool::Select, &[], [0.0; 2]).extend_line_ends(parent, &[p, center])[0])
    };
    let (la, lb) = (leg("a")?, leg("b")?);
    let tool = Tool::AlignedSection { parent: Some(parent), center: Some(center), a: Some(la), b: Some(lb) };
    let v = t.m.pick(&tool, &[], pt_arg(a, "position")?).pending_view(&ViewOptions::default()).ok_or_else(|| OpError::param("position", "the section view can't be placed there"))?;
    let id = v.id;
    let mut t = t;
    t.d.sheets[t.sheet].views.push(v);
    done(od, t, "Aligned section view", "view", id.0)
}

pub(super) const CORNERS: [(&str, TableCorner); 4] = [("top_left", TableCorner::TopLeft), ("top_right", TableCorner::TopRight), ("bottom_left", TableCorner::BottomLeft), ("bottom_right", TableCorner::BottomRight)];

fn general_table(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let mut t = target(od, a)?;
    let n = |k: &str| -> Result<String, OpError> { Ok(a.opt_u64(k)?.map(|n| n.to_string()).unwrap_or_default()) };
    let (rows, cols) = dt::table_size(&n("rows")?, &n("columns")?);
    let corner = enum_arg(a, "corner", &CORNERS, TableCorner::TopRight)?;
    let mut table = dt::general_table(rows, cols, a.bool_or("title_row", false)?, a.bool_or("header_row", false)?, corner, pt_arg(a, "position")?);
    let sheet = t.sheet;
    let id = t.d.sheets[sheet].next_table_id();
    table.id = id;
    t.d.sheets[sheet].tables.push(table);
    done(od, t, "Table", "table", id)
}

fn table_cell(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let mut t = target(od, a)?;
    let id = a.opt_u64("table")?.ok_or_else(|| OpError::param("table", "a table id"))?;
    let sheet = t.sheet;
    let table = t.d.sheets[sheet].tables.iter_mut().find(|x| x.id == id).ok_or_else(|| OpError::param("table", "no such table on the sheet"))?;
    if !matches!(table.kind, noshape_model::drawing::TableKind::General { .. }) {
        return Err(OpError::param("table", "not a general table (BOM, hole, cut list and revision tables fill themselves)"));
    }
    let (row, col) = (a.opt_u64("row")?.unwrap_or(0) as usize, a.opt_u64("column")?.unwrap_or(0) as usize);
    dt::set_general_cell(table, row, col, a.str("text")?);
    done(od, t, "Edit cell", "table", id)
}

fn hatch_region(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    use noshape_model::drawing::hatch_pat;
    let od = ctx.doc(a.opt_str("doc")?)?;
    let mut t = target(od, a)?;
    let edit = a.opt_u64("hatch")?;
    let existing = match edit {
        Some(id) => Some(t.d.sheets[t.sheet].sketch.hatches.iter().find(|h| h.id == id).cloned().ok_or_else(|| OpError::param("hatch", "no such hatch region on the sheet"))?),
        None => None,
    };
    let text = |name: &str| -> Result<Option<String>, OpError> {
        Ok(match a.get(name) {
            None => None,
            Some(Value::Number(n)) => Some(n.to_string()),
            Some(Value::String(s)) => Some(s.clone()),
            Some(_) => return Err(OpError::param(name, "\"Auto\" or a number")),
        })
    };
    let standard = match a.opt_str("standard")? {
        Some(_) => enum_arg(a, "standard", &[("ansi", 0), ("iso", 1), ("general", 2)], 0)?,
        None => existing.as_ref().map_or(0, |h| h.standard),
    };
    let pattern = match a.opt_str("pattern")? {
        Some(name) => {
            let (_, list) = hatch_pat::GROUPS[standard];
            let lower = name.trim().to_lowercase();
            list.iter().position(|p| p.name.to_lowercase() == lower).or_else(|| list.iter().position(|p| p.name.split_whitespace().next().is_some_and(|w| w.to_lowercase() == lower))).ok_or_else(|| OpError::param("pattern", &format!("one of {}", list.iter().map(|p| p.name).collect::<Vec<_>>().join(", "))))?
        }
        None => existing.as_ref().filter(|h| h.standard == standard).map_or(hatch_pat::default_pattern(standard), |h| h.pattern),
    };
    // Unset fields keep the edited hatch's values (as the dialog opens them).
    let (scale0, angle0) = match &existing {
        Some(h) => (format!("{}", h.scale), if h.angle.abs() < 1e-9 { "Auto".to_string() } else { format!("{}", (h.angle + hatch_pat::base_angle(h.standard, h.pattern)).rem_euclid(360.0)) }),
        None => ("Auto".into(), "Auto".into()),
    };
    let color = match a.opt_array("color")? {
        Some(c) => Some([0, 1, 2].map(|i| c.get(i).and_then(Value::as_u64).unwrap_or(0).min(255) as u8)),
        None => existing.as_ref().and_then(|h| h.color),
    };
    let mut profiles = existing.as_ref().map(|h| vec![((h.boundary.clone(), h.holes.clone()), h.view)]).unwrap_or_default();
    if let Some(ps) = a.opt_array("points")? {
        for (i, p) in ps.iter().enumerate() {
            let name = format!("points[{i}]");
            let c = point_value(p, &name)?;
            let picked = t.m.pick(&Tool::Select, &[], c).hatch_profile_at(c).ok_or_else(|| OpError::param(&name, "not inside a closed profile of the sheet sketch or a view (detail and break views take no hatches)"))?;
            dt::hatch_toggle(&mut profiles, picked, edit.is_some());
        }
    }
    if profiles.is_empty() {
        return Err(OpError::param("points", "pick at least one closed profile"));
    }
    let opts = dt::HatchOpts { standard, pattern, scale: text("scale")?.unwrap_or(scale0), angle: text("angle")?.unwrap_or(angle0), drawing_color: color.is_none(), color, profiles };
    let sk = &mut t.d.sheets[t.sheet].sketch;
    let regions = opts.regions(edit.unwrap_or_else(|| sk.next_hatch_id()));
    let ids: Vec<u64> = regions.iter().map(|r| r.id).collect();
    dt::apply_hatches(sk, edit, regions);
    commit(od, t.tab, t.sheet, t.d, if edit.is_some() { "Edit hatch" } else { "Hatch region" })?;
    let mut out = reply(od, t.tab, t.sheet, false)?;
    out["hatches"] = json!(ids);
    Ok(out.into())
}

fn custom_table(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    use noshape_model::document::TabContent;
    use noshape_model::drawing::TableKind;
    let od = ctx.doc(a.opt_str("doc")?)?;
    let mut t = target(od, a)?;
    let doc = od.doc();
    let edit = a.opt_u64("id")?;
    let old = match edit {
        Some(id) => match t.d.sheets[t.sheet].table(id).map(|x| (x.kind.clone(), x.corner)) {
            Some((k @ TableKind::Custom { .. }, c)) => Some((k, c)),
            _ => return Err(OpError::param("id", "no custom table with that id on the sheet")),
        },
        None => None,
    };
    let (old_studio, old_fs, old_name, old_fs_version, old_btt) = match &old {
        Some((TableKind::Custom { studio, fs, name, fs_version, bottom_to_top, .. }, _)) => (Some(*studio), Some(*fs), Some(name.clone()), fs_version.clone(), *bottom_to_top),
        _ => (None, None, None, None, false),
    };
    // The Part Studio: given, the view's, the edited table's, else the first in the drawing.
    let studio = match (a.opt_str("studio")?, a.opt_u64("view")?) {
        (Some(s), _) => {
            let id = crate::doc::find_tab(doc, s).map_err(|e| OpError::param("studio", &e.message))?;
            if !matches!(doc.tab(id).map(|t| &t.content), Some(TabContent::PartStudio(_))) {
                return Err(OpError::param("studio", "not a Part Studio"));
            }
            id
        }
        (None, Some(_)) => {
            let v = view_arg(&t, a, "view")?;
            views_source_tab(&t, v).ok_or_else(|| OpError::param("view", "the view doesn't show a Part Studio"))?
        }
        (None, None) => match old_studio {
            Some(s) => s,
            None => {
                let keys = noshape_model::drawing::views::referenced_sources(&t.d);
                doc.ordered_tabs().into_iter().find(|tab| matches!(tab.content, TabContent::PartStudio(_)) && keys.iter().any(|k| k.tab == tab.id)).map(|tab| tab.id).ok_or_else(|| OpError::param("studio", "the drawing shows no Part Studio: give one"))?
            }
        },
    };
    // The custom table: this document's Feature Studios.
    let (fs, name, fs_version) = match a.opt_str("table")? {
        Some(want) => {
            let only = match a.opt_str("feature_studio")? {
                Some(f) => Some(crate::doc::find_tab(doc, f).map_err(|e| OpError::param("feature_studio", &e.message))?),
                None => None,
            };
            let scripts = noshape_model::custom::DocumentScripts::from_document(doc);
            let all: Vec<(noshape_model::document::TabId, String, String)> = scripts.studios().flat_map(|(tab, _, c)| c.tables.iter().map(move |d| (tab, d.name.clone(), d.display_name.clone()))).filter(|(tab, ..)| only.is_none_or(|o| o == *tab)).collect();
            let hit = all.iter().find(|x| x.1 == want).or_else(|| all.iter().find(|x| x.2 == want)).ok_or_else(|| OpError::param("table", &format!("no custom table \"{want}\" in this document's Feature Studios{}", if all.is_empty() { String::new() } else { format!(" (one of {})", all.iter().map(|x| x.2.as_str()).collect::<Vec<_>>().join(", ")) })))?;
            (hit.0, hit.1.clone(), None)
        }
        None => match (old_fs, old_name) {
            (Some(f), Some(n)) => (f, n, old_fs_version),
            _ => return Err(OpError::param("table", "required")),
        },
    };
    let bottom_to_top = a.bool_or("bottom_to_top", old_btt)?;
    let corner = enum_arg(a, "corner", &CORNERS, old.as_ref().map_or(TableCorner::TopRight, |o| o.1))?;
    let kind = dt::custom_table_kind(studio, fs, name, fs_version, bottom_to_top, old.as_ref().map(|o| &o.0));
    let sheet = t.sheet;
    let (id, description) = match edit {
        Some(id) => {
            if let Some(x) = t.d.sheets[sheet].tables.iter_mut().find(|x| x.id == id) {
                x.kind = kind;
                x.corner = corner;
            }
            (id, "Custom table properties")
        }
        None => {
            let id = t.d.sheets[sheet].next_table_id();
            let table = dt::custom_table(id, kind, corner, pt_arg(a, "position")?);
            t.d.sheets[sheet].tables.push(table);
            (id, "Custom table")
        }
    };
    done(od, t, description, "table", id)
}

/// The Part Studio a view shows.
fn views_source_tab(t: &Target, v: ViewId) -> Option<noshape_model::document::TabId> {
    noshape_model::drawing::views::view_source(&t.d.sheets[t.sheet], v).map(|(k, _)| k.tab)
}

/// The file an insert tool takes: a file tab of `kind`, or a local file
/// imported as a file tab first (the picker's Import).
fn insert_file(od: &mut OpenDoc, a: &Args, kind: noshape_model::file_tab::FileKind, what: &str) -> Result<noshape_model::file_tab::FileTab, OpError> {
    use noshape_model::document::TabContent;
    use noshape_model::file_tab::FileTab;
    let file = match (a.opt_str("file")?, a.opt_str("path")?) {
        (Some(f), _) => {
            let id = crate::doc::find_tab(od.doc(), f).map_err(|e| OpError::param("file", &e.message))?;
            match od.doc().tab(id).map(|t| &t.content) {
                Some(TabContent::File(x)) => x.clone(),
                _ => return Err(OpError::param("file", "not a file tab")),
            }
        }
        (None, Some(p)) => {
            let path = std::path::Path::new(p);
            let bytes = std::fs::read(path).map_err(|e| OpError::param("path", &format!("can't read {p}: {e}")))?;
            let name = path.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
            let file = FileTab::new(&name, &bytes, noshape_model::file_tab::now());
            if file.kind() != kind {
                return Err(OpError::param("path", &format!("not {what}")));
            }
            let f2 = file.clone();
            od.session.edit_document(&format!("Import {name}"), |d| {
                crate::import::add_file_tab(d, f2, None, None);
            })?;
            file
        }
        (None, None) => return Err(OpError::param("file", "a file tab, or a local file `path`")),
    };
    if file.kind() != kind {
        return Err(OpError::param("file", &format!("not {what}")));
    }
    Ok(file)
}

fn insert_image(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let file = insert_file(od, a, noshape_model::file_tab::FileKind::Image, "an image (PNG, JPEG, GIF)")?;
    let aspect = file.bytes().ok_or_else(|| OpError::param("file", "the file data is damaged")).and_then(|b| dt::image_aspect(&b).map_err(|e| OpError::param("file", &format!("Insert image failed: {e}"))))?;
    let mut t = target(od, a)?;
    let start = pt_arg(a, "position")?;
    let end = match a.get("to") {
        Some(v) => point_value(v, "to")?,
        None => start,
    };
    let (pos, size) = t.m.pick(&Tool::Select, &[], end).image_box(start, aspect);
    let id = dt::add_sheet_image(&mut t.d.sheets[t.sheet], file.file_name.clone(), file.data_base64.clone(), pos, size);
    done(od, t, "Insert image", "image", id)
}

fn insert_cad(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let file = insert_file(od, a, noshape_model::file_tab::FileKind::Drawing2d, "a DXF or DWG file")?;
    let bytes = file.bytes().ok_or_else(|| OpError::param("file", "the file data is damaged"))?;
    let mut t = target(od, a)?;
    let units_mm = t.d.units_mm;
    let sheet = &mut t.d.sheets[t.sheet];
    let (n, notes) = dt::insert_cad(sheet, &bytes, units_mm).map_err(|e| OpError::param("file", &format!("Insert DXF or DWG failed: {e}")))?;
    let _ = sheet.sketch.sketch.solve();
    commit(od, t.tab, t.sheet, t.d, "Insert DXF or DWG")?;
    let mut out = reply(od, t.tab, t.sheet, false)?;
    out["entities"] = json!(n);
    out["notes"] = json!(notes);
    Ok(out.into())
}

fn image(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let tab = drawing_tab(od, a)?;
    let mut d = dr::drawing_of(od.doc(), tab).cloned().expect("drawing");
    let sheet = sheet_index(&d, a)?;
    let id = a.opt_u64("image")?.ok_or_else(|| OpError::param("image", "required"))?;
    if !d.sheets[sheet].images.iter().any(|x| x.id == id) {
        return Err(OpError::param("image", "no such image on the sheet (drawing_get `images`)"));
    }
    match (a.get("move_to"), a.get("borders").and_then(Value::as_bool)) {
        (Some(m), None) => {
            let to = match m {
                Value::Number(n) => n.as_u64().filter(|n| (1..=d.sheets.len() as u64).contains(n)).map(|n| n as usize - 1),
                Value::String(s) => d.sheets.iter().position(|x| x.name == *s),
                _ => None,
            }
            .ok_or_else(|| OpError::param("move_to", &format!("a sheet number or name: {}", d.sheets.iter().map(|x| x.name.clone()).collect::<Vec<_>>().join(", "))))?;
            if to == sheet {
                return Err(OpError::param("move_to", "the image is on that sheet"));
            }
            let new = dr::move_image_to_sheet(&mut d, sheet, id, to).expect("checked");
            // The drawing stays on its sheet, as the menu leaves it.
            commit(od, tab, sheet, d, dr::MOVE_IMAGE_TO_SHEET)?;
            let mut out = reply(od, tab, to, false)?;
            out["image"] = json!(new);
            Ok(out.into())
        }
        (None, Some(on)) => {
            d.props.image_borders = on;
            commit(od, tab, sheet, d, dr::IMAGE_BORDERS)?;
            Ok(reply(od, tab, sheet, false)?.into())
        }
        _ => Err(OpError::invalid("give one of `move_to` or `borders`")),
    }
}

fn drawing_sketch(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    use dt::SkTool;
    use noshape_sketch::{DVec2, EntityId};
    let od = ctx.doc(a.opt_str("doc")?)?;
    let mut t = target(od, a)?;
    let tool = enum_arg(
        a,
        "tool",
        &[
            ("line", SkTool::Line),
            ("circle", SkTool::Circle),
            ("circle_3_point", SkTool::Circle3),
            ("tangent_arc", SkTool::TangentArc),
            ("corner_rectangle", SkTool::CornerRect),
            ("center_rectangle", SkTool::CenterRect),
            ("spline", SkTool::Spline),
            ("spline_point", SkTool::SplinePoint),
            ("point", SkTool::Point),
            ("trim", SkTool::Trim),
        ],
        SkTool::Line,
    )?;
    let cs: Vec<[f64; 2]> = a.array("picks")?.iter().enumerate().map(|(i, p)| point_value(p, &format!("picks[{i}]"))).collect::<Result<_, _>>()?;
    let need = match tool {
        SkTool::Circle | SkTool::CornerRect | SkTool::CenterRect | SkTool::TangentArc => 2,
        SkTool::Circle3 => 3,
        _ => 1,
    };
    if !cs.len().is_multiple_of(need) || (matches!(tool, SkTool::Line | SkTool::Spline) && cs.len() < 2) {
        return Err(OpError::param("picks", &format!("{} takes {}", tool.name(), if need == 1 { "two or more clicks".to_string() } else { format!("clicks in groups of {need}") })));
    }
    let v2 = |p: [f64; 2]| DVec2::new(p[0], p[1]);
    let sheet = t.sheet;
    // The sketch as edited so far: later clicks snap to points made by earlier ones.
    let mut work = t.d.clone();
    let before = work.sheets[sheet].sketch.sketch.entities().count();
    let (mut pts, mut from): (Vec<[f64; 2]>, Option<EntityId>) = (Vec::new(), None);
    for (i, c) in cs.iter().enumerate() {
        let state = Tool::Sketch { tool, pts: pts.clone(), from: from.map(|e| e.0) };
        let mut pk = t.m.pick(&state, &[], *c);
        pk.drawing = &work;
        let last = pts.last().copied();
        let (p, existing, infer) = pk.sketch_target(last, *c);
        let hovered = pk.sketch_entity_at();
        let point_at = pk.sketch_point_at(*c).map(|x| x.0);
        let sk = &mut work.sheets[sheet].sketch.sketch;
        let bad = |m: &str| OpError::param(&format!("picks[{i}]"), m);
        match tool {
            SkTool::Line => match last {
                Some(a0) if ((p[0] - a0[0]).powi(2) + (p[1] - a0[1]).powi(2)).sqrt() >= 1e-6 => {
                    let end = dt::sketch_line(sk, from, a0, existing, p, infer);
                    (pts, from) = (vec![p], Some(end));
                }
                Some(_) => {}
                None => (pts, from) = (vec![p], existing),
            },
            SkTool::Circle | SkTool::CornerRect | SkTool::CenterRect => match last {
                Some(a0) => {
                    dt::sketch_two_click(sk, tool, from, a0, p);
                    (pts, from) = (vec![], None);
                }
                None => (pts, from) = (vec![p], existing),
            },
            SkTool::Circle3 => {
                pts.push(p);
                if pts.len() == 3 {
                    sk.three_point_circle(v2(pts[0]), v2(pts[1]), v2(pts[2])).map_err(|e| bad(&e.to_string()))?;
                    pts.clear();
                }
            }
            SkTool::TangentArc => match (last, from) {
                (Some(_), Some(start)) => {
                    sk.tangent_arc(start, v2(p)).map_err(|e| bad(&e.to_string()))?;
                    (pts, from) = (vec![], None);
                }
                _ => match existing {
                    Some(id) if !sk.users_of_point(id).is_empty() => (pts, from) = (vec![p], Some(id)),
                    _ => return Err(bad("Click the end of a line or arc")),
                },
            },
            SkTool::Spline => pts.push(p),
            SkTool::SplinePoint => {
                let id = hovered.ok_or_else(|| bad("not on a spline of the sheet sketch"))?;
                sk.insert_spline_point(id, v2(p)).map_err(|e| bad(&e.to_string()))?;
            }
            SkTool::Point => {
                sk.add_point(v2(p));
            }
            SkTool::Trim => {
                let id = hovered.or(point_at).ok_or_else(|| bad("not on sheet sketch geometry"))?;
                dt::sketch_trim(sk, id, *c);
            }
        }
    }
    let sk = &mut work.sheets[sheet].sketch.sketch;
    if tool == SkTool::Spline && !dt::sketch_spline(sk, &pts) {
        return Err(OpError::param("picks", "a spline takes two or more distinct points"));
    }
    let _ = sk.solve();
    let created = sk.entities().count() as i64 - before as i64;
    t.d = work;
    commit(od, t.tab, sheet, t.d, tool.name())?;
    let mut out = reply(od, t.tab, sheet, false)?;
    out["created"] = json!(created);
    Ok(out.into())
}

fn format_painter(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    use noshape_model::drawing::painter::{PaintOptions, paint};
    let od = ctx.doc(a.opt_str("doc")?)?;
    let mut t = target(od, a)?;
    let d0 = PaintOptions::default();
    let options = PaintOptions { style: a.bool_or("style", d0.style)?, tolerance_precision: a.bool_or("tolerance_precision", d0.tolerance_precision)?, content: a.bool_or("content", d0.content)? };
    let src = a.opt_u64("source")?.unwrap_or(0);
    let sheet = &mut t.d.sheets[t.sheet];
    let source = sheet.annotations.iter().find(|x| x.id == src).cloned().ok_or_else(|| OpError::param("source", "no such annotation on the sheet"))?;
    for (i, v) in a.array("targets")?.iter().enumerate() {
        let id = v.as_u64().ok_or_else(|| OpError::param(&format!("targets[{i}]"), "an annotation id"))?;
        if id == src {
            continue;
        }
        let dst = sheet.annotations.iter_mut().find(|x| x.id == id).ok_or_else(|| OpError::param(&format!("targets[{i}]"), "no such annotation on the sheet"))?;
        paint(&source, dst, options);
    }
    done(od, t, "Format painter", "source", src)
}

fn inspection(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    use noshape_model::drawing::InspectionShape;
    use noshape_model::drawing::inspection::Scope;
    let od = ctx.doc(a.opt_str("doc")?)?;
    let mut t = target(od, a)?;
    let si = t.sheet;
    let ann = |t: &Target, name: &str, id: Option<u64>| -> Result<u64, OpError> {
        let id = id.ok_or_else(|| OpError::param(name, "an annotation id"))?;
        t.d.sheets[si].annotations.iter().any(|x| x.id == id).then_some(id).ok_or_else(|| OpError::param(name, "no such annotation on the sheet"))
    };
    let list = |t: &Target| -> Result<Vec<u64>, OpError> {
        let v = a.opt_array("annotations")?.ok_or_else(|| OpError::param("annotations", "required"))?;
        v.iter().enumerate().map(|(i, x)| ann(t, &format!("annotations[{i}]"), x.as_u64())).collect()
    };
    let description = match a.str("action")? {
        "add" => {
            let first = a.opt_u64("number")?.map(|n| n as u32);
            for (k, id) in list(&t)?.into_iter().enumerate() {
                // Reorder: the Number field counts up after each item.
                let n = first.map(|f| f + k as u32).unwrap_or_else(|| t.d.next_inspection_number());
                dt::set_inspection(&mut t.d.sheets[si], id, Some(n));
            }
            "Inspection item"
        }
        "automatic" => {
            let scope = match a.opt_str("scope")?.unwrap_or("drawing") {
                "sheet" => Scope::Sheet(si),
                "view" => Scope::View(si, view_arg(&t, a, "view")?),
                "drawing" => Scope::Drawing,
                _ => return Err(OpError::param("scope", "drawing, sheet or view")),
            };
            t.d.auto_inspection(scope);
            "Inspection items"
        }
        "edit" => {
            let id = ann(&t, "id", a.opt_u64("id")?)?;
            let n = a.opt_u64("number")?.ok_or_else(|| OpError::param("number", "required"))? as u32;
            if t.d.sheets[si].annotations.iter().find(|x| x.id == id).is_none_or(|x| x.inspection.is_none()) {
                return Err(OpError::param("id", "the annotation has no inspection item"));
            }
            dt::set_inspection(&mut t.d.sheets[si], id, Some(n));
            "Edit inspection item"
        }
        "move" => {
            let (from, to) = (ann(&t, "id", a.opt_u64("id")?)?, ann(&t, "to", a.opt_u64("to")?)?);
            if t.d.sheets[si].annotations.iter().find(|x| x.id == from).is_none_or(|x| x.inspection.is_none()) {
                return Err(OpError::param("id", "the annotation has no inspection item"));
            }
            if from != to {
                dt::move_inspection(&mut t.d.sheets[si], from, to);
            }
            "Move inspection item"
        }
        "remove" => {
            for id in list(&t)? {
                dt::set_inspection(&mut t.d.sheets[si], id, None);
            }
            "Remove inspection item"
        }
        "remove_all" => {
            t.d.remove_all_inspection();
            "Remove all inspection items"
        }
        "hide" => {
            t.d.props.hide_inspection = true;
            "Hide inspection items"
        }
        "show" => {
            t.d.props.hide_inspection = false;
            "Show inspection items"
        }
        "shape" => {
            let shapes = [("circle", InspectionShape::Circle), ("oval", InspectionShape::Oval), ("rectangle", InspectionShape::Rectangle), ("triangle", InspectionShape::Triangle), ("flag", InspectionShape::Flag)];
            if a.get("shape").is_none() {
                return Err(OpError::param("shape", "required"));
            }
            t.d.props.inspection_shape = enum_arg(a, "shape", &shapes, InspectionShape::Circle)?;
            "Inspection border shape"
        }
        _ => return Err(OpError::param("action", "add, automatic, edit, move, remove, remove_all, hide, show or shape")),
    };
    commit(od, t.tab, si, t.d, description)?;
    Ok(reply(od, t.tab, si, false)?.into())
}

fn properties(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let mut t = target(od, a)?;
    let props = |d: &Drawing| serde_json::to_value(&d.props).expect("json");
    let Some(set) = a.object("set")? else {
        return Ok(json!({ "properties": props(&t.d) }).into());
    };
    let mut j = props(&t.d);
    for (k, v) in set {
        j[k] = v.clone();
    }
    let new: noshape_model::drawing::DrawingProps = serde_json::from_value(j).map_err(|e| OpError::param("set", &format!("{e}")))?;
    // Unset / off fields are left out of the JSON: any other value that
    // didn't land names no property.
    let back = serde_json::to_value(&new).expect("json");
    if let Some((k, _)) = set.iter().find(|(k, v)| back.get(k.as_str()).is_none() && !matches!(v, Value::Null | Value::Bool(false))) {
        return Err(OpError::param("set", &format!("no property `{k}`")));
    }
    dt::set_drawing_props(&mut t.d, |p| *p = new);
    let sheet = t.sheet;
    commit(od, t.tab, sheet, t.d, "Drawing properties")?;
    let d = dr::drawing_of(od.doc(), t.tab).cloned().expect("drawing");
    let mut out = reply(od, t.tab, sheet, false)?;
    out["properties"] = props(&d);
    Ok(out.into())
}

fn edit_dimension(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    use noshape_model::drawing::{DimUnits, Tolerance};
    let od = ctx.doc(a.opt_str("doc")?)?;
    let mut t = target(od, a)?;
    let id = a.opt_u64("annotation")?.unwrap_or(0);
    let drawing = t.d.clone();
    let ann = t.d.sheets[t.sheet].annotations.iter_mut().find(|x| x.id == id).ok_or_else(|| OpError::param("annotation", "no such annotation on the sheet"))?;
    let hole = matches!(ann.kind, AnnotationKind::HoleCallout { .. });
    if !hole && !ann.kind.is_dimension() {
        return Err(OpError::param("annotation", "not a dimension or hole callout"));
    }
    let f = &mut ann.format;
    for (k, slot) in [("above", &mut f.above), ("below", &mut f.below), ("prefix", &mut f.prefix), ("suffix", &mut f.suffix)] {
        if let Some(v) = a.opt_str(k)? {
            *slot = v.to_string();
        }
    }
    for (k, slot) in [("precision", &mut f.precision), ("dual_precision", &mut f.dual_fmt.precision)] {
        match a.get(k) {
            None => {}
            Some(Value::String(s)) if s == "drawing" => *slot = None,
            Some(v) => *slot = Some(v.as_u64().filter(|n| *n <= 8).ok_or_else(|| OpError::param(k, "0–8 or \"drawing\""))? as u8),
        }
    }
    for (k, slot) in [("units", &mut f.units), ("dual_units", &mut f.dual_fmt.units)] {
        if let Some(u) = a.opt_str(k)? {
            *slot = match u {
                "drawing" => None,
                _ => Some(DimUnits::CALLOUT.iter().copied().find(|x| x.name().eq_ignore_ascii_case(u)).ok_or_else(|| OpError::param(k, &format!("one of {} or \"drawing\"", DimUnits::CALLOUT.iter().map(|x| x.name()).collect::<Vec<_>>().join(", "))))?),
            };
        }
    }
    match a.get("dual_show_units") {
        None => {}
        Some(Value::Bool(b)) => f.dual_fmt.show_unit = Some(*b),
        Some(Value::String(s)) if s == "drawing" => f.dual_fmt.show_unit = None,
        Some(_) => return Err(OpError::param("dual_show_units", "true, false or \"drawing\"")),
    }
    for (k, slot) in [("parentheses", &mut f.reference), ("inspection", &mut f.inspection_frame)] {
        if let Some(b) = a.get(k).and_then(Value::as_bool) {
            *slot = b;
        }
    }
    if let Some(d) = a.opt_str("dual")? {
        f.dual = match d {
            "single" => Some(false),
            "dual" => Some(true),
            _ => None,
        };
    }
    // Tolerance values: numbers in the dimension's units, or with units.
    let u = noshape_model::drawing::format::length_units(&drawing, f);
    let len = |k: &str| -> Result<Option<f64>, OpError> {
        Ok(match a.get(k) {
            None => None,
            Some(Value::Number(n)) => n.as_f64().map(|v| v * u.meters()),
            Some(Value::String(s)) => match s.trim().parse::<f64>() {
                Ok(v) => Some(v * u.meters()),
                Err(_) => Some(noshape_units::evaluate_as(s, noshape_units::Dim::LENGTH, &Default::default(), &noshape_units::EmptyScope).map_err(|e| OpError::param(k, &e.to_string()))?.value),
            },
            Some(_) => return Err(OpError::param(k, "a number or a length")),
        })
    };
    let (tv, up, lo) = (len("tolerance_value")?, len("upper")?, len("lower")?);
    let current = dr::tolerance_name(&noshape_model::drawing::Annotation { format: f.clone(), ..noshape_model::drawing::Annotation::default() });
    let name = match a.opt_str("tolerance")? {
        Some(n) => {
            let list: &[&str] = if hole { &dr::HOLE_TOLERANCES } else { &dr::DIM_TOLERANCES };
            list.iter().copied().find(|x| x.eq_ignore_ascii_case(n)).ok_or_else(|| OpError::param("tolerance", &format!("one of {}", list.join(", "))))?
        }
        None => current,
    };
    if a.get("tolerance").is_some() || tv.is_some() || up.is_some() || lo.is_some() {
        let (ca, cb) = match &f.tolerance {
            Tolerance::Symmetric { value } => (*value, 0.0),
            Tolerance::Deviation { upper, lower } | Tolerance::Limits { upper, lower } => (*upper, *lower),
            _ => (0.0, 0.0),
        };
        let (va, vb) = if name == "Symmetrical" { (tv.unwrap_or(ca), 0.0) } else { (up.unwrap_or(ca), lo.unwrap_or(cb)) };
        dr::set_tolerance(f, name, va, vb);
    }
    if let Tolerance::Fit { hole: h, shaft } = &mut f.tolerance {
        if let Some(c) = a.opt_str("hole_class")? {
            *h = c.to_string();
        }
        if let Some(c) = a.opt_str("shaft_class")? {
            *shaft = c.to_string();
        }
    } else if a.get("hole_class").is_some() || a.get("shaft_class").is_some() {
        return Err(OpError::param("hole_class", "fit classes need a Fit tolerance"));
    }
    // A typed value overrides any dimension's (D39).
    if let Some(v) = a.opt_str("value")? {
        let typed = Some(v.to_string()).filter(|s| !s.trim().is_empty());
        match &mut ann.kind {
            AnnotationKind::LinearDimension { override_text, .. } => *override_text = typed,
            AnnotationKind::HoleCallout { .. } => return Err(OpError::param("value", "hole callouts show their hole feature (dimensions take a typed value)")),
            _ => ann.format.value = typed,
        }
    }
    if let Some(d) = a.get("diameter").and_then(Value::as_bool) {
        let AnnotationKind::RadialDimension { diameter, .. } = &mut ann.kind else { return Err(OpError::param("diameter", "only radial dimensions")) };
        *diameter = d;
    }
    if let Some(o) = a.get("override_tolerance").and_then(Value::as_bool) {
        if !hole {
            return Err(OpError::param("override_tolerance", "only hole callouts"));
        }
        ann.hole.get_or_insert_with(Default::default).override_tol = o;
        if ann.hole.as_deref().is_some_and(|h| *h == Default::default()) {
            ann.hole = None;
        }
    }
    done(od, t, if hole { "Edit hole callout" } else { "Edit dimension" }, "annotation", id)
}

fn delete(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    use dt::Sel;
    let od = ctx.doc(a.opt_str("doc")?)?;
    let mut t = target(od, a)?;
    let sheet = &t.d.sheets[t.sheet];
    let mut sel = Vec::new();
    let ids = |k: &str| -> Result<Vec<u64>, OpError> {
        a.opt_array(k)?.cloned().unwrap_or_default().iter().enumerate().map(|(i, v)| v.as_u64().ok_or_else(|| OpError::param(&format!("{k}[{i}]"), "an id"))).collect()
    };
    for (i, v) in ids("views")?.into_iter().enumerate() {
        sheet.view(ViewId(v)).ok_or_else(|| OpError::param(&format!("views[{i}]"), "no such view on the sheet"))?;
        sel.push(Sel::View(ViewId(v)));
    }
    for (i, v) in ids("annotations")?.into_iter().enumerate() {
        if !sheet.annotations.iter().any(|x| x.id == v) {
            return Err(OpError::param(&format!("annotations[{i}]"), "no such annotation on the sheet"));
        }
        sel.push(Sel::Annotation(v));
    }
    for (i, v) in ids("tables")?.into_iter().enumerate() {
        if !sheet.tables.iter().any(|x| x.id == v) {
            return Err(OpError::param(&format!("tables[{i}]"), "no such table on the sheet"));
        }
        sel.push(Sel::Table(v));
    }
    for (i, v) in ids("images")?.into_iter().enumerate() {
        if !sheet.images.iter().any(|x| x.id == v) {
            return Err(OpError::param(&format!("images[{i}]"), "no such image on the sheet"));
        }
        sel.push(Sel::Image(v));
    }
    for (i, v) in ids("hatches")?.into_iter().enumerate() {
        if !sheet.sketch.hatches.iter().any(|x| x.id == v) {
            return Err(OpError::param(&format!("hatches[{i}]"), "no such hatch region on the sheet"));
        }
        sel.push(Sel::Hatch(v));
    }
    for (i, v) in ids("sketch_entities")?.into_iter().enumerate() {
        if sheet.sketch.sketch.entity(noshape_sketch::EntityId(v as u32)).is_none() {
            return Err(OpError::param(&format!("sketch_entities[{i}]"), "no such sketch entity on the sheet"));
        }
        sel.push(Sel::Sketch(v as u32));
    }
    for (i, b) in a.opt_array("breaks")?.cloned().unwrap_or_default().iter().enumerate() {
        let field = format!("breaks[{i}]");
        let (v, k) = (b.get("view").and_then(Value::as_u64), b.get("index").and_then(Value::as_u64));
        let (Some(v), Some(k)) = (v, k) else { return Err(OpError::param(&field, "{\"view\": id, \"index\": n}")) };
        if sheet.view(ViewId(v)).is_none_or(|x| (k as usize) >= x.breaks.len()) {
            return Err(OpError::param(&field, "no such break"));
        }
        sel.push(Sel::Break(ViewId(v), k as usize));
    }
    if sel.is_empty() {
        return Err(OpError::invalid("nothing to delete: give views, annotations, tables, images, hatches, sketch_entities or breaks"));
    }
    dt::delete_selection(&mut t.d.sheets[t.sheet], &sel);
    let n = sel.len() as u64;
    done(od, t, "Delete", "deleted", n)
}
